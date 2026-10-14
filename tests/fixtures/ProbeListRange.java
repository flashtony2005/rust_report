/*
 * ProbeListRange.java -- M2-12: is the region handler's LIST-ITERATE branch
 * reachable at all, and what does the product compute there?
 *
 * Source chain (all under reportlite/03-model/com/raqsoft/reportl/model/expression):
 *   ParamParser.java:66-79    separators are tried in the order ; -> , -> :
 *   ParamParser.java:81-129   the scanner SKIPS what is inside () [] {} --
 *                             so `sum(@:[B2:B4])` splits into TWO sub-params
 *   Sum.java:129-140          cellSet = sub1, param2 = sub0
 *   Average.java:139-149      SWAPPED: cellSet = sub0, param2 = sub1
 *   Sum.java:155-172          if cellSet evaluates to a List -> iterate
 *                             (every element must be CSVariable/ExtCell),
 *                             setCurrent(each), evaluate param2, Variant2.add
 *   Sum.java:173-187          else: single cell, evaluate param2 ONCE
 *   ValueList.java:161-162    `[B2:B4]` evaluates to List<ExtCell>
 *   Link.java:78-89           `(B2:B4)` (paren form) also gives List<ExtCell>
 *   CSCurrent.java:23-28      `@` = current cell's value (getValue(false))
 *
 * Predicted, if the List branch is reached with `@` as the value operand:
 *   sum(@:[B2:B4]) = 100+200+120 = 420   vs  sum(B2:B4) = 100 (else branch)
 *   avg([B2:B4]:@) = 420/3 = 140.0       vs  avg(B2:B4) = 120.0 (operand2)
 *   max(@:[B2:B4]) = 200                 vs  max(B2:B4) = 100
 *   min(@:[B2:B4]) = 100                 vs  min(B2:B4) = 100
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.IReport;

public class ProbeListRange {

	public static void main(String[] args) {
		String[] forms = {
				// controls -- the else (single-cell) branch, already pinned
				"sum(B2:B4)", "avg(B2:B4)", "max(B2:B4)", "min(B2:B4)",
				// List branch via ValueList `[...]`
				"sum(@:[B2:B4])", "max(@:[B2:B4])", "min(@:[B2:B4])",
				"avg([B2:B4]:@)",
				// List branch via Link `(...)` (parens survive the split)
				"sum(@:(B2:B4))", "max(@:(B2:B4))", "min(@:(B2:B4))",
				"avg((B2:B4):@)",
				// reversed operands -- cellSet becomes the single cell again,
				// so the List must NOT be iterated (control for the reversal)
				"sum([B2:B4]:@)", "avg(@:[B2:B4])" };
		for (String f : forms) {
			probe(f);
		}
	}

	private static void probe(String formula) {
		try {
			ReportDefine2 rd = new ReportDefine2(6, 6);
			// distinct literals so every branch yields a different number
			rd.getCell(2, 2).setValue(Integer.valueOf(100)); // B2
			rd.getCell(3, 2).setValue(Integer.valueOf(200)); // B3
			rd.getCell(4, 2).setValue(Integer.valueOf(120)); // B4
			CalcProbe.setExp(rd.getCell(2, 3), formula); // C2
			IReport rpt = CalcProbe.calc(rd, null);
			CalcProbe.dump("LIST " + formula, rpt, true);
		} catch (Throwable t) {
			System.out.println("[LIST " + formula + "] FAIL: "
					+ t.getClass().getName() + ": " + t.getMessage());
		}
	}
}
