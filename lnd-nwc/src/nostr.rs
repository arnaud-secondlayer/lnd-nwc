use crate::config::{Config, load_config};

use nostr_sdk::prelude::*;
use nwc::prelude::*;

const FEATURES: [&str; 1] = ["get_info"];

pub async fn start_deamon(keys: Keys) {
    let cfg = load_config();

    println!("Starting deamon");

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
        println!("Adding relay: {}", relay_url);
        client.add_relay(&relay_url).await.unwrap();
    }

    client.connect().await;
    let builder = EventBuilder::new(Kind::WalletConnectInfo, FEATURES.join(" "));
    let output = client.send_event_builder(builder).await.unwrap();

    if !output.failed.is_empty() {
        println!("Post info event to server success: {:?}", output.success);
        println!("Post info event to server failed: {:?}", output.failed);
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

async fn handler(notification: Notification) -> Result<bool> {
    println!("Found notification: {:?}", notification);
    Ok(true)
}
