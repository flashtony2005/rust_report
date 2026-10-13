/*
 * ProbeHeadMap.java -- §12.1 第 5/6 步（head 图）的**真机基线**
 *
 * 为什么必须先取基线：head 图是当前唯一有「写错了但测不出」风险的部分 ——
 * `_$3`（左 head）与 `_$2`（上 head）都是"从邻格向上/向左**递减**扫，命中第一个就返回"，
 * 一旦方向写反、或者把 `sameRowTypeAs` 的两处不对称写漏，**结果依然是一张合法的 head 图**，
 * 没有任何断言会失败。所以下面的数字必须先落到盘上，当作实现的对照基线。
 *
 * 只读不写：不碰 `src/**`，只 dump。
 * 用法：java -cp "lib/*;out" rpx.fixtures.ProbeHeadMap
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.util.ReportUtils;

public class ProbeHeadMap {

	private static final String[] NAMES = { "engine_sum", "engine_ext", "engine_expr",
			"engine_pager", "engine_merge", "engine_cycle", "engine_samemerge",
			"engine_disp", "engine_types", "engine_rowheight",
			"engine_head_top", "engine_head_left" };

	public static void main(String[] args) {
		System.out.println("=== 0. 基线事实（源码侧，先读懂再对照数字）===");
		System.out.println("  ExtCellSet.java:776-777  cell.setCellLeftHead(_$3(cell)); cell.setCellTopHead(_$2(cell));");
		System.out.println("  _$3（左 head）:648-665  for(col=cell.col-1; col>=1; --col)  递减、不含 0");
		System.out.println("                          命中条件 containRow(cell) && 18==extend；**无 sameRowTypeAs**");
		System.out.println("                          兜底 return _$10（cell00）");
		System.out.println("  _$2（上 head）:685-704  for(row=cell.row-1; row>=1; --row)  递减、不含 0");
		System.out.println("                          首分支 sameRowTypeAs && containCol && 19==extend；:695 再判一次 sameRowTypeAs");
		System.out.println("                          兜底 return _$10（cell00）");
		System.out.println("  ExtExtendCell._$5=topSubs、_$6=leftSubs（:69-76）；基类 ExtCell 的两个 size 都 return 0");

		System.out.println();
		System.out.println("=== 1. 逐夹具逐格 head 基线 ===");
		for (String name : NAMES) {
			dump(name);
		}

		System.out.println();
		System.out.println("=== 1b. 坐标口径核对：格自身 (r,c) vs src，以及 head 目标的自身 vs src ===");
		coordAudit();

		System.out.println();
		System.out.println("=== 2. cell00 的 sub 成员数（_$5 topSubs / _$6 leftSubs）===");
		for (String name : NAMES) {
			cell00(name);
		}
	}

	/** 一个夹具的逐格 head；顺带列出 leftHead != cell00 的反例。 */
	private static void dump(String name) {
		ExtCellSet ecs = calc(name);
		if (ecs == null) {
			return;
		}
		ExtCell c00 = ecs.getCell00();
		int rows = ecs.getRowCount();
		int cols = ecs.getColCount();
		System.out.println("--- " + name + "  grid=" + rows + "x" + cols
				+ "  cell00=(" + c00.getRow() + "," + c00.getCol() + ") class=" + c00.getClass().getSimpleName());
		int notCell00Left = 0;
		int notCell00Top = 0;
		StringBuilder ex = new StringBuilder();
		for (int r = 1; r <= rows; r++) {
			StringBuilder line = new StringBuilder();
			for (int c = 1; c <= cols; c++) {
				ExtCell cell = ecs.getSource(r, c);
				if (cell == null) {
					continue;
				}
				ExtCell lh = cell.getCellLeftHead();
				ExtCell th = cell.getCellTopHead();
				boolean lIs00 = lh == c00;
				boolean tIs00 = th == c00;
				if (!lIs00) {
					notCell00Left++;
					ex.append(" (" + r + "," + c + ")→L" + at(lh) + srcAt(lh) + " extend="
							+ (lh == null ? "?" : lh.getExtensible()));
				}
				if (!tIs00) {
					notCell00Top++;
					ex.append(" (" + r + "," + c + ")→T" + at(th) + srcAt(th) + " extend="
							+ (th == null ? "?" : th.getExtensible()));
				}
				if (line.length() == 0) {
					line.append("  r" + r + ": ");
				}
				line.append(c + "[" + (lIs00 ? "00" : at(lh)) + "/" + (tIs00 ? "00" : at(th))
						+ "] ");
			}
			System.out.println(line);
		}
		System.out.println("  leftHead != cell00 的格数 = " + notCell00Left
				+ " ；topHead != cell00 的格数 = " + notCell00Top);
		if (ex.length() > 0) {
			System.out.println("  ⛔ 反例（head 不是 cell00）：" + ex);
		}
	}

	/** cell00 的 sub 成员数与运行时类型。 */
	private static void cell00(String name) {
		ExtCellSet ecs = calc(name);
		if (ecs == null) {
			return;
		}
		ExtCell c00 = ecs.getCell00();
		System.out.println("  " + pad(name) + " class=" + pad(c00.getClass().getSimpleName())
				+ " leftSubSize(_$6)=" + c00.getLeftSubSize()
				+ " topSubSize(_$5)=" + c00.getTopSubSize());
	}

	private static ExtCellSet calc(String name) {
		try {
			IReport read = ReportUtils.read(name + ".rpx");
			return (ExtCellSet) CalcProbe.calc((ReportDefine2) read, null);
		} catch (Throwable t) {
			System.out.println("--- " + name + " 跳过：" + t.getClass().getSimpleName());
			return null;
		}
	}

	/** head 的**模板出处**坐标：插入行会让 head 对象自身的 row/col 与模板行号不同。 */
	private static String srcAt(ExtCell c) {
		return c == null ? "" : "{src=" + c.getSourceRow() + "," + c.getSourceCol() + "}";
	}

	/** 统计两件事：① 每个格的 calc 坐标 vs 自己的 src；② 被引用的 head 目标自身坐标 vs src。 */
	private static void coordAudit() {
		int cellTotal = 0, cellDiff = 0, headTotal = 0, headDiff = 0;
		for (String name : NAMES) {
			ExtCellSet ecs = calc(name);
			if (ecs == null) {
				continue;
			}
			java.util.Set<String> heads = new java.util.HashSet<String>();
			for (int r = 1; r <= ecs.getRowCount(); r++) {
				for (int c = 1; c <= ecs.getColCount(); c++) {
					ExtCell cell = ecs.getCell(r, c) instanceof ExtCell
							? (ExtCell) ecs.getCell(r, c) : null;
					if (cell == null) {
						continue;
					}
					if (cell.getSource() != null) {
						cellTotal++;
						if (cell.getRow() != cell.getSourceRow() || cell.getCol() != cell.getSourceCol()) {
							cellDiff++;
						}
					}
					if (cell.getCellLeftHead() != null) {
						heads.add(key(cell.getCellLeftHead()));
					}
					if (cell.getCellTopHead() != null) {
						heads.add(key(cell.getCellTopHead()));
					}
				}
			}
			for (String k : heads) {
				String[] xy = k.split(",");
				ExtCell h = ecs.getCell(Integer.parseInt(xy[0]), Integer.parseInt(xy[1])) instanceof ExtCell
						? (ExtCell) ecs.getCell(Integer.parseInt(xy[0]), Integer.parseInt(xy[1])) : null;
				if (h == null) {
					continue;
				}
				headTotal++;
				if (h.getRow() != h.getSourceRow() || h.getCol() != h.getSourceCol()) {
					headDiff++;
					System.out.println("  head 目标自身≠src：自身=(" + h.getRow() + "," + h.getCol()
							+ ") src=(" + h.getSourceRow() + "," + h.getSourceCol() + ")  来自 " + name);
				}
			}
		}
		System.out.println("  ① 带 src 的格：" + cellTotal + " 个，其中 calc 坐标 != src 的 = " + cellDiff);
		System.out.println("  ② 被引用的 head 目标：" + headTotal + " 个，其中自身坐标 != src 的 = " + headDiff);
	}

	private static String key(ExtCell c) {
		return c.getRow() + "," + c.getCol();
	}

	private static String at(ExtCell c) {
		return c == null ? "null" : ("(" + c.getRow() + "," + c.getCol() + ")");
	}

	private static String pad(String s) {
		StringBuilder sb = new StringBuilder(s);
		while (sb.length() < 18) {
			sb.append(' ');
		}
		return sb.toString();
	}
}