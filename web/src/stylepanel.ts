/**
 * 样式面板（M7-W W2-2）：7 字段面板的**逐字段补丁**模型。
 *
 * ## 面板的两个方向（读看 Univer，写落 JSON）
 *
 * * **读（显示）**：面板显示态从 Univer 活态读（[`panelFromUniverStyle]）——
 *   用户选一格，看到的是画布上那一格**现在的样子**。
 * * **写（补丁）**：用户在面板上改哪个字段，就只把**那一个字段**写进定义 JSON
 *   （[`applyPanelToRange]`），Univer 侧另外同步落地（[`applyFieldToRange]`）。
 *
 * ## 为什么是「逐字段」而不是复用 `stylemap.ts::applyCellStyle`（12 槽整体覆盖）
 *
 * `applyCellStyle` 的语义是「把 Univer 活态**整体**读回来覆盖 12 个槽」。它对本
 * 面板**不能用**：服务端快照是有损的 —— `snapshot/style.rs::style_json`
 * （`crates/rpx-server/src/snapshot/style.rs:15-45`）只发 9 个字段
 * （`ff/fs/bl/it/ul/cl/bg/ht/vt`），`border` / `format` / `rotation` 不在其中
 * （录得快照 `tests/fixtures/out/engine_merge.snapshot.json` 逐格实测一致）。而 Univer
 * 侧一切样式都来自这份快照 ⇒ 画布里这些槽**永远是空的**。若走整体覆盖，
 * 用户仅仅点一下「加粗」，JSON 里原有的 `border` / `format` / `rotation` /
 * `indent` / `extend_mode` 就被一起抹掉 —— 正是 `stylemap.ts` 文件头警告的
 * 「快照有损 ⇒ 必须打补丁」那条纪律。
 * ⇒ 本模块只动**用户刚改的那一个字段**（7 个字段 ↔ 7 个 JSON 槽的固定映射，
 * 见 `FIELD_SPEC`），其余键逐个原样保留（`CellDef` 索引签名 + 整体展开）。
 *
 * ## 缺省即删键（与 `applyCellStyle` 同一条语义）
 *
 * 字段值等于 rpx 缺省（`crates/rpx-core/src/props.rs` 的 `def` 模块）⇒ **删掉该键**，
 * 而不是写 `bold:false` / `font_size:12`。否则「加粗 → 撤销加粗」会在 JSON 里
 * 留下冗余键，与「未声明 = 缺省」的服务端语义（`reportdef/style.rs::apply_style`
 * 逐字段 `if let Some`）形成两个都合法但形态不同的写法。
 *
 * ## 选区的三个服务端事实（决定了本模块的边界检查）
 *
 * 1. **越界 400**：`cells[].row/col` 必须落在 `1..=grid.rows|cols`
 *    （`reportdef/parse.rs:186-197`）；而 Univer 画布比网格大
 *    （`snapshot.rs:64-65`：`rowCount=(grid.rows+5).max(100)`）⇒ 选区必须**裁剪**到网格内。
 * 2. **合并矩形内的非锚点槽会被静默跳过**：`parse.rs:88-99` 对这些槽
 *    只拦有内容的，仅样式的条目 `continue` 丢弃、**不报错**。渲染以合并锚点为准
 *    （合并区画的就是锚点那一格），⇒ 打补丁时**跳过**这些槽是对的（不是信息丢失），
 *    但「整个选区全落在合并区内部」时没有任何可落点 ⇒ 显式报错，指路锚点。
 * 3. **上限**：选区可能被 Ctrl+A 撑到整个画布（`100×26=2600` 槽）—— 一次改这么多
 *    会把定义 JSON 撑成不可读。超过 [`MAX_PANEL_SLOTS] 直接报错，不静默截断
 *    （截断 = 用户以为改全了）。
 *
 * ## Univer 侧落地的坑（探针/类型声明实测，见 `docs/spec-m7-web.md` §7 坑位 4）
 *
 * `FontWeight = 'normal' | 'bold'`、`FontStyle = 'normal' | 'italic'`、
 * `FontLine = 'none' | 'underline' | 'line-through'`（`f-range.d.ts:24-26`）——
 * 关闭样式不是传 `false`，而是传 `'normal'` / `'none'`，见 [`applyFieldToRange]。
 */
