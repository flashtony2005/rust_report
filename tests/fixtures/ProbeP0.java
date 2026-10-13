/*
 * ProbeP0.java -- experiments for the three missing P0 ground truths.
 *
 *   S1 mergeH     merged area on the extending row (horizontal span)
 *   S2 mergeV     merged area spanning rows 2..3 on the extending anchor
 *   S3 cycleExpr  B2="C2", C2="B2"      (expression cycle)
 *   S4 selfExpr   B2="B2"               (self reference)
 *   S5 cycleLeft  B2.leftHead=C2, C2.leftHead=B2  (main-cell chain cycle)
 *   S6 same98/99/100 + mode 120  mergeSame codes against duplicate values
 *   S7 extSplit   extendMode 17/18/19 on three separate rows
 *
 * Read-only: nothing is written. Prints what the product engine actually
 * does so a fixture can then be frozen from the observed behaviour.
 */
package rpx.fixtures;

import com.raqsoft.reportl.dataset.DataSet;
import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.model.engine.ExtNormalCell;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.usermodel.PageBuilder;
import com.raqsoft.reportl.usermodel.PagerInfo;
import com.raqsoft.reportl.util.ReportUtils2;
import com.scudata.common.Area;

public class ProbeP0 {

	public static void main(String[] args) {
		s1();
		s2();
		s3();
		s4();
		s5();
		s6(98);
		s6(99);
		s6(100);
		s7();
		s8(98);
		s8(99);
	}

	// =================================================================
	// M2-04 : merge area on an extending anchor
	// =================================================================

	/** S1 -- the whole data row is one merged cell A2:C2, then extends. */
	private static void s1() {
		ReportDefine2 rd = new ReportDefine2(3, 4);
		lit(rd, 1, 1, "r");
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
		a2.setMergedArea(new Area(2, 1, 2, 3));
		run("S1 mergeH  A2 merged A2:C2, extend=18", rd, dup());
	}

	/** S2 -- the merge spans rows 2..3 while the anchor row extends. */
	private static void s2() {
		ReportDefine2 rd = new ReportDefine2(4, 3);
		lit(rd, 1, 1, "r");
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(region)");
		a2.setMergedArea(new Area(2, 1, 3, 1));
		run("S2 mergeV  A2 merged A2:A3, extend=18", rd, dup());
	}

	// =================================================================
	// M2-07 : circular references
	// =================================================================

	private static void s3() {
		ReportDefine2 rd = new ReportDefine2(3, 3);
		CalcProbe.setExp(rd.getCell(2, 2), "C2");
		CalcProbe.setExp(rd.getCell(2, 3), "B2");
		run("S3 cycleExpr  B2=C2, C2=B2", rd, dup());
	}

	private static void s4() {
		ReportDefine2 rd = new ReportDefine2(3, 3);
		CalcProbe.setExp(rd.getCell(2, 2), "B2");
		run("S4 selfExpr  B2=B2", rd, dup());
	}

