use std::path::Path;

use oxc_allocator::Allocator;
use unstave_core::facts::{ExportRecord, ImportKind};
use unstave_core::parse::parse_module;

fn parse(source: &str) -> unstave_core::ModuleFacts {
    let allocator = Allocator::default();
    parse_module(Path::new("mod.tsx"), source, &allocator)
}

#[test]
fn extracts_import_kinds() {
    let facts = parse(
        r#"
        import React from 'react';
        import { a, type B } from './a';
        import * as ns from './ns';
        import './side-effect.css';
        import type { T } from './types';
        const lazy = () => import('./lazy');
        "#,
    );

    assert!(facts.parse_errors.is_empty(), "{:?}", facts.parse_errors);

    let kinds: Vec<_> = facts.imports.iter().map(|i| i.kind).collect();
    assert_eq!(
        kinds,
        vec![
            ImportKind::Default,
            ImportKind::Named,
            ImportKind::Namespace,
            ImportKind::SideEffect,
            ImportKind::Named,
            ImportKind::Dynamic,
        ]
    );

    // Inline `type B` is marked per-binding, not statement-wide.
    let named = &facts.imports[1];
    assert!(!named.type_only);
    assert!(!named.bindings[0].type_only);
    assert!(named.bindings[1].type_only);

    // `import type { T }` marks the whole statement.
    assert!(facts.imports[4].type_only);
    assert!(facts.imports[4].is_type_only());

    assert_eq!(facts.imports[5].specifier, "./lazy");
}

#[test]
fn distinguishes_reexports_from_local_declarations() {
    let facts = parse(
        r#"
        export { A } from './a';
        export { B as C } from './b';
        export * from './star';
        export * as ns from './ns';
        export const local = 1;
        export type Alias = string;
        export default function main() {}
        "#,
    );

    assert!(facts.parse_errors.is_empty(), "{:?}", facts.parse_errors);
    assert_eq!(
        facts.exports,
        vec![
            ExportRecord::Named {
                name: "A".into(),
                imported: "A".into(),
                from: "./a".into(),
                type_only: false
            },
            ExportRecord::Named {
                name: "C".into(),
                imported: "B".into(),
                from: "./b".into(),
                type_only: false
            },
            ExportRecord::Star {
                from: "./star".into()
            },
            ExportRecord::NamespaceStar {
                name: "ns".into(),
                from: "./ns".into()
            },
            ExportRecord::Local {
                name: "local".into(),
                type_only: false
            },
            ExportRecord::Local {
                name: "Alias".into(),
                type_only: true
            },
            ExportRecord::Default,
        ]
    );

    // `local`, `Alias`, and the default function.
    assert_eq!(facts.own_decl_count, 3);
}

#[test]
fn detects_top_level_side_effects() {
    let pure = parse("import { a } from './a'; export const b = a;");
    assert!(!pure.has_side_effects);

    let dirty = parse("import { init } from './a'; init();");
    assert!(dirty.has_side_effects);
}

#[test]
fn spans_cover_the_whole_import_statement() {
    let source = "const x = 1;\nimport { a } from './a';\n";
    let facts = parse(source);
    let span = facts.imports[0].span;
    assert_eq!(
        &source[span.start as usize..span.end as usize],
        "import { a } from './a';"
    );
}
