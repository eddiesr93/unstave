import { test } from 'node:test'
import assert from 'node:assert/strict'
import { EventEmitter } from 'node:events'
import { cp, mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import unstave from '../dist/index.js'

test('plugin is non-blocking and defaults to the expected Vite hooks', () => {
  const plugin = unstave()
  assert.equal(plugin.name, 'vite-plugin-unstave')
  assert.equal(typeof plugin.configureServer, 'function')
  assert.equal(typeof plugin.buildEnd, 'function')
})

test('plugin can be explicitly disabled', () => {
  const plugin = unstave({ enabled: false })
  const middlewareUses = []
  plugin.configResolved?.({ command: 'serve', root: process.cwd() })
  const result = plugin.configureServer?.({
    watcher: { on() { throw new Error('disabled plugin attached a watcher') } },
    middlewares: { use(...args) { middlewareUses.push(args) } },
  })
  assert.equal(result, undefined)
  assert.equal(middlewareUses.length, 0)
})

test('dev analysis starts without blocking and serves a computing placeholder', async () => {
  const plugin = unstave()
  const middlewares = []
  const watcher = new EventEmitter()
  plugin.configResolved?.({
    command: 'serve',
    root: fixture('simple'),
    logger: { error() {}, warn() {} },
  })

  const returned = plugin.configureServer?.({
    watcher,
    middlewares: { use(route, handler) { middlewares.push([route, handler]) } },
  })

  assert.equal(returned, undefined, 'configureServer must not await analysis')
  assert.equal(middlewares[0][0], '/__unstave')
  assert.match(request(middlewares[0][1]), /computing the module graph/)

  let page = ''
  for (let attempt = 0; attempt < 50; attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 10))
    page = request(middlewares[0][1])
    if (page.includes('unstave-data')) break
  }
  assert.match(page, /id="unstave-data"/)
})

test('enabled production build writes JSON and HTML reports', async () => {
  const root = await mkdtemp(join(tmpdir(), 'unstave-vite-test-'))
  await cp(fixture('simple'), root, { recursive: true })
  const plugin = unstave({ enabled: true, outDir: '.analysis' })
  plugin.configResolved?.({
    command: 'build',
    root,
    logger: { error() {}, warn() {} },
  })

  await plugin.buildEnd?.()

  const json = JSON.parse(await readFile(join(root, '.analysis/unstave-report.json'), 'utf8'))
  const html = await readFile(join(root, '.analysis/unstave-report.html'), 'utf8')
  assert.equal(json.schemaVersion, 1)
  assert.match(html, /^<!doctype html>/)
  await rm(root, { recursive: true, force: true })
})

function request(middleware) {
  let body = ''
  middleware({}, {
    setHeader() {},
    end(value) { body = value },
  })
  return body
}

function fixture(name) {
  return join(dirname(fileURLToPath(import.meta.url)), '../../../tests/fixtures', name)
}
