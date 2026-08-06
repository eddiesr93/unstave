import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

import { analyze, renderHtml } from '@unstave/node'
import type { AnalysisReport } from '@unstave/node'
import type { Plugin, ResolvedConfig, ViteDevServer } from 'vite'

export interface UnstaveOptions {
  /** Enable explicitly. By default the plugin runs only for the dev server. */
  enabled?: boolean
  /** Warn when a barrel reaches this amplification ratio. */
  warnAmplification?: number
  /** Serve the live report at `/__unstave`. */
  serveReport?: boolean
  /** JSON and HTML output directory, relative to the Vite root. */
  outDir?: string
  /** Graph nodes in the HTML report before directories collapse into one node. */
  maxNodes?: number
}

const COMPUTING_HTML = `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="refresh" content="1"><title>unstave</title></head><body style="font:16px system-ui;background:#121510;color:#edfbd6;padding:2rem">unstave is computing the module graph…</body></html>`
const SOURCE_FILE = /\.(?:[cm]?[jt]sx?)$/

export default function unstave(options: UnstaveOptions = {}): Plugin {
  const warnAmplification = options.warnAmplification ?? 5
  const serveReport = options.serveReport ?? true
  const outDir = options.outDir ?? '.unstave'
  const maxNodes = options.maxNodes
  let config: ResolvedConfig | undefined
  let report: AnalysisReport | undefined
  let html: string | undefined
  let running: Promise<void> | undefined
  let rerunRequested = false
  let debounce: NodeJS.Timeout | undefined

  const run = async (): Promise<void> => {
    if (!config) return
    if (running) {
      rerunRequested = true
      return running
    }
    running = (async () => {
      try {
        const nextReport = await analyze({ root: config.root })
        const nextHtml = await renderHtml(nextReport, maxNodes)
        report = nextReport
        html = nextHtml
        warn(config, nextReport, warnAmplification)
      } catch (error) {
        config.logger.error(`[unstave] ${messageOf(error)}`)
      }
    })().finally(() => {
      running = undefined
      if (rerunRequested) {
        rerunRequested = false
        void run()
      }
    })
    return running
  }

  const schedule = (): void => {
    if (debounce) clearTimeout(debounce)
    debounce = setTimeout(() => void run(), 150)
  }

  return {
    name: 'vite-plugin-unstave',
    enforce: 'post',
    configResolved(resolved) {
      config = resolved
    },
    configureServer(server: ViteDevServer) {
      if (!isEnabled(options, config, 'serve')) return
      queueMicrotask(() => void run())
      const changed = (path: string): void => {
        if (SOURCE_FILE.test(path) && !path.startsWith(resolve(config?.root ?? '.', outDir))) {
          schedule()
        }
      }
      server.watcher.on('add', changed)
      server.watcher.on('change', changed)
      server.watcher.on('unlink', changed)
      if (serveReport) {
        server.middlewares.use('/__unstave', (_request, response) => {
          response.statusCode = 200
          response.setHeader('content-type', 'text/html; charset=utf-8')
          response.setHeader('cache-control', 'no-store')
          response.end(running || !html ? COMPUTING_HTML : html)
        })
      }
    },
    async buildEnd(error) {
      if (error || !isEnabled(options, config, 'build') || !config) return
      await run()
      if (!report || !html) return
      const directory = resolve(config.root, outDir)
      await mkdir(directory, { recursive: true })
      await Promise.all([
        writeFile(resolve(directory, 'unstave-report.json'), `${JSON.stringify(report, null, 2)}\n`),
        writeFile(resolve(directory, 'unstave-report.html'), html),
      ])
    },
  }
}

function isEnabled(
  options: UnstaveOptions,
  config: ResolvedConfig | undefined,
  command: 'serve' | 'build',
): boolean {
  if (options.enabled !== undefined) return options.enabled
  return (config?.command ?? command) === 'serve'
}

function warn(config: ResolvedConfig, report: AnalysisReport, threshold: number): void {
  for (const barrel of report.amplification.barrels) {
    if (barrel.maxAmplification !== null && barrel.maxAmplification > threshold) {
      config.logger.warn(
        `[unstave] ${barrel.barrel} amplifies imports ${barrel.maxAmplification.toFixed(1)}× ` +
          `(${barrel.totalExcess} excess module edges)`,
      )
    }
  }
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
