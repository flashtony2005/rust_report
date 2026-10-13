/*
 * ProbeHeadMap2.java -- head 图的**判别性样本**（现有夹具给不出的那部分）
 *
 * 为什么要专门造：`ProbeHeadMap` 在 9 个可算夹具上实测到
 *   - `leftHead != cell00` 的反例**存在**（engine_ext 2 个，与已知一致）
 *   - `topHead != cell00` 的格数 **全部为 0**
 * ⇒ `_$2`（上 head）的两条命中分支（19-extend 与 head-of-head）在现有语料里
 * **零覆盖**，方向也判别不出来（没有候选可比）。
 *
 * 于是造四个形状，每个只为回答一个问题：
 *   S1 上 head 的 19 分支能不能命中？（head 正上方是一个 19 扩展格）
 *   S2 上 head 的方向：两个 19 扩展格上下相邻时，取**最近**还是**最远**？
 *      —— 递增扫描会取到"最上"，递减扫描取到"最近"，两者结果不同即为判别样本
 *   S3 左 head 的方向（受控版）：同行两个 18 扩展格，取最近还是最远？
 *   S4 连续三格：能否拿到"头的头"（`_$3`/`_$2` 的第二条返回路径）
 *
 * 只读不写：不碰 `src/**`。用法：java -cp "lib/*;out" rpx.fixtures.ProbeHeadMap2
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.Engine;
import com.raqsoft.reportl.usermodel.INormalCell;

public class ProbeHeadMap2 {

	public static void main(String[] args) {
		header();
		s1_topHead_19branch();
		s2_topHead_direction();
		s3_leftHead_direction();
		s4_head_of_head();
	}

	private static void header() {
		System.out.println("=== 0. 本探针要回答的问题 ===");
		System.out.println("  S1: _$2 的首分支（sameRowTypeAs && containCol && 19==extend）能不能命中？");
		System.out.println("  S2: 两个 19 扩展格上下相邻时，上 head 取**最近**（递减）还是**最远**（递增）？");
		System.out.println("  S3: 同行两个 18 扩展格时，左 head 取最近还是最远？（受控判别样本）");
		System.out.println("  S4: 连续三格能否拿到「头的头」（_$3/_$2 的第二条返回路径）？");
		System.out.println("  判读：结果若是 cell00 ⇒ 该分支未命中；若是具体坐标 ⇒ 命中，且坐标本身判别方向");
	}

	// ---------------------------------------------------------------- S1
	private static void s1_topHead_19branch() {
		System.out.println();
		System.out.println("=== S1 上 head 的 19 分支 ===");
		// 3 行 4 列；行 1 的 (1,1) 是 19 扩展格（横向），(2,1) 是查询格
		ReportDefine2 rd = new ReportDefine2(3, 4);
		EngineModels.lit(rd, 1, 2, "h2");
		EngineModels.lit(rd, 1, 3, "h3");
		EngineModels.lit(rd, 2, 1, "Q1");
		EngineModels.lit(rd, 3, 1, "Q3");
		INormalCell anchor = rd.getCell(1, 1);
		anchor.setExtendMode(INormalCell.EXTEND_HORIZONTAL);
		CalcProbe.setExp(anchor, "ds1.select(region)");
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_sum.csv");
		report("S1", rd, new int[][] { { 1, 1 }, { 2, 1 }, { 3, 1 } });
	}

	// ---------------------------------------------------------------- S2
	private static void s2_topHead_direction() {
		System.out.println();
		System.out.println("=== S2 上 head 的方向（两个 19 扩展格上下相邻）===");
		ReportDefine2 rd = new ReportDefine2(6, 4);
		EngineModels.lit(rd, 1, 2, "a2");
		EngineModels.lit(rd, 1, 3, "a3");
		EngineModels.lit(rd, 3, 2, "b2");
		EngineModels.lit(rd, 3, 3, "b3");
		EngineModels.lit(rd, 6, 1, "Q6");
		for (int r : new int[] { 1, 3 }) {
			INormalCell c = rd.getCell(r, 1);
			c.setExtendMode(INormalCell.EXTEND_HORIZONTAL);
			CalcProbe.setExp(c, "ds1.select(region)");
		}
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_sum.csv");
		report("S2", rd, new int[][] { { 1, 1 }, { 3, 1 }, { 6, 1 } });
		System.out.println("  判读：若 (6,1) 的 topHead = (3,1) ⇒ **递减**（最近）；= (1,1) ⇒ 递增（最远）");
	}

	// ---------------------------------------------------------------- S3
	private static void s3_leftHead_direction() {
		System.out.println();
		System.out.println("=== S3 左 head 的方向（同行两个 18 扩展格，受控）===");
		ReportDefine2 rd = new ReportDefine2(4, 5);
		EngineModels.lit(rd, 1, 1, "hdr");
		for (int c = 1; c <= 5; c++) {
			EngineModels.lit(rd, 2, c, "v" + c);
			EngineModels.lit(rd, 3, c, "w" + c);
		}
		// (2,1) 与 (2,3) 都是 18 扩展格；(2,5) 是查询格 ⇒ 最近=(2,3)、最远=(2,1)
		for (int c : new int[] { 1, 3 }) {
			INormalCell cell = rd.getCell(2, c);
			cell.setExtendMode(INormalCell.EXTEND_VERTICAL);
			CalcProbe.setExp(cell, "ds1.select(qty)");
		}
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_sum.csv");
		report("S3", rd, new int[][] { { 2, 1 }, { 2, 3 }, { 2, 5 } });
		System.out.println("  判读：若 (2,5) 的 leftHead = (2,3) ⇒ **递减**（最近）；= (2,1) ⇒ 递增（最远）");
	}

	// ---------------------------------------------------------------- S4
	private static void s4_head_of_head() {
		System.out.println();
		System.out.println("=== S4 连续三格：能否拿到「头的头」===");
		ReportDefine2 rd = new ReportDefine2(5, 6);
		for (int r = 1; r <= 5; r++) {
			for (int c = 1; c <= 6; c++) {
				EngineModels.lit(rd, r, c, r + "-" + c);
			}
		}
		// 行 2 的 (2,1)(2,2)(2,3) 连续三个 18 扩展格 ⇒ (2,4) 的左 head 会是 (2,3)，
		// 而 (2,3) 自己不是 head 时就落到「头的头」分支
		for (int c = 1; c <= 3; c++) {
			INormalCell cell = rd.getCell(2, c);
			cell.setExtendMode(INormalCell.EXTEND_VERTICAL);
			CalcProbe.setExp(cell, "ds1.select(qty)");
		}
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_sum.csv");
		report("S4", rd, new int[][] { { 2, 1 }, { 2, 2 }, { 2, 3 }, { 2, 4 }, { 2, 5 } });
	}

	/** 算一遍并打印指定坐标的 head；坐标可能被扩展挪走，故按 source 找。 */
	private static void report(String tag, ReportDefine2 rd, int[][] want) {
		try {
			ExtCellSet ecs = (ExtCellSet) new Engine(rd, new com.raqsoft.reportl.usermodel.Context())
					.calc();
			ExtCell c00 = ecs.getCell00();
			System.out.println("  " + tag + " grid=" + ecs.getRowCount() + "x" + ecs.getColCount());
			for (int r = 1; r <= ecs.getRowCount(); r++) {
				for (int c = 1; c <= ecs.getColCount(); c++) {
					ExtCell cell = ecs.getSource(r, c);
					if (cell == null) {
						continue;
					}
					boolean interesting = false;
					for (int[] w : want) {
						if (cell.getSourceRow() == w[0] && cell.getSourceCol() == w[1]) {
							interesting = true;
						}
					}
					if (!interesting) {
						continue;
					}
					ExtCell lh = cell.getCellLeftHead();
					ExtCell th = cell.getCellTopHead();
					System.out.println("    模板(" + cell.getSourceRow() + "," + cell.getSourceCol()
							+ ") → 计算格(" + r + "," + c + ")"
							+ " extend=" + cell.getExtensible()
							+ " leftHead=" + (lh == c00 ? "cell00" : at(lh))
							+ " topHead=" + (th == c00 ? "cell00" : at(th)));
				}
			}
		} catch (Throwable t) {
			System.out.println("  " + tag + " 失败：" + t.getClass().getSimpleName()
					+ " " + CalcProbe.repr(t.getMessage()));
		}
	}

	private static String at(ExtCell c) {
		return c == null ? "null" : ("(" + c.getRow() + "," + c.getCol() + ")");
	}
}