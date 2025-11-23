use hex;
use nostr_sdk::prelude::Keys;
use secp256k1::rand::{RngCore, rngs::OsRng};
use urlencoding::encode;

pub fn create_uri(relay: &str) -> String {
    format!(
        "nostr+walletconnect://{}?relay={}&secret={}",
        Keys::generate().public_key().to_hex(),
        encode(relay),
        generate_secret()
    )
}

fn generate_secret() -> String {
    let mut buf = [0u8; 32];
    let mut rng = OsRng;
    rng.fill_bytes(&mut buf);
    hex::encode(buf)
}
