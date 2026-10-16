/**
 * 样式映射层单测（M7-W W2-2a）。
 *
 * 三组判据，缺一不可：
 *
 * 1. **与 Univer 真实枚举对齐**（`alignsWithUniverRuntimeEnums`）—— 本模块把码表
 *    以字面量钉在本地（不 import 运行时枚举，理由见 `stylemap.ts` 文件头）。
 *    字面量会随 Univer 升级而过期，而这**不会报错、只会静默映射错**。
 *    本组从 `@univerjs/core` 取真枚举逐项对照 —— 升级改了码就红。
 * 2. **与服务端录到的真实快照对齐**（`matchesTheRecordedServerSnapshot`）——
 *    `tests/fixtures/out/engine_merge.snapshot.json` 是**服务端跑出来的**快照，
 *    不是手写期望值。拿它当 oracle 钉住「只出非缺省」这条不变量。
 * 3. **六条陷阱逐条钉住**（`traps`）—— 每条都对应一个「不改就会静默错」的形态。
 *
 * ⚠ 本文件**故意不 import 任何 React/DOM**：映射层是纯函数，进纯逻辑单测
 * （`docs/spec-m7-web.md` §4 W0-2 的口径）。
 */
import { describe, expect, it } from 'vitest'
import * as univer from '@univerjs/core'
import type { IStyleData } from '@univerjs/core'

import {
  DEFAULT_BACK_COLOR,
  DEFAULT_FONT_NAME,
  DEFAULT_FONT_SIZE,
  DEFAULT_FORE_COLOR,
  applyCellStyle,
  borderStyleFromCode,
  hAlignFromCode,
  hAlignToCode,
  jsonBorderToUniver,
  jsonToUniverStyle,
  normalizeColor,
  univerBorderToJson,
  univerStyleToJson,
  vAlignFromCode,
  vAlignToCode,
  type CellDef,
  type JsonStyleFields,
} from './stylemap'

/** 把 `IStyleData` 之外的键随便塞（测试里造畸形输入用）。 */
type Loose = Record<string, unknown>

// ---------------------------------------------------------------------------
// 1. 与 Univer 真实运行时枚举对齐
// ---------------------------------------------------------------------------

describe('码表与 Univer 真实枚举对齐', () => {
  /**
   * 这条测试存在的唯一理由：`stylemap.ts` 把码表写成字面量（不 import 运行时
   * 枚举），于是「Univer 升级改了枚举值」这个错误**没有任何编译器或运行时会报**，
   * 只会让映射静默错位。取真枚举逐项对照，把这个静默错误变成红灯。
   *
   * 判据有效性：把本文件里任一项期望值改错 → 本组必红（已故障注入验证）。
   */
  it('HorizontalAlign / VerticalAlign / BorderStyleTypes 逐项一致', () => {
    const core = univer as unknown as {
      HorizontalAlign: Record<string, number>
      VerticalAlign: Record<string, number>
      BorderStyleTypes: Record<string, number>
    }
    expect(core.HorizontalAlign).toMatchObject({
      UNSPECIFIED: 0,
      LEFT: 1,
      CENTER: 2,
      RIGHT: 3,
      JUSTIFIED: 4,
      BOTH: 5,
      DISTRIBUTED: 6,
    })
    expect(core.VerticalAlign).toMatchObject({ UNSPECIFIED: 0, TOP: 1, MIDDLE: 2, BOTTOM: 3 })
    expect(core.BorderStyleTypes).toMatchObject({
      NONE: 0,
      THIN: 1,
      HAIR: 2,
      DOTTED: 3,
      DASHED: 4,
      DASH_DOT: 5,
      DASH_DOT_DOT: 6,
      DOUBLE: 7,
      MEDIUM: 8,
      MEDIUM_DASHED: 9,
      MEDIUM_DASH_DOT: 10,
      MEDIUM_DASH_DOT_DOT: 11,
      SLANT_DASH_DOT: 12,
      THICK: 13,
    })
  })

  it('四条对齐函数与真实枚举的双向映射一致', () => {
    const core = univer as unknown as {
      HorizontalAlign: Record<string, number>
      VerticalAlign: Record<string, number>
    }
    expect(hAlignToCode('left')).toBe(core.HorizontalAlign.LEFT)
    expect(hAlignToCode('center')).toBe(core.HorizontalAlign.CENTER)
    expect(hAlignToCode('right')).toBe(core.HorizontalAlign.RIGHT)
    expect(hAlignToCode('justify')).toBe(core.HorizontalAlign.JUSTIFIED)
    expect(hAlignToCode('distribute')).toBe(core.HorizontalAlign.DISTRIBUTED)
    expect(vAlignToCode('top')).toBe(core.VerticalAlign.TOP)
    expect(vAlignToCode('middle')).toBe(core.VerticalAlign.MIDDLE)
    expect(vAlignToCode('bottom')).toBe(core.VerticalAlign.BOTTOM)
  })
})

