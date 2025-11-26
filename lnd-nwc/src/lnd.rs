use lnd_grpc_rust;
use std::fs;

use crate::config::load_config;

pub async fn lnd_display_info() {
    let info = get_info().await.unwrap();
    tracing::info!("{:?}", info);
}

async fn get_info() -> Result<lnd_grpc_rust::lnrpc::GetInfoResponse, Box<dyn std::error::Error>> {
    let cfg = load_config();

    let cert_bytes = fs::read(&cfg.lnd.cert_file)
        .expect(format!("Failed to read certificate file: {:?}", &cfg.lnd.cert_file).as_str());
    let mac_bytes = fs::read(&cfg.lnd.macaroon_file)
        .expect(format!("Failed to read macaroon file {:?}", &cfg.lnd.macaroon_file).as_str());

    let cert = buffer_as_hex(cert_bytes);
    let macaroon = buffer_as_hex(mac_bytes);
    let socket = cfg.lnd.uri.clone();

    let mut client = lnd_grpc_rust::connect(cert, macaroon, socket)
        .await
        .expect("failed to connect");

    let info = client
        .lightning()
        .get_info(lnd_grpc_rust::lnrpc::GetInfoRequest {})
        .await
        .expect("failed to get info")
        .into_inner();

    Ok(info)
}

fn buffer_as_hex(bytes: Vec<u8>) -> String {
    return bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
}
