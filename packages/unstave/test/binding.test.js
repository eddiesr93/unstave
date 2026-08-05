'use strict'

const { test } = require('node:test')
const assert = require('node:assert/strict')
const { join } = require('node:path')

const { analyze, renderHtml } = require('..')

test('native binding exposes exactly analyze and renderHtml', () => {
  assert.deepEqual(Object.keys(require('..')).sort(), ['analyze', 'renderHtml'])
})

test('analyze and renderHtml cross the async native boundary', async () => {
  const report = await analyze({
    root: join(__dirname, '../../../tests/fixtures/simple'),
    noCache: true,
  })
  assert.equal(report.schemaVersion, 1)
  assert.equal(report.summary.filesAnalyzed, 3)

  const html = await renderHtml(report)
  assert.match(html, /^<!doctype html>/)
  assert.match(html, /"schemaVersion":1/)
})
