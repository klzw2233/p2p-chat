//! Slash Command and chat-line parser for the REPL.

use p2p_trust::PeerId;

use crate::store::peer_id_from_hex;

/// One line of REPL input after parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserInput {
    Message(String),
    Dial(PeerId),
    Sas,
    Verify,
    Info,
    Close,
    Help,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    UnknownCommand(String),
    DialUsage,
    ExtraArgs(&'static str),
    InvalidPeerId,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnknownCommand(cmd) => {
                write!(f, "unknown command: /{cmd} (try /help)")
            }
            ParseError::DialUsage => f.write_str("usage: /dial <64-hex-peer-id>"),
            ParseError::ExtraArgs(cmd) => write!(f, "/{cmd} takes no arguments"),
            ParseError::InvalidPeerId => f.write_str("invalid Peer ID"),
        }
    }
}

/// Parse one trimmed REPL line.
///
/// Empty input is `Ok(None)` (ignore). Slash lines are commands; anything else
/// is chat text.
pub fn parse_line(line: &str) -> Result<Option<UserInput>, ParseError> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(None);
    }
    let Some(rest) = line.strip_prefix('/') else {
        return Ok(Some(UserInput::Message(line.to_string())));
    };
    let (cmd, arg) = match rest.split_once(char::is_whitespace) {
        Some((cmd, arg)) => (cmd, arg.trim()),
        None => (rest, ""),
    };
    match cmd {
        "dial" => {
            if arg.is_empty() {
                return Err(ParseError::DialUsage);
            }
            let peer = peer_id_from_hex(arg).ok_or(ParseError::InvalidPeerId)?;
            Ok(Some(UserInput::Dial(peer)))
        }
        "sas" => no_arg("sas", arg, UserInput::Sas),
        "verify" => no_arg("verify", arg, UserInput::Verify),
        "info" => no_arg("info", arg, UserInput::Info),
        "close" => no_arg("close", arg, UserInput::Close),
        "help" => no_arg("help", arg, UserInput::Help),
        "quit" | "exit" => no_arg("quit", arg, UserInput::Quit),
        other => Err(ParseError::UnknownCommand(other.to_string())),
    }
}

fn no_arg(cmd: &'static str, arg: &str, value: UserInput) -> Result<Option<UserInput>, ParseError> {
    if arg.is_empty() {
        Ok(Some(value))
    } else {
        Err(ParseError::ExtraArgs(cmd))
    }
}

pub const HELP: &str = "\
/dial <peer-id>  connect to a Remote Peer (64 hex)
/sas             show SAS Display
/verify          mark Remote Peer Verified
/info            Chat Session status and Trust State
/close           end Chat Session
/help            show this help
/quit, /exit     exit";

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_trust::IdentityKey;

    fn sample_peer() -> (PeerId, String) {
        let id = IdentityKey::generate().peer_id();
        (id, crate::store::peer_id_hex(id))
    }

    #[test]
    fn empty_and_whitespace_are_ignored() {
        assert_eq!(parse_line(""), Ok(None));
        assert_eq!(parse_line("   \t  "), Ok(None));
    }

    #[test]
    fn plain_text_is_a_chat_message() {
        assert_eq!(
            parse_line("hello there"),
            Ok(Some(UserInput::Message("hello there".into())))
        );
        assert_eq!(
            parse_line("  dial me  "),
            Ok(Some(UserInput::Message("dial me".into())))
        );
    }

    #[test]
    fn slash_commands_without_args() {
        assert_eq!(parse_line("/sas"), Ok(Some(UserInput::Sas)));
        assert_eq!(parse_line("  /verify  "), Ok(Some(UserInput::Verify)));
        assert_eq!(parse_line("/info"), Ok(Some(UserInput::Info)));
        assert_eq!(parse_line("/close"), Ok(Some(UserInput::Close)));
        assert_eq!(parse_line("/help"), Ok(Some(UserInput::Help)));
        assert_eq!(parse_line("/quit"), Ok(Some(UserInput::Quit)));
        assert_eq!(parse_line("/exit"), Ok(Some(UserInput::Quit)));
    }

    #[test]
    fn extra_args_on_simple_commands_are_rejected() {
        assert_eq!(parse_line("/sas extra"), Err(ParseError::ExtraArgs("sas")));
        assert_eq!(parse_line("/quit now"), Err(ParseError::ExtraArgs("quit")));
    }

    #[test]
    fn unknown_slash_command_is_an_error() {
        assert_eq!(
            parse_line("/foo"),
            Err(ParseError::UnknownCommand("foo".into()))
        );
        assert_eq!(
            parse_line("/"),
            Err(ParseError::UnknownCommand(String::new()))
        );
    }

    #[test]
    fn dial_requires_a_64_char_hex_peer_id() {
        assert_eq!(parse_line("/dial"), Err(ParseError::DialUsage));
        assert_eq!(parse_line("/dial not-hex"), Err(ParseError::InvalidPeerId));
        assert_eq!(parse_line("/dial aabbcc"), Err(ParseError::InvalidPeerId));
    }

    #[test]
    fn dial_accepts_generated_peer_id_hex() {
        let (peer, hex) = sample_peer();
        assert_eq!(
            parse_line(&format!("/dial {hex}")),
            Ok(Some(UserInput::Dial(peer)))
        );
        assert_eq!(
            parse_line(&format!("/dial  {hex}  ")),
            Ok(Some(UserInput::Dial(peer)))
        );
        assert_eq!(
            parse_line(&format!("/dial {}", hex.to_uppercase())),
            Ok(Some(UserInput::Dial(peer)))
        );
    }
}
