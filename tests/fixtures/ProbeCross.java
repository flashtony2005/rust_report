/*
 * ProbeCross.java -- M5 立项探针：多数据集关联（主子表）的产品真机语义。
 *
 * 背景：数据蒋堂讲座（docs/lecture-notes.md 行动序 #2）把「多数据集关联」列为
 * 报表工具的核心差异化，官方口径（docs/report-model-and-cell-extend.md §3.3）：
 *   「润乾报表的多个数据集不需要进行整合运算，而是直接用于报表的运算，在报表的
 *     运算过程中，通过一定的规则把多个数据集关联起来」——没有 join、没有临时表，
 *     关联发生在求值时刻，粒度是「当前格的主格指向的那条记录」。
 *   官方主子表样例（dpkz101.html）：
 *     B2 = ds1.select(订单ID:1,,,订单ID)                    ← 主表：去重扩展
 *     B8 = ds2.select(产品ID:1, 订单ID==B2,,产品ID)          ← 子表：用主格过滤
 *
 * 本探针在金仓 report schema 上用两张真表复刻该模式，并额外测 Root 态变体：
 *
 *   report.emp_info   (id, dept, name, salary)   5 名员工（ProbeSqlDemo 已建）
 *   report.emp_bonus  (id, emp_id, item, amount) 奖金明细，本探针建：
 *     张伟(1) 2 项（多匹配） / 李娜(2) 1 项 / 王强(3) 1 项 /
 *     赵敏(4) 2 项（多匹配） / 孙丽(5) 0 项（无匹配）
 *
 * 三个模板（各自 round-trip 门）：
 *   T1 cross_emp_bonus —— 经典主子表：主格 ds1.select(id)，从格
 *      ds2.select(item, emp_id==A3)（过滤条件引用主格单元格，CSVariable）；
 *      另测 num 限量/去重/次序变体与 Row 态跟随格（裸 ds1.col）。
 *   T2 cross_dept_band —— 去重主格 ds1.select(dept:1,,,dept) + 同数据集从格
 *      ds1.select(name, dept==A3) + 跨数据集从格 ds2.select(item, emp_id==C3)。
 *   T3 cross_root      —— Root 态（无扩展格引用）select 带过滤/限量/去重、
 *      裸 ds2.col、count()。
 *
 * 产出：out/cross_emp_bonus_product.json / cross_dept_band / cross_root
 * （产品真值，供 M5 实现期对拍）；模板与 truth 均不含凭据。
 *
 * 用法（cwd 必须为 tests/fixtures）：
 *   "F:/project/report/common/jre/bin/java.exe" -Dfile.encoding=UTF-8 \
 *     -cp "F:/project/report/reportlite/lib/*;out" rpx.fixtures.ProbeCross \
 *     <fixturesDir> <jdbcUrl> <user> <password>
 */
package rpx.fixtures;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.Statement;

import com.raqsoft.reportl.dataset.NormalConnectionFactory;
import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.Context;
import com.raqsoft.reportl.usermodel.DataSetMetaData;
import com.raqsoft.reportl.usermodel.Engine;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.usermodel.SQLDataSetConfig;
import com.raqsoft.reportl.util.ReportUtils;
import com.scudata.common.Area;

public class ProbeCross {

	private static final String EMP_SQL = "SELECT id, dept, name, salary FROM "
			+ "report.emp_info ORDER BY id";
	private static final String BONUS_SQL = "SELECT id, emp_id, item, amount FROM "
			+ "report.emp_bonus ORDER BY id";

	public static void main(String[] args) throws Exception {
		if (args.length < 4) {
			throw new IllegalArgumentException(
					"usage: ProbeCross <fixturesDir> <jdbcUrl> <user> <password>");
		}
		String dir = args[0];
		String url = args[1];
		String user = args[2];
		String pwd = args[3];
		String dsn = "kingbase";

		seedBonus(url, user, pwd);

		run(dir, "cross_emp_bonus", buildEmpBonus(), dsn, url, user, pwd);
		run(dir, "cross_dept_band", buildDeptBand(), dsn, url, user, pwd);
		for (String[] v : ROOT_VARIANTS) {
			run(dir, v[0], buildRootVariant(v[1], v[2]), dsn, url, user, pwd);
		}
		run(dir, "cross_nest", buildNest(false), dsn, url, user, pwd);
		run(dir, "cross_nest_vals", buildNest(true), dsn, url, user, pwd);
		System.out.println("VERIFY OK: probe cross round-trip (9 templates)");
	}

