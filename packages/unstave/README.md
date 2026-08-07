# @unstave/node

Native (Rust) module-graph analysis for TypeScript and React workspaces, exposed
to Node as a small async API. `@unstave/node` is the engine behind the
[`unstave` CLI](https://github.com/eddiesr93/unstave) and the
[`@unstave/vite-plugin`](https://www.npmjs.com/package/@unstave/vite-plugin): it
discovers your workspace, builds the module graph, and reports how much your
`index.ts` barrel files amplify your imports — no `tsc`, no type inference.

## Install

```bash
npm install @unstave/node
```

## Usage

```js
import { analyze, renderHtml } from '@unstave/node'

const report = await analyze({ root: 'path/to/workspace' })

// `report` is the full AnalysisReport (schemaVersion 2): workspace, summary,
// amplification, dead exports, fan-in/fan-out, cycles.
console.log(report.summary)
console.log(report.amplification.barrels)

// Render the report to one self-contained HTML page.
const html = await renderHtml(report)
```

`analyze` accepts `root` (defaults to the current directory), an optional
explicit `config` path to `unstave.toml`, `includeTypeEdges`, and `noCache` to
bypass the content-addressed cache. `renderHtml(report, maxNodes?)` collapses
directories past `maxNodes` (default 150), matching the CLI's `--max-nodes`.

## Docs

- [Main repository README](https://github.com/eddiesr93/unstave)
- [Documentation](https://eddiesr93.github.io/unstave/docs/)
- [Product site](https://eddiesr93.github.io/unstave/)
