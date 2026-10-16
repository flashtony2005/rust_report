/**
 * 单元格属性面板（M7-W W2-3）：边框 / 数字格式 / 旋转的**逐键补丁**模型。
 *
 * ## 与 W2-2 的关键差异：读方向在**定义 JSON**，不在 Univer
 *
 * W2-2 的 7 个样式字段在快照里（`style_json` 发 `ff/fs/bl/it/ul/cl/bg/ht/vt`），
 * 所以那层面板从 Univer 活态读、看的是画布真相。
 * W2-3 这三项**不在快照里** —— `docs/report-def-json.md` §4.7 末段明写
 * 「边框 / `format` / `indent` / `rotation` / 打印设置不在其中」⇒ 画布上
 * **永远是空的**，从 Univer 根本读不回来。⇒ 面板显示态一律从**持有的定义
 * JSON** 读（[`cellPropsFromEntry]）：JSON 是这三项的唯一真理来源。
 *
 * 写方向与 W2-2 同规：只动用户改的那**一个**键，其余键（含边内 `width` ——
 * `JSON_ONLY`，`stylemap.ts` 陷阱 6）逐个原样保留；缺省即删键。
 * Univer 侧同步落地（[`applyCellPropToRange]）只是**视觉反馈**（画布即时可见），
 * **不**参与往返判据 —— 重渲染后画布这三项会回到空白（快照有损的固有结果）。
 *
 * ## 语义要点（全部有契约出处）
 *
 * - 边框四边 `border.{top,right,bottom,left}`；边内 `style` **必填**、
 *   `width`/`color` 可选（`reportdef/cellprops.rs::parse_side`）。
 *   面板用**边开关**表达存在性（勾选 = 该边存在）；`style:"none"` **不进下拉**
 *   —— 一种状态一种写法（`none` 与「没这一边」视觉相同，开两个名字只会让人猜）。
 * - 「线型 / 颜色」改动**只作用于已存在的边**（不凭空给没边框的边加边框）；
 *   逐边改动只动 `style`/`color` 一个键，`width` 保留。
 * - `format`：字符串、缺省无格式（§4.7）；清空 ⇒ 删键。
 * - `rotation`：数字 ≥0（`cellprops.rs:79-86`，负值 400）；面板只开放 **0..360 整数**，
 *   `0` ⇒ 删键。超出手工 JSON 会写出的值不在面板范围（**不夹取**，越界显式报错）。
 * - 条目删空（只剩 `row`/`col`）⇒ 整条移除（不残留空壳；W2-1 同一条纪律）。
 */
import type { BorderType } from '@univerjs/core'

import { findCellIndex, toLogical, type ReportDef } from './designer'
import { finishRangePatch, patchRangeSlots, type PanelRange } from './stylepanel'
import {
  DEFAULT_BORDER_COLOR,
  DEFAULT_BORDER_WIDTH,
  borderStyleToUniverCode,
  normalizeColor,
  U_BORDER_STYLE,
  type BorderStyleName,
  type CellDef,
  type JsonBorder,
  type JsonBorderSide,
} from './stylemap'

// ---------------------------------------------------------------------------
// 面板模型
// ---------------------------------------------------------------------------

export type BorderSideName = 'top' | 'right' | 'bottom' | 'left'

/** 四边的固定遍历序（读「第一条存在的边」与落 Univer 都用它）。 */
export const BORDER_SIDES = ['top', 'right', 'bottom', 'left'] as const satisfies readonly BorderSideName[]

/** 面板线型下拉的开放项（4 种实线型）。`none` 不进面板：存在性由边开关表达。 */
export const BORDER_LINE_STYLES = ['dotted', 'dashed', 'solid', 'double'] as const

/** 契约允许的全部线型（含 `none` —— 手写 JSON 可能带它，归一/读回要认）。 */
const BORDER_STYLE_NAMES: readonly BorderStyleName[] = ['none', 'dotted', 'dashed', 'solid', 'double']

/** 数字格式预设（值 == 写进 JSON 的 `format` 字符串；`''` = 无格式）。
 * 例 `#,##0.00` 即契约 §3.2 给的样例。 */
