/**
 * 样式面板单测（M7-W W2-2）。
 *
 * 判据分六组：
 *
 * 1. **读方向**（`panelFromUniverStyle`）—— 面板显示态从 Univer 活态读；
 *    与录得快照对齐（`tests/fixtures/out/engine_merge.snapshot.json` 里每格
 *    恒带 4 缺省 + `ht/vt`，进面板必须显示为**全缺省**，不是"被改过"）。
 * 2. **值归一**（`normalizePanelValue`）—— UI 输入 → 契约值；颜色统一**大写**
 *    （服务端 `argb_hex` 出大写，混用会产生假差异）、字号界 `1..=32767`。
 * 3. **逐字段补丁**（`applyPanelToRange`）—— 本里程碑最核心的不变量：
 *    只动用户改的那**一个**键，`expr` / `indent` / `extend_mode` / `border` /
 *    用户手写字段**逐个原样保留**（快照有损 ⇒ 不能整体覆盖，见 `stylepanel.ts`
 *    文件头）。
 * 4. **边界**（越界裁剪 / 全出网格报错 / 合并区跳过与全合并报错 / 槽数上限）。
 * 5. **恢复缺省**（`clearPanelRange`）—— 7 键全删、其余不动。
 * 6. **Univer 落地**（`applyFieldToRange` / `resetRangeToDefaults`）——
 *    ⚠ 关闭位样式传 `'normal'` / `'none'` 而不是 `false`（`f-range.d.ts:24-26`）。
 *
 * ⚠ 本文件不 import React/DOM：面板逻辑层是纯函数（G9 的 vitest 口径）。
 */
import { describe, expect, it } from 'vitest'
import type { IStyleData } from '@univerjs/core'

import type { ReportDef } from './designer'
import type { CellDef } from './stylemap'
import {
  MAX_PANEL_SLOTS,
  PANEL_DEFAULTS,
  applyFieldToRange,
  applyPanelToRange,
  clearPanelRange,
  fontOptions,
  normalizePanelValue,
  panelFromUniverStyle,
  resetRangeToDefaults,
  type PanelRange,
  type PanelRangeLike,
} from './stylepanel'

const R = (row0: number, col0: number, height = 1, width = 1): PanelRange => ({
  row0,
  col0,
  height,
  width,
})

const defWith = (cells: CellDef[] | undefined, rows = 3, cols = 3): ReportDef => ({
  grid: { rows, cols },
  ...(cells ? { cells } : {}),
})

/** 深快照，用于「纯函数不改入参」的断言。 */
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T

/** 记录调用的假 FRange（第 6 组用）。 */
function fakeRange(): { calls: [string, unknown][]; range: PanelRangeLike } {
  const calls: [string, unknown][] = []
  const rec =
    (name: string) =>
    (v: unknown): void => {
      calls.push([name, v])
    }
  const range = {
    setFontFamily: rec('setFontFamily'),
    setFontSize: rec('setFontSize'),
    setFontWeight: rec('setFontWeight'),
    setFontStyle: rec('setFontStyle'),
    setFontLine: rec('setFontLine'),
    setFontColor: rec('setFontColor'),
    setBackgroundColor: rec('setBackgroundColor'),
  }
  return { calls, range: range as PanelRangeLike }
}

// ---------------------------------------------------------------------------
// 1. 读方向
// ---------------------------------------------------------------------------

describe('读方向：Univer 活态 → 面板显示态', () => {
  /** 与服务端**录得快照**同形的对象（`engine_merge.snapshot.json` 实测每格都带
   * 这四个缺省 + `ht/vt`）——进面板必须显示为全缺省，否则「选中任意一格」
   * 都会看到 4 个假样式值。 */
  const RECORDED = {
    bg: { rgb: '#FFFFFF' },
    cl: { rgb: '#000000' },
    ff: '宋体',
    fs: 12,
    ht: 1,
    vt: 2,
  }

  it('录得样式（恒带缺省值）→ 显示全缺省', () => {
    expect(panelFromUniverStyle(RECORDED as unknown as IStyleData)).toEqual(PANEL_DEFAULTS)
  })

  it('无样式对象 ⇒ 视作全缺省', () => {
    expect(panelFromUniverStyle(undefined)).toEqual(PANEL_DEFAULTS)
    expect(panelFromUniverStyle(null)).toEqual(PANEL_DEFAULTS)
  })

  it('bl/it 数值 1 才置位；ul 是对象、只认 s === 1', () => {
    const p = panelFromUniverStyle({ bl: 1, it: 1, ul: { s: 1 } } as unknown as IStyleData)
    expect(p.bold).toBe(true)
    expect(p.italic).toBe(true)
    expect(p.underline).toBe(true)
    // 畸形输入：把 ul 写成布尔（非真实形态）不得当成下划线。
    const bad = panelFromUniverStyle({ ul: true } as unknown as IStyleData)
    expect(bad.underline).toBe(false)
    expect(panelFromUniverStyle({ ul: { s: 0 } } as unknown as IStyleData).underline).toBe(false)
  })

  it('小数字号取整、越界夹回 1..=32767', () => {
    expect(panelFromUniverStyle({ fs: 12.6 } as unknown as IStyleData).fontSize).toBe(13)
    expect(panelFromUniverStyle({ fs: 0 } as unknown as IStyleData).fontSize).toBe(1)
    expect(panelFromUniverStyle({ fs: 99999 } as unknown as IStyleData).fontSize).toBe(32767)
  })

  it('颜色归一成大写；主题色（th）回落缺省', () => {
    const p = panelFromUniverStyle({
      cl: { rgb: '#ff0000' },
      bg: { th: 1 },
    } as unknown as IStyleData)
    expect(p.foreColor).toBe('#FF0000')
    expect(p.backColor).toBe('#FFFFFF')
  })
})

