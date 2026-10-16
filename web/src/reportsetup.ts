/**
 * 报表设置面板补丁（M7-W W3-1）：`print_setup` 16 个开放字段的逐键补丁。
 *
 * 覆盖 `report-def-json.md` §3.6（M7-6 落地的服务端契约子集）。PDF 课程
 * （`docs/_ref_ReportLite-course.pdf`）里「报表设置」的产品形态 = 1.4 报表区域
 * 的行属性 + 9.4 页眉页脚/分页码 + 第 2 章外观；本模块承接其中**定义 JSON
 * 有槽**的那部分（页面尺寸/边距/分页/份数/缩放/版面/打印布尔）——行类型与
 * 页眉页脚表达式在定义 JSON 里**没有槽**，登记为契约缺口（spec-m7-web §W3）。
 *
 * ## 三条口径（与 W2 系列面板同规）
 *
 * 1. **读方向是定义 JSON 派生**（W2-3 模式）：`print_setup` 不在 Univer 模型里
 *    （画布只管格子）⇒ 面板显示态 = JSON 的 `print_setup` 键，未声明字段显示
 *    **契约缺省**（不是空）——用户看到的就是「不显式设也会生效的值」。
 * 2. **写方向逐键补丁 + 缺省即删键**：只动用户改的那个键；值等于契约缺省 ⇒
 *    删键（与 `stylepanel` 的「未声明 = 缺省」同语义，不留冗余键）。
 * 3. **生效靠重渲染**：纸张/边距/分页只在服务端出快照时兑现 ⇒ Univer 无落地腿
 *    （与 W2-5 断页同模式），App 顶部「渲染」已改为持有 def 优先（W2-5）。
 *
 * ## 校验镜像服务端 `printsetup::apply_print_setup`（`reportdef/printsetup.rs`）
 *
 * | 档位 | 字段 | 规则 |
 * | --- | --- | --- |
 * | i16 | zoom_scale / table_column_num / layout_rows / layout_cols | 0..=32767 |
 * | i32 | row_num_per_page / copy_count | 0..=2147483647 |
 * | f32 | paper_* / *_margin | ≥0 |
 * | bool | back_graph_printed / column_repeated_group_header / full_screen_preview / blank_line_filled | — |
 *
 * `virtual_printer`（字符串）契约开放但**不做 UI**：虚拟打印机名是打印链路
 * 概念，Web 设计器设它没有意义（用户本机打印机列表服务端不可见）⇒ 不猜。
 */
import type { ReportDef } from './designer'

// ---------------------------------------------------------------------------
// 契约常量与类型
// ---------------------------------------------------------------------------

/** 契约缺省（`printsetup.rs` 的 `PrintSetup::default_values()` + §3.6 表）。 */
export const PRINT_SETUP_DEFAULTS = {
  paper_width: 210,
  paper_height: 297,
  left_margin: 19,
  right_margin: 19,
  top_margin: 25,
  bottom_margin: 25,
  row_num_per_page: 20,
  copy_count: 1,
  zoom_scale: 100,
  table_column_num: 1,
  layout_rows: 1,
  layout_cols: 1,
  back_graph_printed: false,
  column_repeated_group_header: false,
  full_screen_preview: false,
  blank_line_filled: false,
} as const

/** i16 档字段（服务端 `I16_FIELDS`；0..=32767）。 */
const I16_MAX = 32767
const I16_FIELDS = new Set(['zoom_scale', 'table_column_num', 'layout_rows', 'layout_cols'])
/** i32 档字段（服务端 `I32_FIELDS`；0..=i32::MAX）。 */
const I32_MAX = 2147483647
const I32_FIELDS = new Set(['row_num_per_page', 'copy_count'])
/** f32 档字段（服务端 `F32_FIELDS`；≥0）。 */
const F32_FIELDS = new Set([
  'paper_width',
  'paper_height',
  'left_margin',
  'right_margin',
  'top_margin',
  'bottom_margin',
])
/** 布尔档字段（服务端 `BOOL_FIELDS`）。 */
const BOOL_FIELDS = new Set([
  'back_graph_printed',
  'column_repeated_group_header',
  'full_screen_preview',
  'blank_line_filled',
])

/** 面板可设字段（16 个开放字段去掉 `virtual_printer`，理由见文件头）。 */
export type PrintSetupField = keyof typeof PRINT_SETUP_DEFAULTS

/** 面板补丁（只带要改的键；值 = 契约缺省 ⇒ 删键语义由 `applyPrintSetup` 执行）。 */
export type PrintSetupPatch = Partial<Record<PrintSetupField, number | boolean>>

