/**
 * 报表设置面板单测（M7-W W3-1）。
 *
 * 判据分四组：
 * 1. **读方向**（`printSetupOf`）—— 未声明显示契约缺省（不是空）；类型不符回落缺省。
 * 2. **写方向逐键补丁**（`applyPrintSetup`）—— 只动 patch 的键；缺省即删键；
 *    删空后 `print_setup` 整键移除；未知键原样保留（服务端会拦，前端不拦不删）。
 * 3. **校验镜像服务端**（`printsetup.rs::apply_print_setup`）—— i16 档
 *    0..=32767、i32 档 0..=i32MAX、f32 ≥0、布尔类型；越界/非整/非有限数显式报错。
 * 4. **横竖互换**（`swapPaperOrientation`）+ 纯函数性。
 */
import { describe, expect, it } from 'vitest'

import type { ReportDef } from './designer'
import {
  PRINT_SETUP_DEFAULTS,
  applyPrintSetup,
  printSetupOf,
  swapPaperOrientation,
} from './reportsetup'

const baseDef = (): ReportDef => ({
  version: 1,
  grid: { rows: 3, cols: 2 },
  cells: [{ row: 1, col: 1, text: 'A' }],
})

const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T

// ---------------------------------------------------------------------------
// 1. 读方向
// ---------------------------------------------------------------------------

describe('读方向：契约缺省', () => {
  it('未声明 print_setup ⇒ 全契约缺省（210x297 / 边距 19,19,25,25 / 20 行每页…）', () => {
    const v = printSetupOf(baseDef())
    expect(v).toEqual({ ...PRINT_SETUP_DEFAULTS })
  })

  it('部分声明 ⇒ 其余仍缺省；数字/布尔各自回落', () => {
    const def: ReportDef = {
      ...baseDef(),
      print_setup: { row_num_per_page: 8, back_graph_printed: true },
    }
    const v = printSetupOf(def)
    expect(v.row_num_per_page).toBe(8)
    expect(v.back_graph_printed).toBe(true)
    expect(v.paper_width).toBe(210)
    expect(v.copy_count).toBe(1)
  })

  it('类型不符的字段回落缺省（不猜、不把字符串当数字显示）', () => {
    const def = { ...baseDef(), print_setup: { paper_width: '宽', copy_count: null } } as unknown as ReportDef
    const v = printSetupOf(def)
    expect(v.paper_width).toBe(210)
    expect(v.copy_count).toBe(1)
  })
})

// ---------------------------------------------------------------------------
// 2. 写方向
// ---------------------------------------------------------------------------

describe('applyPrintSetup：逐键补丁', () => {
  it('无声明 ⇒ 建 print_setup 且只含补丁键', () => {
    const r = applyPrintSetup(baseDef(), { row_num_per_page: 8 })
    expect(r.error).toBeUndefined()
    expect(r.def?.print_setup).toEqual({ row_num_per_page: 8 })
    expect(r.warnings.join()).toContain('渲染')
  })

  it('已有声明 ⇒ 只动补丁键，其余原样', () => {
    const def: ReportDef = { ...baseDef(), print_setup: { paper_width: 100, copy_count: 3 } }
    const r = applyPrintSetup(def, { paper_width: 120 })
    expect(r.def?.print_setup).toEqual({ paper_width: 120, copy_count: 3 })
  })

  it('值 = 契约缺省 ⇒ 删键（不留冗余）', () => {
    const def: ReportDef = { ...baseDef(), print_setup: { paper_width: 120, copy_count: 3 } }
    const r = applyPrintSetup(def, { paper_width: 210 }) // = 缺省
    expect(r.def?.print_setup).toEqual({ copy_count: 3 })
  })

  it('删空后 print_setup 整键移除（{} = 全缺省，与服务端口径一致）', () => {
    const def: ReportDef = { ...baseDef(), print_setup: { paper_width: 120 } }
    const r = applyPrintSetup(def, { paper_width: 210 })
    expect('print_setup' in (r.def as Record<string, unknown>)).toBe(false)
  })

  it('未知键原样保留（服务端 400 的锅服务端背，前端不删用户手写键）', () => {
    const def = { ...baseDef(), print_setup: { mystery: 1 } } as unknown as ReportDef
    const r = applyPrintSetup(def, { copy_count: 2 })
    expect(r.def?.print_setup).toEqual({ mystery: 1, copy_count: 2 })
  })

  it('布尔字段逐键补丁', () => {
    const r = applyPrintSetup(baseDef(), { full_screen_preview: true, blank_line_filled: true })
    expect(r.def?.print_setup).toEqual({ full_screen_preview: true, blank_line_filled: true })
    const r2 = applyPrintSetup({ ...baseDef(), ...r.def } as ReportDef, { full_screen_preview: false })
    expect(r2.def?.print_setup).toEqual({ blank_line_filled: true })
  })

  it('纯函数：输入 def 不被改写', () => {
    const def: ReportDef = { ...baseDef(), print_setup: { paper_width: 100 } }
    const snap = clone(def)
    applyPrintSetup(def, { paper_width: 150, copy_count: 2 })
    expect(def).toEqual(snap)
  })
})