export const FORMAT_PRESETS = ['', '#,##0', '#,##0.00', '0', '0.00', '0%', '0.00%'] as const

/** 面板显示态（全部来自定义 JSON 的选区起点格）。 */
export type CellPropsPanel = {
  /** 每边是否存在（勾选态）。 */
  sides: Record<BorderSideName, boolean>
  /** 线型控件值（`BORDER_LINE_STYLES` 之一，或 JSON 手写的 `none`）。 */
  borderStyle: BorderStyleName
  /** 边框色（大写 `#RRGGBB`）。 */
  borderColor: string
  /** 数字格式串（`''` = 无格式）。 */
  format: string
  /** 旋转角（0..360 整数；`0` = 缺省）。 */
  rotation: number
}

export const CELL_PROPS_DEFAULTS: CellPropsPanel = {
  sides: { top: false, right: false, bottom: false, left: false },
  borderStyle: 'solid',
  borderColor: DEFAULT_BORDER_COLOR,
  format: '',
  rotation: 0,
}

/** 线型下拉选项：开放项 + 当前值（当前值不在开放项里也必须出现在选项里，
 * 否则 select 会静默错显第一项 —— 与 `fontOptions` 同一条理由）。 */
export function borderStyleOptions(current: string): string[] {
  const open = BORDER_LINE_STYLES as readonly string[]
  if (!current || open.includes(current)) return [...open]
  return [current, ...open]
}

/** 数字格式下拉选项：预设 + 当前值（同上，手写格式串不能丢）。 */
export function formatOptions(current: string): string[] {
  const presets = FORMAT_PRESETS as readonly string[]
  if (current === '' || presets.includes(current)) return [...presets]
  return [current, ...presets]
}

// ---------------------------------------------------------------------------
// 读方向：定义 JSON 条目 → 面板显示态
// ---------------------------------------------------------------------------

/** 线型名归一：契约内原样；不在契约内（手写怪值）回落 `solid` 显示。 */
function styleName(v: unknown): BorderStyleName {
  return typeof v === 'string' && (BORDER_STYLE_NAMES as readonly string[]).includes(v)
    ? (v as BorderStyleName)
    : 'solid'
}

/** 一个格子的属性项 → 面板显示态（纯函数）。
 *
 * - 每边存在性 = `border.<side>` 是否有值；
 * - 线型/颜色取**第一条存在的边**（按 `BORDER_SIDES` 序）的值，都没有则缺省；
 * - 颜色走 `normalizeColor`（大写）；不认的形态回落缺省（显示层）；
 * - `format` 只认字符串；`rotation` 只认有限非负数，其余回落缺省。 */
export function cellPropsFromEntry(cell: CellDef | undefined): CellPropsPanel {
  const sides: Record<BorderSideName, boolean> = { top: false, right: false, bottom: false, left: false }
  let borderStyle: BorderStyleName = CELL_PROPS_DEFAULTS.borderStyle
  let borderColor = CELL_PROPS_DEFAULTS.borderColor
  let first = true

  const border = cell?.border
  if (border && typeof border === 'object') {
    for (const s of BORDER_SIDES) {
      const side = border[s]
      if (side === undefined || side === null) continue
      sides[s] = true
      if (first) {
        borderStyle = styleName(side.style)
        borderColor = normalizeColor(side.color) ?? CELL_PROPS_DEFAULTS.borderColor
        first = false
      }
    }
  }

  const fmt = cell?.format
  const rot = cell?.rotation
  return {
    sides,
    borderStyle,
    borderColor,
    format: typeof fmt === 'string' ? fmt : '',
    rotation: typeof rot === 'number' && Number.isFinite(rot) && rot >= 0 ? rot : 0,
  }
}

/** 选区的**起点格**（Univer 0 起）→ 面板显示态。 */
export function cellPropsFromDef(def: ReportDef, range: PanelRange): CellPropsPanel {
  const { row, col } = toLogical(range.row0, range.col0)
  const idx = findCellIndex(def, row, col)
  return cellPropsFromEntry(idx >= 0 ? def.cells?.[idx] : undefined)
}

