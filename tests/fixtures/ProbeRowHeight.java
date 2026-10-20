/*
 * ProbeRowHeight.java -- 行高单位与分页口径标定（M5 §6.3 ⑦ 的前置探针）。
 *
 * 要回答的问题：
 *   Q1 `IRowCell.setRowHeight(float)` 的单位是 mm 还是 pt？
 *   Q2 `ReportUtils2.getPagerInfo(rpt, 595, 842, 1)` 的 595/842 是 pt 还是 px？
 *   Q3 每页能放几行 —— 只由高度决定，还是还扣了页眉/边距？
 *   Q4 `rowNumPerPage` 与真实每页行数的关系。
 *
 * 做法：单列 12 行、每行同一高度 H，跑一组 H（5/10/15/20/30/50/100/150），
 * 读 pageCount / rowNumPerPage / 每页实际行数，反解单位与可用高度。
 *
 * 用法（cwd = tests/fixtures）：
 *   javac --release 8 -cp "F:/project/report/reportlite/lib/*;out" ProbeRowHeight.java -d out
 *   F:/project/report/common/jre/bin/java.exe -cp "...;out" rpx.fixtures.ProbeRowHeight .
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.IReport;

public final class ProbeRowHeight {

	private static final float[] HEIGHTS = { 5f, 10f, 15f, 20f, 30f, 50f, 100f, 150f };

	public static void main(String[] args) throws Throwable {
		for (float h : HEIGHTS) {
			run("h" + (int) h, h);
		}
		// 混合高度：前 3 行 100，其余 20 —— 看是否按累计高度切页
		runMixed("mixed_100_20");
	}

	private static void run(String name, float height) throws Throwable {
		ReportDefine2 rd = new ReportDefine2(14, 3);
		for (int r = 2; r <= 13; r++) {
			rd.getRowCell(r).setRowHeight(height);
			EngineModels.lit(rd, r, 1, "r" + r);
		}
		report(name, rd, "uniform=" + height);
	}

	private static void runMixed(String name) throws Throwable {
		ReportDefine2 rd = new ReportDefine2(14, 3);
		for (int r = 2; r <= 13; r++) {
			float height = r <= 4 ? 100f : 20f;
			rd.getRowCell(r).setRowHeight(height);
			EngineModels.lit(rd, r, 1, "r" + r);
		}
		report(name, rd, "rows2-4=100, rows5-13=20");
	}

	private static void report(String name, ReportDefine2 rd, String note)
			throws Throwable {
		IReport rpt = CalcProbe.calc(rd, null, null);
		System.out.println("### " + name + " | " + note);
		CalcProbe.dumpPages(name, rpt, 595, 842, 1);
	}
}
