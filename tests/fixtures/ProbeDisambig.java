/*
 * ProbeDisambig.java -- M2 disambiguation: does sum(range) aggregate?
 *
 * probe_out.txt Phase F showed a D column of 100/200/150, which reads
 * identically under two explanations: (a) the function ran, or (b) it failed
 * and the cell fell back to displaying its own row's B value. These
 * experiments use controlled inputs -- literal cells, no dataset, so no
 * extension can be confused with evaluation.
 *
 *   D1a  sum(B2:B4) over literals  -> 450 if it aggregates
 *   D1b  the same over an extended dataset grid, three shifted ranges
 *   D1c  every sum() form we could get the product to parse
 *   D1d  list-producing forms, the only ones that really aggregate
 *
 * VERDICT: sum(B2:B4) does NOT aggregate. Sum.calculate branches on
 * param.isLeaf(); the ':' makes the parameter compound, so it takes the
 * cellSet branch and evaluates operand 1 once with operand 2 as the current
 * cell (Sum.java:104-106 and 174-190). Aggregation only happens when the
 * operand evaluates to a List (Sum.java:112-121).
 *
 * Sibling: ProbeExtend.java covers extendMode, pagination and group headers.
 *
 * Run:
 *   "F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
 *       rpx.fixtures.ProbeDisambig . > probe_disambig_out.txt 2> probe_disambig_err.txt
 */
package rpx.fixtures;


import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.usermodel.IRowCell;

import java.io.File;

public class ProbeDisambig {

	public static void main(String[] args) throws Exception {
		DIR = args.length > 0 ? args[0] : ".";
		int bad = 0;
		bad += d1a();
		bad += d1b();
		bad += d1c();
		bad += d1d();
		System.out.println("----");
		System.out.println(bad == 0 ? "DISAMBIG OK" : "DISAMBIG: " + bad + " blocked");
		System.exit(bad == 0 ? 0 : 2);
	}

	// =================================================================
	// D1a -- sum(range) with NO dataset: no extension, no ambiguity.
	//   B2=100 B3=200 B4=150 are literals, so a real range sum must be 450.
	//   Any other number means the function did not run.
	// =================================================================
	private static int d1a() {
		System.out.println("== D1a: sum(range) over literal cells, no dataset");
		try {
			ReportDefine2 rd = new ReportDefine2(6, 4);
			setLit(rd, 1, 1, "label");
			setLit(rd, 1, 2, "qty");
			setLit(rd, 1, 3, "sumAll");
			setLit(rd, 1, 4, "sumMid");
			setLit(rd, 2, 2, 100);
			setLit(rd, 3, 2, 200);
			setLit(rd, 4, 2, 150);
			CalcProbe.setExp(rd.getCell(2, 3), "sum(B2:B4)");
			CalcProbe.setExp(rd.getCell(2, 4), "sum(B3:B4)");
			System.out.println("[D1a] D2=sum(B2:B4)  E2=sum(B3:B4)");
			CalcProbe.dump("D1a", CalcProbe.calc(rd, null), true);
			return 0;
		} catch (Throwable t) {
			return CalcProbe.fail("D1a", t);
		}
	}

	// =================================================================
	// D1b -- the same formulas over an extended dataset grid, plus two
	//   shifted ranges, to see whether each row evaluates its own range.
	// =================================================================
	private static int d1b() {
		System.out.println("== D1b: sum(range) over the extended dataset grid");
		try {
			ReportDefine2 rd = new ReportDefine2(6, 5);
			setLit(rd, 1, 1, "region");
			setLit(rd, 1, 2, "qty");
			setLit(rd, 1, 3, "sumB2B4");
			setLit(rd, 1, 4, "sumB3B5");
			setLit(rd, 1, 5, "sumB4B6");
			CalcProbe.setExp(rd.getCell(2, 1), "ds1.select(region)");
			CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
			CalcProbe.setExp(rd.getCell(2, 3), "sum(B2:B4)");
			CalcProbe.setExp(rd.getCell(2, 4), "sum(B3:B5)");
			CalcProbe.setExp(rd.getCell(2, 5), "sum(B4:B6)");
			CalcProbe.attachFileDataSet(rd, "ds1", "disambig_data.csv");
			CalcProbe.writeCsv(DIR + File.separator + "disambig_data.csv",
					new String[] { "region", "qty" }, new String[][] {
							{ "North", "100" }, { "East", "200" }, { "South", "150" } });
			CalcProbe.dump("D1b", CalcProbe.calc(rd, null), true);
			new File(DIR + File.separator + "disambig_data.csv").delete();
			return 0;
		} catch (Throwable t) {
			return CalcProbe.fail("D1b", t);
		}
	}

