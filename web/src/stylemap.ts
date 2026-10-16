/**
 * 样式映射层：Univer `IStyleData`（短码）↔ 定义 JSON 的样式 / 属性字段。
 *
 * ## 为什么必须有这一层
 *
 * 两侧**码空间不同**，而且服务端快照是**有损**的 ——
 * `crates/rpx-server/src/snapshot/style.rs::style_json` 只暴露 9 个字段
 * （`bl/it/ul/ff/fs/cl/bg/ht/vt`），边框 / `format` / `indent` / `rotation` 不在其中
 * （`docs/spec-m4.md` §4.3 列为后置，判据走模型往返而非快照读回，
 * 见 `docs/report-def-json.md` §4.7 末段）。
 *
 * ⇒ 设计器**必须持有原始定义 JSON 并对它打补丁**（[`applyCellStyle]），**不能拿快照
 * 反推 JSON** —— 否则 `indent` / `extend_mode` / `border.width` / `print_setup` 会被
 * 静默抹掉。这是本模块存在的第一理由。
 *
 * ## 保真级别（每个字段归一类，读代码先看这个）
 *
 * | 级别 | 含义 | 字段 |
 * | --- | --- | --- |
 * | `EXACT` | 双向原样往返 | `font_name` `font_size` `bold` `italic` `underline` `fore_color` `back_color` `h_align` `v_align` `format` `rotation` `border.{style,color}` |
 * | `LOSSY` | 读得回、有信息损失（**会进 warnings**） | 小数字号、`ul` 的 `c`/`cl`/`t`、`tr.v`、`th` 主题色、对角边框、无 JSON 槽的线型 |
 * | `JSON_ONLY` | 只能写进 JSON，Univer 读不到 | `border.*.width` `indent` `extend_mode` `print_setup` |
 *
 * ## 六条实测陷阱（全部有出处，别凭记忆改）
 *
 * 1. **`ht` 必须读原始数值，不能用门面对齐 getter。** 门面
 *    `transformCoreHorizontalAlignment`（`@univerjs/sheets/lib/es/facade.js`）把
 *    `JUSTIFIED(4)`/`BOTH(5)`/`DISTRIBUTED(6)`/`UNSPECIFIED(0)` **全部塌成 `'general'`**
 *    （`default: return "general"`）⇒ 走 getter 就分不出「未设置」和「两端对齐」。
 *    直接读 `IStyleData.ht` 的数值则四者可分。
 * 2. **门面没有 `'right'`，`'normal'` 就是右对齐。**
 *    `FHorizontalAlignment = 'left' | 'center' | 'normal'`，且官方注释明写
 *    「`normal` means right alignment」（`facade/utils.d.ts`）；实测实现
 *    `case "normal": return HorizontalAlign.RIGHT`。本模块不走门面，故不踩这条，
 *    但**UI 下拉框若要显示「右对齐」，值必须是 `'normal'` 而不是 `'right'`**。
 * 3. **`ul` 是对象不是布尔。** `ITextDecoration { s: BooleanNumber, c?, cl?, t? }`
 *    （`i-style-data.d.ts`）—— 我们的 JSON / 快照里 `ul` 是 `{s:1}` 或布尔语义。
 *    只取 `s`，`c`/`cl`/`t`（下划线颜色跟随字体 / 线型）**没有 JSON 槽**。
 * 4. **颜色是对象不是字符串。** `IColorStyle { rgb?: string, th?: ThemeColorType }`。
 *    `rgb` 按 Univer 注释可以是 `#RRGGBB` **或** `rgb(r, g, b)` 两种形态，
 *    而 JSON 只收 6 位 `#RRGGBB`（`reportdef/style.rs::parse_color`）。
 * 5. **边框线型码空间完全不同。** Univer `BorderStyleTypes` 是 0..13 的稠密枚举，
 *    rpx 契约是 `none=0 / dotted=81 / dashed=82 / solid=83 / double=84`
 *    （`docs/report-def-json.md` §4.7）。`DOTTED=3` vs `dotted=81`、`DOUBLE=7` vs
 *    `double=84`、`THIN=1` vs `solid=83` —— **不是平移关系，只能查表**。
 *    且 Univer 有 14 种线型而 JSON 只开 5 种 ⇒ 其余 9 种**没有 JSON 槽**。
 * 6. **`IBorderStyleData` 没有宽度槽**（`BORDER_STYLE_KEYS = ["s","cl"]`）⇒
 *    JSON 的 `border.*.width` 是 `JSON_ONLY`：打补丁时**从原 JSON 继承**，
 *    不能当成「用户在 Univer 里改过」。
 *
 * 本模块只 `import type` Univer 的类型（构建期擦除），**不 import 它的运行时枚举**
 * —— 码表在本地以字面量钉住，`stylemap.test.ts` 另有「与 Univer 真实枚举对齐」
 * 的对照测试，Univer 升级改了码就会红（否则改码是静默的）。
 */
