use std::path::{Path, PathBuf};

use unstave_core::analysis::amplification::{self, SkipReason};
use unstave_core::analysis::barrel::{self, BarrelKind};
use unstave_core::analysis::dead_exports;
use unstave_core::analysis::symbols::{Resolution, SymbolResolver};
use unstave_core::config::BarrelConfig;
use unstave_core::graph::ModuleGraph;
use unstave_core::pipeline::analyze;
use unstave_core::{Analysis, Config};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

struct Ctx {
    analysis: Analysis,
    graph: ModuleGraph,
    root: PathBuf,
}

fn ctx(name: &str) -> Ctx {
    let root = fixture(name);
    let analysis = analyze(&root, &Config::default()).expect("analysis should not fail");
    let graph = ModuleGraph::build(&analysis.modules);
    let root = analysis.workspace.root.clone();
    Ctx {
        analysis,
        graph,
        root,
    }
}

impl Ctx {
    fn node(&self, rel: &str) -> petgraph::graph::NodeIndex {
        self.graph
            .index_of(&self.root.join(rel))
            .unwrap_or_else(|| panic!("{rel} should be a graph node"))
    }

    fn rel(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn resolver(&self) -> SymbolResolver<'_> {
        SymbolResolver::new(&self.graph, &self.analysis.modules)
    }

    /// Resolve a name and render the definition site as a relative path.
    fn define(&self, resolver: &SymbolResolver<'_>, module: &str, name: &str) -> String {
        match resolver.resolve(self.node(module), name) {
            Resolution::Definition { module, name } => {
                format!("{}#{name}", self.rel(&module))
            }
            other => format!("{other:?}"),
        }
    }
}

#[test]
fn resolves_a_simple_reexport_to_its_definition() {
    let ctx = ctx("pure-barrel");
    let r = ctx.resolver();
    assert_eq!(
        ctx.define(&r, "src/clients/index.ts", "AlphaClient"),
        "src/clients/alpha.ts#AlphaClient"
    );
}

#[test]
fn follows_alias_chains_through_two_barrels() {
    let ctx = ctx("aliases");
    let r = ctx.resolver();

    // FinalName -> Renamed -> Original, across three modules.
    assert_eq!(
        ctx.define(&r, "src/index.ts", "FinalName"),
        "src/inner/impl.ts#Original"
    );
    assert_eq!(
        ctx.define(&r, "src/inner/index.ts", "Renamed"),
        "src/inner/impl.ts#Original"
    );
}

#[test]
fn resolves_aliased_local_reexport_to_the_real_module() {
    // `src/index.ts` does `import { foo } from './impl'; export { foo as bar };`.
    // `bar` is a local alias of the imported `foo`, so it must resolve to `impl.ts`,
    // not back to the barrel itself. Previously the resolver looked up the *alias*
    // (`bar`) among the barrel's imports and fell back to the barrel.
    let ctx = ctx("local-alias");
    let r = ctx.resolver();

    assert_eq!(ctx.define(&r, "src/index.ts", "bar"), "src/impl.ts#foo");

    // A plain (non-aliased) local re-export still resolves the same way.
    assert_eq!(ctx.define(&r, "src/index.ts", "one"), "src/one.ts#one");
}

#[test]
fn follows_star_reexports_three_barrels_deep() {
    let ctx = ctx("nested-barrels");
    let r = ctx.resolver();

    assert_eq!(
        ctx.define(&r, "src/a/index.ts", "one"),
        "src/a/b/c/one.ts#one"
    );
    assert_eq!(
        ctx.define(&r, "src/a/b/index.ts", "two"),
        "src/a/b/c/two.ts#two"
    );
}

#[test]
fn colliding_star_reexports_are_ambiguous_not_guessed() {
    let ctx = ctx("star-collision");
    let r = ctx.resolver();
    let index = ctx.node("src/index.ts");

    match r.resolve(index, "shared") {
        Resolution::Ambiguous { candidates } => {
            let mut rel: Vec<String> = candidates.iter().map(|p| ctx.rel(p)).collect();
            rel.sort();
            assert_eq!(rel, vec!["src/left.ts", "src/right.ts"]);
        }
        other => panic!("expected Ambiguous, got {other:?}"),
    }

    // A name exported by only one star is not ambiguous.
    assert_eq!(
        ctx.define(&r, "src/index.ts", "onlyLeft"),
        "src/left.ts#onlyLeft"
    );
    assert_eq!(
        ctx.define(&r, "src/index.ts", "onlyRight"),
        "src/right.ts#onlyRight"
    );
}

#[test]
fn a_local_declaration_beats_a_star_reexport() {
    let ctx = ctx("star-collision");
    let r = ctx.resolver();

    // shadowed.ts does `export * from './left'` and also declares `shared` itself.
    assert_eq!(
        ctx.define(&r, "src/shadowed.ts", "shared"),
        "src/shadowed.ts#shared"
    );
    // The barrel's own declaration likewise wins.
    assert_eq!(
        ctx.define(&r, "src/index.ts", "localWins"),
        "src/index.ts#localWins"
    );
}