import type { IColorStyle, IStyleData, ITextDecoration } from '@univerjs/core'

import { coveredSlots, slotKey, type ReportDef } from './designer'
import {
  DEFAULT_BACK_COLOR,
  DEFAULT_FONT_NAME,
  DEFAULT_FONT_SIZE,
  DEFAULT_FORE_COLOR,
  FONT_SIZE_MAX,
  FONT_SIZE_MIN,
  colorOf,
  normalizeColor,
  type CellDef,
  type JsonStyleFields,
} from './stylemap'

// ---------------------------------------------------------------------------
// 面板模型
// ---------------------------------------------------------------------------

/** 面板显示的 7 个字段（= M7-3 的 7 个样式字段，`docs/report-def-json.md` §3.2）。
 *
 * 全部是**具体值**（没有 `null`/三态）：本里程碑只做单/矩形选区（选区不齐时
 * 以活动格显示），三态混合显示留待后续。 */
export type PanelStyle = {
  fontName: string
  fontSize: number
  bold: boolean
  italic: boolean
  underline: boolean
  foreColor: string
  backColor: string
}

export type PanelField = keyof PanelStyle
export type PanelValue = string | number | boolean

/** 7 字段的固定遍历序（导出 JSON 的键序稳定性靠它 + 面板交互顺序一致）。 */
export const PANEL_FIELDS = [
  'fontName',
  'fontSize',
  'bold',
  'italic',
  'underline',
  'foreColor',
  'backColor',
] as const satisfies readonly PanelField[]

/** rpx 缺省（`props.rs::def`：`DEFAULT_FONT_NAME="宋体"` / `FONT_SIZE=12` /
 * `COLOR_BLACK` / `COLOR_WHITE`；三个位标志缺省即关）。等价于「这 7 个键全不声明」。 */
export const PANEL_DEFAULTS: PanelStyle = {
  fontName: DEFAULT_FONT_NAME,
  fontSize: DEFAULT_FONT_SIZE,
  bold: false,
  italic: false,
  underline: false,
  foreColor: DEFAULT_FORE_COLOR,
  backColor: DEFAULT_BACK_COLOR,
}

/** 字段 → 定义 JSON 键名 + 缺省值（缺省 ⇒ 删键，见文件头）。 */
const FIELD_SPEC: Record<PanelField, { key: keyof JsonStyleFields & string; default: PanelValue }> = {
  fontName: { key: 'font_name', default: DEFAULT_FONT_NAME },
  fontSize: { key: 'font_size', default: DEFAULT_FONT_SIZE },
  bold: { key: 'bold', default: false },
  italic: { key: 'italic', default: false },
  underline: { key: 'underline', default: false },
  foreColor: { key: 'fore_color', default: DEFAULT_FORE_COLOR },
  backColor: { key: 'back_color', default: DEFAULT_BACK_COLOR },
}

/** 选区一次可改的槽数上限。防 Ctrl+A 撑爆 JSON 的护栏，见文件头事实 3。 */
export const MAX_PANEL_SLOTS = 400

// ---------------------------------------------------------------------------
// 读方向：Univer 活态 → 面板显示态
// ---------------------------------------------------------------------------

/** 字体下拉的常用项：前 5 个是中文报表的常客，后 3 个 English/等宽。 */
export const COMMON_FONT_NAMES = [
  '宋体',
  '黑体',
  '微软雅黑',
  '楷体',
  '仿宋',
  'Times New Roman',
  'Arial',
  'Courier New',
] as const

/** 字体下拉选项：常用项 + 当前值。
 *
 * ⚠ 当前值不在常用表里时**必须补进选项**（放首位）—— 否则 `<select>` 找不到
 * 匹配项会显示第一项，用户以为字体是「宋体」而实际是别的（静默错显）。 */
export function fontOptions(current: string): string[] {
  const common = COMMON_FONT_NAMES as readonly string[]
  if (!current || common.includes(current)) return [...common]
  return [current, ...common]
}

