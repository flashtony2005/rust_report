/**
 * 行列属性 + 网格尺寸补丁单测（M7-W W2-5a）。
 *
 * 判据分六组：
 *
 * 1. **读方向**（`rowHeightMm` / `colWidthMm` / `rowBreakPage`）—— 面板显示态
 *    从**定义 JSON** 派生（`cellprops.ts` 同款理由：声明过什么只有 JSON 知道；
 *    Univer 活态是 px 且含快照换算噪声）。
 * 2. **行高/列宽补丁**（`applyRowHeights` / `applyColWidths`）—— 只动 `height`/
 *    `width` 键；其余键（含**字符串形态**的表达式透传）原样保留；null = 删键，
 *    条目删空整条移除；越界/非正数显式报错。
 * 3. **断页补丁**（`applyRowBreakPages`）—— 布尔写键、null 删键（服务端
 *    `build.rs` 负责布尔 → `"1==1"`/`"1==0"` 的转换，前端只写契约形态）。
 * 4. **grid 缩小校验**（`applyGridResize`）—— cells 锚点/合并矩形/row_props/
 *    col_props 越界各自显式报错（不静默删声明）；扩大无副作用。
 * 5. **单位换算**（`mmToPx`）—— 与快照 `geom.rs::px` 同公式（8mm→30.236…、
 *    25mm→94.488…，快照值是它四舍五入到 2 位的结果）。
 * 6. **纯函数性** —— 输入 def 不被改写（与 `designer.test.ts` 同款深拷贝对照）。
 */
import { describe, expect, it } from 'vitest'

import type { ReportDef } from './designer'
import {
  DEFAULT_COL_MM,
  DEFAULT_ROW_MM,
  applyColWidths,
  applyGridResize,
  applyRowBreakPages,
  applyRowHeights,
  colPropOf,
  colWidthMm,
  mmToPx,
  rowBreakPage,
  rowHeightMm,
  rowPropOf,
} from './rowcol'

const baseDef = (): ReportDef => ({
  version: 1,
  grid: { rows: 3, cols: 2 },
  cells: [{ row: 1, col: 1, text: 'A' }],
})

const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T

// ---------------------------------------------------------------------------
// 1. 读方向
// ---------------------------------------------------------------------------

describe('读方向：从定义 JSON 派生', () => {
  it('未声明的行/列显示契约缺省（8mm / 25mm）', () => {
    const def = baseDef()
    expect(rowHeightMm(def, 1)).toBe(DEFAULT_ROW_MM)
    expect(colWidthMm(def, 2)).toBe(DEFAULT_COL_MM)
    expect(rowPropOf(def, 1)).toBeUndefined()
    expect(colPropOf(def, 1)).toBeUndefined()
  })

  it('数字字面量原样读回', () => {
    const def: ReportDef = {
      ...baseDef(),
      row_props: [{ row: 2, height: 12 }],
      col_props: [{ col: 1, width: 30 }],
    }
    expect(rowHeightMm(def, 2)).toBe(12)
    expect(colWidthMm(def, 1)).toBe(30)
  })

  it('字符串形态（表达式透传）不猜数值 ⇒ undefined', () => {
    const def: ReportDef = { ...baseDef(), row_props: [{ row: 1, height: 'A1*2' }] }
    expect(rowHeightMm(def, 1)).toBeUndefined()
    expect(rowPropOf(def, 1)?.height).toBe('A1*2')
  })

  it('断页三态：true / false / 未声明；字符串表达式不算布尔', () => {
    const def: ReportDef = {
      ...baseDef(),
      row_props: [
        { row: 1, break_page: true },
        { row: 2, break_page: false },
        { row: 3, break_page: 'A1==1' },
      ],
    }
    expect(rowBreakPage(def, 1)).toBe(true)
    expect(rowBreakPage(def, 2)).toBe(false)
    expect(rowBreakPage(def, 3)).toBeUndefined()
  })
})

// ---------------------------------------------------------------------------
// 2. 行高 / 列宽补丁
// ---------------------------------------------------------------------------

