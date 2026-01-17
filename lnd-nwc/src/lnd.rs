use lnd_grpc_rust;
use lnd_grpc_rust::invoicesrpc::lookup_invoice_msg::InvoiceRef;
use lnd_grpc_rust::invoicesrpc::{LookupInvoiceMsg, SubscribeSingleInvoiceRequest};
use lnd_grpc_rust::lnrpc::{self, invoice::InvoiceState, payment::PaymentStatus};
use lnd_grpc_rust::routerrpc;
use secp256k1::rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io;

use crate::config::load_config;

const KEYSEND_PREIMAGE_TYPE: u64 = 5_482_373_484;
type LndResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub async fn display_info() {
    let info = get_info().await;
    tracing::info!("{:?}", info);
}

async fn get_info() -> LndResult<lnd_grpc_rust::lnrpc::GetInfoResponse> {
    let mut client = connect_to_lnd().await?;

    let info = client
        .lightning()
        .get_info(lnd_grpc_rust::lnrpc::GetInfoRequest {})
        .await
        .expect("failed to get info")
        .into_inner();

    Ok(info)
}

pub(crate) async fn wallet_balance() -> LndResult<lnd_grpc_rust::lnrpc::WalletBalanceResponse> {
    let mut client = connect_to_lnd().await?;

    let info = client
        .lightning()
        .wallet_balance(lnd_grpc_rust::lnrpc::WalletBalanceRequest {
            account: "default".to_string(),
            min_confs: 0,
        })
        .await
        .expect("failed to get balance")
        .into_inner();

    Ok(info)
}

pub(crate) async fn pay_invoice(
    invoice: &str,
    amount_msat: Option<u64>,
) -> LndResult<lnrpc::Payment> {
    let request = create_payment_request(invoice, amount_msat, default_fee_limit_msat(amount_msat));
    execute_payment(request).await
}

fn create_payment_request(
    invoice: &str,
    amount_msat: Option<u64>,
    fee_limit_msat: i64,
) -> routerrpc::SendPaymentRequest {
    routerrpc::SendPaymentRequest {
        payment_request: invoice.to_string(),
        amt_msat: amount_msat
            .map(|value| i64::try_from(value).unwrap_or(i64::MAX))
            .unwrap_or(0),
        fee_limit_msat: fee_limit_msat,
        ..Default::default()
    }
}

pub(crate) async fn pay_keysend(
    pubkey: &str,
    amount_msat: u64,
    preimage: Option<&str>,
    tlv_records: &[(u64, String)],
) -> LndResult<lnrpc::Payment> {
    let dest = hex::decode(pubkey).map_err(map_to_other)?;
    if dest.len() != 33 {
        return Err(Box::new(map_to_other("Destination pubkey must be 33 bytes")));
    }

    let payment_preimage = match preimage {
        Some(raw) => hex::decode(raw).map_err(map_to_other)?,
        None => {
            let mut bytes = [0u8; 32];
            OsRng.fill_bytes(&mut bytes);
            bytes.to_vec()
        }
    };

    if payment_preimage.len() != 32 {
        return Err(Box::new(map_to_other("Keysend preimage must be 32 bytes")));
    }

    let payment_hash = Sha256::digest(&payment_preimage).to_vec();

    let mut dest_custom_records: HashMap<u64, Vec<u8>> = HashMap::new();
    dest_custom_records.insert(KEYSEND_PREIMAGE_TYPE, payment_preimage.clone());
    for (typ, value) in tlv_records {
        if *typ == KEYSEND_PREIMAGE_TYPE {
            continue;
        }

        let value_bytes = hex::decode(value).unwrap_or_else(|_| value.as_bytes().to_vec());
        dest_custom_records.insert(*typ, value_bytes);
    }

    let request = routerrpc::SendPaymentRequest {
        dest,
        amt_msat: i64::try_from(amount_msat).unwrap_or(i64::MAX),
        payment_hash,
        dest_custom_records,
        fee_limit_msat: default_fee_limit_msat(Some(amount_msat)),
        timeout_seconds: 60,
        ..Default::default()
    };

    execute_payment(request).await
}

async fn execute_payment(
    request: routerrpc::SendPaymentRequest,
) -> LndResult<lnrpc::Payment> {
    let mut client = connect_to_lnd().await?;
    let mut stream = client.router().send_payment_v2(request).await?.into_inner();

    while let Some(payment) = stream.message().await? {
        match PaymentStatus::from_i32(payment.status) {
            Some(PaymentStatus::Succeeded) => return Ok(payment),
            Some(PaymentStatus::Failed) => {
                return Err(Box::new(map_to_other(format!(
                    "Payment failed with reason {:?}",
                    payment.failure_reason
                ))))
            }
            _ => continue,
        }
    }

    Err(Box::new(map_to_other("Failed to receive payment")))
}