/** Univer 活态样式 → 面板显示态。
 *
 * 口径：
 * - 缺样式对象 ⇒ 全缺省（画布上这类格就是这样渲染的）；
 * - 字号取整（`Math.round`）并夹到 `1..=32767`（与服务端 `font_size` 界
 *   `reportdef/style.rs:29-34` 一致，避免面板显示一个永远提交不了的数）；
 * - 下划线读 `ul.s === 1`（⚠ `ul` 是对象不是布尔，陷阱 3，`stylemap.ts` 文件头）；
 * - 颜色读 `IColorStyle.rgb` 归一成大写 `#RRGGBB`；主题色/不认的形态**回落缺省**
 *   （面板是显示层；写方向的 warning 由 `applyPanelToRange` 走 `stylemap` 给）。 */
export function panelFromUniverStyle(style: IStyleData | undefined | null): PanelStyle {
  if (!style) return { ...PANEL_DEFAULTS }
  const s = style as IStyleData & Record<string, unknown>
  const ul = s.ul as ITextDecoration | null | undefined
  const fs = typeof s.fs === 'number' && Number.isFinite(s.fs) ? Math.round(s.fs) : DEFAULT_FONT_SIZE
  return {
    fontName: typeof s.ff === 'string' && s.ff ? s.ff : DEFAULT_FONT_NAME,
    fontSize: Math.min(Math.max(fs, FONT_SIZE_MIN), FONT_SIZE_MAX),
    bold: s.bl === 1,
    italic: s.it === 1,
    underline: !!ul && typeof ul === 'object' && ul.s === 1,
    // `Nullable<IColorStyle>`（Univer 的 `Nullable` 含 `void`）⇒ 窄化到色板函数的入参型。
    foreColor: colorOf(s.cl as IColorStyle | null | undefined) ?? DEFAULT_FORE_COLOR,
    backColor: colorOf(s.bg as IColorStyle | null | undefined) ?? DEFAULT_BACK_COLOR,
  }
}

// ---------------------------------------------------------------------------
// 值归一化（UI 输入 → 契约值）
// ---------------------------------------------------------------------------

export type PanelValueResult = { value?: PanelValue; error?: string }

/** 面板控件原值 → 服务端契约值。不合法就**显式报错**（不猜、不截断）。
 *
 * - `fontName`：非空字符串（`font_name` 是 `opt_str`，空串无意义）；
 * - `fontSize`：整数且 `1..=32767`（`reportdef/style.rs:29-34`，非整数/越界都 400）；
 * - 三个位标志：布尔（`style.rs:43-47` 要求 `as_bool`）；
 * - 两个颜色：`normalizeColor` 归一成**大写** `#RRGGBB`（服务端
 *   `argb_hex` 出大写，`parse_color` 大小写都收 —— 统一出大写才不会与
 *   录得快照/服务端产物出现「大小写不同」的假差异，见 `spec-m7-web.md` §7 坑位 5c）。 */
export function normalizePanelValue(field: PanelField, raw: unknown): PanelValueResult {
  switch (field) {
    case 'fontName': {
      const s = typeof raw === 'string' ? raw : String(raw)
      if (!s.trim()) return { error: '字体名不能为空' }
      return { value: s }
    }
    case 'fontSize': {
      if (typeof raw === 'string' && raw.trim() === '') return { error: '字号不能为空' }
      const n = typeof raw === 'number' ? raw : Number(String(raw).trim())
      if (!Number.isInteger(n)) return { error: `字号必须是整数（实得 ${JSON.stringify(raw)}）` }
      if (n < FONT_SIZE_MIN || n > FONT_SIZE_MAX) {
        return { error: `字号越界（要求 ${FONT_SIZE_MIN}..=${FONT_SIZE_MAX}，实得 ${n}）` }
      }
      return { value: n }
    }
    case 'bold':
    case 'italic':
    case 'underline': {
      if (typeof raw !== 'boolean') return { error: `${field} 应为布尔值（实得 ${JSON.stringify(raw)}）` }
      return { value: raw }
    }
    case 'foreColor':
    case 'backColor': {
      const c = normalizeColor(raw)
      if (c === null) return { error: `颜色须为 #RRGGBB（实得 ${JSON.stringify(raw)}）` }
      return { value: c }
    }
  }
}

