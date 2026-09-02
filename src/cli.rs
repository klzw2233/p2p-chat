use std::path::PathBuf;

use clap::Parser;

/// Startup flags for p2p-chat. REPL is Ticket 3.
#[derive(Parser, Debug)]
#[command(name = "p2p-chat")]
pub struct Args {
    /// Persistent store directory (`identity.key` + `trust.store`).
    #[arg(long, conflicts_with = "temp")]
    pub data_dir: Option<PathBuf>,

    /// In-memory Identity Key; do not write files.
    #[arg(long, conflicts_with = "data_dir")]
    pub temp: bool,

    /// Password for FileKeyStore. Also reads `P2P_PASSWORD`.
    #[arg(long, env = "P2P_PASSWORD")]
    pub password: Option<String>,

    /// Custom Relay URL.
    #[arg(long, conflicts_with = "n0_public")]
    pub relay: Option<String>,

    /// Opt in to n0 public relays.
    #[arg(long, conflicts_with = "relay")]
    pub n0_public: bool,
}
