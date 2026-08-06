'use strict'

const { existsSync } = require('node:fs')
const { join } = require('node:path')

function platformSuffix() {
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

const platform = platformSuffix()
const localCandidates = [
  join(__dirname, 'unstave.node'),
  join(__dirname, `unstave.${platform}.node`),
]
const localBinding = localCandidates.find(existsSync)

try {
  const binding = localBinding ? require(localBinding) : require(`@unstave/node-${platform}`)
  module.exports.analyze = binding.analyze
  module.exports.renderHtml = binding.renderHtml
} catch (error) {
  error.message = `Unable to load the unstave native binding for ${process.platform}-${process.arch}: ${error.message}`
  throw error
}
