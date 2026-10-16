/**
 * 单元格属性面板单测（M7-W W2-3）。
 *
 * 判据分五组：
 *
 * 1. **读方向**（`cellPropsFromEntry`）—— 面板显示态从**定义 JSON** 读
 *    （快照不含这三项，见 `cellprops.ts` 文件头）。
 * 2. **选项注入**（`borderStyleOptions` / `formatOptions`）—— 手写值不能丢，
 *    否则 select 静默错显第一项。
 * 3. **值归一**（`normalizeCellPropsEdit`）—— 线型限契约 5 值、颜色大写、
 *    旋转 0..360 整数（越界**报错不夹取**）。
 * 4. **逐键补丁**（`applyCellPropsToRange`）—— 只动用户改的那一个键；
 *    `width`（JSON_ONLY）保留；四边删空删 `border` 键；条目删空整条移除；
 *    未登记字段原样保留；幂等；纯函数。
 * 5. **Univer 落地**（`applyCellPropToRange`）—— 单边开/关、线型/色落到
 *    「面板里开着的边」、格式空串=清除、旋转原样。
 */
import { describe, expect, it } from 'vitest'
import * as univer from '@univerjs/core'

import type { ReportDef } from './designer'
import type { PanelRange } from './stylepanel'
import type { CellDef } from './stylemap'
import {
  BORDER_LINE_STYLES,
  CELL_PROPS_DEFAULTS,
  applyCellPropToRange,
  applyCellPropsToRange,
  borderStyleOptions,
  cellPropsFromDef,
  cellPropsFromEntry,
  formatOptions,
  normalizeCellPropsEdit,
  type CellPropsRangeLike,
} from './cellprops'

const R = (row0: number, col0: number, height = 1, width = 1): PanelRange => ({
  row0,
  col0,
  height,
  width,
})

const defWith = (cells?: CellDef[], rows = 3, cols = 3): ReportDef => ({
  grid: { rows, cols },
  ...(cells ? { cells } : {}),
})

const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T

/** 记录调用的假 FRange（第 5 组用）。 */
function fakeRange(): { calls: [string, unknown[]][]; range: CellPropsRangeLike } {
  const calls: [string, unknown[]][] = []
  const rec =
    (name: string) =>
    (...args: unknown[]): void => {
      calls.push([name, args])
    }
  const range = {
    setBorder: rec('setBorder'),
    setNumberFormat: rec('setNumberFormat'),
    setTextRotation: rec('setTextRotation'),
  }
  return { calls, range: range as CellPropsRangeLike }
}

// ---------------------------------------------------------------------------
// 1. 读方向
// ---------------------------------------------------------------------------

describe('读方向：定义 JSON → 面板显示态', () => {
  it('无条目 ⇒ 全缺省', () => {
    expect(cellPropsFromEntry(undefined)).toEqual(CELL_PROPS_DEFAULTS)
  })

  it('四边齐 ⇒ 全勾选；线型/色取第一条存在的边（top 优先）', () => {
    const cell: CellDef = {
      row: 1,
      col: 1,
      border: {
        left: { style: 'dashed' },
        top: { style: 'double', color: '#0000ff' },
        bottom: { style: 'solid' },
        right: { style: 'dotted' },
      },
    }
    const p = cellPropsFromEntry(cell)
    expect(p.sides).toEqual({ top: true, right: true, bottom: true, left: true })
    expect(p.borderStyle).toBe('double')
    expect(p.borderColor).toBe('#0000FF') // 小写进、大写出
  })

  it('部分边 ⇒ 只勾该边；没有边 ⇒ 线型/色回落缺省', () => {
    const p = cellPropsFromEntry({ row: 1, col: 1, border: { bottom: { style: 'solid' } } })
    expect(p.sides).toEqual({ top: false, right: false, bottom: true, left: false })
    expect(p.borderStyle).toBe('solid')
    expect(p.borderColor).toBe('#000000')
  })

  it('format / rotation 读取；怪值回落缺省', () => {
    expect(cellPropsFromEntry({ row: 1, col: 1, format: '#,##0.00', rotation: 45 })).toMatchObject({
      format: '#,##0.00',
      rotation: 45,
    })
    expect(cellPropsFromEntry({ row: 1, col: 1, rotation: -5 }).rotation).toBe(0)
    expect(cellPropsFromEntry({ row: 1, col: 1, format: 123 as unknown as string }).format).toBe('')
  })

  it('cellPropsFromDef 取选区起点格的条目', () => {
    const def = defWith([{ row: 2, col: 2, format: '0.00' }])
    expect(cellPropsFromDef(def, R(1, 1)).format).toBe('0.00') // 逻辑 (2,2)
    expect(cellPropsFromDef(def, R(0, 0)).format).toBe('') // 逻辑 (1,1) 无条目
  })
})

