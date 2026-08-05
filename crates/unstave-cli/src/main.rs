use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use unstave_core::{analyze, Config};
use unstave_report::{terminal, RenderOptions};

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

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Analyze(args) => run_analyze(&cli.global, args),
        Command::Barrels { .. } => not_yet("barrels", "M4"),
        Command::Cycles => not_yet("cycles", "M3"),
        Command::DeadExports => not_yet("dead-exports", "M5"),
        Command::Fix(_) => not_yet("fix", "M6"),
        Command::Cache(CacheCommand::Clear) => not_yet("cache clear", "M8"),
    }
}

fn run_analyze(global: &GlobalArgs, args: &AnalyzeArgs) -> Result<()> {
    let config = Config::load(&global.root, global.config.as_deref())
        .with_context(|| format!("loading config for {}", global.root.display()))?;

    let analysis = analyze(&global.root, &config)
        .with_context(|| format!("analyzing {}", global.root.display()))?;

    for format in dedup_formats(&args.format) {
        match format {
            Format::Terminal => {
                let opts = RenderOptions {
                    color: use_color(),
                    max_rows: 0,
                };
                print!("{}", terminal::render(&analysis, &opts));
            }
            Format::Json | Format::Dot | Format::Mermaid | Format::Html => {
                return not_yet("this --format", "M5");
            }
        }
    }

    if global.verbose > 0 {
        eprintln!(
            "packages: {}",
            analysis
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

fn use_color() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

fn not_yet(what: &str, milestone: &str) -> Result<()> {
    anyhow::bail!("`{what}` is not implemented yet — it arrives at milestone {milestone}")
}
