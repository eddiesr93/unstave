'use strict'

const { test } = require('node:test')
const assert = require('node:assert/strict')
const { mkdtempSync, writeFileSync } = require('node:fs')
const { tmpdir } = require('node:os')
const { join } = require('node:path')

const { analyze, renderHtml } = require('..')

const SIMPLE = join(__dirname, '../../../tests/fixtures/simple')
const TYPE_REEXPORT = join(__dirname, '../../../tests/fixtures/type-reexport')

test('native binding exposes exactly analyze and renderHtml', () => {
  assert.deepEqual(Object.keys(require('..')).sort(), ['analyze', 'renderHtml'])
})

test('analyze and renderHtml cross the async native boundary', async () => {
  const report = await analyze({
    root: SIMPLE,
    noCache: true,
  })
  assert.equal(report.schemaVersion, 1)
  assert.equal(report.summary.filesAnalyzed, 3)

  const html = await renderHtml(report)
  assert.match(html, /^<!doctype html>/)
  assert.match(html, /"schemaVersion":1/)
})

test('includeTypeEdges changes which modules reachability analyses count', async () => {
  // `type-reexport` links main -> barrel -> load only through a type-only import
  // chain, so the DTO modules are invisible to fan unless type edges are included.
  const withoutType = await analyze({
    root: TYPE_REEXPORT,
    noCache: true,
    includeTypeEdges: false,
  })
  const withType = await analyze({
    root: TYPE_REEXPORT,
    noCache: true,
    includeTypeEdges: true,
  })

  const fanInPaths = (report) => report.fan.fanIn.map((entry) => entry.path)

  assert.ok(
    !fanInPaths(withoutType).includes('src/clients/models/ThingDto.ts'),
    'type-only module should be absent from fan-in when type edges are excluded',
  )
  assert.ok(
    fanInPaths(withType).includes('src/clients/models/ThingDto.ts'),
    'type-only module should appear in fan-in when type edges are included',
  )

  const thingDto = withType.fan.fanIn.find((entry) =>
    entry.path.endsWith('ThingDto.ts'),
  )
  assert.equal(thingDto.direct, 2)
})

test('a config file is loaded and respected', async () => {
  // Restrict analysis to main.ts via the `include` glob; filesAnalyzed should drop.
  const dir = mkdtempSync(join(tmpdir(), 'unstave-binding-'))
  const config = join(dir, 'unstave.toml')
  writeFileSync(config, 'include = ["src/main.ts"]\n')

  const report = await analyze({
    root: SIMPLE,
    config,
    noCache: true,
  })
  assert.equal(report.summary.filesAnalyzed, 1)
})

test('cache and noCache both return a valid report and are consistent', async () => {
  const cached = await analyze({ root: SIMPLE })
  const noCache = await analyze({ root: SIMPLE, noCache: true })

  assert.equal(cached.schemaVersion, 1)
  assert.equal(noCache.schemaVersion, 1)
  assert.equal(cached.summary.filesAnalyzed, 3)
  assert.equal(noCache.summary.filesAnalyzed, 3)
  assert.equal(
    cached.summary.filesAnalyzed,
    noCache.summary.filesAnalyzed,
    'cached and uncached analysis should agree on the file set',
  )
})

test('renderHtml accepts maxNodes and still returns valid output', async () => {
  // No fixture is large enough to cross the default 150-node collapse threshold,
  // so assert the option is accepted and the result remains a valid page.
  const report = await analyze({ root: SIMPLE, noCache: true })
  const html = await renderHtml(report, 1)
  assert.match(html, /^<!doctype html>/)
  assert.match(html, /"schemaVersion":1/)
})

test('analyze rejects when the root does not exist', async () => {
  await assert.rejects(
    analyze({ root: join(__dirname, 'definitely-not-a-real-workspace'), noCache: true }),
    (err) => {
      assert.match(err.message, /No such file or directory/)
      // The structured `unstave_core::Error` variant is surfaced to JS via `err.cause`.
      assert.equal(err.cause?.message, 'Io')
      return true
    },
  )
})
