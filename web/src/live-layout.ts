/**
 * 活态版面 → 打印预览的取数层。
 *
 * # 为什么要有这个模块
 *
 * 画廊「报表结果」是 Univer 画布，打印预览是另一套 HTML 表格渲染。两者原本吃
 * 的是**同一个静态快照**（`workbook.json`），于是画布上拖过列宽/行高后，
 * 预览仍按拖之前的版面渲染 —— 用户看到的是「排版改了但预览没反应」。
 *
 * 实测（`tests/fixtures/out/drag_scan2.mjs`）：画廊画布**能**拖列宽（只读只锁
 * 数据编辑，不锁版式），拖完活态列宽确实变了（A 列 52.91 → 90.91），而预览
 * 列宽前后完全一致。断点就在这里。
 *
 * # 口径：取「用户拖出来的增量」，不是「画布上的绝对宽度」
 *
 * 画布装载的是 `fitColumns(workbook)` —— **显示层加宽**后的副本（数字格放不下
 * 会画成 `####`，网页侧按「看得清」加宽）。这个加宽是**显示层**的，按
 * `fit-columns.ts` 的约定**不得**影响打印分页与 PDF 页数。
 *
 * 所以不能把活态的绝对列宽直接覆盖到快照上 —— 那会把显示层加宽一并带进打印
 * 版面（实测活态列 2/4 是 73.16/65.32，快照是 56.69/49.13，直接覆盖等于
 * 偷偷改了打印版面）。
 *
 * 正确算法是**增量**：
 *
 * ```
 * 增量 = 活态宽度 − 画布装载时的显示宽度（= fitColumns 后的宽度）
 * 打印宽度 = 快照宽度 + 增量
 * ```
 *
 * 于是：
 *
 * - 没拖过 ⇒ 增量为 0 ⇒ 预览与拖之前**逐像素一致**（不回归）；
 * - 拖过 ⇒ 预览按拖的量跟着变（用户要的）；
 * - 显示层加宽永远不进打印版面（分页契约不破）。
 *
 * # 内容永远以快照为准
 *
 * 单元格值、合并区、图形、打印设置一律取 `workbook.json`（引擎产物的权威
 * 版本）。画布是只读的（`setEditable(false)` 只锁数据编辑），内容不会被改动，
 * 但仍以快照为准，避免引入活态里的临时状态。
 *
 * ⚠ 活态快照的 `sheets` 可能是 Map（实测），取 sheet 要两种都兜住。
 */

/** 行/列尺寸项（打印渲染只读 `h` / `w` / `hd`）。 */
interface Size {
  h?: number
  w?: number
  hd?: number
}

/** 打印预览需要的最小 sheet 形态（与 `PrintPreview.tsx` 的 `Sheet` 对齐）。 */
export interface LiveSheet {
  name?: string
  printRowCount?: number
  printColumnCount?: number
  defaultRowHeight?: number
  defaultColumnWidth?: number
  rowData?: Record<string, Size>
  columnData?: Record<string, Size>
  /** 内容类字段只做「原样保留」，不参与合并，故用宽类型。 */
  cellData?: unknown
  mergeData?: unknown[]
  rpxGraphics?: unknown[]
  pageBackground?: string
  printSize?: { width: number; height: number; margins: number[] }
  printOptions?: unknown
}

/** Univer 活态快照的最小形态。 */
export interface LiveWorkbook {
  sheetOrder?: string[]
  sheets?: Record<string, LiveSheet> | Map<string, LiveSheet>
  [key: string]: unknown
}

/** 从活态快照里取一个 sheet（`sheets` 可能是普通对象，也可能是 Map）。 */
export function liveSheet(live: LiveWorkbook | null | undefined, sheetId: string): LiveSheet | undefined {
  if (!live) return undefined
  const sheets = live.sheets
  if (!sheets) return undefined
  const sheet = sheets instanceof Map ? sheets.get(sheetId) : sheets[sheetId]
  return sheet ?? undefined
}

/** 打印工作簿的最小形态（`sheetOrder` + `sheets` 必填）。 */
interface LayoutBook {
  sheetOrder: string[]
  sheets: Record<string, LiveSheet>
}

/** 尺寸基线：每个 sheet 的行高/列宽（装载时的值，冻结）。 */
export type SizeBaseline = Record<string, { columns: Record<string, number>; rows: Record<string, number> }>

/**
 * 冻结「画布装载时的尺寸」基线。
 *
 * # 为什么必须冻结
 *
 * Univer 装载工作簿后会**原地修改**传进去的对象（拖列宽就是直接改那份
 * `columnData`）。若把装载用的那份对象当基线，用户拖完之后基线也跟着变，
 * 增量恒为 0（实测：拖完 `display` 的列 0 也变成了 90.91）。
 *
 * 所以装载前先**深拷贝**一份尺寸出来当基线——它与画布里的对象再无引用关系。
 *
 * @param display 画布即将装载的工作簿（`fitColumns` 加宽后的显示副本）
 * @returns 每个 sheet 的行高/列宽基线（纯数字，冻结）
 */