// ---------------------------------------------------------------------------
// 2. 选项注入
// ---------------------------------------------------------------------------

describe('下拉选项', () => {
  it('线型：开放 4 项；手写 none 补进首位', () => {
    expect(borderStyleOptions('solid')).toEqual([...BORDER_LINE_STYLES])
    expect(borderStyleOptions('none')[0]).toBe('none')
  })

  it('格式：预设含空串（无格式）；手写串补进首位', () => {
    expect(formatOptions('')[0]).toBe('')
    expect(formatOptions('#,##0.00')[0]).toBe('')
    expect(formatOptions('yyyy-MM-dd')[0]).toBe('yyyy-MM-dd')
  })
})

// ---------------------------------------------------------------------------
// 3. 值归一
// ---------------------------------------------------------------------------

describe('值归一：UI 输入 → 契约值', () => {
  it('旋转角 0..360 整数；越界/非整数/空 ⇒ 报错（不夹取）', () => {
    expect(normalizeCellPropsEdit({ kind: 'rotation', value: '45' }).edit).toEqual({
      kind: 'rotation',
      value: 45,
    })
    expect(normalizeCellPropsEdit({ kind: 'rotation', value: 0 }).edit).toEqual({ kind: 'rotation', value: 0 })
    expect(normalizeCellPropsEdit({ kind: 'rotation', value: 360 }).edit).toEqual({ kind: 'rotation', value: 360 })
    expect(normalizeCellPropsEdit({ kind: 'rotation', value: 361 }).error).toContain('0..360')
    expect(normalizeCellPropsEdit({ kind: 'rotation', value: -1 }).error).toContain('0..360')
    expect(normalizeCellPropsEdit({ kind: 'rotation', value: '12.5' }).error).toContain('整数')
    expect(normalizeCellPropsEdit({ kind: 'rotation', value: '' }).error).toContain('不能为空')
  })

  it('颜色大写；非法报错', () => {
    expect(normalizeCellPropsEdit({ kind: 'borderColor', color: '#00ff00' }).edit).toEqual({
      kind: 'borderColor',
      color: '#00FF00',
    })
    expect(normalizeCellPropsEdit({ kind: 'borderColor', color: 'red' }).error).toContain('#RRGGBB')
  })

  it('线型限契约 5 值', () => {
    expect(normalizeCellPropsEdit({ kind: 'borderStyle', style: 'dashed' }).edit).toEqual({
      kind: 'borderStyle',
      style: 'dashed',
    })
    expect(normalizeCellPropsEdit({ kind: 'borderStyle', style: 'wavy' }).error).toContain('不在契约取值')
  })

  it('borderSide 缺省线型/色按 solid/#000000', () => {
    const r = normalizeCellPropsEdit({ kind: 'borderSide', side: 'top', on: true })
    expect(r.edit).toEqual({ kind: 'borderSide', side: 'top', on: true, style: 'solid', color: '#000000' })
  })
})

// ---------------------------------------------------------------------------
// 4. 逐键补丁
// ---------------------------------------------------------------------------