// ---------------------------------------------------------------------------
// 2. 与服务端录到的真实快照对齐
// ---------------------------------------------------------------------------

describe('与服务端录到的真实快照对齐', () => {
  /**
   * `tests/fixtures/out/engine_merge.snapshot.json` 是服务端 `/render` 跑出来的
   * 快照（`IWorkbookData`）。这里取其中一格**原样**当 oracle：
   *
   * ```json
   * {"s":{"bg":{"rgb":"#FFFFFF"},"cl":{"rgb":"#000000"},"ff":"宋体","fs":12,
   *       "ht":1,"vt":2},"t":4,"v":"region"}
   * ```
   *
   * 它恒带 4 个缺省值（`ff`/`fs`/`cl`/`bg`）—— 正是「只出非缺省」这条不变量要
   * 防的东西：若实现改成全量输出，这一格会吐出 4 个它从没声明过的样式键。
   */
  const RECORDED: Loose = {
    bg: { rgb: '#FFFFFF' },
    cl: { rgb: '#000000' },
    ff: '宋体',
    fs: 12,
    ht: 1,
    vt: 2,
  }

  it('全缺省的录得样式 → 空 fields（不产生 4 个假样式键）', () => {
    const { fields, warnings } = univerStyleToJson(RECORDED as unknown as IStyleData)
    expect(fields).toEqual({})
    expect(warnings).toEqual([])
  })

  it('JSON 全缺省 → 复现录得的那份 IStyleData（写方向镜像服务端）', () => {
    // 与录得的快照**逐键相同**（不含 `t`/`v` —— 那两个是值不是样式）。
    expect(jsonToUniverStyle({})).toEqual(RECORDED)
  })

  /**
   * `s` **缺省**是真实状态：同一份快照里 `cellData[4][0]` 就是 `{"t":4,"v":"East"}`
   * （没有 `s`）⇒ 必须视作全缺省，而不是抛错或当成「全黑」。
   */
  it('无样式对象 → 视作全缺省', () => {
    expect(univerStyleToJson(undefined)).toEqual({ fields: {}, warnings: [] })
    expect(univerStyleToJson(null)).toEqual({ fields: {}, warnings: [] })
  })
})

// ---------------------------------------------------------------------------
// 3. 六条陷阱
// ---------------------------------------------------------------------------

describe('陷阱 1：对齐必须读原始数值，不能走门面', () => {
  /**
   * 门面 `transformCoreHorizontalAlignment` 把 `JUSTIFIED(4)`/`BOTH(5)`/
   * `DISTRIBUTED(6)`/`UNSPECIFIED(0)` **全部塌成 `'general'`**
   * （`@univerjs/sheets/lib/es/facade.js`：`default: return "general"`）。
   *
   * 本判据钉的是「4 与 6 必须能分辨」：走门面就分不出「两端对齐」和「未设置」，
   * 而 JSON 有 `justify`/`distribute` 两个槽 —— 塌成一个就是静默丢语义。
   */
  it('JUSTIFIED / DISTRIBUTED / UNSPECIFIED 三者可分辨', () => {
    expect(hAlignFromCode(4)).toBe('justify')
    expect(hAlignFromCode(6)).toBe('distribute')
    expect(hAlignFromCode(0)).toBeNull()
    // 门面给不出这两个值（会返回 'general'）—— 本函数给得出。
    const facade = (univer as unknown as { FHorizontalAlignment?: unknown }).FHorizontalAlignment
    expect(facade).toBeUndefined() // 门面对齐类型是纯类型，运行时不可用 ⇒ 更必须读数值
  })

  it('BOTH(5) 无 JSON 槽 → 报 warning 不静默丢', () => {
    const { fields, warnings } = univerStyleToJson({ ht: 5 } as unknown as IStyleData)
    expect(fields.h_align).toBeUndefined()
    expect(warnings.map((w) => w.key)).toContain('h_align')
  })

  it('left / middle 是缺省 ⇒ 不出字段', () => {
    // rpx 缺省 `h_align` = HALIGN_LEFT(-48) → `ht:1`；`v_align` = VALIGN_MIDDLE(-31) → `vt:2`。
    const { fields } = univerStyleToJson({ ht: 1, vt: 2 } as unknown as IStyleData)
    expect(fields).toEqual({})
  })
})

