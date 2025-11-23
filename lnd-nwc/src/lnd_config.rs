use std::fs;
use std::path::Path;

use crate::config::{load_config, store_config};

pub fn store_lnd_config(cert_file: &str, macaroon_file: &str, uri: &str) {
    let cert_path = fs::canonicalize(Path::new(cert_file))
        .expect("Could not create the absolute path the certificate file");
    let macaroon_path = fs::canonicalize(Path::new(macaroon_file))
        .expect("Could not create the absolute path the macaroon file");

    if !cert_path.exists() {
        println!("Certificate file {:?} does not exist", cert_path);
        return;
    }

    if !macaroon_path.exists() {
        println!("Macaroon file {:?} does not exist", macaroon_path);
        return;
    }

    let mut cfg = load_config();
    cfg.lnd.cert_file = cert_path;
    cfg.lnd.macaroon_file = macaroon_path;
    cfg.lnd.uri = uri.to_string();
    store_config(&cfg);
}
