/*
 * ProbeDisp.java -- dispValue（显示值）覆盖缺口探针
 *
 * 为什么单独一个文件：`ProbeCalc.java` 是 P0-2 的八阶段链路驱动，把新假设塞进去
 * 会超 300 行门禁（团队的 Java 单文件硬上限）。本探针只回答一个问题：
 *
 *   **`INormalCell.setDispValue(x)` 写进去的显示值，能不能走到计算态 / 二进制？**
 *
 * 背景（本探针存在的理由）：整个夹具语料 11 个 `.expected.json` 里
 * `disp_value`/`dispValue` 出现 **0 次**。而 rpx-core 有 `cell.disp_value`
 * （`cell.rs:67`，从 DISPVALUE 槽 `read_string`），`golden.rs:99` 与
 * `common/loader.rs:159` 都建模了它 —— 三处断言全是 `if let Some(..)`，
 * 语料里没有非空值，等于**永真式**。一个静默丢槽的解析器不会被任何测试发现。
 *
 * 四个待证点：
 *   DISP-1  模板态 dispValue 是否存活到 ExtCellSet（字面格 / 表达式格）
 *   DISP-2  只设 dispValue、value 为 null 的格，是否算非空（关系到 is_blank 契约）
 *   DISP-3  .rpx 写→读 是否原样携带 dispValue（二进制槽是否真的序列化）
 *   DISP-4  纵向扩展格的 dispValue 是否下发给附属格
 *
 * 输出：所有数字打到 stdout，异常栈打到 stderr。用法：
 *   java -cp "lib/*;out" rpx.fixtures.ProbeDisp .
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.util.ReportUtils;

final class ProbeDisp {

	private static final String[] REGIONS = { "North", "East", "South" };
	private static final int[] QTYS = { 100, 200, 150 };
	/** 逐格对拍的坐标（模板态与回读态共用）。 */
	private static final int[][] PROBE_CELLS = { { 2, 1 }, { 2, 2 }, { 2, 3 }, { 2, 4 }, { 3, 1 } };

	private ProbeDisp() {
	}

	public static void main(String[] args) {
		String dir = args.length > 0 ? args[0] : ".";
		int rc;
		try {
			rc = run(dir);
		} catch (Throwable t) {
			rc = CalcProbe.fail("DISP", t);
		}
		System.out.println(rc == 0 ? "[DISP] exit 0" : "[DISP] exit " + rc);
	}

	private static int run(String dir) throws Exception {
		ReportDefine2 rd = model();

		System.out.println("=== DISP-0 模板态：写进去的值 / 显示值 ===");
		for (int[] p : PROBE_CELLS) {
			System.out.println("  (" + p[0] + "," + p[1] + ") " + desc(rd.getCell(p[0], p[1])));
		}

		// ---------------- DISP-1 / DISP-2 / DISP-4 ----------------
		System.out.println();
		System.out.println("=== DISP-1 计算态：dispValue 是否存活 ===");
		IReport rpt = CalcProbe.calc(rd, CalcProbe.memDataSet("ds1", REGIONS, QTYS));
		int alive = 0;
		if (rpt instanceof ExtCellSet) {
			ExtCellSet ecs = (ExtCellSet) rpt;
			int rows = ecs.getRowCount();
			int cols = ecs.getColCount();
			System.out.println("  grid " + rows + " x " + cols);
			for (int r = 1; r <= rows; r++) {
				for (int c = 1; c <= cols; c++) {
					INormalCell cell = ecs.getCell(r, c);
					if (cell == null) {
						continue;
					}
					Object v = cell.getValue();
					String dv = cell.getDispValue();
					if (v == null && dv == null) {
						continue;
					}
					boolean shown = dv != null && !dv.equals(String.valueOf(v));
					System.out.println("  (" + r + "," + c + ") " + desc(cell)
							+ (shown ? "   <== 非平凡显示值" : ""));
					if (shown) {
						alive++;
					}
				}
			}
			dumpSubs(ecs);
		} else {
			System.out.println("  calc returned " + (rpt == null ? "null" : rpt.getClass().getName()));
		}
		System.out.println("  DISP-1 判决：非平凡 dispValue 存活格数 = " + alive
				+ (alive > 0 ? "（存活）" : "（未存活：计算态不保留显示值）"));

		// ---------------- DISP-2 ----------------
		System.out.println();
		System.out.println("=== DISP-2 只设 dispValue、value=null 的格 (2,3) ===");
		System.out.println("  模板态：" + desc(rd.getCell(2, 3)));
		if (rpt instanceof ExtCellSet) {
			INormalCell cc = ((ExtCellSet) rpt).getCell(2, 3);
			System.out.println("  计算态：" + (cc == null ? "null（该格被丢弃）" : desc(cc)));
		}

		// ---------------- DISP-3 ----------------
		System.out.println();
		System.out.println("=== DISP-3 .rpx 写→读 是否携带 dispValue ===");
		String path = dir + "/out/disp_probe.rpx";
		ReportUtils.write(path, rd);
		java.io.File f = new java.io.File(path);
		System.out.println("  写出 " + f.getPath() + " " + f.length() + " 字节");
		ReportDefine2 back = (ReportDefine2) ReportUtils.read(path);
		int round = 0;
		for (int[] p : PROBE_CELLS) {
			INormalCell o = rd.getCell(p[0], p[1]);
			INormalCell b = back.getCell(p[0], p[1]);
			boolean ok = same(o, b);
			if (ok) {
				round++;
			}
			System.out.println("  (" + p[0] + "," + p[1] + ") 写[" + desc(o) + "] 读[" + desc(b) + "]"
					+ (ok ? "  OK" : "  MISMATCH"));
		}
		System.out.println("  DISP-3 判决：回读一致 " + round + "/" + PROBE_CELLS.length + " 格");

		return (alive > 0 && round == PROBE_CELLS.length) ? 0 : 1;
	}

	/** DISP-4：纵向扩展锚点 (3,1) 的附属格是否各自带显示值。 */
	private static void dumpSubs(ExtCellSet ecs) {
		ExtCell src = ecs.getSource(3, 1);
		if (src == null) {
			System.out.println("  DISP-4 锚点(3,1) 无 source（未扩展）");
			return;
		}
		System.out.println("  DISP-4 锚点(3,1) extend=" + src.getExtensible()
				+ " subCells=" + src.extCellSize()
				+ " rowMerge=" + src.getRowMerge() + " colMerge=" + src.getColMerge());
		for (int i = 0; i < src.extCellSize(); i++) {
			ExtCell sub = src.getExtCell(i);
			if (sub == null) {
				System.out.println("    sub[" + i + "] == null");
				continue;
			}
			INormalCell sc = ecs.getCell(sub.getRow(), sub.getCol());
			System.out.println("    sub[" + i + "] (" + sub.getRow() + "," + sub.getCol()
					+ ") value=" + CalcProbe.repr(sub.getValue(false))
					+ " dispValue=" + CalcProbe.repr(sc == null ? null : sc.getDispValue()));
		}
	}

	/** 4x4 模板：4 种 dispValue 形态 + 1 个纵向扩展锚点。 */
	private static ReportDefine2 model() {
		ReportDefine2 rd = new ReportDefine2(4, 4);
		EngineModels.lit(rd, 1, 1, "A");
		EngineModels.lit(rd, 1, 2, "B");
		EngineModels.lit(rd, 1, 3, "C");
		EngineModels.lit(rd, 1, 4, "D");

		EngineModels.lit(rd, 2, 1, "North");
		disp(rd, 2, 1, "北区");                     // 字面字符串 + 中文显示值
		EngineModels.lit(rd, 2, 2, Integer.valueOf(100));
		disp(rd, 2, 2, "100.00");                   // 字面整数 + 两位小数显示值
		disp(rd, 2, 3, "仅显示值");                 // 只有显示值，没有值
		CalcProbe.setExp(rd.getCell(2, 4), "6*1");
		disp(rd, 2, 4, "6 件");                     // 表达式格 + 显示值
		CalcProbe.setExp(rd.getCell(3, 1), "ds1.select(qty)");
		disp(rd, 3, 1, "Q");                        // 纵向扩展锚点 + 显示值
		CalcProbe.attachFileDataSet(rd, "ds1", EngineModels.csvName("engine_sum"));
		return rd;
	}

	private static void disp(ReportDefine2 rd, int r, int c, String dv) {
		INormalCell cell = rd.getCell(r, c);
		if (cell == null) {
			System.out.println("  !! getCell(" + r + "," + c + ") == null，无法写入 dispValue");
			return;
		}
		cell.setDispValue(dv);
	}

	private static String desc(INormalCell cell) {
		if (cell == null) {
			return "<null 格>";
		}
		Object v = cell.getValue();
		return "value=" + CalcProbe.repr(v)
				+ " type=" + (v == null ? "null" : v.getClass().getSimpleName())
				+ " dispValue=" + CalcProbe.repr(cell.getDispValue());
	}

	private static boolean same(INormalCell a, INormalCell b) {
		if (a == null || b == null) {
			return a == b;
		}
		return eq(a.getValue(), b.getValue()) && eq(a.getDispValue(), b.getDispValue());
	}

	private static boolean eq(Object a, Object b) {
		return a == null ? b == null : a.equals(b);
	}
}
