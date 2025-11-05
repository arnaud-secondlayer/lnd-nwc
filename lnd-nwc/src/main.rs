mod rpc;

use crate::rpc::lnrpc::GetInfoRequest;
use crate::rpc::lnrpc::lightning_client::LightningClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = LightningClient::connect("http://192.168.1.8:10009").await?;
    let request = tonic::Request::new(GetInfoRequest {});
    let response = client.get_info(request).await?;
    println!("RESPONSE={:?}", response);

    Ok(())
}