describe('字体下拉选项', () => {
  it('当前值在常用表里 ⇒ 不重复追加', () => {
    const opts = fontOptions('宋体')
    expect(opts[0]).toBe('宋体')
    expect(opts.filter((f) => f === '宋体')).toHaveLength(1)
  })

  it('当前值不在常用表里 ⇒ 补进首位（否则 select 静默错显）', () => {
    expect(fontOptions('华文琥珀')[0]).toBe('华文琥珀')
  })

  it('空当前值 ⇒ 只有常用项', () => {
    expect(fontOptions('')).not.toContain('')
  })
})

// ---------------------------------------------------------------------------
// 2. 值归一化
// ---------------------------------------------------------------------------

describe('值归一化：UI 输入 → 契约值', () => {
  it('字体名非空即可', () => {
    expect(normalizePanelValue('fontName', '黑体').value).toBe('黑体')
    expect(normalizePanelValue('fontName', '   ').error).toContain('不能为空')
  })

  it('字号必须是 1..=32767 的整数', () => {
    expect(normalizePanelValue('fontSize', '18').value).toBe(18)
    expect(normalizePanelValue('fontSize', 12).value).toBe(12)
    expect(normalizePanelValue('fontSize', '12.5').error).toContain('整数')
    expect(normalizePanelValue('fontSize', 0).error).toContain('越界')
    expect(normalizePanelValue('fontSize', 32768).error).toContain('越界')
    expect(normalizePanelValue('fontSize', '').error).toContain('不能为空')
    expect(normalizePanelValue('fontSize', 'abc').error).toContain('整数')
  })

  it('位标志必须是布尔', () => {
    expect(normalizePanelValue('bold', true).value).toBe(true)
    expect(normalizePanelValue('italic', 'yes').error).toContain('布尔')
  })

  it('颜色统一出大写 #RRGGBB（与服务端 argb_hex 同形）', () => {
    expect(normalizePanelValue('foreColor', '#ff0000').value).toBe('#FF0000')
    expect(normalizePanelValue('backColor', '#f00').value).toBe('#FF0000')
    expect(normalizePanelValue('foreColor', 'rgb(255, 0, 0)').value).toBe('#FF0000')
    // rgba 带 alpha / 命名色 都不认 ⇒ 显式报错，不猜。
    expect(normalizePanelValue('foreColor', 'rgba(255,0,0,0.5)').error).toContain('#RRGGBB')
    expect(normalizePanelValue('foreColor', 'red').error).toContain('#RRGGBB')
  })
})

// ---------------------------------------------------------------------------
// 3. 逐字段补丁（核心不变量）
// ---------------------------------------------------------------------------

