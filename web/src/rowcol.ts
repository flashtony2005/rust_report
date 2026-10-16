/**
 * 行列属性 + 网格尺寸补丁（M7-W W2-5a）。
 *
 * 覆盖 `report-def-json.md` §3.4（`row_props[]` / `col_props[]`）与 §3.1（`grid`）。
 *
 * ## 单位与换算（本模块最重要的契约事实）
 *
 * 定义 JSON 的 `height` / `width` 单位是 **mm**（与 `.rpx` 模板一致：缺省行高
 * `def::ROW_HEIGHT`=8mm、列宽 `def::COL_WIDTH`=25mm，`crates/rpx-core/src/props/def.rs`）；
 * 服务端快照按 **96dpi 线性换算** 成 px（`snapshot/geom.rs::px`：
 * `mm * 96 / 25.4`，保留 2 位 ⇒ 8mm→30.24px、25mm→94.49px）。
 * Univer 的 `setRowHeight`/`setColumnWidth` 吃 **px**。
 * ⇒ 面板输入/显示用 **mm**（= JSON 字面量，无换算损耗），Univer 落地时经
 * [`mmToPx] 换算（同一公式，只做画布即时反馈）。
 *
 * ## 读方向是定义 JSON（W2-3 模式），不是 Univer 活态
 *
 * 「这一行声明过什么」只有 JSON 知道：Univer 活态是 px 且混着快照换算值，
 * px→mm 反算会漂出噪声（30.24 → 7.9999…）。⇒ 显示态从 `row_props`/`col_props`
 * 派生；未声明的行显示缺省 8/25（`DEFAULT_ROW_MM` / `DEFAULT_COL_MM`）。
 *
 * ## 写方向仍是「逐键补丁」
 *
 * 改行高只动该行的 `height` 键；恢复缺省 = **删键**（条目不再有任何属性时
 * **删整条**——只留 `row` 的空壳是冗余）；`break_page` 同规。其余键
 * （含字符串形态的 height——契约允许「数字→字面量、字符串→当表达式透传」，
 * `report-def-json.md` §3.4）原样保留。
 *
 * ## `grid` 缩小必须先校验越界（约束 4 的网格版）
 *
 * `cells[].row/col` 须落在 `1..=grid.rows|cols`（`reportdef/parse.rs`），
 * 合并矩形整体也须在网格内；`row_props[].row` / `col_props[].col` 同理。
 * 改小 `grid` 会让这些既有声明越界 ⇒ 服务端 400。本模块在改 `grid` 前
 * 逐条检查并**显式报错**（含首个越界位置），不静默截断、不自动删声明
 * （自动删 = 用户以为数据还在）。
 */
import type { ReportDef } from './designer'

// ---------------------------------------------------------------------------
// 契约常量与类型
// ---------------------------------------------------------------------------

/** 缺省行高（mm）：`props/def.rs::ROW_HEIGHT` = 8。 */
export const DEFAULT_ROW_MM = 8
/** 缺省列宽（mm）：`props/def.rs::COL_WIDTH` = 25。 */
export const DEFAULT_COL_MM = 25

/** mm → px（96dpi；与快照 `geom.rs::px` 同公式，不 round——Univer 接受小数）。 */
export function mmToPx(mm: number): number {
  return (mm * 96.0) / 25.4
}

/** `row_props[]` 单条（契约 §3.4；未知键原样保留）。 */
export type RowPropEntry = { row: number; height?: number | string; break_page?: boolean | string }
/** `col_props[]` 单条。 */
export type ColPropEntry = { col: number; width?: number | string }

/** 复用 W2-1 的结果形状：`error` 与 `def` 互斥，不产出半成品。 */
export type RowColResult = { def?: ReportDef; error?: string; warnings: string[] }

// ---------------------------------------------------------------------------
// 读方向（面板显示态）
// ---------------------------------------------------------------------------

/** 某行的 `row_props[]` 条目（没有声明返回 undefined）。 */
export function rowPropOf(def: ReportDef, row: number): RowPropEntry | undefined {
  return (def.row_props as RowPropEntry[] | undefined)?.find((p) => p.row === row)
}

/** 某列的 `col_props[]` 条目。 */
export function colPropOf(def: ReportDef, col: number): ColPropEntry | undefined {
  return (def.col_props as ColPropEntry[] | undefined)?.find((p) => p.col === col)
}

/** 某行的显示用行高（mm）：声明过的**数字**字面量，否则缺省。
 *
 * ⚠ 字符串形态（表达式透传）没有具体数值 ⇒ 返回 undefined，面板显示占位
 * （不猜表达式求值结果）。 */
export function rowHeightMm(def: ReportDef, row: number): number | undefined {
  const h = rowPropOf(def, row)?.height
  return typeof h === 'number' ? h : typeof h === 'string' ? undefined : DEFAULT_ROW_MM
}

