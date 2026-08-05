//! Renderers for [`unstave_core`] analysis results.
//!
//! Every renderer takes a borrowed [`AnalysisReport`]. Nothing here writes to stdout
//! or chooses output paths — that is the CLI's job.

pub mod dot;
pub mod html;
pub mod json;
pub mod mermaid;
pub mod report;
pub mod terminal;

mod visualization;

pub use report::{build as build_report, AnalysisReport};

use owo_colors::OwoColorize;

/// Presentation knobs shared by the renderers.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// Emit ANSI colors. The CLI decides this from TTY detection and `NO_COLOR`.
    pub color: bool,
    /// Rows per section before truncating. `0` means "use the renderer default".
    pub max_rows: usize,
}

impl RenderOptions {
    pub fn bold(&self, text: &str) -> String {
        if self.color {
            text.bold().to_string()
        } else {
            text.to_string()
        }
    }

    pub fn dim(&self, text: &str) -> String {
        if self.color {
            text.dimmed().to_string()
        } else {
            text.to_string()
        }
    }
}
