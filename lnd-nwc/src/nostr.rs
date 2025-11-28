use crate::config::{Config, load_config};

use nostr_sdk::prelude::*;
use nwc::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

const FEATURES: [&str; 1] = ["get_info"];

#[derive(Serialize, Deserialize, Debug)]
struct NwcRequest {
    method: String,
    params: HashMap<String, String>,
}

pub async fn start_deamon(service_keys: Keys) {
    let cfg = load_config();

    tracing::info!("Starting deamon");

    // post_info_to_all_servers(keys.clone(), &cfg).await;
    handle_all_uri_events(&service_keys, &cfg).await;
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

async fn handle_all_uri_events(service_keys: &Keys, cfg: &Config) -> Vec<NWC> {
    let nwc_uris = cfg
        .uris
        .values()
        .map(|uri| NostrWalletConnectURI::parse(uri.clone()))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let mut nwcs = Vec::new();
    for nwc_uri in nwc_uris {
        nwcs.push(
            handle_single_uri_events(service_keys, &nwc_uri)
                .await
                .expect("Should be fine"),
        );
    }
    nwcs
}

async fn handle_single_uri_events(
    service_keys: &Keys,
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

    let subscription_id = client
        .subscribe(filter.clone(), None)
        .await
        .expect("Failed to subscribe");

    tracing::info!("Ready to handle notifications for {nwc_uri}");

    client
        .handle_notifications(|notification| async {
            handler(
                service_keys,
                notification,
                &nwc_uri.clone(),
                subscription_id.val.clone(),
            )
            .await
        })
        .await
        .expect("Could not add relay");

    Ok(nwc)
}

async fn handler(
    service_keys: &Keys,
    notification: RelayPoolNotification,
    nwc_uri: &NostrWalletConnectURI,
    requested_subscription_id: SubscriptionId,
) -> Result<bool> {
    tracing::info!("Received notification");
    if let RelayPoolNotification::Event {
        subscription_id,
        event,
        ..
    } = notification
    {
        tracing::info!(
            "Received event of kind {} : {} : {}",
            event.kind,
            event.pubkey,
            event.content
        );
        if subscription_id != requested_subscription_id {
            tracing::error!("Incorrect subscription ID");
            return Ok(false);
        }
        if event.kind == Kind::WalletConnectRequest {
            if let Ok(msg) = nip04::decrypt(&nwc_uri.secret, &nwc_uri.public_key, &event.content) {
                tracing::info!("Decrypted message: {}", msg);
                if let Ok(request) = serde_json::from_str::<NwcRequest>(&msg) {
                    handle_nwc_request(service_keys, &event.id, &request, &nwc_uri).await;
                } else {
                    tracing::error!("Invalid NWC_Request");
                }
            } else {
                tracing::error!("Impossible to decrypt direct message");
            }
        } else {
            tracing::info!("Received clear event");
        }
    }
    Ok(false)
}

async fn handle_nwc_request(
    service_keys: &Keys,
    event_id: &EventId,
    request: &NwcRequest,
    uri: &NostrWalletConnectURI,
) {
    let content: String;
    match request.method.as_str() {
        "get_info" => {
            let data = json!({
                            "result_type": "get_info",
                            "result": json!({
                                "methods": ["get_info"]
                            })
            });
            content = serde_json::to_string(&data).unwrap();
            tracing::info!("Ready to send response {content}");
        }
        _ => {
            tracing::error!("Unsupported method {}", request.method);
            return;
        }
    }

    let client = Client::default();
    for relay_url in uri.relays.iter() {
        client.add_relay(relay_url.clone()).await.unwrap();
    }
    client.connect().await;

    let event = create_event(service_keys, &content, &event_id.clone(), uri).unwrap();
    tracing::info!("Ready to send response {}", event.id);
    client.send_event(&event).await.unwrap();

    let filter = Filter::new()
        .author(uri.public_key)
        .kind(Kind::WalletConnectResponse)
        .event(event_id.clone());

    tracing::info!(
        "Sent response {} => {}",
        event.id,
        filter.match_event(&event, MatchEventOptions::default())
    );
}

fn create_event(
    service_keys: &Keys,
    content: &str,
    event_id: &EventId,
    uri: &NostrWalletConnectURI,
) -> Option<Event> {
    let encrypted = nip04::encrypt(&uri.secret, &uri.public_key, content).unwrap();
    // let keys: Keys = Keys::new(uri.secret.clone());
    let event = EventBuilder::new(Kind::WalletConnectResponse, encrypted)
        .tag(Tag::event(event_id.clone()))
        .build(uri.public_key)
        .sign_with_keys(service_keys)
        .unwrap();
    Some(event)
}

pub async fn test() -> Result<()> {
    let uri = NostrWalletConnectURI::parse(
        "nostr+walletconnect://36edf4087c40bc5e3d52405dfcddbdeb259b9917b44f6c7513d049b9f00f66af?relay=ws%3A%2F%2F127.0.0.1%3A8080&secret=4b4a4a4f4f7232494c049b192f4e431becf78974e2e462f13c10ada4cfb904ad",
    )?;
    let nwc = NWC::new(uri);

    tracing::info!("Test for {nwc:?}");

    let info = nwc.get_info().await.expect("Could not get info");
    tracing::info!("Supported methods: {:?}", info.methods);

    Ok(())
}
