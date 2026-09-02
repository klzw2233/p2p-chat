//! Length-prefixed JSON message frame codec for `p2p-chat`.
//!
//! Wire format (ADR-0001): `[u32 BE length][JSON ChatMessage]`.

use p2p_core::Session;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum payload size allowed for a single frame (1 MB).
pub const MAX_FRAME_PAYLOAD_SIZE: usize = 1024 * 1024;

/// Application chat message payload.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub text: String,
    pub timestamp: u64,
}

impl ChatMessage {
    pub fn new(text: impl Into<String>, timestamp: u64) -> Self {
        Self {
            text: text.into(),
            timestamp,
        }
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum FrameError {
    #[error("session I/O failed")]
    Session,
    #[error("frame length exceeds maximum allowed limit ({size} > {max})")]
    FrameTooLarge { size: usize, max: usize },
    #[error("failed to serialize/deserialize frame JSON")]
    Json,
    #[error("unexpected EOF while reading frame")]
    UnexpectedEof,
}

impl From<p2p_core::Error> for FrameError {
    fn from(_: p2p_core::Error) -> Self {
        FrameError::Session
    }
}

impl From<serde_json::Error> for FrameError {
    fn from(_: serde_json::Error) -> Self {
        FrameError::Json
    }
}

/// Encode `msg` as a length-prefixed JSON frame.
pub fn encode_frame(msg: &ChatMessage) -> Result<Vec<u8>, FrameError> {
    let payload = serde_json::to_vec(msg)?;
    if payload.len() > MAX_FRAME_PAYLOAD_SIZE {
        return Err(FrameError::FrameTooLarge {
            size: payload.len(),
            max: MAX_FRAME_PAYLOAD_SIZE,
        });
    }
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(&payload);
    Ok(buf)
}

/// Decode a complete length-prefixed JSON frame from `bytes`.
///
/// `bytes` must contain the 4-byte length prefix plus the full payload.
pub fn decode_frame(bytes: &[u8]) -> Result<ChatMessage, FrameError> {
    if bytes.len() < 4 {
        return Err(FrameError::UnexpectedEof);
    }
    let payload_len = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
    if payload_len > MAX_FRAME_PAYLOAD_SIZE {
        return Err(FrameError::FrameTooLarge {
            size: payload_len,
            max: MAX_FRAME_PAYLOAD_SIZE,
        });
    }
    let payload = bytes
        .get(4..4 + payload_len)
        .ok_or(FrameError::UnexpectedEof)?;
    Ok(serde_json::from_slice(payload)?)
}

/// Asynchronously write a length-prefixed JSON frame over the given Session.
pub async fn write_frame(session: &mut Session, msg: &ChatMessage) -> Result<(), FrameError> {
    session.send(&encode_frame(msg)?).await?;
    Ok(())
}

/// Asynchronously read a length-prefixed JSON frame from the given Session.
///
/// Returns `Ok(None)` if the Session was closed gracefully by the peer.
pub async fn read_frame(session: &mut Session) -> Result<Option<ChatMessage>, FrameError> {
    let mut len_buf = [0u8; 4];
    let mut read_bytes = 0;
    while read_bytes < 4 {
        let n = session.recv(&mut len_buf[read_bytes..]).await?;
        if n == 0 {
            if read_bytes == 0 {
                return Ok(None);
            }
            return Err(FrameError::UnexpectedEof);
        }
        read_bytes += n;
    }

    let payload_len = u32::from_be_bytes(len_buf) as usize;
    if payload_len > MAX_FRAME_PAYLOAD_SIZE {
        return Err(FrameError::FrameTooLarge {
            size: payload_len,
            max: MAX_FRAME_PAYLOAD_SIZE,
        });
    }

    let mut payload = vec![0u8; payload_len];
    session.recv_exact(&mut payload).await?;
    Ok(Some(serde_json::from_slice(&payload)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_round_trip() {
        let msg = ChatMessage::new("hello", 1_700_000_000);
        let bytes = encode_frame(&msg).unwrap();
        assert_eq!(&bytes[..4], &(bytes.len() as u32 - 4).to_be_bytes());
        assert_eq!(decode_frame(&bytes).unwrap(), msg);
    }

    #[test]
    fn empty_text_round_trips() {
        let msg = ChatMessage::new("", 0);
        let bytes = encode_frame(&msg).unwrap();
        assert_eq!(decode_frame(&bytes).unwrap(), msg);
    }

    #[test]
    fn decode_rejects_truncated_header() {
        assert_eq!(decode_frame(&[0, 0, 1]), Err(FrameError::UnexpectedEof));
    }

    #[test]
    fn decode_rejects_truncated_payload() {
        let mut bytes = encode_frame(&ChatMessage::new("hello", 1)).unwrap();
        bytes.pop();
        assert_eq!(decode_frame(&bytes), Err(FrameError::UnexpectedEof));
    }

    #[test]
    fn decode_rejects_claimed_oversize() {
        let mut bytes = 2_000_000u32.to_be_bytes().to_vec();
        bytes.extend_from_slice(&[0; 8]);
        assert_eq!(
            decode_frame(&bytes),
            Err(FrameError::FrameTooLarge {
                size: 2_000_000,
                max: MAX_FRAME_PAYLOAD_SIZE
            })
        );
    }

    #[test]
    fn encode_rejects_oversize_payload() {
        let msg = ChatMessage::new("x".repeat(MAX_FRAME_PAYLOAD_SIZE + 10), 1);
        assert!(matches!(
            encode_frame(&msg),
            Err(FrameError::FrameTooLarge { .. })
        ));
    }

    #[test]
    fn decode_rejects_invalid_json() {
        let payload = b"not-json";
        let mut bytes = (payload.len() as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(payload);
        assert_eq!(decode_frame(&bytes), Err(FrameError::Json));
    }
}
