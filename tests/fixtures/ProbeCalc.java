/*
 * ProbeCalc.java -- P0-2 headless calculation probe (phase driver).
 *
 * Question: can the product compute a .rpx to its "calculated state"
 * (ExtCellSet) without a GUI and without a database, and can that state be
 * dumped as JSON ground truth for the Rust engine (rpx-engine, M2)?
 *
 *   A  simple.rpx            -- no dataset at all
 *   B  full.rpx              -- FileDataSetConfig -> missing data.csv
 *   C  full.rpx + data.csv   -- real FileDataSetFactory path
 *   D  in-memory dataset     -- DataSet injected into Context, expression cells
 *   E  TextReport view       -- lightweight text channel over the calc result
 *   F  expression variants   -- cell refs / ranges over the extended grid
 *   G  pagination            -- PageBuilder over the calculated grid
 *   H  group header rows     -- rowType -90 + groupHeaderLevel in pagination
 *
 * Shared model builders / dumpers live in CalcProbe.java.
 *
 * Compile (Java 8 target, product jars on classpath):
 *   javac --release 8 -cp "F:/project/report/reportlite/lib/*" *.java -d out
 * Run (product JRE 1.8, from the fixtures directory):
 *   "F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
 *       rpx.fixtures.ProbeCalc . > probe_out.txt 2> probe_err.txt
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.usermodel.IRowCell;
import com.raqsoft.reportl.util.ReportUtils;
import com.raqsoft.reportl.view.text.TextReport;

import java.io.File;

public class ProbeCalc {

	public static void main(String[] args) throws Exception {
		String dir = args.length > 0 ? args[0] : ".";
		int bad = 0;
		bad += phaseA(dir + "/simple.rpx");
		bad += phaseB(dir + "/full.rpx");
		bad += phaseC(dir);
		bad += phaseD();
		bad += phaseE(dir);
		bad += phaseF();
		bad += phaseG();
		bad += phaseH();
		System.out.println("----");
		System.out.println(bad == 0
				? "PROBE OK: headless calculation is achievable"
				: "PROBE PARTIAL: " + bad + " phase(s) blocked");
		System.exit(bad == 0 ? 0 : 2);
	}

	/** Phase A -- no dataset: the pure structural path. */
	private static int phaseA(String path) {
		System.out.println("== Phase A: headless calc, no dataset (" + path + ")");
		try {
			CalcProbe.dump("A", CalcProbe.calc((ReportDefine2) ReportUtils.read(path), null));
			return 0;
		} catch (Throwable t) {
			return CalcProbe.fail("A", t);
		}
	}

	/** Phase B -- dataset config present, file missing: is it fatal? */
	private static int phaseB(String path) {
		System.out.println("== Phase B: calc with FileDataSetConfig, data.csv absent ("
				+ path + ")");
		try {
			CalcProbe.dump("B", CalcProbe.calc((ReportDefine2) ReportUtils.read(path), null));
			return 0;
		} catch (Throwable t) {
			System.out.println("[B] calc threw: " + t.getClass().getName() + ": "
					+ t.getMessage());
			System.out.println("[B] -> a missing dataset source IS fatal unless the dataset");
			System.out.println("[B]    is pre-injected into Context (see Phase D)");
			return 0; // informational, not a probe failure
		}
	}

	/** Phase C -- real FileDataSetFactory reading a real CSV. */
	private static int phaseC(String dir) {
		System.out.println("== Phase C: calc through the real FileDataSetFactory");
		try {
			String csv = dir + File.separator + "data.csv";
			CalcProbe.writeCsv(csv, new String[] { "region", "qty" }, new String[][] {
					{ "North", "100" }, { "East", "200" }, { "South", "150" } });
			System.out.println("[C] wrote " + csv);
			CalcProbe.dump("C", CalcProbe.calc(listReport(4), null));
			new File(csv).delete();
			return 0;
		} catch (Throwable t) {
			return CalcProbe.fail("C", t);
		}
	}

	/** Phase D -- in-memory DataSet injected into Context. */
	private static int phaseD() {
		System.out.println("== Phase D: in-memory DataSet injection + expression cells");
		try {
			CalcProbe.dump("D", CalcProbe.calc(listReport(4), CalcProbe.stdDataSet()));
			return 0;
		} catch (Throwable t) {
			return CalcProbe.fail("D", t);
		}
	}

	/** Phase E -- TextReport as a lightweight cross-check channel. */
	private static int phaseE(String dir) {
		System.out.println("== Phase E: TextReport view over the calculated grid");
		try {
			IReport rpt = CalcProbe.calc((ReportDefine2) ReportUtils.read(dir
					+ "/simple.rpx"), null);
			String text = new TextReport(rpt).toString();
			System.out.println("[E] TextReport length=" + text.length()
					+ " first line: " + firstLine(text));
			return 0;
		} catch (Throwable t) {
			return CalcProbe.fail("E", t);
		}
	}

	/** Phase F -- expression variants over the extended grid. */
	private static int phaseF() {
		System.out.println("== Phase F: expression variants over the extended grid");
		String[][] variants = {
				{ "sum($B2:B2)", "range, absolute col, single cell" },
				{ "sum(B2:B2)", "range, relative, single cell" },
				{ "sum($B2:B4)", "range, absolute col, multi cell" },
				{ "B2*2", "plain cell ref arithmetic (control)" },
		};
		int bad = 0;
		for (String[] v : variants) {
			try {
				ReportDefine2 rd = listReport(4);
				CalcProbe.setExp(rd.getCell(2, 4), v[0]);
				System.out.println("[F] --- " + v[0] + "  (" + v[1] + ")");
				CalcProbe.dump("F", CalcProbe.calc(rd, CalcProbe.stdDataSet()));
			} catch (Throwable t) {
				System.out.println("[F] FAIL " + v[0] + ": " + t.getClass().getName()
						+ ": " + t.getMessage());
				bad++;
			}
		}
		return bad;
	}

	/** Phase G -- pagination over the calculated grid. */
	private static int phaseG() {
		System.out.println("== Phase G: pagination via PageBuilder");
		try {
			IReport rpt = CalcProbe.calc(tallReport(40), CalcProbe.tallDataSet("ds1", 40));
			CalcProbe.dumpPages("G", rpt, 595, 842, 1);
			return 0;
		} catch (Throwable t) {
			return CalcProbe.fail("G", t);
		}
	}

	/**
	 * Phase H -- group header rows. NOTE: the row type is copied onto every
	 * row the extension produces, so marking the anchor row turns the whole
	 * block -90; that is not a valid group report and the pager NPEs.
	 */
	private static int phaseH() {
		System.out.println("== Phase H: group header rows in pagination");
		try {
			ReportDefine2 rd = tallReport(40);
			rd.getRowCell(2).setRowType(IRowCell.TYPE_GROUP_HEADER);
			rd.getRowCell(2).setGroupHeaderLevel((byte) 1);
			ExtCellSet ecs = (ExtCellSet) CalcProbe.calc(rd,
					CalcProbe.tallDataSet("ds1", 40));
			StringBuilder types = new StringBuilder();
			for (int r = 1; r <= Math.min(ecs.getRowCount(), 8); r++) {
				types.append(r).append(":").append(ecs.getRowCell(r).getRowType())
						.append(" ");
			}
			System.out.println("[H] calc grid " + ecs.getRowCount() + " rows x "
					+ ecs.getColCount() + " cols, hasGroupHeader=" + ecs.hasGroupHeader()
					+ ", first row types: " + types);
			CalcProbe.dumpPages("H", ecs, 595, 842, 1);
			return 0;
		} catch (Throwable t) {
			return CalcProbe.fail("H", t);
		}
	}

	// =================================================================
	// Report builders
	// =================================================================

	/**
	 * Header row + one template data row extended down by the dataset:
	 * A=region, B=qty, C=B2*2, D=sum($B2:B2).
	 */
	private static ReportDefine2 listReport(int cols) {
		ReportDefine2 rd = new ReportDefine2(6, cols);
		INormalCell c = rd.getCell(1, 1);
		c.setValue("Region");
		c = rd.getCell(1, 2);
		c.setValue("Qty");
		c = rd.getCell(1, 3);
		c.setValue("Doubled");
		c = rd.getCell(1, 4);
		c.setValue("Running");
		CalcProbe.setExp(rd.getCell(2, 1), "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
		CalcProbe.setExp(rd.getCell(2, 3), "B2*2");
		CalcProbe.setExp(rd.getCell(2, 4), "sum($B2:B2)");
		CalcProbe.attachFileDataSet(rd, "ds1", "data.csv");
		return rd;
	}

	/**
	 * A tall report: 1 header row + 1 template data row extended down by the
	 * dataset, uniform row heights so the total height exceeds one A4 page.
	 * Used for the pagination baseline.
	 */
	private static ReportDefine2 tallReport(int dataRows) {
		ReportDefine2 rd = new ReportDefine2(3, 3);
		rd.getCell(1, 1).setValue("Region");
		rd.getCell(1, 2).setValue("Qty");
		rd.getCell(1, 3).setValue("Doubled");
		CalcProbe.setExp(rd.getCell(2, 1), "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
		CalcProbe.setExp(rd.getCell(2, 3), "B2*2");
		for (int r = 1; r <= 3; r++) {
			rd.getRowCell(r).setRowHeight(20f);
		}
		CalcProbe.attachFileDataSet(rd, "ds1", "data.csv");
		return rd;
	}

	private static String firstLine(String s) {
		int i = s.indexOf('\n');
		return i < 0 ? s : s.substring(0, i);
	}
}