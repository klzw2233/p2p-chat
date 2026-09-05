//! REPL event loop: stdin, inbound accept, and Chat Session recv.

use std::future::Future;
use std::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};

use p2p_core::{DialHints, Endpoint, Error, Session};
use p2p_trust::{PeerId, TrustState, sas};
use rustyline_async::ReadlineEvent;

use crate::command::{HELP, UserInput, parse_line};
use crate::frame::{ChatMessage, read_frame, write_frame};
use crate::store::peer_id_hex;
use crate::ui::{self, Ui, prompt_for, trust_label};

/// Relay URLs to pass as DialHints (empty → `DialHints::none()`).
pub async fn run_repl(endpoint: Endpoint, relay_urls: Vec<String>) -> Result<(), Error> {
    let mut current = prompt_for(None);
    let (mut input, mut ui) = ui::init(&current);

    ui.raw(&format!("Peer ID: {}", peer_id_hex(endpoint.peer_id())));
    ui.system("Type /help for Slash Commands. Listening for inbound Chat Sessions.");

    let mut session: Option<Session> = None;
    type DialFuture<'a> = Pin<Box<dyn Future<Output = Result<Session, Error>> + Send + 'a>>;
    let mut dialing: Option<DialFuture<'_>> = None;

    loop {
        let want = prompt_for(remote_of(&endpoint, &session));
        if want != current {
            input.set_prompt(&want);
            current = want;
        }
        tokio::select! {
            event = input.next() => {
                match event {
                    Ok(ReadlineEvent::Line(line)) => {
                        input.remember(&line);
                        if handle_line(&endpoint, &relay_urls, &mut session, &mut dialing, &mut ui, &line).await {
                            break;
                        }
                    }
                    Ok(ReadlineEvent::Interrupted) => continue,
                    Ok(ReadlineEvent::Eof) => break,
                    Err(e) => {
                        ui.error(&format!("stdin: {e}"));
                        break;
                    }
                }
            }
            incoming = endpoint.accept(), if session.is_none() => {
                match incoming {
                    Ok(s) => on_session(&endpoint, &mut session, &mut ui, s, "inbound"),
                    Err(Error::Closed) => break,
                    Err(e) => ui.error(&format!("accept failed: {e}")),
                }
            }
            msg = recv_active(&mut session), if session.is_some() => {
                match msg {
                    Ok(Some(m)) => ui.recv(&m.text),
                    Ok(None) => {
                        ui.system("Remote Peer closed the Chat Session.");
                        session = None;
                    }
                    Err(e) => {
                        ui.error(&format!("recv failed: {e}"));
                        session = None;
                    }
                }
            }
            result = async { dialing.as_mut().unwrap().as_mut().await }, if dialing.is_some() => {
                dialing = None;
                match result {
                    Ok(s) => on_session(&endpoint, &mut session, &mut ui, s, "outbound"),
                    Err(e) => ui.error(&format!("dial failed: {e}")),
                }
            }
        }
    }

    input.flush();
    if let Some(s) = session.take() {
        s.close();
    }
    endpoint.close().await;
    Ok(())
}

fn remote_of(endpoint: &Endpoint, session: &Option<Session>) -> Option<(PeerId, TrustState)> {
    session.as_ref().map(|s| {
        let id = s.remote_peer_id();
        (id, trust_of(endpoint, &id))
    })
}

async fn recv_active(
    session: &mut Option<Session>,
) -> Result<Option<ChatMessage>, crate::frame::FrameError> {
    match session.as_mut() {
        Some(s) => read_frame(s).await,
        None => std::future::pending().await,
    }
}

/// Returns true if the REPL should quit.
async fn handle_line<'a>(
    endpoint: &'a Endpoint,
    relay_urls: &'a [String],
    session: &mut Option<Session>,
    dialing: &mut Option<Pin<Box<dyn Future<Output = Result<Session, Error>> + Send + 'a>>>,
    ui: &mut Ui,
    line: &str,
) -> bool {
    match parse_line(line) {
        Ok(None) => false,
        Ok(Some(UserInput::Quit)) => true,
        Ok(Some(input)) => {
            dispatch(endpoint, relay_urls, session, dialing, ui, input).await;
            false
        }
        Err(e) => {
            ui.error(&format!("{e}"));
            false
        }
    }
}