/** 某列的显示用列宽（mm），口径同 [`rowHeightMm]。 */
export function colWidthMm(def: ReportDef, col: number): number | undefined {
  const w = colPropOf(def, col)?.width
  return typeof w === 'number' ? w : typeof w === 'string' ? undefined : DEFAULT_COL_MM
}

/** 某行的断页声明三态：`true` / `false` / 未声明（undefined）。
 *
 * 字符串形态（原样透传的表达式）无法归约为布尔 ⇒ 也算未声明（面板不猜）。 */
export function rowBreakPage(def: ReportDef, row: number): boolean | undefined {
  const b = rowPropOf(def, row)?.break_page
  return typeof b === 'boolean' ? b : undefined
}

// ---------------------------------------------------------------------------
// 内部：条目 upsert / 删条目（保持其余键原样）
// ---------------------------------------------------------------------------

/** 取 `row_props` 数组副本（无则新建）。 */
function rowPropsCopy(def: ReportDef): RowPropEntry[] {
  return [...((def.row_props as RowPropEntry[] | undefined) ?? [])]
}

/** 取 `col_props` 数组副本。 */
function colPropsCopy(def: ReportDef): ColPropEntry[] {
  return [...((def.col_props as ColPropEntry[] | undefined) ?? [])]
}

/** 条目是否已无任何属性键（只剩 row/col ⇒ 该删整条）。
 *
 * ⚠ `height: undefined` 这种「删键」写法键仍在（值是 undefined），必须按值判，
 * 不能只数 `Object.keys`。 */
function isEmptyProp(entry: RowPropEntry | ColPropEntry): boolean {
  const rec = entry as Record<string, unknown>
  return Object.keys(rec).every((k) => k === 'row' || k === 'col' || rec[k] === undefined)
}

/** 对 `row_props` 里 (或新追加) 的第 `index` 条打 `patch`，空了删整条。 */
function upsertRowProp(
  props: RowPropEntry[],
  row: number,
  patch: Partial<RowPropEntry> | 'remove-if-exists',
): { props: RowPropEntry[]; changed: boolean } {
  const idx = props.findIndex((p) => p.row === row)
  if (patch === 'remove-if-exists') {
    if (idx < 0) return { props, changed: false }
    return { props: props.filter((_, i) => i !== idx), changed: true }
  }
  if (idx < 0) {
    return { props: [...props, { row, ...patch }], changed: true }
  }
  const next = { ...props[idx], ...patch }
  if (isEmptyProp(next)) {
    return { props: props.filter((_, i) => i !== idx), changed: true }
  }
  const copy = [...props]
  copy[idx] = next
  return { props: copy, changed: true }
}

/** 对 `col_props` 同上。 */
function upsertColProp(
  props: ColPropEntry[],
  col: number,
  patch: Partial<ColPropEntry> | 'remove-if-exists',
): { props: ColPropEntry[]; changed: boolean } {
  const idx = props.findIndex((p) => p.col === col)
  if (patch === 'remove-if-exists') {
    if (idx < 0) return { props, changed: false }
    return { props: props.filter((_, i) => i !== idx), changed: true }
  }
  if (idx < 0) {
    return { props: [...props, { col, ...patch }], changed: true }
  }
  const next = { ...props[idx], ...patch }
  if (isEmptyProp(next)) {
    return { props: props.filter((_, i) => i !== idx), changed: true }
  }
  const copy = [...props]
  copy[idx] = next
  return { props: copy, changed: true }
}

// ---------------------------------------------------------------------------
// 写方向：行高 / 列宽 / 断页（批量入口 —— 选区覆盖多行多列时一次补丁）
// ---------------------------------------------------------------------------

/** 批量设置行高（mm）。`mm = null` = 恢复缺省（删 height 键）。
 *
 * 口径：
 * - 行号必须在 `1..=grid.rows`（约束 4；越界服务端 400，这里前置成人话）；
 * - **只动 `height` 键**：该行已声明的 `break_page` 等其余键原样保留；
 * - 非正数 / 非有限数报错（mm 必须是正数字面量）。 */
export function applyRowHeights(def: ReportDef, rows: number[], mm: number | null): RowColResult {
  const warnings: string[] = []
  for (const row of rows) {
    if (!Number.isInteger(row) || row < 1 || row > def.grid.rows) {
      return { error: `行 ${row} 超出网格（grid 是 ${def.grid.rows} 行，逻辑坐标 1 起）`, warnings }
    }
  }
  if (mm !== null && (!Number.isFinite(mm) || mm <= 0)) {
    return { error: `行高须为正数（mm），实得 ${mm}`, warnings }
  }
  const next: ReportDef = { ...def }
  let props = rowPropsCopy(def)
  for (const row of rows) {
    props = upsertRowProp(props, row, mm === null ? { height: undefined } : { height: mm }).props
  }
  next.row_props = props
  if (mm === null) {
    warnings.push('已恢复缺省行高（删除 height 声明；重渲染后生效）')
  }
  return { def: next, warnings }
}

