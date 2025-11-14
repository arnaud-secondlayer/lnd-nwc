use hex;
use secp256k1::rand::{RngCore, rngs::OsRng};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use urlencoding::encode;

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
