/*
 * EngineModelsP0.java -- models for the three P0 ground truths that were
 * missing when M2 was audited (2026-10-12):
 *
 *   engine_merge      M2-04  a merged area that survives/crosses extension
 *   engine_cycle      M2-07  a reference cycle -> ReportError, not a grid
 *   engine_samemerge  M2-08  mergeSame=98 collapsing equal adjacent values
 *
 * Facts this file encodes, all observed on the product engine (ProbeP0):
 *
 *  - mergeSame is NOT applied by Engine.calc(). The only callers of
 *    ExtCellSet.mergeSame() are SimplePager.createPages() (:133,:179) and
 *    CalcSubRpt. So a mergeSame fixture MUST be read off a PageBuilder page,
 *    never off the raw calc() grid. This is exactly why the first M2 audit
 *    found no calculated-state merge anywhere.
 *  - merge area codes: 89 = mergeSame (default 97 = off), 90 = mergeNull
 *    (default 113), 91 = mergeSameMode (default 118). 98 = vertical merge,
 *    99 = horizontal merge, 100 = both.
 *  - A merge that spans INTO the extended rows keeps its span on the anchor
 *    (A2 merged A2:A3 -> rowMerge=2 on the extending anchor).
 *  - A3/A2 style reference cycles are caught by the engine itself and raise
 *    ReportError with an IIllIIIIIIIIIlll diagnostic carrying the chain.
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.scudata.common.Area;

final class EngineModelsP0 {

	private EngineModelsP0() {
	}

	/**
	 * engine_merge -- M2-04. Two merged areas on the extending row:
	 *   A2 merged A2:A3   (vertical span crossing into the extended rows)
	 *   C2 merged C2:D2   (horizontal span carried into every instance)
	 * The extension must not shrink either span.
	 */
	static ReportDefine2 mergeFixture() {
		ReportDefine2 rd = new ReportDefine2(4, 4);
		EngineModels.lit(rd, 1, 1, "region");
		EngineModels.lit(rd, 1, 2, "qty");
		EngineModels.lit(rd, 1, 3, "hspan");

		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(region)");
		a2.setMergedArea(new Area(2, 1, 3, 1));

		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");

		INormalCell c2 = rd.getCell(2, 3);
		c2.setValue("span");
		c2.setMergedArea(new Area(2, 3, 2, 4));

		EngineModels.lit(rd, 4, 1, "tail");
		CalcProbe.attachFileDataSet(rd, "ds1", EngineModels.csvName("engine_merge"));
		return rd;
	}

	/**
	 * engine_cycle -- M2-07. A2<-B2<-A2 style mutual reference. Calculation
	 * must fail with ReportError; the fixture pins the failure, there is no
	 * grid to compare. The dataset is attached only so the report is a
	 * realistic one -- the cycle is detected before any dataset is touched.
	 */
	static ReportDefine2 cycleFixture() {
		ReportDefine2 rd = new ReportDefine2(3, 3);
		EngineModels.lit(rd, 1, 1, "cycle");
		CalcProbe.setExp(rd.getCell(2, 2), "C2");
		CalcProbe.setExp(rd.getCell(2, 3), "B2");
		CalcProbe.attachFileDataSet(rd, "ds1", EngineModels.csvName("engine_cycle"));
		return rd;
	}

	/**
	 * engine_samemerge -- M2-08. Column A repeats A,A,B,B,C; the extending
	 * cell carries mergeSame=98 (vertical). Read off the paged grid: A
	 * collapses to rowMerge=2 over rows 2..3 and B over rows 4..5, while the
	 * unique trailing C stays rowMerge=1.
	 */
	static ReportDefine2 samemergeFixture() {
		ReportDefine2 rd = new ReportDefine2(3, 3);
		EngineModels.lit(rd, 1, 1, "region");
		EngineModels.lit(rd, 1, 2, "qty");
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
		a2.setMergeSame(SAME_V);
		CalcProbe.attachFileDataSet(rd, "ds1",
				EngineModels.csvName("engine_samemerge"));
		return rd;
	}

	/** mergeSame code meaning, verified on the engine. */
	static final byte SAME_OFF = (byte) 97;
	static final byte SAME_V = (byte) 98;
	static final byte SAME_H = (byte) 99;
	static final byte SAME_VH = (byte) 100;

	/** engine_samemerge needs duplicate adjacent values. */
	static String[][] sameRows() {
		return new String[][] { { "A", "10" }, { "A", "20" }, { "B", "30" },
				{ "B", "40" }, { "C", "50" } };
	}
}
