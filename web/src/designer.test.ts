/**
 * 编辑面单测（M7-W W2-1）。
 *
 * 重点钉**四条服务端契约**（写错一个字段就 400）与**坐标系**（差一格不报错、
 * 只是落到隔壁格 —— 最危险的形态）。坐标系期望值不是推导来的，是
 * `crates/rpx-server/src/snapshot/tests.rs` 里**已实证**的那组：
 * `engine_merge` 的逻辑 `North (2,1)纵2` → 快照 `startRow=1, startColumn=0,
 * endRow=2, endColumn=0`。
 *
 * ⚠ 与 `stylemap.test.ts` 同规：纯逻辑、不碰 DOM（`downloadDefJson` 除外，
 * 它故意不在此测）。
 */
import { describe, expect, it } from 'vitest'

import {
  applyValueEdit,
  assertEditable,
  coveredSlots,
  exportDefJson,
  findCellIndex,
  toLogical,
  toUniver,
  type ReportDef,
} from './designer'

/** 一个 3×2 网格、带一处 1×2 合并（锚点在逻辑 (1,1)）的定义。 */
function demo(): ReportDef {
  return {
    version: 1,
    grid: { rows: 3, cols: 2 },
    cells: [
      { row: 1, col: 1, text: '标题', merge: { rows: 1, cols: 2 }, h_align: 'center' },
      { row: 2, col: 1, text: 'Top N', h_align: 'right' },
      { row: 2, col: 2, expr: '@topN' },
      { row: 3, col: 1, text: '区域', h_align: 'right' },
      { row: 3, col: 2, expr: '@region' },
    ],
    row_props: [{ row: 1, height: 26 }],
    col_props: [
      { col: 1, width: 18 },
      { col: 2, width: 24 },
    ],
    params: [{ name: 'topN', data_type: 'int', default: '10' }],
  }
}

// ---------------------------------------------------------------------------
// 坐标系（约束 1 —— 差一格不报错，只是落到隔壁格）
// ---------------------------------------------------------------------------

describe('坐标系：Univer 0 起 ↔ 定义 JSON 1 起', () => {
  it('两轴各 +1 / −1，且互为逆', () => {
    expect(toLogical(0, 0)).toEqual({ row: 1, col: 1 })
    expect(toLogical(2, 1)).toEqual({ row: 3, col: 2 })
    expect(toUniver(1, 1)).toEqual({ row: 0, col: 0 })
    expect(toUniver(3, 2)).toEqual({ row: 2, col: 1 })
    for (const [r, c] of [
      [0, 0],
      [1, 0],
      [0, 1],
      [9, 4],
    ]) {
      expect(toUniver(toLogical(r, c).row, toLogical(r, c).col)).toEqual({ row: r, col: c })
    }
  })

  /**
   * **这是本条判据的核心**：用 `snapshot/tests.rs` 里已实证的那组期望值反查。
   * `engine_merge` 的 `North (2,1)纵2` 在快照里是
   * `startRow=1, startColumn=0, endRow=2, endColumn=0`。
   * ⇒ 若有人把 `toLogical` 写成「不减一」（即直接用 Univer 坐标当逻辑坐标），
   * 下面这组断言必红 —— 而**服务端不会报错**，只会把内容写到隔壁格。
   */
  it('锚点换算与 snapshot/tests.rs 实证的那组一致', () => {
    // engine_merge：逻辑 (2,1) 是 North 合并块的锚点
    expect(toLogical(1, 0)).toEqual({ row: 2, col: 1 })
    // 纵 2 ⇒ 覆盖逻辑 (2,1)(3,1) ⇒ 0 起 (1,0)(2,0)
    const covered = coveredSlots({
      grid: { rows: 10, cols: 10 },
      cells: [{ row: 2, col: 1, text: 'North', merge: { rows: 2, cols: 1 } }],
    })
    expect([...covered].sort()).toEqual(['3,1'])
    // 换算回 Univer：被覆盖的是 (2,0)，锚点是 (1,0)
    expect(toUniver(3, 1)).toEqual({ row: 2, col: 0 })
  })
})

