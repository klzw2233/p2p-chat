use clap::Parser;
use p2p_chat::cli::Args;
use p2p_chat::store::bind_endpoint;

#[tokio::main]
async fn main() {
    let mut args = Args::parse();
    if let Err(e) = resolve_password(&mut args) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    let relay_urls = if args.n0_public {
        n0_public_relay_urls()
    } else {
        args.relay.iter().cloned().collect()
    };
    match bind_endpoint(&args).await {
        Ok(ep) => {
            if let Err(e) = p2p_chat::app::run_repl(ep, relay_urls).await {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

fn resolve_password(args: &mut Args) -> std::io::Result<()> {
    if args.data_dir.is_some() && args.password.is_none() {
        args.password = Some(rpassword::prompt_password("Password: ")?);
    }
    Ok(())
}

/// iroh 1.1.0 production n0 relays (`iroh::defaults::prod`).
fn n0_public_relay_urls() -> Vec<String> {
    [
        "https://use1-1.relay.n0.iroh.link.",
        "https://usw1-1.relay.n0.iroh.link.",
        "https://euc1-1.relay.n0.iroh.link.",
        "https://aps1-1.relay.n0.iroh.link.",
    ]
    .map(str::to_string)
    .into()
}
