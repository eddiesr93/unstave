//! Module graph analysis for TypeScript/React monorepos.
//!
//! This crate is a library first: it makes no assumptions about being run from a
//! terminal and never prints. Rendering lives in `unstave-report`.

pub mod config;
pub mod discovery;
pub mod error;
pub mod facts;
pub mod parse;
pub mod pipeline;
pub mod resolve;

pub use config::Config;
pub use discovery::{discover, Package, Workspace};
pub use error::{Error, Result};
pub use facts::{Binding, ExportRecord, ImportKind, ImportRecord, ModuleFacts, Span};
pub use pipeline::{analyze, analyze_root, Analysis, Module};
pub use resolve::{Resolved, ResolverSet, UnresolvedSpecifier};
