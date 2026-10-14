//! Pure transformers between git2 values and domain values (docs/02 §4).
//!
//! The evidence bridged its domain and engine layers with 21 explicit
//! transformers; this module is their counterpart — one file per mapping
//! family, every function pure. `transform` is the only place a `git2` value
//! is allowed to become a domain value.

pub mod file_status;
