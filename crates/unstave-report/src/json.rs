//! Versioned JSON renderer.

use crate::AnalysisReport;

/// Render the complete, untruncated public report schema.
pub fn render(report: &AnalysisReport) -> serde_json::Result<String> {
    serde_json::to_string_pretty(report).map(|mut json| {
        json.push('\n');
        json
    })
}