import type { IBorderData, IBorderStyleData, IColorStyle, IStyleData, ITextDecoration } from '@univerjs/core'

// ---------------------------------------------------------------------------
// 类型：定义 JSON 侧（`docs/report-def-json.md` §3.2）
// ---------------------------------------------------------------------------

/** `h_align` 枚举名（§4.3）。⚠ `v_align` 的同名取值故意不同名：垂直方向是
 * `top|middle|bottom`，**没有 `center`** —— 避免「写了个 center 不知道指哪个轴」。 */
export type HAlignName = 'left' | 'center' | 'right' | 'justify' | 'distribute'
export type VAlignName = 'top' | 'middle' | 'bottom'

/** `border.<side>.style` 枚举名（§4.7）。rpx 契约只开这 5 种。
 * `85`（'U'）刻意不开放：契约写明它与 `81` 同为 dotted，开两个名字对应同一视觉
 * 只会让人猜「到底哪个对」。 */
export type BorderStyleName = 'none' | 'dotted' | 'dashed' | 'solid' | 'double'
export type ExtendModeName = 'default' | 'none' | 'vertical' | 'horizontal'

export type JsonBorderSide = {
  style: BorderStyleName
  /** `JSON_ONLY`：Univer 无此槽（陷阱 6），打补丁时从原 JSON 继承。 */
  width?: number
  color?: string
}
export type JsonBorder = {
  left?: JsonBorderSide
  right?: JsonBorderSide
  top?: JsonBorderSide
  bottom?: JsonBorderSide
}

/** 本模块能打的补丁所覆盖的 JSON 字段（7 样式 + 2 对齐 + 边框 / format / rotation）。 */
export type JsonStyleFields = {
  h_align?: HAlignName
  v_align?: VAlignName
  font_name?: string
  font_size?: number
  bold?: boolean
  italic?: boolean
  underline?: boolean
  fore_color?: string
  back_color?: string
  border?: JsonBorder
  format?: string
  rotation?: number
}

/** 定义 JSON `cells[]` 的一条（W2 视角）。
 *
 * ⚠ 索引签名是**刻意**的：打补丁走整体展开 `{...cell}`，未在本模块登记的槽
 * （`text`/`expr`/`indent`/`extend_mode`/`merge`/用户手写的任何字段）必须**原样
 * 保留**。设计器吃掉用户手写字段 = 静默丢数据，是本仓纪律明确禁止的。 */
export type CellDef = JsonStyleFields & {
  row: number
  col: number
  text?: string
  expr?: string
  indent?: number
  extend_mode?: ExtendModeName
  merge?: { rows: number; cols: number }
  merge_same?: string
  [k: string]: unknown
}

/** 读 Univer 样式时「看到了但 JSON 装不下」的条目。
 *
 * 本模块的硬规矩：**宁可报 warning，不静默丢**。调用方（W2 面板）要把 warnings
 * 上屏 —— 丢掉的若是边框线型或主题色，用户完全看不出渲染会变。 */
export type PatchWarning = {
  /** 出问题的字段（用 JSON 路径口径，便于定位）。 */
  key: string
  detail: string
}
export type StyleReadResult = { fields: JsonStyleFields; warnings: PatchWarning[] }
export type CellPatchResult = { cell: CellDef; warnings: PatchWarning[] }

// ---------------------------------------------------------------------------
// 默认值（`crates/rpx-core/src/props.rs` 的 `def` 模块）
// ---------------------------------------------------------------------------

