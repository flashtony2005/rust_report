import { CellValueType, type ICellData } from '@univerjs/core'
import { coveredSlots, findCellIndex, slotKey, type EditResult, type ReportDef } from './designer'
import { jsonBorderToUniver, jsonToUniverStyle, type CellDef, type ExtendModeName } from './stylemap'
import { mmToPx } from './rowcol'

export type CellDraft = {
  kind: 'text' | 'expr'
  content: string
  extend: ExtendModeName
  left: string
  top: string
  rows: number
  cols: number
}

export function address(row0: number, col0: number): string {
  let letters = ''
  for (let col = col0 + 1; col > 0; col = Math.floor((col - 1) / 26)) {
    letters = String.fromCharCode(65 + (col - 1) % 26) + letters
  }
  return `${letters}${row0 + 1}`
}

function position(id: string): [number, number] | null {
  const match = /^([A-Z]+)([1-9]\d*)$/.exec(id)
  if (!match) return null
  const col = [...match[1]].reduce((n, c) => n * 26 + c.charCodeAt(0) - 64, 0)
  return [Number(match[2]), col]
}

export function cellDraft(def: ReportDef, row0: number, col0: number): CellDraft {
  const cell = def.cells?.find(c => c.row === row0 + 1 && c.col === col0 + 1)
  return {
    kind: typeof cell?.expr === 'string' ? 'expr' : 'text',
    content: cell?.expr ?? cell?.text ?? (cell?.literal?.value == null ? '' : String(cell.literal.value)),
    extend: cell?.extend_mode ?? 'default',
    left: typeof cell?.left_head === 'string' ? cell.left_head : '',
    top: typeof cell?.top_head === 'string' ? cell.top_head : '',
    rows: cell?.merge?.rows ?? 1,
    cols: cell?.merge?.cols ?? 1,
  }
}

/** Edit template coordinates only; calculated output coordinates never enter this function. */
export function applyTemplateCell(def: ReportDef, row0: number, col0: number, draft: CellDraft): EditResult {
  const fail = (error: string): EditResult => ({ error, warnings: [] })
  const row = row0 + 1, col = col0 + 1
  if (!Number.isInteger(row) || !Number.isInteger(col) || row < 1 || col < 1 || row > def.grid.rows || col > def.grid.cols) {
    return fail('所选单元格超出模板网格，请先调整网格尺寸')
  }
  if (coveredSlots(def).has(slotKey(row, col))) return fail('请选择合并区域的左上角单元格')
  if (!['default', 'none', 'vertical', 'horizontal'].includes(draft.extend)) return fail('扩展方向不合法')
  if (draft.kind === 'expr' && !draft.content.replace(/^=/, '').trim()) return fail('公式不能为空')
  if (![draft.rows, draft.cols].every(n => Number.isInteger(n) && n >= 1)) return fail('合并行列数须为正整数')
  if (row + draft.rows - 1 > def.grid.rows || col + draft.cols - 1 > def.grid.cols) return fail('合并区域超出模板网格')
  const cells = [...(def.cells ?? [])]
  const index = findCellIndex(def, row, col)
  const next: CellDef = { ...(cells[index] ?? { row, col }) }
  delete next.text
  delete next.expr
  delete next.literal
  if (draft.content !== '') next[draft.kind] = draft.kind === 'expr' ? draft.content.replace(/^=/, '') : draft.content
  next.extend_mode = draft.extend
  delete next.left_head
  delete next.top_head
  if (draft.left.trim()) next.left_head = draft.left.trim().toUpperCase()
  if (draft.top.trim()) next.top_head = draft.top.trim().toUpperCase()
  delete next.merge
  if (draft.rows > 1 || draft.cols > 1) next.merge = { rows: draft.rows, cols: draft.cols }
  for (const other of cells) {
    if (other.row === row && other.col === col) continue
    const overlaps = other.row < row + draft.rows && other.row + (other.merge?.rows ?? 1) > row &&
      other.col < col + draft.cols && other.col + (other.merge?.cols ?? 1) > col
    if (overlaps && (other.text !== undefined || other.expr !== undefined || other.merge)) return fail(`合并区域与 ${address(other.row - 1, other.col - 1)} 的内容或合并区域重叠`)
  }
  if (index < 0) cells.push(next)
  else cells[index] = next
  const resolveHead = (head: string): CellDef | undefined => {
    const at = position(head.toUpperCase())
    return at ? cells.find(c => c.row <= at[0] && c.row + (c.merge?.rows ?? 1) > at[0] && c.col <= at[1] && c.col + (c.merge?.cols ?? 1) > at[1]) : undefined
  }
  for (const cell of cells) {
    for (const field of ['left_head', 'top_head'] as const) {
      const head = cell[field]
      if (typeof head !== 'string' || !head) continue
      // 根终止本轴继承，没有实际主格，因而不受主格方向约束。
      if (head === '`0') continue
      const target = resolveHead(head)
      if (!target) return fail(`主格 ${head} 不存在，请先在该格填写公式或文本`)
      const mode = target.extend_mode ?? 'default'
      const wanted = field === 'left_head' ? 'vertical' : 'horizontal'
      if (mode !== 'default' && mode !== wanted) return fail(`${field === 'left_head' ? '左' : '上'}主格 ${head} 必须为${wanted === 'vertical' ? '纵向' : '横向'}扩展`)
      if ((field === 'left_head' && cell.extend_mode === 'horizontal') || (field === 'top_head' && cell.extend_mode === 'vertical')) {
        return fail('横向扩展格不能设置左主格，纵向扩展格不能设置上主格')
      }
      const visited = new Set<string>([address(cell.row - 1, cell.col - 1)])
      let current: CellDef | undefined = target
      while (current) {
        const id = address(current.row - 1, current.col - 1)
        if (visited.has(id)) return fail(`主格关系形成循环：${[...visited, id].join(' → ')}`)
        visited.add(id)
        const parent: unknown = current[field]
        current = typeof parent === 'string' ? resolveHead(parent) : undefined
      }
    }
  }
  return { def: { ...def, cells }, warnings: [] }
}

