/**
 * `mergeLiveLayout` / `captureSizeBaseline` 单测：打印预览取数层。
 *
 * 契约（见 `live-layout.ts` 头注释）：
 * - 打印尺寸 = 快照 + **增量**（增量 = 活态 − 装载时基线）——
 *   这样「显示层加宽」不会混进打印版面，用户拖的量才会反映；
 * - 基线必须在装载前**冻结**（Univer 会原地修改装载进去的对象）；
 * - 内容（cellData / mergeData / printSize / rpxGraphics）以**快照**为准；
 * - 活态缺失/形状不符 ⇒ 原样返回快照（不回归）。
 */
import { describe, expect, it } from 'vitest'
import { captureSizeBaseline, captureStyleBaseline, liveSheet, mergeLiveLayout, mergeLivePrint, mergeLiveStyles, type LiveWorkbook } from './live-layout'

/** 引擎快照：打印契约的权威来源。 */
const snapshot = {
  sheetOrder: ['s1'],
  sheets: {
    s1: {
      name: 'Sheet1',
      printSize: { width: 210, height: 297, margins: [25, 19, 25, 19] },
      columnData: { 0: { w: 52.91 }, 1: { w: 68.03 } },
      rowData: { 0: { h: 113.39 } },
      cellData: { 0: { 0: { v: 486000 } } },
      mergeData: [{ startRow: 0, endRow: 0, startColumn: 0, endColumn: 1 }],
      rpxGraphics: [{ row: 0, col: 0 }],
    },
  },
}

/** 画布装载的显示副本：`fitColumns` 把列 2 从 56.69 加宽到 73.16。 */
const display = {
  sheetOrder: ['s1'],
  sheets: {
    s1: {
      columnData: { 0: { w: 52.91 }, 1: { w: 68.03 }, 2: { w: 73.16 } },
      rowData: { 0: { h: 113.39 } },
    },
  },
}

/** 装载时冻结的基线（生产代码走 `captureSizeBaseline`）。 */
const baseline = captureSizeBaseline(display)

describe('captureSizeBaseline', () => {
  it('冻结每个 sheet 的行高/列宽', () => {
    expect(baseline.s1.columns['0']).toBe(52.91)
    expect(baseline.s1.columns['2']).toBe(73.16)
    expect(baseline.s1.rows['0']).toBe(113.39)
  })

  it('基线是深拷贝：改动源对象不影响基线（Univer 原地改装载对象）', () => {
    const source = {
      sheetOrder: ['s1'],
      sheets: { s1: { columnData: { 0: { w: 10 } }, rowData: {} } },
    }
    const frozen = captureSizeBaseline(source)
    source.sheets.s1.columnData['0'].w = 999
    expect(frozen.s1.columns['0']).toBe(10)
  })

  it('缺输入时返回空表', () => {
    expect(captureSizeBaseline(null)).toEqual({})
    expect(captureSizeBaseline(undefined)).toEqual({})
  })
})

