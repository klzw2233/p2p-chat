use clap::Parser;
use p2p_chat::cli::Args;
use p2p_chat::store::{bind_endpoint, peer_id_hex};

#[tokio::main]
async fn main() {
    let mut args = Args::parse();
    if let Err(e) = resolve_password(&mut args) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    match bind_endpoint(&args).await {
        Ok(ep) => {
            println!("Peer ID: {}", peer_id_hex(ep.peer_id()));
            ep.close().await;
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
