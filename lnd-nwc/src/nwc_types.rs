use core::fmt;
// use core::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;

use nostr_sdk::prelude::*;
pub use nostr_sdk::nips::nip47::{
    KeysendTLVRecord, LookupInvoiceRequest, MakeInvoiceRequest, PayInvoiceRequest,
    PayKeysendRequest, TransactionState, TransactionType,
};

#[derive(Debug)]
pub enum NwcError {
    UnknownMethod,
    Json(serde_json::Error),
    Message(String),
}

impl std::error::Error for NwcError {}

impl From<serde_json::Error> for NwcError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl fmt::Display for NwcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMethod => f.write_str("Unknown method"),
            Self::Json(e) => e.fmt(f),
            Self::Message(msg) => f.write_str(msg),
        }
    }
}

// Requests

pub enum NwcRequest {
    GetInfo(GetInfoRequest),
    GetBalance(GetBalanceRequest),
    PayInvoice(PayInvoiceRequest),
    PayKeysend(PayKeysendRequest),
    MakeInvoice(MakeInvoiceRequest),
    LookupInvoice(LookupInvoiceRequest),
}

#[derive(Serialize, Deserialize)]
struct RequestTemplate {
    /// Request method
    method: Method,
    /// Params
    #[serde(default)] // handle no params as `Value::Null`
    params: Value,
}

impl NwcRequest {
    pub fn from_value(value: &str) -> Result<Self, NwcError> {
        let request: RequestTemplate = serde_json::from_str(value)?;

        match request.method {
            Method::GetInfo => Ok(Self::GetInfo(GetInfoRequest {})),
            Method::GetBalance => Ok(Self::GetBalance(GetBalanceRequest {})),
            Method::PayInvoice => {
                let params: PayInvoiceRequest = serde_json::from_value(request.params)?;
                Ok(Self::PayInvoice(params))
            }
            Method::PayKeysend => {
                let params: PayKeysendRequest = serde_json::from_value(request.params)?;
                Ok(Self::PayKeysend(params))
            }
            Method::MakeInvoice => {
                let params: MakeInvoiceRequest = serde_json::from_value(request.params)?;
                Ok(Self::MakeInvoice(params))
            }
            Method::LookupInvoice => {
                let params: LookupInvoiceRequest = serde_json::from_value(request.params)?;
                Ok(Self::LookupInvoice(params))
            }
            _ => Err(NwcError::UnknownMethod),
        }
    }
}

pub struct GetInfoRequest {}

pub struct GetBalanceRequest {}

// Resposne
#[derive(Debug, Clone)]
pub enum NwcResponse {
    GetInfo(GetInfoResult),
    GetBalance(GetBalanceResult),
    PayInvoice(PayInvoiceResult),
    PayKeysend(PayKeysendResult),
    MakeInvoice(MakeInvoiceResult),
    LookupInvoice(LookupInvoiceResult),
}

impl NwcResponse {
    pub fn to_event_content(&self) -> Result<String, NwcError> {
        let output = serde_json::to_string(&self.to_content())?;
        Ok(output)
    }

    pub fn result_type(&self) -> &'static str {
        match self {
            Self::GetInfo(p) => p.result_type(),
            Self::GetBalance(p) => p.result_type(),
            Self::PayInvoice(p) => p.result_type(),
            Self::PayKeysend(p) => p.result_type(),
            Self::MakeInvoice(p) => p.result_type(),
            Self::LookupInvoice(p) => p.result_type(),
        }
    }

    pub fn default_responses() -> Vec<NwcResponse> {
        let info = GetInfoResult::default();
        let balance = GetBalanceResult::default();
        let pay_invoice = PayInvoiceResult::default();
        let pay_keysend = PayKeysendResult::default();
        let make_invoice = MakeInvoiceResult::default();
        let lookup_invoice = LookupInvoiceResult::default();
        vec![
            NwcResponse::GetInfo(info),
            NwcResponse::GetBalance(balance),
            NwcResponse::PayInvoice(pay_invoice),
            NwcResponse::PayKeysend(pay_keysend),
            NwcResponse::MakeInvoice(make_invoice),
            NwcResponse::LookupInvoice(lookup_invoice),
        ]
    }

    fn to_content(&self) -> Value {
        match self {
            Self::GetInfo(result) => result.to_content(),
            Self::GetBalance(result) => result.to_content(),
            Self::PayInvoice(result) => result.to_content(),
            Self::PayKeysend(result) => result.to_content(),
            Self::MakeInvoice(result) => result.to_content(),
            Self::LookupInvoice(result) => result.to_content(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GetInfoResult {
    pub methods: Vec<String>,
}

impl GetInfoResult {
    pub fn default() -> Self {
        Self {
            methods: vec![]
        }
    }

    fn result_type(&self) -> &'static str {
        "get_info"
    }

    fn to_content(&self) -> Value {
        json!({"result_type":self.result_type(), "result": json!({ "methods": self.methods.clone() })})
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GetBalanceResult {
    pub balance: i64,
}

impl GetBalanceResult {
    fn default() -> Self {
        Self { balance: 0 }
    }

    fn result_type(&self) -> &'static str {
        "get_balance"
    }

    fn to_content(&self) -> Value {
        json!({"result_type": self.result_type(), "result": json!({ "balance": self.balance })})
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayInvoiceResult {
    pub preimage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fees_paid: Option<u64>,
}

impl PayInvoiceResult {
    pub fn default() -> Self {
        Self {
            preimage: "".to_string(),
            fees_paid: None,
        }
    }

    fn result_type(&self) -> &'static str {
        "pay_invoice"
    }

    fn to_content(&self) -> Value {
        json!({"result_type": self.result_type(), "result": self})
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayKeysendResult {
    pub preimage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fees_paid: Option<u64>,
}

impl PayKeysendResult {
    pub fn default() -> Self {
        Self {
            preimage: "".to_string(),
            fees_paid: None,
        }
    }

    fn result_type(&self) -> &'static str {
        "pay_keysend"
    }

    fn to_content(&self) -> Value {
        json!({"result_type": self.result_type(), "result": self})
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakeInvoiceResult {
    pub invoice: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preimage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Timestamp>,
}

impl MakeInvoiceResult {
    pub fn default() -> Self {
        Self {
            invoice: "".to_string(),
            payment_hash: None,
            description: None,
            description_hash: None,
            preimage: None,
            amount: None,
            created_at: None,
            expires_at: None,
        }
    }

    fn result_type(&self) -> &'static str {
        "make_invoice"
    }

    fn to_content(&self) -> Value {
        json!({"result_type": self.result_type(), "result": self})
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupInvoiceResult {
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<TransactionType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<TransactionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preimage: Option<String>,
    pub payment_hash: String,
    pub amount: u64,
    pub fees_paid: u64,
    pub created_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settled_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

impl LookupInvoiceResult {
    pub fn default() -> Self {
        Self {
            transaction_type: None,
            state: None,
            invoice: None,
            description: None,
            description_hash: None,
            preimage: None,
            payment_hash: "".to_string(),
            amount: 0,
            fees_paid: 0,
            created_at: Timestamp::now(),
            expires_at: None,
            settled_at: None,
            metadata: None,
        }
    }

    fn result_type(&self) -> &'static str {
        "lookup_invoice"
    }

    fn to_content(&self) -> Value {
        json!({"result_type": self.result_type(), "result": self})
    }
}
