/*
 * EngineModelsM5.java -- M5 多数据集关联的产品夹具（走 GenEngineFixtures 主管线）。
 *
 * 与 ProbeCross.java 的探针区别：ProbeCross 连金仓真库（SQL 数据集），本类用
 * CSV 文件数据集（attachFileDataSet），因此可由 GenEngineFixtures 离线生成 +
 * round-trip 门校验，不依赖任何外部数据库。语义与 ProbeCross.buildNest 对齐
 * （显式左主格 slot 30 绑定从格带到主格 A3），作为 M5-03 机制的「一等夹具」闭环。
 *
 * 两个模型：
 *   engine_cross      —— 嵌套主子表（经典布局）：主格 ds1.select(id/name/dept/salary)
 *                        + 从格 ds2.select(item/amount, emp_id==A3)，从格带显式左主格 A3。
 *   engine_cross_root —— Root 态跨数据集：无扩展格引用，ds2.select 带字面量过滤
 *                        （amount>=2000），走 Root 态求值路径（对照 cross_root_*）。
 *
 * 数据集（CSV，离线）：
 *   ds1 = engine_cross.csv      （emp：id,name,dept,salary）
 *   ds2 = engine_cross.csv2     （bonus：id,emp_id,item,amount）
 * ds2 的 csv2 文件为静态检入夹具（同 engine_sum.csv2 的模式），GenEngineFixtures
 * 只写 ds1 的 csv，ds2 的 csv2 由仓库提供。
 *
 * 用法（cwd = tests/fixtures）：
 *   java rpx.fixtures.GenEngineFixtures            # 生成全部 18 + 本类 2 = 20 夹具
 *   java rpx.fixtures.GenEngineFixtures --verify . # 全部 round-trip 门
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.scudata.common.Area;

public final class EngineModelsM5 {

	/** 员工主表（ds1）：5 人，与 ProbeCross.bonusRows 同口径。 */
	static String[][] empRows() {
		return new String[][] {
				{ "1", "张伟", "人事部", "8500" },
				{ "2", "李娜", "财务部", "9200" },
				{ "3", "王强", "技术部", "12000" },
				{ "4", "赵敏", "技术部", "11000" },
				{ "5", "孙丽", "市场部", "7800" } };
	}

	/** 奖金明细（ds2）：张伟/赵敏各 2 项（多匹配），孙丽 0 项（无匹配）。 */
	static String[][] bonusRows() {
		return new String[][] {
				{ "1", "1", "全勤奖", "500" },
				{ "2", "1", "年终奖", "3000" },
				{ "3", "2", "全勤奖", "500" },
				{ "4", "3", "餐补", "300" },
				{ "5", "4", "项目奖", "2000" },
				{ "6", "4", "全勤奖", "500" } };
	}

	// ------------------------------------------------------------------
	// engine_cross：嵌套主子表（经典布局，显式左主格）。
	// 与 ProbeCross.buildNest(false) 同构，只是数据集从 SQL 换成 CSV。
	// ------------------------------------------------------------------
	public static ReportDefine2 crossFixture() {
		ReportDefine2 rd = new ReportDefine2(4, 6);
		INormalCell title = rd.getCell(1, 1);
		title.setValue("嵌套主子表·经典布局（显式左主格）");
		title.setMergedArea(new Area(1, 1, 1, 6));

		EngineModels.lit(rd, 2, 1, "编号");
		EngineModels.lit(rd, 2, 2, "姓名");
		EngineModels.lit(rd, 2, 3, "部门");
		EngineModels.lit(rd, 2, 4, "工资");
		EngineModels.lit(rd, 2, 5, "奖金项");
		EngineModels.lit(rd, 2, 6, "金额");

		// 主格带（r3）：ds1 四个字段同带扩展
		CalcProbe.setExp(rd.getCell(3, 1), "ds1.select(id)");
		CalcProbe.setExp(rd.getCell(3, 2), "ds1.select(name)");
		CalcProbe.setExp(rd.getCell(3, 3), "ds1.select(dept)");
		CalcProbe.setExp(rd.getCell(3, 4), "ds1.select(salary)");

		// 从格带（r4）：跨数据集 select 带过滤（引用主格 A3）
		CalcProbe.setExp(rd.getCell(4, 5), "ds2.select(item, emp_id==A3)");
		CalcProbe.setExp(rd.getCell(4, 6), "ds2.select(amount, emp_id==A3)");
		// 显式左主格：从格挂在主格 A3 上（产品向导同构，ReportEditor.java:4770-4776）
		rd.getCell(4, 5).setLeftHead("A3");
		rd.getCell(4, 6).setLeftHead("A3");

		attachDataSets(rd);
		return rd;
	}

	// ------------------------------------------------------------------
	// engine_cross_root：Root 态跨数据集（无扩展格引用，字面量过滤）。
	// 对照 ProbeCross 的 cross_root_filter 变体，但用 CSV 数据集离线可跑。
	// ------------------------------------------------------------------
	public static ReportDefine2 crossRootFixture() {
		ReportDefine2 rd = new ReportDefine2(3, 2);
		INormalCell t = rd.getCell(1, 1);
		t.setValue("Root 态跨数据集：amount>=2000");
		t.setMergedArea(new Area(1, 1, 1, 2));
		EngineModels.lit(rd, 2, 1, "奖金项");
		EngineModels.lit(rd, 2, 2, "金额");
		// Root 态：ds2.select 带字面量过滤，无主格引用 → 走 Root 态求值
		CalcProbe.setExp(rd.getCell(3, 1), "ds2.select(item, amount>=2000)");
		CalcProbe.setExp(rd.getCell(3, 2), "ds2.select(amount, amount>=2000)");
		attachDataSets(rd);
		return rd;
	}

	// ------------------------------------------------------------------
	// engine_crossalign{,_lit,_ds2}（M5-07 维度对齐，探针 ProbeCrossAlign 实证）：
	// 同带 19（横向）× 18（纵向）交叉展开 —— 交叉点格被复制为
	// （横实例数 × 纵实例数）矩阵，且相对引用**分别跟随两个方向的实例**
	// （B1 横向跟列、A2 纵向跟行，产品实测 "华北/A"…"华东/C"）。
	// 一模板一语义点：
	//   engine_crossalign      交叉点 = B1+"/"+A2（相对引用双头 —— 核心语义）
	//   engine_crossalign_lit  交叉点 = 字面量（纯拓扑对照）
	//   engine_crossalign_ds2  横向换 ds2（跨数据集双扩展对照）
	// 数据集：ds1 = engine_crossalign.csv（region 2 值 × product 3 值真交叉），
	//         ds2 = engine_crossalign2.csv（x 列 A/B/C，静态检入，engine_sum.csv2 模式）。
	// ------------------------------------------------------------------
	public static ReportDefine2 crossAlignFixture(String name) {
		ReportDefine2 rd = new ReportDefine2(3, 3);
		EngineModels.lit(rd, 1, 1, "区域\\产品");
		INormalCell head = rd.getCell(1, 2);
		head.setExtendMode(INormalCell.EXTEND_HORIZONTAL);
		if ("engine_crossalign_ds2".equals(name)) {
			CalcProbe.setExp(head, "ds2.select(x:1,,,x)");
		} else {
			CalcProbe.setExp(head, "ds1.select(region:1,,,region)");
		}
		INormalCell side = rd.getCell(2, 1);
		side.setExtendMode(INormalCell.EXTEND_VERTICAL);
		if ("engine_crossalign_ds2".equals(name)) {
			CalcProbe.setExp(side, "ds1.select(region:1,,,region)");
		} else {
			CalcProbe.setExp(side, "ds1.select(product:1,,,product)");
		}
		if ("engine_crossalign".equals(name)) {
			CalcProbe.setExp(rd.getCell(2, 2), "B1+\"/\"+A2");
		} else {
			EngineModels.lit(rd, 2, 2, "·");
		}
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_crossalign.csv");
		CalcProbe.attachFileDataSet(rd, "ds2", "engine_crossalign2.csv");
		return rd;
	}

	/** 两个 CSV 文件数据集：ds1=员工主表，ds2=奖金明细（均 FileDataSet）。 */
	private static void attachDataSets(ReportDefine2 rd) {
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_cross.csv");
		CalcProbe.attachFileDataSet(rd, "ds2", "engine_cross.csv2");
	}
}
