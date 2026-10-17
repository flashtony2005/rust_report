/**
 * 数据集绑定补丁（M7-W W2-5b）。
 *
 * 覆盖 `report-def-json.md` §3.3（`datasets[]`：每条给 **`sql` 或 `file` 之一**）。
 *
 * ## Univer 不知道数据集 ⇒ 纯 JSON 编辑（无画布落地腿）
 *
 * 数据集不在 Univer 的模型里（画布只有格子），本模块是纯粹的定义 JSON 补丁；
 * 改动的生效路径 = 「导出 JSON → 重渲染（数据集文件随 multipart 上传）」。
 * `file.file_name` 必须与 multipart `dataset` 字段的文件名**逐字一致**
 * （`spec-m3.md` §3.2 主链路 ①），面板只存声明、不替用户改名。
 *
 * ## 校验全部镜像服务端 `parse_dataset`（`reportdef/parse.rs:277`）
 *
 * | # | 约束 | 服务端错误消息（同款口径） |
 * | --- | --- | --- |
 * | 1 | `sql` 与 `file` 都给 ⇒ 报错 | "`sql` 与 `file` 互斥" |
 * | 2 | 都不给 ⇒ 报错 | "必须给 `sql` 或 `file` 之一" |
 * | 3 | `sql` 分支：`dsn` 与 `sql` 必填 | `field()` 缺失报错 |
 * | 4 | `file` 分支：`file_name` 必填；`encoding` 可选（缺省 GBK） | 同上 |
 *
 * 数据集名称必须唯一；执行端也会拒绝重名，避免表达式绑定歧义。
 */
import type { ReportDef } from './designer'

// ---------------------------------------------------------------------------
// 类型
// ---------------------------------------------------------------------------

/** 面板口径的数据集条目（契约 §3.3 的展示形态）。 */
export type DatasetInput = {
  name?: string
  sql?: { dsn: string; sql: string }
  file?: { file_name: string; encoding?: string; sheet_name?: string; delimiter?: string }
}

/** 服务端 JSON 里的原始条目形态（未知键原样保留）。 */
export type DatasetEntry = {
  name?: string
  sql?: { dsn: string; sql: string; [k: string]: unknown }
  file?: { file_name: string; encoding?: string; sheet_name?: string; delimiter?: string; [k: string]: unknown }
  [k: string]: unknown
}

/** 复用 W2 系列的结果形状：`error` 与 `def` 互斥。 */
export type DatasetResult = { def?: ReportDef; error?: string; warnings: string[] }

// ---------------------------------------------------------------------------
// 读方向（面板显示）
// ---------------------------------------------------------------------------

/** 数据集清单（面板显示态；直接从 JSON 读，未声明 ⇒ 空）。 */
export function datasetsOf(def: ReportDef): DatasetEntry[] {
  return (def.datasets as DatasetEntry[] | undefined) ?? []
}

/** 条目的展示摘要（面板列表用；没有可读内容时给占位）。 */
export function datasetSummary(d: DatasetEntry): string {
  if (d.sql) return `SQL · ${d.sql.dsn || '（无 dsn）'} · ${(d.sql.sql || '').slice(0, 24)}`
  if (d.file) return `文件 · ${d.file.file_name || '（无文件名）'}${d.file.encoding ? ` · ${d.file.encoding}` : ''}`
  return '（未声明 sql/file）'
}

// ---------------------------------------------------------------------------
// 内部：单条校验（镜像 parse_dataset）
// ---------------------------------------------------------------------------

