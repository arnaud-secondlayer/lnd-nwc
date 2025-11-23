use crate::config::{load_config, store_config};
use nostr_sdk::{Keys, SecretKey};

pub fn load_or_generate_keys() -> Result<Keys, Box<dyn std::error::Error>> {
    let mut cfg = load_config();

    if cfg.secret_key.is_empty() {
        let keys = Keys::generate();
        let secret_key = keys.secret_key();
        cfg.secret_key = secret_key.to_secret_hex();
        store_config(&cfg);
        return Ok(keys);
    }

    let secret_key = SecretKey::from_hex(&cfg.secret_key).unwrap();
    Ok(Keys::new(secret_key))
}
