use p2p_core::{Endpoint, Error, RelayConfig};
use p2p_trust::{
    FileKeyStore, FileTrustStore, IdentityKey, KeyStore, MemoryKeyStore, MemoryTrustStore, PeerId,
};

use crate::cli::Args;

/// Bind a local Endpoint from CLI flags. Does not start REPL.
///
/// `--temp`, or neither `--temp` nor `--data-dir`, uses in-memory stores.
pub async fn bind_endpoint(args: &Args) -> Result<Endpoint, Error> {
    let relay = relay_config(args)?;
    match args.data_dir.as_ref() {
        Some(dir) => {
            let password = args.password.as_deref().unwrap_or("");
            let mut keys = FileKeyStore::new(dir, password.as_bytes());
            let identity = match keys.load().map_err(Error::Trust)? {
                Some(id) => id,
                None => {
                    let id = IdentityKey::generate();
                    keys.save(&id).map_err(Error::Trust)?;
                    id
                }
            };
            let trust = FileTrustStore::open(dir, identity).map_err(Error::Trust)?;
            Endpoint::bind(&mut keys, Box::new(trust), relay).await
        }
        None => {
            let mut keys = MemoryKeyStore::new();
            Endpoint::bind(&mut keys, Box::new(MemoryTrustStore::new()), relay).await
        }
    }
}

/// 64-character lowercase hex Peer ID.
pub fn peer_id_hex(id: PeerId) -> String {
    id.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

/// Parse a 64-character hex Peer ID. Case-insensitive.
pub fn peer_id_from_hex(hex: &str) -> Option<PeerId> {
    if hex.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).ok()?;
        bytes[i] = u8::from_str_radix(s, 16).ok()?;
    }
    PeerId::from_bytes(bytes).ok()
}

fn relay_config(args: &Args) -> Result<RelayConfig, Error> {
    if args.n0_public {
        Ok(RelayConfig::n0_public())
    } else if let Some(url) = &args.relay {
        RelayConfig::custom([url.as_str()])
    } else {
        Ok(RelayConfig::disabled())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_trust::IdentityKey;

    #[test]
    fn peer_id_hex_round_trips() {
        let id = IdentityKey::generate().peer_id();
        let hex = peer_id_hex(id);
        assert_eq!(hex.len(), 64);
        assert_eq!(peer_id_from_hex(&hex), Some(id));
        assert_eq!(peer_id_from_hex(&hex.to_uppercase()), Some(id));
        assert!(peer_id_from_hex("zz").is_none());
        assert!(peer_id_from_hex(&hex[..63]).is_none());
    }
}