/** 校验一条输入；不合法返回人话错误（不猜、不补全）。 */
function validateInput(d: DatasetInput): string | null {
  if (d.sql && d.file) return '一个数据集只能给 `sql` 或 `file` 之一（两者互斥）'
  if (!d.sql && !d.file) return '一个数据集必须给 `sql` 或 `file` 之一'
  if (d.sql) {
    if (!d.sql.dsn || !d.sql.dsn.trim()) return 'SQL 数据集必须给 dsn（数据源名，连接由服务端 --datasources 注入）'
    if (!d.sql.sql || !d.sql.sql.trim()) return 'SQL 数据集必须给 sql 语句'
  }
  if (d.file) {
    if (!d.file.file_name || !d.file.file_name.trim()) {
      return '文件数据集必须给 file_name（须与运行时上传的数据文件名逐字一致）'
    }
    if (d.file.delimiter !== undefined && (!/^[\x00-\x7f]$/.test(d.file.delimiter) || /[\r\n"]/.test(d.file.delimiter))) return '分隔符必须是一个 ASCII 字符，且不能为引号或换行'
  }
  return null
}

/** 与执行端使用同一个有效名称；未显式命名的文件使用文件名。 */
function duplicateNameError(def: ReportDef, input: DatasetInput, index: number): string | null {
  const nameOf = (d: DatasetInput) => d.name ?? d.file?.file_name ?? '<unnamed-sql>'
  const name = nameOf(input)
  return datasetsOf(def).some((d, i) => i !== index && nameOf(d) === name)
    ? `数据集名称「${name}」重复，请修改名称`
    : null
}

/** 输入 → 服务端 JSON 条目（只写面板产出的键，不造空壳）。 */
function toEntry(d: DatasetInput): DatasetEntry {
  const entry: DatasetEntry = {}
  if (d.name !== undefined && d.name !== '') entry.name = d.name
  if (d.sql) entry.sql = { dsn: d.sql.dsn, sql: d.sql.sql }
  if (d.file) {
    entry.file = { ...d.file }
  }
  return entry
}

// ---------------------------------------------------------------------------
// 写方向：增 / 删 / 改（按下标定位 —— 面板列表的行序即声明序）
// ---------------------------------------------------------------------------

/** 追加一条数据集。校验镜像 `parse_dataset`；名称必须唯一。 */
export function applyDatasetAdd(def: ReportDef, input: DatasetInput): DatasetResult {
  const warnings: string[] = []
  const bad = validateInput(input) ?? duplicateNameError(def, input, -1)
  if (bad) return { error: bad, warnings }
  const next: ReportDef = { ...def }
  const list = [...datasetsOf(def), toEntry(input)]
  next.datasets = list
  warnings.push('数据集已写入 JSON；文件来源须提供对应 Excel / CSV / TSV，数据库来源按连接名读取')
  return { def: next, warnings }
}

/** 删除第 `index` 条（越界显式报错——静默不删会让用户以为删掉了）。 */
export function applyDatasetRemove(def: ReportDef, index: number): DatasetResult {
  const warnings: string[] = []
  const list = datasetsOf(def)
  if (!Number.isInteger(index) || index < 0 || index >= list.length) {
    return { error: `要删除的数据集下标 ${index} 不存在（当前共 ${list.length} 条）`, warnings }
  }
  const next: ReportDef = { ...def }
  next.datasets = list.filter((_, i) => i !== index)
  return { def: next, warnings }
}

/** 替换第 `index` 条（整条替换语义：面板编辑的是完整一条，不是逐键）。
 *
 * 与样式/内容的「逐键补丁」不同：数据集面板的每行就是一个完整条目，
 * 用户点「保存」时给的是整条新值 —— 旧键整体作废（不然删不掉 dsn）。 */
export function applyDatasetUpdate(def: ReportDef, index: number, input: DatasetInput): DatasetResult {
  const warnings: string[] = []
  const list = datasetsOf(def)
  if (!Number.isInteger(index) || index < 0 || index >= list.length) {
    return { error: `要修改的数据集下标 ${index} 不存在（当前共 ${list.length} 条）`, warnings }
  }
  const bad = validateInput(input) ?? duplicateNameError(def, input, index)
  if (bad) return { error: bad, warnings }
  const next: ReportDef = { ...def }
  const newList = [...list]
  newList[index] = toEntry(input)
  next.datasets = newList
  warnings.push('数据集已更新；文件来源须提供对应 Excel / CSV / TSV，数据库来源按连接名读取')
  return { def: next, warnings }
}
