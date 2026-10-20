/*
 * ProbeHc3.java -- M5-09 第三批探针：合并区坐标 / 坐标进 ds 过滤 / 三级主格链 / 非主格 &。
 *
 * 四个独立场景（互不影响，任一失败只影响本场景）：
 *   merge  3x3 交叉 + B2:C2 合并，看坐标落锚点/延续槽
 *   ds     固定格用 `ds1.select(amount, product==A2[2])`，看坐标值能否当过滤参数
 *   order  三级纵向链（部门→组→人，三张维表），看 `[A2:2,B2:1,C2:2]` 的从远到近
 *   amp    固定格写 `&A2`（A2 不是 F8 的主格）⇒ 记录错误原文
 *
 * 用法（cwd = tests/fixtures）：
 *   javac --release 8 -cp "F:/project/report/reportlite/lib/*;out" ProbeHc3.java -d out
 *   F:/project/common/jre/bin/java.exe -cp "...;out" rpx.fixtures.ProbeHc3 .
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.scudata.common.Area;

public final class ProbeHc3 {

	public static void main(String[] args) throws Exception {
		String dir = args.length > 0 ? args[0] : ".";
		writeCsvs(dir);

		run("merge_anchor", merge(true), 8, 6, "B2[A2:1;B1:2]");
		run("merge_tail", merge(true), 8, 6, "C2[A2:1;B1:2]");
		run("merge_nomerge", merge(false), 8, 6, "B2[A2:1;B1:2]");
		run("ds_coord", matrix(), 8, 6, "ds1.select(amount, product==A2[2])");
		run("ds_region", matrix(), 8, 6, "ds1.select(amount, region==B1[2])");
		run("order_212", order(), 8, 6, "D2[A2:2,B2:1,C2:2]");
		run("order_121", order(), 8, 6, "D2[A2:1,B2:2,C2:1]");
		run("amp_fixed", matrix(), 8, 6, "&A2");
	}

	private static void run(String name, ReportDefine2 rd, int r, int c, String exp) {
		CalcProbe.setExp(rd.getCell(r, c), exp);
		try {
			IReport rpt = CalcProbe.calc(rd, null, null);
			System.out.println("### " + name + " | OK | exp=" + exp);
			CalcProbe.dump(name, rpt);
		} catch (Throwable t) {
			System.out.println("### " + name + " | ERR | exp=" + exp + " | "
					+ t.getClass().getName() + ": " + t.getMessage());
		}
	}

	/** 3x3 交叉矩阵（与 ProbeHc 同构）；merge=true 时把 B2:C2 合并。 */
	private static ReportDefine2 matrix() {
		ReportDefine2 rd = new ReportDefine2(12, 8);
		EngineModels.lit(rd, 1, 1, "R\\C");
		INormalCell head = rd.getCell(1, 2);
		head.setExtendMode(INormalCell.EXTEND_HORIZONTAL);
		CalcProbe.setExp(head, "ds1.select(region:1,,,region)");
		INormalCell side = rd.getCell(2, 1);
		side.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(side, "ds1.select(product:1,,,product)");
		CalcProbe.setExp(rd.getCell(2, 2), "B1+\"/\"+A2");
		CalcProbe.attachFileDataSet(rd, "ds1", "hc3_matrix.csv");
		return rd;
	}

	private static ReportDefine2 merge(boolean merged) {
		ReportDefine2 rd = matrix();
		if (merged) {
			rd.getCell(2, 2).setMergedArea(new Area(2, 2, 2, 3));
		}
		return rd;
	}

	/** 三级纵向链：部门（ds1）→ 组（ds2）→ 人（ds3），D2 用 & 合成可辨识值。 */
	private static ReportDefine2 order() {
		ReportDefine2 rd = new ReportDefine2(10, 8);
		EngineModels.lit(rd, 1, 1, "部门\\组\\人");
		INormalCell dept = rd.getCell(2, 1);
		dept.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(dept, "ds1.select(dept:1,,,dept)");
		INormalCell team = rd.getCell(2, 2);
		team.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(team, "ds2.select(team, dept==A2)");
		INormalCell person = rd.getCell(2, 3);
		person.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(person, "ds3.select(person, team==B2)");
		CalcProbe.setExp(rd.getCell(2, 4), "&A2*100+&B2*10+&C2");
		CalcProbe.attachFileDataSet(rd, "ds1", "hc3_dept.csv");
		CalcProbe.attachFileDataSet(rd, "ds2", "hc3_team.csv");
		CalcProbe.attachFileDataSet(rd, "ds3", "hc3_person.csv");
		return rd;
	}

	private static void writeCsvs(String dir) throws Exception {
		StringBuilder m = new StringBuilder("region\tproduct\tamount\r\n");
		String[][] rows = { { "N", "A", "10" }, { "N", "B", "20" },
				{ "N", "C", "30" }, { "E", "A", "40" }, { "E", "B", "50" },
				{ "E", "C", "60" } };
		for (String[] r : rows) {
			m.append(r[0]).append('\t').append(r[1]).append('\t')
					.append(r[2]).append("\r\n");
		}
		FileIO.write(dir + "/hc3_matrix.csv", m.toString());
		FileIO.write(dir + "/hc3_dept.csv", "dept\r\nD1\r\nD2\r\n");
		FileIO.write(dir + "/hc3_team.csv",
				"dept\tteam\r\nD1\tT1\r\nD1\tT2\r\nD2\tT1\r\nD2\tT2\r\n");
		FileIO.write(dir + "/hc3_person.csv",
				"team\tperson\r\nT1\tP1\r\nT1\tP2\r\nT2\tP1\r\nT2\tP2\r\n");
	}
}
