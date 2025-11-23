use crate::config::{load_config, store_config};
use nostr_sdk::{Keys, SecretKey};

pub fn load_or_generate_keys() -> Result<Keys, Box<dyn std::error::Error>> {
    let mut cfg = load_config();

    if cfg.nostr.secret.is_empty() {
        let keys = Keys::generate();
        let secret_key = keys.secret_key();
        cfg.nostr.secret = secret_key.to_secret_hex();
        store_config(&cfg);
        return Ok(keys);
    }

    let secret_key = SecretKey::from_hex(&cfg.nostr.secret).unwrap();
    Ok(Keys::new(secret_key))
}