pub(crate) async fn make_invoice(
    amount_msat: u64,
    description: Option<&str>,
    description_hash: Option<&str>,
    expiry_secs: Option<u64>,
) -> LndResult<lnrpc::AddInvoiceResponse> {
    let mut client = connect_to_lnd().await?;
    let description_hash_bytes = match description_hash {
        Some(hash) if !hash.is_empty() => Some(hex::decode(hash).map_err(map_to_other)?),
        _ => None,
    };

    let request = lnrpc::Invoice {
        memo: description.unwrap_or_default().to_string(),
        value_msat: i64::try_from(amount_msat).unwrap_or(i64::MAX),
        description_hash: description_hash_bytes.unwrap_or_default(),
        expiry: expiry_secs
            .map(|v| i64::try_from(v).unwrap_or(i64::MAX))
            .unwrap_or_default(),
        ..Default::default()
    };

    let response = client.lightning().add_invoice(request).await?.into_inner();
    Ok(response)
}

pub(crate) async fn lookup_invoice(
    payment_hash_hex: Option<&str>,
    payment_request: Option<&str>,
) -> LndResult<lnrpc::Invoice> {
    let mut client = connect_to_lnd().await?;

    let payment_hash = if let Some(hash_hex) = payment_hash_hex {
        hex::decode(hash_hex).map_err(map_to_other)?
    } else if let Some(pay_req) = payment_request {
        let decoded = client
            .lightning()
            .decode_pay_req(lnrpc::PayReqString {
                pay_req: pay_req.to_string(),
            })
            .await?
            .into_inner();
        hex::decode(decoded.payment_hash).map_err(map_to_other)?
    } else {
        return Err(Box::new(map_to_other(
            "Missing payment hash or payment request",
        )));
    };

    let request = LookupInvoiceMsg {
        invoice_ref: Some(InvoiceRef::PaymentHash(payment_hash)),
        ..Default::default()
    };

    let invoice = client
        .invoices()
        .lookup_invoice_v2(request)
        .await?
        .into_inner();

    Ok(invoice)
}

pub(crate) async fn wait_for_invoice_settlement(
    payment_hash: Vec<u8>,
) -> LndResult<lnrpc::Invoice> {
    let mut client = connect_to_lnd().await?;
    let mut stream = client
        .invoices()
        .subscribe_single_invoice(SubscribeSingleInvoiceRequest {
            r_hash: payment_hash,
        })
        .await?
        .into_inner();

    while let Some(invoice) = stream.message().await? {
        match InvoiceState::from_i32(invoice.state) {
            Some(InvoiceState::Settled) => return Ok(invoice),
            Some(InvoiceState::Canceled) => {
                return Err(Box::new(map_to_other("Invoice canceled")))
            }
            _ => continue,
        }
    }

    Err(Box::new(map_to_other(
        "Invoice stream ended before settlement",
    )))
}

fn default_fee_limit_msat(amount_msat: Option<u64>) -> i64 {
    match amount_msat {
        Some(amount) if amount > 0 => {
            let limit = if amount <= 1_000_000 {
                amount
            } else {
                amount / 20 // ~5%
            };
            i64::try_from(limit).unwrap_or(i64::MAX)
        }
        _ => i64::MAX,
    }
}

async fn connect_to_lnd() -> LndResult<lnd_grpc_rust::LndClient> {
    let cfg = load_config();

    let cert_bytes = fs::read(&cfg.lnd.cert_file)?;
    let mac_bytes = fs::read(&cfg.lnd.macaroon_file)?;

    let cert = buffer_as_hex(cert_bytes);
    let macaroon = buffer_as_hex(mac_bytes);
    let socket = cfg.lnd.uri.clone();

    let client = lnd_grpc_rust::connect(cert, macaroon, socket)
        .await
        .map_err(map_to_other)?;
    Ok(client)
}

fn map_to_other<E: ToString>(err: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err.to_string())
}

fn buffer_as_hex(bytes: Vec<u8>) -> String {
    return bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
}



