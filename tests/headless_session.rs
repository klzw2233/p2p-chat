use p2p_chat::frame::{ChatMessage, encode_frame};

#[test]
fn framed_bytes_are_length_prefixed_json() {
    let msg = ChatMessage::new("hi", 42);
    let bytes = encode_frame(&msg).unwrap();
    let len = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
    assert_eq!(len, bytes.len() - 4);
    let parsed: ChatMessage = serde_json::from_slice(&bytes[4..]).unwrap();
    assert_eq!(parsed, msg);
}