/** 复用 W2 系列的结果形状。 */
export type SetupResult = { def?: ReportDef; error?: string; warnings: string[] }

// ---------------------------------------------------------------------------
// 读方向（面板显示态：16 字段全具体值，未声明 ⇒ 契约缺省）
// ---------------------------------------------------------------------------

/** 当前 `print_setup` 的显示值（缺失/非对象 ⇒ 全缺省；类型不符的字段回落缺省）。 */
export function printSetupOf(def: ReportDef): Record<PrintSetupField, number | boolean> {
  const raw = (def.print_setup ?? {}) as Record<string, unknown>
  const out = {} as Record<PrintSetupField, number | boolean>
  for (const [k, dft] of Object.entries(PRINT_SETUP_DEFAULTS) as [PrintSetupField, number | boolean][]) {
    const v = raw[k]
    if (typeof dft === 'boolean') {
      out[k] = typeof v === 'boolean' ? v : dft
    } else {
      out[k] = typeof v === 'number' && Number.isFinite(v) ? v : dft
    }
  }
  return out
}

// ---------------------------------------------------------------------------
// 写方向：逐键补丁（校验镜像服务端）
// ---------------------------------------------------------------------------

/** 单字段校验（镜像 `apply_print_setup` 的三档规则；不猜、不夹取）。 */
function validateField(field: PrintSetupField, value: number | boolean): string | null {
  if (BOOL_FIELDS.has(field)) {
    if (typeof value !== 'boolean') return `${field} 必须是布尔`
    return null
  }
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return `${field} 必须是有限数字`
  }
  if (I16_FIELDS.has(field)) {
    if (!Number.isInteger(value) || value < 0 || value > I16_MAX) {
      return `${field} 须为 0..=${I16_MAX} 的整数（服务端 i16 档），实得 ${value}`
    }
  }
  if (I32_FIELDS.has(field)) {
    if (!Number.isInteger(value) || value < 0 || value > I32_MAX) {
      return `${field} 须为 0..=${I32_MAX} 的整数（服务端 i32 档），实得 ${value}`
    }
  }
  if (F32_FIELDS.has(field)) {
    if (value < 0) return `${field} 不得为负（服务端 f32 档），实得 ${value}`
  }
  return null
}

/**
 * 把面板补丁落进定义 JSON 的 `print_setup`（返回**新**对象，纯函数）。
 *
 * 口径：
 * - **只动 patch 里的键**，`print_setup` 其余键（含用户手写的未知键——服务端
 *   会拦，这里不拦也不删）原样保留；
 * - 值 = 契约缺省 ⇒ **删键**（`undefined` 值在 `JSON.stringify` 时自然消失，
 *   与 `rowcol.ts` 的删键同款；`isEmpty` 判断按值不数键）；
 * - 校验失败 ⇒ 返回 error、不产出 def（不猜、不半成品）。
 */
export function applyPrintSetup(def: ReportDef, patch: PrintSetupPatch): SetupResult {
  const warnings: string[] = []
  const entries = Object.entries(patch) as [PrintSetupField, number | boolean][]
  for (const [field, value] of entries) {
    const bad = validateField(field, value)
    if (bad) return { error: bad, warnings }
  }
  const cur = ((def.print_setup ?? {}) as Record<string, unknown>)
  const next: Record<string, unknown> = { ...cur }
  for (const [field, value] of entries) {
    const dft = PRINT_SETUP_DEFAULTS[field]
    if (value === dft) {
      delete next[field] // 缺省即删键（不写冗余）
    } else {
      next[field] = value
    }
  }
  // 空对象 ⇒ 删整键（`print_setup: {}` 语义 = 全缺省，与服务端同口径）
  const hasKeys = Object.values(next).some((v) => v !== undefined)
  const out: ReportDef = { ...def }
  if (hasKeys) {
    out.print_setup = next
  } else {
    delete out.print_setup
  }
  warnings.push('报表设置已写入 JSON；纸张/边距/分页在服务端出快照时兑现——点「渲染」生效')
  return { def: out, warnings }
}

/** 横竖互换：`paper_width` ↔ `paper_height`（PDF 未讲方向字段；契约的口径是
 * 「横向版 = 与 paper_height 互换」，§3.6 表备注）。 */
export function swapPaperOrientation(def: ReportDef): SetupResult {
  const cur = printSetupOf(def)
  return applyPrintSetup(def, { paper_width: cur.paper_height, paper_height: cur.paper_width })
}
