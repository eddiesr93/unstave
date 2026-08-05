//! Generate a synthetic Vite/React/TypeScript workspace for benchmarking.
//!
//! The shape mirrors the motivating case in the spec: a large `src/clients`
//! directory behind a single barrel, and a large number of feature modules that
//! each import one or two symbols through that barrel.
//!
//! ```text
//! cargo run --release -p unstave-core --example gen_synthetic -- /tmp/synthetic 6000
//! ```
//!
//! Nothing here is used by the library; it exists to guard the performance budget
//! and to exercise the analyses at realistic scale.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Proportions roughly matching a real app of this size.
const CLIENT_SHARE: f64 = 0.08;
const UTIL_SHARE: f64 = 0.10;
const HOOK_SHARE: f64 = 0.07;
// The remainder become components/features, the modules that import via the barrel.

fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().unwrap_or_else(|| "/tmp/synthetic".into()));
    let total: usize = args.next().and_then(|n| n.parse().ok()).unwrap_or(6000);

    if root.exists() {
        fs::remove_dir_all(&root)?;
    }

    let clients = ((total as f64 * CLIENT_SHARE) as usize).max(1);
    let utils = ((total as f64 * UTIL_SHARE) as usize).max(1);
    let hooks = ((total as f64 * HOOK_SHARE) as usize).max(1);
    let components = total.saturating_sub(clients + utils + hooks).max(1);

    write_manifests(&root)?;
    write_utils(&root, utils)?;
    write_clients(&root, clients, utils)?;
    write_hooks(&root, hooks, clients)?;
    write_components(&root, components, clients, hooks)?;
    write_entrypoint(&root, components)?;

    let written = clients + utils + hooks + components + 2; // + barrel + main
    println!("generated {written} modules under {}", root.display());
    println!("  clients:    {clients} (behind one barrel)");
    println!("  utils:      {utils}");
    println!("  hooks:      {hooks}");
    println!("  components: {components}");
    Ok(())
}

fn write(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

fn write_manifests(root: &Path) -> io::Result<()> {
    write(
        &root.join("package.json"),
        r#"{
  "name": "synthetic-app",
  "private": true,
  "version": "0.0.0",
  "type": "module"
}
"#,
    )?;
    write(
        &root.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": { "@/*": ["src/*"] },
    "jsx": "react-jsx",
    "moduleResolution": "bundler"
  }
}
"#,
    )
}

/// Leaf modules with no imports of their own.
fn write_utils(root: &Path, count: usize) -> io::Result<()> {
    for i in 0..count {
        write(
            &root.join(format!("src/utils/util{i}.ts")),
            &format!(
                "export function util{i}(value: number): number {{\n  return value + {i};\n}}\n"
            ),
        )?;
    }
    Ok(())
}

/// The expensive part: many client modules, all re-exported through one barrel.
fn write_clients(root: &Path, count: usize, utils: usize) -> io::Result<()> {
    let mut barrel = String::new();

    for i in 0..count {
        // Each client pulls in a couple of utils, so the closure behind the barrel
        // is meaningfully larger than the client count alone.
        let a = i % utils;
        let b = (i * 7 + 3) % utils;
        write(
            &root.join(format!("src/clients/client{i}.ts")),
            &format!(
                "import {{ util{a} }} from '@/utils/util{a}';\n\
                 import {{ util{b} }} from '@/utils/util{b}';\n\
                 \n\
                 export class Client{i} {{\n  \
                   readonly id = {i};\n  \
                   compute(): number {{\n    return util{a}({i}) + util{b}({i});\n  }}\n\
                 }}\n"
            ),
        )?;
        let _ = writeln!(barrel, "export {{ Client{i} }} from './client{i}';");
    }

    write(&root.join("src/clients/index.ts"), &barrel)
}

/// Hooks import through the barrel too, so the barrel has more than one consumer kind.
fn write_hooks(root: &Path, count: usize, clients: usize) -> io::Result<()> {
    for i in 0..count {
        let c = i % clients;
        write(
            &root.join(format!("src/hooks/useThing{i}.ts")),
            &format!(
                "import {{ Client{c} }} from '@/clients';\n\
                 \n\
                 export function useThing{i}() {{\n  \
                   return new Client{c}().compute();\n\
                 }}\n"
            ),
        )?;
    }
    Ok(())
}

/// The bulk of the tree: each imports one symbol through the barrel.
fn write_components(root: &Path, count: usize, clients: usize, hooks: usize) -> io::Result<()> {
    for i in 0..count {
        let c = i % clients;
        let h = i % hooks;
        // Nested directories, as a real app would have.
        let dir = format!("src/features/f{}/", i % 40);
        write(
            &root.join(format!("{dir}Component{i}.tsx")),
            &format!(
                "import {{ Client{c} }} from '@/clients';\n\
                 import {{ useThing{h} }} from '@/hooks/useThing{h}';\n\
                 \n\
                 export function Component{i}() {{\n  \
                   const client = new Client{c}();\n  \
                   return useThing{h}() + client.id;\n\
                 }}\n"
            ),
        )?;
    }
    Ok(())
}

/// One entrypoint reaching everything, so the per-entrypoint projection has a target.
fn write_entrypoint(root: &Path, components: usize) -> io::Result<()> {
    let mut main = String::new();
    // Import a slice of components directly; the rest are reachable through them.
    let sample = components.min(40);
    for i in 0..sample {
        let _ = writeln!(
            main,
            "import {{ Component{i} }} from '@/features/f{}/Component{i}';",
            i % 40
        );
    }
    main.push_str("\nexport const app = [\n");
    for i in 0..sample {
        let _ = writeln!(main, "  Component{i},");
    }
    main.push_str("];\n");

    write(&root.join("src/main.tsx"), &main)
}
