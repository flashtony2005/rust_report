/*
 * ProbeHc.java -- M5-09 层次坐标/格集 前置探针（8 问，只 dump 不断言）。
 *
 * 目的：在生成一等夹具之前，用产品引擎回答「NopReport 源码与手册都推不出」的
 * 问题：序基准（1-based?）、越界行为、位移基准、根坐标写法、格集类型、
 * 条件格集 $ 绑定、& 基准、落点边界。探针结论回填 docs/spec-m5-09-hier-coord.md §2。
 *
 * 布局：与 engine_crossalign 同构的 3x3 交叉模板（B1 横、A2 纵、B2 交叉值），
 * 外加矩阵外的固定格（独立格运算）。数据集用 ASCII，避开 GBK 干扰。
 *
 * 用法（cwd = tests/fixtures）：
 *   javac --release 8 -cp "F:/project/report/reportlite/lib/*;out" ProbeHc.java -d out
 *   F:/project/report/common/jre/bin/java.exe -cp "...;out" rpx.fixtures.ProbeHc .
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;

public final class ProbeHc {

	private static final String CSV = "hc_probe.csv";
	private static final String CSV_EMPTY = "hc_probe_empty.csv";

	public static void main(String[] args) throws Exception {
		String dir = args.length > 0 ? args[0] : ".";
		writeCsv(dir + "/" + CSV, true);
		writeCsv(dir + "/" + CSV_EMPTY, false);

		run("00_baseline", CSV, -1, -1, null);
		run("01_abs_both", CSV, 8, 6, "B2[A2:2;B1:2]");
		run("02_abs_row_only", CSV, 8, 6, "B2[A2:3]");
		run("03_abs_col_only", CSV, 8, 6, "B2[;B1:2]");
		run("04_root_short", CSV, 8, 6, "B2[`0;B1:2]");
		run("05_root_long", CSV, 8, 6, "B2[`0,A2:2;`0,B1:2]");
		run("06_offset_minus_band", CSV, 2, 3, "B2[A2:-1;B1:1]");
		run("07_offset_plus_band", CSV, 2, 3, "B2[A2:+1;B1:1]");
		run("08_offset_fixed", CSV, 8, 6, "B2[A2:-1;B1:1]");
		run("09_reverse_band", CSV, 2, 3, "B2[A2:!-1;B1:1]");
		run("10_oob", CSV, 8, 6, "B2[A2:99;B1:1]");
		run("11_cellset_root", CSV, 8, 6, "count(B2[`0]{})");
		run("12_cellset_col", CSV, 8, 6, "count(B2[;B1:1]{})");
		run("13_cellset_row_sum", CSV, 8, 6, "sum(B2[A2:1]{})");
		run("14_cond_band", CSV, 2, 3, "B2-B2[;B1:-1]{$A2==A2}");
		run("15_amp_band", CSV, 2, 3, "&A2");
		run("16_amp_fixed", CSV, 8, 6, "&A2");
		run("17_empty_dataset", CSV_EMPTY, 8, 6, "count(B2[;B1:1]{})");
		// 第二轮：列相对/环比必须写在 B1 的列带内（B3），C2 没有 B1 顶主格
		run("18_rel_col_band", CSV, 3, 2, "B2[;B1:-1]");
		run("19_cond_band2", CSV, 3, 2, "B2-B2[;B1:-1]{$A2==A2}");
		run("20_amp_col_band", CSV, 3, 2, "&B1");
		runNumericSum();
	}

	private static void runNumericSum() {
		ReportDefine2 rd = base(CSV, true);
		CalcProbe.setExp(rd.getCell(8, 6), "sum(B2[`0]{})");
		execute("21_numeric_sum", rd, "sum(B2[`0]{})");
	}

	private static void run(String name, String csv, int r, int c, String exp) {
		ReportDefine2 rd = base(csv, false);
		if (exp != null) {
			CalcProbe.setExp(rd.getCell(r, c), exp);
		}
		execute(name, rd, exp);
	}

	private static void execute(String name, ReportDefine2 rd, String exp) {
		try {
			IReport rpt = CalcProbe.calc(rd, null, null);
			System.out.println("### " + name + " | OK | exp=" + exp);
			CalcProbe.dump(name, rpt);
		} catch (Throwable t) {
			System.out.println("### " + name + " | ERR | exp=" + exp + " | "
					+ t.getClass().getName() + ": " + t.getMessage());
		}
	}

	/**
	 * 3x3 交叉模板 + 矩阵外固定标记格（row10）。
	 * 固定格用来观察「独立格运算」是否保持单格，以及坐标在无主格的格中如何失败。
	 */
	private static ReportDefine2 base(String csv, boolean numericB2) {
		ReportDefine2 rd = new ReportDefine2(12, 8);
		EngineModels.lit(rd, 1, 1, "R\\C");
		INormalCell head = rd.getCell(1, 2);
		head.setExtendMode(INormalCell.EXTEND_HORIZONTAL);
		CalcProbe.setExp(head, "ds1.select(region:1,,,region)");
		INormalCell side = rd.getCell(2, 1);
		side.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(side, "ds1.select(product:1,,,product)");
		if (numericB2) {
			EngineModels.lit(rd, 2, 2, "1");
		} else {
			CalcProbe.setExp(rd.getCell(2, 2), "B1+\"/\"+A2");
		}
		EngineModels.lit(rd, 10, 2, "FIXA");
		EngineModels.lit(rd, 10, 6, "FIXB");
		CalcProbe.attachFileDataSet(rd, "ds1", csv);
		return rd;
	}

	private static void writeCsv(String path, boolean withRows) throws Exception {
		// 产品 FileDataSet 的 .csv 实际是 TAB 分隔（engine_crossalign.csv 实测 \t）
		StringBuilder sb = new StringBuilder("region\tproduct\tamount\r\n");
		if (withRows) {
			String[][] rows = { { "N", "A", "10" }, { "N", "B", "20" },
					{ "N", "C", "30" }, { "E", "A", "40" }, { "E", "B", "50" },
					{ "E", "C", "60" } };
			for (String[] r : rows) {
				sb.append(r[0]).append('\t').append(r[1]).append('\t')
						.append(r[2]).append("\r\n");
			}
		}
		FileIO.write(path, sb.toString());
	}
}