// ---------------------------------------------------------------------------
// 约束 4：网格边界
// ---------------------------------------------------------------------------

describe('约束 4：越界显式报错', () => {
  it('超出 grid 行列 ⇒ error 且不产出 def', () => {
    const d = demo()
    const r = applyValueEdit(d, 3, 0, 'x') // 逻辑 (4,1)，grid 只有 3 行
    expect(r.error).toContain('超出网格')
    expect(r.def).toBeUndefined()
    const r2 = applyValueEdit(d, 0, 2, 'x') // 逻辑 (1,3)，grid 只有 2 列
    expect(r2.error).toContain('超出网格')
  })

  /**
   * ⚠ 边界格要挑**可编辑**的那种：`demo()` 的最后一行是 `@region` 表达式格
   * （会被约束 2 正确拦下），所以这里自建一个角上是纯文本的网格。
   */
  it('边界值（最后一行/列）合法', () => {
    const d: ReportDef = {
      grid: { rows: 3, cols: 2 },
      cells: [{ row: 3, col: 2, text: '角' }],
    }
    const r = applyValueEdit(d, 2, 1, '尾行') // 逻辑 (3,2) = 最后一行最后一列
    expect(r.error).toBeUndefined()
    expect(r.def?.cells?.[0]).toMatchObject({ row: 3, col: 2, text: '尾行' })
    // 再往外一格就必须报错
    expect(applyValueEdit(d, 3, 1, 'x').error).toContain('超出网格')
    expect(applyValueEdit(d, 2, 2, 'x').error).toContain('超出网格')
  })

  it('错误消息用 Univer 坐标（用户看见的是这个）', () => {
    const r = applyValueEdit(demo(), 5, 5, 'x')
    expect(r.error).toContain('(5,5)')
    expect(r.error).not.toContain('(6,6)')
  })
})

// ---------------------------------------------------------------------------
// 约束 3：合并矩形内的非锚点槽
// ---------------------------------------------------------------------------

describe('约束 3：非锚点槽不得带内容', () => {
  it('coveredSlots 只收非锚点（锚点自己不在集合里）', () => {
    const covered = coveredSlots(demo())
    // demo 的锚点是逻辑 (1,1)，矩形 1×2 ⇒ 覆盖 (1,2)
    expect([...covered]).toEqual(['1,2'])
  })

  it('声明顺序无关（非锚点槽先于锚点出现也算覆盖）', () => {
    const d: ReportDef = {
      grid: { rows: 4, cols: 4 },
      cells: [
        { row: 1, col: 2 }, // 空壳，先出现
        { row: 1, col: 1, text: 'T', merge: { rows: 1, cols: 2 } }, // 锚点，后出现
      ],
    }
    expect([...coveredSlots(d)]).toEqual(['1,2'])
  })

  it('往非锚点槽写内容 ⇒ error（服务端会 400）', () => {
    const r = applyValueEdit(demo(), 0, 1, '硬塞') // 逻辑 (1,2) = 被覆盖
    expect(r.error).toContain('合并矩形内部')
    expect(r.error).toContain('左上角')
    expect(r.def).toBeUndefined()
  })

  it('往锚点写内容合法', () => {
    const r = applyValueEdit(demo(), 0, 0, '新标题') // 逻辑 (1,1) = 锚点
    expect(r.error).toBeUndefined()
    expect(r.def?.cells?.[0]).toMatchObject({ row: 1, col: 1, text: '新标题' })
  })
})

// ---------------------------------------------------------------------------
// 约束 2：不得用 text 覆盖 expr
// ---------------------------------------------------------------------------