export function captureSizeBaseline(display: LayoutBook | null | undefined): SizeBaseline {
  const baseline: SizeBaseline = {}
  if (!display?.sheetOrder || !display.sheets) return baseline
  for (const sheetId of display.sheetOrder) {
    const sheet = display.sheets[sheetId]
    if (!sheet) continue
    const columns: Record<string, number> = {}
    for (const [index, item] of Object.entries(sheet.columnData ?? {})) {
      if (typeof item?.w === 'number') columns[index] = item.w
    }
    const rows: Record<string, number> = {}
    for (const [index, item] of Object.entries(sheet.rowData ?? {})) {
      if (typeof item?.h === 'number') rows[index] = item.h
    }
    baseline[sheetId] = { columns, rows }
  }
  return baseline
}

/**
 * 用**用户拖出来的增量**更新快照的打印尺寸。
 *
 * @param snapshot 引擎产出的 `workbook.json`（内容与打印契约的权威来源）
 * @param baseline 画布装载时的尺寸基线（`captureSizeBaseline` 的产物）
 * @param live     Univer 活态快照（用户拖出来的当前版面）
 * @returns 合并后的工作簿；无增量或形状不符时**原样返回 snapshot**
 */
export function mergeLiveLayout<T extends LayoutBook>(
  snapshot: T,
  baseline: SizeBaseline | null | undefined,
  live: LiveWorkbook | null | undefined,
): T {
  if (!live || !baseline || !snapshot.sheetOrder || !snapshot.sheets) return snapshot
  const sheets: Record<string, LiveSheet> = { ...snapshot.sheets }
  let changed = false
  for (const sheetId of snapshot.sheetOrder) {
    const base = sheets[sheetId]
    const liveData = liveSheet(live, sheetId)
    const baseSizes = baseline[sheetId]
    if (!base || !liveData || !baseSizes) continue
    // 只取**尺寸**：内容/合并/图形/打印设置一律以快照为准。
    const columnData = applyDelta(base.columnData, liveData.columnData, baseSizes.columns, 'w')
    const rowData = applyDelta(base.rowData, liveData.rowData, baseSizes.rows, 'h')
    if (!columnData && !rowData) continue
    sheets[sheetId] = { ...base, columnData: columnData ?? base.columnData, rowData: rowData ?? base.rowData }
    changed = true
  }
  return changed ? ({ ...snapshot, sheets } as T) : snapshot
}

/**
 * 逐项算增量并叠加：`结果 = 快照值 + (活态值 − 基线值)`。
 *
 * @param key 取尺寸的字段名（列用 `w`，行用 `h`）
 * @returns 有增量的项合成的表；无增量返回 undefined（调用方据此判定「没动过」）
 */
function applyDelta(
  base: Record<string, Size> | undefined,
  live: Record<string, Size> | undefined,
  baseline: Record<string, number>,
  key: 'w' | 'h',
): Record<string, Size> | undefined {
  if (!base || !live) return undefined
  const merged: Record<string, Size> = { ...base }
  let touched = false
  for (const [index, liveItem] of Object.entries(live)) {
    const liveValue = liveItem?.[key]
    if (typeof liveValue !== 'number') continue
    const baselineValue = baseline[index]
    const baseValue = base[index]?.[key]
    // 基线缺失（该列不在装载副本里）⇒ 无从判断增量，保留快照值。
    if (typeof baselineValue !== 'number' || typeof baseValue !== 'number') continue
    const delta = liveValue - baselineValue
    if (delta === 0) continue
    merged[index] = { ...base[index], [key]: baseValue + delta }
    touched = true
  }
  return touched ? merged : undefined
}

// ---------------------------------------------------------------------------
// 样式层（2026-10-20）：用户在结果画布上改的**格式**也要进打印预览
// ---------------------------------------------------------------------------
//
// 与尺寸的差别：尺寸是「连续量」，可以算增量；样式是「离散量」，只能比等不等。
// 所以样式走绝对值替换：活态样式 != 装载基线 ⇒ 用活态样式覆盖快照里那一格。
//
// ⚠ 两个实测坑（都会让「读样式」读出空）：
// 1. `save()` 里单元格的 `s` 可能是**字符串 id**（指向 `workbook.styles[id]`），
//    也可能是内联对象 —— 改过一次格式后就会变成 id 形态，必须先 resolve；
// 2. Univer 会**原地修改**装载进去的对象，所以基线必须在装载前深拷贝。

/** Univer 样式对象（只做透传，不解释字段）。 */
export type CellStyle = Record<string, unknown>

/** 打印侧的单元格记录（只需要 `s`，其余原样保留）。 */
interface PrintCell {
  v?: unknown
  t?: number
  s?: unknown
}

/** 单格样式基线：`sheetId → "rowIndex:colIndex" → 规范化 JSON`。 */
export type StyleBaseline = Record<string, Record<string, string>>

