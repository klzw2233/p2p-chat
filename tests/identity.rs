use p2p_chat::cli::Args;
use p2p_chat::store::bind_endpoint;

fn data_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "p2p-chat-{}-{}-{}",
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

fn persist_args(dir: &std::path::Path) -> Args {
    Args {
        data_dir: Some(dir.to_path_buf()),
        temp: false,
        password: Some("correct-horse".into()),
        relay: None,
        n0_public: false,
    }
}

#[tokio::test]
async fn persistent_mode_keeps_peer_id_across_restarts() {
    let dir = data_dir("persist");
    let args = persist_args(&dir);
    let ep1 = bind_endpoint(&args).await.unwrap();
    let id = ep1.peer_id();
    ep1.close().await;

    let ep2 = bind_endpoint(&args).await.unwrap();
    assert_eq!(ep2.peer_id(), id);
    let hex = p2p_chat::store::peer_id_hex(id);
    assert_eq!(hex.len(), 64);
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    ep2.close().await;
    let _ = std::fs::remove_file(dir.join("identity.key"));
    let _ = std::fs::remove_file(dir.join("trust.store"));
    let _ = std::fs::remove_dir(&dir);
}

#[tokio::test]
async fn wrong_password_fails_to_bind() {
    let dir = data_dir("wrong-pw");
    let args = persist_args(&dir);
    let ep = bind_endpoint(&args).await.unwrap();
    ep.close().await;

    let mut bad = persist_args(&dir);
    bad.password = Some("wrong-battery".into());
    let err = bind_endpoint(&bad)
        .await
        .err()
        .expect("wrong password must fail");
    assert!(
        matches!(
            err,
            p2p_core::Error::UnlockFailed | p2p_core::Error::Trust(_)
        ),
        "got {err:?}"
    );
    let _ = std::fs::remove_file(dir.join("identity.key"));
    let _ = std::fs::remove_file(dir.join("trust.store"));
    let _ = std::fs::remove_dir(&dir);
}

fn temp_args() -> Args {
    Args {
        data_dir: None,
        temp: true,
        password: None,
        relay: None,
        n0_public: false,
    }
}

#[tokio::test]
async fn temp_mode_binds_without_writing_files() {
    let cwd = data_dir("temp-cwd");
    let prev = std::env::current_dir().unwrap();
    std::env::set_current_dir(&cwd).unwrap();

    let ep = bind_endpoint(&temp_args()).await.unwrap();
    let hex = p2p_chat::store::peer_id_hex(ep.peer_id());
    assert_eq!(hex.len(), 64);
    ep.close().await;

    let entries: Vec<_> = std::fs::read_dir(&cwd)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    std::env::set_current_dir(prev).unwrap();
    assert!(
        entries.is_empty(),
        "--temp must not write files, got {entries:?}"
    );
    let _ = std::fs::remove_dir(&cwd);
}
