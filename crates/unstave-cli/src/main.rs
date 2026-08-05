use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use unstave_core::analysis::symbols::SymbolResolver;
use unstave_core::analysis::{amplification, barrel, cycles};
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

    /// Maximum graph nodes before DOT/Mermaid collapse directories.
    #[arg(long, value_name = "N", default_value_t = 150, value_parser = parse_positive_usize)]
    max_nodes: usize,
}

#[derive(Args)]
struct FixArgs {
    #[arg(long, value_name = "PATH")]
    barrel: Option<PathBuf>,
    #[arg(long, value_name = "GLOB")]
    only: Option<String>,
    #[arg(long)]
    write: bool,
    #[arg(long)]
    check: bool,
    #[arg(long, value_name = "STYLE")]
    import_style: Option<String>,
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
        Command::Fix(_) => not_yet("fix", "M6"),
        Command::Cache(CacheCommand::Clear) => not_yet("cache clear", "M8"),
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
                    Format::Html => html::render(&report).context("serializing HTML report")?,
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
    let config = Config::load(&global.root, global.config.as_deref())
        .with_context(|| format!("loading config for {}", global.root.display()))?;
    let analysis = analyze(&global.root, &config)
        .with_context(|| format!("analyzing {}", global.root.display()))?;
    let graph = ModuleGraph::build(&analysis.modules);
    Ok(Loaded {
        config,
        analysis,
        graph,
    })
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

fn not_yet(what: &str, milestone: &str) -> Result<()> {
    anyhow::bail!("`{what}` is not implemented yet — it arrives at milestone {milestone}")
}
