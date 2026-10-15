/*
 * ProbeSqlDemo.java -- browser-demo template: a SQLDataSetConfig hitting the
 * `report` schema in the KingbaseES `test` database (PG wire-compatible).
 *
 * Unlike ProbeSql (fixture generation with round-trip gates), this probe
 * prepares a DEMO: it creates the schema + table the director asked for,
 * seeds Chinese employee rows, writes report_demo.rpx, and dumps the
 * product engine's own snapshot JSON for comparison with the Rust side.
 *
 * Layout (3 rows x 4 cols):
 *   r1  title merged across 4 cols: 员工信息查询（金仓 report schema）
 *   r2  headers: 编号 | 部门 | 姓名 | 工资
 *   r3  ds1.select(id) | ds1.dept | ds1.name | ds1.salary
 *
 * Usage:
 *   ProbeSqlDemo <fixturesDir> <jdbcUrl> <user> <password>
 * Writes report_demo.rpx + out/report_demo_product.json (product truth).
 */
package rpx.fixtures;

import java.io.File;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.Statement;

import com.raqsoft.reportl.dataset.NormalConnectionFactory;
import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.Context;
import com.raqsoft.reportl.usermodel.Engine;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.usermodel.SQLDataSetConfig;
import com.raqsoft.reportl.util.ReportUtils;
import com.scudata.common.Area;

public class ProbeSqlDemo {

	private static final String SCHEMA = "report";
	private static final String TABLE = "report.emp_info";
	private static final String SQL = "SELECT id, dept, name, salary FROM "
			+ TABLE + " ORDER BY id";

	public static void main(String[] args) throws Exception {
		if (args.length < 4) {
			throw new IllegalArgumentException(
					"usage: ProbeSqlDemo <fixturesDir> <jdbcUrl> <user> <password>");
		}
		String dir = args[0];
		String url = args[1];
		String user = args[2];
		String pwd = args[3];
		String dsn = "kingbase";

		seedDatabase(url, user, pwd);
		ReportDefine2 rd = buildTemplate();

		String rpxPath = dir + File.separator + "report_demo.rpx";
		ReportUtils.write(rpxPath, rd);
		System.out.println("wrote " + rpxPath + " ("
				+ new File(rpxPath).length() + " bytes)");

		// Product truth: the live engine's calc grid, same JSON shape as the
		// fixture probes (formulas + calculated cells) for spot comparison.
		IReport rpt = calcWithConnection(rd, dsn, url, user, pwd);
		String truth = render((ExtCellSet) rpt);
		String outPath = dir + File.separator + "out"
				+ File.separator + "report_demo_product.json";
		try (java.io.OutputStreamWriter w = new java.io.OutputStreamWriter(
				new java.io.FileOutputStream(outPath),
				java.nio.charset.StandardCharsets.UTF_8)) {
			w.write(truth);
			w.write("\n");
		}
		System.out.println("wrote " + outPath);

		// Round-trip gate: re-read + re-calc must agree byte for byte.
		ReportDefine2 rd2 = (ReportDefine2) ReportUtils.read(rpxPath);
		String second = render((ExtCellSet) calcWithConnection(rd2, dsn,
				url, user, pwd));
		if (!truth.trim().equals(second.trim())) {
			System.out.println("FAIL report_demo: round-trip mismatch");
			System.exit(1);
		}
		System.out.println("VERIFY OK: report_demo round-trip");
	}

	/** Creates the report schema + demo table (schema persists, table idempotent). */
	private static void seedDatabase(String url, String user, String pwd)
			throws Exception {
		Class.forName("org.postgresql.Driver");
		try (Connection con = DriverManager.getConnection(url, user, pwd);
				Statement st = con.createStatement()) {
			st.execute("CREATE SCHEMA IF NOT EXISTS " + SCHEMA);
			st.execute("DROP TABLE IF EXISTS " + TABLE);
			st.execute("CREATE TABLE " + TABLE + " ("
					+ "id INTEGER PRIMARY KEY, dept VARCHAR(20), "
					+ "name VARCHAR(20), salary INTEGER)");
			String[][] rows = rows();
			for (String[] r : rows) {
				st.execute("INSERT INTO " + TABLE + " VALUES (" + r[0]
						+ ", '" + r[1] + "', '" + r[2] + "', " + r[3] + ")");
			}
		}
		System.out.println("seeded " + SCHEMA + " schema + " + TABLE
				+ " (" + rows().length + " rows)");
	}

	/** The demo template: merged title + header row + SQL expansion band. */
	private static ReportDefine2 buildTemplate() {
		ReportDefine2 rd = new ReportDefine2(3, 4);
		INormalCell title = rd.getCell(1, 1);
		title.setValue("员工信息查询（金仓 report schema 真数）");
		title.setMergedArea(new Area(1, 1, 1, 4));

		EngineModels.lit(rd, 2, 1, "编号");
		EngineModels.lit(rd, 2, 2, "部门");
		EngineModels.lit(rd, 2, 3, "姓名");
		EngineModels.lit(rd, 2, 4, "工资");

		CalcProbe.setExp(rd.getCell(3, 1), "ds1.select(id)");
		CalcProbe.setExp(rd.getCell(3, 2), "ds1.dept");
		CalcProbe.setExp(rd.getCell(3, 3), "ds1.name");
		CalcProbe.setExp(rd.getCell(3, 4), "ds1.salary");

		SQLDataSetConfig dsc = new SQLDataSetConfig();
		dsc.setName("ds1");
		dsc.setDataSourceName("kingbase");
		dsc.setSQL(SQL);
		if (rd.getDataSetMetaData() == null) {
			rd.setDataSetMetaData(new com.raqsoft.reportl.usermodel.DataSetMetaData());
		}
		rd.getDataSetMetaData().addDataSetConfig(dsc);
		return rd;
	}

	/** Runs the product engine with a NormalConnectionFactory in the Context. */
	private static IReport calcWithConnection(ReportDefine2 rd, String dsn,
			String url, String user, String pwd) throws Exception {
		Context ctx = new Context();
		ctx.setConnectionFactory(dsn, new NormalConnectionFactory(
				"org.postgresql.Driver", url, user, pwd));
		return new Engine(rd, ctx).calc();
	}

	/** Demo rows (mirror engine_emp so the expansion shape is known). */
	private static String[][] rows() {
		return new String[][] { { "1", "人事部", "张伟", "8500" },
				{ "2", "财务部", "李娜", "9200" }, { "3", "技术部", "王强", "12000" },
				{ "4", "技术部", "赵敏", "11000" }, { "5", "市场部", "孙丽", "7800" } };
	}

	/** Calc-grid JSON (spot-comparison shape; not a fixture expected file). */
	private static String render(ExtCellSet ecs) {
		return CalcJson.renderCalc(ecs);
	}
}
