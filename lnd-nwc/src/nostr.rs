use core::fmt;
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process;

use libc;

use futures::future::join_all;
use nostr_sdk::nips::nip47::{
    Notification as Nip47Notification, NotificationResult, NotificationType, PaymentNotification,
    TransactionState, TransactionType,
};
use nostr_sdk::prelude::*;

use crate::config::{Config, load_config};
use crate::lnd;
use crate::nwc_types;

pub async fn start_deamon(service_keys: Keys, pid_file: &PathBuf) -> Result<()> {
    let cfg = load_config();

    // Block if already running (pid file exists)
    if !pid_file.as_os_str().is_empty() && Path::new(&pid_file).exists() {
        tracing::error!(
            "Daemon already appears to be running (pid file exists at {:?}).",
            pid_file
        );
        return Ok(());
    }

    // Store current process id
    if !pid_file.as_os_str().is_empty() {
        if let Some(parent) = pid_file.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Create new pid file atomically (fail if it already exists)
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pid_file)
        {
            Ok(mut f) => {
                let pid = process::id();
                let _ = writeln!(f, "{pid}");
            }
            Err(e) => {
                tracing::error!("Could not create pid file {:?}: {e}", pid_file);
                return Ok(());
            }
        }
    } else {
        tracing::error!("Warning: pid_file is not configured; daemon start will not write a pid file.");
    }

    tracing::info!("Starting deamon");

    post_info_to_all_servers(&service_keys, &cfg).await;
    tokio::select! {
        result = handle_all_uri_events(&service_keys, &cfg) => {
            if let Err(e) = result {
                tracing::error!("Error while handling URI events: {e}");
            }
        }
        _ = wait_for_shutdown() => {
            tracing::info!("Shutdown signal received, exiting daemon.");
        }
    }

    if !pid_file.as_os_str().is_empty() {
        if let Err(e) = fs::remove_file(&pid_file) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::error!("Failed to remove pid file {:?}: {e}", pid_file);
            }
        }
    }

    Ok(())
}

pub fn stop_deamon(pid_file: &PathBuf) -> Result<()> {
    tracing::info!("Stopping deamon");

    if pid_file.as_os_str().is_empty() {
        eprintln!("pid_file is not configured; cannot stop daemon.");
        return Ok(());
    }

    if !Path::new(&pid_file).exists() {
        eprintln!(
            "Daemon is not running (pid file {:?} does not exist).",
            pid_file
        );
        return Ok(());
    }

    let pid_str = match fs::read_to_string(&pid_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Could not read pid file {:?}: {e}", pid_file);
            return Ok(());
        }
    };

    let pid: i32 = match pid_str.trim().parse() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Invalid pid file contents {:?}: {e}", pid_file);
            return Ok(());
        }
    };

    // Send SIGTERM to the process
    let rc = unsafe { libc::kill(pid, libc::SIGTERM) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        eprintln!("Failed to stop daemon (pid {pid}): {err}");

        // If the process doesn't exist anymore, treat pid file as stale and remove it.
        if err.raw_os_error() == Some(libc::ESRCH) {
            let _ = fs::remove_file(&pid_file);
        }

        return Ok(());
    }

    // Remove pid file after signalling
    if let Err(e) = fs::remove_file(&pid_file) {
        eprintln!(
            "Stopped daemon (pid {pid}) but failed to remove pid file {:?}: {e}",
            pid_file
        );
        return Ok(());
    }

    println!("Stopped daemon (pid {pid}).");
    Ok(())
}

