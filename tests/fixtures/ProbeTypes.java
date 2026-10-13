/*
 * ProbeTypes.java -- empirical probe: which value types can the product
 * carry into the calculated grid, and what typeName comes out?
 *
 * Why: CalcJson.typeName has 9 branches but the 7 shipped engine_* fixtures
 * only ever exercise 3 (string/int/list) -- so M2-14 (type fidelity:
 * Integer(100) must not come back as Double(100.0)) has no product backing.
 * This probe decides, empirically, whether a LITERAL route and/or an
 * EXPRESSION route can produce the remaining types. Guessing is not allowed.
 *
 * Record-layer background (ByteArrayOutputRecord.writeObject:202-266 and
 * ByteArrayInputRecord.readObject:265-343): every value type has its own
 * tag byte and the reader reconstructs the exact boxed type --
 *   1 String, 2 BigDecimal, 4 Integer, 5 Long, 6 Boolean,
 *   9 Double, 10 Float, 11 Byte, 12 Short
 * so a literal *should* survive .rpx write->read without demotion. This
 * probe checks that end to end through ReportUtils.write/read + Engine.calc.
 *
 * Run:  java -cp "lib/*;out" rpx.fixtures.ProbeTypes out
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.util.ReportUtils;

import java.math.BigDecimal;

public class ProbeTypes {

	public static void main(String[] args) throws Exception {
		String dir = args.length > 0 ? args[0] : ".";
		System.out.println("==== PART 1: literal cells (in-memory vs .rpx read-back) ====");
		literalRoute(dir);
		System.out.println();
		System.out.println("==== PART 2: expressions (no dataset) ====");
		expressionRoute(dir);
		System.out.println();
		System.out.println("==== PART 3: dataset aggregates ====");
		aggregateRoute(dir);
		System.out.println();
		System.out.println("==== PART 4: typeName coverage of the shipped fixtures ====");
		System.out.println("[note] see _shape_audit.py for the full histogram");
	}

	// =================================================================
	// PART 1 -- literal cells, each Java boxed type planted verbatim
	// =================================================================

	/** Labels + one typed literal per column; no expressions, no datasets. */
	private static ReportDefine2 literalReport() {
		Object[] vals = {
				Integer.valueOf(100),              // int
				Long.valueOf(1234567890123L),      // long
				Double.valueOf(100.0),             // double, integral value
				Double.valueOf(92.5),              // double, fractional
				Float.valueOf(1.5f),               // float
				new BigDecimal("100.00"),          // decimal, scale 2
				new BigDecimal("3.14159"),         // decimal, scale 5
				Boolean.TRUE,                      // boolean
				Boolean.FALSE,                     // boolean
				"txt",                             // string
				Short.valueOf((short) 7),          // short
				Byte.valueOf((byte) 9),            // byte
		};
		ReportDefine2 rd = new ReportDefine2(2, vals.length);
		for (int i = 0; i < vals.length; i++) {
			rd.getCell(1, i + 1).setValue("L" + (i + 1) + ":" + vals[i].getClass().getSimpleName());
			rd.getCell(2, i + 1).setValue(vals[i]);
		}
		return rd;
	}

	private static void literalRoute(String dir) {
		ReportDefine2 rd = literalReport();
		System.out.println("[lit] planted " + rd.getColCount() + " typed literals");

		// (a) in-memory model -> calc
		IReport a = null;
		try {
			a = CalcProbe.calc(rd, null);
			dumpRow("[lit-inmem]", a, 2);
		} catch (Throwable t) {
			CalcProbe.fail("[lit-inmem]", t);
		}

		// (b) .rpx write -> read -> calc  (the round-trip that matters)
		String path = dir + "/_probe_types_lit.rpx";
		try {
			ReportUtils.write(path, rd);
			ReportDefine2 rd2 = (ReportDefine2) ReportUtils.read(path);
			IReport b = CalcProbe.calc(rd2, null);
			dumpRow("[lit-readback]", b, 2);
			compareRow("[lit] ROUND-TRIP", a, b, 2);
		} catch (Throwable t) {
			CalcProbe.fail("[lit-readback]", t);
		}
	}

	// =================================================================
	// PART 2 -- one report per expression (isolated: a bad exp must not
	// kill the whole probe)
	// =================================================================

	private static void expressionRoute(String dir) {
		// operands live in row 2: A2=100(B int), B2=200(int),
		// C2=2.0(double), D2=3(decimal)
		String[] exps = {
				"A2/3",              // non-exact division
				"A2*1.0",            // int * double
				"A2/2",              // exact division
				"A2+B2",             // int + int
				"A2*C2",             // int * double
				"A2-B2",             // int - int
				"A2/C2",             // int / double
				"A2*100000000000",   // overflow int on purpose
				"A2>B2",             // comparison -> boolean?
				"A2==B2",            // equality -> boolean?
				"D2*2",              // decimal * int
				"A2/D2",             // int / decimal
		};
		for (String exp : exps) {
			ReportDefine2 rd = expReport(exp);
			String tag = "[exp " + exp + "]";
			try {
				IReport rpt = CalcProbe.calc(rd, null);
				dumpCell(tag, rpt, 3, 1);
			} catch (Throwable t) {
				CalcProbe.fail(tag, t);
			}
		}
	}

	/** 3 rows x 4 cols: operands in row 2, the expression in (3,1). */
	private static ReportDefine2 expReport(String exp) {
		ReportDefine2 rd = new ReportDefine2(3, 4);
		rd.getCell(2, 1).setValue(Integer.valueOf(100));
		rd.getCell(2, 2).setValue(Integer.valueOf(200));
		rd.getCell(2, 3).setValue(Double.valueOf(2.0));
		rd.getCell(2, 4).setValue(new BigDecimal("3"));
		CalcProbe.setExp(rd.getCell(3, 1), exp);
		return rd;
	}

	// =================================================================
	// PART 3 -- dataset aggregates (no extension: no select() anywhere,
	// so the dataset current stays at the root group -> true aggregates)
	// =================================================================

	private static void aggregateRoute(String dir) {
		String[] exps = {
				"ds1.sum(qty)",
				"sum(ds1.select(qty))",
				"ds1.count()",
				"ds1.avg(qty)",
				"ds1.max(qty)",
				"ds1.min(qty)",
		};
		ReportDefine2 rd = new ReportDefine2(3, exps.length);
		for (int i = 0; i < exps.length; i++) {
			rd.getCell(1, i + 1).setValue(exps[i]);
			CalcProbe.setExp(rd.getCell(3, i + 1), exps[i]);
		}
		// The engine resolves the dataset file relative to the CWD, so the
		// path we attach must be exactly the path we wrote (dir is relative
		// to the CWD when the probe is launched, e.g. "out").
		String csvPath = dir + "/_probe_types_ds.csv";
		CalcProbe.attachFileDataSet(rd, "ds1", csvPath);
		try {
			CalcProbe.writeCsv(csvPath, new String[] { "region", "qty" },
					new String[][] { { "North", "100" }, { "East", "200" }, { "South", "150" } });
		} catch (Throwable t) {
			CalcProbe.fail("[agg] writeCsv", t);
			return;
		}
		try {
			IReport rpt = CalcProbe.calc(rd, null);
			dumpRow("[agg]", rpt, 3);
		} catch (Throwable t) {
			CalcProbe.fail("[agg]", t);
		}
	}

	// =================================================================
	// Dump helpers -- typeName is the authority, value() is separate
	// =================================================================

	private static void dumpRow(String tag, IReport rpt, int row) {
		if (!(rpt instanceof ExtCellSet)) {
			System.out.println(tag + " not an ExtCellSet: " + rpt);
			return;
		}
		ExtCellSet ecs = (ExtCellSet) rpt;
		for (int c = 1; c <= ecs.getColCount(); c++) {
			INormalCell cell = ecs.getCell(row, c);
			Object v = cell == null ? null : cell.getValue();
			String label = colLabel(ecs, row, c);
			System.out.println(tag + " (" + row + "," + c + ") " + label
					+ " value=" + CalcJson.value(v) + " type=" + CalcJson.typeName(v));
		}
	}

	private static void dumpCell(String tag, IReport rpt, int r, int c) {
		if (!(rpt instanceof ExtCellSet)) {
			System.out.println(tag + " not an ExtCellSet: " + rpt);
			return;
		}
		ExtCellSet ecs = (ExtCellSet) rpt;
		INormalCell cell = ecs.getCell(r, c);
		Object v = cell == null ? null : cell.getValue();
		System.out.println(tag + " (" + r + "," + c + ") value=" + CalcJson.value(v)
				+ " type=" + CalcJson.typeName(v));
	}

	/** Row-1 label of a column, if the literal report planted one. */
	private static String colLabel(ExtCellSet ecs, int row, int c) {
		INormalCell h = ecs.getCell(1, c);
		Object v = h == null ? null : h.getValue();
		return v == null ? "" : v.toString() + "\t";
	}

	/** Loudly reports any type mismatch between the in-memory and read-back grids. */
	private static void compareRow(String tag, IReport a, IReport b, int row) {
		if (!(a instanceof ExtCellSet) || !(b instanceof ExtCellSet)) {
			System.out.println(tag + " SKIP (not ExtCellSet)");
			return;
		}
		ExtCellSet ea = (ExtCellSet) a;
		ExtCellSet eb = (ExtCellSet) b;
		boolean ok = true;
		for (int c = 1; c <= ea.getColCount(); c++) {
			String ta = CalcJson.typeName(ea.getCell(row, c).getValue());
			String tb = CalcJson.typeName(eb.getCell(row, c).getValue());
			String va = CalcJson.value(ea.getCell(row, c).getValue());
			String vb = CalcJson.value(eb.getCell(row, c).getValue());
			if (!ta.equals(tb) || !va.equals(vb)) {
				ok = false;
				System.out.println(tag + " MISMATCH c" + c + ": mem type=" + ta
						+ " value=" + va + " | read type=" + tb + " value=" + vb);
			}
		}
		System.out.println(tag + (ok ? " OK: every literal type survived .rpx" : " FAIL: type demotion"));
	}
}
