#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { chmodSync, existsSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, join } from 'node:path'

import { binaryFileName, isSupportedPlatform, requiredPlatformPackage } from './platform.mjs'

function findBinary() {
  if (process.env.UNSTAVE_CLI_BINARY) {
    return process.env.UNSTAVE_CLI_BINARY
  }
  if (!isSupportedPlatform()) {
    console.error(
      `unstave does not yet ship a prebuilt CLI for ${process.platform}-${process.arch}. ` +
        `Install from source instead: cargo install unstave-cli`,
    )
    process.exit(1)
  }
  const require = createRequire(import.meta.url)
  let packageDirectory
  try {
    packageDirectory = dirname(require.resolve(`${requiredPlatformPackage()}/package.json`))
  } catch {
    console.error(
      `Could not locate the unstave binary for ${requiredPlatformPackage()}. ` +
        `If this package was installed manually, run the install script from the @unstave/cli README.`,
    )
    process.exit(1)
  }
  return join(packageDirectory, 'bin', binaryFileName())
}

const binary = findBinary()

if (!existsSync(binary)) {
  console.error(`unstave binary not found at ${binary}. Reinstall @unstave/cli to restore it.`)
  process.exit(1)
}

if (process.platform !== 'win32') {
  try {
    chmodSync(binary, 0o755)
  } catch {
    // best effort; the binary may be read-only in some caches
  }
}

if (process.env.UNSTAVE_CLI_DEBUG) {
  console.error(`[unstrv] spawning ${binary}`)
}

const result = spawnSync(binary, process.argv.slice(2), {
  stdio: 'inherit',
  cwd: process.cwd(),
})

if (result.error) {
  console.error(`failed to run unstave: ${result.error.message}`)
  process.exit(1)
}

process.exit(result.status ?? 1)