/** 缺省字体。`def::DEFAULT_FONT_NAME = "宋体"`。 */
export const DEFAULT_FONT_NAME = '宋体'
/** 缺省字号。`def::FONT_SIZE = 12`。 */
export const DEFAULT_FONT_SIZE = 12
/** 缺省前景色。`def::COLOR_BLACK = 0xFF000000` → `#000000`。 */
export const DEFAULT_FORE_COLOR = '#000000'
/** 缺省背景色。`def::COLOR_WHITE = 0xFFFFFFFF` → `#FFFFFF`。 */
export const DEFAULT_BACK_COLOR = '#FFFFFF'
/** 缺省左边框宽。`def::BORDER_WIDTH = 1.0`。 */
export const DEFAULT_BORDER_WIDTH = 1
/** 缺省边框色。`def::COLOR_BLACK`。 */
export const DEFAULT_BORDER_COLOR = '#000000'
/** 字号上下界。`reportdef/style.rs`：`1..=i16::MAX`，0 与越界都 400。 */
export const FONT_SIZE_MIN = 1
export const FONT_SIZE_MAX = 32767

// ---------------------------------------------------------------------------
// Univer 码表（本地字面量，出处见各常量注释；`stylemap.test.ts` 对照真实枚举）
// ---------------------------------------------------------------------------

/** `HorizontalAlign`（`@univerjs/core` `enum/text-style.d.ts`）。
 * 与 rpx 的 `h_align` 码一一对应：`h_align_code`（`snapshot/style.rs`）把
 * `-48/-47/-46/-45/-44` 映到 `1/2/3/4/6`。 */
export const UA_H = { UNSPECIFIED: 0, LEFT: 1, CENTER: 2, RIGHT: 3, JUSTIFIED: 4, BOTH: 5, DISTRIBUTED: 6 } as const
/** `VerticalAlign`。rpx `-32/-31/-30` → `1/2/3`。 */
export const UA_V = { UNSPECIFIED: 0, TOP: 1, MIDDLE: 2, BOTTOM: 3 } as const

/** `BorderStyleTypes`（`enum/border-style-types.d.ts`，14 种稠密码）。 */
export const U_BORDER_STYLE = {
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
} as const

/** `IBorderData` 的对角线槽（`i-style-data.d.ts`）—— JSON 只有四边，这 6 个
 * **没有槽**。Univer 的面板能画对角线，故必须显式报 warning 而不是吃掉。 */
export const U_DIAGONAL_BORDER_KEYS = ['tl_br', 'tl_bc', 'tl_mr', 'bl_tr', 'ml_tr', 'bc_tr'] as const

/** `IBorderData` 的四边槽 ↔ JSON 的 `border.<side>` 键。
 * ⚠ Univer 用 `t/r/b/l`，JSON 用 `top/right/bottom/left` —— **名字不同**，
 * 别照抄成 `border.t`。 */
const BD_TO_JSON_SIDE = { t: 'top', r: 'right', b: 'bottom', l: 'left' } as const
const JSON_SIDE_TO_BD = { top: 't', right: 'r', bottom: 'b', left: 'l' } as const
export type JsonBorderSideName = keyof typeof JSON_SIDE_TO_BD

// ---------------------------------------------------------------------------
// 颜色
// ---------------------------------------------------------------------------

/** 把 Univer 的颜色值归一成 JSON 的 `#RRGGBB`（大写）。
 *
 * 接受三种形态（陷阱 4）：`#RRGGBB`、`#RGB`（3 位展开）、`rgb(r, g, b)`。
 * 返回 `null` 表示**不认**（主题色 / rgba / 空值）—— 调用方据此报 warning。
 *
 * ⚠ 服务端 `argb_hex` 出的是**大写**十六进制，而 `parse_color` 大小写都收。
 * 这里统一出大写，与 `engine_merge.snapshot.json` 里录到的
 * `{"rgb":"#FFFFFF"}` 逐字一致 —— 大小写不一致会让快照对照测试假红。 */
