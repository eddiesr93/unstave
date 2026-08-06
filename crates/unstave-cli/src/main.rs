use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use unstave_core::analysis::symbols::SymbolResolver;
use unstave_core::analysis::{amplification, barrel, cycles};
use unstave_core::config::ImportStyle;
use unstave_core::graph::ModuleGraph;
use unstave_core::{analyze, Config};
use unstave_report::{dot, html, json, mermaid, terminal, RenderOptions};

#[derive(Parser)]
#[command(
    name = "unstave",
    version,
    about = "Module graph analyzer and barrel codemod for TypeScript/React monorepos"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    #[command(flatten)]
    global: GlobalArgs,
}

#[derive(Args, Clone)]
struct GlobalArgs {
    /// Path to unstave.toml. Defaults to <root>/unstave.toml.
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Workspace root to analyze.
    #[arg(long, global = true, value_name = "PATH", default_value = ".")]
    root: PathBuf,

    /// Ignore any cached analysis.
    #[arg(long, global = true)]
    no_cache: bool,

    /// Extra export condition to honour when resolving `exports` maps. Repeatable.
    ///
    /// Monorepos often point a custom condition at TypeScript source, so that dev
    /// resolves to `src/` while published builds resolve to `dist/`. Without it,
    /// resolution lands on build output that may not exist yet and cross-package
    /// imports silently fail. Examples: `development`, `source`,
    /// `@tanstack/custom-condition`.
    #[arg(long = "condition", global = true, value_name = "NAME")]
    conditions: Vec<String>,

    /// Increase verbosity (-v, -vv).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Command {
    /// Analyze the workspace and report on its module graph.
    Analyze(AnalyzeArgs),
    /// Focused barrel amplification report.
    Barrels {
        #[arg(long, value_name = "F")]
        min_amplification: Option<f64>,
    },
    /// Report import cycles.
    Cycles,
    /// Report exported symbols with no inbound references.
    DeadExports,
    /// Rewrite barrel imports to point at definition sites.
    Fix(FixArgs),
    /// Cache maintenance.
    #[command(subcommand)]
    Cache(CacheCommand),
}

#[derive(Args)]
struct AnalyzeArgs {
    /// Output format. Repeatable.
    #[arg(long, value_enum, default_value = "terminal")]
    format: Vec<Format>,

    /// Directory for non-terminal output.
    #[arg(long, value_name = "DIR")]
    out: Option<PathBuf>,

    /// Include type-only edges in runtime-cost analyses.
    #[arg(long)]
    include_type_edges: bool,

    /// Maximum graph nodes before DOT, Mermaid, and HTML collapse directories.
    #[arg(long, value_name = "N", default_value_t = 150, value_parser = parse_positive_usize)]
    max_nodes: usize,
}