describe('applyCellPropsToRange：只动一个键', () => {
  it('开边 ⇒ 写该边；缺省色不写 color 键（缺省即删键同规）', () => {
    const r = applyCellPropsToRange(defWith(undefined), R(0, 0), {
      kind: 'borderSide',
      side: 'top',
      on: true,
      style: 'solid',
      color: '#000000',
    })
    expect(r.error).toBeUndefined()
    expect(r.def?.cells).toEqual([{ row: 1, col: 1, border: { top: { style: 'solid' } } }])
  })

  it('开边保留已存在的 width（JSON_ONLY 不被抹）', () => {
    const cell: CellDef = { row: 1, col: 1, border: { top: { style: 'solid', width: 2.5, color: '#FF0000' } } }
    const r = applyCellPropsToRange(defWith([cell]), R(0, 0), {
      kind: 'borderSide',
      side: 'top',
      on: true,
      style: 'dashed',
      color: '#00FF00',
    })
    expect(r.def?.cells?.[0].border).toEqual({ top: { style: 'dashed', color: '#00FF00', width: 2.5 } })
  })

  it('关边 ⇒ 删该边；四边删空 ⇒ 删 border 键；条目删空 ⇒ 整条移除', () => {
    const only: CellDef = { row: 1, col: 1, border: { top: { style: 'solid' } } }
    const r1 = applyCellPropsToRange(defWith([only]), R(0, 0), { kind: 'borderSide', side: 'top', on: false })
    expect(r1.def?.cells).toEqual([]) // 条目本来只有 border ⇒ 整条移除
    const two: CellDef = { row: 1, col: 1, border: { top: { style: 'solid' }, left: { style: 'solid' } } }
    const r2 = applyCellPropsToRange(defWith([two]), R(0, 0), { kind: 'borderSide', side: 'top', on: false })
    expect(r2.def?.cells?.[0].border).toEqual({ left: { style: 'solid' } })
  })

  it('关边保留条目里的其它键（text 等）', () => {
    const cell: CellDef = { row: 1, col: 1, text: 'x', border: { top: { style: 'solid' } } }
    const r = applyCellPropsToRange(defWith([cell]), R(0, 0), { kind: 'borderSide', side: 'top', on: false })
    expect(r.def?.cells).toEqual([{ row: 1, col: 1, text: 'x' }])
  })

  it('改线型 ⇒ 只改已存在的边，color/width 保留（不凭空给没有的边加边框）', () => {
    const cell: CellDef = {
      row: 1,
      col: 1,
      border: { top: { style: 'solid', color: '#FF0000', width: 2 }, left: { style: 'dotted' } },
    }
    const r = applyCellPropsToRange(defWith([cell]), R(0, 0), { kind: 'borderStyle', style: 'double' })
    expect(r.def?.cells?.[0].border).toEqual({
      top: { style: 'double', color: '#FF0000', width: 2 },
      left: { style: 'double' },
    })
  })

  it('改颜色 ⇒ 只改已存在的边；设为缺省 #000000 ⇒ color 键被删', () => {
    const cell: CellDef = { row: 1, col: 1, border: { top: { style: 'solid', color: '#00FF00' } } }
    const r = applyCellPropsToRange(defWith([cell]), R(0, 0), { kind: 'borderColor', color: '#000000' })
    expect(r.def?.cells?.[0].border).toEqual({ top: { style: 'solid' } })
  })

  it('整选区没有边框 ⇒ 改线型/色无改动 + 专用提示（不是「已是该值」）', () => {
    const r = applyCellPropsToRange(defWith([{ row: 1, col: 1, text: 'x' }]), R(0, 0), {
      kind: 'borderStyle',
      style: 'dashed',
    })
    expect(r.error).toBeUndefined()
    expect(r.warnings.join()).toContain('没有边框可改')
  })

  it('format 设/清；rotation 设/清', () => {
    const r1 = applyCellPropsToRange(defWith(undefined), R(0, 0), { kind: 'format', value: '#,##0.00' })
    expect(r1.def?.cells).toEqual([{ row: 1, col: 1, format: '#,##0.00' }])
    const r2 = applyCellPropsToRange(r1.def!, R(0, 0), { kind: 'format', value: '' })
    expect(r2.def?.cells).toEqual([]) // 条目只有 format ⇒ 删空后整条移除
    const r3 = applyCellPropsToRange(defWith(undefined), R(0, 0), { kind: 'rotation', value: 45 })
    expect(r3.def?.cells).toEqual([{ row: 1, col: 1, rotation: 45 }])
    const r4 = applyCellPropsToRange(r3.def!, R(0, 0), { kind: 'rotation', value: 0 })
    expect(r4.def?.cells).toEqual([])
  })

  it('未登记字段逐个原样保留（indent/extend_mode/text/expr/手写字段）', () => {
    const rich: CellDef = {
      row: 1,
      col: 1,
      text: 'T',
      indent: 2,
      extend_mode: 'horizontal',
      custom_prop: '手写',
      border: { top: { style: 'solid' } },
    }
    const r = applyCellPropsToRange(defWith([rich]), R(0, 0), { kind: 'format', value: '#,##0' })
    expect(r.def?.cells?.[0]).toEqual({ ...rich, format: '#,##0' })
  })

  it('幂等：同值重复打补丁 = 无改动', () => {
    const def = defWith([{ row: 1, col: 1, border: { top: { style: 'solid' } }, rotation: 30 }])
    const r = applyCellPropsToRange(def, R(0, 0), { kind: 'rotation', value: 30 })
    expect(r.def).toBe(def)
    expect(r.warnings.join()).toContain('无需改动')
    const r2 = applyCellPropsToRange(def, R(0, 0), {
      kind: 'borderSide',
      side: 'top',
      on: true,
      style: 'solid',
      color: '#000000',
    })
    expect(r2.def).toBe(def)
  })

  it('纯函数：入参 def 不被修改', () => {
    const def = defWith([{ row: 1, col: 1, border: { top: { style: 'solid' } } }])
    const before = clone(def)
    applyCellPropsToRange(def, R(0, 0, 3, 3), { kind: 'borderStyle', style: 'dashed' })
    expect(def).toEqual(before)
  })

  it('共用脚手架：越界报错 / 合并区跳过 / 多格落笔', () => {
    const bad = applyCellPropsToRange(defWith(undefined, 2, 2), R(5, 0), { kind: 'format', value: '0' })
    expect(bad.error).toContain('不覆盖网格')
    const merged = applyCellPropsToRange(
      defWith([{ row: 1, col: 1, merge: { rows: 2, cols: 2 } }]),
      R(0, 0, 2, 2),
      { kind: 'format', value: '0' },
    )
    expect(merged.def?.cells).toEqual([{ row: 1, col: 1, merge: { rows: 2, cols: 2 }, format: '0' }])
    const multi = applyCellPropsToRange(defWith(undefined), R(0, 0, 2, 1), { kind: 'format', value: '0' })
    expect(multi.def?.cells).toEqual([
      { row: 1, col: 1, format: '0' },
      { row: 2, col: 1, format: '0' },
    ])
  })
})

