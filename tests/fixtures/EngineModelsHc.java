/*
 * EngineModelsHc.java -- M5-09 层次坐标/格集的产品夹具（走 GenEngineFixtures 主管线）。
 *
 * 布局与探针 ProbeHc 同构：3x3 交叉模板 + 矩阵外固定格。
 *   B1 横向扩展（region = N/E），A2 纵向扩展（product = A/B/C），
 *   B2 交叉值 = B1+"/"+A2（E/A … N/C）。
 * 计算态：14 行 x 9 列（扩展插入 2 行 1 列）；
 *   模板 (8,6) -> 计算态 (10,7)；(8,7)->(10,8)；(8,8)->(10,9)；
 *   标记 (10,2)=FIXA 会横向复制成 (12,2)+(12,3)，(10,6)=FIXB 仅 (12,7) 一份。
 *
 * 数据集 engine_hc*.csv 为 TAB 分隔（产品 FileDataSet 口径），ASCII 列名。
 * 每个夹具只钉一类语义（一模板一语义点）。
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.scudata.common.Area;

final class EngineModelsHc {

	private EngineModelsHc() {
	}

	static boolean isHc(String name) {
		return name.startsWith("engine_hc");
	}

	static ReportDefine2 build(String name) {
		if ("engine_hc_abs".equals(name)) {
			return abs(name);
		}
		if ("engine_hc_offset".equals(name)) {
			return offset(name);
		}
		if ("engine_hc_root".equals(name)) {
			return root(name);
		}
		if ("engine_hc_cellset".equals(name)) {
			return cellset(name);
		}
		if ("engine_hc_amp".equals(name)) {
			return amp(name);
		}
		if ("engine_hc_cond".equals(name)) {
			return cond(name);
		}
		if ("engine_hc_empty".equals(name)) {
			return empty(name);
		}
		if ("engine_hc_ds".equals(name)) {
			return ds(name);
		}
		if ("engine_hc_order".equals(name)) {
			return order(name);
		}
		if ("engine_hc_err".equals(name)) {
			return err(name);
		}
		if ("engine_hc_cycle".equals(name)) {
			return cycle(name);
		}
		if ("engine_hc_merge".equals(name)) {
			return merge(name);
		}
		throw new IllegalArgumentException("unknown hc fixture: " + name);
	}

	/** 数据集：region × product 全交叉，amount 留给后续数值聚合夹具。 */
	static String[][] rows() {
		return new String[][] { { "N", "A", "10" }, { "N", "B", "20" },
				{ "N", "C", "30" }, { "E", "A", "40" }, { "E", "B", "50" },
				{ "E", "C", "60" } };
	}

	/** 按夹具名给数据集行：order 用部门维表；cond 用 year/month/amount；empty 用 0 行。 */
	static String[][] rows(String name) {
		if ("engine_hc_order".equals(name)) {
			return new String[][] { { "D1" }, { "D2" } };
		}
		// merge：三行部门，用来分辨"第 1 实例还带合并区、第 2/3 实例不带"。
		if ("engine_hc_merge".equals(name)) {
			return new String[][] { { "D1" }, { "D2" }, { "D3" } };
		}
		if ("engine_hc_empty".equals(name)) {
			return new String[0][];
		}
		if ("engine_hc_cond".equals(name)) {
			return new String[][] { { "Y2025", "M1", "10" },
					{ "Y2025", "M2", "20" }, { "Y2025", "M3", "30" },
					{ "Y2026", "M1", "15" }, { "Y2026", "M2", "25" },
					{ "Y2026", "M3", "35" } };
		}
		return rows();
	}

	/**
	 * 绝对层次坐标：双主格 / 单侧（缺列取行首、缺行取列首）。
	 * 手算：A2:2 = product B，B1:2 = region E ⇒ "E/B"（探针 01 实测 N/B 为
	 * region 顺序相反；本夹具按 CSV 顺序 N 在前 ⇒ 第 2 列是 E）。
	 */
	private static ReportDefine2 abs(String name) {
		ReportDefine2 rd = base(name);
		CalcProbe.setExp(rd.getCell(8, 6), "B2[A2:2;B1:2]");
		CalcProbe.setExp(rd.getCell(8, 7), "B2[A2:3]");
		CalcProbe.setExp(rd.getCell(8, 8), "B2[;B1:2]");
		return rd;
	}

	/**
	 * 位移坐标：± 偏移 + 越界（有主格但越界 ⇒ null）。
	 * C2/D2/E2 在 B1 带外、A2 带内 ⇒ 按行实例复制 3 份。
	 */
	private static ReportDefine2 offset(String name) {
		ReportDefine2 rd = base(name);
		CalcProbe.setExp(rd.getCell(2, 3), "B2[A2:-1;B1:1]");
		CalcProbe.setExp(rd.getCell(2, 4), "B2[A2:+1;B1:1]");
		CalcProbe.setExp(rd.getCell(2, 5), "B2[A2:-9;B1:1]");
		return rd;
	}

	/** 根坐标：短形态与完整形态等价。 */
	private static ReportDefine2 root(String name) {
		ReportDefine2 rd = base(name);
		CalcProbe.setExp(rd.getCell(8, 6), "B2[`0;B1:2]");
		CalcProbe.setExp(rd.getCell(8, 7), "B2[`0,A2:2;`0,B1:2]");
		return rd;
	}

	/** 格集：全量 count / 单列 count / 单主格 count（不碰文本 sum）。 */
	private static ReportDefine2 cellset(String name) {
		ReportDefine2 rd = base(name);
		CalcProbe.setExp(rd.getCell(8, 6), "count(B2[`0]{})");
		CalcProbe.setExp(rd.getCell(8, 7), "count(B2[;B1:1]{})");
		CalcProbe.setExp(rd.getCell(8, 8), "count(B2[A2:2]{})");
		return rd;
	}

	/**
	 * `&` 序号：行主格（C2 在 A2 带内）与列主格（B3 在 B1 列带内）。
	 * 手算：C2 三个实例 = 1/2/3；B3 两个副本 = 1/2。
	 */
	private static ReportDefine2 amp(String name) {
		ReportDefine2 rd = base(name);
		CalcProbe.setExp(rd.getCell(2, 3), "&A2");
		CalcProbe.setExp(rd.getCell(3, 2), "&B1");
		return rd;
	}

	/**
	 * 条件格集 + `$`：教程式同比布局（探针 ProbeHc2 实证）。
	 * A2 年（ds1）、B2 月（ds2，左主格 A2）、C2 值、D2 差额（左主格 B2）。
	 * 手算：2025 三行 = 全额（目标为空 ⇒ 当前值）；2026 三行 = 5/5/5；
	 * 去掉 `{...}` 则 2026 M2/M3 错取去年 M1（15/25）——条件承重。
	 */
	private static ReportDefine2 cond(String name) {
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
		CalcProbe.setExp(rd.getCell(2, 4), "C2-C2[A2:-1]{$B2==B2}");
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_hc_cond_years.csv");
		CalcProbe.attachFileDataSet(rd, "ds2", name + ".csv");
		return rd;
	}

	/**
	 * 空数据集：坐标退化为模板单格（探针 17 实测 count=1，不是 0）。
	 * B2 的 `B1+"/"+A2` 在空表下求值为 "/"。
	 */
	private static ReportDefine2 empty(String name) {
		ReportDefine2 rd = base(name);
		CalcProbe.setExp(rd.getCell(8, 6), "count(B2[;B1:1]{})");
		return rd;
	}

	/**
	 * 坐标值作 ds 过滤参数：`product==A2[2]` ⇒ 命中 20/50 两行（探针 10,7/11,7）。
	 * 固定格因此纵向扩展成 2 行——这是产品行为，不是错误。
	 */
	private static ReportDefine2 ds(String name) {
		ReportDefine2 rd = base(name);
		CalcProbe.setExp(rd.getCell(8, 6), "ds1.select(amount, product==A2[2])");
		return rd;
	}

	/**
	 * 三级主格链（部门→组→人，三张维表）：坐标段从远到近。
	 * 手算：`[A2:2,B2:1,C2:2]` = D2/T1 的第 2 人 ⇒ 212；
	 * `[A2:1,B2:2,C2:1]` = D1/T2 的第 1 人 ⇒ 121。
	 */
	private static ReportDefine2 order(String name) {
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
		CalcProbe.setExp(rd.getCell(8, 6), "D2[A2:2,B2:1,C2:2]");
		CalcProbe.setExp(rd.getCell(8, 7), "D2[A2:1,B2:2,C2:1]");
		CalcProbe.attachFileDataSet(rd, "ds1", name + ".csv");
		CalcProbe.attachFileDataSet(rd, "ds2", "engine_hc_order_team.csv");
		CalcProbe.attachFileDataSet(rd, "ds3", "engine_hc_order_person.csv");
		return rd;
	}

	/** `&` 的非主格形态：产品报「A2不是F8的主格」（错误型夹具，只比 class/chain）。 */
	private static ReportDefine2 err(String name) {
		ReportDefine2 rd = base(name);
		CalcProbe.setExp(rd.getCell(8, 6), "&A2");
		return rd;
	}

	/** 坐标自引用：C2 = C2[A2:1] ⇒ 产品报 C2-C2 循环引用。 */
	private static ReportDefine2 cycle(String name) {
		ReportDefine2 rd = base(name);
		CalcProbe.setExp(rd.getCell(2, 3), "C2[A2:1]");
		return rd;
	}

	/**
	 * 坐标落合并区（探针 ProbeHc4 实证的单方向布局）。
	 *
	 * 上一轮把合并区塞进双向交叉矩阵时产品把横向扩展弄歪了 ⇒ 本夹具改成
	 * **只纵向扩展**：A2 部门三行，B2:C2 横向合并，锚点 B2 = `A2+"#"`。
	 *
	 * 手算（探针实测，均为计算态）：
	 * - `B2[A2:1]` = "D1#"（锚点第 1 实例）
	 * - `C2[A2:2]` = "D2#"、`C2[A2:3]` = "D3#"（**延续槽读锚点值**）
	 * - `count(C2[`0]{})` = `count(B2[`0]{})` = **3**（延续槽不被重复计数）
	 * - 合并只留在第 1 实例（(2,3) colMerge=2），第 2/3 实例 colMerge=1
	 */
	private static ReportDefine2 merge(String name) {
		ReportDefine2 rd = new ReportDefine2(12, 8);
		EngineModels.lit(rd, 1, 1, "dept");
		EngineModels.lit(rd, 1, 2, "val");
		INormalCell dept = rd.getCell(2, 1);
		dept.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(dept, "ds1.select(dept)");
		CalcProbe.setExp(rd.getCell(2, 2), "A2+\"#\"");
		// B2:C2 横向合并：B2 锚点、C2 延续槽
		rd.getCell(2, 2).setMergedArea(new Area(2, 2, 2, 3));
		// 锚点：第 1 实例
		CalcProbe.setExp(rd.getCell(8, 6), "B2[A2:1]");
		// 延续槽：第 2 / 3 实例（读锚点值）
		CalcProbe.setExp(rd.getCell(8, 7), "C2[A2:2]");
		CalcProbe.setExp(rd.getCell(8, 8), "C2[A2:3]");
		// 格集：延续槽与锚点计数一致（不被重复计数）
		CalcProbe.setExp(rd.getCell(9, 6), "count(C2[`0]{})");
		CalcProbe.setExp(rd.getCell(9, 7), "count(B2[`0]{})");
		CalcProbe.attachFileDataSet(rd, "ds1", name + ".csv");
		return rd;
	}

	/** 3x3 交叉模板 + 矩阵外标记格（FIXA 在列带内会被复制，FIXB 在带外单份）。 */
	private static ReportDefine2 base(String name) {
		ReportDefine2 rd = new ReportDefine2(12, 8);
		EngineModels.lit(rd, 1, 1, "R\\C");
		INormalCell head = rd.getCell(1, 2);
		head.setExtendMode(INormalCell.EXTEND_HORIZONTAL);
		CalcProbe.setExp(head, "ds1.select(region:1,,,region)");
		INormalCell side = rd.getCell(2, 1);
		side.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(side, "ds1.select(product:1,,,product)");
		CalcProbe.setExp(rd.getCell(2, 2), "B1+\"/\"+A2");
		EngineModels.lit(rd, 10, 2, "FIXA");
		EngineModels.lit(rd, 10, 6, "FIXB");
		CalcProbe.attachFileDataSet(rd, "ds1", name + ".csv");
		return rd;
	}
}
