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
        // Remote failures first: libgit2 reports them under `Net`, `Http`,
        // `Ssh` and `Ssl` classes with a mix of codes, and `Auth` can arrive
        // under any of them.
        (_, Git2Code::Auth) => (ErrorCode::AuthRequired, false),
        (_, Git2Code::NotFastForward) => (ErrorCode::PushRejected, false),
        (ErrorClass::Net | ErrorClass::Http | ErrorClass::Ssh | ErrorClass::Ssl, _) => {
            (ErrorCode::NetworkUnreachable, true)
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn classified(code: Git2Code, class: ErrorClass, message: &str) -> ErrorPayload {
        classify(&git2::Error::new(code, class, message))
    }

    #[test]
    fn an_unreachable_host_is_a_network_problem_not_a_file_problem() {
        let payload = classified(
            Git2Code::GenericError,
            ErrorClass::Net,
            "failed to resolve address for example.invalid",
        );
        assert_eq!(payload.code, ErrorCode::NetworkUnreachable);
        assert!(payload.retryable, "a network failure is worth retrying");
        for class in [ErrorClass::Http, ErrorClass::Ssh, ErrorClass::Ssl] {
            assert_eq!(
                classified(Git2Code::GenericError, class, "x").code,
                ErrorCode::NetworkUnreachable
            );
        }
    }

    #[test]
    fn a_refused_credential_is_an_authentication_failure_whatever_the_class() {
        for class in [ErrorClass::Http, ErrorClass::Ssh, ErrorClass::Net] {
            let payload = classified(Git2Code::Auth, class, "authentication required");
            assert_eq!(payload.code, ErrorCode::AuthRequired);
            assert!(
                !payload.retryable,
                "retrying the same credential cannot help"
            );
        }
    }

    #[test]
    fn a_non_fast_forward_is_a_rejected_push() {
        let payload = classified(
            Git2Code::NotFastForward,
            ErrorClass::Reference,
            "cannot push non-fastforward reference",
        );
        assert_eq!(payload.code, ErrorCode::PushRejected);
    }

    #[test]
    fn local_failures_keep_their_classification() {
        assert_eq!(
            classified(Git2Code::Locked, ErrorClass::Index, "locked").code,
            ErrorCode::IndexLocked
        );
        assert_eq!(
            classified(Git2Code::Conflict, ErrorClass::Checkout, "conflicts").code,
            ErrorCode::CheckoutConflicts
        );
        assert_eq!(
            classified(Git2Code::GenericError, ErrorClass::Os, "disk").code,
            ErrorCode::Unclassified
        );
    }

    #[test]
    fn the_wire_names_are_stable_snake_case() {
        let name = |code: ErrorCode| serde_json::to_value(code).unwrap();
        assert_eq!(name(ErrorCode::NetworkUnreachable), "network_unreachable");
        assert_eq!(name(ErrorCode::AuthRequired), "auth_required");
        assert_eq!(name(ErrorCode::PushRejected), "push_rejected");
    }
}
