//! Async Node-API boundary for unstave.

use std::path::PathBuf;

use napi::bindgen_prelude::{AsyncTask, Env, Task, Unknown};
use napi::{Error as NapiError, Status};
use napi_derive::napi;
use serde_json::Value;
use unstave_core::graph::ModuleGraph;

/// Inputs accepted by [`analyze`].
#[napi(object)]
pub struct AnalyzeOptions {
    /// Workspace root. Defaults to the current directory.
    pub root: Option<String>,
    /// Optional explicit `unstave.toml` path.
    pub config: Option<String>,
    /// Include type-only edges in runtime-cost analyses.
    pub include_type_edges: Option<bool>,
    /// Bypass the content-addressed analysis cache.
    pub no_cache: Option<bool>,
}

#[doc(hidden)]
pub struct AnalyzeTask {
    options: AnalyzeOptions,
}

impl Task for AnalyzeTask {
    type Output = Value;
    type JsValue = Unknown<'static>;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        analyze_sync(&self.options)
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        env.to_js_value(&output)
    }
}

#[doc(hidden)]
pub struct RenderHtmlTask {
    report: Value,
    max_nodes: usize,
}

impl Task for RenderHtmlTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        unstave_report::html::render_value(&self.report, self.max_nodes)
            .map_err(|error| napi::Error::from_reason(error.to_string()))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

/// Analyze a workspace away from Node's main thread and return the report object.
#[napi(ts_return_type = "Promise<AnalysisReport>")]
pub fn analyze(options: AnalyzeOptions) -> AsyncTask<AnalyzeTask> {
    AsyncTask::new(AnalyzeTask { options })
}

/// Render a report object to one self-contained HTML string off the main thread.
///
/// `maxNodes` is the point past which the graph collapses directories into single
/// nodes, matching the CLI's `--max-nodes`.
#[napi(ts_return_type = "Promise<string>")]
pub fn render_html(report: Value, max_nodes: Option<u32>) -> AsyncTask<RenderHtmlTask> {
    let max_nodes = max_nodes
        .map(|value| value as usize)
        .unwrap_or(unstave_report::html::DEFAULT_MAX_NODES);
    AsyncTask::new(RenderHtmlTask { report, max_nodes })
}

fn analyze_sync(options: &AnalyzeOptions) -> napi::Result<Value> {
    let root = options
        .root
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let config_path = options.config.as_deref().map(PathBuf::from);
    let config = unstave_core::Config::load(&root, config_path.as_deref())
        .map_err(|error| core_error(error, &format!("loading config for {}", root.display())))?;
    let analysis = if options.no_cache.unwrap_or(false) {
        unstave_core::analyze(&root, &config)
    } else {
        unstave_core::analyze_cached(&root, &config)
    }
    .map_err(|error| core_error(error, &format!("analyzing {}", root.display())))?;
    let graph = ModuleGraph::build(&analysis.modules);
    let report = unstave_report::build_report(
        &analysis,
        &graph,
        &config,
        options.include_type_edges.unwrap_or(false),
    );
    serde_json::to_value(report)
        .map_err(|error| napi::Error::from_reason(format!("serializing report: {error}")))
}

/// Convert an `unstave_core::Error` into a `napi::Error`, preserving the structured
/// variant instead of collapsing it to a bare string.
///
/// The full `Display` message (which already carries the variant context and the
/// underlying source chain) becomes the JS `message`. The stable variant name is
/// carried in `cause`, which napi-rs propagates to JS as `err.cause`, so consumers
/// can distinguish error kinds programmatically via `err.cause?.message` (e.g.
/// `'Config'`) rather than parsing the human-readable message.
fn core_error(error: unstave_core::Error, context: &str) -> NapiError {
    let mut err = NapiError::new(Status::GenericFailure, format!("{context}: {error}"));
    err.cause = Some(Box::new(NapiError::new(
        Status::GenericFailure,
        error.variant_name(),
    )));
    err
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name)
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn synchronous_core_returns_the_public_report_shape() {
        let report = analyze_sync(&AnalyzeOptions {
            root: Some(fixture("simple")),
            config: None,
            include_type_edges: Some(false),
            no_cache: Some(true),
        })
        .expect("fixture should analyze");

        assert_eq!(report["schemaVersion"], 1);
        assert_eq!(report["summary"]["filesAnalyzed"], 3);
    }

    #[test]
    fn html_task_renders_a_serialized_report_value() {
        let report = analyze_sync(&AnalyzeOptions {
            root: Some(fixture("simple")),
            config: None,
            include_type_edges: None,
            no_cache: Some(true),
        })
        .expect("fixture should analyze");

        let html =
            unstave_report::html::render_value(&report, unstave_report::html::DEFAULT_MAX_NODES)
                .expect("report should render");
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("\"schemaVersion\":1"));
    }
}