pub fn status_deamon(pid_file: &PathBuf) -> Result<()> {
    if pid_file.as_os_str().is_empty() {
        tracing::error!("Status: unknown (pid_file is not configured).");
        return Ok(());
    }

    if !Path::new(&pid_file).exists() {
        tracing::error!("Status: stopped (no pid file at {:?}).", pid_file);
        return Ok(());
    }

    let mut pid_str = String::new();
    match fs::File::open(&pid_file).and_then(|mut f| f.read_to_string(&mut pid_str)) {
        Ok(_) => {}
        Err(e) => {
            tracing::error!(
                "Status: unknown (could not read pid file {:?}: {e}).",
                pid_file
            );
            return Ok(());
        }
    }

    let pid: i32 = match pid_str.trim().parse() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Status: unknown (invalid pid in {:?}: {e}).", pid_file);
            return Ok(());
        }
    };

    // Check whether the PID exists (signal 0 doesn't send a signal)
    let rc = unsafe { libc::kill(pid, 0) };
    if rc == 0 {
        tracing::info!("Status: running (pid {pid}).");
        return Ok(());
    }

    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        tracing::error!(
            "Status: stale pid file (pid {pid} not running). Remove {:?} if needed.",
            pid_file
        );
    } else {
        tracing::error!("Status: unknown (pid {pid} check failed: {err}).");
    }

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

async fn wait_for_shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to register SIGTERM handler: {e}");
                return;
            }
        };
        let mut sigint = match signal(SignalKind::interrupt()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to register SIGINT handler: {e}");
                return;
            }
        };

        tokio::select! {
            _ = sigterm.recv() => {}
            _ = sigint.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(e) = signal::ctrl_c().await {
            tracing::error!("Failed to listen for shutdown signal: {e}");
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
            "Received event of kind {} at {} : {} : {}",
            event.kind,
            event.created_at,
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

        tracing::info!("uri_id: {:?}", uri_id);

        if let Some(nwc_uri) = uri_id.map(|(uri, _)| uri) {
            let msg = nip04::decrypt(&nwc_uri.secret, &nwc_uri.public_key, &event.content);
            if let Err(e) = msg {
                tracing::error!("Impossible to decrypt direct message: {} for {}", e, nwc_uri);
                return;
            }

            let request = nwc_types::NwcRequest::from_value(&msg.unwrap());
            if let Err(e) = request {
                tracing::error!("Impossible to retrieve the request {} for {}", e, nwc_uri);
                return;
            }

            let result =
                handle_nwc_request(service_keys, &event.id, &request.unwrap(), &nwc_uri).await;
            if let Err(ref e) = result {
                tracing::error!("Error while handling the request {} for {}", e, nwc_uri);
            }
        } else {
            tracing::error!("Incorrect subscription ID {} vs {:?}", subscription_id, uri_ids);
        }
    }
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
        nwc_types::NwcRequest::PayInvoice(params) => {
            run_pay_invoice(service_keys, uri, params).await
        }
        nwc_types::NwcRequest::PayKeysend(params) => {
            run_pay_keysend(service_keys, uri, params).await
        }
        nwc_types::NwcRequest::MakeInvoice(params) => {
            run_make_invoice(service_keys, uri, params).await
        }
        nwc_types::NwcRequest::LookupInvoice(params) => {
            run_lookup_invoice(service_keys, uri, params).await
        }
    }
    .map_err(Error::NwcError)?;

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

async fn run_get_info() -> Result<nwc_types::NwcResponse, nwc_types::NwcError> {
    Ok(nwc_types::NwcResponse::GetInfo(nwc_types::GetInfoResult {
        methods: nwc_types::NwcResponse::default_responses()
            .iter()
            .map(|r| r.result_type().to_string())
            .collect::<Vec<_>>(),
    }))
}

async fn run_get_balance() -> Result<nwc_types::NwcResponse, nwc_types::NwcError> {
    let lnd_balance = lnd::channel_balance()
        .await
        .map_err(|e| nwc_types::NwcError::Message(e.to_string()))?;
    Ok(nwc_types::NwcResponse::GetBalance(
        nwc_types::GetBalanceResult {
            balance: lnd_balance.local_balance.map_or(0, |balance| balance.msat.cast_signed()),
        },
    ))
}

