/*
 * ProbeMergePage.java -- M5 §6.3 ⑦：合并段跨分页点时的产品行为（行高标定后的正式探针）。
 *
 * 前置结论（`ProbeRowHeight`，行高单位标定）：
 *   - `IRowCell.setRowHeight(x)` 的单位是 **mm**；
 *   - `getPagerInfo(rpt, 595, 842, 1)` 的 595/842 是 **pt**（A4 纵向），
 *     842pt ≈ 297mm ⇒ **每页可用高度 297mm**；
 *   - 每页行数 = 累计高度不超过 297mm 的最大行数（不扣页眉边距）；
 *   - `rowNumPerPage` 是常量 20，与真实切页无关，别看它。
 *
 * 本探针用 100mm 行高 ⇒ **每页 2 行**，把分页点钉死；6 行数据（g = A,A,A,B,B,B）
 * + mergeSame ⇒ A 段跨第 1/2 页边界（计算态第 3/4 行之间）。
 *
 * 要问：
 *   Q1 合并段跨页：产品是**断开求交**（本仓现状）还是把分页点前移/后移？
 *   Q2 带内再加一行分组头（rowType=-90）时是否不同？
 *   Q3 对照：不合并；以及合并不跨页（行高 60mm ⇒ 每页 4 行）。
 *
 * 用法（cwd = tests/fixtures）：
 *   javac --release 8 -cp "F:/project/report/reportlite/lib/*;out" ProbeMergePage.java -d out
 *   F:/project/report/common/jre/bin/java.exe -cp "...;out" rpx.fixtures.ProbeMergePage .
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IRowCell;
import com.raqsoft.reportl.usermodel.IReport;

public final class ProbeMergePage {

	private static final String CSV = "mergepage.csv";

	public static void main(String[] args) throws Throwable {
		String dir = args.length > 0 ? args[0] : ".";
		writeCsv(dir + "/" + CSV);

		// Q1：单行带 + mergeSame，A 段（3 行）跨第 1/2 页边界
		run("Q1_samemerge_cross", true, false, 100f);
		// Q2：**带内行本身就是分组头行**（rowType=-90 会随实例复制到每一行）
		run("Q2_band_row_is_group_header", true, false, 100f,
				IRowCell.TYPE_GROUP_HEADER);
		// Q5 对照：带内行是另一种行类型（表头 -95），验证是不是 -90 专属
		run("Q5_band_row_is_table_header", true, false, 100f,
				IRowCell.TYPE_TABLE_HEADER);
		// Q6：两行带（数据行 + 分组头行，left_head=A2）+ 90mm 行高 ⇒ 每页 3 行，
		// 分页点正好落在分组头行上（第 2 页首行 = 记录 2 的分组头行）。
		runTwoRowBand("Q6_break_on_group_header", 90f);
		// Q3 对照：不合并
		run("Q3_no_samemerge", false, false, 100f);
		// Q4 对照：行高 60mm（每页 4 行）⇒ A 段 3 行整块落在第 1 页，不跨页
		run("Q4_segment_within_page", true, false, 60f);
	}

	private static void run(String name, boolean mergeSame, boolean groupHeader,
			float height) throws Throwable {
		run(name, mergeSame, groupHeader, height, (byte) 0);
	}

	private static void run(String name, boolean mergeSame, boolean groupHeader,
			float height, byte bandRowType) throws Throwable {
		ReportDefine2 rd = new ReportDefine2(8, 4);
		if (bandRowType != 0) {
			rd.getRowCell(2).setRowType(bandRowType);
			rd.getRowCell(2).setGroupHeaderLevel((byte) 1);
		}
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(g)");
		if (mergeSame) {
			a2.setMergeSame(EngineModelsP0.SAME_V);
		}
		INormalCell b2 = rd.getCell(2, 2);
		b2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(b2, "ds1.select(id)");
		if (groupHeader) {
			// 分组头行：与数据行同带（left_head = A2），逐实例复制一份
			INormalCell b3 = rd.getCell(3, 2);
			rd.getRowCell(3).setRowType(IRowCell.TYPE_GROUP_HEADER);
			rd.getRowCell(3).setGroupHeaderLevel((byte) 1);
			CalcProbe.setExp(b3, "A2");
		}
		// 只给带内行设高度；模板剩余行用缺省小行高，避免把分页结构撑乱。
		for (int r = 2; r <= 2; r++) {
			rd.getRowCell(r).setRowHeight(height);
		}
		CalcProbe.attachFileDataSet(rd, "ds1", CSV);
		try {
			IReport rpt = CalcProbe.calc(rd, null, null);
			System.out.println("### " + name + " | OK | mergeSame=" + mergeSame
					+ " groupHeader=" + groupHeader + " height=" + height
					+ " bandRowType=" + bandRowType);
			CalcProbe.dump(name, rpt);
			CalcProbe.dumpPages(name, rpt, 595, 842, 1);
			System.out.println("[" + name + "] merged="
					+ CalcMergeJson.renderMerged(rpt, 595, 842, 1));
		} catch (Throwable t) {
			System.out.println("### " + name + " | ERR | " + t.getClass().getName()
					+ ": " + t.getMessage());
			t.printStackTrace(System.out);
		}
	}

	/** 两行带：第 2 行数据（mergeSame），第 3 行分组头（left_head=A2，逐实例复制）。 */
	private static void runTwoRowBand(String name, float height) throws Throwable {
		ReportDefine2 rd = new ReportDefine2(10, 4);
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(g)");
		a2.setMergeSame(EngineModelsP0.SAME_V);
		INormalCell b2 = rd.getCell(2, 2);
		b2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(b2, "ds1.select(id)");
		INormalCell b3 = rd.getCell(3, 2);
		b3.setLeftHead("A2");
		rd.getRowCell(3).setRowType(IRowCell.TYPE_GROUP_HEADER);
		rd.getRowCell(3).setGroupHeaderLevel((byte) 1);
		CalcProbe.setExp(b3, "A2");
		rd.getRowCell(2).setRowHeight(height);
		rd.getRowCell(3).setRowHeight(height);
		CalcProbe.attachFileDataSet(rd, "ds1", CSV);
		try {
			IReport rpt = CalcProbe.calc(rd, null, null);
			System.out.println("### " + name + " | OK | height=" + height);
			CalcProbe.dump(name, rpt);
			CalcProbe.dumpPages(name, rpt, 595, 842, 1);
			System.out.println("[" + name + "] merged="
					+ CalcMergeJson.renderMerged(rpt, 595, 842, 1));
		} catch (Throwable t) {
			System.out.println("### " + name + " | ERR | " + t.getClass().getName()
					+ ": " + t.getMessage());
			t.printStackTrace(System.out);
		}
	}

	private static void writeCsv(String path) throws Exception {
		StringBuilder sb = new StringBuilder("id\tg\r\n");
		String[][] rows = { { "1", "A" }, { "2", "A" }, { "3", "A" },
				{ "4", "B" }, { "5", "B" }, { "6", "B" } };
		for (String[] r : rows) {
			sb.append(r[0]).append('\t').append(r[1]).append("\r\n");
		}
		FileIO.write(path, sb.toString());
	}
}