// ---------------------------------------------------------------------------
// 选区脚手架（W2-2 样式补丁与 W2-3 属性补丁共用；`cellprops.ts` 也走这一条）
// ---------------------------------------------------------------------------

/** 选区（Univer 0 起坐标；`height`/`width` 是**格数**，即 `FRange.getHeight()`
 * / `getWidth()` 的口径 —— `f-range.d.ts:168/181` 明写「count, not pixels」）。 */
export type PanelRange = { row0: number; col0: number; height: number; width: number }

/** 补丁结果。`error` 与 `def`/`normalized` **互斥**：出错就不产出补丁（不猜）。 */
export type PanelPatchResult = {
  def?: ReportDef
  /** 归一后的值（Univer 侧落地要用同一份，避免两处各归一一次产生分叉）。 */
  normalized?: PanelValue
  warnings: string[]
  error?: string
}

/** [`patchRangeSlots] 的原始结果（未组成 warnings/error 文案）。 */
export type RangeSlotsResult = {
  /** 有改动时的**新** `cells` 数组；零改动为 `undefined`（保持原引用）。 */
  cells?: CellDef[]
  /** 裁剪后为空 / 超上限（工具层错误，先于逐槽判断）。 */
  error?: string
  /** 实际改动的槽数。 */
  changed: number
  /** 被合并矩形覆盖而跳过的槽数。 */
  skipped: number
  /** 裁剪后的槽总数（= changed + skipped + 未改动槽）。 */
  total: number
  /** 有重复 `(row,col)` 条目的槽（只改第一条，另行告警）。 */
  dup: string[]
}

/** 选区脚手架：裁剪到网格 → 槽数上限 → 合并区跳过 → 逐槽回调。
 *
 * W2-2 的样式补丁与 W2-3 的属性补丁**共用这一份**裁剪/上限/合并跳过/重复检测
 * —— 两处各写一套，判定就会静默错位（与 `slotKey` 同一条纪律）。
 *
 * @param fn 逐槽回调（逻辑坐标）：入参是该槽**当前**的条目（可能 `undefined`）
 *   与坐标；返回 `undefined` = 该槽不改变，`null` = **删除**该槽的条目
 *   （空壳清理：条目删到只剩 `row`/`col` 时不该留在 `cells[]` 里），
 *   返回新条目 = 写入（整条目替换）。回调**不**负责边界与合并判定（脚手架已做）。 */
export function patchRangeSlots(
  def: ReportDef,
  range: PanelRange,
  fn: (cell: CellDef | undefined, slot: { row: number; col: number }) => CellDef | null | undefined,
): RangeSlotsResult {
  const empty: RangeSlotsResult = { changed: 0, skipped: 0, total: 0, dup: [] }
  // ---- 裁剪到网格（服务端越界 400；画布比网格大，见文件头事实 1）----
  const r1 = Math.max(1, range.row0 + 1)
  const c1 = Math.max(1, range.col0 + 1)
  const r2 = Math.min(def.grid.rows, range.row0 + range.height)
  const c2 = Math.min(def.grid.cols, range.col0 + range.width)
  if (r1 > r2 || c1 > c2) {
    return {
      ...empty,
      error: `选区不覆盖网格内的任何单元格（grid 是 ${def.grid.rows} 行 × ${def.grid.cols} 列，逻辑坐标 1 起）`,
    }
  }
  const total = (r2 - r1 + 1) * (c2 - c1 + 1)
  if (total > MAX_PANEL_SLOTS) {
    return {
      ...empty,
      error: `选区过大：${total} 个槽超过上限 ${MAX_PANEL_SLOTS} —— 一次改这么多格会让定义 JSON 不可读，请缩小选区`,
    }
  }

  const covered = coveredSlots(def)
  const cells = def.cells ? [...def.cells] : []
  let changed = 0
  let skipped = 0
  const dup: string[] = []

  for (let r = r1; r <= r2; r++) {
    for (let c = c1; c <= c2; c++) {
      if (covered.has(slotKey(r, c))) {
        skipped++
        continue
      }
      const idx = cells.findIndex((x) => x.row === r && x.col === c)
      if (idx >= 0 && cells.filter((x) => x.row === r && x.col === c).length > 1) {
        dup.push(`(${r},${c})`)
      }
      const next = fn(idx >= 0 ? cells[idx] : undefined, { row: r, col: c })
      if (next === undefined) continue
      if (next === null) {
        if (idx >= 0) {
          cells.splice(idx, 1)
          changed++
        }
        continue
      }
      if (idx >= 0) cells[idx] = next
      else cells.push(next)
      changed++
    }
  }
  return { cells: changed > 0 ? cells : undefined, changed, skipped, total, dup }
}