async fn run_pay_invoice(
    service_keys: &Keys,
    uri: &NostrWalletConnectURI,
    request: &nwc_types::PayInvoiceRequest,
) -> Result<nwc_types::NwcResponse, nwc_types::NwcError> {
    let payment = lnd::pay_invoice(&request.invoice, request.amount)
        .await
        .map_err(|e| nwc_types::NwcError::Message(e.to_string()))?;

    let notification = payment_sent_notification(
        &payment,
        TransactionType::Outgoing,
        request.invoice.clone(),
        request.amount,
    );
    if let Err(e) = send_payment_notification(
        service_keys,
        uri,
        NotificationType::PaymentSent,
        notification,
    )
    .await
    {
        tracing::error!("Failed to send payment_sent notification: {e}");
    }

    Ok(nwc_types::NwcResponse::PayInvoice(
        nwc_types::PayInvoiceResult {
            preimage: payment.payment_preimage.clone(),
            fees_paid: payment.fee_msat.try_into().ok(),
        },
    ))
}

async fn run_pay_keysend(
    service_keys: &Keys,
    uri: &NostrWalletConnectURI,
    request: &nwc_types::PayKeysendRequest,
) -> Result<nwc_types::NwcResponse, nwc_types::NwcError> {
    let tlv_records: Vec<(u64, String)> = request
        .tlv_records
        .iter()
        .map(|record: &nwc_types::KeysendTLVRecord| (record.tlv_type, record.value.clone()))
        .collect();

    let payment = lnd::pay_keysend(
        &request.pubkey,
        request.amount,
        request.preimage.as_deref(),
        &tlv_records,
    )
    .await
    .map_err(|e| nwc_types::NwcError::Message(e.to_string()))?;

    let notification = payment_sent_notification(
        &payment,
        TransactionType::Outgoing,
        "".to_string(),
        Some(request.amount),
    );
    if let Err(e) = send_payment_notification(
        service_keys,
        uri,
        NotificationType::PaymentSent,
        notification,
    )
    .await
    {
        tracing::error!("Failed to send payment_sent notification: {e}");
    }

    Ok(nwc_types::NwcResponse::PayKeysend(
        nwc_types::PayKeysendResult {
            preimage: if payment.payment_preimage.is_empty() {
                request.preimage.clone().unwrap_or_else(|| "".to_string())
            } else {
                payment.payment_preimage.clone()
            },
            fees_paid: payment.fee_msat.try_into().ok(),
        },
    ))
}

async fn run_make_invoice(
    service_keys: &Keys,
    uri: &NostrWalletConnectURI,
    request: &nwc_types::MakeInvoiceRequest,
) -> Result<nwc_types::NwcResponse, nwc_types::NwcError> {
    let invoice = lnd::make_invoice(
        request.amount,
        request.description.as_deref(),
        request.description_hash.as_deref(),
        request.expiry,
    )
    .await
    .map_err(|e| nwc_types::NwcError::Message(e.to_string()))?;

    let payment_hash = hex::encode(invoice.r_hash.clone());
    let created_at = Timestamp::now();
    let expires_at = request
        .expiry
        .map(|secs| Timestamp::from(created_at.as_secs() + secs));

    spawn_payment_received_notifier(
        service_keys.clone(),
        uri.clone(),
        invoice.r_hash.clone(),
        invoice.payment_request.clone(),
    );

    Ok(nwc_types::NwcResponse::MakeInvoice(
        nwc_types::MakeInvoiceResult {
            invoice: invoice.payment_request,
            payment_hash: Some(payment_hash),
            description: request.description.clone(),
            description_hash: request.description_hash.clone(),
            preimage: None,
            amount: Some(request.amount),
            created_at: Some(created_at),
            expires_at,
        },
    ))
}

