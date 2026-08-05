//! Analyses over the module graph. Each is independent and takes a borrowed
//! [`ModuleGraph`](crate::graph::ModuleGraph).

pub mod cycles;
pub mod fan;
pub mod reach;