/*
NIP-47 to LND gRPC Mapping

| NIP-47 Command     | LND gRPC Method (Service)         | Required Field(s)                            | Description                                                                                   |
|--------------------|-----------------------------------|----------------------------------------------|-----------------------------------------------------------------------------------------------|
| **get_info**       | GetInfo                           | —                                            | None required. Returns node info.                                                             |
| **get_balance**    | WalletBalance                     | —                                            | None required. Returns wallet saldo.                                                          |
| **make_invoice**   | AddInvoice                        | `value` (sats, integer)\*<br>`memo` (string, optional) | Creates invoice. Use `value=0` for zero-amount invoice.                                       |
| **pay_invoice**    | SendPaymentSync<br>or SendPaymentV2| `payment_request` (string)<br>`amt` (optional, only if invoice has no value) | Pays a BOLT11 invoice.                                                                        |
| **pay_keysend**    | SendPaymentV2                     | `dest` (bytes: recipient pubkey)<br>`amt` (sats, integer)<br>`key_send=true` | Pay using keysend (send to pubkey w/o invoice).                                               |
| **lookup_invoice** | LookupInvoice                     | `r_hash_str` (string, payment hash)<br> or `payment_hash` | Looks up an invoice by payment hash or preimage.                                              |
| **list_invoices**  | ListInvoices                      | —                                            | Optionally filter with fields like `pending_only`, `reversed`, etc.                           |
| **list_payments**  | ListPayments                      | —                                            | Optionally filter by index or reversed.                                                       |
| **list_channels**  | ListChannels                      | —                                            | Returns open channels.                                                                        |
| **close_channel**  | CloseChannel                      | `channel_point` (object: funding_txid_str + output_index)<br>`force` (optional) | Close a channel, `force` if you want to force close.                                          |
| **open_channel**   | OpenChannelSync<br>or OpenChannel | `node_pubkey` (bytes)<br>`local_funding_amount` (sats, integer) | Opens channel with peer.                                                                      |

\* For `AddInvoice`, required fields for a basic invoice:
- `value` (the amount, in satoshis).
- Use `value=0` for a zero-amount invoice (then payer specifies amount).

---

### **Explanations/Examples for Key Commands**

- **pay_invoice**:
    - `payment_request`: set to BOLT11 invoice string (required)
    - `amt`: only if invoice does not contain an amount (integer, in sats)

- **pay_keysend**:
    - `dest`: public key as a byte array (recipient node)
    - `amt`: amount in sats
    - `key_send`: must be true

- **make_invoice**:
    - `value` (amount), `memo` (optional)

- **lookup_invoice**:
    - Use the `payment_hash` or `r_hash_str` field

- **open_channel**:
    - `node_pubkey`: target peer
    - `local_funding_amount`: amount to fund channel with

---

Command-line Mapping Table**

### 1. **get_info**
Returns node and build info.

```sh
grpcurl -plaintext localhost:10009 lnrpc.Lightning.GetInfo
```

---

### 2. **get_balance**
Returns wallet balance.

```sh
grpcurl -plaintext localhost:10009 lnrpc.Lightning.WalletBalance
```

---

### 3. **make_invoice**
Creates a Lightning invoice (amount in sats).

```sh
grpcurl -plaintext -d '{ "value": 12345, "memo": "sample" }' \
  localhost:10009 lnrpc.Lightning.AddInvoice
```
- For **zero-amount invoice**: use `"value": 0`

---

### 4. **pay_invoice**
Pay a BOLT11 invoice (from `make_invoice` or elsewhere):

```sh
# If the invoice includes amount:
grpcurl -plaintext -d '{ "payment_request": "<BOLT11_STRING>" }' \
  localhost:10009 lnrpc.Lightning.SendPaymentSync

# If the invoice is zero-amount (amount not encoded in invoice):
grpcurl -plaintext -d '{ "payment_request": "<BOLT11_STRING>", "amt": 5000 }' \
  localhost:10009 lnrpc.Lightning.SendPaymentSync
```

---

### 5. **pay_keysend**
Send a keysend payment **without an invoice** (to a node pubkey):

```sh
grpcurl -plaintext -d '{ "dest": "<PUBKEY_HEX>", "amt": 1234, "key_send": true }' \
  localhost:10009 lnrpc.Lightning.SendPaymentV2
```
- Replace `<PUBKEY_HEX>` with the recipient node’s pubkey (no 0x prefix, hex string).

---

### 6. **lookup_invoice**
Look up details of an invoice by payment hash:

```sh
grpcurl -plaintext -d '{ "r_hash_str": "<PAYMENT_HASH>" }' \
  localhost:10009 lnrpc.Lightning.LookupInvoice
```
- Use `"payment_hash"` instead of `"r_hash_str"` if you have it as bytes.

---

### 7. **list_invoices**
List invoices (optionally filtered with more fields):

```sh
grpcurl -plaintext localhost:10009 lnrpc.Lightning.ListInvoices
```

---

### 8. **list_payments**
List outgoing payments sent:

```sh
grpcurl -plaintext localhost:10009 lnrpc.Lightning.ListPayments
```

---

### 9. **list_channels**
Get all open channels:

```sh
grpcurl -plaintext localhost:10009 lnrpc.Lightning.ListChannels
```

---

### 10. **close_channel**
Close a channel by its outpoint.
You must get the funding transaction and output index (see `ListChannels`).

```sh
grpcurl -plaintext -d '{
  "channel_point": {
      "funding_txid_str": "<FUNDING_TXID>",
      "output_index": <INDEX>
  },
  "force": false
}' localhost:10009 lnrpc.Lightning.CloseChannel
```
- Set `"force": true` to force close.
- Channel point must be correct (from `list_channels` output).

---

### 11. **open_channel**
Open a channel to a peer with a given amount:

```sh
grpcurl -plaintext -d '{
    "node_pubkey": "<PEER_PUBKEY_HEX>",
    "local_funding_amount": 100000
}' localhost:10009 lnrpc.Lightning.OpenChannelSync
```
- `<PEER_PUBKEY_HEX>` is the peer’s node pubkey (hex string).

---

## **Tips**
- Omit `-plaintext` and add TLS flags for production use.
- If you need to pass binary fields (e.g., `node_pubkey` as bytes), convert your hex string as needed, as gRPC/LND expects bytes.
- For streaming methods (`OpenChannel`, `SendPaymentV2`), you may need special handling or a `-emit-defaults` for all fields.

*/