/** Native grid edits accept report expressions, including pasted rectangular ranges. */
export function applyTemplateValues(def: ReportDef, edits: { row0: number; col0: number; value: unknown }[]): EditResult {
  let next = def
  for (const edit of edits) {
    if (coveredSlots(next).has(slotKey(edit.row0 + 1, edit.col0 + 1))) continue
    const content = edit.value == null ? '' : String(edit.value)
    const draft = cellDraft(next, edit.row0, edit.col0)
    draft.content = content.startsWith('=') ? content.slice(1) : content
    draft.kind = content.startsWith('=') ? 'expr' : 'text'
    // Content changes may precede the creation of an explicitly referenced master.
    const row = edit.row0 + 1, col = edit.col0 + 1
    if (![row, col].every(Number.isInteger) || row < 1 || col < 1 || row > next.grid.rows || col > next.grid.cols) return { error: '粘贴区域超出模板网格', warnings: [] }
    if (draft.kind === 'expr' && !draft.content.trim()) return { error: '公式不能为空', warnings: [] }
    const cells = [...(next.cells ?? [])]
    const index = findCellIndex(next, row, col)
    const cell = { ...(cells[index] ?? { row, col }) }
    delete cell.literal
    delete cell.expr
    delete cell.text
    if (draft.content !== '') cell[draft.kind] = draft.content
    if (index < 0) cells.push(cell)
    else cells[index] = cell
    next = { ...next, cells }
  }
  return { def: next, warnings: [] }
}

export function templateCellData(cell: CellDef): ICellData {
  const style = jsonToUniverStyle(cell)
  const border = jsonBorderToUniver(cell.border)
  if (border) style.bd = border
  return { v: cell.expr !== undefined ? `=${cell.expr}` : cell.text ?? (cell.literal?.value == null ? '' : String(cell.literal.value)), t: CellValueType.FORCE_STRING, f: null, p: null, s: style }
}