// ---------------------------------------------------------------------------
// 编辑（归一）
// ---------------------------------------------------------------------------

/** 面板发出的编辑（控件原值）。 */
export type CellPropsEdit =
  | { kind: 'borderSide'; side: BorderSideName; on: boolean; style?: string; color?: string }
  | { kind: 'borderStyle'; style: string }
  | { kind: 'borderColor'; color: string }
  | { kind: 'format'; value: string }
  | { kind: 'rotation'; value: string | number }

/** 归一后的编辑（JSON 补丁与 Univer 落地读的是同一份 —— 不两处各归一一次）。 */
export type NormalizedCellPropsEdit =
  | { kind: 'borderSide'; side: BorderSideName; on: boolean; style: BorderStyleName; color: string }
  | { kind: 'borderStyle'; style: BorderStyleName }
  | { kind: 'borderColor'; color: string }
  | { kind: 'format'; value: string }
  | { kind: 'rotation'; value: number }

/** 控件原值 → 契约值；不合法**显式报错**（不猜、不夹取）。 */
export function normalizeCellPropsEdit(e: CellPropsEdit): {
  edit?: NormalizedCellPropsEdit
  error?: string
} {
  switch (e.kind) {
    case 'borderSide': {
      const style = e.style ?? 'solid'
      if (!(BORDER_STYLE_NAMES as readonly string[]).includes(style)) {
        return { error: `线型 ${JSON.stringify(style)} 不在契约取值里（${BORDER_STYLE_NAMES.join(' / ')}）` }
      }
      const color = normalizeColor(e.color ?? DEFAULT_BORDER_COLOR)
      if (color === null) return { error: `边框色须为 #RRGGBB（实得 ${JSON.stringify(e.color)}）` }
      return { edit: { kind: 'borderSide', side: e.side, on: e.on, style: style as BorderStyleName, color } }
    }
    case 'borderStyle': {
      if (!(BORDER_STYLE_NAMES as readonly string[]).includes(e.style)) {
        return { error: `线型 ${JSON.stringify(e.style)} 不在契约取值里（${BORDER_STYLE_NAMES.join(' / ')}）` }
      }
      return { edit: { kind: 'borderStyle', style: e.style as BorderStyleName } }
    }
    case 'borderColor': {
      const color = normalizeColor(e.color)
      if (color === null) return { error: `边框色须为 #RRGGBB（实得 ${JSON.stringify(e.color)}）` }
      return { edit: { kind: 'borderColor', color } }
    }
    case 'format': {
      if (typeof e.value !== 'string') return { error: `数字格式须为字符串（实得 ${JSON.stringify(e.value)}）` }
      return { edit: { kind: 'format', value: e.value } }
    }
    case 'rotation': {
      if (typeof e.value === 'string' && e.value.trim() === '') return { error: '旋转角不能为空' }
      const n = typeof e.value === 'number' ? e.value : Number(String(e.value).trim())
      if (!Number.isInteger(n)) return { error: `旋转角必须是整数（实得 ${JSON.stringify(e.value)}）` }
      if (n < 0 || n > 360) {
        return { error: `旋转角须在 0..360 之间（服务端只要求 ≥0，面板口径是 0..360；实得 ${n}）` }
      }
      return { edit: { kind: 'rotation', value: n } }    }
  }
}

// ---------------------------------------------------------------------------
// 写方向：定义 JSON 补丁（唯一入口）
// ---------------------------------------------------------------------------

export type CellPropsPatchResult = {
  def?: ReportDef
  normalized?: NormalizedCellPropsEdit
  warnings: string[]
  error?: string
}

/** 边内两值相等判定（缺省与显式缺省视为相同 —— 幂等判据用）。 */
function sameSide(a: JsonBorderSide | undefined, b: JsonBorderSide | undefined): boolean {
  if (!a || !b) return false
  const aColor = normalizeColor(a.color) ?? DEFAULT_BORDER_COLOR
  const bColor = normalizeColor(b.color) ?? DEFAULT_BORDER_COLOR
  const aWidth = typeof a.width === 'number' ? a.width : DEFAULT_BORDER_WIDTH
  const bWidth = typeof b.width === 'number' ? b.width : DEFAULT_BORDER_WIDTH
  return a.style === b.style && aColor === bColor && aWidth === bWidth
}