describe('陷阱 2：门面没有 right，normal 就是右对齐', () => {
  /**
   * `FHorizontalAlignment = 'left' | 'center' | 'normal'`，官方注释明写
   * 「`normal` means right alignment」；实测 `case "normal": return HorizontalAlign.RIGHT`。
   *
   * 本模块不走门面，但**UI 面板会**（Univer 的门面 setter 只收这三个值）。
   * 这条判据钉住映射关系本身，供 W2 面板取下拉框的值时对照 ——
   * 写成 `'right'` 传给门面会 `throw new Error('Invalid horizontal alignment')`。
   */
  it('数值 3 是右对齐，JSON 名是 right', () => {
    expect(hAlignFromCode(3)).toBe('right')
    expect(hAlignToCode('right')).toBe(3)
    expect(vAlignFromCode(3)).toBe('bottom') // 垂直方向 3 是 bottom，别和水平的搞混
  })
})

describe('陷阱 3：ul 是对象不是布尔', () => {
  it('ul.s === 1 → underline:true，且只取 s', () => {
    const { fields } = univerStyleToJson({ ul: { s: 1 } } as unknown as IStyleData)
    expect(fields.underline).toBe(true)
  })

  it('ul.s === 0 / ul 缺省 → 不写 underline', () => {
    expect(univerStyleToJson({ ul: { s: 0 } } as unknown as IStyleData).fields.underline).toBeUndefined()
    expect(univerStyleToJson({} as unknown as IStyleData).fields.underline).toBeUndefined()
  })

  it('ul 的 c/cl/t（颜色跟随字体 / 下划线色 / 线型）没有 JSON 槽 → warning', () => {
    const { fields, warnings } = univerStyleToJson({
      ul: { s: 1, c: 0, cl: { rgb: '#FF0000' }, t: 1 },
    } as unknown as IStyleData)
    expect(fields.underline).toBe(true)
    expect(warnings.map((w) => w.key)).toContain('underline')
    expect(warnings[0].detail).toContain('c/cl/t')
  })

  it('写方向产出 {s:1} 对象（与服务端快照同形）', () => {
    expect(jsonToUniverStyle({ underline: true })).toMatchObject({ ul: { s: 1 } })
  })
})

describe('陷阱 4：颜色是对象，且可能是主题色或 rgb() 形态', () => {
  it('#RRGGBB 对象 → 大写 #RRGGBB（服务端 argb_hex 出大写）', () => {
    const { fields } = univerStyleToJson({ cl: { rgb: '#ff8800' } } as unknown as IStyleData)
    expect(fields.fore_color).toBe('#FF8800')
  })

  it('rgb(r, g, b) 形态也归一（Univer 注释明说 rgb 可以是两种形态）', () => {
    const { fields, warnings } = univerStyleToJson({ cl: { rgb: 'rgb(255, 136, 0)' } } as unknown as IStyleData)
    expect(fields.fore_color).toBe('#FF8800')
    expect(warnings).toEqual([])
  })

  it('3 位 #RGB 展开成 6 位', () => {
    expect(normalizeColor('#f80')).toBe('#FF8800')
  })

  it('rgba() 带 alpha → 不认（JSON parse_color 只收 6 位）', () => {
    expect(normalizeColor('rgba(255,136,0,0.5)')).toBeNull()
    const { fields, warnings } = univerStyleToJson({ bg: { rgb: 'rgba(0,0,0,0.5)' } } as unknown as IStyleData)
    expect(fields.back_color).toBeUndefined()
    expect(warnings.map((w) => w.key)).toContain('back_color')
  })

  it('主题色 th → warning，不猜成黑色', () => {
    const { fields, warnings } = univerStyleToJson({ cl: { th: 3 } } as unknown as IStyleData)
    expect(fields.fore_color).toBeUndefined()
    expect(warnings.map((w) => w.key)).toContain('fore_color')
    expect(warnings[0].detail).toContain('主题色')
  })

  it('缺省色 #000000 / #FFFFFF 不写字段', () => {
    const { fields } = univerStyleToJson({
      cl: { rgb: DEFAULT_FORE_COLOR },
      bg: { rgb: DEFAULT_BACK_COLOR },
    } as unknown as IStyleData)
    expect(fields).toEqual({})
  })
})

