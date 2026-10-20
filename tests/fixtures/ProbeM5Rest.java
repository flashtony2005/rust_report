/*
 * ProbeM5Rest.java -- M5 §6.3 残账四条里的三条（数据级，不含分页）一次性探针。
 *
 * 只 dump 不断言，结论回填 docs/spec-m5.md §6.3 与 RUNLOG。
 *
 *   P1 多键排序「全键相等」的产出次序（SortDsValue 手写归并 vs 稳定排序）
 *   P2 Row 态过滤非布尔的产品消息文本（error.Row.test）
 *   P4 Group.filter@b 二分快速路径在**无序**数据上的行为
 *
 * 数据集 m5r.csv（TAB 分隔，产品 FileDataSet 口径）：
 *   id k1 k2 g
 *   7  K  Z  B      ← k1/k2 全相等（P1）；g 无序（P4）
 *   3  K  Z  A
 *   9  K  Z  B
 *   1  K  Z  A
 *   5  K  Z  B
 *
 * 用法（cwd = tests/fixtures）：
 *   javac --release 8 -cp "F:/project/report/reportlite/lib/*;out" ProbeM5Rest.java -d out
 *   F:/project/report/common/jre/bin/java.exe -cp "...;out" rpx.fixtures.ProbeM5Rest .
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;

public final class ProbeM5Rest {

	private static final String CSV = "m5r.csv";

	public static void main(String[] args) throws Exception {
		String dir = args.length > 0 ? args[0] : ".";
		writeCsv(dir + "/" + CSV);

		// P1：多键排序，全键相等 —— 看产出是否保持输入序 7/3/9/1/5
		run("P1_multikey_all_equal", "ds1.select(id,,,;k1,k2)");
		// 对照：单键全相等
		run("P1_singlekey_all_equal", "ds1.select(id,,,;k1)");
		// 对照：键不等时的正常升序（应 1/3/5/7/9）
		run("P1_multikey_sorted", "ds1.select(id,,,;k2,k1)");
		// P1 数值对照：按 id 排序（应 1/3/5/7/9）
		run("P1_by_id", "ds1.select(id,,,;id)");

		// P2：过滤条件非布尔
		run("P2_filter_const_nonbool", "ds1.select(id, 5)");
		run("P2_filter_expr_nonbool", "ds1.select(id, id+1)");
		run("P2_filter_string", "ds1.select(id, \"x\")");
		// 对照：正常布尔过滤
		run("P2_filter_bool", "ds1.select(id, id>3)");

		// P4：group@b 二分路径 vs 无选项的线性扫描（g 列无序：B/A/B/A/B）。
		// ⛔ 不能用 `group(...)` 直接看：它返回的是**组键**（B），不是成员数。
		// 用带过滤的 count/sum —— 它们内部同样走 Group.filter(flt, ctx, option)。
		run("P4_count_binary", "ds1.count@b(id, g==\"B\")");
		run("P4_count_linear", "ds1.count(id, g==\"B\")");
		run("P4_sum_binary", "ds1.sum@b(id, g==\"B\")");
		run("P4_sum_linear", "ds1.sum(id, g==\"B\")");
		// 对照：数据已按 g 有序（B/B/B/A/A）时两者应一致
		run("P4_sum_binary_sorted", "ds2.sum@b(id, g==\"B\")");
		run("P4_sum_linear_sorted", "ds2.sum(id, g==\"B\")");
	}

	private static ReportDefine2 base() {
		ReportDefine2 rd = new ReportDefine2(8, 4);
		EngineModels.lit(rd, 1, 1, "v");
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.attachFileDataSet(rd, "ds1", CSV);
		CalcProbe.attachFileDataSet(rd, "ds2", "m5r_sorted.csv");
		return rd;
	}

	private static void run(String name, String exp) {
		ReportDefine2 rd = base();
		CalcProbe.setExp(rd.getCell(2, 1), exp);
		try {
			IReport rpt = CalcProbe.calc(rd, null, null);
			System.out.println("### " + name + " | OK | exp=" + exp);
			CalcProbe.dump(name, rpt);
		} catch (Throwable t) {
			System.out.println("### " + name + " | ERR | exp=" + exp + " | "
					+ t.getClass().getName() + ": " + t.getMessage());
		}
	}

	private static void writeCsv(String path) throws Exception {
		StringBuilder sb = new StringBuilder("id\tk1\tk2\tg\r\n");
		String[][] rows = { { "7", "K", "Z", "B" }, { "3", "K", "Z", "A" },
				{ "9", "K", "Z", "B" }, { "1", "K", "Z", "A" },
				{ "5", "K", "Z", "B" } };
		for (String[] r : rows) {
			sb.append(r[0]).append('\t').append(r[1]).append('\t').append(r[2])
					.append('\t').append(r[3]).append("\r\n");
		}
		FileIO.write(path, sb.toString());

		// P4 对照：同一批行按 g 排好序（B B B A A）
		StringBuilder sb2 = new StringBuilder("id\tk1\tk2\tg\r\n");
		String[][] sorted = { { "7", "K", "Z", "B" }, { "9", "K", "Z", "B" },
				{ "5", "K", "Z", "B" }, { "3", "K", "Z", "A" },
				{ "1", "K", "Z", "A" } };
		for (String[] r : sorted) {
			sb2.append(r[0]).append('\t').append(r[1]).append('\t').append(r[2])
					.append('\t').append(r[3]).append("\r\n");
		}
		FileIO.write(path.replace("m5r.csv", "m5r_sorted.csv"), sb2.toString());
	}
}