export function normalizeColor(value: unknown): string | null {
  if (typeof value !== 'string') return null
  const s = value.trim()
  if (!s) return null
  const rgbFn = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+)\s*)?\)$/i.exec(s)
  if (rgbFn) {
    // `rgba(...)` 带 alpha ⇒ JSON 装不下（`parse_color` 只认 6 位十六进制）。
    if (rgbFn[4] !== undefined) return null
    const hex = rgbFn.slice(1, 4).map((n) => {
      const v = Number(n)
      if (v < 0 || v > 255) return null
      return v.toString(16).padStart(2, '0').toUpperCase()
    })
    return hex.includes(null) ? null : `#${hex.join('')}`
  }
  const body = s.startsWith('#') ? s.slice(1) : s
  if (body.length === 6 && /^[0-9a-fA-F]{6}$/.test(body)) return `#${body.toUpperCase()}`
  if (body.length === 3 && /^[0-9a-fA-F]{3}$/.test(body)) {
    return `#${body.toUpperCase().replace(/./g, (c) => c + c)}`
  }
  return null
}

/** 从 `IColorStyle` 里取 `#RRGGBB`；主题色 / 不认的形态返回 `null`。 */
export function colorOf(c: IColorStyle | null | undefined): string | null {
  if (!c) return null
  if (typeof c.rgb === 'string') return normalizeColor(c.rgb)
  return null
}

/** 该颜色是不是主题色（`th` 槽）—— 用来给出精确的 warning 文案。 */
function isThemeColor(c: IColorStyle | null | undefined): boolean {
  return !!c && c.th !== undefined && c.th !== null
}

// ---------------------------------------------------------------------------
// 对齐
// ---------------------------------------------------------------------------

/** Univer `ht` 数值 → JSON `h_align` 枚举名（§4.3 的反向）。
 *
 * ⚠ **读原始数值，不走门面**（陷阱 1）：`0`/`5` 落到 `null`（= JSON 缺省 `left`，
 * 因 `def::HALIGN_LEFT = -48` 映到 `ht:1`，两者视觉相同），而 `4`/`6`
 * （两端 / 分散）**必须能分辨** —— 门面把它们和「未设置」一起塌成 `'general'`。 */
export function hAlignFromCode(code: unknown): HAlignName | null {
  switch (code) {
    case UA_H.LEFT:
      return 'left'
    case UA_H.CENTER:
      return 'center'
    case UA_H.RIGHT:
      return 'right'
    case UA_H.JUSTIFIED:
      return 'justify'
    case UA_H.DISTRIBUTED:
      return 'distribute'
    default:
      return null
  }
}

/** Univer `vt` 数值 → JSON `v_align` 枚举名。⚠ 垂直方向叫 **`middle`**，不是 `center`。 */
export function vAlignFromCode(code: unknown): VAlignName | null {
  switch (code) {
    case UA_V.TOP:
      return 'top'
    case UA_V.MIDDLE:
      return 'middle'
    case UA_V.BOTTOM:
      return 'bottom'
    default:
      return null
  }
}

/** JSON `h_align` → Univer `ht` 数值。与 `snapshot/style.rs::h_align_code` 同表。
 * 返回 `null` = 不发这个字段（Univer 落自己的默认）。 */
export function hAlignToCode(name: string | undefined): number | null {
  switch (name) {
    case 'left':
      return UA_H.LEFT
    case 'center':
      return UA_H.CENTER
    case 'right':
      return UA_H.RIGHT
    case 'justify':
      return UA_H.JUSTIFIED
    case 'distribute':
      return UA_H.DISTRIBUTED
    default:
      return null
  }
}

/** JSON `v_align` → Univer `vt` 数值。与 `v_align_code` 同表。 */
export function vAlignToCode(name: string | undefined): number | null {
  switch (name) {
    case 'top':
      return UA_V.TOP
    case 'middle':
      return UA_V.MIDDLE
    case 'bottom':
      return UA_V.BOTTOM
    default:
      return null
  }
}

// ---------------------------------------------------------------------------
// 边框线型（陷阱 5：查表，不是平移）
// ---------------------------------------------------------------------------

/** rpx 线型名 → Univer `BorderStyleTypes` 码。出处 `docs/report-def-json.md` §4.7。 */
const JSON_STYLE_TO_UB: Record<BorderStyleName, number> = {
  none: U_BORDER_STYLE.NONE,
  dotted: U_BORDER_STYLE.DOTTED,
  dashed: U_BORDER_STYLE.DASHED,
  solid: U_BORDER_STYLE.THIN,
  double: U_BORDER_STYLE.DOUBLE,
}