export function templateInputValue(cell: ICellData | null | undefined | void): unknown {
  return cell?.f ?? cell?.p?.body?.dataStream?.replace(/\r?\n$/, '') ?? cell?.v ?? ''
}

/** Build the unexpanded design sheet directly from the authoritative definition. */
export function templateWorkbook(def: ReportDef, name: string) {
  const cellData: Record<number, Record<number, ICellData>> = {}
  for (const cell of def.cells ?? []) (cellData[cell.row - 1] ??= {})[cell.col - 1] = templateCellData(cell)
  const rowData: Record<number, { h: number }> = {}
  const columnData: Record<number, { w: number }> = {}
  for (const prop of (def.row_props ?? []) as { row: number; height?: unknown }[]) {
    if (typeof prop.height === 'number') rowData[prop.row - 1] = { h: mmToPx(prop.height) }
  }
  for (const prop of (def.col_props ?? []) as { col: number; width?: unknown }[]) {
    if (typeof prop.width === 'number') columnData[prop.col - 1] = { w: mmToPx(prop.width) }
  }
  return {
    id: 'report-template', name, locale: 'zhCN', appVersion: '1.0.0', sheetOrder: ['template'],
    sheets: { template: {
      id: 'template', name: '模板设计', rowCount: def.grid.rows, columnCount: def.grid.cols,
      defaultRowHeight: mmToPx(8), defaultColumnWidth: mmToPx(25), rowData, columnData, cellData,
      mergeData: (def.cells ?? []).filter(c => c.merge).map(c => ({
        startRow: c.row - 1, endRow: c.row + c.merge!.rows - 2,
        startColumn: c.col - 1, endColumn: c.col + c.merge!.cols - 2,
      })),
    } },
  }
}

export function parseTemplate(text: string): ReportDef {
  const value: unknown = JSON.parse(text)
  if (!value || typeof value !== 'object' || !('grid' in value)) throw new Error('模板缺少 grid 字段')
  const def = value as ReportDef
  if (!def.grid || ![def.grid.rows, def.grid.cols].every(n => Number.isInteger(n) && n >= 1)) throw new Error('模板行列数须为正整数')
  if (def.cells !== undefined && !Array.isArray(def.cells)) throw new Error('模板 cells 必须为数组')
  const occupied = new Set<string>()
  for (const cell of def.cells ?? []) {
    if (!cell || ![cell.row, cell.col].every(Number.isInteger) || cell.row < 1 || cell.col < 1 || cell.row > def.grid.rows || cell.col > def.grid.cols) throw new Error('模板单元格坐标须在 grid 范围内')
    const id = address(cell.row - 1, cell.col - 1)
    if (occupied.has(id)) throw new Error(`模板单元格 ${id} 重复`)
    occupied.add(id)
    if ((cell.text !== undefined && typeof cell.text !== 'string') || (cell.expr !== undefined && typeof cell.expr !== 'string') || (cell.text !== undefined && cell.expr !== undefined)) throw new Error(`${id} 的 text / expr 须为字符串且互斥`)
    if (cell.merge && (![cell.merge.rows, cell.merge.cols].every(n => Number.isInteger(n) && n >= 1) || cell.row + cell.merge.rows - 1 > def.grid.rows || cell.col + cell.merge.cols - 1 > def.grid.cols)) throw new Error(`${id} 的合并区域不合法`)
  }
  const covered = coveredSlots(def)
  if (def.cells?.some(c => covered.has(slotKey(c.row, c.col)) && (c.text !== undefined || c.expr !== undefined || c.merge))) throw new Error('模板合并区域重叠或覆盖了其他内容')
  for (const field of ['datasets', 'params', 'row_props', 'col_props'] as const) {
    if (def[field] !== undefined && (!Array.isArray(def[field]) || (def[field] as unknown[]).some(item => !item || typeof item !== 'object'))) throw new Error(`模板 ${field} 须为对象数组`)
  }
  return def
}