	// ------------------------------------------------------------------
	// 建表：report.emp_bonus（幂等）。emp_info 由 ProbeSqlDemo 负责，
	// 这里只读不改（两探针共用同一张主表，正是「多数据集模型」的常态）。
	// ------------------------------------------------------------------
	private static void seedBonus(String url, String user, String pwd)
			throws Exception {
		Class.forName("org.postgresql.Driver");
		try (Connection con = DriverManager.getConnection(url, user, pwd);
				Statement st = con.createStatement()) {
			st.execute("CREATE SCHEMA IF NOT EXISTS report");
			st.execute("DROP TABLE IF EXISTS report.emp_bonus");
			st.execute("CREATE TABLE report.emp_bonus ("
					+ "id INTEGER PRIMARY KEY, emp_id INTEGER, "
					+ "item VARCHAR(40), amount INTEGER)");
			for (String[] r : bonusRows()) {
				st.execute("INSERT INTO report.emp_bonus VALUES (" + r[0]
						+ ", " + r[1] + ", '" + r[2] + "', " + r[3] + ")");
			}
		}
		System.out.println("seeded report.emp_bonus ("
				+ bonusRows().length + " rows)");
	}

	/** 奖金明细：张伟/赵敏各 2 项（多匹配），孙丽 0 项（无匹配）。 */
	private static String[][] bonusRows() {
		return new String[][] { { "1", "1", "全勤奖", "500" },
				{ "2", "1", "年终奖", "3000" }, { "3", "2", "全勤奖", "500" },
				{ "4", "3", "餐补", "300" }, { "5", "4", "项目奖", "2000" },
				{ "6", "4", "全勤奖", "500" } };
	}

	// ------------------------------------------------------------------
	// T1 经典主子表：主格员工 + 从格奖金（过滤引用主格 A3）
	//   r1 标题（跨 6 列合并）
	//   r2 表头：编号 | 姓名 | 部门 | 工资 | 奖金项 | 金额
	//   r3 主格带：ds1.select(id/name/dept/salary)      —— 5 名员工
	//   r4 从格带：ds1.id/name/dept/salary（Row 态跟随）
	//              + ds2.select(item, emp_id==A3) / ds2.select(amount, emp_id==A3)
	//              + 变体：限量 ds2.select(item, emp_id==A3, 1)
	//                    去重 ds2.select(item, , , item)
	//                    降序 ds2.select(item:-1, emp_id==A3)
	// ------------------------------------------------------------------
	private static ReportDefine2 buildEmpBonus() {
		ReportDefine2 rd = new ReportDefine2(4, 9);
		INormalCell title = rd.getCell(1, 1);
		title.setValue("员工奖金主子表（金仓 report schema 真数）");
		title.setMergedArea(new Area(1, 1, 1, 9));

		EngineModels.lit(rd, 2, 1, "编号");
		EngineModels.lit(rd, 2, 2, "姓名");
		EngineModels.lit(rd, 2, 3, "部门");
		EngineModels.lit(rd, 2, 4, "工资");
		EngineModels.lit(rd, 2, 5, "奖金项");
		EngineModels.lit(rd, 2, 6, "金额");
		EngineModels.lit(rd, 2, 7, "限量1项");
		EngineModels.lit(rd, 2, 8, "去重项");
		EngineModels.lit(rd, 2, 9, "降序项");

		// 主格带（r3）：ds1 四个字段同带扩展
		CalcProbe.setExp(rd.getCell(3, 1), "ds1.select(id)");
		CalcProbe.setExp(rd.getCell(3, 2), "ds1.select(name)");
		CalcProbe.setExp(rd.getCell(3, 3), "ds1.select(dept)");
		CalcProbe.setExp(rd.getCell(3, 4), "ds1.select(salary)");

		// 从格带（r4）：前四列 Row 态跟随（官方推荐 dsn.colname 形态），
		// 后四列跨数据集 select 带过滤（引用主格 A3）。
		CalcProbe.setExp(rd.getCell(4, 1), "ds1.id");
		CalcProbe.setExp(rd.getCell(4, 2), "ds1.name");
		CalcProbe.setExp(rd.getCell(4, 3), "ds1.dept");
		CalcProbe.setExp(rd.getCell(4, 4), "ds1.salary");
		CalcProbe.setExp(rd.getCell(4, 5), "ds2.select(item, emp_id==A3)");
		CalcProbe.setExp(rd.getCell(4, 6), "ds2.select(amount, emp_id==A3)");
		CalcProbe.setExp(rd.getCell(4, 7), "ds2.select(item, emp_id==A3, 1)");
		CalcProbe.setExp(rd.getCell(4, 8), "ds2.select(item, , , item)");
		CalcProbe.setExp(rd.getCell(4, 9), "ds2.select(item:-1, emp_id==A3)");

		addDataSets(rd);
		return rd;
	}

