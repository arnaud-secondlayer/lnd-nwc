use std::fs;
use lnd_grpc_rust;

pub async fn lnd_display_info() {
    let info =  get_info().await.unwrap();
    println!("{:?}", info);
}

async fn get_info() -> Result<lnd_grpc_rust::lnrpc::GetInfoResponse, Box<dyn std::error::Error>> {
    let cert_bytes = fs::read("tls.cert").expect("FailedToReadTlsCertFile");
    let mac_bytes = fs::read("admin.macaroon").expect("FailedToReadMacaroonFile");

    let cert = buffer_as_hex(cert_bytes);
    let macaroon = buffer_as_hex(mac_bytes);
    let socket = "192.168.1.8:10009".to_string();

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