describe('applyPanelToRange：只动一个键', () => {
  it('空 cells + 非缺省值 ⇒ 追加一条只含该键的条目', () => {
    const r = applyPanelToRange(defWith(undefined), R(0, 0), 'bold', true)
    expect(r.error).toBeUndefined()
    expect(r.def?.cells).toEqual([{ row: 1, col: 1, bold: true }])
    expect(r.normalized).toBe(true)
  })

  it('已有条目 ⇒ 在原条目上改这一个键，text 原样保留', () => {
    const r = applyPanelToRange(defWith([{ row: 1, col: 1, text: '合计' }]), R(0, 0), 'fontSize', 18)
    expect(r.def?.cells?.[0]).toEqual({ row: 1, col: 1, text: '合计', font_size: 18 })
  })

  it('expr 驱动的格可以改样式（样式与内容互斥纪律无关）', () => {
    const cell: CellDef = { row: 1, col: 1, expr: 'A1+B1' }
    const r = applyPanelToRange(defWith([cell]), R(0, 0), 'bold', true)
    expect(r.error).toBeUndefined()
    expect(r.def?.cells?.[0]).toEqual({ row: 1, col: 1, expr: 'A1+B1', bold: true })
  })

  /** ⭐ 本里程碑的地基判据：快照没有 `indent` / `extend_mode` / `border` /
   * `format` / 用户手写字段 —— 整体覆盖会把它们抹掉，逐字段补丁必须原样保留。 */
  it('未登记字段（indent/extend_mode/border/format/手写字段）逐个原样保留', () => {
    const rich: CellDef = {
      row: 1,
      col: 1,
      text: '标题',
      indent: 2,
      extend_mode: 'horizontal',
      merge: { rows: 1, cols: 2 },
      border: { top: { style: 'solid', width: 2, color: '#0000FF' } },
      format: '#,##0.00',
      rotation: 45,
      custom_prop: '用户手写',
    }
    const r = applyPanelToRange(defWith([rich]), R(0, 0), 'bold', true)
    expect(r.def?.cells?.[0]).toEqual({ ...rich, bold: true })
  })

  it('改回缺省 ⇒ 删键（不写 bold:false / font_size:12 这类冗余）', () => {
    const cell: CellDef = { row: 1, col: 1, text: 'x', bold: true, font_size: 18 }
    const r1 = applyPanelToRange(defWith([cell]), R(0, 0), 'bold', false)
    expect(r1.def?.cells?.[0]).toEqual({ row: 1, col: 1, text: 'x', font_size: 18 })
    const r2 = applyPanelToRange(defWith([cell]), R(0, 0), 'fontSize', 12)
    expect(r2.def?.cells?.[0]).toEqual({ row: 1, col: 1, text: 'x', bold: true })
  })

  it('缺省值 + 无条目 ⇒ 不凭空造空壳（无改动 + warning）', () => {
    const def = defWith(undefined)
    const r = applyPanelToRange(def, R(0, 0), 'bold', false)
    expect(r.error).toBeUndefined()
    expect(r.def).toBe(def) // 原样返回（引用不变）
    expect(r.def?.cells).toBeUndefined()
    expect(r.warnings.join()).toContain('无需改动')
  })

  it('颜色进 JSON 是大写', () => {
    const r = applyPanelToRange(defWith([{ row: 1, col: 1, text: 'x' }]), R(0, 0), 'foreColor', '#00ff00')
    expect(r.def?.cells?.[0].fore_color).toBe('#00FF00')
  })

  it('多格选区：范围内每格都落笔', () => {
    const r = applyPanelToRange(defWith(undefined), R(0, 0, 2, 2), 'italic', true)
    expect(r.def?.cells).toEqual([
      { row: 1, col: 1, italic: true },
      { row: 1, col: 2, italic: true },
      { row: 2, col: 1, italic: true },
      { row: 2, col: 2, italic: true },
    ])
  })

  it('纯函数：入参 def 不被修改', () => {
    const def = defWith([{ row: 1, col: 1, text: 'x', bold: true }])
    const before = clone(def)
    applyPanelToRange(def, R(0, 0, 3, 3), 'bold', false)
    expect(def).toEqual(before)
  })

  it('重复 (row,col) 条目 ⇒ 只改第一条并告警', () => {
    const def = defWith([
      { row: 1, col: 1, text: 'a' },
      { row: 1, col: 1, text: 'b' },
    ])
    const r = applyPanelToRange(def, R(0, 0), 'bold', true)
    expect(r.def?.cells?.[0]).toEqual({ row: 1, col: 1, text: 'a', bold: true })
    expect(r.def?.cells?.[1]).toEqual({ row: 1, col: 1, text: 'b' })
    expect(r.warnings.join()).toContain('重复条目')
  })
})

// ---------------------------------------------------------------------------
// 4. 边界
// ---------------------------------------------------------------------------

