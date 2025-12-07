use core::fmt;
use std::collections::HashSet;

use futures::future::join_all;
use nostr_sdk::prelude::*;

use crate::config::{Config, load_config};
use crate::lnd;
use crate::nwc_types;

pub async fn start_deamon(service_keys: Keys) -> Result<()> {
    let cfg = load_config();

    tracing::info!("Starting deamon");

    post_info_to_all_servers(&service_keys, &cfg).await;
    handle_all_uri_events(&service_keys, &cfg).await;

    Ok(())
}

#[derive(Debug)]
enum Error {
    NwcError(nwc_types::NwcError),
    ClientError(nostr_sdk::client::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NwcError(e) => e.fmt(f),
            Self::ClientError(e) => e.fmt(f),
        }
    }
}

async fn post_info_to_all_servers(keys: &Keys, cfg: &Config) {
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
        .collect::<HashSet<_>>()
    {
        client.add_relay(&relay_url).await.unwrap();
    }

    client.connect().await;

    let content = nwc_types::NwcResponse::default_responses()
        .iter()
        .map(|r| r.result_type().to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let builder = EventBuilder::new(Kind::WalletConnectInfo, content).tag(Tag::custom(
        TagKind::Custom("encryption".into()),
        ["nip44_v2"],
    ));
    let output = client.send_event_builder(builder).await.unwrap();

    if !output.failed.is_empty() {
        tracing::debug!("Post info event to server success: {:?}", output.success);
        tracing::debug!("Post info event to server failed: {:?}", output.failed);
    }
}

async fn handle_all_uri_events(service_keys: &Keys, cfg: &Config) -> Result<(), Error> {
    let nwc_uris = cfg
        .uris
        .values()
        .map(|uri| NostrWalletConnectURI::parse(uri.clone()))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let timestamp = Timestamp::now();

    let client = Client::default();
    for relay_url in nwc_uris
        .iter()
        .flat_map(|uri| uri.relays.clone())
        .collect::<HashSet<_>>()
    {
        client.add_relay(&relay_url).await.unwrap();
    }
    client.connect().await;

    let filters = nwc_uris
        .iter()
        .map(|nwc_uri| {
            Filter::new()
                .pubkey(nwc_uri.public_key)
                .kind(Kind::WalletConnectRequest)
                .since(timestamp)
        })
        .collect::<Vec<_>>();

    let subscription_ids = join_all(
        filters
            .iter()
            .map(|filter| client.subscribe(filter.clone(), None))
            .collect::<Vec<_>>(),
    )
    .await
    .into_iter()
    .map(|subscription| subscription.map_or(None, |id| Some(id.val)))
    .collect::<Vec<_>>();

    let uri_id_map: Vec<(NostrWalletConnectURI, Option<SubscriptionId>)> =
        nwc_uris.into_iter().zip(subscription_ids).collect();

    let result = client
        .handle_notifications(|notification| async {
            handler(service_keys, notification, &uri_id_map).await;
            Ok(false)
        })
        .await;
    if let Err(e) = result {
        return Err(Error::ClientError(e));
    }

    Ok(())
}

async fn handler(
    service_keys: &Keys,
    notification: RelayPoolNotification,
    uri_ids: &Vec<(NostrWalletConnectURI, Option<SubscriptionId>)>,
) {
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

        if event.kind != Kind::WalletConnectRequest {
            tracing::error!("Received unexpected event kind");
            return;
        }

        let uri_id = uri_ids
            .iter()
            .filter(|(_, opt_id)| opt_id.as_ref() == Some(&subscription_id))
            .next();

        if let Some(nwc_uri) = uri_id.map(|(uri, _)| uri) {
            let msg = nip04::decrypt(&nwc_uri.secret, &nwc_uri.public_key, &event.content);
            if let Err(e) = msg {
                tracing::error!("Impossible to decrypt direct message: {e}");
                return;
            }

            let request = nwc_types::NwcRequest::from_value(&msg.unwrap());
            if let Err(e) = request {
                tracing::error!("Impossible to decrypt direct message {e}");
                return;
            }

            let result =
                handle_nwc_request(service_keys, &event.id, &request.unwrap(), &nwc_uri).await;
            if let Err(ref e) = result {
                tracing::error!("Error while handling the request {e:?}");
            }
        } else {
            tracing::error!("Incorrect subscription ID {subscription_id} vs {uri_ids:?}");
        }
    }

    tracing::info!("Return true");
}

async fn handle_nwc_request(
    service_keys: &Keys,
    event_id: &EventId,
    request: &nwc_types::NwcRequest,
    uri: &NostrWalletConnectURI,
) -> Result<(), Error> {
    let response = match request {
        nwc_types::NwcRequest::GetInfo(_) => run_get_info().await,
        nwc_types::NwcRequest::GetBalance(_) => run_get_balance().await,
    };

    let content = response
        .to_event_content()
        .map_err(|e| Error::NwcError(e))?;

    let client = Client::default();
    for relay_url in uri.relays.iter() {
        client.add_relay(relay_url.clone()).await.unwrap();
    }
    client.connect().await;

    let event = create_event(service_keys, &content, &event_id.clone(), uri).unwrap();
    tracing::info!("Ready to send response {}", event.id);
    client.send_event(&event).await.unwrap();

    tracing::info!("Sent response {}", event.id);
    Ok(())
}

fn create_event(
    service_keys: &Keys,
    content: &str,
    event_id: &EventId,
    uri: &NostrWalletConnectURI,
) -> Option<Event> {
    let encrypted = nip04::encrypt(&uri.secret, &uri.public_key, content).unwrap();
    EventBuilder::new(Kind::WalletConnectResponse, encrypted)
        .tag(Tag::event(event_id.clone()))
        .build(uri.public_key)
        .sign_with_keys(service_keys)
        .ok()
}

// Calls

async fn run_get_info() -> nwc_types::NwcResponse {
    nwc_types::NwcResponse::GetInfo(nwc_types::GetInfoResult {
        methods: nwc_types::NwcResponse::default_responses()
            .iter()
            .map(|r| r.result_type().to_string())
            .collect::<Vec<_>>(),
    })
}

async fn run_get_balance() -> nwc_types::NwcResponse {
    let lnd_balance = lnd::wallet_balance()
        .await
        .expect("Could not retrieve balance");
    nwc_types::NwcResponse::GetBalance(nwc_types::GetBalanceResult {
        balance: lnd_balance.confirmed_balance,
    })
}
