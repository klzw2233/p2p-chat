//! REPL event loop: stdin, inbound accept, and Chat Session recv.

use std::time::{SystemTime, UNIX_EPOCH};

use p2p_core::{DialHints, Endpoint, Error, Session};
use p2p_trust::{PeerId, TrustState, sas};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::command::{HELP, UserInput, parse_line};
use crate::frame::{ChatMessage, read_frame, write_frame};
use crate::store::peer_id_hex;
use crate::ui::trust_label;

/// Relay URLs to pass as DialHints (empty → `DialHints::none()`).
pub async fn run_repl(endpoint: Endpoint, relay_urls: Vec<String>) -> Result<(), Error> {
    println!("Peer ID: {}", peer_id_hex(endpoint.peer_id()));
    println!("Type /help for Slash Commands. Listening for inbound Chat Sessions.");

    let mut stdin = BufReader::new(tokio::io::stdin()).lines();
    let mut session: Option<Session> = None;

    loop {
        tokio::select! {
            line = stdin.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if handle_line(&endpoint, &relay_urls, &mut session, &line).await {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        eprintln!("stdin: {e}");
                        break;
                    }
                }
            }
            incoming = endpoint.accept(), if session.is_none() => {
                match incoming {
                    Ok(s) => on_session(&endpoint, &mut session, s, "inbound"),
                    Err(Error::Closed) => break,
                    Err(e) => eprintln!("accept failed: {e}"),
                }
            }
            msg = recv_active(&mut session), if session.is_some() => {
                match msg {
                    Ok(Some(m)) => println!("< {}", m.text),
                    Ok(None) => {
                        println!("Remote Peer closed the Chat Session.");
                        session = None;
                    }
                    Err(e) => {
                        eprintln!("recv failed: {e}");
                        session = None;
                    }
                }
            }
        }
    }

    if let Some(s) = session.take() {
        s.close();
    }
    endpoint.close().await;
    Ok(())
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
async fn handle_line(
    endpoint: &Endpoint,
    relay_urls: &[String],
    session: &mut Option<Session>,
    line: &str,
) -> bool {
    match parse_line(line) {
        Ok(None) => false,
        Ok(Some(UserInput::Quit)) => true,
        Ok(Some(input)) => {
            dispatch(endpoint, relay_urls, session, input).await;
            false
        }
        Err(e) => {
            eprintln!("{e}");
            false
        }
    }
}

async fn dispatch(
    endpoint: &Endpoint,
    relay_urls: &[String],
    session: &mut Option<Session>,
    input: UserInput,
) {
    match input {
        UserInput::Help => println!("{HELP}"),
        UserInput::Info => print_info(endpoint, session),
        UserInput::Sas => print_sas(endpoint, session),
        UserInput::Verify => verify(endpoint, session),
        UserInput::Close => close_session(session),
        UserInput::Dial(peer) => dial(endpoint, relay_urls, session, peer).await,
        UserInput::Message(text) => send_text(session, &text).await,
        UserInput::Quit => {}
    }
}

fn on_session(endpoint: &Endpoint, slot: &mut Option<Session>, s: Session, how: &str) {
    let remote = s.remote_peer_id();
    let hex = peer_id_hex(remote);
    let trust = trust_label(trust_of(endpoint, &remote));
    println!("Chat Session {how}: {hex} ({trust})");
    *slot = Some(s);
}

fn print_info(endpoint: &Endpoint, session: &Option<Session>) {
    println!("Local Peer ID: {}", peer_id_hex(endpoint.peer_id()));
    match session {
        None => println!("Chat Session: none"),
        Some(s) => {
            let remote = s.remote_peer_id();
            println!("Remote Peer ID: {}", peer_id_hex(remote));
            println!("Trust State: {}", trust_label(trust_of(endpoint, &remote)));
        }
    }
}

fn print_sas(endpoint: &Endpoint, session: &Option<Session>) {
    let Some(s) = session else {
        eprintln!("no Chat Session");
        return;
    };
    let code = sas(
        endpoint.peer_id().public_key(),
        s.remote_peer_id().public_key(),
    );
    println!("SAS Display: {code}");
}

fn verify(endpoint: &Endpoint, session: &Option<Session>) {
    let Some(s) = session else {
        eprintln!("no Chat Session");
        return;
    };
    let remote = s.remote_peer_id();
    match endpoint.mark_verified(remote) {
        Ok(_) => println!("Trust State: Verified ({})", peer_id_hex(remote)),
        Err(e) => eprintln!("verify failed: {e}"),
    }
}

fn close_session(session: &mut Option<Session>) {
    match session.take() {
        Some(s) => {
            s.close();
            println!("Chat Session closed.");
        }
        None => eprintln!("no Chat Session"),
    }
}

async fn dial(
    endpoint: &Endpoint,
    relay_urls: &[String],
    session: &mut Option<Session>,
    peer: PeerId,
) {
    if session.is_some() {
        eprintln!("already in a Chat Session; /close first");
        return;
    }
    let hints = if relay_urls.is_empty() {
        DialHints::none()
    } else {
        DialHints::relays(relay_urls.iter().cloned())
    };
    match endpoint.dial(peer, hints).await {
        Ok(s) => on_session(endpoint, session, s, "outbound"),
        Err(e) => eprintln!("dial failed: {e}"),
    }
}

async fn send_text(session: &mut Option<Session>, text: &str) {
    let Some(s) = session.as_mut() else {
        eprintln!("no Chat Session; /dial or wait for inbound");
        return;
    };
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Err(e) = write_frame(s, &ChatMessage::new(text, ts)).await {
        eprintln!("send failed: {e}");
        *session = None;
    }
}

fn trust_of(endpoint: &Endpoint, peer: &PeerId) -> TrustState {
    endpoint.trust_state(peer).unwrap_or(TrustState::Unknown)
}
