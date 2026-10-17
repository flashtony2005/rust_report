import type { ReportDef } from './designer'
import { AUTH_REQUIRED_EVENT, authToken } from './auth'
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
  const token = authToken()
  const headers = new Headers(options?.headers)
  if (token) headers.set('Authorization', `Bearer ${token}`)
  const response = await fetch(url, { ...options, headers })
  let value: any = null
  try { value = await response.json() } catch { /* non-JSON error body */ }
  if (!response.ok) {
    if (response.status === 401) {
      window.dispatchEvent(new Event(AUTH_REQUIRED_EVENT))
    }
    throw new LibraryError(response.status, value?.error?.message ?? `HTTP ${response.status}`)
  }
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