#[test]
fn a_cyclic_reexport_chain_is_reported_not_looped_forever() {
    let ctx = ctx("cycles");
    let r = ctx.resolver();
    // a.ts imports from b.ts and vice versa; `a` is declared in a.ts.
    assert_eq!(ctx.define(&r, "src/a.ts", "a"), "src/a.ts#a");
}

#[test]
fn unknown_names_are_not_found() {
    let ctx = ctx("pure-barrel");
    let r = ctx.resolver();
    assert_eq!(
        r.resolve(ctx.node("src/clients/index.ts"), "NotAThing"),
        Resolution::NotFound
    );
}

#[test]
fn classifies_pure_and_mixed_barrels() {
    let pure = ctx("pure-barrel");
    let barrels = barrel::classify(&pure.graph, &BarrelConfig::default());
    assert_eq!(barrels.len(), 1);
    assert_eq!(pure.rel(&barrels[0].path), "src/clients/index.ts");
    assert_eq!(barrels[0].kind, BarrelKind::Pure);
    assert_eq!(barrels[0].reexport_count, 5);
    assert_eq!(barrels[0].own_decl_count, 0);

    // star-collision's index re-exports twice and declares once: 2/3 is below the
    // default 0.8 ratio, so it is NOT a barrel at default thresholds.
    let collision = ctx("star-collision");
    let barrels = barrel::classify(&collision.graph, &BarrelConfig::default());
    let paths: Vec<String> = barrels.iter().map(|b| collision.rel(&b.path)).collect();
    assert!(
        !paths.contains(&"src/index.ts".to_string()),
        "2 of 3 exports being re-exports is below the 0.8 ratio, got {paths:?}"
    );

    // Loosening the ratio brings it in, as Mixed.
    let loose = BarrelConfig {
        reexport_ratio: 0.6,
        max_own_decls: 2,
    };
    let barrels = barrel::classify(&collision.graph, &loose);
    let index = barrels
        .iter()
        .find(|b| collision.rel(&b.path) == "src/index.ts")
        .expect("index should classify as a barrel at ratio 0.6");
    assert_eq!(index.kind, BarrelKind::Mixed);
}

#[test]
fn measures_amplification_for_a_single_symbol_import() {
    let ctx = ctx("pure-barrel");
    let r = ctx.resolver();
    let barrels = barrel::classify(&ctx.graph, &BarrelConfig::default());
    let report =
        amplification::compute(&ctx.graph, &ctx.analysis.modules, &barrels, &r, &[], false);

    assert_eq!(report.sites.len(), 1);
    let site = &report.sites[0];
    assert_eq!(ctx.rel(&site.importer), "src/main.ts");

    // The barrel plus its five clients.
    assert_eq!(site.actual_cost, 6);
    // Only AlphaClient was imported, and alpha.ts depends on nothing.
    assert_eq!(site.minimal_cost, 1);
    assert_eq!(site.excess(), 5);
    assert_eq!(site.amplification(), 6.0);
    assert!(site.is_fully_rewritable());
    assert_eq!(site.rewritable.len(), 1);
    assert_eq!(ctx.rel(&site.rewritable[0].1), "src/clients/alpha.ts");
}

#[test]
fn a_barrel_with_side_effects_is_never_rewritable() {
    let ctx = ctx("side-effects");
    let r = ctx.resolver();
    let barrels = barrel::classify(&ctx.graph, &BarrelConfig::default());

    let index = barrels
        .iter()
        .find(|b| ctx.rel(&b.path) == "src/index.ts")
        .expect("index.ts should classify as a barrel");
    assert!(index.has_side_effects);

    let report =
        amplification::compute(&ctx.graph, &ctx.analysis.modules, &barrels, &r, &[], false);
    let site = &report.sites[0];

    // The symbol resolves fine — it is the side effect that blocks the rewrite.
    assert!(site.rewritable.is_empty());
    assert_eq!(site.skipped.len(), 1);
    assert_eq!(site.skipped[0].1, SkipReason::BarrelHasSideEffects);
    assert!(!site.is_fully_rewritable());
}

#[test]
fn ambiguous_symbols_are_excluded_from_rewriting() {
    let ctx = ctx("star-collision");
    let r = ctx.resolver();
    let loose = BarrelConfig {
        reexport_ratio: 0.6,
        max_own_decls: 2,
    };
    let barrels = barrel::classify(&ctx.graph, &loose);
    let report =
        amplification::compute(&ctx.graph, &ctx.analysis.modules, &barrels, &r, &[], false);

    let site = report
        .sites
        .iter()
        .find(|s| ctx.rel(&s.importer) == "src/main.ts")
        .expect("main imports the barrel");

    let ambiguous: Vec<&String> = site
        .skipped
        .iter()
        .filter(|(_, reason)| *reason == SkipReason::Ambiguous)
        .map(|(name, _)| name)
        .collect();
    assert_eq!(ambiguous, vec!["shared"]);

    // onlyLeft and localWins still resolve, so the site is partially rewritable.
    assert_eq!(site.rewritable.len(), 2);
    assert!(!site.is_fully_rewritable());
    assert!(report
        .skipped_by_reason
        .iter()
        .any(|(r, n)| *r == SkipReason::Ambiguous && *n == 1));
}