#[derive(Args)]
struct FixArgs {
    #[arg(long, value_name = "PATH")]
    barrel: Option<PathBuf>,
    #[arg(long, value_name = "GLOB")]
    only: Option<String>,
    /// Print a unified diff without changing files (the default).
    #[arg(long, conflicts_with_all = ["write", "check"])]
    dry_run: bool,
    /// Apply rewrites to source files.
    #[arg(long, conflicts_with_all = ["dry_run", "check"])]
    write: bool,
    /// Exit 1 when rewrites would be made, without changing files.
    #[arg(long, conflicts_with_all = ["dry_run", "write"])]
    check: bool,
    #[arg(long, value_name = "STYLE", value_enum)]
    import_style: Option<ImportStyleArg>,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ImportStyleArg {
    Alias,
    Relative,
    Preserve,
}

impl From<ImportStyleArg> for ImportStyle {
    fn from(value: ImportStyleArg) -> Self {
        match value {
            ImportStyleArg::Alias => ImportStyle::Alias,
            ImportStyleArg::Relative => ImportStyle::Relative,
            ImportStyleArg::Preserve => ImportStyle::Preserve,
        }
    }
}

#[derive(Subcommand)]
enum CacheCommand {
    /// Remove the on-disk analysis cache.
    Clear,
}

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum Format {
    Terminal,
    Json,
    Dot,
    Mermaid,
    Html,
}

impl Format {
    fn file_name(self) -> &'static str {
        match self {
            Format::Terminal => "",
            Format::Json => "unstave-report.json",
            Format::Dot => "unstave-report.dot",
            Format::Mermaid => "unstave-report.mmd",
            Format::Html => "unstave-report.html",
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Analyze(args) => run_analyze(&cli.global, args),
        Command::Barrels { min_amplification } => run_barrels(&cli.global, *min_amplification),
        Command::Cycles => run_cycles(&cli.global),
        Command::DeadExports => run_dead_exports(&cli.global),
        Command::Fix(args) => run_fix(&cli.global, args),
        Command::Cache(CacheCommand::Clear) => run_cache_clear(&cli.global),
    }
}

fn run_analyze(global: &GlobalArgs, args: &AnalyzeArgs) -> Result<()> {
    let loaded = load(global)?;
    let report = unstave_report::build_report(
        &loaded.analysis,
        &loaded.graph,
        &loaded.config,
        args.include_type_edges,
    );
    let formats = dedup_formats(&args.format);
    let output_directory = formats
        .iter()
        .any(|format| *format != Format::Terminal)
        .then(|| {
            args.out
                .clone()
                .unwrap_or_else(|| loaded.analysis.workspace.root.join(".unstave"))
        });

    if let Some(directory) = &output_directory {
        std::fs::create_dir_all(directory)
            .with_context(|| format!("creating output directory {}", directory.display()))?;
    }

    for format in formats {
        match format {
            Format::Terminal => {
                let opts = RenderOptions {
                    color: use_color(),
                    max_rows: 0,
                };
                print!("{}", terminal::render_report(&report, &opts));
            }
            Format::Json | Format::Dot | Format::Mermaid | Format::Html => {
                let contents = match format {
                    Format::Json => json::render(&report).context("serializing JSON report")?,
                    Format::Dot => dot::render(&report, args.max_nodes),
                    Format::Mermaid => mermaid::render(&report, args.max_nodes),
                    Format::Html => {
                        html::render(&report, args.max_nodes).context("serializing HTML report")?
                    }
                    Format::Terminal => unreachable!("terminal handled above"),
                };
                let directory = output_directory
                    .as_ref()
                    .context("non-terminal format has no output directory")?;
                let path = directory.join(format.file_name());
                std::fs::write(&path, contents)
                    .with_context(|| format!("writing {}", path.display()))?;
                eprintln!("wrote {}", path.display());
            }
        }
    }

    if global.verbose > 0 {
        eprintln!(
            "packages: {}",
            loaded
                .analysis
                .workspace
                .packages
                .iter()
                .map(|p| p.root.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    Ok(())
}

/// Shared setup for the graph-level commands.
struct Loaded {
    config: Config,
    analysis: unstave_core::Analysis,
    graph: ModuleGraph,
}

fn load(global: &GlobalArgs) -> Result<Loaded> {
    let mut config = Config::load(&global.root, global.config.as_deref())
        .with_context(|| format!("loading config for {}", global.root.display()))?;
    // CLI conditions layer on top of the file's, ahead of them: a condition passed
    // explicitly for this run should win over one configured for the workspace.
    if !global.conditions.is_empty() {
        let mut conditions = global.conditions.clone();
        conditions.extend(config.resolve.conditions.iter().cloned());
        conditions.dedup();
        config.resolve.conditions = conditions;
    }
    let analysis = if global.no_cache {
        analyze(&global.root, &config)
    } else {
        unstave_core::analyze_cached(&global.root, &config)
    }
    .with_context(|| format!("analyzing {}", global.root.display()))?;
    if global.verbose > 0 {
        let state = if global.no_cache {
            "disabled"
        } else if analysis.cache_hit {
            "hit"
        } else {
            "miss"
        };
        eprintln!("cache: {state}");
    }
    let graph = ModuleGraph::build(&analysis.modules);
    Ok(Loaded {
        config,
        analysis,
        graph,
    })
}

fn run_cache_clear(global: &GlobalArgs) -> Result<()> {
    if unstave_core::clear_cache(&global.root)
        .with_context(|| format!("clearing cache for {}", global.root.display()))?
    {
        println!(
            "cleared {}",
            unstave_core::cache_path(&global.root).display()
        );
    } else {
        println!("cache already clear");
    }
    Ok(())
}

fn amplification_report(
    loaded: &Loaded,
    include_type_edges: bool,
) -> amplification::AmplificationReport {
    let symbols = SymbolResolver::new(&loaded.graph, &loaded.analysis.modules);
    let barrels = barrel::classify(&loaded.graph, &loaded.config.barrel);
    let entrypoints = loaded
        .config
        .entrypoint_paths(&loaded.analysis.workspace.root);
    amplification::compute(
        &loaded.graph,
        &loaded.analysis.modules,
        &barrels,
        &symbols,
        &entrypoints,
        include_type_edges,
    )
}

fn run_barrels(global: &GlobalArgs, min_amplification: Option<f64>) -> Result<()> {
    let loaded = load(global)?;
    let report = amplification_report(&loaded, false);
    let opts = RenderOptions {
        color: use_color(),
        max_rows: 0,
    };
    print!(
        "{}",
        terminal::render_barrels(&loaded.analysis, &report, min_amplification, &opts)
    );
    Ok(())
}

fn run_cycles(global: &GlobalArgs) -> Result<()> {
    let Loaded {
        config,
        analysis,
        graph,
    } = load(global)?;
    let found = cycles::find(&graph, false);
    let opts = RenderOptions {
        color: use_color(),
        max_rows: 0,
    };
    print!("{}", terminal::render_cycles(&analysis, &found, &opts));

    // `max_cycles` is a threshold the user opted into, so exceeding it is a failure.
    if found.len() > config.thresholds.max_cycles {
        anyhow::bail!(
            "{} cycle(s) found, threshold is {}",
            found.len(),
            config.thresholds.max_cycles
        );
    }
    Ok(())
}

fn run_dead_exports(global: &GlobalArgs) -> Result<()> {
    let loaded = load(global)?;
    let report =
        unstave_report::build_report(&loaded.analysis, &loaded.graph, &loaded.config, false);
    let opts = RenderOptions {
        color: use_color(),
        max_rows: 0,
    };
    print!(
        "{}",
        terminal::render_dead_exports(&report.dead_exports, &opts, 0)
    );
    Ok(())
}

fn run_fix(global: &GlobalArgs, args: &FixArgs) -> Result<()> {
    let loaded = load(global)?;
    let options = unstave_codemod::CodemodOptions {
        import_style: args
            .import_style
            .map(ImportStyle::from)
            .unwrap_or(loaded.config.codemod.import_style),
        only: args.only.clone(),
        barrel: args.barrel.clone(),
    };
    let plan = unstave_codemod::plan(&loaded.analysis, &loaded.graph, &loaded.config, &options)
        .context("planning barrel import rewrites")?;
    if args.write {
        for change in &plan.files {
            std::fs::write(&change.path, &change.rewritten)
                .with_context(|| format!("writing {}", change.path.display()))?;
        }
        print!(
            "{}",
            fix_summary(
                &plan,
                &format!(
                    "{} file(s) changed, {} import(s) rewritten",
                    plan.files_changed(),
                    plan.imports_rewritten
                )
            )
        );
        return Ok(());
    }
    if args.check {
        if plan.files_changed() > 0 {
            anyhow::bail!(fix_summary(
                &plan,
                &format!(
                    "{} file(s) would change, {} import(s) would be rewritten",
                    plan.files_changed(),
                    plan.imports_rewritten
                )
            ));
        }
        eprint!("{}", fix_summary(&plan, "0 files would change"));
        return Ok(());
    }
    let _explicit_dry_run = args.dry_run;
    print!("{}", plan.unified_diff(&loaded.analysis.workspace.root));
    eprint!(
        "{}",
        fix_summary(
            &plan,
            &format!(
                "{} file(s) would change, {} import(s) would be rewritten",
                plan.files_changed(),
                plan.imports_rewritten
            )
        )
    );
    Ok(())
}

fn fix_summary(plan: &unstave_codemod::CodemodPlan, headline: &str) -> String {
    let mut summary = format!("{headline}\n");
    for skipped in &plan.skipped {
        summary.push_str(&format!(
            "  {}: {}\n",
            skipped.reason.label(),
            skipped.imports
        ));
    }
    summary
}

/// Preserve the order the user gave, but render each format once.
fn dedup_formats(formats: &[Format]) -> Vec<Format> {
    let mut seen = Vec::new();
    for f in formats {
        if !seen.contains(f) {
            seen.push(*f);
        }
    }
    seen
}

fn parse_positive_usize(value: &str) -> std::result::Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("`{value}` is not a positive integer"))?;
    if parsed == 0 {
        Err("value must be at least 1".to_string())
    } else {
        Ok(parsed)
    }
}

fn use_color() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}
