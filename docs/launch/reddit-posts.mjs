export const SUBREDDITS = {
  typescript: {
    url: 'https://www.reddit.com/r/typescript/',
    title: 'One import loads 1,000 modules. I built a tool to find and remove the barrel files causing it.',
    text: `I'm not here to sell you a linter. I noticed "why is my dev server slow" is always answered with the same guesses - so I went looking for the actual number.

A barrel file (index.ts that re-exports a folder) is one node that depends on everything behind it. My dev server resolves and transforms the *whole* closure, even though production tree-shaking makes the shipped bundle fine. So the cost is real but invisible in every metric you watch.

On one setup, this import:
    import { Client0 } from '@/clients'
pulled 1,047 modules. The app needed 3 of them.

I wrote unstave (Rust, built on oxc) to:
- map the module graph and rank barrel amplification
- surface cycles and dead exports
- rewrite imports to the declaring file, byte-safely

Warmed up it analyzes 6,000 files in ~117ms. I ran it against pinned revisions of Vite, TanStack Query, and Astro, and the rewritten code passed each project's own build, typecheck, and tests (one Astro import went from 143 modules of closure).

I'd genuinely like the TS crowd's take on the barrel-detection thresholds and the codemod safety rules before I call it 1.0.

Try it without installing Rust: npx @unstave/cli analyze
Repo: https://github.com/eddiesr93/unstave`,
  },
  reactjs: {
    url: 'https://www.reddit.com/r/reactjs/',
    title: 'Vite dev server slow? One barrel import was dragging ~1,000 files into my graph.',
    text: `If your Vite dev server gets slower as the codebase grows, check your barrel files (index.ts re-export hubs) before you blame the bundler.

A barrel is one node that eagerly resolves everything behind it in dev. I found an import that pulled ~1,000 modules when it needed 3. Production was fine - tree-shaking hides it - so the dev cost kept compounding invisibly.

Built a small CLI (Rust + oxc) that maps the Vite-relevant module graph, ranks barrel amplification, and offers a safe rewrite. There's also a Vite plugin that serves a live /__unstave report as a non-blocking dev-mode plugin.

The part that took longest is the codemod: proving where a symbol is actually declared before touching an import. Ambiguous cases are left alone.

Feedback very welcome. For the curious, without Rust:
    npx @unstave/cli analyze
Repo: https://github.com/eddiesr93/unstave`,
  },
  programming: {
    url: 'https://www.reddit.com/r/programming/',
    title: 'Barrel files are an unpaid performance tax in TS dev servers - a tool that measures and removes them',
    text: `Short version: an import looks like one file, but a dev server resolves the whole re-export closure behind an index.ts. Production tree-shakes it away, so the cost is invisible in the metrics that matter - and it compounds for years.

unstave (Rust, oxc-based) treats this as an instrumentable, fixable problem:
1. Map the module graph - 5 edge kinds, tsconfig aliases, package exports.
2. Rank barrels by amplification, find cycles and dead exports.
3. Rewrite imports to declaration sites, byte-preserving, dry-run first.

Performance: 6,000 files in ~117ms warm. Validated against Vite, TanStack Query, and Astro (excess went 167 => 65 on one real Astro barrel) - every rewrite passed the projects' own tests.

Trade-off I'm honest about in the README: it does no type-checking and no bundle-size analysis. It's a measurement instrument + codemod, deliberately.

npx @unstave/cli analyze   # try on your own repo, no Rust install
https://github.com/eddiesr93/unstave`,
  },
  rust: {
    url: 'https://www.reddit.com/r/rust/',
    title: 'Barrel files are an unpaid performance tax in TS dev servers - a tool that measures and removes them',
    text: `Short version: an import looks like one file, but a dev server resolves the whole re-export closure behind an index.ts. Production tree-shakes it away, so the cost is invisible in the metrics that matter - and it compounds for years.

unstave (Rust, oxc-based) treats this as an instrumentable, fixable problem:
1. Map the module graph - 5 edge kinds, tsconfig aliases, package exports.
2. Rank barrels by amplification, find cycles and dead exports.
3. Rewrite imports to declaration sites, byte-preserving, dry-run first.

Performance: 6,000 files in ~117ms warm. Validated against Vite, TanStack Query, and Astro (excess went 167 => 65 on one real Astro barrel) - every rewrite passed the projects' own tests.

The oxc parse + resolution pipeline makes a 6,000-file graph fast enough to fit in the dev loop; the codemod does span-based, byte-preserving edits. No type-checking, no bundle-size analysis - deliberately just measurement + a safe codemod.

npx @unstave/cli analyze   # try on your own repo, no Rust install
https://github.com/eddiesr93/unstave`,
  },
}