/** Univer 码 → rpx 线型名；`null` = 该线型 JSON 没有槽（14 种里只开 5 种）。 */
export function borderStyleFromCode(code: unknown): BorderStyleName | null {
  switch (code) {
    case U_BORDER_STYLE.NONE:
      return 'none'
    case U_BORDER_STYLE.THIN:
      return 'solid'
    case U_BORDER_STYLE.DOTTED:
      return 'dotted'
    case U_BORDER_STYLE.DASHED:
      return 'dashed'
    case U_BORDER_STYLE.DOUBLE:
      return 'double'
    default:
      return null
  }
}

/** Univer 线型码 → 可读名字（只用于 warning 文案，让用户知道丢的是什么）。 */
function borderStyleLabel(code: unknown): string {
  const hit = Object.entries(U_BORDER_STYLE).find(([, v]) => v === code)
  return hit ? hit[0] : `未知码 ${String(code)}`
}

// ---------------------------------------------------------------------------
// 读方向：Univer `IStyleData` → JSON 非缺省子集
// ---------------------------------------------------------------------------

/** Univer 样式 → JSON 字段（**只出非缺省的**）。
 *
 * 「只出非缺省」与 Rust `apply_style` 的「给了才改，不给不动」严格对偶，也是补丁
 * **幂等**的来源：对同一状态重复打补丁，产物不变（`stylemap.test.ts` 钉住）。
 *
 * 为什么不能全量出：快照对 `ff`/`fs`/`cl`/`bg` **恒发缺省值**
 * （实测 `engine_merge.snapshot.json` 每格都带 `{"ff":"宋体","fs":12,
 * "cl":{"rgb":"#000000"},"bg":{"rgb":"#FFFFFF"}}`），全量出会把每个被碰过的格子
 * 都塞进 4 个它从没声明过的样式键。
 *
 * @param style 该格的内联样式；`undefined` = Univer 里没有样式对象
 *   （快照里 `cellData[4][0]` 就没有 `s`）⇒ 视作全缺省 ⇒ 返回空 `fields`。
 */
