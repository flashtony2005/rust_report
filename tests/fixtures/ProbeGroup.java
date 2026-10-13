/*
 * ProbeGroup.java -- settles the open spec item "分组头动态生成" by measurement.
 *
 * M2 spec §8 flags this as unverified. Two independent mechanisms are involved and
 * this probe keeps them apart:
 *
 *   (A) rowType == -90 (IRowCell.TYPE_GROUP_HEADER, IRowCell.java:21) is a STATIC
 *       template property. ExtCellSet's constructor latches hasGroupHeader()
 *       (_$12 |= 2) while building rows at ExtCellSet.java:153; SimplePager re-reads
 *       rowType == -90 to repeat the header on each page.
 *
 *   (B) ds.group(...) is a DYNAMIC grouping expression (registered at
 *       FunctionLib.java:114). DSGroup.isExtended() returns true
 *       (DSGroup.java:23-25) -- that is precisely what makes such a cell an
 *       ExtExtendCell, i.e. an extension SOURCE.
 *
 * THE QUESTION: does the engine synthesise new -90 rows per group key, or must the
 * author declare -90 in the template while group() only drives row COUNT?
 *
 * The decisive evidence is the rowType mapping: TEMPLATE rowTypes vs CALCULATED
 * rowTypes on the same report. If every calculated row inherits the rowType of the
 * template row it came from, the engine is copying, not authoring. If a -90 appears
 * at a row that had none in the template, the engine is authoring.
 *
 * Each scenario also records the grid height, because DSGroup.calculate has a
 * counter-intuitive branch: a single distinct group key returns list.get(0), a
 * SCALAR (DSGroup.java:139-141), so no extension happens at all -- only >= 2 keys
 * produce a List.
 *
 * Run (cwd must be tests/fixtures):
 *   "F:/project/report/common/jre/bin/java.exe" -Dfile.encoding=UTF-8 \
 *     -cp "F:/project/report/reportlite/lib/*;out" rpx.fixtures.ProbeGroup
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.dataset.DataSet;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.usermodel.IRowCell;

public class ProbeGroup {

	/** Header row plus two ordinary extending data cells. */
	private static void base(ReportDefine2 rd) {
		rd.getCell(1, 1).setValue("region");
		rd.getCell(1, 2).setValue("qty");
		rd.getCell(1, 3).setValue("grp");
		CalcProbe.setExp(rd.getCell(2, 1), "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
	}

	/** Marks template row `r` as a group header at level 1. */
	private static void markGroupHeader(ReportDefine2 rd, int r) {
		rd.getRowCell(r).setRowType(IRowCell.TYPE_GROUP_HEADER);
		rd.getRowCell(r).setGroupHeaderLevel((byte) 1);
	}

	private static int rowTypeOf(ReportDefine2 rd, int r) {
		IRowCell rc = rd.getRowCell(r);
		return rc == null ? 999 : rc.getRowType();
	}

	/** Prints template rowTypes -- the baseline the engine's behaviour is judged against. */
	private static void dumpTemplate(String tag, ReportDefine2 rd) {
		StringBuilder sb = new StringBuilder();
		for (int r = 1; r <= 3; r++) {
			sb.append(r).append(':').append(rowTypeOf(rd, r)).append(' ');
		}
		System.out.println("  [" + tag + "] TEMPLATE rowTypes={" + sb.toString().trim() + "}");
	}

	/** Prints calculated rowTypes and how many are -90. */
	private static void dumpCalculated(String tag, IReport rpt) {
		ExtCellSet ecs = (ExtCellSet) rpt;
		int n = ecs.getRowCount();
		StringBuilder sb = new StringBuilder();
		int minus90 = 0;
		for (int r = 1; r <= n; r++) {
			IRowCell rc = ecs.getRowCell(r);
			int t = rc == null ? 999 : rc.getRowType();
			if (t == IRowCell.TYPE_GROUP_HEADER) {
				minus90++;
			}
			sb.append(r).append(':').append(t).append(' ');
		}
		System.out.println("  [" + tag + "] CALC rows=" + n + " rowTypes={" + sb.toString().trim()
				+ "} minus90=" + minus90 + " hasGroupHeader=" + ecs.hasGroupHeader());
	}

	private static void run(String tag, IReport rpt) {
		dumpCalculated(tag, rpt);
		CalcProbe.dump(tag, rpt);
	}

	private static void tryCalc(String tag, ReportDefine2 rd, DataSet ds) {
		dumpTemplate(tag, rd);
		try {
			run(tag, CalcProbe.calc(rd, ds));
		} catch (Throwable t) {
			CalcProbe.fail(tag, t);
		}
	}

	public static void main(String[] args) {
		// S1: group() on a row the author marked -90. 3 distinct keys.
		ReportDefine2 s1 = new ReportDefine2(4, 3);
		base(s1);
		markGroupHeader(s1, 2);
		CalcProbe.setExp(s1.getCell(2, 3), "ds1.group(region)");
		tryCalc("S1 minus90 + group(3 keys)", s1, CalcProbe.stdDataSet());

		// S2: control -- identical expression on an ordinary (non -90) row.
		ReportDefine2 s2 = new ReportDefine2(4, 3);
		base(s2);
		CalcProbe.setExp(s2.getCell(2, 3), "ds1.group(region)");
		tryCalc("S2 plain + group(3 keys) CONTROL", s2, CalcProbe.stdDataSet());

		// S3: static baseline -- a -90 row carrying a plain literal, no group().
		ReportDefine2 s3 = new ReportDefine2(4, 3);
		base(s3);
		markGroupHeader(s3, 2);
		s3.getCell(2, 3).setValue("static header");
		tryCalc("S3 minus90 + literal (static baseline)", s3, CalcProbe.stdDataSet());

		// S4: ONE distinct key -> DSGroup returns a scalar, so NOTHING extends.
		ReportDefine2 s4 = new ReportDefine2(4, 3);
		base(s4);
		markGroupHeader(s4, 2);
		CalcProbe.setExp(s4.getCell(2, 3), "ds1.group(region)");
		tryCalc("S4 minus90 + group(1 key)", s4,
				CalcProbe.memDataSet("ds1", new String[] { "North", "North", "North" },
						new int[] { 1, 2, 3 }));

		// S5: the case the spec calls out as an NPE risk -- -90 on an extending
		// anchor row, 40 rows, then paginate. If the engine copies -90 onto every
		// inserted row, SimplePager's header-repeat logic sees far more -90 rows
		// than the author declared.
		ReportDefine2 s5 = new ReportDefine2(4, 3);
		base(s5);
		markGroupHeader(s5, 2);
		CalcProbe.setExp(s5.getCell(2, 3), "ds1.group(region)");
		dumpTemplate("S5 minus90 + group(40 rows)", s5);
		try {
			IReport rpt = CalcProbe.calc(s5, CalcProbe.tallDataSet("ds1", 40));
			dumpCalculated("S5 40-row dataset", rpt);
			CalcProbe.dumpPages("S5 paginate", rpt, 297f, 297f, 1);
		} catch (Throwable t) {
			CalcProbe.fail("S5", t);
		}

		// S6: does a group() on a NON -90 row ever get promoted to -90 by the
		// engine? Same question as S1 but from the other direction.
		ReportDefine2 s6 = new ReportDefine2(4, 3);
		base(s6);
		CalcProbe.setExp(s6.getCell(3, 3), "ds1.group(region)");
		tryCalc("S6 group on row3 (row3 never -90)", s6, CalcProbe.stdDataSet());

		// --- The NPE control pair -------------------------------------------
		// S5 showed NPE at SimplePager.createPages:261 with 40 rows when the -90
		// mark sits ON the extending anchor row. Two controls isolate the cause:
		//   S7: same 40 rows, but -90 on a NON-anchor row (this is the shape the
		//       engine_pager fixture uses, which paginates fine into 4 pages).
		//   S8: -90 on the anchor row, but only 3 rows (no real pagination).
		// If S7 paginates and S8 does not, the trigger is specifically
		// "-90 cloned onto every inserted row", not "-90" and not "many rows".

		ReportDefine2 s7 = new ReportDefine2(5, 3);
		s7.getCell(1, 1).setValue("region");
		s7.getCell(1, 2).setValue("qty");
		s7.getCell(1, 3).setValue("hdr");
		markGroupHeader(s7, 2);                 // -90 on a row with NO expression
		s7.getCell(2, 3).setValue("literal");
		CalcProbe.setExp(s7.getCell(3, 1), "ds1.select(region)");
		CalcProbe.setExp(s7.getCell(3, 2), "ds1.select(qty)");
		for (int r = 1; r <= 5; r++) {
			s7.getRowCell(r).setRowHeight(20f);
		}
		dumpTemplate("S7 CONTROL: -90 NOT on anchor", s7);
		try {
			IReport rpt = CalcProbe.calc(s7, CalcProbe.tallDataSet("ds1", 40));
			dumpCalculated("S7 40-row dataset", rpt);
			CalcProbe.dumpPages("S7 CONTROL paginate", rpt, 297f, 297f, 1);
			System.out.println("  [S7] PAGINATION OK -- -90 off the anchor does NOT NPE");
		} catch (Throwable t) {
			System.out.println("  [S7] PAGINATION FAILED: " + t);
			CalcProbe.fail("S7", t);
		}

		// S8: -90 on the anchor row, only 3 rows.
		ReportDefine2 s8 = new ReportDefine2(4, 3);
		base(s8);
		markGroupHeader(s8, 2);
		CalcProbe.setExp(s8.getCell(2, 3), "ds1.group(region)");
		dumpTemplate("S8 CONTROL: -90 on anchor, 3 rows", s8);
		try {
			IReport rpt = CalcProbe.calc(s8, CalcProbe.stdDataSet());
			dumpCalculated("S8 3-row dataset", rpt);
			CalcProbe.dumpPages("S8 CONTROL paginate", rpt, 297f, 297f, 1);
			System.out.println("  [S8] PAGINATION OK -- few rows does NOT NPE");
		} catch (Throwable t) {
			System.out.println("  [S8] PAGINATION FAILED: " + t);
			CalcProbe.fail("S8", t);
		}
	}
}