async fn run_lookup_invoice(
    service_keys: &Keys,
    uri: &NostrWalletConnectURI,
    request: &nwc_types::LookupInvoiceRequest,
) -> Result<nwc_types::NwcResponse, nwc_types::NwcError> {
    let invoice = lnd::lookup_invoice(request.payment_hash.as_deref(), request.invoice.as_deref())
        .await
        .map_err(|e| nwc_types::NwcError::Message(e.to_string()))?;

    let result = invoice_to_lookup_result(&invoice)?;

    if matches!(result.state, Some(TransactionState::Settled)) {
        if let Err(e) = send_payment_notification(
            service_keys,
            uri,
            NotificationType::PaymentReceived,
            payment_received_notification(&invoice),
        )
        .await
        {
            tracing::error!("Failed to send payment_received notification: {e}");
        }
    }

    Ok(nwc_types::NwcResponse::LookupInvoice(result))
}

fn payment_sent_notification(
    payment: &lnd_grpc_rust::lnrpc::Payment,
    transaction_type: TransactionType,
    invoice: String,
    amount_override_msat: Option<u64>,
) -> PaymentNotification {
    let amount_msat = amount_override_msat
        .or_else(|| payment.value_msat.try_into().ok())
        .unwrap_or(0);
    let fee_msat = payment.fee_msat.try_into().ok().unwrap_or(0);

    let created_at = nanos_to_timestamp(payment.creation_time_ns);
    let settled_at = payment
        .htlcs
        .iter()
        .filter_map(|htlc| nanos_to_timestamp_opt(htlc.resolve_time_ns))
        .max()
        .unwrap_or_else(Timestamp::now);

    PaymentNotification {
        transaction_type: Some(transaction_type),
        state: Some(TransactionState::Settled),
        invoice,
        description: None,
        description_hash: None,
        preimage: payment.payment_preimage.clone(),
        payment_hash: payment.payment_hash.clone(),
        amount: amount_msat,
        fees_paid: fee_msat,
        created_at,
        expires_at: None,
        settled_at,
        metadata: None,
    }
}

fn payment_received_notification(invoice: &lnd_grpc_rust::lnrpc::Invoice) -> PaymentNotification {
    let amount_msat = if invoice.amt_paid_msat > 0 {
        invoice.amt_paid_msat as u64
    } else {
        invoice.value_msat as u64
    };
    let created_at = Timestamp::from(invoice.creation_date as u64);
    let expires_at = if invoice.expiry > 0 {
        Some(Timestamp::from(
            invoice.creation_date as u64 + invoice.expiry as u64,
        ))
    } else {
        None
    };
    let settled_at = if invoice.settle_date > 0 {
        Some(Timestamp::from(invoice.settle_date as u64))
    } else {
        None
    };

    PaymentNotification {
        transaction_type: Some(TransactionType::Incoming),
        state: Some(TransactionState::Settled),
        invoice: invoice.payment_request.clone(),
        description: if invoice.memo.is_empty() {
            None
        } else {
            Some(invoice.memo.clone())
        },
        description_hash: if invoice.description_hash.is_empty() {
            None
        } else {
            Some(hex::encode(&invoice.description_hash))
        },
        preimage: if invoice.r_preimage.is_empty() {
            String::new()
        } else {
            hex::encode(&invoice.r_preimage)
        },
        payment_hash: hex::encode(&invoice.r_hash),
        amount: amount_msat,
        fees_paid: 0,
        created_at,
        expires_at,
        settled_at: settled_at.unwrap_or(created_at),
        metadata: None,
    }
}

async fn send_payment_notification(
    service_keys: &Keys,
    uri: &NostrWalletConnectURI,
    notification_type: NotificationType,
    notification: PaymentNotification,
) -> Result<(), nwc_types::NwcError> {
    let nip47_notification = Nip47Notification {
        notification_type,
        notification: match notification_type {
            NotificationType::PaymentSent => NotificationResult::PaymentSent(notification),
            NotificationType::PaymentReceived => NotificationResult::PaymentReceived(notification),
            _ => NotificationResult::PaymentSent(notification),
        },
    };
    let content = serde_json::to_string(&nip47_notification)
        .map_err(|e| nwc_types::NwcError::Message(e.to_string()))?;
    let encrypted = nip04::encrypt(&uri.secret, &uri.public_key, &content)
        .map_err(|e| nwc_types::NwcError::Message(e.to_string()))?;

    let client = Client::default();
    for relay_url in uri.relays.iter() {
        client.add_relay(relay_url.clone()).await.unwrap();
    }
    client.connect().await;

    let event = EventBuilder::new(Kind::WalletConnectNotification, encrypted)
        .sign_with_keys(service_keys)
        .map_err(|e| nwc_types::NwcError::Message(e.to_string()))?;

    client
        .send_event(&event)
        .await
        .map_err(|e| nwc_types::NwcError::Message(e.to_string()))?;

    Ok(())
}

