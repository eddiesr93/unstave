export interface AnalyzeOptions {
  root?: string
  config?: string
  includeTypeEdges?: boolean
  noCache?: boolean
}

export interface AnalysisReport {
  schemaVersion: 1
  workspace: { root: string; kind: string; packages: unknown[] }
  summary: {
    filesAnalyzed: number
    modules: number
    edges: number
    cycles: number
    classifiedBarrels: number
  }
  amplification: {
    barrels: Array<{
      barrel: string
      maxAmplification: number | null
      totalExcess: number
    }>
  }
  [key: string]: unknown
}

export function analyze(options: AnalyzeOptions): Promise<AnalysisReport>

/**
 * Render a report to one self-contained HTML page.
 *
 * `maxNodes` (default 150) is the point past which the graph collapses directories
 * into single nodes, matching the CLI's `--max-nodes`. Raising it past a few hundred
 * makes the layout slow to settle on large workspaces.
 */
export function renderHtml(report: AnalysisReport, maxNodes?: number): Promise<string>
