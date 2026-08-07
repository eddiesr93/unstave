# Example: barrel bloat in a tiny TypeScript workspace

This directory is a minimal, self-contained TypeScript project that reproduces
the exact pattern `unstave` targets: two `index.ts` **barrel** files that
re-export the modules in their directory, imported through a path alias.

```
src/
  main.ts            # imports from both barrels
  clients/
    index.ts         # barrel: re-exports alpha, beta, gamma
    alpha.ts
    beta.ts
    gamma.ts
  ui/
    index.ts         # barrel: re-exports button, input
    button.ts
    input.ts
```

Each import in `main.ts` that goes through a barrel drags the *whole directory*
into the module graph instead of just the modules that declare the symbols.
`unstave analyze` measures that cost and `unstave fix` rewrites the imports to
point at the defining modules.

There are no dependencies to install. `package.json` and `tsconfig.json` are
only there so the workspace is complete; `tsconfig.json` defines the
`@/* → src/*` alias that `main.ts` imports through.

## Prerequisites

- **Installed CLI** — `cargo install unstave-cli` (or Homebrew: `brew install
  unstave`). Then use `unstave …` below.
- **From source** — clone the repo, then run from the repo root with
  `cargo run -p unstave-cli --` and always add the workspace root
  `--root examples/barrels`, e.g.
  `cargo run -p unstave-cli -- analyze --root examples/barrels`.

All commands below assume you are inside this directory (`examples/barrels`),
so the workspace root defaults to `.`. From elsewhere, pass the root explicitly
with `--root <path>` (a global flag) — for example `unstave analyze --root
examples/barrels` from the repository root.

## 1. Analyze the workspace

```bash
unstave analyze
```

Prints the workspace metrics and, further down, the amplification table:

```
┌──────────────────────┬───────┬──────┬────────┬───────┬──────┬────────────┐
│ barrel               ┆ sites ┆ cost ┆ excess ┆ worst ┆ amp  ┆ rewritable │
╞══════════════════════╪═══════╪══════╪════════╪═══════╪══════╪════════════╡
│ src/clients/index.ts ┆ 1     ┆ 4    ┆ 2      ┆ 2     ┆ 2.0× ┆ 2/2        │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
│ src/ui/index.ts      ┆ 1     ┆ 3    ┆ 1      ┆ 1     ┆ 1.5× ┆ 2/2        │
└──────────────────────┴───────┴──────┴────────┴───────┴──────┴────────────┘
```

The two barrels are real: `src/clients/index.ts` amplifies each import 2.0×
(cost 4 modules when 2 would do), and `src/ui/index.ts` 1.5×.

To also write a JSON report and a self-contained HTML graph, add formats and an
output directory:

```bash
unstave analyze --format json --format html --out .unstave
```

## 2. Focused barrel report

```bash
unstave barrels
```

Same amplification table as `analyze` but focused on barrels only, plus the
worst import sites:

```
Worst import sites
┌────────────────────────────────────┬──────────────────────────┬────────┬─────────┬────────┐
│ import site                        ┆ symbols                  ┆ actual ┆ minimal ┆ excess │
╞════════════════════════════════════╪══════════════════════════╪════════╪═════════╪════════╡
│ src/main.ts → src/clients/index.ts ┆ AlphaClient, GammaClient ┆ 4      ┆ 2       ┆ 2      │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┤
│ src/main.ts → src/ui/index.ts      ┆ Button, Input            ┆ 3      ┆ 2       ┆ 1      │
└────────────────────────────────────┴──────────────────────────┴────────┴─────────┴────────┘
```

## 3. Preview the fix (dry-run, default)

```bash
unstave fix
```

`fix` is conservative and dry-runs by default: it prints a unified diff and
touches nothing.

```
--- a/src/main.ts
+++ b/src/main.ts
@@ -2,8 +2,11 @@
 // drags the whole directory into the module graph. unstave measures that cost
 // (`analyze`, `barrels`) and rewrites the imports to the defining modules
 // (`fix`).
-import { AlphaClient, GammaClient } from '@/clients';
-import { Button, Input } from '@/ui';
+import { AlphaClient } from '@/clients/alpha';
+import { GammaClient } from '@/clients/gamma';
+import { Button } from '@/ui/button';
+import { Input } from '@/ui/input';
+

 const client = new AlphaClient();
 console.log(client.name, new GammaClient().name);
1 file(s) would change, 2 import(s) would be rewritten
```

The imports now point at the modules that actually declare each symbol. The
`@/` alias style is preserved by default; pass `--import-style relative` to
rewrite to relative paths instead.

## 4. Apply the fix

```bash
unstave fix --write
```

Prints `1 file(s) changed, 2 import(s) rewritten` and rewrites `src/main.ts` in
place. Re-running `fix` afterwards is a no-op — the workspace is already
direct-imported.

## 5. CI check

```bash
unstave fix --check
```

Exits with status `1` while any import still needs rewriting, without modifying
files — drop this into CI to keep barrels from creeping back:

```
Error: 1 file(s) would change, 2 import(s) would be rewritten
```

Once `--write` has been applied, `fix --check` exits `0`.

## Expected output after the fix

After `unstave fix --write`, `src/main.ts` becomes:

```ts
import { AlphaClient } from '@/clients/alpha';
import { GammaClient } from '@/clients/gamma';
import { Button } from '@/ui/button';
import { Input } from '@/ui/input';


const client = new AlphaClient();
console.log(client.name, new GammaClient().name);
console.log(Button({ label: 'Go' }), Input({ placeholder: 'Search' }));
```

`unstave analyze` then reports both barrels with **0 import sites**, and
`unstave barrels` reports `0 barrel(s) classified, 0 of them imported`.
