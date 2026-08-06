use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::facts::{Binding, ExportRecord, ImportKind, ImportRecord, ModuleFacts};

/// Parse one file's source into owned [`ModuleFacts`].
///
/// The caller owns the [`Allocator`]; it must be created inside the worker closure
/// because it is not `Send`. Nothing borrowed from the arena escapes this function.
pub fn parse_module(path: &Path, source: &str, allocator: &Allocator) -> ModuleFacts {
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::tsx());
    let ret = Parser::new(allocator, source, source_type).parse();

    let mut facts = ModuleFacts::empty(path.to_path_buf());
    facts.content_hash = xxhash_rust::xxh3::xxh3_64(source.as_bytes());
    facts.parse_errors = ret
        .diagnostics
        .iter()
        .map(|e| e.message.to_string())
        .collect();

    Extractor { facts: &mut facts }.run(&ret.program);

    facts
}

struct Extractor<'f> {
    facts: &'f mut ModuleFacts,
}

impl Extractor<'_> {
    fn run(&mut self, program: &Program<'_>) {
        for stmt in &program.body {
            self.visit_top_level(stmt);
        }
        // Dynamic imports can appear at any depth, so they get their own pass.
        let mut dyn_visitor = DynamicImportVisitor { found: Vec::new() };
        dyn_visitor.visit_program(program);
        for (specifier, span) in dyn_visitor.found {
            self.facts.imports.push(ImportRecord {
                specifier,
                kind: ImportKind::Dynamic,
                type_only: false,
                bindings: Vec::new(),
                span,
            });
        }
    }

    fn visit_top_level(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::ImportDeclaration(decl) => self.import_decl(decl),
            // `export const x = 1` — a declaration that happens to be exported.
            Statement::ExportDeclaration(decl) => self.export_declaration(&decl.declaration),
            // `export { a, b }` — forwarding local bindings, no source.
            Statement::ExportNamedDeclaration(decl) => {
                let type_only = decl.export_kind.is_type();
                for spec in &decl.specifiers {
                    // The local name before any `as` alias is what the symbol resolver
                    // uses to follow the import chain (e.g. `export { foo as bar }`
                    // with `import { foo } from './x'` resolves `bar` to `./x`).
                    self.facts.exports.push(ExportRecord::Local {
                        name: module_export_name(&spec.exported),
                        local: module_export_name(&spec.local),
                        type_only: type_only || spec.export_kind.is_type(),
                    });
                }
            }
            // `export { a } from './x'` — a true re-export.
            Statement::ExportFromDeclaration(decl) => self.export_from(decl),
            Statement::ExportAllDeclaration(decl) => self.export_all(decl),
            Statement::ExportDefaultDeclaration(_) => {
                self.facts.exports.push(ExportRecord::Default);
                self.facts.own_decl_count += 1;
            }
            other => {
                if let Some(decl) = other.as_declaration() {
                    self.facts.own_decl_count += declared_names(decl).len();
                } else {
                    // A top-level statement that is neither a declaration nor a module
                    // declaration is, for our purposes, a side effect.
                    self.facts.has_side_effects = true;
                }
            }
        }
    }

    fn import_decl(&mut self, decl: &ImportDeclaration<'_>) {
        let type_only = decl.import_kind.is_type();
        let specifier = decl.source.value.to_string();
        let span = decl.span.into();

        let Some(specifiers) = &decl.specifiers else {
            // `import "./side-effect"` — no bindings at all.
            self.facts.imports.push(ImportRecord {
                specifier,
                kind: ImportKind::SideEffect,
                type_only,
                bindings: Vec::new(),
                span,
            });
            return;
        };

        // One statement can mix a default and a named/namespace clause; we record the
        // dominant kind and keep every binding, which is what the codemod needs.
        let mut kind = ImportKind::Named;
        let mut bindings = Vec::new();
        for spec in specifiers {
            match spec {
                ImportDeclarationSpecifier::ImportSpecifier(s) => {
                    bindings.push(Binding {
                        local: s.local.name.to_string(),
                        imported: module_export_name(&s.imported),
                        type_only: type_only || s.import_kind.is_type(),
                    });
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                    if bindings.is_empty() {
                        kind = ImportKind::Default;
                    }
                    bindings.push(Binding {
                        local: s.local.name.to_string(),
                        imported: "default".to_string(),
                        type_only,
                    });
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                    kind = ImportKind::Namespace;
                    bindings.push(Binding {
                        local: s.local.name.to_string(),
                        imported: "*".to_string(),
                        type_only,
                    });
                }
            }
        }

        self.facts.imports.push(ImportRecord {
            specifier,
            kind,
            type_only,
            bindings,
            span,
        });
    }

    fn export_declaration(&mut self, inner: &Declaration<'_>) {
        let type_only = is_type_declaration(inner);
        for name in declared_names(inner) {
            self.facts.own_decl_count += 1;
            self.facts.exports.push(ExportRecord::Local {
                name: name.clone(),
                local: name,
                type_only,
            });
        }
    }

    fn export_from(&mut self, decl: &ExportFromDeclaration<'_>) {
        let type_only = decl.export_kind.is_type();
        let from = decl.source.value.to_string();
        for spec in &decl.specifiers {
            self.facts.exports.push(ExportRecord::Named {
                name: module_export_name(&spec.exported),
                imported: module_export_name(&spec.local),
                from: from.clone(),
                type_only: type_only || spec.export_kind.is_type(),
            });
        }
    }

    fn export_all(&mut self, decl: &ExportAllDeclaration<'_>) {
        let from = decl.source.value.to_string();
        match &decl.exported {
            Some(name) => self.facts.exports.push(ExportRecord::NamespaceStar {
                name: module_export_name(name),
                from,
            }),
            None => self.facts.exports.push(ExportRecord::Star { from }),
        }
    }
}

