//! Terminal presentation layer: output sinks, prompt rendering, dual-mode input.
//!
//! Output and input are separate types on purpose. [`Readline::readline`] is held
//! as a live future inside `tokio::select!` while output handlers write
//! concurrently, so the two must be separately borrowable.

use std::io::{IsTerminal, Write};

use p2p_trust::{PeerId, TrustState};
use rustyline_async::{Readline, ReadlineError, ReadlineEvent};
use tokio::io::{AsyncBufReadExt, BufReader, Lines, Stdin};

use crate::store::peer_id_hex;

/// Characters of the 64-char hex Peer ID shown in the prompt.
const SHORT_LEN: usize = 8;

/// Label for a Trust State, shared by `/info` and the prompt.
pub fn trust_label(state: TrustState) -> &'static str {
    match state {
        TrustState::Verified => "Verified",
        TrustState::Tofu => "TOFU",
        TrustState::Unknown => "unknown",
    }
}

/// Prompt string for the current Chat Session, or `[idle]> ` when there is none.
pub fn prompt_for(remote: Option<(PeerId, TrustState)>) -> String {
    match remote {
        None => "[idle]> ".to_string(),
        Some((id, state)) => format!(
            "[{}|{}]> ",
            &peer_id_hex(id)[..SHORT_LEN],
            trust_label(state)
        ),
    }
}

/// Build the input driver and output sinks for this process.
///
/// Takes the interactive path only when stdin *and* stdout are both terminals:
/// `rustyline-async` renders to stdout, so `p2p-chat > log.txt` from a terminal
/// must still stream plainly. Falls back to plain mode if raw mode cannot be
/// entered rather than failing startup.
///
/// `Readline::new` enables raw mode before it queries the terminal size, and
/// only owns a droppable `Readline` after that query. A `size()` failure in
/// that window would leave raw mode on with nothing to restore it. Undefended:
/// reaching it needs the tty to vanish between two consecutive calls on it,
/// and a tty that is gone cannot be left unusable.
pub fn init(prompt: &str) -> (Input, Ui) {
    if std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal()
        && let Ok((rl, writer)) = Readline::new(prompt.to_string())
    {
        return (
            Input::Tty(rl),
            Ui {
                out: Box::new(writer.clone()),
                err: Box::new(writer),
            },
        );
    }
    (
        Input::Plain(BufReader::new(tokio::io::stdin()).lines()),
        Ui {
            out: Box::new(std::io::stdout()),
            err: Box::new(std::io::stderr()),
        },
    )
}

/// Output sinks. In TTY mode both are clones of one
/// [`SharedWriter`](rustyline_async::SharedWriter): everything must funnel
/// through it or it overwrites the prompt and the in-progress draft. In plain
/// mode errors stay on stderr, where scripts expect them.
pub struct Ui {
    out: Box<dyn Write + Send>,
    err: Box<dyn Write + Send>,
}

impl Ui {
    /// A message from the Remote Peer.
    pub fn recv(&mut self, text: &str) {
        // Remote-controlled text reaching a raw-mode terminal: neutralise control
        // characters so a Remote Peer cannot move the cursor, clear the screen,
        // or forge a `< ` line via embedded newlines.
        let safe: String = text
            .chars()
            .map(|c| if c.is_control() { '\u{fffd}' } else { c })
            .collect();
        self.line(Sink::Out, &format!("< {safe}"));
    }

    /// A local lifecycle event.
    pub fn system(&mut self, msg: &str) {
        self.line(Sink::Out, &format!("* {msg}"));
    }

    /// A failed command or a recoverable error.
    pub fn error(&mut self, err: &str) {
        self.line(Sink::Err, &format!("! {err}"));
    }

    /// Unadorned output, for multi-line command results such as `/help`.
    pub fn raw(&mut self, text: &str) {
        self.line(Sink::Out, text);
    }

    /// One `write_all` per call: `SharedWriter` only forwards its buffer when a
    /// write ends in a newline, and its `flush` is a no-op.
    fn line(&mut self, sink: Sink, text: &str) {
        let target = match sink {
            Sink::Out => &mut self.out,
            Sink::Err => &mut self.err,
        };
        // `SharedWriter` appends before it tries to send, so a full channel
        // (500 unrendered lines, only reachable while a handler blocks the
        // loop) returns `WouldBlock` with the bytes still buffered: this line
        // is deferred onto the next one, not lost. Nothing to recover here.
        let _ = target.write_all(format!("{text}\n").as_bytes());
    }
}