describe('约束 2：expr 格不许改内容', () => {
  it('改 expr 格 ⇒ error 且点明表达式', () => {
    const r = applyValueEdit(demo(), 1, 1, '123') // 逻辑 (2,2) = @topN
    expect(r.error).toContain('@topN')
    expect(r.error).toContain('切断数据绑定')
    expect(r.def).toBeUndefined()
  })

  it('expr 为空串不拦截（它不是真表达式）', () => {
    const d: ReportDef = {
      grid: { rows: 2, cols: 2 },
      cells: [{ row: 1, col: 1, expr: '' }],
    }
    const r = applyValueEdit(d, 0, 0, 'x')
    expect(r.error).toBeUndefined()
  })

  it('text 与 expr 原本都有 ⇒ 仍拦截（不猜哪个优先）', () => {
    const d: ReportDef = {
      grid: { rows: 2, cols: 2 },
      cells: [{ row: 1, col: 1, text: '旧', expr: '@p' }],
    }
    const r = applyValueEdit(d, 0, 0, '新')
    expect(r.error).toContain('@p')
  })
})

// ---------------------------------------------------------------------------
// 补丁语义
// ---------------------------------------------------------------------------

describe('applyValueEdit 补丁语义', () => {
  it('原对象不被修改（纯函数）', () => {
    const d = demo()
    const before = JSON.stringify(d)
    applyValueEdit(d, 1, 0, '改过')
    expect(JSON.stringify(d)).toBe(before)
  })

  it('只补这一格，其余 cells 与顶层字段原样保留', () => {
    const r = applyValueEdit(demo(), 1, 0, '改过') // 逻辑 (2,1)
    expect(r.def?.cells?.[1]).toMatchObject({ row: 2, col: 1, text: '改过', h_align: 'right' })
    expect(r.def?.cells?.[0]).toMatchObject({ row: 1, col: 1, text: '标题', merge: { rows: 1, cols: 2 } })
    expect(r.def?.row_props).toEqual([{ row: 1, height: 26 }])
    expect(r.def?.params).toEqual([{ name: 'topN', data_type: 'int', default: '10' }])
    expect(r.def?.grid).toEqual({ rows: 3, cols: 2 })
  })

  it('该格没有条目 ⇒ 追加（保持声明序在末尾）', () => {
    const d: ReportDef = { grid: { rows: 3, cols: 3 }, cells: [{ row: 1, col: 1, text: 'a' }] }
    const r = applyValueEdit(d, 1, 1, 'b') // 逻辑 (2,2)
    expect(r.def?.cells).toEqual([
      { row: 1, col: 1, text: 'a' },
      { row: 2, col: 2, text: 'b' },
    ])
  })

  it('清空 ⇒ 删 text 键（不写空串）', () => {
    const r = applyValueEdit(demo(), 1, 0, '') // 逻辑 (2,1) 原本 text:'Top N'
    const cell = r.def?.cells?.[1] as Record<string, unknown>
    expect('text' in cell).toBe(false)
    // 其余字段仍在（只删 text）
    expect(cell.h_align).toBe('right')
    expect(cell.row).toBe(2)
  })

  it('清空一个没有条目的格 ⇒ 不凭空造空壳', () => {
    const d: ReportDef = { grid: { rows: 3, cols: 3 }, cells: [{ row: 1, col: 1, text: 'a' }] }
    const r = applyValueEdit(d, 2, 2, null)
    expect(r.def?.cells).toEqual([{ row: 1, col: 1, text: 'a' }])
  })

  it('数值/布尔一律 String()（JSON 的 text 契约类型是字符串）', () => {
    const d: ReportDef = { grid: { rows: 3, cols: 3 } }
    expect(applyValueEdit(d, 0, 0, 100).def?.cells?.[0]).toEqual({ row: 1, col: 1, text: '100' })
    expect(applyValueEdit(d, 0, 0, true).def?.cells?.[0]).toEqual({ row: 1, col: 1, text: 'true' })
    expect(applyValueEdit(d, 0, 0, 3.5).def?.cells?.[0]).toEqual({ row: 1, col: 1, text: '3.5' })
  })

  it('幂等：同一改动打两次，产物不变', () => {
    const d = demo()
    const once = applyValueEdit(d, 1, 0, '改过').def as ReportDef
    const twice = applyValueEdit(once, 1, 0, '改过').def as ReportDef
    expect(twice).toEqual(once)
  })

  it('重复 (row,col) 条目 ⇒ 告警且只改第一条', () => {
    const d: ReportDef = {
      grid: { rows: 2, cols: 2 },
      cells: [
        { row: 1, col: 1, text: '一' },
        { row: 1, col: 1, text: '二' },
      ],
    }
    const r = applyValueEdit(d, 0, 0, '三')
    expect(r.warnings.join()).toContain('重复条目')
    expect(r.def?.cells?.[0].text).toBe('三')
    expect(r.def?.cells?.[1].text).toBe('二') // 第二条没动
  })

  it('无 cells[] 字段的定义也能追加（自动建数组）', () => {
    const r = applyValueEdit({ grid: { rows: 2, cols: 2 } }, 0, 0, 'x')
    expect(r.def?.cells).toEqual([{ row: 1, col: 1, text: 'x' }])
  })

  it('error 与 def 互斥（不出半成品）', () => {
    for (const args of [
      [9, 9, 'x'],
      [0, 1, 'x'],
      [1, 1, 'x'],
    ] as [number, number, string][]) {
      const r = applyValueEdit(demo(), args[0], args[1], args[2])
      expect(r.error).toBeTruthy()
      expect(r.def).toBeUndefined()
    }
  })
})

