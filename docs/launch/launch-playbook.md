# unstave Launch Playbook — 100 Stars in a Sprint

Status: **0 stars.** Goal: **≥100 stars** on `github.com/eddiesr93/unstave`.

**LAUNCH READY ✅ — as of 2026-08-07:**
- `@unstave/cli` 0.2.2 is **live on npm** (verified: `npx @unstave/cli --version`
  → `unstave 0.2.2`), plus 7 prebuilt platform packages.
- crates.io + npm all published; release page
  `releases/tag/v0.2.2` titled and noted; CI matrix green on every target.
- `npx @unstave/cli analyze` is the zero-friction entry point for the TS/React
  audience. The posts below are final and ready to publish.

The product is genuinely differentiated and the README/site are strong. Stars at
this stage come from **distribution, not code**. The plan below is ordered by
impact. Every post is drafted for you to approve and publish.

---

## 0. One-line reality check

Stars follow **first-day signal + repeatable discovery**. 100 stars from a cold
repo means: (1) a strong launch burst that lands on a front page somewhere, and
(2) people finding it later via npm/crates.io/GitHub search. Both need the
「try it in 30 seconds」path to exist. Right now **it doesn't** for the JS majority
audience. Fix that first, then launch.

---

## 1. The one code gap that mattered most — now implemented ✅

The flagship CLI was not distributable to the target audience (TS/React = npm
users). Resolved with **`@unstave/cli`**: an npm package that ships the prebuilt
native binary per platform and shims it directly.

```bash
npx @unstave/cli analyze        # no Rust, no project changes — try it anywhere
npx @unstave/cli fix --dry-run  # see the diff, nothing touched
```

Implementation notes:
- `@unstave/cli` (loader) + 7 platform packages (`@unstave/cli-<os>-<arch>`):
  darwin arm64/x64, linux arm64/x64 (glibc + musl), win32 x64. Windows on Arm
  falls back to `cargo install` with a clear message.
- Loader is a thin `spawnSync` shim (`packages/cli/bin/unstave.mjs`); behavior is
  byte-identical to the crates.io binary.
- `optionalDependencies` are injected at publish time only (keeps local/CI
  `pnpm install` hermetic) via `scripts/inject-cli-optional-dependencies.mjs`;
  release.yml `build-cli` job cross-compiles the binaries (`cargo zigbuild` for
  linux) and `publish-npm` publishes all eight packages with provenance.
- Verified end-to-end locally: `npm pack` → install tarballs → `unstave
  analyze/barrels/fix --dry-run` all run through the shim with exit 0.

**Remaining quick wins:**
- Homebrew `brew install unstave` — confirm the tap actually works end-to-end
  (`Formula/unstave.rb` + tap). A broken `brew` path at launch is an own-goal.
- An `asciinema`/`.gif` of `unstave analyze` on a mid-size real repo in the README
  hero. Text tables are great; a moving terminal demo converts better than words.
- `unstave demo` (nice-to-have): generate a synthetic project and analyze it in
  one shot using `gen_synthetic`, so any livestream/quote is trivially
  reproducible with zero config.

---

## 2. Launch schedule (sprint)

All infrastructure is shipped. The burst is a **one-week push**, concentrated so
traffic compounds. Dates below assume today is Fri 2026-08-07; Show HN performs
best **Tue–Thu, early morning US Pacific** (the "why is this front-page-worthy"
judgement hours are US morning; EU comes online mid-morning).

| Day | Move | Windows / notes |
|-----|------|-----------------|
| **Tue 08-11** ~7:00 PT | **Show HN** post — the big swing | Tue heartbeat; aim 7:00–7:30 PT, not the weekend/Mon/Fri |
| Tue 08-11 ~later | **X thread** with live report images | ride Show HN attention |
| **Wed 08-12** ~6:00 PT | Reddit `r/typescript` (question-led) | weekday mornings US/EU |
| Wed 08-12 ~9:00 PT | Reddit `r/reactjs` (Vite slowness angle) | different text, same proof |
| **Thu 08-13** ~6:00 PT | Reddit `r/programming` + `r/rust` (perf angle) | broad reach |
| Thu 08-13 | DEV.to cross-post (update the lead, add CTA) | `#typescript #react #vite #showdev` |
| **Fri 08-14** | LinkedIn (professional: monorepo perf, measurable ROI) | weekday |
| **Mon 08-17+** | OSS newsletters / mega-lists / Product Hunt | long tail |
| ongoing | Reply to every comment; fold answers into a FAQ; re-engage | retention |

