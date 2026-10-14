use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Which subsystem produced an error (docs/02 §14).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorDomain {
    Git,
    Service,
    Fs,
    Internal,
    Cancelled,
}

/// Stable machine codes. One code per error class; the web UI maps `code` to
/// an icon/tone and renders `message` verbatim (docs/02 §14). The set grows as
/// command groups land — codes are never renamed, only added.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    // Repository / filesystem
    InvalidRepository,
    NoWorkingCopy,
    Io,
    InvalidRequest,
    // Working copy
    IndexLocked,
    CheckoutConflicts,
    CheckoutFailed,
    // Refs / revisions
    UnknownRevision,
    InvalidReferenceName,
    ReferenceExists,
    ReferenceNotFound,
    OperationInProgress,
    // Internal
    Cancelled,
    Unclassified,
}

/// The one error envelope every command returns (docs/02 §14).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub code: ErrorCode,
    /// User-facing text from our own catalogue.
    pub message: String,
    /// Technical detail for the log / copy-report action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Interpolation values for the UI (path, ref name, …).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
    pub retryable: bool,
    pub domain: ErrorDomain,
}

impl ErrorPayload {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
            params: BTreeMap::new(),
            retryable: false,
            domain: ErrorDomain::Git,
        }
    }

    pub fn with_domain(mut self, domain: ErrorDomain) -> Self {
        self.domain = domain;
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_param(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_string(), value.into());
        self
    }

    pub fn retryable(mut self, yes: bool) -> Self {
        self.retryable = yes;
        self
    }
}

impl fmt::Display for ErrorPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(detail) = &self.detail {
            write!(f, " ({detail})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ErrorPayload {}