enum Sink {
    Out,
    Err,
}

/// Line source: an async line editor on a terminal, plain buffered lines otherwise.
pub enum Input {
    Tty(Readline),
    Plain(Lines<BufReader<Stdin>>),
}

impl Input {
    /// Next input event. Plain mode reports end of stdin as [`ReadlineEvent::Eof`],
    /// the same event Ctrl-D produces, so callers need one shutdown path.
    pub async fn next(&mut self) -> Result<ReadlineEvent, ReadlineError> {
        match self {
            Input::Tty(rl) => rl.readline().await,
            Input::Plain(lines) => match lines.next_line().await {
                Ok(Some(line)) => Ok(ReadlineEvent::Line(line)),
                Ok(None) => Ok(ReadlineEvent::Eof),
                Err(e) => Err(ReadlineError::IO(e)),
            },
        }
    }

    /// Redraw with a new prompt. No-op in plain mode, which shows no prompt.
    pub fn set_prompt(&mut self, prompt: &str) {
        if let Input::Tty(rl) = self {
            let _ = rl.update_prompt(prompt);
        }
    }

    /// Record a line for Up/Down recall. In-process only; nothing is written to disk.
    pub fn remember(&mut self, line: &str) {
        if let Input::Tty(rl) = self {
            rl.add_history_entry(line.to_string());
        }
    }

    /// Drain buffered output and erase the prompt. Call once before teardown.
    pub fn flush(&mut self) {
        if let Input::Tty(rl) = self {
            let _ = rl.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_trust::IdentityKey;

    fn sample() -> (PeerId, String) {
        let id = IdentityKey::generate().peer_id();
        (id, peer_id_hex(id))
    }

    #[test]
    fn idle_prompt_has_no_peer() {
        assert_eq!(prompt_for(None), "[idle]> ");
    }

    #[test]
    fn session_prompt_shows_short_peer_and_trust_label() {
        let (id, hex) = sample();
        for (state, label) in [
            (TrustState::Tofu, "TOFU"),
            (TrustState::Verified, "Verified"),
            (TrustState::Unknown, "unknown"),
        ] {
            assert_eq!(
                prompt_for(Some((id, state))),
                format!("[{}|{}]> ", &hex[..SHORT_LEN], label)
            );
        }
    }

    #[test]
    fn short_peer_id_is_eight_chars_and_a_prefix_of_the_full_hex() {
        let (id, hex) = sample();
        let prompt = prompt_for(Some((id, TrustState::Tofu)));
        let short = prompt
            .trim_start_matches('[')
            .split('|')
            .next()
            .expect("prompt has a peer segment");
        assert_eq!(short.len(), SHORT_LEN);
        assert!(hex.starts_with(short), "{short} should prefix {hex}");
    }

    /// Capture what a sink receives, so formatting is asserted on bytes rather
    /// than on a terminal.
    #[derive(Clone, Default)]
    struct Buf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl Write for Buf {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Buf {
        fn taken(&self) -> String {
            String::from_utf8(std::mem::take(&mut *self.0.lock().unwrap())).unwrap()
        }
    }

    fn captured() -> (Ui, Buf, Buf) {
        let (out, err) = (Buf::default(), Buf::default());
        let ui = Ui {
            out: Box::new(out.clone()),
            err: Box::new(err.clone()),
        };
        (ui, out, err)
    }

    #[test]
    fn each_kind_is_prefixed_and_newline_terminated() {
        let (mut ui, out, err) = captured();
        ui.recv("hi");
        ui.system("Chat Session inbound");
        ui.raw("line one\nline two");
        assert_eq!(
            out.taken(),
            "< hi\n* Chat Session inbound\nline one\nline two\n"
        );
        ui.error("no Chat Session");
        assert_eq!(err.taken(), "! no Chat Session\n");
    }

    #[test]
    fn errors_do_not_reach_the_output_sink() {
        let (mut ui, out, _err) = captured();
        ui.error("dial failed");
        assert_eq!(out.taken(), "");
    }

    #[test]
    fn recv_neutralises_remote_control_characters() {
        let (mut ui, out, _err) = captured();
        ui.recv("evil\x1b[2Jwipe\nfake");
        assert_eq!(out.taken(), "< evil\u{fffd}[2Jwipe\u{fffd}fake\n");
    }

    #[test]
    fn recv_leaves_ordinary_text_alone() {
        let (mut ui, out, _err) = captured();
        ui.recv("héllo 世界 🙂");
        assert_eq!(out.taken(), "< héllo 世界 🙂\n");
    }
}
