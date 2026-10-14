use git2::{ErrorClass, ErrorCode as Git2Code};

use crate::domain::error::{ErrorCode, ErrorDomain, ErrorPayload};

/// Classify a libgit2 failure into the domain catalogue (docs/02 §14).
///
/// The observed engine had ~50 error classes; P0 wires the ones its commands
/// can actually produce, and keeps everything else `unclassified` with the
/// libgit2 class/code preserved in `detail` so the log still says what happened.
pub fn classify(err: &git2::Error) -> ErrorPayload {
    // A credential failure is wrapped with its structured class before it
    // crosses libgit2 (see `engine::credentials`); recover it so the UI can
    // branch on an authentication failure instead of an `unclassified` blob.
    if let Some(payload) = super::credentials::decode_credential_error(err.message()) {
        return payload;
    }

    let (code, retryable) = match (err.class(), err.code()) {
        (ErrorClass::Repository, Git2Code::NotFound) => (ErrorCode::InvalidRepository, false),
        (ErrorClass::Repository, _) => (ErrorCode::InvalidRepository, false),
        (ErrorClass::Index, Git2Code::Locked) => (ErrorCode::IndexLocked, true),
        (ErrorClass::Reference, Git2Code::NotFound) => (ErrorCode::ReferenceNotFound, false),
        (ErrorClass::Reference, Git2Code::Exists) => (ErrorCode::ReferenceExists, false),
        (ErrorClass::Reference, Git2Code::Invalid) => (ErrorCode::InvalidReferenceName, false),
        (ErrorClass::Object, Git2Code::NotFound) => (ErrorCode::UnknownRevision, false),
        (ErrorClass::Object, Git2Code::InvalidSpec) => (ErrorCode::UnknownRevision, false),
        (ErrorClass::Checkout, Git2Code::Conflict) => (ErrorCode::CheckoutConflicts, false),
        (ErrorClass::Checkout, _) => (ErrorCode::CheckoutFailed, false),
        (ErrorClass::Invalid, _) => (ErrorCode::InvalidRequest, false),
        _ => (ErrorCode::Unclassified, false),
    };

    ErrorPayload {
        code,
        message: err.message().to_string(),
        detail: Some(format!(
            "libgit2 class {:?}, code {:?}",
            err.class(),
            err.code()
        )),
        params: Default::default(),
        retryable,
        domain: ErrorDomain::Git,
    }
}

impl From<git2::Error> for ErrorPayload {
    fn from(err: git2::Error) -> Self {
        classify(&err)
    }
}
