/**
 * 快照装载 + 定义 JSON 通道（M4 + M7-W W1）。
 *
 * 契约：`docs/spec-m4.md` §1/§2 —— 前端**零业务逻辑**，只负责装载。
 * 数据管道 v2（M4-06，2026-10-14）：`POST /api/render`（multipart）——
 * `report` = `.rpx` 模板**或定义 JSON**（M7-1，服务端按内容分派），
 * `dataset` = 模板引用的 CSV（可多个，文件名须与模板 `FileDataSetConfig.file_name`
 * 一致），`params` 可选（参数 JSON）。
 * 请求/响应契约见 `docs/spec-m3.md` §3.2 与 `docs/spec-m4.md` §4；
 * vite dev 把 `/api` 代理到 rpx-server（127.0.0.1:8090，`vite.config.ts`）。
 *
 * W1（M7-W）接上的三条服务端既有能力：
 * * W1-1 `report` 字段收 `.json`（[`pickReportFile]`）；
 * * W1-2 参数表单改读 `/meta`（[`fetchMeta]` + [`metaToRows]`）；
 * * W1-3 「另存为」入库拿 id（[`saveReport]`）。
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
 * 其余 → 字符串（不猜类型；数组/对象 v1 不支持，表单只出标量行）。
 *
 * W1-2：`dynamic` 由 `/meta` 的 `is_static` 派生（`is_static = param_type == 0`，
 * 见 `rpx-ds/src/asset.rs`），**只为表单标注用**，不进 `paramsToObject` 的产物
 * （见 `src/report.test.ts::dynamic 标记不会泄漏进 params`）。手工新增的行
 * 不设它（undefined = 未标注）。 */
export type ParamRow = { name: string; value: string; dynamic?: boolean }

/** `GET /reports/{id}/meta` 的单条返回（M7-4：六个字段全给，缺省为 `null`）。 */
export type ParamMetaItem = {
  name: string
  param_type: number
  is_static: boolean
  data_type: number
  default_val: string | null
  description: string | null
}

/** 报表源后缀，**按优先级排列**：`.rpx`（原生）优先，`.json`（定义 JSON）次之。
 *
 * 服务端 `reportdef::open_report` 按**内容**分派（`RQQR` 魔数 → `.rpx`；
 * 首个非空白字节 `{` → 定义 JSON），不看扩展名 —— 故前端这里只做「哪个文件
 * 是报表源」的选择，不做格式判定。 */
const REPORT_SUFFIXES = ['.rpx', '.json'] as const

/** 文件名能否当报表源（大小写不敏感）。
 *
 * ⚠ `a.rpx.csv` **不是**报表源：判据是「以该后缀结尾」而不是「包含」，
 * 与 M4 的 `endsWith('.rpx')` 同口径（`src/report.test.ts` 钉住）。 */
export function isReportSource(name: string): boolean {
  const lower = name.toLowerCase()
  return REPORT_SUFFIXES.some((s) => lower.endsWith(s))
}

/** 从选中文件里挑出报表源（大小写不敏感；挑不到返回 undefined）。
 *
 * 口径（**与 M4 逐字兼容**，勿轻改）：`.rpx` 优先、其次 `.json`；同类里取**第一个**
 * （与改前的 `Array.find` 同口径）。`.rpx` 优先是为了让「只选 `.rpx`」的既有
 * 行为一个字节都不变 —— W1-1 只是**加宽**入口，不是改优先级。
 *
 * 纯函数，与 [`fetchSnapshot`] 分开放是为了可单测（W0-2：本仓要求前端改动
 * 也有机器判据，`docs/spec-m7-web.md` §4 W0）。 */
export function pickReportFile(files: File[]): File | undefined {
  for (const suffix of REPORT_SUFFIXES) {
    const hit = files.find((f) => f.name.toLowerCase().endsWith(suffix))
    if (hit) return hit
  }
  return undefined
}

/** `/meta` 清单 → 参数表单行（W1-2）。
 *
 * 口径：
 * - 顺序 = 清单顺序（服务端按 `rowid` 排序，即声明序）；
 * - `value` = `default_val ?? ''` —— 静态参数的默认值**预填**进表单，
 *   动态/属性参数没有默认值（服务端落 `null`）⇒ 留空等人填；
 * - `dynamic` = `!is_static` —— `param_type != 0`（动态 1 / 属性 2）即标注。
 *
 * ⚠ 这是**替换**语义：拿到清单就把表单整体换成服务端声明的那一份
 * （W1-2 判据「不再手工填名」）。手工行在「另存为」成功后即被声明覆盖。 */
export function metaToRows(items: ParamMetaItem[]): ParamRow[] {
  return items.map((p) => ({
    name: p.name,
    value: p.default_val ?? '',
    dynamic: !p.is_static,
  }))
}

/** 参数行 → `params` 表单段的对象。
 *
 * 口径（**与改前逐字节等价**，勿轻改）：
 * - 空名（trim 后）**跳过**——服务端按未登记参数显式报错；
 * - 整数字面量（`/^-?\d+$/`，trim 后判）→ `Number`；
 * - 小数（`/^-?(\d+\.\d*|\.\d+)([eE][+-]?\d+)?$/`，trim 后判）→ `Number`
 *   —— ⚠ **该正则要求有小数点**，故**无小数点的科学计数（`1e3`）不是数字**，
 *   走字符串分支。`src/report.test.ts` 钉住了这个口径；
 * - 其余 → **原样 `value`（未 trim）**。
 *
 * ⚠ 最后一条是既有行为、也是刻意保留的：数字必须 trim 才解析得了，
 * 而字符串保留用户原样输入（含首尾空格）是「用户就是想传这个串」。
 * 这个不对称在 M7-W 之前没有判据钉住，现在有了 —— 见
 * `src/report.test.ts::string_values_are_not_trimmed`。
 * 若将来要改成统一 trim，**先改测试再改实现**（本仓纪律：判据先行）。
 */