// ---------------------------------------------------------------------------
// 3. 校验镜像服务端
// ---------------------------------------------------------------------------

describe('校验：三档规则镜像 printsetup.rs', () => {
  it('i16 档：0..=32767 整数（zoom/分栏/版面）', () => {
    expect(applyPrintSetup(baseDef(), { zoom_scale: 0 }).error).toBeUndefined()
    expect(applyPrintSetup(baseDef(), { zoom_scale: 32767 }).error).toBeUndefined()
    expect(applyPrintSetup(baseDef(), { zoom_scale: 32768 }).error).toContain('i16')
    expect(applyPrintSetup(baseDef(), { zoom_scale: -1 }).error).toContain('i16')
    expect(applyPrintSetup(baseDef(), { zoom_scale: 12.5 }).error).toContain('i16')
  })

  it('i32 档：每页行数/份数（0..=i32MAX）', () => {
    expect(applyPrintSetup(baseDef(), { row_num_per_page: 0 }).error).toBeUndefined()
    expect(applyPrintSetup(baseDef(), { row_num_per_page: 2147483647 }).error).toBeUndefined()
    expect(applyPrintSetup(baseDef(), { row_num_per_page: 2147483648 }).error).toContain('i32')
  })

  it('f32 档：纸张/边距非负', () => {
    expect(applyPrintSetup(baseDef(), { paper_width: 0 }).error).toBeUndefined()
    expect(applyPrintSetup(baseDef(), { paper_width: -1 }).error).toContain('负')
    expect(applyPrintSetup(baseDef(), { top_margin: -0.5 }).error).toContain('负')
  })

  it('非有限数（NaN/Infinity）显式报错', () => {
    expect(applyPrintSetup(baseDef(), { paper_width: NaN }).error).toContain('有限')
    expect(applyPrintSetup(baseDef(), { paper_width: Infinity }).error).toContain('有限')
  })

  it('多键补丁逐个校验，任一不过整单拒（不半成品）', () => {
    const r = applyPrintSetup(baseDef(), { paper_width: 100, copy_count: -1 })
    expect(r.error).toContain('copy_count')
    expect(r.def).toBeUndefined()
  })
})

// ---------------------------------------------------------------------------
// 4. 横竖互换 + 纯函数
// ---------------------------------------------------------------------------

describe('swapPaperOrientation', () => {
  it('宽高互换（含未声明时按缺省互换：210x297 → 297x210）', () => {
    const r = swapPaperOrientation(baseDef())
    expect(r.def?.print_setup).toEqual({ paper_width: 297, paper_height: 210 })
    const back = swapPaperOrientation({ ...baseDef(), ...r.def } as ReportDef)
    // 297x210 换回去 = 210x297 = 缺省 ⇒ 两键全删 ⇒ print_setup 整键移除（undefined）
    expect(back.def?.print_setup).toBeUndefined()
    expect('print_setup' in (back.def as Record<string, unknown>)).toBe(false)
  })

  it('自定义纸张正确互换', () => {
    const def: ReportDef = { ...baseDef(), print_setup: { paper_width: 100, paper_height: 50 } }
    const r = swapPaperOrientation(def)
    expect(r.def?.print_setup).toEqual({ paper_width: 50, paper_height: 100 })
  })
})