Never post the same text twice; each channel gets its own angle. Show HN first
comment should be substantive (explain the invisible dev-server cost mechanic) so
the thread doesn't read as a bare link drop.

---

## 3. SEO / discovery quick wins (already partly done — finish these)

Verified already in place: GitHub topics (10 good ones), site has schema.org +
OG/Twitter + sitemap + canonical, crates.io/npm READMEs, `keywords` metadata.

Still open:
- [x] **`@unstave/cli` npm path** — shipped in 0.2.2; linked from the README
  install block (first), and the site install panel defaults to `npx`.
- [x] **GitHub release page** for v0.2.2 — titled and noted (`releases/tag/v0.2.2`).
- [ ] **crates.io description** is good but generic. Consider a more search-y line
  once 1.0-ish: mention "barrel files", "dead exports", "import cycles". (Republish
  only alongside a release — don't churn.)
- [ ] **npm READMEs**: `@unstave/node` and `@unstave/vite-plugin` already link the
  CLI; confirm the homepage + description keywords stay current after the launch.
- [ ] **Search intent coverage**: people search "barrel files typescript",
  "why is my vite dev server slow", "index.ts re-export too many modules". Make
  sure the README + one docs page literally contain those phrases. The site page
  already covers the concept well.
- [ ] Add the repo to **OSS newsletters / dev-tool roundups** (see §5).

---

## 4. Drafted launch assets

Paste these after approving. Replace `[NAME]`/`[handle]` placeholders.

### 4.0 Assets for the posts (run these yourself — 30 seconds)

The posts reference real output. On **any TypeScript repo you have** (or the
repo's own fixture for a tiny example), run:

```bash
npx @unstave/cli analyze --format terminal --format html     # big summary + HTML
npx @unstave/cli barrels                                     # amplification table
npx @unstave/cli fix --dry-run                               # the diff, nothing touched
```

Screenshot the **barrels table** (X post 4) and the **fix diff** (X post 6); the
HTML opens as the interactive graph you can record. Producer's own fixture gives
a clean, small output to demo (nested-barrels, 7 files):

```
$ npx @unstave/cli barrels --root nested-barrels
3 barrel(s) classified, 1 of them imported

Barrel amplification
┌────────────────┬───────┬──────┬────────┬───────┬──────┬────────────┐
│ barrel         ┆ sites ┆ cost ┆ excess ┆ worst ┆ amp  ┆ rewritable │
╞════════════════╪═══════╪══════╪════════╪═══════╪══════╪════════════╡
│ src/a/index.ts ┆ 1     ┆ 6    ┆ 5      ┆ 5     ┆ 6.0× ┆ 1/1        │
└────────────────┴───────┴──────┴────────┴───────┴──────┴────────────┘

$ npx @unstave/cli fix --root nested-barrels
--- a/src/main.ts
+++ b/src/main.ts
@@ -1,2 +1,2 @@
-import { one } from './a';
+import { one } from './a/b/c/one';
 export const used = one;
1 file(s) would change, 1 import(s) would be rewritten
```

Live HTML report for the same fixture: https://eddiesr93.github.io/unstave/sample-report.html

### 4.1 Show Hacker News

**Title options (pick one; HN titles are critical):**
1. `Show HN: I built a tool that finds why one import loads 1,000 modules`
2. `Show HN: unstave – measure and remove TypeScript barrel-file bloat`
3. `Show HN: unstave – find the import that drags 1,000 modules into your dev graph`

**Body:**
```
Barrel files (index.ts re-exports) look free:
    import { Client0 } from '@/clients'   # looks like one file

but your dev server eagerly resolves the whole re-export closure. On a real
workspace this single import pulls in 1,047 modules when it needs 3 — 349x.

unstave maps your TypeScript/React module graph (Rust + oxc), ranks every barrel
by how much it amplifies your imports, then rewrites imports to point at the
actual declaration sites — byte-safely, dry-run by default.

    unstave barrels                # ranked amplification table
    unstave analyze --format html  # interactive graph report
    unstave fix --write            # prove + rewrite each import

Analyzing 6,000 files: 117ms warm / 234ms cold (M4 Pro).

Validated against pinned revisions of Vite, TanStack Query, and Astro — one Astro
import reached 143 modules, and every rewrite passed that project's own build,
typecheck, and tests.

Try it on your own repo without installing Rust:
    npx @unstave/cli analyze

Repo: https://github.com/eddiesr93/unstave
Docs: https://eddiesr93.github.io/unstave/

The claim "metrics are invisible because dev tools resolve eagerly but production
tree-shakes it away" is the whole pitch — barrels were never a Sin, just an unpaid
tax. I'd love feedback on the analysis model and the codemod safety rules.
```

**Comment strategy:** the first 4–6 comments on Show HN determine trajectory.
Pre-arrange 1–2 knowledgeable people (or post a substantive first comment
explaining the "invisible dev-server cost" mechanic) so it doesn't look empty.

### 4.2 X / Twitter thread

One thread, ~8 tweets, with 2–3 images (the amplification table + HTML report):

```
1/ One TypeScript import can quietly load 1,000 files into your dev server.

It's not a bug. It's a barrel file — an index.ts that re-exports a directory.
Dev servers resolve the whole closure eagerly. Production tree-shakes it away,
so the cost never shows up in the metrics you watch.

2/ That's why it survives for years. "Looks fine, ships fine."

3/ So I built unstave — a tool that measures it, then removes it. Rust + oxc.

4/ Measure: rank every barrel by how much it amplifies your imports.
    unstave barrels

[IMAGE: amplification table]

5/ See it: interactive graph of everything one import drags in.
    unstave analyze --format html

[IMAGE: HTML report]

6/ Fix it: rewrite imports to point at the real declaration sites.
Byte-preserving, ambiguous cases left untouched, dry-run by default.
    unstave fix --dry-run

7/ The receipt: 6,000 files analyzed in ~117ms. Validated against Vite,
TanStack Query, and Astro — every rewrite passed their own test suites.

8/ Try it on your own repo. No Rust required.
    npx @unstave/cli analyze

Repo: github.com/eddiesr93/unstave

#typescript #react #vite #buildperformance
```

### 4.3 Reddit — r/typescript

Title: `One import loads 1,000 modules. I built a tool to find and remove the barrel files causing it.`

Body:
```
I'm not here to sell you a linter. I noticed "why is my dev server slow" is
always answered with the same guesses — so I went looking for the actual number.

A barrel file (index.ts that re-exports a folder) is one node that depends on
everything behind it. My dev server resolves and transforms the *whole* closure,
even though production tree-shaking makes the shipped bundle fine. So the cost is
real but invisible in every metric you watch.

On one setup, this import:
    import { Client0 } from '@/clients'
pulled 1,047 modules. The app needed 3 of them.

I wrote unstave (Rust, built on oxc) to:
- map the module graph and rank barrel amplification
- surface cycles and dead exports
- rewrite imports to the declaring file, byte-safely

Warmed up it analyzes 6,000 files in ~117ms. I ran it against pinned revisions of
Vite, TanStack Query, and Astro, and the rewritten code passed each project's own
build, typecheck, and tests (one Astro import went from 143 modules of closure).

I'd genuinely like the TS crowd's take on the barrel-detection thresholds and the
codemod safety rules before I call it 1.0.

Try it without installing Rust: npx @unstave/cli analyze
Repo: https://github.com/eddiesr93/unstave
```

### 4.4 Reddit — r/reactjs (Vite angle)

Title: `Vite dev server slow? One barrel import was dragging ~1,000 files into my graph.`

Body:
```
If your Vite dev server gets slower as the codebase grows, check your barrel
files (index.ts re-export hubs) before you blame the bundler.

A barrel is one node that eagerly resolves everything behind it in dev. I found
an import that pulled ~1,000 modules when it needed 3. Production was fine —
tree-shaking hides it — so the dev cost kept compounding invisibly.

Built a small CLI (Rust + oxc) that maps the Vite-relevant module graph, ranks
barrel amplification, and offers a safe rewrite. The Vite plugin that serves a
live /__unstave report as a non-blocking dev-mode plugin is here too.

The bit that took the longest is the codemod: proving where a symbol is actually
declared before touching an import. Ambiguous cases are left alone.

Feedback very welcome. For the curious, without Rust:
    npx @unstave/cli analyze
Repo: https://github.com/eddiesr93/unstave
```

### 4.5 Reddit — r/programming (or r/rust)

Title: `Barrel files are an unpaid performance tax in TS dev servers — here's a tool that measures and removes them`

Body:
```
Short version: an import looks like one file, but a dev server resolves the whole
re-export closure behind an index.ts. Production tree-shakes it away, so the cost
is invisible in the metrics that matter — and it compounds for years.

unstave (Rust, oxc-based) treats this as an instrumentable, fixable problem:
1. Map the module graph — 5 edge kinds, tsconfig aliases, package exports.
2. Rank barrels by amplification, find cycles and dead exports.
3. Rewrite imports to declaration sites, byte-preserving, dry-run first.

Performance: 6,000 files in ~117ms warm. Validated against Vite, TanStack Query,
and Astro (excess went 167 → 65 on one real Astro barrel) — every rewrite passed
the projects' own tests.

Trade-off I'm honest about in the README: it does no type-checking and no
bundle-size analysis. It's a measurement instrument + codemod, deliberately.

npx @unstave/cli analyze   # try on your own repo, no Rust install
https://github.com/eddiesr93/unstave
```

### 4.6 DEV.to cross-post

Cross-post the existing launch article
(`docs/articles/what-barrel-files-actually-cost.md`) with a new under-the-fold CTA
and a "try it" line near the top. Add tags: `typescript`, `react`, `vite`,
`showdev`. Title: **"What barrel files actually cost (and how to remove them)"**.

### 4.7 LinkedIn

```
Most "why is my dev server slow" advice is guessing. I started from a number:

One import — import { Client0 } from '@/clients' — pulled ~1,000 modules into the
dev-server graph. The app needed 3.

That cost is invisible because production tree-shaking hides it while development
resolves everything eagerly. So it compounds for years, in every monorepo.

I open-sourced a fix: unstave (Rust + oxc) maps your TypeScript module graph,
ranks barrel amplification, and safely rewrites imports to the declaration site —
byte-preserving, dry-run by default. Validated against Vite, TanStack Query, and
Astro; 6,000 files in ~117ms on an M4 Pro.

Try it on your own repo: npx @unstave/cli analyze
MIT → github.com/eddiesr93/unstave

If you've hit "slow dev server in a big TS repo," I'd love to hear your actual
cause. Mine turned out to be barrels.
```

---

## 5. Long-tail distribution (set-and-forget)

- [ ] Add to **OSS newsletters**: Awesome Rust (curated), TypeScript Weekly, React
  Status, JavaScript Weekly submission form, "This Week in Rust".
- [ ] Add to **awesome lists**: sign-up/via PR to `awesome-rust`,
  `awesome-typescript`, `awesome-react`, `awesome-vite` where applicable.
- [ ] **ProductHunt** — a launch on PH is worth a few dozen eyes even with a cold
  base; schedule post-burst.
- [ ] **Discuss the problem, not the tool**: search GitHub issues / HN / Reddit for
  "dev server slow", "barrel files", "too many modules" and give concrete, helpful
  answers that link the tool only where apt. This is the most reliable source of
  genuine, qualified stars.
- [ ] **GitHub social proof**: CONTRIBUTING and **Discussions** are both live and
  enabled on the repo (verified 2026-08-07). Link the categories from posts.
- [ ] **Star button priming**: the README already asks for stars in a tasteful,
  outcome-based way ("if it surfaces a real bottleneck"). Keep it.

---

## 6. Metrics & course-correct

- Stars/day target for the burst: **+15–25/day for first 3 days**, then taper.
- Front-page Show HN ≈ biggest lever; if it misses, ride the Reddit + X + newsletter
  tail.
- Watch **where traffic actually converts** (site nav → docs → repo). GitHub repo
  Traffic tab (needs auth) shows referrers; prioritize the top 2.

---

## 7. What I need from you to proceed

1. **Publish the posts** in §4 on the schedule in §2. Everything is live and
   verified; the only remaining decision is when you press publish.
2. OPTIONAL: tell me your **X/LinkedIn handles** to slot into placeholders, or
   leave generic.
3. OPTIONAL: pick the **HN title** from §4.1 (default recommendation: #1 —
   `Show HN: I built a tool that finds why one import loads 1,000 modules`).

Then: watch the Metrics in §6 and feed answers back — I'll turn comment themes
into a FAQ and iterate the next release around what the community cares about.