export function paramsToObject(rows: ParamRow[]): Record<string, string | number> {
  const obj: Record<string, string | number> = {}
  for (const { name, value } of rows) {
    const n = name.trim()
    if (!n) continue
    const v = value.trim()
    obj[n] = /^-?\d+$/.test(v)
      ? Number(v)
      : /^-?(\d+\.\d*|\.\d+)([eE][+-]?\d+)?$/.test(v)
        ? Number(v)
        : value
  }
  return obj
}

/** 取失败响应里的服务端错误消息（`{"error": "..."}`）。
 *
 * 服务端 `src/http.rs::error_json` 的错误体一律这个形状，前端**原样上屏**
 * （`web/src/App.tsx`）。取不到就退化成状态码 —— 不猜、不吞。 */
async function errorMessage(resp: Response): Promise<string> {
  const detail = await resp.json().catch(() => null)
  if (detail && typeof detail.error === 'string') return detail.error
  return `HTTP ${resp.status}`
}

/** 取服务端快照 JSON（`IWorkbookData`，契约见 `docs/spec-m4.md` §4）。
 *
 * W1-1：`report` 字段现在同时收 **`.rpx` 与定义 `.json`**（服务端按内容分派，
 * 见 [`pickReportFile]）。选中 `.json` + 同目录 CSV 时，CSV 仍按 `dataset`
 * 上传 —— 与 `.rpx` 模板引用 CSV 的口径完全一致。 */
export async function fetchSnapshot(
  files: File[],
  params: ParamRow[],
): Promise<unknown> {
  const report = pickReportFile(files)
  if (!report) {
    throw new Error('未选择报表源文件（.rpx 或 .json；可同时选中模板引用的 CSV，一起上传）')
  }
  const form = new FormData()
  form.append('report', report, report.name)
  // 其余文件一律按数据集 CSV 上传；服务端按文件名与模板引用对齐
  // （少给一个文件会显式报「缺少数据集文件：<文件名>」）。
  for (const f of files) {
    if (f !== report) form.append('dataset', f, f.name)
  }
  // 参数（M4-04）：非空行才提交；空名跳过（服务端按未登记参数显式报错）。
  const obj = paramsToObject(params)
  if (Object.keys(obj).length > 0) {
    form.append('params', JSON.stringify(obj))
  }
  const resp = await fetch('/api/render', { method: 'POST', body: form })
  if (!resp.ok) {
    throw new Error(`渲染失败：${await errorMessage(resp)}`)
  }
  return resp.json()
}

/** `POST /reports`（W1-3「另存为」）：报表源入库，拿到服务端 id。
 *
 * ⚠ 字段名是 **`file`**，不是 `/render` 的 `report` —— 服务端
 * `src/http.rs::read_upload` 按 `file` 取（两者不同名，别照抄）。
 * 报表名 = 文件名去扩展名（服务端口径）。
 *
 * 返回 `{ id, name }`；形状不符时**显式抛错**（不猜、不静默拿 undefined 当 id）。 */
export async function saveReport(file: File): Promise<{ id: number; name: string }> {
  const form = new FormData()
  form.append('file', file, file.name)
  const resp = await fetch('/api/reports', { method: 'POST', body: form })
  if (!resp.ok) {
    throw new Error(`入库失败：${await errorMessage(resp)}`)
  }
  const j: unknown = await resp.json()
  if (
    !j ||
    typeof j !== 'object' ||
    typeof (j as { id?: unknown }).id !== 'number' ||
    typeof (j as { name?: unknown }).name !== 'string'
  ) {
    throw new Error(`入库响应缺 id/name：${JSON.stringify(j)}`)
  }
  const { id, name } = j as { id: number; name: string }
  return { id, name }
}

/** `GET /reports/{id}/meta`（W1-2）：读回参数清单。
 *
 * 形状不符（非数组 / 条目缺 `name` 或 `is_static`）**显式抛错** —— 清单是
 * 「不猜」边界：错把它当空清单会让参数表单静默清空，用户看不出哪错了。 */
export async function fetchMeta(id: number): Promise<ParamMetaItem[]> {
  const resp = await fetch(`/api/reports/${id}/meta`)
  if (!resp.ok) {
    throw new Error(`读参数清单失败：${await errorMessage(resp)}`)
  }
  const j: unknown = await resp.json()
  if (!Array.isArray(j)) {
    throw new Error(`参数清单应为数组，实得：${JSON.stringify(j)}`)
  }
  for (const item of j) {
    const p = item as Partial<ParamMetaItem>
    if (typeof p?.name !== 'string' || typeof p?.is_static !== 'boolean') {
      throw new Error(`参数清单条目缺 name/is_static：${JSON.stringify(item)}`)
    }
    if (p.default_val !== null && p.default_val !== undefined && typeof p.default_val !== 'string') {
      throw new Error(`参数 default_val 应为 string|null：${JSON.stringify(item)}`)
    }
  }
  return j as ParamMetaItem[]
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
