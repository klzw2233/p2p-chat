use clap::Parser;
use p2p_chat::cli::Args;

#[test]
fn parses_data_dir_password_and_relay() {
    let args = Args::try_parse_from([
        "p2p-chat",
        "--data-dir",
        "/tmp/p2p-data",
        "--password",
        "secret",
        "--relay",
        "https://relay.example",
    ])
    .unwrap();
    assert_eq!(
        args.data_dir.as_deref(),
        Some(std::path::Path::new("/tmp/p2p-data"))
    );
    assert_eq!(args.password.as_deref(), Some("secret"));
    assert_eq!(args.relay.as_deref(), Some("https://relay.example"));
    assert!(!args.temp);
    assert!(!args.n0_public);
}

#[test]
fn no_flags_defaults_to_ephemeral() {
    let args = Args::try_parse_from(["p2p-chat"]).unwrap();
    assert!(!args.temp);
    assert!(args.data_dir.is_none());
    assert!(args.password.is_none());
    assert!(args.relay.is_none());
    assert!(!args.n0_public);
}

#[test]
fn parses_temp_and_n0_public() {
    let args = Args::try_parse_from(["p2p-chat", "--temp", "--n0-public"]).unwrap();
    assert!(args.temp);
    assert!(args.n0_public);
    assert!(args.data_dir.is_none());
    assert!(args.relay.is_none());
}

#[test]
fn data_dir_conflicts_with_temp() {
    assert!(Args::try_parse_from(["p2p-chat", "--temp", "--data-dir", "/tmp/x"]).is_err());
}

#[test]
fn relay_conflicts_with_n0_public() {
    assert!(
        Args::try_parse_from([
            "p2p-chat",
            "--temp",
            "--relay",
            "https://relay.example",
            "--n0-public"
        ])
        .is_err()
    );
}