// ---------------------------------------------------------------------------
// 可编辑性边界
// ---------------------------------------------------------------------------

describe('assertEditable：.rpx 是能力边界，不是错误', () => {
  it('.json 可编辑', () => {
    expect(assertEditable('json')).toBeNull()
  })
  it('.rpx 给「为什么不能」的文案', () => {
    const msg = assertEditable('rpx')
    expect(msg).toContain('.rpx')
    expect(msg).toContain('只支持定义 JSON')
  })
  it('还没选文件也给文案', () => {
    expect(assertEditable(null)).toContain('还没有可编辑的报表')
  })
})

// ---------------------------------------------------------------------------
// 导出
// ---------------------------------------------------------------------------

describe('导出', () => {
  it('导出的是**打过补丁的** JSON，且可直接回读', () => {
    const patched = applyValueEdit(demo(), 1, 0, '改过').def as ReportDef
    const text = exportDefJson(patched)
    const back = JSON.parse(text) as ReportDef
    expect(back.cells?.[1]).toMatchObject({ row: 2, col: 1, text: '改过' })
    // 顶层字段一个不少
    expect(back.grid).toEqual({ rows: 3, cols: 2 })
    expect(back.row_props).toEqual([{ row: 1, height: 26 }])
    expect(back.params).toHaveLength(1)
  })

  it('导出不带末尾换行', () => {
    expect(exportDefJson(demo()).endsWith('\n')).toBe(false)
  })

  it('导出的 JSON 重新喂回 applyValueEdit 仍可继续编辑', () => {
    const once = applyValueEdit(demo(), 1, 0, 'A').def as ReportDef
    const reloaded = JSON.parse(exportDefJson(once)) as ReportDef
    const twice = applyValueEdit(reloaded, 2, 0, 'B').def as ReportDef
    expect(twice.cells?.find((c) => c.row === 3 && c.col === 1)).toMatchObject({ text: 'B' })
    expect(twice.cells?.find((c) => c.row === 2 && c.col === 1)).toMatchObject({ text: 'A' })
  })
})

// ---------------------------------------------------------------------------
// findCellIndex
// ---------------------------------------------------------------------------

describe('findCellIndex', () => {
  it('按逻辑坐标找下标', () => {
    const d = demo()
    expect(findCellIndex(d, 1, 1)).toBe(0)
    expect(findCellIndex(d, 2, 2)).toBe(2)
    expect(findCellIndex(d, 9, 9)).toBe(-1)
  })
  it('无 cells[] 不炸', () => {
    expect(findCellIndex({ grid: { rows: 1, cols: 1 } }, 1, 1)).toBe(-1)
  })
})
