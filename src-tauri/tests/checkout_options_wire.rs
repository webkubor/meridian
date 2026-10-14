//! Regression for the `branch_checkout` options envelope the webview actually
//! sends. The TS bridge always serialises `opts: {}`; Rust must accept it.

use meridian::domain::refs::CheckoutOptions;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BranchCheckoutWire {
    repo: String,
    name: String,
    #[serde(default)]
    opts: CheckoutOptions,
}

#[test]
fn empty_opts_object_from_webview_deserializes_as_the_default() {
    let wire = serde_json::json!({
        "repo": "repo-id",
        "name": "feature",
        "opts": {},
    });

    let parsed: BranchCheckoutWire = serde_json::from_value(wire).unwrap();

    assert_eq!(parsed.repo, "repo-id");
    assert_eq!(parsed.name, "feature");
    assert!(!parsed.opts.detach);
}