export function univerStyleToJson(style: IStyleData | undefined | null): StyleReadResult {
  const fields: JsonStyleFields = {}
  const warnings: PatchWarning[] = []
  if (!style) return { fields, warnings }
  const s = style as IStyleData & Record<string, unknown>

  // --- 字体 ---
  if (typeof s.ff === 'string' && s.ff !== DEFAULT_FONT_NAME) fields.font_name = s.ff
  // `eastAsiaFontFamily` 是第二个字体槽（`IStyleBase`），快照从不发它 ⇒ 无 JSON 槽。
  if (typeof s.eastAsiaFontFamily === 'string' && s.eastAsiaFontFamily) {
    warnings.push({
      key: 'eastAsiaFontFamily',
      detail: `中日韩字体 "${s.eastAsiaFontFamily}" 在定义 JSON 里没有对应槽（只有 font_name），未写入`,
    })
  }

  // --- 字号（陷阱：小数字号 JSON 只收整数）---
  if (typeof s.fs === 'number' && Number.isFinite(s.fs)) {
    if (!Number.isInteger(s.fs)) {
      const rounded = Math.round(s.fs)
      if (rounded >= FONT_SIZE_MIN && rounded <= FONT_SIZE_MAX) {
        fields.font_size = rounded
      }
      warnings.push({
        key: 'font_size',
        detail: `Univer 字号 ${s.fs} 是小数，JSON 只收整数，已取整为 ${rounded}`,
      })
    } else if (s.fs !== DEFAULT_FONT_SIZE) {
      if (s.fs < FONT_SIZE_MIN || s.fs > FONT_SIZE_MAX) {
        warnings.push({
          key: 'font_size',
          detail: `字号 ${s.fs} 越界（JSON 要求 ${FONT_SIZE_MIN}..=${FONT_SIZE_MAX}），已按缺省 ${DEFAULT_FONT_SIZE} 处理`,
        })
      } else {
        fields.font_size = s.fs
      }
    }
  }

  // --- 三个位标志（`bl`/`it` 缺省即关；只有 1 才置位）---
  if (s.bl === 1) fields.bold = true
  if (s.it === 1) fields.italic = true

  // --- 下划线（陷阱 3：`ul` 是对象）---
  const ul = s.ul as ITextDecoration | null | undefined
  if (ul && typeof ul === 'object') {
    if (ul.s === 1) fields.underline = true
    // `c`（颜色跟随字体）/ `cl`（下划线颜色）/ `t`（线型）都没有 JSON 槽。
    const extra = (['c', 'cl', 't'] as const).filter((k) => ul[k] !== undefined && ul[k] !== null)
    if (extra.length > 0) {
      warnings.push({
        key: 'underline',
        detail: `下划线的 ${extra.join('/')}（颜色跟随字体 / 下划线颜色 / 线型）在定义 JSON 里没有槽，未写入`,
      })
    }
  }

  // --- 颜色（陷阱 4：对象；可能带 `th` 主题色）---
  const fg = colorOf(s.cl as IColorStyle | null | undefined)
  if (isThemeColor(s.cl as IColorStyle | null | undefined)) {
    warnings.push({ key: 'fore_color', detail: '前景色是主题色（th），JSON 只收 #RRGGBB，未写入' })
  } else if (fg === null && s.cl) {
    warnings.push({ key: 'fore_color', detail: `前景色 ${JSON.stringify(s.cl)} 无法归一成 #RRGGBB，未写入` })
  } else if (fg !== null && fg !== DEFAULT_FORE_COLOR) {
    fields.fore_color = fg
  }
  const bgc = colorOf(s.bg as IColorStyle | null | undefined)
  if (isThemeColor(s.bg as IColorStyle | null | undefined)) {
    warnings.push({ key: 'back_color', detail: '背景色是主题色（th），JSON 只收 #RRGGBB，未写入' })
  } else if (bgc === null && s.bg) {
    warnings.push({ key: 'back_color', detail: `背景色 ${JSON.stringify(s.bg)} 无法归一成 #RRGGBB，未写入` })
  } else if (bgc !== null && bgc !== DEFAULT_BACK_COLOR) {
    fields.back_color = bgc
  }

  // --- 对齐（陷阱 1：读原始数值）---
  const hName = hAlignFromCode(s.ht)
  if (hName && hName !== 'left') fields.h_align = hName
  if (hName === null && s.ht !== undefined && s.ht !== null && s.ht !== UA_H.UNSPECIFIED) {
    warnings.push({
      key: 'h_align',
      detail: `水平对齐码 ${String(s.ht)} 在 JSON 枚举（left/center/right/justify/distribute）里没有对应项，未写入`,
    })
  }
  const vName = vAlignFromCode(s.vt)
  if (vName && vName !== 'middle') fields.v_align = vName
  if (vName === null && s.vt !== undefined && s.vt !== null && s.vt !== UA_V.UNSPECIFIED) {
    warnings.push({
      key: 'v_align',
      detail: `垂直对齐码 ${String(s.vt)} 在 JSON 枚举（top/middle/bottom）里没有对应项，未写入`,
    })
  }

  // --- 数字格式 ---
  if (typeof s.n === 'string' && s.n) fields.format = s.n

  // --- 旋转 ---
  const tr = s.tr as { a?: number; v?: unknown } | null | undefined
  if (tr && typeof tr === 'object') {
    if (typeof tr.a === 'number' && Number.isFinite(tr.a)) {
      if (tr.a < 0) {
        warnings.push({ key: 'rotation', detail: `旋转角 ${tr.a} 为负，JSON 要求 ≥0，已按缺省 0 处理` })
      } else if (tr.a !== 0) {
        fields.rotation = tr.a
      }
    }
    // `tr.v` = 竖排文字（0/1），JSON 无槽。
    if (tr.v !== undefined && tr.v !== null && tr.v !== 0) {
      warnings.push({ key: 'rotation', detail: 'Univer 的「竖排文字」在定义 JSON 里没有槽，未写入' })
    }
  }

  return { fields, warnings }
}

// ---------------------------------------------------------------------------
// 读方向：边框
// ---------------------------------------------------------------------------

