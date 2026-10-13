/*
 * ProbeExtend.java -- M2 disambiguation: extendMode, pagination, group headers.
 *
 *   D2   extendMode 16 / 18 / 17 / 19 against a 5-row dataset. Answers
 *         "does the default mode 16 extend when a dataset is present?"
 *   D2b  EXTEND_HORIZONTAL from the first vs the last column, to see whether
 *         the column growth is clipped at the matrix edge.
 *   D3   pagination baseline through ReportUtils2.getPagerInfo + PageBuilder.
 *   D4   group header rows: -90 on the extending anchor row (invalid) vs a
 *         dedicated -90 group row above a normal data row (valid).
 *
 * VERDICTS (see RUNLOG §10.2-§10.4):
 *   - mode 16 DOES extend; the product rewrites 16 -> 18/19/17 at runtime from
 *     the leftHead/topHead topology (ExtExtendCell.prepareCalculate:314-328).
 *   - row count = template rows + (dataset rows - 1).
 *   - 40 rows at 20mm on A4 with unit=1 (mm) -> 3 pages of 14 rows;
 *     rowNumPerPage=20 is only a hint from propMap slot 33.
 *   - group headers paginate only with a dedicated -90 row; the header then
 *     repeats at the top of every following page.
 *
 * Sibling: ProbeDisambig.java covers the sum()/range questions.
 *
 * Run:
 *   "F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
 *       rpx.fixtures.ProbeExtend . > probe_extend_out.txt 2> probe_extend_err.txt
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IRowCell;

import java.io.File;

public class ProbeExtend {

	static String DIR = ".";

	public static void main(String[] args) throws Exception {
		DIR = args.length > 0 ? args[0] : ".";
		int bad = 0;
		bad += d2();
		bad += d2b();
		bad += d3();
		bad += d4();
		System.out.println("----");
		System.out.println(bad == 0 ? "EXTEND OK" : "EXTEND: " + bad + " blocked");
		System.exit(bad == 0 ? 0 : 2);
	}

	// =================================================================
	// D2 -- extendMode 16 (default) vs 18 vs 17 with a 5-row dataset.
	//   Report = 1 header row + 1 template data row. If the matrix grows to
	//   6 rows the cell extended; if it stays at 2 it did not.
	//   D2b repeats mode 19 from the LAST column: horizontal growth may be
	//   clipped at the matrix edge.
	// =================================================================
	private static int d2() {
		System.out.println("== D2: extendMode 16 / 18 / 17 against a 5-row dataset");
		int bad = 0;
		int[] modes = { 16, 18, 17, 19 };
		String[] names = { "EXTEND_DEFAULT(16)", "EXTEND_VERTICAL(18)",
				"EXTEND_NONE(17)", "EXTEND_HORIZONTAL(19)" };
		for (int i = 0; i < modes.length; i++) {
			try {
				ReportDefine2 rd = new ReportDefine2(2, 3);
				setLit(rd, 1, 1, "region");
				setLit(rd, 1, 2, "qty");
				INormalCell d = rd.getCell(2, 1);
				d.setExtendMode((byte) modes[i]);
				CalcProbe.setExp(d, "ds1.select(region)");
				CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
				CalcProbe.attachFileDataSet(rd, "ds1", "disambig_d2.csv");
				CalcProbe.writeCsv(DIR + File.separator + "disambig_d2.csv",
						new String[] { "region", "qty" }, new String[][] {
								{ "R1", "10" }, { "R2", "20" }, { "R3", "30" },
								{ "R4", "40" }, { "R5", "50" } });
				ExtCellSet ecs = (ExtCellSet) CalcProbe.calc(rd, null);
				ExtCell src = ecs.getSource(2, 1);
				System.out.println("[D2] " + names[i] + " -> calc grid "
						+ ecs.getRowCount() + " rows x " + ecs.getColCount() + " cols"
						+ " anchor subCells=" + src.extCellSize()
						+ " anchor extend=" + src.getExtensible()
						+ " effective extend=" + ecs.getCell(2, 1).getExtendMode());
				for (int r = 1; r <= ecs.getRowCount(); r++) {
					INormalCell c = ecs.getCell(r, 1);
					System.out.println("[D2]   row " + r + " col1="
							+ CalcProbe.repr(c == null ? null : c.getValue()));
				}
				new File(DIR + File.separator + "disambig_d2.csv").delete();
			} catch (Throwable t) {
				bad += CalcProbe.fail("D2:" + modes[i], t);
			}
		}
		return bad;
	}

	// =================================================================
	// D2b -- EXTEND_HORIZONTAL from the first vs the last column.
	//   D2 put mode 19 in column 1 of a 3-column report and the matrix grew
	//   to 7 columns; engine_ext puts mode 19 in the last column of a
	//   4-column report and it stayed 4. This pins whether the growth is
	//   clipped at the right edge.
	// =================================================================
	private static int d2b() {
		System.out.println("== D2b: EXTEND_HORIZONTAL(19) from column 1 vs last column");
		int[] cols = { 3, 4 };
		int[] at = { 1, 4 };
		int bad = 0;
		for (int i = 0; i < cols.length; i++) {
			try {
				ReportDefine2 rd = new ReportDefine2(2, cols[i]);
				INormalCell c = rd.getCell(2, at[i]);
				c.setExtendMode(INormalCell.EXTEND_HORIZONTAL);
				CalcProbe.setExp(c, "ds1.select(region)");
				CalcProbe.attachFileDataSet(rd, "ds1", "disambig_d2b.csv");
				CalcProbe.writeCsv(DIR + File.separator + "disambig_d2b.csv",
						new String[] { "region", "qty" }, new String[][] {
								{ "R1", "10" }, { "R2", "20" }, { "R3", "30" },
								{ "R4", "40" }, { "R5", "50" } });
				ExtCellSet ecs = (ExtCellSet) CalcProbe.calc(rd, null);
				StringBuilder sb = new StringBuilder();
				for (int cc = 1; cc <= ecs.getColCount(); cc++) {
					INormalCell g = ecs.getCell(2, cc);
					sb.append(cc).append("=")
							.append(CalcProbe.repr(g == null ? null : g.getValue()))
							.append(" ");
				}
				System.out.println("[D2b] template cols=" + cols[i] + " mode19 at col "
						+ at[i] + " -> calc grid " + ecs.getRowCount() + "x"
						+ ecs.getColCount() + " | row2: " + sb);
				new File(DIR + File.separator + "disambig_d2b.csv").delete();
			} catch (Throwable t) {
				bad += CalcProbe.fail("D2b:col" + at[i], t);
			}
		}
		return bad;
	}

	// =================================================================
	// D3 -- pagination baseline with an explicit unit, so the rows-per-page
	//   figure is derivable: unit=1 means millimetres, so a 20mm row on a
	//   297mm A4 page fits 14 rows, not 20.
	// =================================================================
	private static int d3() {
		System.out.println("== D3: pagination baseline, explicit unit");
		try {
			ReportDefine2 rd = new ReportDefine2(3, 3);
			setLit(rd, 1, 1, "region");
			setLit(rd, 1, 2, "qty");
			setLit(rd, 1, 3, "doubled");
			CalcProbe.setExp(rd.getCell(2, 1), "ds1.select(region)");
			CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
			CalcProbe.setExp(rd.getCell(2, 3), "B2*2");
			for (int r = 1; r <= 3; r++) {
				rd.getRowCell(r).setRowHeight(20f);
			}
			CalcProbe.attachFileDataSet(rd, "ds1", "disambig_d3.csv");
			CalcProbe.writeCsv(DIR + File.separator + "disambig_d3.csv",
					new String[] { "region", "qty" }, d3Rows(40));
			ExtCellSet ecs = (ExtCellSet) CalcProbe.calc(rd, null);
			System.out.println("[D3] unit=" + rd.getUnit() + " (1=mm) rowHeight=20"
					+ " calc grid " + ecs.getRowCount() + " rows");
			CalcProbe.dumpPages("D3", ecs, 595, 842, 1);
			new File(DIR + File.separator + "disambig_d3.csv").delete();
			return 0;
		} catch (Throwable t) {
			return CalcProbe.fail("D3", t);
		}
	}

	// =================================================================
	// D4 -- group header rows. Variant 1 marks the extending anchor row
	//   (the naive approach); variant 2 puts the -90 group row above a
	//   separate normal data row, which is the shape the pager expects.
	// =================================================================
	private static int d4() {
		System.out.println("== D4: group header row variants");
		int bad = 0;
		bad += d4v1();
		bad += d4v2();
		return bad;
	}

	/** Variant 1: -90 on the extending anchor row (what ProbeCalc Phase H did). */
	private static int d4v1() {
		try {
			ReportDefine2 rd = pagerBody();
			rd.getRowCell(2).setRowType(IRowCell.TYPE_GROUP_HEADER);
			rd.getRowCell(2).setGroupHeaderLevel((byte) 1);
			ExtCellSet ecs = (ExtCellSet) CalcProbe.calc(rd, d3DataSet(40));
			StringBuilder types = new StringBuilder();
			for (int r = 1; r <= Math.min(ecs.getRowCount(), 6); r++) {
				types.append(r).append(":").append(ecs.getRowCell(r).getRowType())
						.append(" ");
			}
			System.out.println("[D4v1] -90 on anchor row -> " + ecs.getRowCount()
					+ " rows, hasGroupHeader=" + ecs.hasGroupHeader()
					+ ", row types: " + types);
			CalcProbe.dumpPages("D4v1", ecs, 595, 842, 1);
			return 0;
		} catch (Throwable t) {
			System.out.println("[D4v1] FAIL (expected): " + t.getClass().getName());
			return 0; // recorded, not a probe failure
		}
	}

	/** Variant 2: a dedicated -90 group row above a normal data row. */
	private static int d4v2() {
		try {
			ReportDefine2 rd = new ReportDefine2(4, 3);
			setLit(rd, 1, 1, "region");
			setLit(rd, 1, 2, "qty");
			setLit(rd, 1, 3, "doubled");
			// row 2 = group header row, row 3 = normal data row
			setLit(rd, 2, 1, "group: R*");
			rd.getRowCell(2).setRowType(IRowCell.TYPE_GROUP_HEADER);
			rd.getRowCell(2).setGroupHeaderLevel((byte) 1);
			CalcProbe.setExp(rd.getCell(3, 1), "ds1.select(region)");
			CalcProbe.setExp(rd.getCell(3, 2), "ds1.select(qty)");
			CalcProbe.setExp(rd.getCell(3, 3), "B3*2");
			for (int r = 1; r <= 4; r++) {
				rd.getRowCell(r).setRowHeight(20f);
			}
			CalcProbe.attachFileDataSet(rd, "ds1", "disambig_d4.csv");
			CalcProbe.writeCsv(DIR + File.separator + "disambig_d4.csv",
					new String[] { "region", "qty" }, d3Rows(40));
			ExtCellSet ecs = (ExtCellSet) CalcProbe.calc(rd, null);
			StringBuilder types = new StringBuilder();
			for (int r = 1; r <= Math.min(ecs.getRowCount(), 6); r++) {
				types.append(r).append(":").append(ecs.getRowCell(r).getRowType())
						.append(" ");
			}
			System.out.println("[D4v2] dedicated -90 group row -> " + ecs.getRowCount()
					+ " rows, hasGroupHeader=" + ecs.hasGroupHeader()
					+ ", row types: " + types);
			CalcProbe.dumpPages("D4v2", ecs, 595, 842, 1);
			new File(DIR + File.separator + "disambig_d4.csv").delete();
			return 0;
		} catch (Throwable t) {
			return CalcProbe.fail("D4v2", t);
		}
	}

	private static ReportDefine2 pagerBody() {
		ReportDefine2 rd = new ReportDefine2(3, 3);
		setLit(rd, 1, 1, "region");
		setLit(rd, 1, 2, "qty");
		setLit(rd, 1, 3, "doubled");
		CalcProbe.setExp(rd.getCell(2, 1), "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
		CalcProbe.setExp(rd.getCell(2, 3), "B2*2");
		for (int r = 1; r <= 3; r++) {
			rd.getRowCell(r).setRowHeight(20f);
		}
		CalcProbe.attachFileDataSet(rd, "ds1", "disambig_d4.csv");
		return rd;
	}

	private static com.raqsoft.reportl.dataset.DataSet d3DataSet(int n) {
		return CalcProbe.tallDataSet("ds1", n);
	}

	private static String[][] d3Rows(int n) {
		String[][] rows = new String[n][2];
		for (int i = 0; i < n; i++) {
			rows[i][0] = "R" + (i + 1);
			rows[i][1] = Integer.toString((i + 1) * 10);
		}
		return rows;
	}

	private static void setLit(ReportDefine2 rd, int r, int c, Object v) {
		rd.getCell(r, c).setValue(v);
	}
}
