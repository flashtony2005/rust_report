export interface GalleryInput { name: string; kind: string; file?: string; sheet?: string; dsn?: string }
export interface GalleryCheck { label: string; origin: number[]; actual: unknown[]; passed: boolean }
export interface GalleryReport {
  id: string; title: string; section: string; description: string; features: string[]
  base: string; run_id: string; status: 'succeeded' | 'failed'; error?: string
  run?: { template_rows: number; template_cols: number; rows: number; cols: number; pages: number; inputs: GalleryInput[]; checks: GalleryCheck[] }
}
export interface GalleryCatalog { version: number; run_id: string; source: string; reports: GalleryReport[] }

export async function artifact(url: string): Promise<Response> {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`读取 ${url} 失败：HTTP ${response.status}`)
  return response
}

/** 固定运行目录中的定义与附件一起交给设计器；连接凭证始终由执行端管理。 */
export async function galleryFiles(base: string): Promise<File[]> {
  if (!/^\/report-gallery\/run-\d+\/[a-zA-Z0-9-]+$/.test(base)) throw new Error('无效的示例运行目录')
  const text = await (await artifact(`${base}/template.json`)).text()
  const definition = JSON.parse(text) as { datasets?: { file?: { file_name?: string } }[] }
  const names = [...new Set((definition.datasets ?? []).flatMap(d => d.file?.file_name ? [d.file.file_name] : []))]
  if (names.some(name => !name || /[/\\]/.test(name) || name === '.' || name === '..')) throw new Error('示例数据文件必须位于运行目录内')
  const files = await Promise.all(names.map(async name => new File([await (await artifact(`${base}/${encodeURIComponent(name)}`)).blob()], name)))
  return [new File([text], 'template.json', { type: 'application/json' }), ...files]
}
