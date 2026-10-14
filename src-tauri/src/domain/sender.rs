use serde::{Deserialize, Serialize};

/// Where a command invocation came from (docs/02 §3; evidence F2).
///
/// The sender selects *which preconditions apply* and what defaults the UI
/// supplies — never whether an operation exists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SenderTag {
    Menu,
    Toolbar,
    SidebarContextMenu,
    QuickActionPalette,
    Dialogue,
    Cli,
    /// Programmatic calls (tests, scripts) that supply every argument.
    #[default]
    Api,
}
