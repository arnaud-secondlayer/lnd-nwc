use crate::config::Config;

use confy;
use hex;
use secp256k1::rand::{RngCore, rngs::OsRng};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use urlencoding::encode;

// Config is stored in
// Linux: $XDG_CONFIG_HOME/<project_path> or $HOME/.config/<project_path>
// OSX: $HOME/.config/<project_path>
// Windows: {FOLDERID_RoamingAppData}/<project_path>/config

pub fn load_and_display() -> Result<(), Box<dyn std::error::Error>> {
    let cfg: Config = confy::load("lnd-nwc", None)?;

    println!("Names and URIs:");
    if cfg.uris.is_empty() {
        println!("\tEmpty");
    } else {
        for uri in cfg.uris {
            println!("\t{}: {}", uri.0, uri.1);
        }
    }
    println!("");

    Ok(())
}

pub fn create_and_save(name: &str, relay: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg: Config = confy::load("lnd-nwc", None)?;
    if cfg.uris.contains_key(name) {
        panic!("Uri name `{name}`already exists, remove it or use another one")
    }

    let new_uri = create_uri(relay);
    let _ = &cfg.uris.insert(name.into(), new_uri.clone());

    confy::store("lnd-nwc", None, cfg)?;

    println!("New URI created for {name}:\n{new_uri}");

    Ok(())
}

pub fn remove_and_save(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg: Config = confy::load("lnd-nwc", None)?;
    if cfg.uris.contains_key(name) == false {
        panic!("Uri name `{name}` does not exist")
    }

    let _ = &cfg.uris.remove(name);

    confy::store("lnd-nwc", None, cfg)?;

    println!("Removed URI for {name}");

    Ok(())
}

pub fn create_uri(relay: &str) -> String {
    format!(
        "nostr+walletconnect://{}?relay={}&secret={}",
        generate_nostr_pubkey(),
        encode(relay),
        generate_secret()
    )
}

fn generate_nostr_pubkey() -> String {
    let secp = Secp256k1::new();
    let mut rng = OsRng;

    let sk = SecretKey::new(&mut rng);
    let pk = PublicKey::from_secret_key(&secp, &sk);
    let serialized = pk.serialize(); // Compressed: 33 bytes (starts with 0x02 or 0x03)
    // But nostr actually expects the 32 bytes of the x-coordinate. Get from serialize.
    // nostr uses the "serialized x-only" public key:
    // Drop the first byte (format identifier) and use the next 32 bytes
    let x_only = &serialized[1..33];
    hex::encode(x_only)
}

fn generate_secret() -> String {
    let mut buf = [0u8; 32];
    let mut rng = OsRng;
    rng.fill_bytes(&mut buf);
    hex::encode(buf)
}