// ---------------------------------------------------------------------------
// 5. Univer 落地
// ---------------------------------------------------------------------------

describe('applyCellPropToRange：Univer 侧落地', () => {
  /** 边名/线型码与 Univer **运行时枚举**逐项对齐 —— 映射层把值以字面量钉在本地
   * （`cellprops.ts` 的 `BORDER_TYPE_OF` 与 `stylemap` 的码表），Univer 升级改了
   * 枚举值**不会报错、只会静默映射错**，所以必须取真枚举对照。 */
  it('边名与 BorderType 运行时值一致；线型码与 BorderStyleTypes 一致', () => {
    const core = univer as unknown as {
      BorderType: Record<string, string>
      BorderStyleTypes: Record<string, number>
    }
    expect(core.BorderType.TOP).toBe('top')
    expect(core.BorderType.RIGHT).toBe('right')
    expect(core.BorderType.BOTTOM).toBe('bottom')
    expect(core.BorderType.LEFT).toBe('left')
    expect(core.BorderStyleTypes.NONE).toBe(0)
    expect(core.BorderStyleTypes.THIN).toBe(1)
    expect(core.BorderStyleTypes.DOTTED).toBe(3)
    expect(core.BorderStyleTypes.DASHED).toBe(4)
    expect(core.BorderStyleTypes.DOUBLE).toBe(7)
  })

  const panel = {
    sides: { top: true, right: false, bottom: true, left: false },
    borderStyle: 'dashed' as const,
    borderColor: '#FF0000',
  }

  it('单边开 ⇒ setBorder(side, 线型码, 色)；关 ⇒ NONE 码', () => {
    const { calls, range } = fakeRange()
    applyCellPropToRange(range, { kind: 'borderSide', side: 'top', on: true, style: 'solid', color: '#000000' }, panel)
    applyCellPropToRange(range, { kind: 'borderSide', side: 'left', on: false, style: 'solid', color: '#000000' }, panel)
    expect(calls).toEqual([
      ['setBorder', ['top', 1, '#000000']], // THIN = 1
      ['setBorder', ['left', 0]], // NONE = 0
    ])
  })

  it('改线型 ⇒ 落到「开着的边」（top/bottom），色取面板值', () => {
    const { calls, range } = fakeRange()
    applyCellPropToRange(range, { kind: 'borderStyle', style: 'double' }, panel)
    expect(calls).toEqual([
      ['setBorder', ['top', 7, '#FF0000']], // DOUBLE = 7
      ['setBorder', ['bottom', 7, '#FF0000']],
    ])
  })

  it('改颜色 ⇒ 落到「开着的边」，线型取面板值', () => {
    const { calls, range } = fakeRange()
    applyCellPropToRange(range, { kind: 'borderColor', color: '#00FF00' }, panel)
    expect(calls).toEqual([
      ['setBorder', ['top', 4, '#00FF00']], // DASHED = 4
      ['setBorder', ['bottom', 4, '#00FF00']],
    ])
  })

  it('格式空串=清除也照传（Univer 侧 removeCells）；旋转原样', () => {
    const { calls, range } = fakeRange()
    applyCellPropToRange(range, { kind: 'format', value: '' }, panel)
    applyCellPropToRange(range, { kind: 'format', value: '#,##0.00' }, panel)
    applyCellPropToRange(range, { kind: 'rotation', value: 45 }, panel)
    expect(calls).toEqual([
      ['setNumberFormat', ['']],
      ['setNumberFormat', ['#,##0.00']],
      ['setTextRotation', [45]],
    ])
  })
})