	// ------------------------------------------------------------------
	// T2 去重主格 + 两层从格：部门（去重）→ 员工（同数据集过滤）→ 奖金
	// （跨数据集，过滤引用员工编号格 C3）。这是官方样例的同构复刻。
	//   r3: A3=ds1.select(dept:1,,,dept)  B3=ds1.select(name, dept==A3)
	//       C3=ds1.select(id, dept==A3)   D3=ds1.select(salary, dept==A3)
	//   r4: E4=ds2.select(item, emp_id==C3)  F4=ds2.select(amount, emp_id==C3)
	// ------------------------------------------------------------------
	private static ReportDefine2 buildDeptBand() {
		ReportDefine2 rd = new ReportDefine2(4, 6);
		INormalCell title = rd.getCell(1, 1);
		title.setValue("部门-员工-奖金三级关联（去重主格）");
		title.setMergedArea(new Area(1, 1, 1, 6));

		EngineModels.lit(rd, 2, 1, "部门");
		EngineModels.lit(rd, 2, 2, "姓名");
		EngineModels.lit(rd, 2, 3, "编号");
		EngineModels.lit(rd, 2, 4, "工资");
		EngineModels.lit(rd, 2, 5, "奖金项");
		EngineModels.lit(rd, 2, 6, "金额");

		CalcProbe.setExp(rd.getCell(3, 1), "ds1.select(dept:1,,,dept)");
		CalcProbe.setExp(rd.getCell(3, 2), "ds1.select(name, dept==A3)");
		CalcProbe.setExp(rd.getCell(3, 3), "ds1.select(id, dept==A3)");
		CalcProbe.setExp(rd.getCell(3, 4), "ds1.select(salary, dept==A3)");
		CalcProbe.setExp(rd.getCell(4, 5), "ds2.select(item, emp_id==C3)");
		CalcProbe.setExp(rd.getCell(4, 6), "ds2.select(amount, emp_id==C3)");

		addDataSets(rd);
		return rd;
	}

	// ------------------------------------------------------------------
	// T3 Root 态变体：每个变体单独一个模板（隔离共带干扰——T3 合带版实测：
	// 同带扩展格的 evaluateRows 会留下 ds.current 状态，互相污染，见
	 // spec-m5 §4.4 的探针记录）。每模板只有一个表达式格 + 一个表头。
	//   r2 表头（变体名）
	//   r3 唯一表达式格
	// ------------------------------------------------------------------
	private static final String[][] ROOT_VARIANTS = {
			{ "cross_root_filter", "过滤List", "ds2.select(item, amount>=2000)" },
			{ "cross_root_num", "限量1", "ds2.select(item, emp_id==1, 1)" },
			{ "cross_root_distinct", "去重List", "ds2.select(item, , , item)" },
			{ "cross_root_desc", "降序List", "ds2.select(item:-1)" },
			{ "cross_root_bare", "裸列", "ds2.item" },
			{ "cross_root_count", "计数", "ds2.count()" } };