describe('边界：越界 / 合并 / 上限', () => {
  it('选区完全在网格外 ⇒ 报错（不是静默无改动）', () => {
    const r = applyPanelToRange(defWith(undefined, 2, 2), R(5, 0, 1, 1), 'bold', true)
    expect(r.error).toContain('不覆盖网格')
  })

  it('选区越出网格 ⇒ 裁剪后仍落笔（Univer 画布比网格大）', () => {
    // grid 2×2；选区从 (0,0) 到 (0,0)+1×3（3 列）⇒ 裁剪到 2 列。
    const r = applyPanelToRange(defWith(undefined, 2, 2), R(0, 0, 1, 3), 'bold', true)
    expect(r.def?.cells).toEqual([
      { row: 1, col: 1, bold: true },
      { row: 1, col: 2, bold: true },
    ])
  })

  it('合并矩形内（非锚点）槽被跳过，锚点照落笔', () => {
    // (1,1) 锚点，2 行 2 列 ⇒ (1,2)/(2,1)/(2,2) 是覆盖槽。
    const def = defWith([{ row: 1, col: 1, merge: { rows: 2, cols: 2 } }])
    const r = applyPanelToRange(def, R(0, 0, 2, 2), 'bold', true)
    expect(r.error).toBeUndefined()
    expect(r.def?.cells).toEqual([{ row: 1, col: 1, merge: { rows: 2, cols: 2 }, bold: true }])
  })

  it('选区全部落在合并区内 ⇒ 报错指路锚点', () => {
    const def = defWith([{ row: 1, col: 1, merge: { rows: 2, cols: 2 } }])
    const r = applyPanelToRange(def, R(1, 1, 1, 1), 'bold', true) // 逻辑 (2,2)：覆盖槽
    expect(r.error).toContain('合并矩形内部')
  })

  it(`槽数超上限（>${MAX_PANEL_SLOTS}）⇒ 报错，不静默截断`, () => {
    const r = applyPanelToRange(defWith(undefined, 30, 30), R(0, 0, 30, 30), 'bold', true)
    expect(r.error).toContain('超过上限')
  })
})

// ---------------------------------------------------------------------------
// 5. 恢复缺省
// ---------------------------------------------------------------------------

describe('clearPanelRange：7 键全删、其余不动', () => {
  it('7 个样式键全删，未登记字段保留', () => {
    const cell: CellDef = {
      row: 1,
      col: 1,
      text: 'x',
      font_name: '黑体',
      font_size: 20,
      bold: true,
      italic: true,
      underline: true,
      fore_color: '#FF0000',
      back_color: '#FFFF00',
      indent: 1,
      format: '#,##0.00',
    }
    const r = clearPanelRange(defWith([cell]), R(0, 0))
    expect(r.error).toBeUndefined()
    expect(r.def?.cells?.[0]).toEqual({
      row: 1,
      col: 1,
      text: 'x',
      indent: 1,
      format: '#,##0.00',
    })
  })

  it('已是缺省 ⇒ 无改动 + 合并成一句提示', () => {
    const def = defWith([{ row: 1, col: 1, text: 'x' }])
    const r = clearPanelRange(def, R(0, 0))
    expect(r.def).toBe(def)
    expect(r.warnings).toEqual(['所选范围已是缺省样式（无需改动）'])
  })

  it('合并区报错口径与单字段一致', () => {
    const def = defWith([{ row: 1, col: 1, merge: { rows: 2, cols: 2 } }])
    expect(clearPanelRange(def, R(1, 1)).error).toContain('合并矩形内部')
  })
})

// ---------------------------------------------------------------------------
// 6. Univer 落地
// ---------------------------------------------------------------------------

describe('applyFieldToRange：Univer 侧落地', () => {
  it('关闭位样式传 normal/none 而不是 false（字符串枚举）', () => {
    const { calls, range } = fakeRange()
    applyFieldToRange(range, 'bold', false)
    applyFieldToRange(range, 'italic', false)
    applyFieldToRange(range, 'underline', false)
    expect(calls).toEqual([
      ['setFontWeight', 'normal'],
      ['setFontStyle', 'normal'],
      ['setFontLine', 'none'],
    ])
  })

  it('打开位样式传 bold/italic/underline', () => {
    const { calls, range } = fakeRange()
    applyFieldToRange(range, 'bold', true)
    applyFieldToRange(range, 'italic', true)
    applyFieldToRange(range, 'underline', true)
    expect(calls).toEqual([
      ['setFontWeight', 'bold'],
      ['setFontStyle', 'italic'],
      ['setFontLine', 'underline'],
    ])
  })

  it('字体 / 字号 / 颜色字段各自落到对应 setter', () => {
    const { calls, range } = fakeRange()
    applyFieldToRange(range, 'fontName', '黑体')
    applyFieldToRange(range, 'fontSize', 18)
    applyFieldToRange(range, 'foreColor', '#FF0000')
    applyFieldToRange(range, 'backColor', '#FFFF00')
    expect(calls).toEqual([
      ['setFontFamily', '黑体'],
      ['setFontSize', 18],
      ['setFontColor', '#FF0000'],
      ['setBackgroundColor', '#FFFF00'],
    ])
  })

  it('恢复缺省 ⇒ 7 个 setter 按固定序落缺省值', () => {
    const { calls, range } = fakeRange()
    resetRangeToDefaults(range)
    expect(calls).toEqual([
      ['setFontFamily', '宋体'],
      ['setFontSize', 12],
      ['setFontWeight', 'normal'],
      ['setFontStyle', 'normal'],
      ['setFontLine', 'none'],
      ['setFontColor', '#000000'],
      ['setBackgroundColor', '#FFFFFF'],
    ])
  })
})