describe('陷阱 5：边框线型码空间不同，只能查表', () => {
  it('Univer THIN(1) → JSON solid（不是平移，也不是同名）', () => {
    expect(borderStyleFromCode(1)).toBe('solid')
    expect(borderStyleFromCode(3)).toBe('dotted') // Univer 3 vs JSON dotted=81
    expect(borderStyleCodeOf('dotted')).toBe(3)
    expect(borderStyleFromCode(7)).toBe('double') // Univer 7 vs JSON double=84
  })

  it('Univer 14 种线型里只有 5 种有 JSON 槽，其余报 warning', () => {
    for (const code of [2, 5, 6, 8, 9, 10, 11, 12, 13]) {
      expect(borderStyleFromCode(code), `码 ${code} 应无 JSON 槽`).toBeNull()
    }
    const { border, warnings } = univerBorderToJson({ t: { s: 5, cl: { rgb: '#000000' } } })
    expect(border).toBeNull()
    expect(warnings.map((w) => w.key)).toContain('border.top.style')
  })

  it('写方向：solid → THIN(1)、double → DOUBLE(7)（查表而非同名）', () => {
    expect(jsonBorderToUniver({ top: { style: 'solid' } })).toMatchObject({ t: { s: 1 } })
    expect(jsonBorderToUniver({ top: { style: 'double' } })).toMatchObject({ t: { s: 7 } })
    expect(jsonBorderToUniver({ top: { style: 'dotted' } })).toMatchObject({ t: { s: 3 } })
  })

  it('Univer 的 t/r/b/l ↔ JSON 的 top/right/bottom/left（名字不同）', () => {
    const { border } = univerBorderToJson({
      t: { s: 1, cl: { rgb: '#FF0000' } },
      r: { s: 1, cl: { rgb: '#FF0000' } },
      b: { s: 1, cl: { rgb: '#FF0000' } },
      l: { s: 1, cl: { rgb: '#FF0000' } },
    })
    expect(border).toEqual({
      top: { style: 'solid', color: '#FF0000' },
      right: { style: 'solid', color: '#FF0000' },
      bottom: { style: 'solid', color: '#FF0000' },
      left: { style: 'solid', color: '#FF0000' },
    })
  })

  it('对角线边框（6 个槽）没有 JSON 槽 → warning', () => {
    const { warnings } = univerBorderToJson({ tl_br: { s: 1, cl: { rgb: '#000000' } } })
    expect(warnings.map((w) => w.key)).toContain('border')
    expect(warnings[0].detail).toContain('tl_br')
  })

  it('四边全无 → border:null（JSON 不写这个字段）', () => {
    expect(univerBorderToJson({}).border).toBeNull()
    expect(univerBorderToJson(undefined).border).toBeNull()
  })
})

describe('陷阱 6：IBorderStyleData 没有宽度槽，width 只能继承', () => {
  it('Univer 读回的边框不带 width ⇒ 从原 JSON 继承非缺省宽度', () => {
    const cell: CellDef = {
      row: 1,
      col: 1,
      border: { left: { style: 'solid', width: 2.5, color: '#FF0000' } },
    }
    const { cell: next } = applyCellStyle(cell, {
      bd: { l: { s: 1, cl: { rgb: '#FF0000' } } },
    } as unknown as IStyleData)
    expect(next.border).toEqual({ left: { style: 'solid', color: '#FF0000', width: 2.5 } })
  })

  it('原 JSON 宽度是缺省 1 ⇒ 不写 width（不塞噪音键）', () => {
    const cell: CellDef = { row: 1, col: 1, border: { left: { style: 'solid', width: 1 } } }
    const { cell: next } = applyCellStyle(cell, {
      bd: { l: { s: 1, cl: { rgb: '#000000' } } },
    } as unknown as IStyleData)
    expect(next.border).toEqual({ left: { style: 'solid' } })
  })
})

// ---------------------------------------------------------------------------
// 补丁语义
// ---------------------------------------------------------------------------

