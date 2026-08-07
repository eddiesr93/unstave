import { test } from 'node:test'
import assert from 'node:assert/strict'
import { existsSync } from 'node:fs'

import { binaryFileName, isSupportedPlatform, platformSuffix, requiredPlatformPackage } from '../bin/platform.mjs'

test('platformSuffix includes the platform and arch', () => {
  const suffix = platformSuffix()
  assert.ok(suffix.startsWith(process.platform), 'suffix should start with platform')
  assert.ok(suffix.includes(process.arch), 'suffix should include arch')
})

test('host platform is supported with a prebuilt binary', () => {
  assert.ok(isSupportedPlatform(), `${platformSuffix()} should be in the published set`)
})

test('binary file name is .exe on win32', () => {
  const expected = process.platform === 'win32' ? 'unstave.exe' : 'unstave'
  assert.equal(binaryFileName(), expected)
})

test('requiredPlatformPackage maps to @unstave/cli-<suffix>', () => {
  assert.equal(requiredPlatformPackage(), `@unstave/cli-${platformSuffix()}`)
})

test('published platform set covers the release matrix', () => {
  const expected = [
    'darwin-arm64',
    'darwin-x64',
    'linux-arm64-gnu',
    'linux-arm64-musl',
    'linux-x64-gnu',
    'linux-x64-musl',
    'win32-x64-msvc',
  ]
  for (const platform of expected) {
    assert.ok(
      existsSync(new URL(`../npm/${platform}/package.json`, import.meta.url)),
      `expected a packaged manifest for ${platform}`,
    )
  }
})
