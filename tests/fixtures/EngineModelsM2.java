package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IRowCell;

/**
 * `engine_rowheight` 的模型 —— M2-22：行高由**属性表达式**算出（附件 L）。
 *
 * <p>为什么不可省：在此之前 9 个夹具的行高全是模板里的定值，于是
 * 「行高算错 ⇒ 页数错」这条失效模式在基准里**根本测不出来**。
 *
 * <p>设计要点：模板行高**故意一律设成 10**，而给三行挂上不同的行高表达式 ——
 * 于是「计算态行高」必然不同于「模板行高」，`pagination` 段的页边界就成了
 * 「属性表达式已在分页之前求值」这条因果的**可观测证据**
 * （时序依据：`ExtCellSet.java:1504` 调 `calcProperties`，`reflush()` 在 `:2258`）。
 *
 * <p>覆盖的属性通道：**内置行属性通道**（`ExtRowCell` 的 prop expMap，
 * key 1/2/7/13/15），本夹具只用 **key 1（行高）**。
 * ⛔ **未覆盖** `customPropEngMap` 那条通道（`ExtRowCell.java:93-95`、`:406-437`，
 * 求值走 `getSingleValue`）—— 两者取值规则不同（拆 List 与否），需要时另造夹具。
 *
 * <p>单独成文件的原因：`EngineModels.java` 加了本夹具后 294 行，贴 300 行硬门禁；
 * 按现有 `EngineModelsP0` / `EngineModelsTypes` 的同一约定拆出。
 */
final class EngineModelsM2 {

	private EngineModelsM2() {
	}

	/**
	 * engine_rowheight -- M2-22：行高由**属性表达式**算出（M2-22 / 附件 L）。
	 *
	 * <p>为什么不可省：在此之前 9 个夹具的行高全是模板里的定值，于是
	 * 「行高算错 ⇒ 页数错」这条失效模式在基准里**根本测不出来**。
	 *
	 * <p>设计要点：模板行高**故意一律设成 10**，而给三行挂上不同的行高表达式 ——
	 * 于是「计算态行高」必然不同于「模板行高」，`pagination` 段的页边界就成了
	 * 「属性表达式已在分页之前求值」这条因果的**可观测证据**
	 * （时序依据：`ExtCellSet.java:1504` 调 `calcProperties`，`reflush()` 在 `:2258`）。
	 *
	 * <p>覆盖的属性通道：**内置行属性通道**（`ExtRowCell` 的 prop expMap，
	 * key 1/2/7/13/15），本夹具只用 **key 1（行高）**。
	 * ⛔ **未覆盖** `customPropEngMap` 那条通道（`ExtRowCell.java:93-95`、`:406-437`，
	 * 求值走 `getSingleValue`）—— 两者取值规则不同（拆 List 与否），需要时另造夹具。
	 */
	static ReportDefine2 rowheightFixture() {
		ReportDefine2 rd = new ReportDefine2(6, 3);
		EngineModels.lit(rd, 1, 1, "region");
		EngineModels.lit(rd, 1, 2, "qty");
		EngineModels.lit(rd, 1, 3, "doubled");
		// 行 2 = 纵向扩展锚点：它的行高表达式会被所有插入行继承
		CalcProbe.setExp(rd.getCell(2, 1), "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
		CalcProbe.setExp(rd.getCell(2, 3), "B2*2");
		// 行 3 不扩展，但**照样挂行高表达式** —— 证明表达式与扩展是两条独立通道
		EngineModels.lit(rd, 3, 1, "静态行");
		EngineModels.lit(rd, 3, 2, "no-expand");
		EngineModels.lit(rd, 3, 3, "keep");
		CalcProbe.attachFileDataSet(rd, "ds1", EngineModels.csvName("engine_rowheight"));
		// 模板行高一律 10（单位 mm）；下面三行的**计算值**由表达式决定
		for (int r = 1; r <= 6; r++) {
			rd.getRowCell(r).setRowHeight(10f);
		}
		rd.getRowCell(2).setRowHeightExp("40");        // 扩展锚点：40
		rd.getRowCell(3).setRowHeightExp("25*2");      // 不扩展的行：50
		rd.getRowCell(4).setRowHeightExp("20+5+2");    // 纯字面量算术：27
		return rd;
	}

	/** 夹具的数据集：12 行，让页边界对行高足够敏感。 */
	static String[][] rows() {
		String[][] r = new String[12][2];
		for (int i = 0; i < 12; i++) {
			r[i][0] = "R" + (i + 1);
			r[i][1] = Integer.toString((i + 1) * 10);
		}
		return r;
	}
}