/** 把脚手架结果收束成补丁结果（两模块共用的收尾口径）：
 * 工具层错误原样上抛；**全落合并区** ⇒ 报错指路锚点；重复条目 ⇒ 告警；
 * 零改动 ⇒ 由调用方给「无需改动」文案（缺省值 / 该值 / 没有可改对象，口径不同）。 */
export function finishRangePatch(
  def: ReportDef,
  sc: RangeSlotsResult,
  noChangeWarning: string,
): { def?: ReportDef; error?: string; warnings: string[] } {
  const warnings: string[] = []
  if (sc.error) return { error: sc.error, warnings }
  if (sc.changed === 0 && sc.skipped === sc.total) {
    return {
      error:
        '选区全部落在合并矩形内部（非锚点槽）——服务端对合并区只认锚点，请选中合并区左上角那一格再改样式',
      warnings,
    }
  }
  if (sc.dup.length > 0) {
    const head = sc.dup.slice(0, 5).join(' ')
    warnings.push(
      `${sc.dup.length} 个槽在 cells[] 里有重复条目，只改了第一条：${head}${sc.dup.length > 5 ? ' …' : ''}`,
    )
  }
  if (sc.changed === 0) {
    warnings.push(noChangeWarning)
    return { def, warnings }
  }
  return { def: { ...def, cells: sc.cells ?? def.cells ?? [] }, warnings }
}

// ---------------------------------------------------------------------------
// 写方向：定义 JSON 补丁（W2-2 唯一入口）
// ---------------------------------------------------------------------------

/** 把一个字段的值补丁进定义 JSON 的**选区**，返回**新**对象（纯函数）。
 *
 * 逐槽语义（与 `designer.ts::applyValueEdit` 的「只补用户真改过的那一格」
 * 同一条纪律，只是「改过的」= 整个选区）：
 * - 槽在 `grid` 外或被裁剪后为空 ⇒ 报错；
 * - 槽落在合并矩形内（非锚点）⇒ **跳过**（服务端静默丢弃，渲染随锚点，见文件头事实 2）；
 * - 有 `cells[]` 条目 ⇒ 在原条目上改这**一个**键（其余键原样：`expr` / `indent` /
 *   `extend_mode` / `border` / 用户手写字段都保留）；
 * - 没有条目且值非缺省 ⇒ **追加** `{row, col, <键>: <值>}`；
 * - 没有条目且值即缺省 ⇒ 什么都不做（不凭空造空壳）；
 * - 值即缺省而条目里有该键 ⇒ **删键**（缺省即删键，见文件头）。
 *
 * @param def   当前持有的定义 JSON（不会被修改）
 * @param range 选区（Univer 0 起）
 * @param field 面板字段
 * @param raw   控件原值（内部走 [`normalizePanelValue] 归一，非法值报错） */
