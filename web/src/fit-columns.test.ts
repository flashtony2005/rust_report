import { describe, expect, it } from 'vitest'
import { displayText, fitColumns, type Measure } from './fit-columns'
import type { PrintWorkbook } from './PrintPreview'

/** 固定字宽：CJK 按 1em、其余按 0.55em（pt→px 同屏换算），测试不碰 DOM。 */
const measure: Measure = (text, font) => {
  const em = (font.size * 96) / 72 * (font.bold ? 1.05 : 1)
  let width = 0
  for (const ch of text) width += /[\u2e80-\u9fff\uff00-\uffef]/.test(ch) ? em : em * 0.55
  return width
}

function cell(v: unknown, extra: Record<string, unknown> = {}) {
  return { v, t: 2, s: { ff: '宋体', fs: 11, ...extra } }
}

function workbook(columns: number[], cells: Record<string, Record<string, unknown>>, merges: PrintWorkbook['sheets'][string]['mergeData'] = []): PrintWorkbook {
  const columnData: Record<string, { w: number }> = {}
  columns.forEach((w, i) => { columnData[String(i)] = { w } })
  const sheet = {
    name: 'Sheet1',
    printRowCount: 1,
    printColumnCount: columns.length,
    defaultRowHeight: 24,
    defaultColumnWidth: 88,
    rowData: {},
    columnData,
    cellData: cells as PrintWorkbook['sheets'][string]['cellData'],
    mergeData: merges,
    printSize: { width: 210, height: 297, margins: [10, 10, 10, 10] },
  }
  return { sheetOrder: ['s1'], sheets: { s1: sheet as PrintWorkbook['sheets'][string] } }
}

const width = (book: PrintWorkbook) => Object.values(book.sheets.s1.columnData ?? {}).map(c => c.w)

describe('displayText', () => {
  it('按格上声明的格式取显示文本，与值类型无关', () => {
    expect(displayText(cell(1286000, { n: { pattern: '#,##0.00' } }))).toBe('1,286,000.00')
    expect(displayText(cell(0.3954, { n: { pattern: '0.0%' } }))).toBe('39.5%')
  })

  it('无格式按 General（长尾小数会被收成 11 字符口径）', () => {
    expect(displayText(cell(1286000))).toBe('1286000')
    expect(displayText(cell(0.39544895448954487))).toHaveLength(11)
  })

  it('文本格不参与（返回 null）', () => {
    expect(displayText({ v: '华东机械集团', t: 1 })).toBeNull()
    expect(displayText({ v: '合计' })).toBeNull()
    expect(displayText({ v: true })).toBeNull()
  })
})

describe('fitColumns', () => {
  it('数字放不下时把列拉宽到刚好放得下', () => {
    const book = workbook([75], { '0': { '0': cell(1286000, { n: { pattern: '#,##0.00' } }) } })
    const fitted = fitColumns(book, measure)
    expect(width(fitted)[0]).toBeGreaterThan(75)
    expect(width(fitted)[0]).toBeCloseTo(measure('1,286,000.00', { family: '宋体', size: 11, bold: false, italic: false }) + 8, 1)
  })

  it('放得下的列一个像素都不动（只加不减）', () => {
    const book = workbook([200], { '0': { '0': cell(12, { n: { pattern: '0' } }) } })
    expect(width(fitColumns(book, measure))).toEqual([200])
  })

  it('文本格不触发加宽（文本溢出到邻格，不会画井号）', () => {
    const book = workbook([40], { '0': { '0': { v: '很长很长很长很长很长的文本' } } })
    expect(width(fitColumns(book, measure))).toEqual([40])
  })

  it('合并区按整段宽度算，缺口平均摊到跨度内各列', () => {
    const book = workbook(
      [40, 40],
      { '0': { '0': cell(1286000, { n: { pattern: '#,##0.00' } }) } },
      [{ startRow: 0, startColumn: 0, endRow: 0, endColumn: 1 }],
    )
    const fitted = fitColumns(book, measure)
    const [a, b] = width(fitted)
    expect(a).toBeCloseTo(b, 5)
    expect(a + b).toBeGreaterThanOrEqual(measure('1,286,000.00', { family: '宋体', size: 11, bold: false, italic: false }) + 8)
  })

  it('隐藏列不参与、也不会被加宽', () => {
    const book = workbook([40, 40], { '0': { '0': cell(1286000, { n: { pattern: '#,##0.00' } }) } })
    book.sheets.s1.columnData['1'].hd = 1
    const fitted = fitColumns(book, measure)
    expect(fitted.sheets.s1.columnData['1']).toEqual({ w: 40, hd: 1 })
    expect(fitted.sheets.s1.columnData['0'].w).toBeGreaterThan(40)
  })

  it('不修改入参快照（打印预览仍用原列宽）', () => {
    const book = workbook([40], { '0': { '0': cell(1286000, { n: { pattern: '#,##0.00' } }) } })
    fitColumns(book, measure)
    expect(book.sheets.s1.columnData['0'].w).toBe(40)
  })

  it('幂等：再适配一次不再变宽', () => {
    const book = workbook([40], { '0': { '0': cell(1286000, { n: { pattern: '#,##0.00' } }) } })
    const once = fitColumns(book, measure)
    expect(width(fitColumns(once, measure))).toEqual(width(once))
  })

  it('单列上限 600px：异常长的数字不把画布拉成一条线', () => {
    const book = workbook([40], { '0': { '0': cell(123456789012, { n: { pattern: '#,##0.00' } }) } })
    expect(width(fitColumns(book, measure))[0]).toBeLessThanOrEqual(600)
  })
})