/** 删掉 `row`/`col` 之外一无所有的条目（返回 `null` = 整条移除）。 */
function stripShell(cell: CellDef): CellDef | null {
  const hasOther = Object.keys(cell).some((k) => k !== 'row' && k !== 'col')
  return hasOther ? cell : null
}

/** 把一次属性编辑补丁进定义 JSON 的**选区**，返回**新**对象（纯函数）。
 *
 * 逐槽语义与 `stylepanel.ts::applyPanelToRange` 同规（裁剪 / 合并区跳过 / 上限 /
 * 重复告警全部复用同一条脚手架），新增一条：**条目删空 ⇒ 整条移除**。 */
export function applyCellPropsToRange(
  def: ReportDef,
  range: PanelRange,
  rawEdit: CellPropsEdit,
): CellPropsPatchResult {
  const warnings: string[] = []
  const nv = normalizeCellPropsEdit(rawEdit)
  if (!nv.edit) return { error: nv.error, warnings }
  const edit = nv.edit

  // 「线型/颜色只作用于已存在的边」：若整个选区一条边都没有，给的是**另一句**提示
  // （「已是该值」会误导 —— 实际是没东西可改）。
  let sidesSeen = 0

  const sc = patchRangeSlots(def, range, (cell, slot) => {
    const base = (): CellDef => cell ?? ({ row: slot.row, col: slot.col } as CellDef)
    switch (edit.kind) {
      case 'borderSide': {
        const curSide = cell?.border?.[edit.side]
        if (edit.on) {
          const sideDef: JsonBorderSide = { style: edit.style }
          if (edit.color !== DEFAULT_BORDER_COLOR) sideDef.color = edit.color
          // 宽度是 JSON_ONLY：已存在则原样保留（≠ 缺省才写回，避免冗余键）。
          if (curSide && typeof curSide.width === 'number' && curSide.width !== DEFAULT_BORDER_WIDTH) {
            sideDef.width = curSide.width
          }
          if (curSide && sameSide(curSide, sideDef)) return undefined
          return { ...base(), border: { ...(cell?.border ?? {}), [edit.side]: sideDef } }
        }
        if (curSide === undefined) return undefined
        const rest = { ...(cell?.border ?? {}) }
        delete rest[edit.side]
        const next = { ...base() }
        if (Object.keys(rest).length === 0) delete next.border
        else next.border = rest
        return stripShell(next)
      }
      case 'borderStyle':
      case 'borderColor': {
        const b = cell?.border
        if (!b) return undefined
        let dirty = false
        const nextBorder: JsonBorder = { ...b }
        for (const s of BORDER_SIDES) {
          const side = nextBorder[s]
          if (!side) continue
          sidesSeen++
          if (edit.kind === 'borderStyle') {
            if (side.style !== edit.style) {
              nextBorder[s] = { ...side, style: edit.style }
              dirty = true
            }
          } else {
            const cur = normalizeColor(side.color) ?? DEFAULT_BORDER_COLOR
            if (cur !== edit.color) {
              const nextSide: JsonBorderSide = { ...side }
              delete nextSide.color
              if (edit.color !== DEFAULT_BORDER_COLOR) nextSide.color = edit.color
              nextBorder[s] = nextSide
              dirty = true
            }
          }
        }
        return dirty ? { ...base(), border: nextBorder } : undefined
      }
      case 'format': {
        const cur = cell?.format
        if (edit.value === '') {
          if (typeof cur !== 'string') return undefined
          const rest = { ...base() } as CellDef
          delete rest.format
          return stripShell(rest)
        }
        if (cur === edit.value) return undefined
        return { ...base(), format: edit.value } as CellDef
      }
      case 'rotation': {
        if (edit.value === 0) {
          if (cell?.rotation === undefined) return undefined
          const rest = { ...base() } as CellDef
          delete rest.rotation
          return stripShell(rest)
        }
        if (cell?.rotation === edit.value) return undefined
        return { ...base(), rotation: edit.value } as CellDef
      }
    }
  })

  const noChange =
    (edit.kind === 'borderStyle' || edit.kind === 'borderColor') && sidesSeen === 0
      ? '所选范围没有边框可改（先在面板里勾选要加边框的边）'
      : edit.kind === 'borderSide'
        ? '所选范围的这些边已是该状态（无需改动）'
        : edit.kind === 'format'
          ? '所选范围的数字格式已是该值（无需改动）'
          : edit.kind === 'rotation'
            ? '所选范围的旋转角已是该值（无需改动）'
            : '所选范围的边框已是该值（无需改动）'
  const fin = finishRangePatch(def, sc, noChange)
  return { ...fin, normalized: edit }
}

