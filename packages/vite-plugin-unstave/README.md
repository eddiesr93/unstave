# @unstave/vite-plugin

Non-blocking [unstave](https://github.com/eddiesr93/unstave) module-graph
reports for Vite. The plugin runs the native analysis asynchronously, so Vite
startup and HMR never wait on the module graph. In development it serves a live
report at `/__unstave`; production analysis is opt-in and writes JSON + HTML at
`buildEnd`.

## Install

```bash
npm install --save-dev @unstave/vite-plugin
```

Installing the plugin pulls in [`@unstave/node`](https://www.npmjs.com/package/@unstave/node),
which contains the native binding.

## Usage

```ts
// vite.config.ts
import { defineConfig } from 'vite'
import unstave from '@unstave/vite-plugin'

export default defineConfig({
  plugins: [
    unstave({
      warnAmplification: 5,   // warn when a barrel amplifies imports past this ratio
      serveReport: true,      // serve the live report at /__unstave
      outDir: '.unstave',     // JSON + HTML output dir, relative to the Vite root
    }),
  ],
})
```

Options: `enabled` (run during builds too, or disable explicitly),
`warnAmplification` (default 5), `serveReport` (default true),
`outDir` (default `.unstave`), and `maxNodes` (graph size before directories
collapse in the HTML report).

## Docs

- [Main repository README](https://github.com/eddiesr93/unstave)
- [Documentation](https://eddiesr93.github.io/unstave/docs/)
- [Product site](https://eddiesr93.github.io/unstave/)
