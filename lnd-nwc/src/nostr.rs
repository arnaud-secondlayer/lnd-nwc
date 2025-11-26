use crate::config::{Config, load_config};

use nostr_sdk::prelude::*;
use nwc::prelude::*;

const FEATURES: [&str; 1] = ["get_info"];

pub async fn start_deamon(keys: Keys) {
    let cfg = load_config();

    tracing::info!("Starting deamon");

    // post_info_to_all_servers(keys.clone(), &cfg).await;
    handle_all_uri_events(&cfg).await;
}

async fn post_info_to_all_servers(keys: Keys, cfg: &Config) {
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

async fn handle_all_uri_events(cfg: &Config) -> Vec<NWC> {
    let nwc_uris = cfg
        .uris
        .values()
        .map(|uri| NostrWalletConnectURI::parse(uri.clone()))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let mut nwcs = Vec::new();
    for nwc_uri in nwc_uris {
        nwcs.push(
            handle_single_uri_events(&nwc_uri)
                .await
                .expect("Should be fine"),
        );
    }
    nwcs
}

async fn handle_single_uri_events(
    nwc_uri: &NostrWalletConnectURI,
) -> Result<nwc::NWC, nostr_sdk::nips::nip47::Error> {
    tracing::debug!("handle_single_uri_events: {}", nwc_uri);

    let nwc = NWC::new(nwc_uri.clone());

    let filter = Filter::new()
        .pubkey(nwc_uri.public_key)
        .kind(Kind::WalletConnectRequest)
        .since(Timestamp::now());

    let client = Client::default();
    for relay_url in nwc_uri.relays.iter() {
        client.add_relay(relay_url.clone()).await.unwrap();
    }
    client.connect().await;

    client
        .subscribe(filter.clone(), None)
        .await
        .expect("Failed to subscribe");

    tracing::debug!("Ready to handle notifications for {}", nwc_uri);

    client
        .handle_notifications(handler)
        .await
        .expect("Could not add relay");

    Ok(nwc)
}

async fn handler(notification: RelayPoolNotification) -> Result<bool> {
    tracing::info!("Found notification: {:?}", notification);
    Ok(false)
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