/** Univer `IBorderData` → JSON `border` 对象；四边都没有 ⇒ `null`（= 不写这个字段）。
 *
 * 语义是**整体替换**而不是逐边合并：JSON 的 `border` 四条边都可选、缺省即无边框
 * （`cellprops.rs` 逐边判 `if let Some`），所以「Univer 里只有上边框」就该产出
 * `{top: …}` 而不是把旧的 `left` 留下。整体替换之所以安全，前提是**读回的
 * Univer 状态是完整的**（JSON 早已载入 Univer，边框在格子里）——
 * 这条前提由 `applyCellStyle` 的调用时序保证，别在别处单独调本函数。
 *
 * @param widths 原 JSON 各边的 `width`，用于继承（陷阱 6：Univer 无宽度槽）。 */
export function univerBorderToJson(
  bd: IBorderData | null | undefined,
  widths?: Partial<Record<JsonBorderSideName, number>>,
): { border: JsonBorder | null; warnings: PatchWarning[] } {
  const warnings: PatchWarning[] = []
  if (!bd) return { border: null, warnings }
  const b = bd as IBorderData & Record<string, unknown>
  const out: JsonBorder = {}

  for (const [bdKey, side] of Object.entries(BD_TO_JSON_SIDE) as [keyof typeof BD_TO_JSON_SIDE, JsonBorderSideName][]) {
    const edge = b[bdKey] as IBorderStyleData | null | undefined
    if (!edge) continue
    const name = borderStyleFromCode(edge.s)
    if (name === null) {
      warnings.push({
        key: `border.${side}.style`,
        detail: `线型 ${borderStyleLabel(edge.s)} 在 JSON 的 5 种线型（none/dotted/dashed/solid/double）里没有对应项，该边未写入`,
      })
      continue
    }
    const color = colorOf(edge.cl)
    const sideDef: JsonBorderSide = { style: name }
    // 宽度只能继承（陷阱 6）。
    const inherited = widths?.[side]
    if (inherited !== undefined && inherited !== DEFAULT_BORDER_WIDTH) sideDef.width = inherited
    if (color !== null) {
      if (color !== DEFAULT_BORDER_COLOR) sideDef.color = color
    } else if (isThemeColor(edge.cl)) {
      warnings.push({ key: `border.${side}.color`, detail: '边框色是主题色（th），JSON 只收 #RRGGBB，已按缺省 #000000 处理' })
    }
    out[side] = sideDef
  }

  // 对角线边框：Univer 有 6 个槽，JSON 一个都没有。
  const diag = U_DIAGONAL_BORDER_KEYS.filter((k) => b[k] !== undefined && b[k] !== null)
  if (diag.length > 0) {
    warnings.push({
      key: 'border',
      detail: `对角线边框（${diag.join('/')}）在定义 JSON 里没有槽，未写入`,
    })
  }

  return { border: Object.keys(out).length > 0 ? out : null, warnings }
}

// ---------------------------------------------------------------------------
// 写方向：JSON → Univer `IStyleData`（镜像服务端 `style_json`）
// ---------------------------------------------------------------------------

/** JSON 样式字段 → Univer 内联样式。**镜像 `snapshot/style.rs::style_json`**：
 * `ff`/`fs`/`cl`/`bg` 恒发（含缺省），`bl`/`it`/`ul` 只在开时发，`ht`/`vt` **恒发**。
 *
 * 为什么必须镜像而不是「只发用户改过的」：Univer 的缺省与 rpx 的缺省**不同**
 * （rpx 默认垂直居中，`def::VALIGN_MIDDLE`）⇒ 省略即视觉漂移。
 * 服务端注释原话：「**对齐总是输出**」。
 *
 * ⚠ **`ht`/`vt` 无条件发**，JSON 没声明也发 `1`/`2`（= `left`/`middle`）。
 * 这不是冗余：服务端 `style_json` 读的是 `Cell.h_align`，而 rpx 的 `Cell`
 * **永远有具体值**（`blank_cell()` 给 `HALIGN_LEFT = -48` / `VALIGN_MIDDLE = -31`，
 * 从不是 `None`）⇒ `h_align_code` 恒返回 `Some(1)`，`v_align_code` 恒返回 `Some(2)`。
 * 判据是 `tests/fixtures/out/engine_merge.snapshot.json` 里**录到的真实快照**：
 * 每一格都带 `"ht":1,"vt":2`。**这条不变量正是被那份录得快照抓出来的** ——
 * 初版实现只在 JSON 声明了对齐时才发，快照对照测试当场红。 */
