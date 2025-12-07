use core::fmt;
// use core::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;

use nostr_sdk::prelude::*;

#[derive(Debug)]
pub enum NwcError {
    UnknownMethod,
    Json(serde_json::Error),
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
        }
    }
}

// Requests

pub enum NwcRequest {
    GetInfo(GetInfoRequest),
    GetBalance(GetBalanceRequest),
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
        }
    }

    pub fn default_responses() -> Vec<NwcResponse> {
        let info = GetInfoResult::default();
        let balance = GetBalanceResult::default();
        vec![NwcResponse::GetInfo(info), NwcResponse::GetBalance(balance)]
    }

    fn to_content(&self) -> Value {
        match self {
            Self::GetInfo(result) => result.to_content(),
            Self::GetBalance(result) => result.to_content(),
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
