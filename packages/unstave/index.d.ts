export interface AnalyzeOptions {
  root?: string
  config?: string
  includeTypeEdges?: boolean
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
export function renderHtml(report: AnalysisReport): Promise<string>