describe('applyCellStyle：补丁而非重建', () => {
  /** 一个有 indent / extend_mode / 手写未知字段的格子 —— 这些必须原样活着。 */
  const rich: CellDef = {
    row: 2,
    col: 3,
    text: '标题',
    h_align: 'center',
    indent: 2,
    extend_mode: 'horizontal',
    merge: { rows: 1, cols: 2 },
    my_own_field: '用户手写的',
  }

  it('快照里没有的槽（indent/extend_mode/未知字段）原样保留', () => {
    const { cell, warnings } = applyCellStyle(rich, { bl: 1 } as unknown as IStyleData)
    expect(cell.indent).toBe(2)
    expect(cell.extend_mode).toBe('horizontal')
    expect(cell.merge).toEqual({ rows: 1, cols: 2 })
    expect(cell.my_own_field).toBe('用户手写的')
    expect(cell.text).toBe('标题')
    expect(cell.row).toBe(2)
    expect(warnings).toEqual([])
  })

  it('原对象不被修改（纯函数）', () => {
    const snapshot = JSON.stringify(rich)
    applyCellStyle(rich, { bl: 1, fs: 20 } as unknown as IStyleData)
    expect(JSON.stringify(rich)).toBe(snapshot)
  })

  it('改回缺省会**删键**（加粗 → 撤销加粗不留 bold:true）', () => {
    const cell: CellDef = { row: 1, col: 1, bold: true, font_size: 20, fore_color: '#FF0000' }
    const { cell: next } = applyCellStyle(cell, {} as unknown as IStyleData)
    expect('bold' in next).toBe(false)
    expect('font_size' in next).toBe(false)
    expect('fore_color' in next).toBe(false)
  })

  it('幂等：对同一 Univer 状态重复打补丁，产物不变', () => {
    const style = {
      bl: 1,
      it: 1,
      ul: { s: 1 },
      ff: '黑体',
      fs: 16,
      cl: { rgb: '#FF0000' },
      bg: { rgb: '#FFFF00' },
      ht: 2,
      vt: 3,
      n: '#,##0.00',
      tr: { a: 45 },
      bd: { t: { s: 1, cl: { rgb: '#123456' } } },
    } as unknown as IStyleData
    const base: CellDef = { row: 1, col: 1, text: 'x' }
    const once = applyCellStyle(base, style).cell
    const twice = applyCellStyle(once, style).cell
    expect(twice).toEqual(once)
    // 顺带钉住产出确实落到了 JSON 上（判据不能只测「不变」）
    expect(once).toMatchObject({
      bold: true,
      italic: true,
      underline: true,
      font_name: '黑体',
      font_size: 16,
      fore_color: '#FF0000',
      back_color: '#FFFF00',
      h_align: 'center',
      v_align: 'bottom',
      format: '#,##0.00',
      rotation: 45,
    })
  })

  it('format / rotation 走 EXACT 往返', () => {
    const f: JsonStyleFields = { format: '#,##0.00', rotation: 45 }
    const read = univerStyleToJson({ n: '#,##0.00', tr: { a: 45 } } as unknown as IStyleData)
    expect(read.fields).toEqual(f)
  })

  it('rotation 为负 → warning 且不写（JSON 要求 ≥0）', () => {
    const { fields, warnings } = univerStyleToJson({ tr: { a: -45 } } as unknown as IStyleData)
    expect(fields.rotation).toBeUndefined()
    expect(warnings.map((w) => w.key)).toContain('rotation')
  })

  it('小数字号 → 取整 + warning', () => {
    const { fields, warnings } = univerStyleToJson({ fs: 10.5 } as unknown as IStyleData)
    expect(fields.font_size).toBe(11)
    expect(warnings.map((w) => w.key)).toContain('font_size')
  })

  it('字号越界 → warning 且不写', () => {
    const { fields, warnings } = univerStyleToJson({ fs: 0 } as unknown as IStyleData)
    expect(fields.font_size).toBeUndefined()
    expect(warnings.map((w) => w.key)).toContain('font_size')
    expect(univerStyleToJson({ fs: 99999 } as unknown as IStyleData).warnings.length).toBe(1)
  })

  it('eastAsiaFontFamily 没有 JSON 槽 → warning', () => {
    const { warnings } = univerStyleToJson({ eastAsiaFontFamily: 'SimSun' } as unknown as IStyleData)
    expect(warnings.map((w) => w.key)).toContain('eastAsiaFontFamily')
  })
})

