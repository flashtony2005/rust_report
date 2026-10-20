/*
 * ProbeHc2.java -- M5-09 第二批探针：同比布局（值格与差额格同带）+ $ 前缀。
 *
 * 教程例 `C2-C2[A2:-1]{$B2==B2}` 的真实布局（逐条对照 058/059）：
 *   A2 = 年（ds1 纵向扩展，2 实例）
 *   B2 = 月（ds2 纵向扩展，左主格 A2，每年 3 实例）—— 必须跨数据集：
 *        同一数据集里 A2 的 select 会把游标钉在组首行，B2 只能取到 1 行
 *   C2 = 值格（ds2 的 amount，B2 的附属格）
 *   D2 = 差额格（左主格 = B2，因为 C2 不是扩展格）⇒ $B2 才拿得到月
 * 预期：2025 行全额，2026 行 = 本年值 − 去年同月值（5/5/5）。
 *
 * 用法（cwd = tests/fixtures）：
 *   javac --release 8 -cp "F:/project/report/reportlite/lib/*;out" ProbeHc2.java -d out
 *   F:/project/report/common/jre/bin/java.exe -cp "...;out" rpx.fixtures.ProbeHc2 .
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;

public final class ProbeHc2 {

	private static final String YEARS = "hc2_years.csv";
	private static final String DATA = "hc2_data.csv";

	public static void main(String[] args) throws Exception {
		String dir = args.length > 0 ? args[0] : ".";
		writeCsvs(dir);

		run("A_layout", -1, -1, null, null);
		run("B_cond", -1, -1, null, "C2-C2[A2:-1]{$B2==B2}");
		run("C_nocond", -1, -1, null, "C2-C2[A2:-1]");
		run("D_saved", 8, 5, "$C2[A2:2,B2:1]", null);
		run("D_plain", 8, 5, "C2[A2:2,B2:1]", null);
	}

	private static void run(String name, int r, int c, String exp, String d2exp) {
		ReportDefine2 rd = base();
		if (exp != null && r > 0 && c > 0) {
			CalcProbe.setExp(rd.getCell(r, c), exp);
		}
		if (d2exp != null) {
			CalcProbe.setExp(rd.getCell(2, 4), d2exp);
		}
		try {
			IReport rpt = CalcProbe.calc(rd, null, null);
			System.out.println("### " + name + " | OK | exp=" + exp + " d2=" + d2exp);
			CalcProbe.dump(name, rpt);
		} catch (Throwable t) {
			System.out.println("### " + name + " | ERR | exp=" + exp + " d2=" + d2exp
					+ " | " + t.getClass().getName() + ": " + t.getMessage());
		}
	}

	private static ReportDefine2 base() {
		ReportDefine2 rd = new ReportDefine2(8, 5);
		EngineModels.lit(rd, 1, 1, "年\\月");
		INormalCell year = rd.getCell(2, 1);
		year.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(year, "ds1.select(year:1,,,year)");
		INormalCell month = rd.getCell(2, 2);
		month.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(month, "ds2.select(month, year==A2)");
		CalcProbe.setExp(rd.getCell(2, 3),
				"ds2.select(amount, year==A2 && month==B2)");
		CalcProbe.attachFileDataSet(rd, "ds1", YEARS);
		CalcProbe.attachFileDataSet(rd, "ds2", DATA);
		return rd;
	}

	private static void writeCsvs(String dir) throws Exception {
		FileIO.write(dir + "/" + YEARS, "year\r\nY2025\r\nY2026\r\n");
		StringBuilder data = new StringBuilder("year\tmonth\tamount\r\n");
		String[][] rows = { { "Y2025", "M1", "10" }, { "Y2025", "M2", "20" },
				{ "Y2025", "M3", "30" }, { "Y2026", "M1", "15" },
				{ "Y2026", "M2", "25" }, { "Y2026", "M3", "35" } };
		for (String[] r : rows) {
			data.append(r[0]).append('\t').append(r[1]).append('\t')
					.append(r[2]).append("\r\n");
		}
		FileIO.write(dir + "/" + DATA, data.toString());
	}
}
