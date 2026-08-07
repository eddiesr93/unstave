import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'

const root = new URL('../', import.meta.url)
const cliManifest = JSON.parse(readFileSync(new URL('packages/cli/package.json', root), 'utf8'))
const version = cliManifest.version

const TARGETS = [
  { platform: 'darwin-arm64', rust: 'aarch64-apple-darwin', os: ['darwin'], cpu: ['arm64'], exe: false },
  { platform: 'darwin-x64', rust: 'x86_64-apple-darwin', os: ['darwin'], cpu: ['x64'], exe: false },
  { platform: 'linux-arm64-gnu', rust: 'aarch64-unknown-linux-gnu', os: ['linux'], cpu: ['arm64'], exe: false },
  { platform: 'linux-arm64-musl', rust: 'aarch64-unknown-linux-musl', os: ['linux'], cpu: ['arm64'], exe: false },
  { platform: 'linux-x64-gnu', rust: 'x86_64-unknown-linux-gnu', os: ['linux'], cpu: ['x64'], exe: false },
  { platform: 'linux-x64-musl', rust: 'x86_64-unknown-linux-musl', os: ['linux'], cpu: ['x64'], exe: false },
  { platform: 'win32-x64-msvc', rust: 'x86_64-pc-windows-msvc', os: ['win32'], cpu: ['x64'], exe: true },
]

for (const { platform, rust, os, cpu, exe } of TARGETS) {
  const directory = new URL(`packages/cli/npm/${platform}/`, root)
  mkdirSync(directory, { recursive: true })

  const name = `@unstave/cli-${platform}`
  const binary = `bin/${exe ? 'unstave.exe' : 'unstave'}`
  const manifest = {
    name,
    version,
    cpu,
    files: [binary],
    bin: { unstave: binary },
    description: 'The unstave CLI binary for your platform',
    license: 'MIT',
    engines: { node: '>=20.19' },
    repository: 'github:eddiesr93/unstave',
    publishConfig: { registry: 'https://registry.npmjs.org/', access: 'public' },
    os,
  }
  writeFileSync(
    new URL('package.json', directory),
    `${JSON.stringify(manifest, null, 2)}\n`,
  )
  writeFileSync(
    new URL('README.md', directory),
    `# \`${name}\`\n\nThis is the **${rust}** binary for \`@unstave/cli\`. It is installed\nautomatically as an optional dependency; use the \`unstave\` command from\n\`@unstave/cli\`.\n`,
  )
  console.log(`generated ${name} v${version} (${rust})`)
}
