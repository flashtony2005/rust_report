/*
 * ProbeSql.java -- engine_sql: a SQLDataSetConfig fixture backed by a REAL
 * database (KingbaseES V8R6 via the PostgreSQL JDBC driver, wire-compatible).
 *
 * WHY this fixture: M3-03 pinned the FILE dataset path end to end; the SQL
 * dataset path (template-declared SQL hitting a live database through
 * rpx-ds) had no evidence. The product splits this into three pieces
 * (re_work/src/reportlite/05-core-runtime/com/raqsoft/reportl/usermodel):
 *   - SQLDataSetConfig (version=5): template-side fields only
 *   - DataSourceConfig: dbType/charsets (runtime concerns, NOT in the .rpx)
 *   - NormalConnectionFactory(driver,url,user,pwd): the connection itself
 * The .rpx therefore stores ONLY a dataSourceName reference; the caller
 * registers the connection factory in the Context
 * (ctx.setConnectionFactory(dsn, factory)). Credentials never enter the
 * template -- rpx-server must reproduce this split (AssetStore/serve side).
 *
 * Column names are ENGLISH on purpose: the first probe minimises moving
 * parts (PG driver x Kingbase x Chinese identifiers); a Chinese-column
 * variant can follow once the path is green.
 *
 * Table is (re)created by this probe so the fixture is self-contained:
 *   DROP TABLE IF EXISTS rpx_sql_smoke; CREATE ...; INSERT 5 rows.
 * Rows mirror engine_emp so the expansion shape is already known.
 *
 * Layout (3 rows x 4 cols):
 *   r1  id | dept | salary | total   (header literals)
 *   r2  ds1.select(id) | ds1.dept | ds1.salary | sum(ds1.salary)
 *
 * Usage:
 *   ProbeSql <fixturesDir> <jdbcUrl> <user> <password>
 * Writes engine_sql.rpx + engine_sql.expected.json, then round-trips
 * (re-read the .rpx, re-calc, re-render) and fails loudly on a mismatch.
 */
package rpx.fixtures;

import java.io.File;
import java.io.FileWriter;
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
import com.raqsoft.reportl.usermodel.IRowCell;
import com.raqsoft.reportl.usermodel.SQLDataSetConfig;
import com.raqsoft.reportl.util.ReportUtils;

public class ProbeSql {

	public static void main(String[] args) throws Exception {
		if (args.length < 4) {
			throw new IllegalArgumentException(
					"usage: ProbeSql <fixturesDir> <jdbcUrl> <user> <password>");
		}
		String dir = args[0];
		String url = args[1];
		String user = args[2];
		String pwd = args[3];
		String dsn = "kingbase";

		seedDatabase(url, user, pwd);
		ReportDefine2 rd = buildTemplate();

		String rpxPath = dir + File.separator + "engine_sql.rpx";
		ReportUtils.write(rpxPath, rd);
		System.out.println("wrote " + rpxPath + " ("
				+ new File(rpxPath).length() + " bytes)");

		// First pass: calculate from the freshly built template.
		String first = render(rd, calcWithConnection(rd, dsn, url, user, pwd));
		String jsonPath = dir + File.separator + "engine_sql.expected.json";
		try (java.io.OutputStreamWriter w = new java.io.OutputStreamWriter(
				new java.io.FileOutputStream(jsonPath), StandardCharsets.UTF_8)) {
			w.write(first);
			w.write("\n");
		}
		System.out.println("wrote " + jsonPath);

		// Round-trip gate: re-read the binary and re-calculate; the JSON must
		// match byte for byte (same gate as GenEngineFixtures --verify).
		ReportDefine2 rd2 = (ReportDefine2) ReportUtils.read(rpxPath);
		String second = render(rd2, calcWithConnection(rd2, dsn, url, user, pwd));
		if (!first.trim().equals(second.trim())) {
			System.out.println("FAIL engine_sql: round-trip JSON mismatch");
			System.exit(1);
		}
		System.out.println("VERIFY OK: engine_sql round-trip");
	}

	/** Creates and seeds the smoke table (idempotent). */
	private static void seedDatabase(String url, String user, String pwd)
			throws Exception {
		Class.forName("org.postgresql.Driver");
		try (Connection con = DriverManager.getConnection(url, user, pwd);
				Statement st = con.createStatement()) {
			st.execute("DROP TABLE IF EXISTS rpx_sql_smoke");
			st.execute("CREATE TABLE rpx_sql_smoke ("
					+ "id INTEGER PRIMARY KEY, dept VARCHAR(20), salary INTEGER)");
			String[][] rows = rows();
			for (String[] r : rows) {
				st.execute("INSERT INTO rpx_sql_smoke VALUES (" + r[0] + ", '"
						+ r[1] + "', " + r[2] + ")");
			}
		}
		System.out.println("seeded rpx_sql_smoke (" + rows().length + " rows)");
	}

	/** The template: header row + one SQL-dataset expansion band. */
	private static ReportDefine2 buildTemplate() {
		ReportDefine2 rd = new ReportDefine2(2, 4);
		rd.getCell(1, 1).setValue("id");
		rd.getCell(1, 2).setValue("dept");
		rd.getCell(1, 3).setValue("salary");
		rd.getCell(1, 4).setValue("total");
		CalcProbe.setExp(rd.getCell(2, 1), "ds1.select(id)");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.dept");
		CalcProbe.setExp(rd.getCell(2, 3), "ds1.salary");
		CalcProbe.setExp(rd.getCell(2, 4), "sum(ds1.salary)");

		SQLDataSetConfig dsc = new SQLDataSetConfig();
		dsc.setName("ds1");
		dsc.setDataSourceName("kingbase");
		dsc.setSQL("SELECT id, dept, salary FROM rpx_sql_smoke ORDER BY id");
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

	/** Fixture rows (mirror engine_emp). */
	private static String[][] rows() {
		return new String[][] { { "1", "HR", "8500" }, { "2", "FIN", "9200" },
				{ "3", "DEV", "12000" }, { "4", "DEV", "11000" },
				{ "5", "MKT", "7800" } };
	}

	/** Mirrors GenEngineFixtures.render: same top-level key set. */
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
				if (cell.getMergeSame() != 97) {
					o.put("merge_same", cell.getMergeSame());
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
		o.putString("name", "engine_sql");
		o.put("unit", rd.getUnit());
		o.putString("unit_note", "1=mm 0=inch 2=point; row/column sizes use this unit");
		o.putString("dataset_file", "SQL[kingbase]: SELECT id, dept, salary FROM"
				+ " rpx_sql_smoke ORDER BY id");
		// The dataset section is informational for file fixtures; for the SQL
		// fixture the live database is the source of truth, so it is null.
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
