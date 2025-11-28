use crate::config::{load_config, store_config};
use crate::uri::create_uri;

use nostr_sdk::{Keys, SecretKey};

pub fn load_and_display() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = load_config();

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
    let mut cfg = load_config();
    if cfg.uris.contains_key(name) {
        panic!("Uri name `{name}`already exists, remove it or use another one")
    }

    let public_key = Keys::new(SecretKey::from_hex(&cfg.nostr.secret).unwrap()).public_key();
    let new_uri = create_uri(&public_key, relay);
    let _ = &cfg.uris.insert(name.into(), new_uri.clone());

    store_config(&cfg);

    println!("New URI created for {name}:\n{new_uri}");

    Ok(())
}

pub fn remove_and_save(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg = load_config();
    if cfg.uris.contains_key(name) == false {
        panic!("Uri name `{name}` does not exist")
    }

    let _ = &cfg.uris.remove(name);
    store_config(&cfg);

    println!("Removed URI for {name}");

    Ok(())
}