export function applyPanelToRange(
  def: ReportDef,
  range: PanelRange,
  field: PanelField,
  raw: unknown,
): PanelPatchResult {
  const warnings: string[] = []
  const nv = normalizePanelValue(field, raw)
  if (nv.value === undefined) return { error: nv.error, warnings }
  const value = nv.value
  const spec = FIELD_SPEC[field]
  const elide = value === spec.default

  const sc = patchRangeSlots(def, range, (cell, slot) => {
    if (cell === undefined) {
      // 没有条目：非缺省才追加（缺省 ⇒ 不凭空造空壳）。
      return elide ? undefined : ({ row: slot.row, col: slot.col, [spec.key]: value } as CellDef)
    }
    if (elide) {
      if (cell[spec.key] === undefined) return undefined
      const rest = { ...cell }
      delete rest[spec.key]
      return rest as CellDef
    }
    if (cell[spec.key] === value) return undefined
    return { ...cell, [spec.key]: value } as CellDef
  })
  const fin = finishRangePatch(def, sc, elide ? '所选范围已是缺省值（无需改动）' : '所选范围已经是该值（无需改动）')
  return { ...fin, normalized: value }
}

/** 「恢复缺省」：把 7 个字段全部打回缺省（= 删掉这 7 个键）。
 *
 * 逐字段复用 [`applyPanelToRange]，所以「选区全落合并区 / 越界 / 超上限」的
 * 报错口径与单字段完全一致（不留第二条判定路径）。 */
export function clearPanelRange(def: ReportDef, range: PanelRange): PanelPatchResult {
  const warnings: string[] = []
  let cur = def
  let anyChange = false
  for (const field of PANEL_FIELDS) {
    const r = applyPanelToRange(cur, range, field, PANEL_DEFAULTS[field])
    if (r.error) return { error: r.error, warnings }
    for (const w of r.warnings) {
      // 逐字段的「无需改动」噪声合并成一句（7 个字段会产生 7 条）。
      if (w.startsWith('所选范围已是')) continue
      warnings.push(w)
    }
    // 变没变看引用：改了 ⇒ 新对象；没改 ⇒ 原样返回同一个引用（`applyPanelToRange`
    // 的「无需改动」分支返回入参）。⚠ 必须在赋回 `cur` **之前**比。
    if (r.def && r.def !== cur) anyChange = true
    if (r.def) cur = r.def
  }
  if (!anyChange) warnings.push('所选范围已是缺省样式（无需改动）')
  return { def: cur, warnings }
}

// ---------------------------------------------------------------------------
// Univer 侧落地（唯一接收 FRange 的地方；测试传同形状的假对象）
// ---------------------------------------------------------------------------

/** [`applyFieldToRange] 需要的最小 FRange 面（结构化类型，真 `FRange` 可直接传）。
 * 签名逐条对齐 `f-range.d.ts:784/1043/1057/1071/1097/1111/1125`。 */
export type PanelRangeLike = {
  setFontFamily(fontFamily: string | null): unknown
  setFontSize(size: number | null): unknown
  setFontWeight(fontWeight: 'normal' | 'bold' | null): unknown
  setFontStyle(fontStyle: 'normal' | 'italic' | null): unknown
  setFontLine(fontLine: 'none' | 'underline' | 'line-through' | null): unknown
  setFontColor(color: string | null): unknown
  setBackgroundColor(color: string): unknown
}

/** 把面板字段落到 Univer 选区（画布立即反馈）。
 *
 * ⚠ 关闭三个位样式不是传 `false`：`FontWeight` / `FontStyle` / `FontLine`
 * 都是字符串枚举（`f-range.d.ts:24-26`），传 `'normal'` / `'none'` 才是关。 */
export function applyFieldToRange(range: PanelRangeLike, field: PanelField, value: PanelValue): void {
  switch (field) {
    case 'fontName':
      range.setFontFamily(String(value))
      break
    case 'fontSize':
      range.setFontSize(Number(value))
      break
    case 'bold':
      range.setFontWeight(value ? 'bold' : 'normal')
      break
    case 'italic':
      range.setFontStyle(value ? 'italic' : 'normal')
      break
    case 'underline':
      range.setFontLine(value ? 'underline' : 'none')
      break
    case 'foreColor':
      range.setFontColor(String(value))
      break
    case 'backColor':
      range.setBackgroundColor(String(value))
      break
  }
}

/** 「恢复缺省」的 Univer 侧：7 个字段逐个落回缺省。 */
export function resetRangeToDefaults(range: PanelRangeLike): void {
  for (const field of PANEL_FIELDS) applyFieldToRange(range, field, PANEL_DEFAULTS[field])
}
