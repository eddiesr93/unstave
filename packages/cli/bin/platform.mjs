const SUPPORTED_PLATFORMS = [
  'darwin-arm64',
  'darwin-x64',
  'linux-arm64-gnu',
  'linux-arm64-musl',
  'linux-x64-gnu',
  'linux-x64-musl',
  'win32-x64-msvc',
]

export function platformSuffix() {
  const libc = process.platform === 'linux' && !process.report?.getReport()?.header?.glibcVersionRuntime
    ? 'musl'
    : 'gnu'
  if (process.platform === 'win32') {
    return `${process.platform}-${process.arch}-msvc`
  }
  return process.platform === 'linux'
    ? `${process.platform}-${process.arch}-${libc}`
    : `${process.platform}-${process.arch}`
}

export function isSupportedPlatform() {
  return SUPPORTED_PLATFORMS.includes(platformSuffix())
}

export function binaryFileName() {
  return process.platform === 'win32' ? 'unstave.exe' : 'unstave'
}

export function requiredPlatformPackage() {
  return `@unstave/cli-${platformSuffix()}`
}