	private static ReportDefine2 buildRootVariant(String title, String exp) {
		ReportDefine2 rd = new ReportDefine2(3, 2);
		INormalCell t = rd.getCell(1, 1);
		t.setValue("Root 态：" + title);
		t.setMergedArea(new Area(1, 1, 1, 2));
		EngineModels.lit(rd, 2, 1, title);
		EngineModels.lit(rd, 2, 2, "值");
		CalcProbe.setExp(rd.getCell(3, 1), exp);
		addDataSets(rd);
		return rd;
	}

	// ------------------------------------------------------------------
	// T4/T5 嵌套主子表：显式左主格（NormalCell slot 30 = leftHead 单元格 ID）
	// 绑定从格带到主格。这是产品向导生成主子表/分组报表的机制
	// （ReportEditor.java:4770-4776 `ac.setProperty((byte)30)` +
	// `CellPosition.toExcelNotation(...)`）；ExtCellSet._$4/_$3 的行跨度
	 // 计算把左子格纳入 ⇒ 主格扩展覆盖从格带，extendRow 的复制循环
	 // （`cell.isLeftHeadOf(sub)`）把从格逐实例克隆。
	//   withValues=false：从格带只放 E/F 两列（经典布局，主格跨行合并）
	//   withValues=true ：A-D 列也放 ds1.col（Row 态），全部挂显式左主格
	// ------------------------------------------------------------------
	private static ReportDefine2 buildNest(boolean withValues) {
		ReportDefine2 rd = new ReportDefine2(4, 6);
		INormalCell title = rd.getCell(1, 1);
		title.setValue(withValues ? "嵌套主子表·从格带跟值（显式左主格）"
				: "嵌套主子表·经典布局（显式左主格）");
		title.setMergedArea(new Area(1, 1, 1, 6));

		EngineModels.lit(rd, 2, 1, "编号");
		EngineModels.lit(rd, 2, 2, "姓名");
		EngineModels.lit(rd, 2, 3, "部门");
		EngineModels.lit(rd, 2, 4, "工资");
		EngineModels.lit(rd, 2, 5, "奖金项");
		EngineModels.lit(rd, 2, 6, "金额");

		// 主格带（r3）
		CalcProbe.setExp(rd.getCell(3, 1), "ds1.select(id)");
		CalcProbe.setExp(rd.getCell(3, 2), "ds1.select(name)");
		CalcProbe.setExp(rd.getCell(3, 3), "ds1.select(dept)");
		CalcProbe.setExp(rd.getCell(3, 4), "ds1.select(salary)");

		// 从格带（r4）：跨数据集 select 带过滤（引用主格 A3）
		CalcProbe.setExp(rd.getCell(4, 5), "ds2.select(item, emp_id==A3)");
		CalcProbe.setExp(rd.getCell(4, 6), "ds2.select(amount, emp_id==A3)");
		// 显式左主格：从格挂在主格 A3 上（产品向导同构）
		rd.getCell(4, 5).setLeftHead("A3");
		if (withValues) {
			CalcProbe.setExp(rd.getCell(4, 1), "ds1.id");
			CalcProbe.setExp(rd.getCell(4, 2), "ds1.name");
			CalcProbe.setExp(rd.getCell(4, 3), "ds1.dept");
			CalcProbe.setExp(rd.getCell(4, 4), "ds1.salary");
			for (int c = 1; c <= 4; c++) {
				rd.getCell(4, c).setLeftHead("A3");
			}
		}

		addDataSets(rd);
		return rd;
	}

