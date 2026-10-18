/**
 * 显示层列宽自适应：数字放不下时把列拉宽，避免画布把它画成 `####`。
 *
 * # 为什么在显示层，而不是改快照
 *
 * `workbook.json` 里的 `columnData.w` 是**打印版面**的一部分（mm→px）：改它会改变
 * 横向分页与 PDF 页数，那是 `run` 的契约。Excel 出口已经在
 * `rpx-export::widths` 里单独加宽过；网页画布是只读展示，按「看得清」加宽即可，
 * 两边互不影响，也不必重跑任何产物。
 *
 * # 口径（与 Excel 出口一致）
 *
 * 1. **只对数值格加宽**：文本放不下会溢出到邻格 / 被裁，不会画 `####`；
 * 2. **合并区按整段宽度算**，缺口平均摊到跨度内的可见列；
 * 3. **隐藏列不参与**，也不会被加宽；
 * 4. 只加不减：放得下的列一个像素都不动。
 *
 * 度量口径是浏览器画布（`measureText`），比 Rust 侧按字宽表估算更准；
 * 默认字体取快照里的 `s.ff` / `s.fs`（快照总是显式写出这两个字段）。
 */
import { numfmt } from '@univerjs/core'
import type { PrintWorkbook } from './PrintPreview'

/** 单元格左右内边距 + 网格线（Univer 实测约 6px），再留 2px 余量。 */
const PADDING_PX = 8
/** 单列上限：再长就换行/缩排，不把画布拉成一条线。 */
const MAX_COL_PX = 600
/** 快照缺字体/字号时的兜底（与 `def::DEFAULT_FONT_NAME` / `FONT_SIZE` 同口径）。 */
const DEFAULT_FAMILY = '宋体'
const DEFAULT_SIZE = 12

type CellStyle = { ff?: string; fs?: number; bl?: number; it?: number; n?: { pattern?: string } }
/** 快照单元格：`t` 是 Univer 的类型标记（1 = 强制字符串/显示覆盖），2 = 数字。 */
type Cell = { v?: unknown; t?: number; s?: CellStyle }
type Sheet = PrintWorkbook['sheets'][string]
type Column = { w: number; hd?: number }

/** 度量函数：给测试注入固定字宽，避免依赖 DOM 画布。 */
export type Measure = (text: string, font: { family: string; size: number; bold: boolean; italic: boolean }) => number

/** 数值格的**显示文本**；非数值返回 `null`（文本格不参与加宽）。 */
export function displayText(cell: Cell): string | null {
  const value = cell.v
  if (typeof value !== 'number' || !Number.isFinite(value)) return null
  const pattern = cell.s?.n?.pattern
  if (!pattern) return numfmt.format('General', value)
  try {
    return numfmt.format(pattern, value)
  } catch {
    return String(value)
  }
}

let context: CanvasRenderingContext2D | null | undefined

/** 无 DOM（node 测试、SSR）时的退路：CJK 按 1em、其余按 0.55em 估。 */
function roughWidth(text: string, size: number): number {
  const em = (size * 96) / 72
  let width = 0
  for (const ch of text) width += /[\u2e80-\u9fff\uff00-\uffef]/.test(ch) ? em : em * 0.55
  return width
}

/** 默认度量：浏览器画布。`pt` 字号在 canvas 里按 CSS 换算，与屏幕一致。 */
export function canvasMeasure(text: string, font: { family: string; size: number; bold: boolean; italic: boolean }): number {
  if (context === undefined) {
    context = typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d')
  }
  if (!context) return roughWidth(text, font.size)
  context.font = `${font.italic ? 'italic ' : ''}${font.bold ? 'bold ' : ''}${font.size}pt "${font.family}"`
  return context.measureText(text).width
}

/** 合并区锚点 → 跨度列数。 */
function spans(sheet: Sheet): Map<string, number> {
  const map = new Map<string, number>()
  for (const merge of sheet.mergeData ?? []) {
    const span = merge.endColumn - merge.startColumn + 1
    if (span > 1) map.set(`${merge.startRow},${merge.startColumn}`, span)
  }
  return map
}

/** 一页（sheet）的列宽副本：只加宽，不改其它字段。 */
function fitSheet(sheet: Sheet, measure: Measure): Sheet {
  const columns: Record<string, Column> = {}
  for (const [index, column] of Object.entries(sheet.columnData ?? {})) columns[index] = { ...column }

  const widthOf = (index: number) => columns[String(index)]?.w ?? sheet.defaultColumnWidth ?? 88
  const hiddenAt = (index: number) => columns[String(index)]?.hd === 1
  const widen = (index: number, width: number) => {
    columns[String(index)] = { ...columns[String(index)], w: Math.min(width, MAX_COL_PX) }
  }

  const merged = spans(sheet)
  for (const [row, cells] of Object.entries(sheet.cellData ?? {})) {
    for (const [col, cell] of Object.entries(cells)) {
      const text = displayText(cell)
      if (text === null) continue
      const start = Number(col)
      if (!Number.isFinite(start)) continue
      const span = merged.get(`${row},${col}`) ?? 1
      const visible: number[] = []
      for (let index = start; index < start + span; index++) if (!hiddenAt(index)) visible.push(index)
      if (visible.length === 0) continue
      const have = visible.reduce((sum, index) => sum + widthOf(index), 0)
      const style = cell.s ?? {}
      const need = measure(text, {
        family: style.ff ?? DEFAULT_FAMILY,
        size: style.fs ?? DEFAULT_SIZE,
        bold: !!style.bl,
        italic: !!style.it,
      }) + PADDING_PX
      if (need <= have) continue
      // 缺口平均摊：跨度内的列一起变宽，相对关系不变。
      const share = (need - have) / visible.length
      for (const index of visible) widen(index, widthOf(index) + share)
    }
  }
  return { ...sheet, columnData: columns }
}

/** 返回一份**列宽已适配**的工作簿副本（其余字段原样引用）。 */
export function fitColumns(workbook: PrintWorkbook, measure: Measure = canvasMeasure): PrintWorkbook {
  const sheets: Record<string, Sheet> = {}
  for (const id of workbook.sheetOrder) {
    const sheet = workbook.sheets[id]
    sheets[id] = sheet ? fitSheet(sheet, measure) : sheet
  }
  return { ...workbook, sheets }
}
