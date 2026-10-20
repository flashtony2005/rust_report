/**
 * Univer 边框（`styles.bd` / `cell.s.bd`）→ 打印用 CSS 边框。
 *
 * # 为什么要单独一层
 *
 * 打印预览是**自研 HTML 渲染**（不是 Univer 的 canvas），所以 Univer 里画的边框
 * 不会自己出现在纸上 —— 必须把 `s.bd` 翻译成 CSS。这段翻译有两个特点：
 *
 * - **稠密枚举**：Univer `BorderStyleTypes` 有 14 档（THIN / MEDIUM / THICK / DOUBLE /
 *   各种虚线），CESS 只认 `solid|dashed|dotted|double`，且**没有线宽字段** ⇒
 *   「档位 + 线型」只能还原成固定的 `宽度 线型` 组合；
 * - **两种写法**：颜色槽是 `cl`（`IBorderStyleData`），但早期快照里有 `c` 的写法，
 *   两个都认，都没有就按黑色（不因为缺色丢掉整条线）。
 *
 * 与 `crates/rpx-runtime/src/print_html.rs::border_side_css` 是**同一张表** ——
 * 屏幕预览和导出 PDF 必须长得一样，改一边就要改另一边。
 */

/**
 * 四边各自的 CSS 值（`undefined` = 这一边没有边框）。
 *
 * ⚠ 键名是 React 的 **`borderTop` / `borderRight` / …**，不是 `top` / `right` / …：
 * 后者展开进 `style` 会被当成**定位偏移**（`style.top = '1pt solid #F00'` 是无效值，
 * 边框静默不生效）。首版就是这么踩的 —— 预览里一条线都没有。
 */
export interface BorderSidesCss {
  borderTop?: string
  borderRight?: string
  borderBottom?: string
  borderLeft?: string
}

/** Univer `BorderStyleTypes` → `"宽度 线型"`。CSS 没有点划线，`DASH_DOT` 系列退化成虚线。 */
const UNIVER_BORDER_CSS: Record<number, string> = {
  1: '0.75pt solid', // THIN
  2: '0.5pt solid', // HAIR
  3: '1pt dotted', // DOTTED
  4: '1pt dashed', // DASHED
  5: '1pt dashed', // DASH_DOT
  6: '1pt dashed', // DASH_DOT_DOT
  7: '2.25pt double', // DOUBLE
  8: '1.5pt solid', // MEDIUM
  9: '1.5pt dashed', // MEDIUM_DASHED
  10: '1.5pt dashed', // MEDIUM_DASH_DOT
  11: '1.5pt dashed', // MEDIUM_DASH_DOT_DOT
  12: '1.5pt dashed', // SLANT_DASH_DOT
  13: '2.25pt solid', // THICK
}

/** 四边槽位名：Univer 用 `t/r/b/l`，CSS 用 `top/right/bottom/left`（名字不同，别照抄）。 */
/** `[CSS 键, Univer 槽位]` 成对出现，避免手写两份顺序不同的数组。 */
const SIDE_KEYS = [
  ['borderTop', 't'],
  ['borderRight', 'r'],
  ['borderBottom', 'b'],
  ['borderLeft', 'l'],
] as const

/** 取一条边的颜色：`cl.rgb`（标准）→ `c.rgb`（旧写法）→ 黑。 */
function sideColor(edge: { cl?: { rgb?: unknown }; c?: { rgb?: unknown } }): string {
  const rgb = edge.cl?.rgb ?? edge.c?.rgb
  return typeof rgb === 'string' && rgb ? rgb : '#000000'
}

/**
 * `s.bd` → 四边 CSS。
 *
 * @param bd Univer 的 `IBorderData`（可以是 `undefined` / 非对象 —— 拿不到就返回空）
 * @returns 只包含**真正有边框**的边；`s` 为 `none`(0) 或未知码的边会被略过
 */
export function borderSides(bd: unknown): BorderSidesCss {
  const out: BorderSidesCss = {}
  if (!bd || typeof bd !== 'object' || Array.isArray(bd)) return out
  const source = bd as Record<string, unknown>
  for (const [styleKey, key] of SIDE_KEYS) {
    const edge = source[key]
    if (!edge || typeof edge !== 'object') continue
    const record = edge as { s?: unknown; cl?: { rgb?: unknown }; c?: { rgb?: unknown } }
    const base = typeof record.s === 'number' ? UNIVER_BORDER_CSS[record.s] : undefined
    // `0`(none) 与未知码都落这里：宁可不画，也不把「无边框」画成默认细线。
    if (!base) continue
    out[styleKey] = `${base} ${sideColor(record)}`
  }
  return out
}

/**
 * 把四边边框叠上「打印网格线」兜底，产出 `<td>` 要的四条 CSS。
 *
 * 逐边兜底而不是用一个 `border` 简写：简写会把用户画的边框整片覆盖掉，
 * 而用户只画了上边框时，其余三边仍应保留网格线。
 *
 * @param bd    `s.bd`
 * @param grid  `printOptions.borderWidth > 0` 时的网格线声明（`"1pt solid #000"`）
 */
export function cellBorders(bd: unknown, grid?: string): BorderSidesCss {
  const sides = borderSides(bd)
  if (!grid) return sides
  return {
    borderTop: sides.borderTop ?? grid,
    borderRight: sides.borderRight ?? grid,
    borderBottom: sides.borderBottom ?? grid,
    borderLeft: sides.borderLeft ?? grid,
  }
}
