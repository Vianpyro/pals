//! Versioned JSON Schemas shared by the components, and their Rust types.
//!
//! The types the API exchanges with the web interface also have a TypeScript twin in `bindings/`,
//! written by `cargo test -p contracts` (ts-rs). CI fails when the committed ones differ.

mod catalog;
mod draft;
mod id;
mod run;
mod session;
mod submission;

pub use catalog::CatalogEntry;
pub use draft::{Draft, DraftSaved, DraftWrite};
pub use id::{Id, InvalidId};
pub use run::{Run, RunStatus};
pub use session::{Credentials, Identity};
pub use submission::{RunKind, Submission, Submitted};
