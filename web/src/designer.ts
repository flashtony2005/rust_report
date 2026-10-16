/**
 * 编辑面（M7-W W2-1）：Univer 里的改动 → 定义 JSON 的**内容补丁** + 导出。
 *
 * ## 这一层只做「内容」，且必须守住四条服务端契约
 *
 * 设计器**不重建 JSON、只打补丁**（理由见 `stylemap.ts` 文件头：快照有损）。
 * 内容补丁比样式补丁更危险，因为 JSON 的 `cells[]` 有**结构约束**，写错一个字段
 * 服务端就 400。本模块把四条约束全部前置成显式错误：
 *
 * | # | 约束 | 出处 | 违反的后果 |
 * | --- | --- | --- | --- |
 * | 1 | **坐标系**：JSON 是**逻辑坐标（1 起）**，Univer 快照是**输出坐标（0 起）** | `report-def-json.md` §4.1；实证 `snapshot/tests.rs`：`engine_merge` 的逻辑 `(2,1)纵2` → 快照 `startRow=1,startColumn=0,endRow=2` | 差一格 ⇒ 内容落到隔壁格，**不报错** |
 * | 2 | **`text` 与 `expr` 互斥**，都给报错 | `report-def-json.md` §3.2 | 用 `text` 覆盖 `expr` = **静默切断数据绑定** |
 * | 3 | **合并矩形内的非锚点槽不能带内容**（带 `text`/`expr` 报错；空壳才容忍） | 同上 + `reportdef/parse.rs` 两趟校验 | 服务端 400，前端却显示成功 |
 * | 4 | **行列号必须在 `grid` 范围内**（1..=`grid.rows`） | 同上 §3.2 | 越界 400 |
 *
 * ## 为什么不整体同步、只跟用户真改过的事件
 *
 * Univer 会把 `text: "0123"` 这类值**按数值渲染**（快照里 `{"t":2,"v":100}`）。
 * 若做「快照 → JSON」整体回灌，没被用户碰过的格子也会被归一化
 * （`"0123"` → `"123"`），那是**静默数据损坏**。
 * ⇒ 本模块的入口只有 [`applyValueEdit]`，由 `SheetValueChanged` 事件驱动 ——
 * **只补用户真改过的那一格**。这条纪律由 `designer.test.ts` 钉住。
 *
 * ⚠ `.rpx` 模板**没有**定义 JSON 可持 ⇒ 编辑面不可用，必须显式告知
 * （[`assertEditable]`），不能退化成「改了但存不下去」。
 */
import type { CellDef } from './stylemap'

// ---------------------------------------------------------------------------
// 类型
// ---------------------------------------------------------------------------

/** 定义 JSON 的顶层（只登记本模块要动的槽；其余原样保留、原样导出）。 */
export type ReportDef = {
  version?: number
  grid: { rows: number; cols: number }
  cells?: CellDef[]
  [k: string]: unknown
}

/** Univer 递给我们的单元格值（`SheetValueChanged` 的 `newValue`）。 */
export type CellValue = string | number | boolean | null | undefined

/** 补丁结果。`error` 与 `def` **互斥**：出错就不产出 JSON（不猜、不半成品）。 */
export type EditResult = {
  def?: ReportDef
  error?: string
  warnings: string[]
}

// ---------------------------------------------------------------------------
// 坐标系（约束 1）
// ---------------------------------------------------------------------------

/** Univer 0 起 → 定义 JSON 1 起。**两轴各 +1**。
 *
 * 实证（`crates/rpx-server/src/snapshot/tests.rs`）：`engine_merge` 的
 * `North (2,1)纵2` 在快照里是 `startRow=1, startColumn=0, endRow=2, endColumn=0`
 * ⇒ 锚点 `(2,1)` → `(1,0)`，2 行高的矩形终点 `endRow=2`（0 起**含**端点）。 */
export function toLogical(row0: number, col0: number): { row: number; col: number } {
  return { row: row0 + 1, col: col0 + 1 }
}