	/** 两个 SQL 数据集：ds1=员工主表，ds2=奖金明细（均只存 dsn 引用）。 */
	private static void addDataSets(ReportDefine2 rd) {
		if (rd.getDataSetMetaData() == null) {
			rd.setDataSetMetaData(new DataSetMetaData());
		}
		SQLDataSetConfig d1 = new SQLDataSetConfig();
		d1.setName("ds1");
		d1.setDataSourceName("kingbase");
		d1.setSQL(EMP_SQL);
		rd.getDataSetMetaData().addDataSetConfig(d1);
		SQLDataSetConfig d2 = new SQLDataSetConfig();
		d2.setName("ds2");
		d2.setDataSourceName("kingbase");
		d2.setSQL(BONUS_SQL);
		rd.getDataSetMetaData().addDataSetConfig(d2);
	}

	/** 跑一个模板：calc → render → 落 JSON → 回读 round-trip 门。 */
	private static void run(String dir, String name, ReportDefine2 rd,
			String dsn, String url, String user, String pwd) throws Exception {
		String first = render(rd, calcWithConnection(rd, dsn, url, user, pwd));
		String jsonPath = dir + File.separator + "out" + File.separator + name
				+ "_product.json";
		try (java.io.OutputStreamWriter w = new java.io.OutputStreamWriter(
				new java.io.FileOutputStream(jsonPath), StandardCharsets.UTF_8)) {
			w.write(first);
			w.write("\n");
		}
		System.out.println("wrote " + jsonPath);

		String rpxPath = dir + File.separator + name + ".rpx";
		ReportUtils.write(rpxPath, rd);
		ReportDefine2 rd2 = (ReportDefine2) ReportUtils.read(rpxPath);
		String second = render(rd2,
				calcWithConnection(rd2, dsn, url, user, pwd));
		if (!first.trim().equals(second.trim())) {
			System.out.println("FAIL " + name + ": round-trip JSON mismatch");
			System.exit(1);
		}
		System.out.println("VERIFY OK: " + name + " round-trip");
	}

	/** 产品引擎 + NormalConnectionFactory（凭据只在 Context，不进模板）。 */
	private static IReport calcWithConnection(ReportDefine2 rd, String dsn,
			String url, String user, String pwd) throws Exception {
		Context ctx = new Context();
		ctx.setConnectionFactory(dsn, new NormalConnectionFactory(
				"org.postgresql.Driver", url, user, pwd));
		return new Engine(rd, ctx).calc();
	}

	/** 与 ProbeSql.render 同构：formulas + row_cells + calculated + 分页 + 合并。 */
	private static String render(ReportDefine2 rd, IReport rpt) {
		ExtCellSet ecs = (ExtCellSet) rpt;
		JsonArr formulas = new JsonArr();
		for (int r = 1; r <= rd.getRowCount(); r++) {
			for (int c = 1; c <= rd.getColCount(); c++) {
				INormalCell cell = rd.getCell(r, c);
				String exp = CalcProbe.expOf(cell);
				if (exp == null) {
					continue;
				}
				JsonObj o = new JsonObj();
				o.put("r", r);
				o.put("c", c);
				o.putString("exp", exp);
				if (cell.getExtendMode() != 16) {
					o.put("extend_mode", cell.getExtendMode());
				}
				formulas.add(o);
			}
		}
		JsonArr rowTypes = new JsonArr();
		for (int r = 1; r <= rd.getRowCount(); r++) {
			com.raqsoft.reportl.usermodel.IRowCell rc = rd.getRowCell(r);
			JsonObj o = new JsonObj();
			o.put("r", r);
			o.put("row_type", rc.getRowType());
			o.put("group_header_level", rc.getGroupHeaderLevel());
			o.put("row_height", rc.getRowHeight());
			rowTypes.add(o);
		}
		JsonObj o = new JsonObj();
		o.putString("dataset_file", "SQL[kingbase]: emp_info + emp_bonus");
		o.put("dataset", "null");
		o.put("template_rows", rd.getRowCount());
		o.put("template_cols", rd.getColCount());
		o.put("formulas", formulas);
		o.put("row_cells", rowTypes);
		o.put("calculated", CalcJson.renderCalc(ecs));
		o.put("pagination", CalcJson.renderPages(ecs, 595, 842, 1));
		o.put("merged", CalcMergeJson.renderMerged(ecs, 595, 842, 1));
		return o.done();
	}
}
