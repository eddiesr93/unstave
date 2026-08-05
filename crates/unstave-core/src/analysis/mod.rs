//! Analyses over the module graph. Each is independent and takes a borrowed
//! [`ModuleGraph`](crate::graph::ModuleGraph).

pub mod amplification;
pub mod barrel;
pub mod cycles;
pub mod dead_exports;
pub mod fan;
pub mod reach;
pub mod symbols;
