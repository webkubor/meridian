//! Pure precondition validators (docs/02 §3, step 3).
//!
//! A validator reads the repository and returns `Result<(), Violation>`; it
//! never mutates anything and never talks to the UI. The command layer turns a
//! [`Violation`] into the shared [`crate::domain::error::ErrorPayload`] at the
//! boundary.

pub mod branches;
pub mod repository;
pub mod working_copy;

use std::collections::BTreeMap;

use crate::domain::error::{ErrorCode, ErrorDomain, ErrorPayload};
use crate::domain::sender::SenderTag;

/// A failed precondition. Plain data, so validators stay free of command-layer
/// types and are trivially unit-testable.
#[derive(Clone, Debug)]
pub struct Violation {
    pub code: ErrorCode,
    pub message: String,
    pub params: BTreeMap<String, String>,
    pub domain: ErrorDomain,
}

impl Violation {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            params: BTreeMap::new(),
            domain: ErrorDomain::Git,
        }
    }

    pub fn with_param(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_string(), value.into());
        self
    }

    pub fn with_domain(mut self, domain: ErrorDomain) -> Self {
        self.domain = domain;
        self
    }
}

impl From<Violation> for ErrorPayload {
    fn from(violation: Violation) -> Self {
        ErrorPayload {
            code: violation.code,
            message: violation.message,
            detail: None,
            params: violation.params,
            retryable: false,
            domain: violation.domain,
        }
    }
}

/// What a validator may look at: the resolved repository plus the sender that
/// triggered the command (sender-scoped preconditions, docs/02 §3).
pub struct ValidateCx<'a> {
    pub repo: &'a git2::Repository,
    pub sender: SenderTag,
}