#[test]
fn projects_the_entrypoint_saving() {
    let ctx = ctx("pure-barrel");
    let r = ctx.resolver();
    let barrels = barrel::classify(&ctx.graph, &BarrelConfig::default());
    let entry = ctx.root.join("src/main.ts");

    let report = amplification::compute(
        &ctx.graph,
        &ctx.analysis.modules,
        &barrels,
        &r,
        &[entry],
        false,
    );

    assert_eq!(report.entrypoints.len(), 1);
    let projection = &report.entrypoints[0];
    // main + barrel + 5 clients.
    assert_eq!(projection.before, 7);
    // main + alpha only.
    assert_eq!(projection.after, 2);
    assert_eq!(projection.removed(), 5);
}

/// A barrel that re-exports types must not smuggle its definition modules into the
/// projected closure. The projection used to walk the graph with type edges hardcoded
/// off while the "before" count honoured the caller's setting, so a module behind
/// `export type { X } from './x'` was counted only on the "after" side and the
/// projected saving silently vanished.
///
/// Fixture graph: main -> clients/index -> {load -> fetcher} at runtime, with
/// ThingDto and OtherDto hanging off the barrel through type-only re-exports.
#[test]
fn type_reexports_do_not_inflate_the_projection() {
    // Runtime edges only: ThingDto is unreachable, so rewriting drops the barrel and
    // keeps main + load + fetcher.
    let projection = project_type_reexport(false);
    assert_eq!(projection.before, 4);
    assert_eq!(projection.after, 3);
    assert_eq!(projection.removed(), 1);

    // With type edges counted, ThingDto is genuinely reachable both before and after,
    // and the two type-only modules the importer never asked for still leave.
    let projection = project_type_reexport(true);
    assert_eq!(projection.before, 6);
    assert_eq!(projection.after, 4);
    assert_eq!(projection.removed(), 2);
}

fn project_type_reexport(include_type_edges: bool) -> amplification::EntrypointProjection {
    let ctx = ctx("type-reexport");
    let r = ctx.resolver();
    let barrels = barrel::classify(&ctx.graph, &BarrelConfig::default());
    let entry = ctx.root.join("src/main.ts");

    let report = amplification::compute(
        &ctx.graph,
        &ctx.analysis.modules,
        &barrels,
        &r,
        &[entry],
        include_type_edges,
    );
    report
        .entrypoints
        .into_iter()
        .next()
        .expect("entrypoint should project")
}

#[test]
fn nested_barrels_amplify_through_every_layer() {
    let ctx = ctx("nested-barrels");
    let r = ctx.resolver();
    let barrels = barrel::classify(&ctx.graph, &BarrelConfig::default());

    // All three index files are barrels.
    assert_eq!(barrels.len(), 3, "expected three nested barrels");

    let report = amplification::compute(
        &ctx.graph,
        &ctx.analysis.modules,
        &barrels,
        &r,
        &[ctx.root.join("src/main.ts")],
        false,
    );

    let site = &report.sites[0];
    assert_eq!(ctx.rel(&site.importer), "src/main.ts");
    // src/a -> src/a/b -> src/a/b/c -> one, two, three = 6 modules.
    assert_eq!(site.actual_cost, 6);
    assert_eq!(site.minimal_cost, 1);
    assert_eq!(ctx.rel(&site.rewritable[0].1), "src/a/b/c/one.ts");

    let projection = &report.entrypoints[0];
    assert_eq!(projection.before, 7);
    assert_eq!(projection.after, 2);
}

#[test]
fn finds_unreferenced_exported_definitions() {
    let ctx = ctx("pure-barrel");
    let resolver = ctx.resolver();
    let dead = dead_exports::find(&ctx.analysis, &ctx.graph, &resolver, &[]);
    let clients = dead
        .iter()
        .filter(|export| ctx.rel(&export.module).starts_with("src/clients/"))
        .map(|export| export.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        clients,
        vec!["BetaClient", "DeltaClient", "EpsilonClient", "GammaClient"]
    );
    assert!(dead.iter().all(|export| export.name != "AlphaClient"));
}

#[test]
fn configured_entrypoint_exports_are_not_reported_dead() {
    let ctx = ctx("simple");
    let resolver = ctx.resolver();
    let dead = dead_exports::find(
        &ctx.analysis,
        &ctx.graph,
        &resolver,
        &[ctx.root.join("src/main.ts")],
    );

    assert!(dead.is_empty(), "entrypoint exports are public: {dead:?}");
}

#[test]
fn star_reexported_definitions_are_low_confidence() {
    let ctx = ctx("nested-barrels");
    let resolver = ctx.resolver();
    let dead = dead_exports::find(&ctx.analysis, &ctx.graph, &resolver, &[]);

    for name in ["two", "three"] {
        let export = dead
            .iter()
            .find(|export| export.name == name)
            .unwrap_or_else(|| panic!("{name} should be unused"));
        assert!(export.low_confidence, "{name} is exposed through export *");
    }
}
