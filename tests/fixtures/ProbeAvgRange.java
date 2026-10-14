/*
 * ProbeAvgRange.java -- M2-12: what does the PRODUCT actually compute for
 * avg/max/min(A:B) region forms?
 *
 * Sum.java/Max.java/Min.java carry cellSet=sub1 (range=B), Average.java
 * carries cellSet=sub0 (range=A) -- the operand roles are SWAPPED for avg.
 * This probe feeds distinct literals B2=100,B3=200,B4=150 and dumps the
 * product result + Java type for every form, so the branch (List iterate vs
 * single-cell else) is decided by evidence, not by reading decompiled code.
 *
 *   sum(B2:B4):  else-branch -> 100 (operand1) ; List-branch -> 450
 *   max(B2:B4):  else-branch -> 100 (operand1) ; List-branch -> 200
 *   avg(B2:B4):  else-branch -> 150 (operand2) ; List-branch -> 150 (mean)
 *   min(B2:B4):  else-branch -> 100 ; List-branch -> 100
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.IReport;

public class ProbeAvgRange {

	public static void main(String[] args) {
		String[] forms = { "sum(B2:B4)", "avg(B2:B4)", "max(B2:B4)",
				"min(B2:B4)", "sum(B2,B3,B4)", "avg(B2,B3,B4)",
				"max(B2,B3,B4)", "min(B2,B3,B4)" };
		for (String f : forms) {
			probe(f);
		}
	}

	private static void probe(String formula) {
		try {
			ReportDefine2 rd = new ReportDefine2(6, 6);
			// distinct literals so each branch yields a different number
			rd.getCell(2, 2).setValue(Integer.valueOf(100)); // B2
			rd.getCell(3, 2).setValue(Integer.valueOf(200)); // B3
			rd.getCell(4, 2).setValue(Integer.valueOf(150)); // B4
			CalcProbe.setExp(rd.getCell(2, 3), formula); // C2
			IReport rpt = CalcProbe.calc(rd, null);
			CalcProbe.dump("AVG " + formula, rpt, true);
		} catch (Throwable t) {
			System.out.println("[AVG " + formula + "] FAIL: "
					+ t.getClass().getName() + ": " + t.getMessage());
		}
	}
}