fn invoice_to_lookup_result(
    invoice: &lnd_grpc_rust::lnrpc::Invoice,
) -> Result<nwc_types::LookupInvoiceResult, nwc_types::NwcError> {
    let state = match lnd_grpc_rust::lnrpc::invoice::InvoiceState::from_i32(invoice.state) {
        Some(lnd_grpc_rust::lnrpc::invoice::InvoiceState::Settled) => {
            Some(TransactionState::Settled)
        }
        Some(lnd_grpc_rust::lnrpc::invoice::InvoiceState::Canceled) => {
            Some(TransactionState::Failed)
        }
        Some(lnd_grpc_rust::lnrpc::invoice::InvoiceState::Accepted)
        | Some(lnd_grpc_rust::lnrpc::invoice::InvoiceState::Open) => {
            Some(TransactionState::Pending)
        }
        _ => None,
    };

    let amount_msat = if invoice.amt_paid_msat > 0 {
        invoice.amt_paid_msat as u64
    } else {
        invoice.value_msat as u64
    };

    Ok(nwc_types::LookupInvoiceResult {
        transaction_type: Some(TransactionType::Incoming),
        state,
        invoice: if invoice.payment_request.is_empty() {
            None
        } else {
            Some(invoice.payment_request.clone())
        },
        description: if invoice.memo.is_empty() {
            None
        } else {
            Some(invoice.memo.clone())
        },
        description_hash: if invoice.description_hash.is_empty() {
            None
        } else {
            Some(hex::encode(&invoice.description_hash))
        },
        preimage: if invoice.r_preimage.is_empty() {
            None
        } else {
            Some(hex::encode(&invoice.r_preimage))
        },
        payment_hash: hex::encode(&invoice.r_hash),
        amount: amount_msat,
        fees_paid: 0,
        created_at: Timestamp::from(invoice.creation_date as u64),
        expires_at: if invoice.expiry > 0 {
            Some(Timestamp::from(
                invoice.creation_date as u64 + invoice.expiry as u64,
            ))
        } else {
            None
        },
        settled_at: if invoice.settle_date > 0 {
            Some(Timestamp::from(invoice.settle_date as u64))
        } else {
            None
        },
        metadata: None,
    })
}

fn spawn_payment_received_notifier(
    service_keys: Keys,
    uri: NostrWalletConnectURI,
    payment_hash: Vec<u8>,
    payment_request: String,
) {
    tokio::spawn(async move {
        match lnd::wait_for_invoice_settlement(payment_hash).await {
            Ok(invoice) => {
                let notification = payment_received_notification(&invoice);
                if let Err(e) = send_payment_notification(
                    &service_keys,
                    &uri,
                    NotificationType::PaymentReceived,
                    notification,
                )
                .await
                {
                    tracing::error!("Failed to send payment_received notification: {e}");
                }
            }
            Err(e) => {
                tracing::error!("Failed to watch invoice {payment_request}: {e}");
            }
        }
    });
}

fn nanos_to_timestamp(nanos: i64) -> Timestamp {
    nanos_to_timestamp_opt(nanos).unwrap_or_else(Timestamp::now)
}

fn nanos_to_timestamp_opt(nanos: i64) -> Option<Timestamp> {
    if nanos <= 0 {
        return None;
    }
    let secs = nanos as u128 / 1_000_000_000u128;
    Some(Timestamp::from(secs as u64))
}
