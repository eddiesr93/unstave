import { readFileSync, readdirSync } from 'node:fs'

const expected = process.argv[2]

if (!expected) {
  console.error('usage: node scripts/check-release-version.mjs <version>')
  process.exit(2)
}

const cargo = readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8')
const cargoVersion = cargo.match(/\[workspace\.package\][\s\S]*?\nversion = "([^"]+)"/)?.[1]
const formula = readFileSync(new URL('../Formula/unstave.rb', import.meta.url), 'utf8')
const formulaVersion = formula.match(/tag: "v([^"]+)"/)?.[1]

const packageFiles = [
  new URL('../packages/unstave/package.json', import.meta.url),
  new URL('../packages/vite-plugin-unstave/package.json', import.meta.url),
  new URL('../packages/cli/package.json', import.meta.url),
]
const nativePackages = new URL('../packages/unstave/npm', import.meta.url)
const cliPackages = new URL('../packages/cli/npm', import.meta.url)

for (const directory of readdirSync(nativePackages)) {
  packageFiles.push(new URL(`../packages/unstave/npm/${directory}/package.json`, import.meta.url))
}
for (const directory of readdirSync(cliPackages)) {
  packageFiles.push(new URL(`../packages/cli/npm/${directory}/package.json`, import.meta.url))
}

const versions = new Map([
  ['Cargo workspace', cargoVersion],
  ['Homebrew formula', formulaVersion],
])
for (const packageFile of packageFiles) {
  const manifest = JSON.parse(readFileSync(packageFile, 'utf8'))
  versions.set(manifest.name, manifest.version)
}

const mismatches = [...versions].filter(([, version]) => version !== expected)
if (mismatches.length > 0) {
  for (const [name, version] of mismatches) {
    console.error(`${name}: expected ${expected}, found ${version ?? 'no version'}`)
  }
  process.exit(1)
}

console.log(`release version ${expected} matches ${versions.size} manifests`)