/** 定义 JSON 1 起 → Univer 0 起（导出/定位用，[`toLogical] 的逆）。 */
export function toUniver(row: number, col: number): { row: number; col: number } {
  return { row: row - 1, col: col - 1 }
}

/** 集合键：逻辑坐标 → 字符串（`Set` 需要可哈希键）。
 *
 * ⚠ 导出给 `stylepanel.ts` 共用：两处若各写各的键格式，`coveredSlots` 的命中
 * 判定就会静默错位（同坐标系两套键 = 隐蔽 bug）。 */
export const slotKey = (row: number, col: number): string => `${row},${col}`

/**
 * 算出**被他人合并矩形覆盖的非锚点槽**（逻辑坐标集合）。
 *
 * 逐字镜像服务端 `reportdef/parse.rs` 的两趟校验：先扫全部 `merge` 声明收矩形，
 * 再把「矩形内且不是锚点」的槽记进集合。服务端对落在这些槽上、**带内容**的
 * `cells[]` 条目**显式报错** ⇒ 前端若往这些槽写内容，服务端必然 400。
 *
 * ⚠ 声明顺序无关（服务端也是两趟）：非锚点槽可以先于锚点出现在 `cells[]` 里。
 */
export function coveredSlots(def: ReportDef): Set<string> {
  const covered = new Set<string>()
  for (const cell of def.cells ?? []) {
    const merge = cell.merge
    if (!merge) continue
    const { rows, cols } = merge
    for (let r = cell.row; r < cell.row + rows; r++) {
      for (let c = cell.col; c < cell.col + cols; c++) {
        if (r !== cell.row || c !== cell.col) covered.add(slotKey(r, c))
      }
    }
  }
  return covered
}

/** 找 `cells[]` 里逻辑坐标 (row, col) 那条的下标；找不到返回 `-1`。
 *
 * ⚠ **不假设唯一**：服务端对重复 (row,col) 的行为是后写覆盖（`Builder::put`），
 * 但本函数返回**第一个**，并在 `applyValueEdit` 里对重复显式告警 ——
 * 静默改第一条会让用户以为改的是另一条。 */
export function findCellIndex(def: ReportDef, row: number, col: number): number {
  return (def.cells ?? []).findIndex((c) => c.row === row && c.col === col)
}

// ---------------------------------------------------------------------------
// 可编辑性前置检查（约束 2/3/4 与 .rpx 边界）
// ---------------------------------------------------------------------------

/** 编辑面前置检查：**能不能编辑**。不满足就给出人话错误，不猜。
 *
 * `.rpx` 模板没有定义 JSON 可持 —— 这是**能力边界**，不是错误状态，
 * 所以文案要说清「为什么不能」而不是只说「不能」。 */
export function assertEditable(source: 'json' | 'rpx' | null): string | null {
  if (source === 'json') return null
  if (source === 'rpx') {
    return '当前是 .rpx 二进制模板，编辑面只支持定义 JSON（.json）——请改用 .json 报表源'
  }
  return '还没有可编辑的报表：请先选 .json 报表源并渲染'
}

// ---------------------------------------------------------------------------
// 内容补丁（唯一入口）
// ---------------------------------------------------------------------------

/** 把 Univer 里**一格**的值改动补丁进定义 JSON，返回**新**对象（纯函数）。
 *
 * @param def   当前持有的定义 JSON（不会被修改）
 * @param row0  Univer 0 起行号
 * @param col0  Univer 0 起列号
 * @param value Univer 给出的新值；`null`/`undefined`/空串 = 用户清空了这一格
 *
 * 口径：
 * - **只补这一格**（不整体回灌，理由见文件头）；
 * - 清空 ⇒ **删掉 `text` 键**（`text` 与 `expr` 都缺 = 合法空白格，§3.2），
 *   而不是写 `text: ""` —— 后者会让 JSON 多一个语义上等于缺省的键；
 * - 该格原本没有 `cells[]` 条目 ⇒ **追加**一条 `{row, col, text}`；
 * - 数值/布尔一律 `String()`：JSON 的 `text` 契约类型是**字符串**（§3.2）。 */
