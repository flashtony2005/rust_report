import { describe, expect, it } from 'vitest'
import { CellValueType } from '@univerjs/core'
import complex from '../../examples/tutorial/complex-master-expansion.json'
import { exportDefJson, type ReportDef } from './designer'
import { applyTemplateCell, applyTemplateValues, cellDraft, parseTemplate, templateInputValue, templateWorkbook } from './template'

const fixture = () => parseTemplate(JSON.stringify(complex))
const simple = (): ReportDef => ({ grid: { rows: 4, cols: 4 }, cells: [
  { row: 1, col: 1, text: '0123', bold: true, custom: { retained: true } },
  { row: 2, col: 2, expr: 'to(1,3)', extend_mode: 'vertical' },
  { row: 3, col: 3, expr: 'to(1,2)', extend_mode: 'horizontal' },
] })

describe('original template coordinates and persistence', () => {
  it('keeps the complex master definition unexpanded, including literal expressions and geometry', () => {
    const def = fixture()
    const before = exportDefJson(def)
    const sheet = templateWorkbook(def, 'complex.json').sheets.template
    expect([sheet.rowCount, sheet.columnCount]).toEqual([12, 11])
    expect(sheet.cellData[5][2]).toMatchObject({ v: '=&D6', t: CellValueType.FORCE_STRING, f: null })
    expect(def.cells?.find(c => c.row === 6 && c.col === 3)?.left_head).toBe('D6')
    expect(sheet.mergeData[0]).toEqual({ startRow: 0, startColumn: 0, endRow: 0, endColumn: 10 })
    expect(sheet.rowData[0].h).toBeCloseTo(12 * 96 / 25.4)
    expect(exportDefJson(def)).toBe(before)
  })

  it('saves edits while preserving datasets, master relations, styles, and unknown fields', () => {
    const original = fixture()
    const result = applyTemplateCell(original, 5, 2, { ...cellDraft(original, 5, 2), content: '&D6+10' })
    expect(result.error).toBeUndefined()
    const loaded = parseTemplate(exportDefJson(result.def!))
    const cell = loaded.cells?.find(c => c.row === 6 && c.col === 3)
    expect(cell).toMatchObject({ expr: '&D6+10', left_head: 'D6', fore_color: '#176B68' })
    expect(loaded.datasets).toEqual(original.datasets)
    expect(loaded.print_setup).toEqual(original.print_setup)
    expect(original.cells?.find(c => c.row === 6 && c.col === 3)?.expr).toBe('&D6')
    const edited = applyTemplateCell(simple(), 0, 0, { ...cellDraft(simple(), 0, 0), kind: 'expr', content: '=1+2' }).def!
    expect(edited.cells?.[0]).toMatchObject({ expr: '1+2', bold: true, custom: { retained: true } })
    expect(edited.cells?.[0]).not.toHaveProperty('text')
    const text = applyTemplateValues(edited, [{ row0: 0, col0: 0, value: '0123' }]).def!.cells![0]
    expect(text.text).toBe('0123')
    expect(text).not.toHaveProperty('expr')
  })
})

describe('master relations', () => {
  it.each(['default', 'none', 'vertical', 'horizontal'] as const)('displays and saves explicit roots with %s extension', extend => {
    const def = simple()
    const result = applyTemplateCell(def, 0, 0, { ...cellDraft(def, 0, 0), extend, left: ' `0 ', top: '`0' })
    expect(result.error).toBeUndefined()
    const loaded = parseTemplate(exportDefJson(result.def!))
    expect(loaded.cells?.[0]).toMatchObject({ left_head: '`0', top_head: '`0', bold: true, custom: { retained: true } })
    expect(cellDraft(loaded, 0, 0)).toMatchObject({ left: '`0', top: '`0', extend })
    expect(def.cells?.[0]).not.toHaveProperty('left_head')
  })

  it('clears a root back to automatic independently on each axis', () => {
    const def = simple()
    const rooted = applyTemplateCell(def, 0, 0, { ...cellDraft(def, 0, 0), left: '`0', top: '`0' }).def!
    const leftAuto = applyTemplateCell(rooted, 0, 0, { ...cellDraft(rooted, 0, 0), left: ' ' }).def!
    expect(leftAuto.cells?.[0]).not.toHaveProperty('left_head')
    expect(cellDraft(parseTemplate(exportDefJson(leftAuto)), 0, 0)).toMatchObject({ left: '', top: '`0' })
    const automatic = applyTemplateCell(leftAuto, 0, 0, { ...cellDraft(leftAuto, 0, 0), top: '' }).def!
    expect(automatic.cells?.[0]).not.toHaveProperty('top_head')
    expect(cellDraft(automatic, 0, 0)).toMatchObject({ left: '', top: '' })
  })

  it('terminates a declared master chain at a root and preserves it during grid edits', () => {
    const def = simple()
    def.cells![1].left_head = '`0'
    const result = applyTemplateCell(def, 0, 0, { ...cellDraft(def, 0, 0), left: 'B2', top: '`0' })
    expect(result.error).toBeUndefined()
    const pasted = applyTemplateValues(result.def!, [{ row0: 0, col0: 0, value: '=1+2' }]).def!
    const loaded = parseTemplate(exportDefJson(pasted))
    expect(loaded.cells?.[0]).toMatchObject({ expr: '1+2', left_head: 'B2', top_head: '`0' })
    expect(loaded.cells?.[1].left_head).toBe('`0')
  })

  it.each(['0', 'root', 'ROOT', '`00', '``0'])('rejects the root alias %s', head => {
    for (const field of ['left', 'top'] as const) {
      const def = simple()
      expect(applyTemplateCell(def, 0, 0, { ...cellDraft(def, 0, 0), [field]: head }).error).toContain('不存在')
    }
  })

  it('accepts a vertical master to the right and a horizontal master below', () => {
    const def = simple()
    const result = applyTemplateCell(def, 0, 0, { ...cellDraft(def, 0, 0), left: ' b2 ', top: 'c3' })
    expect(result.error).toBeUndefined()
    expect(result.def?.cells?.[0]).toMatchObject({ left_head: 'B2', top_head: 'C3' })
  })

  it.each([
    { left: 'Z9', top: '', extend: 'none' as const, error: '不存在' },
    { left: 'C3', top: '', extend: 'none' as const, error: '必须为纵向' },
    { left: '', top: 'B2', extend: 'none' as const, error: '必须为横向' },
    { left: 'B2', top: '', extend: 'horizontal' as const, error: '横向扩展格不能' },
    { left: '', top: 'C3', extend: 'vertical' as const, error: '纵向扩展格不能' },
    { left: 'Z9', top: '`0', extend: 'none' as const, error: '不存在' },
    { left: '`0', top: 'B2', extend: 'none' as const, error: '必须为横向' },
    { left: 'C3', top: '`0', extend: 'none' as const, error: '必须为纵向' },
    { left: 'B2', top: '`0', extend: 'horizontal' as const, error: '横向扩展格不能' },
    { left: '`0', top: 'C3', extend: 'vertical' as const, error: '纵向扩展格不能' },
  ])('rejects invalid relation $left / $top / $extend', ({ error, ...patch }) => {
    const def = simple()
    const result = applyTemplateCell(def, 0, 0, { ...cellDraft(def, 0, 0), ...patch })
    expect(result.error).toContain(error)
    expect(result.def).toBeUndefined()
  })

  it('detects cycles through merged continuation coordinates', () => {
    const def: ReportDef = { grid: { rows: 4, cols: 4 }, cells: [
      { row: 1, col: 1, expr: 'to(1,3)', extend_mode: 'vertical', merge: { rows: 1, cols: 2 }, left_head: 'C2' },
      { row: 2, col: 3, expr: 'to(1,3)', extend_mode: 'vertical' },
    ] }
    const result = applyTemplateCell(def, 1, 2, { ...cellDraft(def, 1, 2), left: 'B1' })
    expect(result.error).toContain('循环')
  })
})

