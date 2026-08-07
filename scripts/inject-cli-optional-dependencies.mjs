import { readFileSync, writeFileSync } from 'node:fs'
import { readdirSync } from 'node:fs'

const cliRoot = new URL('../packages/cli/', import.meta.url)
const packageDirectory = new URL('npm/', cliRoot)

const loaderPath = new URL('package.json', cliRoot)
const loader = JSON.parse(readFileSync(loaderPath, 'utf8'))

const platformManifests = readdirSync(packageDirectory, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => new URL(`${entry.name}/package.json`, packageDirectory))

if (platformManifests.length === 0) {
  console.error('no @unstave/cli platform packages found under packages/cli/npm/')
  process.exit(1)
}

const versions = new Set(platformManifests.map((manifest) => JSON.parse(readFileSync(manifest, 'utf8')).version))
if (versions.size !== 1) {
  console.error(`platform packages do not share a single version: ${[...versions].join(', ')}`)
  process.exit(1)
}
if (loader.version !== [...versions][0]) {
  console.error(`loader version ${loader.version} does not match platform packages ${[...versions][0]}`)
  process.exit(1)
}

const version = [...versions][0]
loader.optionalDependencies = {}
for (const manifest of platformManifests) {
  const name = JSON.parse(readFileSync(manifest, 'utf8')).name
  loader.optionalDependencies[name] = `^${version}`
}

writeFileSync(loaderPath, `${JSON.stringify(loader, null, 2)}\n`)
console.log(`injected ${Object.keys(loader.optionalDependencies).length} CLI optional dependencies at ^${version}`)