describe('jsonToUniverStyle：镜像服务端 style_json', () => {
  it('全量字段 → 完整的 IStyleData（含恒发的 4 个缺省）', () => {
    expect(
      jsonToUniverStyle({
        font_name: '黑体',
        font_size: 16,
        bold: true,
        italic: true,
        underline: true,
        fore_color: '#FF0000',
        back_color: '#FFFF00',
        h_align: 'right',
        v_align: 'bottom',
      }),
    ).toEqual({
      ff: '黑体',
      fs: 16,
      bl: 1,
      it: 1,
      ul: { s: 1 },
      cl: { rgb: '#FF0000' },
      bg: { rgb: '#FFFF00' },
      ht: 3,
      vt: 3,
    })
  })

  it('bl/it/ul 只在开时发（服务端口径：不置位就不发键）', () => {
    const s = jsonToUniverStyle({ bold: false, italic: false, underline: false })
    expect('bl' in s).toBe(false)
    expect('it' in s).toBe(false)
    expect('ul' in s).toBe(false)
  })

  /**
   * ⚠ **本测试 2026-10-16（W2-2a）与初版实现一起改过**：初版断言「不认的枚举名
   * → 不发 `ht`/`vt` 键」，理由是「不猜，让 Univer 落自己的默认」。
   *
   * **那个理由是错的**，被 `engine_merge.snapshot.json` 这份录得的真实快照抓出来：
   * 服务端 `style_json` 读的是 `Cell.h_align`，而 rpx 的 `Cell` **永远有具体值**
   * （`blank_cell()` 给 `HALIGN_LEFT`/`VALIGN_MIDDLE`，从不是 `None`）⇒ 快照里
   * **每格都带** `"ht":1,"vt":2`。省掉这两个键就是让 Univer 用自己的缺省顶替
   * rpx 的缺省 —— 而那正是服务端注释「对齐总是输出」要防的视觉漂移。
   *
   * 正确口径：**恒发**，不认的枚举名退化成 rpx 缺省（`left`=1 / `middle`=2）。
   */
  it('不认的枚举名 → 退化成 rpx 缺省 left/middle（不是省掉键）', () => {
    const s = jsonToUniverStyle({ h_align: 'wat' as unknown as 'left', v_align: 'center' as unknown as 'top' })
    expect(s.ht).toBe(1)
    expect(s.vt).toBe(2)
  })

  it('往返闭合：jsonToUniverStyle ∘ univerStyleToJson 收敛到同一个 JSON', () => {
    // 注意方向：先 JSON → Univer → 回 JSON，应当拿回原样（缺省项被省略是允许的，
    // 因为它们等价于缺省）。
    const original: JsonStyleFields = {
      bold: true,
      font_name: '楷体',
      font_size: 14,
      fore_color: '#123456',
      h_align: 'justify',
    }
    const back = univerStyleToJson(jsonToUniverStyle(original) as unknown as IStyleData)
    expect(back.fields).toEqual(original)
    expect(back.warnings).toEqual([])
  })
})

describe('normalizeColor', () => {
  it('三种形态都归一成大写 6 位', () => {
    expect(normalizeColor('#FF8800')).toBe('#FF8800')
    expect(normalizeColor('#ff8800')).toBe('#FF8800')
    expect(normalizeColor('rgb(255, 136, 0)')).toBe('#FF8800')
    expect(normalizeColor('#F80')).toBe('#FF8800')
  })
  it('非法形态返回 null（调用方据此 warning）', () => {
    for (const bad of ['', 'red', '#GGGGGG', '#12345', '#1234567', 'rgba(1,2,3,1)', null, 42, {}]) {
      expect(normalizeColor(bad), `${JSON.stringify(bad)} 应被拒`).toBeNull()
    }
  })
})

describe('常量与 rpx 缺省一致', () => {
  it('四个缺省值就是 def.rs 的那几个', () => {
    expect(DEFAULT_FONT_NAME).toBe('宋体')
    expect(DEFAULT_FONT_SIZE).toBe(12)
    expect(DEFAULT_FORE_COLOR).toBe('#000000')
    expect(DEFAULT_BACK_COLOR).toBe('#FFFFFF')
  })
})

/**
 * 供「陷阱 5」用例取写方向的码。`jsonBorderToUniver` 返回的是 `IBorderData`
 * （键 `t/r/b/l`），这里只取 `top` 那一边的 `s`，避免在断言里重复解包。
 */
function borderStyleCodeOf(name: 'none' | 'dotted' | 'dashed' | 'solid' | 'double'): number {
  const bd = jsonBorderToUniver({ top: { style: name } }) as unknown as {
    t: { s: number }
  }
  return bd.t.s
}
