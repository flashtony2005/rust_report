/** JSON 源文件选择、下载与 Univer 挂载。执行与取数由 agent CLI 完成。 */
import { LocaleType, createUniver, mergeLocales, type FUniver } from '@univerjs/presets'
import { UniverSheetsCorePreset } from '@univerjs/preset-sheets-core'
import UniverPresetSheetsCoreZhCN from '@univerjs/preset-sheets-core/locales/zh-CN'
import '@univerjs/preset-sheets-core/lib/index.css'

/** JSON 是唯一可编辑的报表源，RPX 通过 CLI 显式导入。 */
export function isReportSource(name: string): boolean { return name.toLowerCase().endsWith('.json') }
export function pickReportFile(files: File[]): File | undefined { return files.find(f => isReportSource(f.name)) }

/** 浏览器下载 Blob（菜单导出的出口；与 `downloadDefJson` 同款 revoke 纪律）。 */
export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

/** 在容器里挂 Univer，返回门面 API。 */
export function mountUniver(container: HTMLElement, template = false): FUniver {
  const { univerAPI } = createUniver({
    locale: LocaleType.ZH_CN,
    locales: { [LocaleType.ZH_CN]: mergeLocales(UniverPresetSheetsCoreZhCN) },
    presets: [UniverSheetsCorePreset({ container, ...(template ? { toolbar: false, contextMenu: false, footer: false } : {}) })],
  })
  return univerAPI
}
