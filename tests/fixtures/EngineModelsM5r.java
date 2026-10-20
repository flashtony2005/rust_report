/*
 * EngineModelsM5r.java -- M5 §6.3 残账（P1 多键并列 / P2 过滤非布尔 / P4 group@b）的产品夹具。
 *
 * 三张夹具各钉一个语义点（一模板一语义点），数据集见下：
 *
 *   engine_tiesort.csv   id k1 k2   —— k1/k2 全相等，id 刻意乱序（7,3,9,1,5）
 *   engine_rowtest.csv   id         —— 只用于触发过滤条件非布尔
 *   engine_groupb.csv    id g       —— 按 g 有序（B B B A A），@b 的前提成立
 *
 * 探针 `ProbeM5Rest` 的实证结论写在 docs/spec-m5.md §6.3（P1/P2/P4 三条）。
 * 产品 CSV 一律 TAB 分隔（`EngineData` 口径，见 RUNBOOK §47）。
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;

final class EngineModelsM5r {

	private EngineModelsM5r() {
	}

	static boolean isM5r(String name) {
		return name.startsWith("engine_tiesort") || name.startsWith("engine_rowtest")
				|| name.startsWith("engine_groupb")
				|| name.startsWith("engine_mergepage");
	}

	static ReportDefine2 build(String name) {
		if ("engine_tiesort".equals(name)) {
			return tiesort();
		}
		if ("engine_rowtest".equals(name)) {
			return rowtest();
		}
		if ("engine_groupb".equals(name)) {
			return groupb();
		}
		if ("engine_mergepage".equals(name)) {
			return mergepage();
		}
		throw new IllegalArgumentException("unknown m5r fixture: " + name);
	}

	/** 各夹具自己的列头（`GenEngineFixtures` 的两处表头分派都调这里）。 */
	static String[] header(String csv) {
		if ("engine_tiesort.csv".equals(csv)) {
			return new String[] { "id", "k1", "k2" };
		}
		if ("engine_rowtest.csv".equals(csv)) {
			return new String[] { "id" };
		}
		return new String[] { "id", "g" };
	}

	static String[][] rows(String name) {
		if ("engine_tiesort".equals(name)) {
			// 全键相等（k1=K, k2=Z），id 刻意乱序：稳定排序 ⇒ 原样输出 7/3/9/1/5；
			// 任何"按值再排一次"或非稳定排序都会打乱它。
			return new String[][] { { "7", "K", "Z" }, { "3", "K", "Z" },
					{ "9", "K", "Z" }, { "1", "K", "Z" }, { "5", "K", "Z" } };
		}
		if ("engine_rowtest".equals(name)) {
			return new String[][] { { "1" }, { "2" }, { "3" } };
		}
		if ("engine_mergepage".equals(name)) {
			// A 段 3 行 + B 段 3 行；行高 100mm ⇒ 每页 2 行 ⇒ A 段跨第 1/2 页。
			return new String[][] { { "1", "A" }, { "2", "A" }, { "3", "A" },
					{ "4", "B" }, { "5", "B" }, { "6", "B" } };
		}
		// 按 g **升序**（A A B B B）—— 这才是 @b 二分的前提；降序会让二分查不到 A
		// （探针实测：降序 B B B A A 下 `sum@b(id, g=="A")` = 0）。
		return new String[][] { { "3", "A" }, { "1", "A" }, { "7", "B" },
				{ "9", "B" }, { "5", "B" } };
	}

	/**
	 * P1 多键排序「全键相等」：产品 `SortDsValue.sort:384-401` 是手写归并——
	 * 先按首键整表 `Collections.sort`（稳定），再对每个首键相等的连续段按次键重排。
	 * 全键相等 ⇒ 所有比较都为 0 且每趟稳定 ⇒ **产出 = 输入序**。
	 * 探针实测（`ProbeM5Rest` P1_multikey_all_equal）：7/3/9/1/5，未被打乱。
	 *
	 * 对照格放 **D8**（A2 行带之外的另一条纵带）——放同一行带里会变成 A2 的从属格，
	 * 取到的是 A2 实例序号对应的元素（实测两列同为 7/3/9/1/5），证明不了排序通道。
	 */
	private static ReportDefine2 tiesort() {
		ReportDefine2 rd = new ReportDefine2(14, 4);
		EngineModels.lit(rd, 1, 1, "tie");
		EngineModels.lit(rd, 7, 4, "sorted");
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(id,,,;k1,k2)");
		INormalCell d8 = rd.getCell(8, 4);
		d8.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(d8, "ds1.select(id,,,;id)");
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_tiesort.csv");
		return rd;
	}

	/**
	 * P2 过滤条件非布尔：产品 `Row.test`（`Row.java:88-94`）对非 Boolean 结果抛
	 * `error.Row.test`（"记录过滤条件应该返回布尔值"），整份报表失败。
	 * 探针 P2 三种形态（常量 5 / `id+1` / 字符串 "x"）同一条消息。
	 */
	private static ReportDefine2 rowtest() {
		ReportDefine2 rd = new ReportDefine2(6, 3);
		EngineModels.lit(rd, 1, 1, "id");
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(id, 5)");
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_rowtest.csv");
		return rd;
	}

	/**
	 * P4 `group@b` 二分快速路径：前提是数据已按该列有序。有序时二分与线性同值
	 * （探针 P4_sum_binary_sorted = P4_sum_linear_sorted = 21），本夹具即钉这条**等价域**。
	 *
	 * ⛔ 无序数据上产品会返回"二分命中点所在的连续段"（探针：无序 `sum@b` = 9，
	 * 线性 = 21）——本仓有意不复刻该缺陷，差异见 docs/spec-m5.md §6.3 ③。
	 */
	private static ReportDefine2 groupb() {
		ReportDefine2 rd = new ReportDefine2(8, 4);
		EngineModels.lit(rd, 1, 1, "binary");
		EngineModels.lit(rd, 1, 2, "linear");
		EngineModels.lit(rd, 1, 3, "binaryA");
		CalcProbe.setExp(rd.getCell(2, 1), "ds1.sum@b(id, g==\"B\")");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.sum(id, g==\"B\")");
		CalcProbe.setExp(rd.getCell(2, 3), "ds1.sum@b(id, g==\"A\")");
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_groupb.csv");
		return rd;
	}

	/**
	 * P3 夹具：mergeSame 段**跨分页点**（行高标定后页边界可控）。
	 * 100mm 行高 ⇒ A4 每页 2 行 ⇒ A 段（3 行）跨第 1/2 页、B 段跨第 2/3 页。
	 * 探针 `ProbeRowHeight`/`ProbeMergePage` 实证：产品把每页的跨度裁成交矩形维度
	 * （A 段在第 1 页报 row_merge=2、在第 2 页只剩 1 行 ⇒ 不上报），
	 * 与本仓 `calc/merged_view.rs::page_slots` 一致。
	 */
	private static ReportDefine2 mergepage() {
		ReportDefine2 rd = new ReportDefine2(6, 3);
		EngineModels.lit(rd, 1, 1, "g");
		EngineModels.lit(rd, 1, 2, "id");
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(g)");
		a2.setMergeSame(EngineModelsP0.SAME_V);
		INormalCell b2 = rd.getCell(2, 2);
		b2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(b2, "ds1.select(id)");
		EngineModels.lit(rd, 3, 1, "尾行");
		// 行高单位是 mm（`ProbeRowHeight` 标定）：100mm ⇒ 每页 2 行。
		rd.getRowCell(1).setRowHeight(8f);
		rd.getRowCell(2).setRowHeight(100f);
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_mergepage.csv");
		return rd;
	}
}
