use std::process::Command;

#[test]
fn temp_mode_prints_64_char_hex_peer_id() {
    let out = Command::new(env!("CARGO_BIN_EXE_p2p-chat"))
        .arg("--temp")
        .output()
        .expect("run p2p-chat --temp");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let hex = stdout
        .split_whitespace()
        .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .expect("stdout should contain a 64-char hex Peer ID");
    assert_eq!(hex.len(), 64);
}

fn data_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "p2p-chat-bin-{}-{}-{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn peer_id_from_output(out: &std::process::Output) -> String {
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .expect("stdout should contain a 64-char hex Peer ID")
        .to_string()
}

#[test]
fn persistent_mode_prints_same_peer_id_after_restart() {
    let dir = data_dir("persist");
    let bin = env!("CARGO_BIN_EXE_p2p-chat");
    let first = Command::new(bin)
        .args([
            "--data-dir",
            dir.to_str().unwrap(),
            "--password",
            "correct-horse",
        ])
        .output()
        .expect("first persist run");
    let id1 = peer_id_from_output(&first);

    let second = Command::new(bin)
        .args([
            "--data-dir",
            dir.to_str().unwrap(),
            "--password",
            "correct-horse",
        ])
        .output()
        .expect("second persist run");
    let id2 = peer_id_from_output(&second);
    assert_eq!(id1, id2);

    let _ = std::fs::remove_file(dir.join("identity.key"));
    let _ = std::fs::remove_file(dir.join("trust.store"));
    let _ = std::fs::remove_dir(&dir);
}