struct DynamicImportVisitor {
    found: Vec<(String, crate::facts::Span)>,
}

impl<'a> Visit<'a> for DynamicImportVisitor {
    fn visit_import_expression(&mut self, it: &ImportExpression<'a>) {
        // Only statically analyzable `import("literal")` gives us an edge.
        if let Expression::StringLiteral(lit) = &it.source {
            self.found.push((lit.value.to_string(), it.span.into()));
        }
        oxc_ast_visit::walk::walk_import_expression(self, it);
    }
}

fn module_export_name(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(id) => id.name.to_string(),
        ModuleExportName::IdentifierReference(id) => id.name.to_string(),
        ModuleExportName::StringLiteral(lit) => lit.value.to_string(),
    }
}

fn is_type_declaration(decl: &Declaration<'_>) -> bool {
    matches!(
        decl,
        Declaration::TSTypeAliasDeclaration(_) | Declaration::TSInterfaceDeclaration(_)
    )
}

/// Names a top-level declaration introduces. A destructuring `const` can introduce
/// several; an anonymous declaration introduces none.
fn declared_names(decl: &Declaration<'_>) -> Vec<String> {
    let mut names = Vec::new();
    match decl {
        Declaration::VariableDeclaration(var) => {
            for d in &var.declarations {
                collect_binding_pattern(&d.id, &mut names);
            }
        }
        Declaration::FunctionDeclaration(f) => {
            if let Some(id) = &f.id {
                names.push(id.name.to_string());
            }
        }
        Declaration::ClassDeclaration(c) => {
            if let Some(id) = &c.id {
                names.push(id.name.to_string());
            }
        }
        Declaration::TSTypeAliasDeclaration(t) => names.push(t.id.name.to_string()),
        Declaration::TSInterfaceDeclaration(t) => names.push(t.id.name.to_string()),
        Declaration::TSEnumDeclaration(t) => names.push(t.id.name.to_string()),
        Declaration::TSModuleDeclaration(t) => names.push(t.id.name().to_string()),
        Declaration::TSImportEqualsDeclaration(t) => names.push(t.id.name.to_string()),
        Declaration::TSGlobalDeclaration(_) => {}
    }
    names
}

fn collect_binding_pattern(pat: &BindingPattern<'_>, out: &mut Vec<String>) {
    match pat {
        BindingPattern::BindingIdentifier(id) => out.push(id.name.to_string()),
        BindingPattern::ObjectPattern(obj) => {
            for prop in &obj.properties {
                collect_binding_pattern(&prop.value, out);
            }
            if let Some(rest) = &obj.rest {
                collect_binding_pattern(&rest.argument, out);
            }
        }
        BindingPattern::ArrayPattern(arr) => {
            for elem in arr.elements.iter().flatten() {
                collect_binding_pattern(elem, out);
            }
            if let Some(rest) = &arr.rest {
                collect_binding_pattern(&rest.argument, out);
            }
        }
        BindingPattern::AssignmentPattern(a) => collect_binding_pattern(&a.left, out),
    }
}