async fn dispatch<'a>(
    endpoint: &'a Endpoint,
    relay_urls: &'a [String],
    session: &mut Option<Session>,
    dialing: &mut Option<Pin<Box<dyn Future<Output = Result<Session, Error>> + Send + 'a>>>,
    ui: &mut Ui,
    input: UserInput,
) {
    match input {
        UserInput::Help => ui.raw(HELP),
        UserInput::Info => print_info(endpoint, session, ui),
        UserInput::Sas => print_sas(endpoint, session, ui),
        UserInput::Verify => verify(endpoint, session, ui),
        UserInput::Close => close_session(session, ui),
        UserInput::Dial(peer) => dial(endpoint, relay_urls, session, dialing, ui, peer),
        UserInput::Message(text) => send_text(session, ui, &text).await,
        UserInput::Quit => {}
    }
}

fn on_session(endpoint: &Endpoint, slot: &mut Option<Session>, ui: &mut Ui, s: Session, how: &str) {
    let remote = s.remote_peer_id();
    let hex = peer_id_hex(remote);
    let trust = trust_label(trust_of(endpoint, &remote));
    ui.system(&format!("Chat Session {how}: {hex} ({trust})"));
    *slot = Some(s);
}

fn print_info(endpoint: &Endpoint, session: &Option<Session>, ui: &mut Ui) {
    let mut text = format!("Local Peer ID: {}", peer_id_hex(endpoint.peer_id()));
    match session {
        None => text.push_str("\nChat Session: none"),
        Some(s) => {
            let remote = s.remote_peer_id();
            text.push_str(&format!("\nRemote Peer ID: {}", peer_id_hex(remote)));
            text.push_str(&format!(
                "\nTrust State: {}",
                trust_label(trust_of(endpoint, &remote))
            ));
        }
    }
    ui.raw(&text);
}

fn print_sas(endpoint: &Endpoint, session: &Option<Session>, ui: &mut Ui) {
    let Some(s) = session else {
        ui.error("no Chat Session");
        return;
    };
    let code = sas(
        endpoint.peer_id().public_key(),
        s.remote_peer_id().public_key(),
    );
    ui.raw(&format!("SAS Display: {code}"));
}

fn verify(endpoint: &Endpoint, session: &Option<Session>, ui: &mut Ui) {
    let Some(s) = session else {
        ui.error("no Chat Session");
        return;
    };
    let remote = s.remote_peer_id();
    match endpoint.mark_verified(remote) {
        Ok(_) => ui.system(&format!("Trust State: Verified ({})", peer_id_hex(remote))),
        Err(e) => ui.error(&format!("verify failed: {e}")),
    }
}

fn close_session(session: &mut Option<Session>, ui: &mut Ui) {
    match session.take() {
        Some(s) => {
            s.close();
            ui.system("Chat Session closed.");
        }
        None => ui.error("no Chat Session"),
    }
}

fn dial<'a>(
    endpoint: &'a Endpoint,
    relay_urls: &'a [String],
    session: &mut Option<Session>,
    dialing: &mut Option<Pin<Box<dyn Future<Output = Result<Session, Error>> + Send + 'a>>>,
    ui: &mut Ui,
    peer: PeerId,
) {
    if session.is_some() {
        ui.error("already in a Chat Session; /close first");
        return;
    }
    if dialing.is_some() {
        ui.error("already dialing; wait for completion or /quit");
        return;
    }
    let hints = if relay_urls.is_empty() {
        DialHints::none()
    } else {
        DialHints::relays(relay_urls.iter().cloned())
    };
    ui.system(&format!("dialing {}", peer_id_hex(peer)));

    *dialing = Some(Box::pin(endpoint.dial(peer, hints)));
}

async fn send_text(session: &mut Option<Session>, ui: &mut Ui, text: &str) {
    let Some(s) = session.as_mut() else {
        ui.error("no Chat Session; /dial or wait for inbound");
        return;
    };
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Err(e) = write_frame(s, &ChatMessage::new(text, ts)).await {
        ui.error(&format!("send failed: {e}"));
        *session = None;
    }
}

fn trust_of(endpoint: &Endpoint, peer: &PeerId) -> TrustState {
    endpoint.trust_state(peer).unwrap_or(TrustState::Unknown)
}
