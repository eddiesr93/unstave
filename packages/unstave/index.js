'use strict'

const { existsSync } = require('node:fs')
const { join } = require('node:path')

function platformPackage() {
  const libc = process.platform === 'linux' && !process.report?.getReport()?.header?.glibcVersionRuntime
    ? 'musl'
    : 'gnu'
  const suffix = process.platform === 'linux'
    ? `${process.platform}-${process.arch}-${libc}`
    : `${process.platform}-${process.arch}`
  return `unstave-${suffix}`
}

const platform = process.platform === 'linux'
  ? `${process.platform}-${process.arch}-${process.report?.getReport()?.header?.glibcVersionRuntime ? 'gnu' : 'musl'}`
  : `${process.platform}-${process.arch}`
const localCandidates = [
  join(__dirname, 'unstave.node'),
  join(__dirname, `unstave.${platform}.node`),
]
const localBinding = localCandidates.find(existsSync)

try {
  const binding = localBinding ? require(localBinding) : require(platformPackage())
  module.exports.analyze = binding.analyze
  module.exports.renderHtml = binding.renderHtml
} catch (error) {
  error.message = `Unable to load the unstave native binding for ${process.platform}-${process.arch}: ${error.message}`
  throw error
}