	// =================================================================
	// D1c -- is ANY sum form a true aggregation?
	//   Sum.calculate branches on param.isLeaf(): a bare sum(B2:B4) is NOT
	//   a leaf (the ':' makes it compound), so it takes the cellSet branch
	//   and evaluates operand 1 once with operand 2 as the current cell.
	//   Only the two-operand cellSet form can iterate a range, and only if
	//   the first operand reads the CURRENT cell.
	// =================================================================
	private static int d1c() {
		System.out.println("== D1c: sum() forms over literal cells B2=100 B3=200 B4=150");
		String[][] forms = {
				{ "sum(B2:B4)", "one operand, range" },
				{ "sum(B2)", "one operand, single cell" },
				{ "sum(B2:B4, value())", "range then current-cell value" },
				{ "sum(value(), B2:B4)", "current-cell value then range" },
				{ "sum(B2, B2:B4)", "expr B2 over range" },
				{ "sum(B2:B4, B2)", "range then expr B2" },
		};
		int bad = 0;
		for (String[] f : forms) {
			try {
				ReportDefine2 rd = new ReportDefine2(6, 3);
				setLit(rd, 1, 2, "qty");
				setLit(rd, 2, 2, 100);
				setLit(rd, 3, 2, 200);
				setLit(rd, 4, 2, 150);
				CalcProbe.setExp(rd.getCell(2, 3), f[0]);
				IReport rpt = CalcProbe.calc(rd, null);
				INormalCell c = ((ExtCellSet) rpt).getCell(2, 3);
				System.out.println("[D1c] " + f[0] + "  (" + f[1] + ") -> "
						+ CalcProbe.repr(c == null ? null : c.getValue()));
			} catch (Throwable t) {
				System.out.println("[D1c] " + f[0] + "  (" + f[1] + ") -> FAIL "
						+ t.getClass().getSimpleName() + ": " + t.getMessage());
				bad++;
			}
		}
		System.out.println("[D1c] expected true aggregation would be 450");
		return 0;
	}

	// =================================================================
	// D1d -- list-producing operands: Sum.calculate only aggregates when
	//   the operand evaluates to a List (leaf branch, line 112-121).
	//   ds1.sum(qty) is the dataset-level aggregate; sum(ds1.select(qty))
	//   feeds a List into the leaf branch.
	// =================================================================
	private static int d1d() {
		System.out.println("== D1d: list-producing sum forms, dataset North=100 East=200 South=150");
		String[][] forms = {
				{ "ds1.sum(qty)", "dataset aggregate (DS function)" },
				{ "sum(ds1.select(qty))", "sum over a List operand" },
				{ "ds1.avg(qty)", "dataset average" },
				{ "ds1.max(qty)", "dataset max" },
				{ "ds1.count()", "dataset row count" },
		};
		int bad = 0;
		for (String[] f : forms) {
			try {
				ReportDefine2 rd = new ReportDefine2(6, 3);
				setLit(rd, 1, 2, "qty");
				setLit(rd, 2, 2, 100);
				setLit(rd, 3, 2, 200);
				setLit(rd, 4, 2, 150);
				CalcProbe.setExp(rd.getCell(2, 3), f[0]);
				CalcProbe.attachFileDataSet(rd, "ds1", "disambig_d1d.csv");
				CalcProbe.writeCsv(DIR + File.separator + "disambig_d1d.csv",
						new String[] { "region", "qty" }, new String[][] {
								{ "North", "100" }, { "East", "200" },
								{ "South", "150" } });
				ExtCellSet ecs = (ExtCellSet) CalcProbe.calc(rd, null);
				INormalCell c = ecs.getCell(2, 3);
				System.out.println("[D1d] " + f[0] + "  (" + f[1] + ") -> "
						+ CalcProbe.repr(c == null ? null : c.getValue())
						+ "  gridRows=" + ecs.getRowCount());
				new File(DIR + File.separator + "disambig_d1d.csv").delete();
			} catch (Throwable t) {
				System.out.println("[D1d] " + f[0] + " -> FAIL "
						+ t.getClass().getSimpleName() + ": " + t.getMessage());
				bad++;
			}
		}
		return bad;
	}

	private static void setLit(ReportDefine2 rd, int r, int c, Object v) {
		rd.getCell(r, c).setValue(v);
	}

	static String DIR = ".";
}
