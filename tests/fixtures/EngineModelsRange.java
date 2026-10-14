/*
 * EngineModelsRange.java -- engine_avgrange: M2-12 avg/max/min(A:B) region
 * forms, pinned by the product.
 *
 * WHY a dedicated fixture: §14.3 declared `avg/max/min(A:B)` an explicit
 * UnsupportedExpr gap. A headless probe (ProbeAvgRange.java) proved the
 * product does NOT aggregate the range -- it runs the region handler's
 * SINGLE-CELL else branch:
 *
 *   max/min/sum(A:B) -> evaluate operand1 (sub0)   [cellSet = sub1]
 *   avg(A:B)         -> evaluate operand2 (sub1)   [cellSet = sub0]  + Double
 *
 * (Sum.java cellSet=sub1; Average.java:145-149 cellSet=sub0 -- operands are
 * SWAPPED for avg. The List-iterate branch needs cellSet to evaluate to a
 * List, which plain cell-range syntax never produces.)
 *
 * The literals are chosen so each function's branch is UNAMBIGUOUS:
 *   B2=100, B3=200, B4=120
 *     sum(B2:B4): operand1 -> 100 ; range-sum -> 420     => 100 pins else-branch
 *     max(B2:B4): operand1 -> 100 ; range-max -> 200     => 100 pins else-branch
 *     avg(B2:B4): operand2 -> 120.0(Double) ; mean -> 140.0 ; operand1 -> 100.0
 *                                                   => pins avg evaluates operand2
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;

final class EngineModelsRange {

	private EngineModelsRange() {
	}

	static ReportDefine2 avgRangeFixture() {
		ReportDefine2 rd = new ReportDefine2(5, 6);
		rd.getCell(1, 2).setValue("B");
		rd.getCell(1, 3).setValue("sumRange");
		rd.getCell(1, 4).setValue("avgRange");
		rd.getCell(1, 5).setValue("maxRange");
		rd.getCell(1, 6).setValue("minRange");
		// distinct literals: mean(140) != operand2(120) != operand1(100)
		rd.getCell(2, 2).setValue(Integer.valueOf(100)); // B2
		rd.getCell(3, 2).setValue(Integer.valueOf(200)); // B3
		rd.getCell(4, 2).setValue(Integer.valueOf(120)); // B4
		CalcProbe.setExp(rd.getCell(2, 3), "sum(B2:B4)");
		CalcProbe.setExp(rd.getCell(2, 4), "avg(B2:B4)");
		CalcProbe.setExp(rd.getCell(2, 5), "max(B2:B4)");
		CalcProbe.setExp(rd.getCell(2, 6), "min(B2:B4)");
		return rd;
	}

	/** dataset rows for the shipped CSV (unused by the cell-range formulas). */
	static String[][] rows() {
		return new String[][] { { "North", "100" }, { "East", "200" },
				{ "South", "150" } };
	}
}
