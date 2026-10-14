use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BranchFilter {
    #[default]
    Local,
    Remote,
    All,
}

/// A branch row as the sidebar contract wants it (docs/03 §6 `branch_list`).
/// UI-only flags (pinned, archived, stack parent) live in SQLite, not here.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    pub full_name: String,
    pub oid: String,
    pub is_head: bool,
    pub is_remote: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub tip_summary: String,
    pub tip_time: i64,
}

/// Result of `branch_checkout`: `branch` is `None` for a detached checkout.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutResult {
    pub branch: Option<String>,
    pub oid: String,
    pub detached: bool,
}

/// Options for `branch_checkout` — the P0 subset of docs/03 §6. The auto-stash
/// policy dialogue arrives with the `policy/` module in a later phase.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutOptions {
    pub detach: bool,
}
