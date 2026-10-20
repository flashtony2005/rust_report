/*
 * ProbeHc4.java -- M5-09 夹具 #7（坐标落合并区）的布局探针（只 dump 不断言）。
 *
 * 上一轮把 B2:C2 合并塞进 3x3 交叉矩阵时，产品把横向扩展弄歪了（两列都出 E/A）
 * —— 症结是**双向扩展 + 合并区**叠加。本探针改成**单方向（只纵向）**布局：
 *
 *   A2 纵向扩展（ds1.select(dept)）        B2:C2 合并，值 = A2+"#"
 *
 * 要回答的问题：
 *   Q1 合并区锚点 B2 的坐标取值（应 = 该实例值）
 *   Q2 合并延续槽 C2 的坐标取值 —— 与 B2 同值（realCell 语义）/ null / 报错？
 *   Q3 格集 {} 是否把延续槽算成独立实例（count(B2[A2]{}) 是 3 还是 6？）
 *   Q4 合并区随扩展复制后，每个实例行仍是合并态吗（dump colMerge）
 *
 * 用法（cwd = tests/fixtures）：
 *   javac --release 8 -cp "F:/project/report/reportlite/lib/*;out" ProbeHc4.java -d out
 *   F:/project/report/common/jre/bin/java.exe -cp "...;out" rpx.fixtures.ProbeHc4 .
 */
package rpx.fixtures;

import com.raqsoft.reportl.dataset.DataSet;
import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.scudata.common.Area;

public final class ProbeHc4 {

	private static final String CSV = "hc4_merge.csv";

	public static void main(String[] args) throws Exception {
		String dir = args.length > 0 ? args[0] : ".";
		writeCsv(dir + "/" + CSV);

		// 基线：不写任何坐标，只看扩展后的格子与合并区
		run("A_baseline", -1, -1, null);
		// Q1/Q2：锚点 vs 延续槽，第 1 / 2 / 3 个实例
		run("B_anchor_1", 8, 5, "B2[A2:1]");
		run("C_slave_1", 8, 6, "C2[A2:1]");
		run("D_anchor_2", 8, 7, "B2[A2:2]");
		run("E_slave_2", 8, 8, "C2[A2:2]");
		run("F_anchor_3", 9, 5, "B2[A2:3]");
		run("G_slave_3", 9, 6, "C2[A2:3]");
		// Q3：格集计数。⛔ 不能写 `B2[A2]{}`（无序号）——固定格没有 A2 主格，
		// 产品报「"[]"位移对象超出范围」；根坐标才不受书写格限制。
		run("H_count_anchor", 9, 7, "count(B2[`0]{})");
		run("I_count_slave", 9, 8, "count(C2[`0]{})");
		// 对照：不带中括号的缺省管辖格集
		run("J_count_default", 10, 5, "count(B2{})");
		run("K_count_default_slave", 10, 6, "count(C2{})");
	}

	private static ReportDefine2 base(String csv) {
		ReportDefine2 rd = new ReportDefine2(12, 8);
		EngineModels.lit(rd, 1, 1, "dept");
		EngineModels.lit(rd, 1, 2, "val");

		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(dept)");

		// 值格 B2 左主格 = A2（表达式引用 A2 ⇒ 随纵向扩展被动复制）
		CalcProbe.setExp(rd.getCell(2, 2), "A2+\"#\"");
		// B2:C2 横向合并：B2 是锚点，C2 是延续槽
		rd.getCell(2, 2).setMergedArea(new Area(2, 2, 2, 3));

		CalcProbe.attachFileDataSet(rd, "ds1", csv);
		return rd;
	}

	private static void run(String name, int r, int c, String exp) {
		ReportDefine2 rd = base(CSV);
		if (exp != null) {
			CalcProbe.setExp(rd.getCell(r, c), exp);
		}
		try {
			IReport rpt = CalcProbe.calc(rd, null, null);
			System.out.println("### " + name + " | OK | exp=" + exp);
			CalcProbe.dump(name, rpt, true);
		} catch (Throwable t) {
			System.out.println("### " + name + " | ERR | exp=" + exp + " | "
					+ t.getClass().getName() + ": " + t.getMessage());
		}
	}

	private static void writeCsv(String path) throws Exception {
		// 产品 FileDataSet 的 .csv 实际是 TAB 分隔（见 RUNLOG §47）
		StringBuilder sb = new StringBuilder("dept\tamount\r\n");
		String[][] rows = { { "D1", "10" }, { "D2", "20" }, { "D3", "30" } };
		for (String[] row : rows) {
			sb.append(row[0]).append('\t').append(row[1]).append("\r\n");
		}
		FileIO.write(path, sb.toString());
	}
}
