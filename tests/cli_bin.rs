use std::io::Write;
use std::process::{Command, Stdio};

fn run_with_stdin(args: &[&str], stdin: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_p2p-chat"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn p2p-chat");
    {
        let mut stdin_h = child.stdin.take().expect("stdin");
        stdin_h.write_all(stdin).expect("write stdin");
    }
    child.wait_with_output().expect("wait p2p-chat")
}

fn run_until_quit(args: &[&str]) -> std::process::Output {
    run_with_stdin(args, b"/quit\n")
}

#[test]
fn temp_mode_prints_64_char_hex_peer_id() {
    let out = run_until_quit(&["--temp"]);
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
    let persist = [
        "--data-dir",
        dir.to_str().unwrap(),
        "--password",
        "correct-horse",
    ];
    let first = run_until_quit(&persist);
    let id1 = peer_id_from_output(&first);
    let second = run_until_quit(&persist);
    let id2 = peer_id_from_output(&second);
    assert_eq!(id1, id2);

    let _ = std::fs::remove_file(dir.join("identity.key"));
    let _ = std::fs::remove_file(dir.join("trust.store"));
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn repl_slash_commands_work_without_a_session() {
    let out = run_with_stdin(
        &["--temp"],
        b"/help\n/info\n/sas\n/verify\n/close\nhello\n/quit\n",
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stdout.contains("/dial"), "help missing: {stdout}");
    assert!(
        stdout.contains("Chat Session: none"),
        "info missing: {stdout}"
    );
    assert!(
        stderr.contains("no Chat Session"),
        "sas/verify/close should complain: {stderr}"
    );
}
