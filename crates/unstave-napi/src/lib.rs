//! Async Node-API boundary for unstave.

use std::path::PathBuf;

use napi::bindgen_prelude::{AsyncTask, Env, Task, Unknown};
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
        analyze_sync(&self.options).map_err(napi::Error::from_reason)
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        env.to_js_value(&output)
    }
}

#[doc(hidden)]
pub struct RenderHtmlTask {
    report: Value,
}

impl Task for RenderHtmlTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        unstave_report::html::render_value(&self.report)
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
#[napi(ts_return_type = "Promise<string>")]
pub fn render_html(report: Value) -> AsyncTask<RenderHtmlTask> {
    AsyncTask::new(RenderHtmlTask { report })
}

fn analyze_sync(options: &AnalyzeOptions) -> Result<Value, String> {
    let root = options
        .root
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let config_path = options.config.as_deref().map(PathBuf::from);
    let config = unstave_core::Config::load(&root, config_path.as_deref())
        .map_err(|error| format!("loading config for {}: {error}", root.display()))?;
    let analysis = if options.no_cache.unwrap_or(false) {
        unstave_core::analyze(&root, &config)
    } else {
        unstave_core::analyze_cached(&root, &config)
    }
    .map_err(|error| format!("analyzing {}: {error}", root.display()))?;
    let graph = ModuleGraph::build(&analysis.modules);
    let report = unstave_report::build_report(
        &analysis,
        &graph,
        &config,
        options.include_type_edges.unwrap_or(false),
    );
    serde_json::to_value(report).map_err(|error| format!("serializing report: {error}"))
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

        let html = unstave_report::html::render_value(&report).expect("report should render");
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("\"schemaVersion\":1"));
    }
}
