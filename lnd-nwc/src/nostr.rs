use crate::config::{Config, load_config};

use nostr_sdk::prelude::*;
use nwc::prelude::*;

const FEATURES: [&str; 1] = ["get_info"];

pub async fn start_deamon(keys: Keys) {
    let cfg = load_config();

    tracing::info!("Starting deamon");

    _post_info_to_all_servers(keys.clone(), &cfg).await;
    _handle_all_uri_events(&cfg).await;
}

async fn _post_info_to_all_servers(keys: Keys, cfg: &Config) {
    let client = Client::new(keys.clone());

    let nwc_uris = cfg
        .uris
        .values()
        .map(|uri| NostrWalletConnectURI::parse(uri.clone()))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    for relay_url in nwc_uris
        .iter()
        .flat_map(|uri| uri.relays.clone())
        .collect::<Vec<_>>()
    {
        client.add_relay(&relay_url).await.unwrap();
    }

    client.connect().await;
    let builder = EventBuilder::new(Kind::WalletConnectInfo, FEATURES.join(" "));
    let output = client.send_event_builder(builder).await.unwrap();

    if !output.failed.is_empty() {
        tracing::debug!("Post info event to server success: {:?}", output.success);
        tracing::debug!("Post info event to server failed: {:?}", output.failed);
    }
}

async fn _handle_all_uri_events(cfg: &Config) -> Vec<NWC> {
    let nwc_uris = cfg
        .uris
        .values()
        .map(|uri| NostrWalletConnectURI::parse(uri.clone()))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let mut nwcs = Vec::new();
    for nwc_uri in nwc_uris {
        nwcs.push(_handle_single_uri_events(&nwc_uri).await);
    }
    nwcs
}

async fn _handle_single_uri_events(nwc_uri: &NostrWalletConnectURI) -> NWC {
    let nwc = NWC::new(nwc_uri.clone());
    nwc.subscribe_to_notifications()
        .await
        .expect("Cannot subscribe to notitfications");
    nwc.handle_notifications(handler)
        .await
        .expect("Cannot handle notifications");
    nwc
}

pub async fn test() -> Result<()> {
    let uri = NostrWalletConnectURI::parse(
        "nostr+walletconnect://6668a16a671b7de512e9cd2e53b58d70d6c748df1ba036e42022d3cb4df2f283?relay=ws%3A%2F%2F127.0.0.1%3A8080&secret=cd33ae06fee87b2bfd7e79b3d914fb354e06748f8328fc9b115a0e90d9fbcfef",
    )?;
    let nwc = NWC::new(uri);

    tracing::info!("Test for {:?}", nwc);

    let info = nwc.get_info().await.expect("Could not get info");
    tracing::info!("Supported methods: {:?}", info.methods);

    Ok(())
}