describe('grid edits and validation', () => {
  it('reads report formulas before native calculated values and preserves rich text identifiers', () => {
    expect(templateInputValue({ f: '=ds1.sum(amount)', v: '#NAME?' })).toBe('=ds1.sum(amount)')
    expect(templateInputValue({ p: { id: 'doc', documentStyle: {}, body: { dataStream: '00123\r\n' } } })).toBe('00123')
    expect(templateInputValue({ v: false })).toBe(false)
    expect(templateInputValue(null)).toBe('')
  })
  it('applies a rectangular paste atomically and keeps formulas and literal identifiers', () => {
    const def = simple()
    const edits = [{ row0: 0, col0: 0, value: '00123' }, { row0: 0, col0: 1, value: '=ds1.sum(amount)' }]
    const result = applyTemplateValues(def, edits)
    expect(result.def?.cells?.find(c => c.row === 1 && c.col === 2)?.expr).toBe('ds1.sum(amount)')
    expect(result.def?.cells?.[0]).toMatchObject({ text: '00123', bold: true })
    expect(applyTemplateValues(def, [...edits, { row0: 4, col0: 0, value: 'bad' }]).def).toBeUndefined()
    expect(def.cells?.[0].text).toBe('0123')
    expect(applyTemplateValues(def, [{ row0: 1, col0: 0, value: '=' }]).error).toContain('不能为空')
  })

  it('rejects merging over content and skips continuation slots in a paste', () => {
    const def = simple()
    expect(applyTemplateCell(def, 0, 0, { ...cellDraft(def, 0, 0), rows: 2, cols: 2 }).error).toContain('重叠')
    const merged = applyTemplateCell(def, 0, 0, { ...cellDraft(def, 0, 0), cols: 2 }).def!
    const pasted = applyTemplateValues(merged, [{ row0: 0, col0: 0, value: '标题' }, { row0: 0, col0: 1, value: 'covered' }]).def!
    expect(pasted.cells?.find(c => c.row === 1 && c.col === 2)).toBeUndefined()
    expect(applyTemplateCell(merged, 0, 1, cellDraft(merged, 0, 1)).error).toContain('左上角')
    const empty: ReportDef = { grid: { rows: 1, cols: 2 }, cells: [{ row: 1, col: 2, text: '' }] }
    expect(applyTemplateCell(empty, 0, 0, { ...cellDraft(empty, 0, 0), cols: 2 }).error).toContain('重叠')
  })

  it.each([
    {}, { grid: { rows: 0, cols: 2 } }, { grid: { rows: 2, cols: 2 }, cells: [null] },
    { grid: { rows: 2, cols: 2 }, cells: [{ row: 3, col: 1 }] },
    { grid: { rows: 2, cols: 2 }, cells: [{ row: 1, col: 1, text: 'a', expr: '1' }] },
    { grid: { rows: 2, cols: 2 }, cells: [{ row: 1, col: 1, merge: { rows: 3, cols: 1 } }] },
    { grid: { rows: 2, cols: 2 }, datasets: {} },
  ])('rejects malformed template %j before mounting Univer', input => {
    expect(() => parseTemplate(JSON.stringify(input))).toThrow()
  })
})
