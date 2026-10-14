/*
 * EngineModelsListRange.java -- engine_listrange: the M2-12 region handler's
 * LIST-ITERATE branch, pinned by the product.
 *
 * WHY a dedicated fixture: §14.3 declared `sum/avg/max/min(A:B)`'s
 * List-iterate branch an explicit gap. A plain `B2:B4` can never reach it --
 * ParamParser splits it into two sub-params and the cellSet operand then
 * evaluates to a CSVariable, not a List. The product DOES reach it as soon
 * as the cellSet operand evaluates to a List of cells:
 *
 *   ParamParser.java:81-129   the top-level scanner SKIPS () [] {}
 *   Sum.java:129-140          cellSet = sub1  ->  `sum(@:[B2:B4])`
 *   Average.java:139-149      cellSet = sub0  ->  `avg([B2:B4]:@)`  [SWAPPED]
 *   ValueList.java:161-162    `[B2:B4]` -> List<ExtCell> over the rectangle
 *   Link.java:78-89           `(B2:B4)` -> the same list
 *   CSCurrent.java:23-28      `@` = current cell's value (getValue(false))
 *
 * Product truth (ProbeListRange.java, B2=100 B3=200 B4=120):
 *   sum(@:[B2:B4]) = 420   vs  sum(B2:B4) = 100     (iterate vs else)
 *   max(@:[B2:B4]) = 200   vs  max(B2:B4) = 100
 *   min(@:[B2:B4]) = 100   vs  min(B2:B4) = 100
 *   avg([B2:B4]:@) = 140.0 vs  avg(B2:B4) = 120.0   (mean vs operand2)
 *
 * The literals are the same as engine_avgrange on purpose, so the two
 * fixtures can be read side by side: the `else` column here must equal
 * engine_avgrange's values.
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;

final class EngineModelsListRange {

	private EngineModelsListRange() {
	}

	static ReportDefine2 listRangeFixture() {
		ReportDefine2 rd = new ReportDefine2(5, 6);
		rd.getCell(1, 2).setValue("B");
		rd.getCell(1, 3).setValue("bracket");
		rd.getCell(1, 4).setValue("paren");
		rd.getCell(1, 5).setValue("else");
		// distinct literals: mean(140) != operand2(120) != operand1(100)
		rd.getCell(2, 2).setValue(Integer.valueOf(100)); // B2
		rd.getCell(3, 2).setValue(Integer.valueOf(200)); // B3
		rd.getCell(4, 2).setValue(Integer.valueOf(120)); // B4
		// `[B2:B4]` -- ValueList colon form -> List<ExtCell>
		CalcProbe.setExp(rd.getCell(2, 3), "sum(@:[B2:B4])");
		CalcProbe.setExp(rd.getCell(3, 3), "avg([B2:B4]:@)");
		CalcProbe.setExp(rd.getCell(4, 3), "max(@:[B2:B4])");
		CalcProbe.setExp(rd.getCell(5, 3), "min(@:[B2:B4])");
		// `(B2:B4)` -- parenthesised Link -> the same list
		CalcProbe.setExp(rd.getCell(2, 4), "sum(@:(B2:B4))");
		CalcProbe.setExp(rd.getCell(3, 4), "avg((B2:B4):@)");
		CalcProbe.setExp(rd.getCell(4, 4), "max(@:(B2:B4))");
		CalcProbe.setExp(rd.getCell(5, 4), "min(@:(B2:B4))");
		// controls -- the single-cell else branch (must match engine_avgrange)
		CalcProbe.setExp(rd.getCell(2, 5), "sum(B2:B4)");
		CalcProbe.setExp(rd.getCell(3, 5), "avg(B2:B4)");
		CalcProbe.setExp(rd.getCell(4, 5), "max(B2:B4)");
		CalcProbe.setExp(rd.getCell(5, 5), "min(B2:B4)");
		CalcProbe.attachFileDataSet(rd, "ds1",
				EngineModels.csvName("engine_listrange"));
		return rd;
	}
}