export function applyValueEdit(
  def: ReportDef,
  row0: number,
  col0: number,
  value: CellValue,
): EditResult {
  const warnings: string[] = []
  const { row, col } = toLogical(row0, col0)
  const at = `(${row0},${col0})` // 报错用 **Univer 坐标** —— 用户看见的是这个

  // ---- 约束 4：网格边界 ----
  if (row < 1 || col < 1 || row > def.grid.rows || col > def.grid.cols) {
    return {
      error: `位置 ${at} 超出网格（grid 是 ${def.grid.rows} 行 × ${def.grid.cols} 列，逻辑坐标 1 起）`,
      warnings,
    }
  }

  // ---- 约束 3：合并矩形内的非锚点槽不得带内容 ----
  if (coveredSlots(def).has(slotKey(row, col))) {
    return {
      error: `位置 ${at} 落在某个合并矩形内部（非锚点）——服务端禁止这些槽带内容，请改矩形的左上角那一格`,
      warnings,
    }
  }

  const cells = def.cells ? [...def.cells] : []
  const idx = findCellIndex(def, row, col)

  // ---- 约束 2：不得用 text 覆盖 expr ----
  if (idx >= 0) {
    const cell = cells[idx]
    if (typeof cell.expr === 'string' && cell.expr) {
      return {
        error: `位置 ${at} 由表达式 \`${cell.expr}\` 驱动——直接改内容会切断数据绑定；要改成静态文本请先在 JSON 里删掉 expr`,
        warnings,
      }
    }
    if (cells.filter((c) => c.row === row && c.col === col).length > 1) {
      warnings.push(`位置 ${at} 在 cells[] 里有重复条目，只改了第一条`)
    }
  }

  const text = value === null || value === undefined ? '' : String(value)

  const next: ReportDef = { ...def }
  if (idx >= 0) {
    const cell = cells[idx]
    if (text === '') {
      // 清空 ⇒ 删键（语义 = 回到契约缺省），不写空串。
      const { text: _drop, ...rest } = cell
      cells[idx] = rest as CellDef
    } else {
      // 写了内容就必须**同时清掉 expr**：两者互斥（§3.2），留着就 400。
      // 能走到这里说明上面已确认原本没有 expr，故这里只是防御。
      const { expr: _drop, ...rest } = cell
      cells[idx] = { ...(rest as CellDef), text }
    }
    next.cells = cells
  } else if (text !== '') {
    // 原本没有条目 ⇒ 追加。**只在真有内容时追加** —— 清空一个不存在的格
    // 不该凭空造出一条空壳。
    next.cells = [...cells, { row, col, text }]
  } else {
    next.cells = cells
  }

  return { def: next, warnings }
}

// ---------------------------------------------------------------------------
// 导出
// ---------------------------------------------------------------------------

/** 导出定义 JSON 文本（2 空格缩进，可直接上传 `/render` 或「另存为」）。
 *
 * 纯 `JSON.stringify`：键序 = 补丁产生的插入序（可读、稳定），
 * 而服务端 `serde_json` 不依赖键序 ⇒ 无兼容性问题。
 * ⚠ 末尾**不带换行**：与上传口径无关，但避免「看起来多个空行」的噪音。 */
export function exportDefJson(def: ReportDef): string {
  return JSON.stringify(def, null, 2)
}

/** 浏览器下载定义 JSON（编辑面的唯一出口）。 */
export function downloadDefJson(def: ReportDef, name: string): void {
  const blob = new Blob([exportDefJson(def)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = name.endsWith('.json') ? name : `${name}.json`
  a.click()
  // 立刻回收：不 revoke 会让每次导出漏一个 object URL（长会话会攒）。
  URL.revokeObjectURL(url)
}
