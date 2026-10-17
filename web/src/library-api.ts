import type { ReportDef } from './designer'
export type RunRecord = {
  run_id: string; revision: string; status: string; started_at: string; finished_at: string | null
  artifacts: Record<string, string>; diagnostics: { message: string }[]; parameters?: unknown; source_files: string[]
}
export type ReportDetail = {
  id: string; manifest: { name: string; description: string; tags: string[] }; template: ReportDef
  revision: string; result_state: 'none' | 'current_revision' | 'older_revision'
  display_run_id: string | null; latest_attempt: RunRecord | null
}
export class LibraryError extends Error { constructor(public status: number, message: string) { super(message) } }
async function request<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(url, options)
  const value = await response.json()
  if (!response.ok) throw new LibraryError(response.status, value.error?.message ?? `HTTP ${response.status}`)
  return value as T
}
const base = (id: string) => `/api/reports/${encodeURIComponent(id)}`
export const listReports = () => request<{ items: ReportDetail[] }>('/api/reports?limit=500')
export const getReport = (id: string) => request<ReportDetail>(base(id))
export const getRuns = (id: string) => request<{ items: RunRecord[] }>(`${base(id)}/runs?limit=500`)
export const saveTemplate = (id: string, revision: string, template: ReportDef) => request<ReportDetail>(`${base(id)}/template`, {
  method: 'PUT', headers: { 'Content-Type': 'application/json', 'If-Match': revision }, body: JSON.stringify(template),
})
export const artifactUrl = (id: string, run: string, name: string) => `${base(id)}/runs/${encodeURIComponent(run)}/artifacts/${encodeURIComponent(name)}`
export const sourceUrl = (id: string, run: string, path: string) => `${base(id)}/runs/${encodeURIComponent(run)}/${path.split('/').map(encodeURIComponent).join('/')}`
export const loadWorkbook = (id: string, run: string) => request<unknown>(artifactUrl(id, run, 'workbook.json'))