	private static void s5() {
		ReportDefine2 rd = new ReportDefine2(3, 3);
		rd.getCell(2, 2).setLeftHead("C2");
		rd.getCell(2, 3).setLeftHead("B2");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(2, 3), "ds1.select(region)");
		run("S5 cycleLeft  leftHead(B2)=C2, leftHead(C2)=B2", rd, dup());
	}

	// =================================================================
	// M2-08 : mergeSame
	// =================================================================

	private static void s6(int code) {
		ReportDefine2 rd = new ReportDefine2(3, 3);
		lit(rd, 1, 1, "region");
		lit(rd, 1, 2, "qty");
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
		a2.setMergeSame((byte) code);
		run("S6 mergeSame=" + code + "  col A = A,A,B,B,C extend=18", rd, dup());
	}

	// =================================================================
	// engine_ext, split so 17/18/19 each own an envelope
	// =================================================================

	private static void s7() {
		ReportDefine2 rd = new ReportDefine2(4, 6);
		lit(rd, 1, 1, "m17");
		lit(rd, 1, 2, "m18");
		lit(rd, 1, 3, "m19");
		INormalCell c17 = rd.getCell(2, 1);
		c17.setExtendMode(INormalCell.EXTEND_NONE);
		CalcProbe.setExp(c17, "ds1.select(region)");
		INormalCell c18 = rd.getCell(3, 1);
		c18.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(c18, "ds1.select(region)");
		INormalCell c19 = rd.getCell(4, 1);
		c19.setExtendMode(INormalCell.EXTEND_HORIZONTAL);
		CalcProbe.setExp(c19, "ds1.select(region)");
		run("S7 extSplit  17@(2,1) 18@(3,1) 19@(4,1)", rd, regions5());
	}

	/**
	 * S8 -- same mergeSame report, but driven through PageBuilder.
	 * SimplePager.createPages() is the ONLY place that calls ExtCellSet
	 * .mergeSame() (SimplePager.java:133,179), so the merge never shows up in
	 * the plain Engine.calc() grid.
	 */
	private static void s8(int code) {
		ReportDefine2 rd = new ReportDefine2(3, 3);
		lit(rd, 1, 1, "region");
		lit(rd, 1, 2, "qty");
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
		a2.setMergeSame((byte) code);
		runPages("S8 mergeSame=" + code + " via PageBuilder", rd, dup());
	}

	// =================================================================
	// Plumbing
	// =================================================================

	private static String[] dupRegions = { "A", "A", "B", "B", "C" };
	private static int[] dupQtys = { 10, 20, 30, 40, 50 };

	private static DataSet dup() {
		return CalcProbe.memDataSet("ds1", dupRegions, dupQtys);
	}

	private static DataSet regions5() {
		String[] r = { "R1", "R2", "R3", "R4", "R5" };
		int[] q = { 10, 20, 30, 40, 50 };
		return CalcProbe.memDataSet("ds1", r, q);
	}

	private static void lit(ReportDefine2 rd, int r, int c, Object v) {
		rd.getCell(r, c).setValue(v);
	}

	private static void run(String label, ReportDefine2 rd, DataSet ds) {
		System.out.println("== " + label);
		try {
			IReport rpt = CalcProbe.calc(rd, ds);
			ExtCellSet ecs = (ExtCellSet) rpt;
			System.out.println("   grid " + ecs.getRowCount() + "x"
					+ ecs.getColCount());
			for (int r = 1; r <= ecs.getRowCount(); r++) {
				for (int c = 1; c <= ecs.getColCount(); c++) {
					INormalCell cell = ecs.getCell(r, c);
					if (cell == null) {
						continue;
					}
					Object v = cell.getValue();
					if (v == null) {
						continue;
					}
					ExtCell src = ecs.getSource(r, c);
					System.out.println(String.format(
							"   (%d,%d) val=%-8s rm=%s cm=%s ext=%s subs=%s",
							r, c, str(v), src == null ? "-" : "" + src.getRowMerge(),
							src == null ? "-" : "" + src.getColMerge(),
							src == null ? "-" : "" + src.getExtensible(),
							src == null ? "-" : "" + src.extCellSize()));
				}
			}
			dumpSubs(ecs);
		} catch (Throwable t) {
			System.out.println("   THROW " + t.getClass().getName() + ": "
					+ t.getMessage());
			Throwable cause = t.getCause();
			if (cause != null) {
				System.out.println("   cause " + cause.getClass().getName() + ": "
						+ cause.getMessage());
			}
		}
	}

	private static void runPages(String label, ReportDefine2 rd, DataSet ds) {
		System.out.println("== " + label);
		try {
			IReport rpt = CalcProbe.calc(rd, ds);
			PagerInfo pi = ReportUtils2.getPagerInfo(rpt, 595, 842, 1);
			PageBuilder pb = new PageBuilder(rpt, pi);
			pb.createPages();
			IReport[] all = pb.getAllPages();
			System.out.println("   pages=" + all.length);
			for (int p = 0; p < all.length; p++) {
				IReport pg = all[p];
				System.out.println("   -- page " + (p + 1) + " rows="
						+ pg.getRowCount() + " cols=" + pg.getColCount());
				for (int r = 1; r <= pg.getRowCount(); r++) {
					for (int c = 1; c <= pg.getColCount(); c++) {
						INormalCell cell = pg.getCell(r, c);
						if (cell == null) {
							continue;
						}
						Object v = cell.getValue();
						if (v == null) {
							continue;
						}
						ExtNormalCell ec = (ExtNormalCell) cell;
						System.out.println(String.format(
								"     (%d,%d) val=%-6s rm=%d cm=%d", r, c,
								str(v), ec.getRowMerge(), ec.getColMerge()));
					}
				}
			}
		} catch (Throwable t) {
			System.out.println("   THROW " + t.getClass().getName() + ": "
					+ t.getMessage());
		}
	}

	private static void dumpSubs(ExtCellSet ecs) {
		for (int r = 1; r <= ecs.getRowCount(); r++) {
			for (int c = 1; c <= ecs.getColCount(); c++) {
				ExtCell src = ecs.getSource(r, c);
				if (src == null || src.extCellSize() <= 1) {
					continue;
				}
				StringBuilder sb = new StringBuilder();
				for (int i = 0; i < src.extCellSize(); i++) {
					ExtCell sub = src.getExtCell(i);
					sb.append("[").append(sub.getRow()).append(",")
							.append(sub.getCol()).append("]=")
							.append(str(((ExtNormalCell) sub).getValue(false)))
							.append(" ");
				}
				System.out.println("   subs of (" + r + "," + c + "): " + sb);
			}
		}
	}

	private static String str(Object o) {
		if (o == null) {
			return "null";
		}
		if (o instanceof Integer || o instanceof Long || o instanceof String) {
			return String.valueOf(o);
		}
		return o.getClass().getSimpleName() + "(" + o + ")";
	}
}