/** 键名排序后再序列化：同一份样式换个键序不该算「改过」。 */
function stableJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(',')}]`
  if (value && typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>)
      .filter(([, item]) => item !== undefined)
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    return `{${entries.map(([key, item]) => `${JSON.stringify(key)}:${stableJson(item)}`).join(',')}}`
  }
  return JSON.stringify(value ?? null)
}

/** 从活态快照里取样式表（`styles` 可能是普通对象，也可能是 Map）。 */
function styleTable(live: LiveWorkbook | null | undefined): Record<string, CellStyle> {
  const raw = (live as { styles?: unknown } | null | undefined)?.styles
  if (!raw) return {}
  if (raw instanceof Map) return Object.fromEntries(raw as Map<string, CellStyle>)
  return raw as Record<string, CellStyle>
}

/** 把单元格的 `s` 解析成样式对象：id → 查 `styles` 表；对象 → 原地返回。 */
export function resolveCellStyle(cell: unknown, styles: Record<string, CellStyle>): CellStyle | undefined {
  const raw = (cell as PrintCell | undefined)?.s
  if (raw == null) return undefined
  if (typeof raw === 'string') return styles[raw]
  return raw as CellStyle
}

/**
 * 冻结「画布装载时的每格样式」基线（深拷贝 + 规范化序列化）。
 *
 * @param display 画布即将装载的工作簿（`fitColumns` 加宽后的显示副本）
 */
export function captureStyleBaseline(display: LiveWorkbook | null | undefined): StyleBaseline {
  const baseline: StyleBaseline = {}
  if (!display) return baseline
  const styles = styleTable(display)
  const sheets = display.sheets
  const order = display.sheetOrder ?? []
  for (const sheetId of order) {
    const sheet = sheets instanceof Map ? sheets.get(sheetId) : sheets?.[sheetId]
    if (!sheet) continue
    const cells: Record<string, string> = {}
    for (const [row, line] of Object.entries((sheet.cellData ?? {}) as Record<string, Record<string, PrintCell>>)) {
      for (const [col, cell] of Object.entries(line ?? {})) {
        const style = resolveCellStyle(cell, styles)
        if (style) cells[`${row}:${col}`] = stableJson(style)
      }
    }
    baseline[sheetId] = cells
  }
  return baseline
}

/**
 * 把用户在结果画布上改过的**样式**覆盖进打印工作簿。
 *
 * @param snapshot 引擎快照（内容与打印契约的权威来源）
 * @param baseline `captureStyleBaseline` 的产物（装载时冻结）
 * @param live     Univer 活态快照
 * @returns 合并后的工作簿；没有样式变化时**原样返回 snapshot**
 */
export function mergeLiveStyles<T extends LayoutBook>(
  snapshot: T,
  baseline: StyleBaseline | null | undefined,
  live: LiveWorkbook | null | undefined,
): T {
  if (!live || !baseline || !snapshot.sheetOrder || !snapshot.sheets) return snapshot
  const styles = styleTable(live)
  let changed = false
  const sheets: Record<string, LiveSheet> = { ...snapshot.sheets }
  for (const sheetId of snapshot.sheetOrder) {
    const base = sheets[sheetId]
    const liveData = liveSheet(live, sheetId)
    const sheetBaseline = baseline[sheetId]
    if (!base || !liveData || !sheetBaseline) continue
    const liveCells = (liveData.cellData ?? {}) as Record<string, Record<string, PrintCell>>
    const baseCells = (base.cellData ?? {}) as Record<string, Record<string, PrintCell>>
    let sheetChanged = false
    const nextCells: Record<string, Record<string, PrintCell>> = { ...baseCells }
    for (const [row, line] of Object.entries(liveCells)) {
      for (const [col, cell] of Object.entries(line ?? {})) {
        const key = `${row}:${col}`
        const style = resolveCellStyle(cell, styles)
        const liveJson = style ? stableJson(style) : ''
        const baselineJson = sheetBaseline[key] ?? ''
        if (liveJson === baselineJson) continue
        const next = { ...(nextCells[row] ?? {}) }
        const record: PrintCell = { ...(next[col] ?? {}) }
        if (style) record.s = style
        else delete record.s
        // 只带样式、没有值的格在快照里可能不存在；这种格本来就是「空」，不必凭空造。
        if (record.v === undefined && record.t === undefined && !record.s) continue
        next[col] = record
        nextCells[row] = next
        sheetChanged = true
      }
    }
    if (!sheetChanged) continue
    sheets[sheetId] = { ...base, cellData: nextCells }
    changed = true
  }
  return changed ? ({ ...snapshot, sheets } as T) : snapshot
}

/** 尺寸 + 样式一起合并（打印预览的唯一入口）。 */
export function mergeLivePrint<T extends LayoutBook>(
  snapshot: T,
  baseline: { sizes?: SizeBaseline | null; styles?: StyleBaseline | null } | null | undefined,
  live: LiveWorkbook | null | undefined,
): T {
  const withSizes = mergeLiveLayout(snapshot, baseline?.sizes, live)
  return mergeLiveStyles(withSizes, baseline?.styles, live)
}