describe('applyRowHeights：逐键补丁', () => {
  it('无声明 ⇒ 追加条目 {row, height}', () => {
    const def = baseDef()
    const r = applyRowHeights(def, [2], 12)
    expect(r.error).toBeUndefined()
    expect(r.def?.row_props).toEqual([{ row: 2, height: 12 }])
  })

  it('多行一次补丁 ⇒ 多条条目（保持其余 JSON 原样）', () => {
    const def: ReportDef = { ...baseDef(), datasets: [{ file: { file_name: 'a.csv' } }] }
    const r = applyRowHeights(def, [1, 3], 10)
    expect(r.def?.row_props).toEqual([
      { row: 1, height: 10 },
      { row: 3, height: 10 },
    ])
    expect(r.def?.datasets).toEqual([{ file: { file_name: 'a.csv' } }])
  })

  it('已有条目只动 height 键（break_page 保留）', () => {
    const def: ReportDef = { ...baseDef(), row_props: [{ row: 1, break_page: true }] }
    const r = applyRowHeights(def, [1], 9.5)
    expect(r.def?.row_props).toEqual([{ row: 1, break_page: true, height: 9.5 }])
  })

  it('null = 恢复缺省：删 height 键；条目无属性 ⇒ 删整条', () => {
    const def: ReportDef = {
      ...baseDef(),
      row_props: [
        { row: 1, height: 12 },
        { row: 2, height: 12, break_page: true },
      ],
    }
    const r = applyRowHeights(def, [1, 2], null)
    expect(r.def?.row_props).toEqual([{ row: 2, break_page: true }])
    expect(r.warnings.join()).toContain('重渲染')
  })

  it('已有 height 保留其余键不动；未涉及的条目原样', () => {
    const def: ReportDef = {
      ...baseDef(),
      row_props: [
        { row: 1, height: 12 },
        { row: 2, height: 20 },
      ],
    }
    const r = applyRowHeights(def, [1], 15)
    expect(r.def?.row_props).toEqual([
      { row: 1, height: 15 },
      { row: 2, height: 20 },
    ])
  })

  it('越界行报错（约束 4 前置）', () => {
    const r = applyRowHeights(baseDef(), [4], 10)
    expect(r.error).toContain('超出网格')
    expect(r.def).toBeUndefined()
  })

  it('非正数 / 非有限数报错', () => {
    expect(applyRowHeights(baseDef(), [1], 0).error).toContain('正数')
    expect(applyRowHeights(baseDef(), [1], -3).error).toContain('正数')
    expect(applyRowHeights(baseDef(), [1], NaN).error).toContain('正数')
  })

  it('纯函数：输入 def 不被改写', () => {
    const def = baseDef()
    const snapshot = clone(def)
    applyRowHeights(def, [1], 12)
    expect(def).toEqual(snapshot)
  })
})

describe('applyColWidths：同款口径', () => {
  it('追加 / 删键 / 删空条目', () => {
    const def: ReportDef = { ...baseDef(), col_props: [{ col: 1, width: 30 }] }
    const added = applyColWidths(def, [2], 18)
    expect(added.def?.col_props).toEqual([
      { col: 1, width: 30 },
      { col: 2, width: 18 },
    ])
    const cleared = applyColWidths(added.def!, [1, 2], null)
    expect(cleared.def?.col_props).toEqual([])
  })

  it('越界列 / 非正数报错', () => {
    expect(applyColWidths(baseDef(), [3], 10).error).toContain('超出网格')
    expect(applyColWidths(baseDef(), [1], 0).error).toContain('正数')
  })
})

// ---------------------------------------------------------------------------
// 3. 断页补丁
// ---------------------------------------------------------------------------

describe('applyRowBreakPages', () => {
  it('true / false 写布尔键（转换成表达式是服务端 build 的事）', () => {
    const r = applyRowBreakPages(baseDef(), [2], true)
    expect(r.def?.row_props).toEqual([{ row: 2, break_page: true }])
    const r2 = applyRowBreakPages(baseDef(), [2], false)
    expect(r2.def?.row_props).toEqual([{ row: 2, break_page: false }])
    expect(r.warnings.join()).toContain('重渲染')
  })

  it('null = 删键；条目删空整条移除', () => {
    const def: ReportDef = { ...baseDef(), row_props: [{ row: 1, break_page: true }] }
    const r = applyRowBreakPages(def, [1], null)
    expect(r.def?.row_props).toEqual([])
  })

  it('不动同条目的 height；字符串表达式 height 原样保留', () => {
    const def: ReportDef = { ...baseDef(), row_props: [{ row: 1, height: 'A1*2' }] }
    const r = applyRowBreakPages(def, [1], true)
    expect(r.def?.row_props).toEqual([{ row: 1, height: 'A1*2', break_page: true }])
  })

  it('越界行报错', () => {
    expect(applyRowBreakPages(baseDef(), [9], true).error).toContain('超出网格')
  })
})