/** 批量设置列宽（mm），口径同 [`applyRowHeights]。 */
export function applyColWidths(def: ReportDef, cols: number[], mm: number | null): RowColResult {
  const warnings: string[] = []
  for (const col of cols) {
    if (!Number.isInteger(col) || col < 1 || col > def.grid.cols) {
      return { error: `列 ${col} 超出网格（grid 是 ${def.grid.cols} 列，逻辑坐标 1 起）`, warnings }
    }
  }
  if (mm !== null && (!Number.isFinite(mm) || mm <= 0)) {
    return { error: `列宽须为正数（mm），实得 ${mm}`, warnings }
  }
  const next: ReportDef = { ...def }
  let props = colPropsCopy(def)
  for (const col of cols) {
    props = upsertColProp(props, col, mm === null ? { width: undefined } : { width: mm }).props
  }
  next.col_props = props
  if (mm === null) {
    warnings.push('已恢复缺省列宽（删除 width 声明；重渲染后生效）')
  }
  return { def: next, warnings }
}

/**
 * 批量设置断页。契约（§3.4 + `build.rs`）：`true` → 服务端落 `"1==1"`、
 * `false` → `"1==0"`（字符串表达式）；`null` = 删键（不断页）。
 * Univer 没有断页概念 ⇒ 无画布落地腿，**重渲染后生效**。
 */
export function applyRowBreakPages(
  def: ReportDef,
  rows: number[],
  on: boolean | null,
): RowColResult {
  const warnings: string[] = []
  for (const row of rows) {
    if (!Number.isInteger(row) || row < 1 || row > def.grid.rows) {
      return { error: `行 ${row} 超出网格（grid 是 ${def.grid.rows} 行，逻辑坐标 1 起）`, warnings }
    }
  }
  const next: ReportDef = { ...def }
  let props = rowPropsCopy(def)
  for (const row of rows) {
    props = upsertRowProp(
      props,
      row,
      on === null ? { break_page: undefined } : { break_page: on },
    ).props
  }
  next.row_props = props
  if (on !== null) warnings.push('断页声明已写入 JSON（Univer 画布无此概念，重渲染后生效）')
  return { def: next, warnings }
}

// ---------------------------------------------------------------------------
// grid 尺寸调整（缩小前校验既有声明越界）
// ---------------------------------------------------------------------------

/**
 * 调整 `grid.rows` / `grid.cols`。**只改 JSON，不重渲染**——画布尺寸是渲染时
 * 由服务端快照定的，调用方须在成功后用持有 def 重新渲染。
 *
 * 缩小校验（越界即报错，不静默删）：
 * - `cells[]`：锚点越界，或**合并矩形整体**溢出新边界（矩形溢出在
 *   `reportdef/parse.rs` 同样被拒）；
 * - `row_props[].row` / `col_props[].col` 越界。
 */
export function applyGridResize(def: ReportDef, rows: number, cols: number): RowColResult {
  const warnings: string[] = []
  if (!Number.isInteger(rows) || rows < 1) {
    return { error: `网格行数须为 ≥1 的整数，实得 ${rows}`, warnings }
  }
  if (!Number.isInteger(cols) || cols < 1) {
    return { error: `网格列数须为 ≥1 的整数，实得 ${cols}`, warnings }
  }
  const shrink = rows < def.grid.rows || cols < def.grid.cols
  if (shrink) {
    for (const cell of def.cells ?? []) {
      const mr = (cell.merge?.rows ?? 1) as number
      const mc = (cell.merge?.cols ?? 1) as number
      if (cell.row > rows || cell.row + mr - 1 > rows) {
        return {
          error: `行 ${cell.row} 上的单元格（含合并矩形）会超出新网格的 ${rows} 行——先删除或移动它，再缩小网格`,
          warnings,
        }
      }
      if (cell.col > cols || cell.col + mc - 1 > cols) {
        return {
          error: `列 ${cell.col} 上的单元格（含合并矩形）会超出新网格的 ${cols} 列——先删除或移动它，再缩小网格`,
          warnings,
        }
      }
    }
    const rp = (def.row_props as RowPropEntry[] | undefined) ?? []
    for (const p of rp) {
      if (p.row > rows) {
        return {
          error: `row_props 里行 ${p.row} 的声明会超出新网格的 ${rows} 行——先清掉该声明`,
          warnings,
        }
      }
    }
    const cp = (def.col_props as ColPropEntry[] | undefined) ?? []
    for (const p of cp) {
      if (p.col > cols) {
        return {
          error: `col_props 里列 ${p.col} 的声明会超出新网格的 ${cols} 列——先清掉该声明`,
          warnings,
        }
      }
    }
  }
  return {
    def: { ...def, grid: { rows, cols } },
    warnings: ['网格尺寸已写入 JSON；请重渲染以让画布按新尺寸装载'],
  }
}