describe('mergeLiveLayout', () => {
  it('用户拖宽一列 ⇒ 预览按增量跟着变', () => {
    // 活态：列 0 从 52.91 拖到 90.91（增量 +38）
    const live: LiveWorkbook = {
      sheetOrder: ['s1'],
      sheets: { s1: { columnData: { 0: { w: 90.91 } }, rowData: {} } },
    }
    const merged = mergeLiveLayout(snapshot, baseline, live)
    expect(merged.sheets.s1.columnData?.[0]?.w).toBe(90.91)
    // 没拖的列保持快照值
    expect(merged.sheets.s1.columnData?.['1']?.w).toBe(68.03)
  })

  it('没拖过 ⇒ 逐项与快照一致（不回归）', () => {
    const live: LiveWorkbook = {
      sheetOrder: ['s1'],
      sheets: { s1: { columnData: { 0: { w: 52.91 } }, rowData: { 0: { h: 113.39 } } } },
    }
    const merged = mergeLiveLayout(snapshot, baseline, live)
    expect(merged.sheets.s1.columnData?.['0']?.w).toBe(52.91)
    expect(merged.sheets.s1.rowData?.['0']?.h).toBe(113.39)
  })

  it('显示层加宽不进打印版面（活态列 2 = 73.16，快照没有列 2）', () => {
    const live: LiveWorkbook = {
      sheetOrder: ['s1'],
      sheets: { s1: { columnData: { 0: { w: 52.91 }, 1: { w: 68.03 }, 2: { w: 73.16 } }, rowData: {} } },
    }
    const merged = mergeLiveLayout(snapshot, baseline, live)
    expect((merged.sheets.s1.columnData as Record<string, { w?: number }>)['2']).toBeUndefined()
    expect(Object.keys(merged.sheets.s1.columnData ?? {})).toEqual(['0', '1'])
  })

  it('行高同样按增量合并', () => {
    const live: LiveWorkbook = {
      sheetOrder: ['s1'],
      sheets: { s1: { columnData: {}, rowData: { 0: { h: 200 } } } },
    }
    const merged = mergeLiveLayout(snapshot, baseline, live)
    expect(merged.sheets.s1.rowData?.['0']?.h).toBe(200)
  })

  it('内容类字段始终来自快照（活态不得污染）', () => {
    const live: LiveWorkbook = {
      sheetOrder: ['s1'],
      sheets: {
        s1: {
          columnData: { 0: { w: 999 } },
          cellData: { 0: { 0: { v: '被改坏的值' } } },
          mergeData: [],
          rpxGraphics: [],
          printSize: { width: 1, height: 1, margins: [0, 0, 0, 0] },
        },
      },
    }
    const merged = mergeLiveLayout(snapshot, baseline, live)
    expect(merged.sheets.s1.cellData?.[0]?.[0]).toEqual({ v: 486000 })
    expect(merged.sheets.s1.mergeData).toHaveLength(1)
    expect(merged.sheets.s1.rpxGraphics).toHaveLength(1)
    expect(merged.sheets.s1.printSize?.width).toBe(210)
  })

  it('活态缺失 ⇒ 原样返回快照', () => {
    expect(mergeLiveLayout(snapshot, baseline, null)).toBe(snapshot)
    expect(mergeLiveLayout(snapshot, baseline, undefined)).toBe(snapshot)
    expect(mergeLiveLayout(snapshot, baseline, { sheetOrder: [], sheets: {} })).toBe(snapshot)
  })

  it('基线缺失 ⇒ 原样返回快照（无从判断增量）', () => {
    const live: LiveWorkbook = {
      sheetOrder: ['s1'],
      sheets: { s1: { columnData: { 0: { w: 999 } }, rowData: {} } },
    }
    expect(mergeLiveLayout(snapshot, null, live)).toBe(snapshot)
  })

  it('活态里没有该 sheet ⇒ 不动快照', () => {
    const live: LiveWorkbook = { sheetOrder: ['other'], sheets: { other: { columnData: { 0: { w: 1 } } } } }
    expect(mergeLiveLayout(snapshot, baseline, live)).toBe(snapshot)
  })

  it('活态 sheet 无尺寸字段 ⇒ 不动快照', () => {
    const live: LiveWorkbook = { sheetOrder: ['s1'], sheets: { s1: { name: 'x' } } }
    expect(mergeLiveLayout(snapshot, baseline, live)).toBe(snapshot)
  })
})

describe('liveSheet', () => {
  it('sheets 是普通对象时取到', () => {
    expect(liveSheet({ sheets: { s1: { name: 'a' } } }, 's1')?.name).toBe('a')
  })

  it('sheets 是 Map 时取到（Univer 活态实测形态）', () => {
    const map = new Map([['s1', { name: 'from-map' }]])
    expect(liveSheet({ sheets: map as unknown as LiveWorkbook['sheets'] }, 's1')?.name).toBe('from-map')
  })

  it('取不到返回 undefined', () => {
    expect(liveSheet(null, 's1')).toBeUndefined()
    expect(liveSheet({}, 's1')).toBeUndefined()
    expect(liveSheet({ sheets: {} }, 's1')).toBeUndefined()
  })
})

