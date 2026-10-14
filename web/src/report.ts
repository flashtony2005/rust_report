/**
 * 快照装载（M4）：文件 → Univer 工作簿。
 *
 * 契约：`docs/spec-m4.md` §1/§2 —— 前端**零业务逻辑**，只负责装载。
 * 数据管道 v2（M4-06，2026-10-14）：`POST /api/render`（multipart）——
 * `report` = .rpx 模板，`dataset` = 模板引用的 CSV（可多个，文件名须与
 * 模板 `FileDataSetConfig.file_name` 一致），`params` 可选（参数 JSON）。
 * 请求/响应契约见 `docs/spec-m3.md` §3.2 与 `docs/spec-m4.md` §4；
 * vite dev 把 `/api` 代理到 rpx-server（127.0.0.1:8090，`vite.config.ts`）。
 *
 * ⚠ import 路径以本机安装的 Univer 1.0.4 实际结构为准（2026-10-14 校准，见
 * `web/node_modules/@univerjs/preset-sheets-core`）。
 */
import { LocaleType, createUniver, mergeLocales, type FUniver } from '@univerjs/presets'
import { UniverSheetsCorePreset } from '@univerjs/preset-sheets-core'
import UniverPresetSheetsCoreZhCN from '@univerjs/preset-sheets-core/locales/zh-CN'
import '@univerjs/preset-sheets-core/lib/index.css'

/** 前端参数行（M4-04）：`render` 的 `params` 表单段口径 —— 值按
 * `form.rs::json_to_value` 同规推断：整数字面量 → Int、小数 → Double、
 * 其余 → 字符串（不猜类型；数组/对象 v1 不支持，表单只出标量行）。 */
export type ParamRow = { name: string; value: string }

/** 取服务端快照 JSON（`IWorkbookData`，契约见 `docs/spec-m4.md` §4）。 */
export async function fetchSnapshot(
  files: File[],
  params: ParamRow[],
): Promise<unknown> {
  const report = files.find((f) => f.name.toLowerCase().endsWith('.rpx'))
  if (!report) {
    throw new Error('未选择 .rpx 模板文件（可同时选中模板引用的 CSV，一起上传）')
  }
  const form = new FormData()
  form.append('report', report, report.name)
  // 其余文件一律按数据集 CSV 上传；服务端按文件名与模板引用对齐
  // （少给一个文件会显式报「缺少数据集文件：<文件名>」）。
  for (const f of files) {
    if (f !== report) form.append('dataset', f, f.name)
  }
  // 参数（M4-04）：非空行才提交；空名跳过（服务端按未登记参数显式报错）。
  const obj: Record<string, string | number> = {}
  for (const { name, value } of params) {
    const n = name.trim()
    if (!n) continue
    obj[n] = /^-?\d+$/.test(value.trim())
      ? Number(value.trim())
      : /^-?(\d+\.\d*|\.\d+)([eE][+-]?\d+)?$/.test(value.trim())
        ? Number(value.trim())
        : value
  }
  if (Object.keys(obj).length > 0) {
    form.append('params', JSON.stringify(obj))
  }
  const resp = await fetch('/api/render', { method: 'POST', body: form })
  if (!resp.ok) {
    // 错误体是 `{"error": "..."}`（rpx-server `src/http.rs` 的 error_json）——原样上屏。
    const detail = await resp.json().catch(() => null)
    const msg =
      detail && typeof detail.error === 'string' ? detail.error : `HTTP ${resp.status}`
    throw new Error(`渲染失败：${msg}`)
  }
  return resp.json()
}

/** 在容器里挂 Univer，返回门面 API。 */
export function mountUniver(container: HTMLElement): FUniver {
  const { univerAPI } = createUniver({
    locale: LocaleType.ZH_CN,
    locales: { [LocaleType.ZH_CN]: mergeLocales(UniverPresetSheetsCoreZhCN) },
    presets: [UniverSheetsCorePreset({ container })],
  })
  return univerAPI
}