// ---------------------------------------------------------------------------
// 4. grid 尺寸
// ---------------------------------------------------------------------------

describe('applyGridResize：缩小先校验越界', () => {
  it('扩大：直接改 grid，无其它副作用', () => {
    const r = applyGridResize(baseDef(), 5, 4)
    expect(r.error).toBeUndefined()
    expect(r.def?.grid).toEqual({ rows: 5, cols: 4 })
    expect(r.def?.cells).toEqual(baseDef().cells)
  })

  it('合法缩小（无声明越界）成立', () => {
    const r = applyGridResize(baseDef(), 1, 1)
    expect(r.error).toBeUndefined()
    expect(r.def?.grid).toEqual({ rows: 1, cols: 1 })
    expect(r.warnings.join()).toContain('重渲染')
  })

  it('缩行：cells 锚点越界报错', () => {
    const r = applyGridResize(baseDef(), 1, 2) // cells 里有 (1,1) 不越界 → 用 (2,1)
    expect(r.error).toBeUndefined()
    const def2: ReportDef = { ...baseDef(), cells: [{ row: 2, col: 1, text: 'B' }] }
    expect(applyGridResize(def2, 1, 2).error).toContain('行 2')
  })

  it('缩行：合并矩形整体溢出也报错（锚点在界内但矩形超出）', () => {
    const def: ReportDef = {
      ...baseDef(),
      cells: [{ row: 2, col: 1, text: 'B', merge: { rows: 2, cols: 1 } }],
    }
    // 新 rows=3：锚点 2 + 跨 2 - 1 = 3 ≤ 3 合法；rows=2 → 3 > 2 溢出。
    expect(applyGridResize(def, 3, 2).error).toBeUndefined()
    expect(applyGridResize(def, 2, 2).error).toContain('合并矩形')
  })

  it('缩列：cells 列越界报错', () => {
    const def2: ReportDef = { ...baseDef(), cells: [{ row: 1, col: 2, text: 'C' }] }
    expect(applyGridResize(def2, 3, 1).error).toContain('列 2')
  })

  it('缩行：row_props 越界报错；缩列：col_props 越界报错', () => {
    const def: ReportDef = {
      ...baseDef(),
      row_props: [{ row: 3, height: 10 }],
      col_props: [{ col: 2, width: 10 }],
    }
    expect(applyGridResize(def, 2, 2).error).toContain('row_props')
    expect(applyGridResize(def, 3, 1).error).toContain('col_props')
  })

  it('非法尺寸（0 / 非整数）报错', () => {
    expect(applyGridResize(baseDef(), 0, 2).error).toContain('≥1')
    expect(applyGridResize(baseDef(), 3.5, 2).error).toContain('≥1')
    expect(applyGridResize(baseDef(), 3, 0).error).toContain('≥1')
  })

  it('纯函数：输入 def 不被改写', () => {
    const def = baseDef()
    const snapshot = clone(def)
    applyGridResize(def, 10, 10)
    expect(def).toEqual(snapshot)
  })
})

// ---------------------------------------------------------------------------
// 5. 单位换算
// ---------------------------------------------------------------------------

describe('mmToPx：与快照 geom.rs::px 同公式', () => {
  it('缺省行高/列宽的换算值（快照的 30.24 / 94.49 是它们 round(2) 的结果）', () => {
    expect(mmToPx(8)).toBeCloseTo(30.236, 2)
    expect(mmToPx(25)).toBeCloseTo(94.488, 2)
    expect(Number(mmToPx(8).toFixed(2))).toBeCloseTo(30.24, 5)
    expect(Number(mmToPx(25).toFixed(2))).toBeCloseTo(94.49, 5)
  })
})