export function jsonToUniverStyle(f: JsonStyleFields): IStyleData {
  const s: Record<string, unknown> = {
    ff: f.font_name ?? DEFAULT_FONT_NAME,
    fs: f.font_size ?? DEFAULT_FONT_SIZE,
    cl: { rgb: f.fore_color ?? DEFAULT_FORE_COLOR },
    bg: { rgb: f.back_color ?? DEFAULT_BACK_COLOR },
  }
  if (f.bold) s.bl = 1
  if (f.italic) s.it = 1
  if (f.underline) s.ul = { s: 1 }
  // 恒发：不认的枚举名退化成 rpx 缺省（left/middle），与 `blank_cell()` 同值。
  s.ht = hAlignToCode(f.h_align) ?? UA_H.LEFT
  s.vt = vAlignToCode(f.v_align) ?? UA_V.MIDDLE
  return s as unknown as IStyleData
}

/** JSON `border` → Univer `IBorderData`；全空返回 `null`。 */
export function jsonBorderToUniver(border: JsonBorder | undefined): IBorderData | null {
  if (!border) return null
  const out: Record<string, IBorderStyleData> = {}
  for (const [jsonSide, bdKey] of Object.entries(JSON_SIDE_TO_BD) as [JsonBorderSideName, 't' | 'r' | 'b' | 'l'][]) {
    const side = border[jsonSide]
    if (!side) continue
    out[bdKey] = {
      s: JSON_STYLE_TO_UB[side.style],
      cl: { rgb: side.color ?? DEFAULT_BORDER_COLOR },
    }
  }
  return Object.keys(out).length > 0 ? (out as unknown as IBorderData) : null
}

// ---------------------------------------------------------------------------
// 打补丁：把 Univer 的当前状态落到定义 JSON 的格子上
// ---------------------------------------------------------------------------

/** 把 Univer 里这一格的样式**补丁**进定义 JSON 的格子，返回**新**对象（纯函数）。
 *
 * 为什么是补丁而不是「用快照重建格子」：快照有损（见文件头）⇒ 重建会抹掉
 * `indent` / `extend_mode` / `border.width` / `print_setup` / 用户手写的任何
 * 未登记字段。补丁只动 [`JsonStyleFields]` 覆盖的那 12 个槽，其余**原样保留**
 * （`CellDef` 的索引签名 + 整体展开就是为此）。
 *
 * 宽度的继承见 [`univerBorderToJson] 的 `widths` 参数（陷阱 6）。
 *
 * @param cell    原始定义 JSON 格子（不会被修改）
 * @param style   Univer 里这一格的内联样式（`undefined` = 无样式 ⇒ 清空样式槽） */
export function applyCellStyle(cell: CellDef, style: IStyleData | undefined | null): CellPatchResult {
  const { fields, warnings } = univerStyleToJson(style)

  // 7 样式槽 + 2 对齐槽 + format / rotation：按「非缺省才写」整体覆盖。
  // 恒写（哪怕 fields 里没有）是为了**清除**用户刚在 Univer 里改回缺省的槽 ——
  // 只写不删会让「加粗 → 撤销加粗」在 JSON 里留下 bold:true。
  const next: CellDef = { ...cell }
  for (const key of [
    'font_name',
    'font_size',
    'bold',
    'italic',
    'underline',
    'fore_color',
    'back_color',
    'h_align',
    'v_align',
    'format',
    'rotation',
  ] as const) {
    if (fields[key] === undefined) delete next[key]
    else next[key] = fields[key] as never
  }

  // 边框：整体替换（语义见 univerBorderToJson），宽度从原 JSON 继承。
  const widths: Partial<Record<JsonBorderSideName, number>> = {}
  for (const side of ['left', 'right', 'top', 'bottom'] as JsonBorderSideName[]) {
    const w = cell.border?.[side]?.width
    if (typeof w === 'number') widths[side] = w
  }
  const { border, warnings: bw } = univerBorderToJson(
    (style as (IStyleData & { bd?: IBorderData }) | undefined | null)?.bd,
    widths,
  )
  warnings.push(...bw)
  if (border === null) delete next.border
  else next.border = border

  return { cell: next, warnings }
}