// ---------------------------------------------------------------------------
// Univer 侧落地（视觉反馈；测试传同形状的假对象）
// ---------------------------------------------------------------------------

/** [`applyCellPropToRange] 需要的最小 FRange 面（真 `FRange` 可直接传）。
 *
 * `setBorder`/`setNumberFormat`/`setTextRotation` 的签名对齐
 * `f-range.d.ts:744` / `sheets-numfmt …/facade/f-range.d.ts:37` / `f-range.d.ts:812`。
 * ⚠ `BorderType` 是**字符串枚举**（运行时值就是 `'top'`/`'right'`/…，
 * `sheets/lib/es/index.js:…`；`cellprops.test.ts` 有「与真实枚举对齐」的对照测试），
 * 故下面用 [`BORDER_TYPE_OF` 显式映射，不直接传字面量。 */
export type CellPropsRangeLike = {
  setBorder(type: BorderType, style: number, color?: string): unknown
  setNumberFormat(pattern: string): unknown
  setTextRotation(rotation: number): unknown
}

/** 边名 → Univer `BorderType` 枚举值（运行时是字符串；类型上显式标注为枚举成员）。 */
const BORDER_TYPE_OF = {
  top: 'top' as BorderType,
  right: 'right' as BorderType,
  bottom: 'bottom' as BorderType,
  left: 'left' as BorderType,
} as const satisfies Record<BorderSideName, BorderType>

/** 把面板编辑落到 Univer 选区（画布立即反馈）。
 *
 * - `borderSide`：开 → `setBorder(side, code, color)`；关 → 该边传 `NONE`
 *   （`SetBorderCommand` 只对命中的那一边落笔，`sheets/lib/es/index.js:13318-13328`）；
 * - `borderStyle`/`borderColor`：作用于**面板里开着的边**（`panel.sides`）——
 *   JSON 侧只改「已存在的边」，Univer 侧就该落同一组边（改后值取 `panel` 的当前值，
 *   与 JSON 产物一致）；
 * - `format`：`setNumberFormat('')` 在 Univer 侧就是**清除**（`SetNumfmtCommand`
 *   把空 pattern 归到 removeCells，`sheets-numfmt/lib/es/index.js:346-347`）；
 * - `rotation`：`setTextRotation(deg)`（`0` 即回正）。 */
export function applyCellPropToRange(
  range: CellPropsRangeLike,
  e: NormalizedCellPropsEdit,
  panel: Pick<CellPropsPanel, 'sides' | 'borderStyle' | 'borderColor'>,
): void {
  switch (e.kind) {
    case 'borderSide':
      if (e.on) range.setBorder(BORDER_TYPE_OF[e.side], borderStyleToUniverCode(e.style), e.color)
      else range.setBorder(BORDER_TYPE_OF[e.side], U_BORDER_STYLE.NONE) // 关边不带色（色对 NONE 无意义）
      break
    case 'borderStyle':
    case 'borderColor': {
      const style = e.kind === 'borderStyle' ? e.style : panel.borderStyle
      const color = e.kind === 'borderColor' ? e.color : panel.borderColor
      for (const s of BORDER_SIDES) {
        if (panel.sides[s]) range.setBorder(BORDER_TYPE_OF[s], borderStyleToUniverCode(style), color)
      }
      break
    }
    case 'format':
      range.setNumberFormat(e.value)
      break
    case 'rotation':
      range.setTextRotation(e.value)
      break
  }
}
