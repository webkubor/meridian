use serde::{Deserialize, Serialize};

/// A revision the user can point at. Every `start`, `from`, … argument in the
/// command surface is a `Revision`; resolution happens inside the engine and
/// failures come back as `UnknownRevision` (docs/03 §5).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum Revision {
    #[default]
    Head,
    Branch(String),
    Tag(String),
    Oid(String),
}
