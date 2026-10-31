/**
 * 工作簿快照的类型（引擎产物 `workbook.json` 的结构）。
 *
 * 来源：master 分支 `src/types.ts:113-169`。结构就是 Univer 的 `IWorkbookData`，
 * 但**这里不依赖 Univer 运行时** —— 只读渲染由 `components/WorkbookViewer.tsx` 自己做。
 *
 * 两条实测事实（对着 `examples/library/.runs/<run>/artifacts/workbook.json` 验过）：
 * - `sheets` 是 **id → 页** 的字典，**顺序**另由 `sheetOrder` 给出（两处顺序可以不一致）；
 * - 单元格 `s` 一般是**内联对象**（引擎直出）；但设计器保存过的快照里可能是
 *   `styles` 表里的**字符串 id**，所以类型写成联合，渲染前必须 resolve（见 `resolveStyle`）。
 */

/** 单元格样式（Univer 的简写键，与引擎 `style_json` 输出一致）。 */
export interface CellStyle {
  /** 加粗（1 = 是） */
  bl?: number;
  /** 倾斜（1 = 是） */
  it?: number;
  ul?: { s?: number };
  /** 字体族 */
  ff?: string;
  /** 字号 */
  fs?: number;
  /** 文字色 */
  cl?: { rgb?: string };
  /** 背景色 */
  bg?: { rgb?: string };
  /** 水平对齐：1 左 / 2 中 / 3 右 */
  ht?: number;
  /** 垂直对齐：1 上 / 2 中 / 3 下 */
  vt?: number;
  /** 换行（1 = 自动换行） */
  tb?: number;
  /** 边框：四边各自可以没有 */
  bd?: Partial<Record<'b' | 't' | 'l' | 'r', { s?: number; cl?: { rgb?: string } }>>;
  /** 数字格式 */
  n?: { pattern?: string };
}

export interface CellData {
  /** 值。数字/布尔保持原类型 —— 右对齐靠它判断，不要一律转字符串 */
  v?: string | number | boolean;
  /** 类型：1 文本 / 2 数字 / 3 布尔 */
  t?: number;
  /** 样式：内联对象，或 `styles` 表里的 id */
  s?: CellStyle | string;
  /** 公式原文（只读渲染不重算，展示用） */
  f?: string;
}

export interface SheetData {
  id: string;
  name: string;
  rowCount: number;
  columnCount: number;
  defaultRowHeight?: number;
  defaultColumnWidth?: number;
  /** 1 = 显示网格线 */
  showGridlines?: number;
  freeze?: { xSplit?: number; ySplit?: number; startRow?: number; startColumn?: number };
  /** 行号 → 列号 → 单元格。键是**字符串**（JSON 对象的键） */
  cellData: Record<string, Record<string, CellData>>;
  mergeData?: Array<{
    startRow: number;
    endRow: number;
    startColumn: number;
    endColumn: number;
  }>;
  /** 行号 → 高度（px） */
  rowData?: Record<string, { h?: number }>;
  /** 列号 → 宽度（px） */
  columnData?: Record<string, { w?: number }>;
}

export interface WorkbookData {
  id: string;
  name: string;
  appVersion?: string;
  locale?: string;
  /** 页签顺序（**不能**用 `Object.keys(sheets)` 代替） */
  sheetOrder: string[];
  sheets: Record<string, SheetData>;
  styles?: Record<string, CellStyle>;
}

/**
 * 把 `s` 解析成真正的样式对象。
 *
 * 为什么要这一步：设计器保存的 workbook 里 `s` 可能是 `"3"` 这样的 id，
 * 直接当对象读会拿到 `undefined`，整张表就变成没有格式的纯文本 —— 看着像"样式丢了"。
 */
export function resolveStyle(
  cell: CellData | undefined,
  styles: Record<string, CellStyle> | undefined,
): CellStyle | undefined {
  if (!cell) return undefined;
  const s = cell.s;
  if (!s) return undefined;
  if (typeof s === 'string') return styles?.[s];
  return s;
}

/** 列号（0 起）→ 表格列名（A、B…AA）。 */
export function colToLetter(col: number): string {
  let n = col;
  let out = '';
  while (n >= 0) {
    out = String.fromCharCode((n % 26) + 65) + out;
    n = Math.floor(n / 26) - 1;
  }
  return out;
}

/** 右下角有内容的范围（用于"这张表到底有多大"，别按 100×26 全渲染）。 */
export function usedRange(sheet: SheetData): { rows: number; cols: number } {
  let rows = 0;
  let cols = 0;
  for (const [r, row] of Object.entries(sheet.cellData ?? {})) {
    const ri = Number(r);
    if (!Number.isFinite(ri)) continue;
    for (const c of Object.keys(row)) {
      const ci = Number(c);
      if (!Number.isFinite(ci)) continue;
      if (ri + 1 > rows) rows = ri + 1;
      if (ci + 1 > cols) cols = ci + 1;
    }
  }
  return { rows, cols };
}
