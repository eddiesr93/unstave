use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

const TOTAL_MODULES: usize = 6_000;
const CLIENTS: usize = 480;
const COLD_BUDGET: Duration = Duration::from_millis(1_500);
const WARM_BUDGET: Duration = Duration::from_millis(200);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("unstave-pipeline-bench-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root)?;
    }
    generate(&root)?;

    let config = unstave_core::Config::default();
    let cold_started = Instant::now();
    let cold = unstave_core::analyze_cached(&root, &config)?;
    let cold_elapsed = cold_started.elapsed();
    if cold.cache_hit || cold.modules.len() != TOTAL_MODULES {
        return Err(io::Error::other("cold benchmark did not analyze the generated tree").into());
    }

    let warm_started = Instant::now();
    let warm = unstave_core::analyze_cached(&root, &config)?;
    let warm_elapsed = warm_started.elapsed();
    if !warm.cache_hit || warm.modules.len() != TOTAL_MODULES {
        return Err(io::Error::other("warm benchmark did not restore the cache").into());
    }

    println!(
        "unstave pipeline: {TOTAL_MODULES} files, cold {:.1} ms, warm {:.1} ms",
        cold_elapsed.as_secs_f64() * 1_000.0,
        warm_elapsed.as_secs_f64() * 1_000.0
    );
    std::fs::remove_dir_all(&root)?;

    if cold_elapsed > COLD_BUDGET {
        return Err(io::Error::other(format!(
            "cold analysis exceeded {} ms budget",
            COLD_BUDGET.as_millis()
        ))
        .into());
    }
    if warm_elapsed > WARM_BUDGET {
        return Err(io::Error::other(format!(
            "warm analysis exceeded {} ms budget",
            WARM_BUDGET.as_millis()
        ))
        .into());
    }
    Ok(())
}

fn generate(root: &Path) -> io::Result<()> {
    write(
        &root.join("package.json"),
        "{\"name\":\"unstave-benchmark\",\"private\":true}\n",
    )?;
    write(
        &root.join("tsconfig.json"),
        "{\"compilerOptions\":{\"baseUrl\":\".\",\"paths\":{\"@/*\":[\"src/*\"]}}}\n",
    )?;

    let mut barrel = String::new();
    for index in 0..CLIENTS {
        write(
            &root.join(format!("src/clients/client{index}.ts")),
            &format!("export class Client{index} {{ readonly id = {index}; }}\n"),
        )?;
        barrel.push_str(&format!(
            "export {{ Client{index} }} from './client{index}';\n"
        ));
    }
    write(&root.join("src/clients/index.ts"), &barrel)?;

    let consumers = TOTAL_MODULES - CLIENTS - 2;
    for index in 0..consumers {
        let client = index % CLIENTS;
        write(
            &root.join(format!(
                "src/features/f{}/feature{index}.ts",
                index % 40
            )),
            &format!(
                "import {{ Client{client} }} from '@/clients';\nexport const feature{index} = new Client{client}().id;\n"
            ),
        )?;
    }
    write(
        &root.join("src/main.ts"),
        "export const benchmark = true;\n",
    )
}

fn write(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}