// ---------------------------------------------------------------------------
// 样式层（2026-10-20）：结果画布上改的格式要进打印预览
// ---------------------------------------------------------------------------
describe('样式：captureStyleBaseline + mergeLiveStyles', () => {
  /** 快照：一格带内联样式，一格无样式。 */
  const book = () => ({
    sheetOrder: ['s1'],
    sheets: {
      s1: {
        cellData: {
          0: { 0: { v: '标题', s: { bl: 1, cl: { rgb: '#111111' } } }, 1: { v: '普通' } },
        },
      },
    },
  })

  it('基线把 `s` 解析成对象（含字符串 id 形态）并规范化键序', () => {
    const display = {
      sheetOrder: ['s1'],
      sheets: { s1: { cellData: { 0: { 0: { s: 'style-1' }, 1: { s: { fs: 12, bl: 1 } } } } } },
      styles: { 'style-1': { cl: { rgb: '#111111' }, bl: 1 } },
    }
    const baseline = captureStyleBaseline(display as unknown as LiveWorkbook)
    // 键序不同的同一份样式，序列化结果必须一致
    expect(baseline.s1['0:0']).toBe('{"bl":1,"cl":{"rgb":"#111111"}}')
    expect(baseline.s1['0:1']).toBe('{"bl":1,"fs":12}')
  })

  it('没改样式 ⇒ 原样返回快照（同一个对象）', () => {
    const snap = book()
    const display = {
      sheetOrder: ['s1'],
      sheets: { s1: { cellData: { 0: { 0: { s: { cl: { rgb: '#111111' }, bl: 1 } } } } } },
    }
    const baseline = captureStyleBaseline(display as unknown as LiveWorkbook)
    const live = {
      sheetOrder: ['s1'],
      sheets: { s1: { cellData: { 0: { 0: { s: 'k1' } } } } },
      styles: { k1: { bl: 1, cl: { rgb: '#111111' } } },
    }
    expect(mergeLiveStyles(snap, baseline, live as unknown as LiveWorkbook)).toBe(snap)
  })

  it('改了填充色（活态是字符串 id）⇒ 覆盖该格样式，其它格不动', () => {
    const snap = book()
    const display = {
      sheetOrder: ['s1'],
      sheets: { s1: { cellData: { 0: { 0: { s: { bl: 1, cl: { rgb: '#111111' } } } }, 1: {} } } },
    }
    const baseline = captureStyleBaseline(display as unknown as LiveWorkbook)
    const live = {
      sheetOrder: ['s1'],
      sheets: { s1: { cellData: { 0: { 0: { s: 'new' }, 1: { s: { bl: 1 } } } } } },
      styles: { new: { bl: 1, cl: { rgb: '#111111' }, bg: { rgb: '#FF0000' } } },
    }
    const merged = mergeLiveStyles(snap, baseline, live as unknown as LiveWorkbook)
    const cells = (merged.sheets.s1 as { cellData: Record<string, Record<string, { v?: unknown; s?: unknown }>> }).cellData
    expect(cells['0']['0'].s).toEqual({ bl: 1, cl: { rgb: '#111111' }, bg: { rgb: '#FF0000' } })
    expect(cells['0']['1'].s).toEqual({ bl: 1 })
    expect(cells['0']['0'].v).toBe('标题') // 内容始终以快照为准
  })

  it('清除格式（活态没有 s）⇒ 快照里那一格的 s 被删掉', () => {
    const snap = book()
    const display = {
      sheetOrder: ['s1'],
      sheets: { s1: { cellData: { 0: { 0: { s: { bl: 1 } } } } } },
    }
    const baseline = captureStyleBaseline(display as unknown as LiveWorkbook)
    const live = { sheetOrder: ['s1'], sheets: { s1: { cellData: { 0: { 0: { v: '标题' } } } } } }
    const merged = mergeLiveStyles(snap, baseline, live as unknown as LiveWorkbook)
    const cell = (merged.sheets.s1 as { cellData: Record<string, Record<string, { s?: unknown }>> }).cellData['0']['0']
    expect(cell.s).toBeUndefined()
    expect(cell).toMatchObject({ v: '标题' })
  })

  it('mergeLivePrint 同时合并尺寸与样式', () => {
    const snap = {
      sheetOrder: ['s1'],
      sheets: { s1: { columnData: { 0: { w: 100 } }, rowData: { 0: { h: 30 } }, cellData: { 0: { 0: { v: 'x', s: { bl: 0 } } } } } },
    }
    const display = {
      sheetOrder: ['s1'],
      sheets: { s1: { columnData: { 0: { w: 100 } }, rowData: { 0: { h: 30 } }, cellData: { 0: { 0: { s: { bl: 0 } } } } } },
    }
    const live = {
      sheetOrder: ['s1'],
      sheets: { s1: { columnData: { 0: { w: 140 } }, rowData: { 0: { h: 30 } }, cellData: { 0: { 0: { s: { bl: 1 } } } } } },
    }
    const merged = mergeLivePrint(snap, { sizes: captureSizeBaseline(display), styles: captureStyleBaseline(display as unknown as LiveWorkbook) }, live as unknown as LiveWorkbook)
    const sheet = merged.sheets.s1 as { columnData: Record<string, { w: number }>; cellData: Record<string, Record<string, { s: unknown }>> }
    expect(sheet.columnData['0'].w).toBe(140)
    expect(sheet.cellData['0']['0'].s).toEqual({ bl: 1 })
  })
})
