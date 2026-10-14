/*
 * EngineModels.java -- the four M2 engine fixture models.
 *
 * Each is built with the product API only (ReportDefine2 + cell setters +
 * CalcProbe.setExp), never by writing bytes. Kept apart from the generator so
 * both files stay under the 300-line gate.
 *
 *   engine_sum    sum()/aggregate forms; two datasets so the "dataset inside
 *                 an extension" case can be compared with the plain one
 *   engine_ext    extendMode 16/17/18/19 pinned one per column
 *   engine_expr   arithmetic + relative/absolute-column cell references
 *   engine_pager  40 data rows + a dedicated -90 group header row
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IRowCell;

final class EngineModels {

	private EngineModels() {
	}

	static ReportDefine2 build(String name) {
		if ("engine_sum".equals(name)) {
			return sumFixture();
		}
		if ("engine_ext".equals(name)) {
			return extFixture();
		}
		if ("engine_expr".equals(name)) {
			return exprFixture();
		}
		if ("engine_pager".equals(name)) {
			return pagerFixture();
		}
		if ("engine_merge".equals(name)) {
			return EngineModelsP0.mergeFixture();
		}
		if ("engine_cycle".equals(name)) {
			return EngineModelsP0.cycleFixture();
		}
		if ("engine_disp".equals(name)) {
			return dispFixture();
		}
		if ("engine_rowheight".equals(name)) {
			return EngineModelsM2.rowheightFixture();
		}
		if ("engine_head_top".equals(name)) {
			return EngineModelsHead.topFixture();
		}
		if ("engine_head_left".equals(name)) {
			return EngineModelsHead.leftFixture();
		}
		if ("engine_types".equals(name)) {
			return EngineModelsTypes.typesFixture();
		}
		if ("engine_avgrange".equals(name)) {
			return EngineModelsRange.avgRangeFixture();
		}
		if ("engine_listrange".equals(name)) {
			return EngineModelsListRange.listRangeFixture();
		}
		if ("engine_collate".equals(name)) {
			return EngineModelsCollate.collateFixture();
		}
		if ("engine_param".equals(name)) {
			return EngineModelsM3.paramFixture();
		}
		return EngineModelsP0.samemergeFixture();
	}

	/**
	 * engine_sum -- every sum form we could get the product to evaluate.
	 *
	 * Two datasets on purpose. ds1 is referenced by extending cells, so its
	 * "current" advances during calculation and ds1.sum(qty) degenerates to
	 * the current row (100/200/150). ds2 is referenced only by aggregates,
	 * so its current stays at the root group and ds2.sum(qty) is the real
	 * 450. Both behaviours are pinned; the Rust engine must reproduce both.
	 *
	 * Columns and formulas:
	 *   C  B2*2                 arithmetic on a cell ref
	 *   D  sum(B2:B4)           does NOT aggregate -- returns operand 1
	 *   E  ds1.sum(qty)         current-row degenerate (dataset in extension)
	 *   F  sum(ds1.select(qty)) same degeneration
	 *   G  ds2.sum(qty)         true dataset aggregate -> 450
	 *   H  sum(ds2.select(qty)) sum over a List operand -> 450
	 *   I  ds2.count()          -> 3
	 */
	private static ReportDefine2 sumFixture() {
		ReportDefine2 rd = new ReportDefine2(6, 9);
		lit(rd, 1, 1, "region");
		lit(rd, 1, 2, "qty");
		lit(rd, 1, 3, "twice");
		lit(rd, 1, 4, "sumRange");
		lit(rd, 1, 5, "ds1Sum");
		lit(rd, 1, 6, "sumList1");
		lit(rd, 1, 7, "ds2Sum");
		lit(rd, 1, 8, "sumList2");
		lit(rd, 1, 9, "ds2Count");
		CalcProbe.setExp(rd.getCell(2, 1), "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
		CalcProbe.setExp(rd.getCell(2, 3), "B2*2");
		CalcProbe.setExp(rd.getCell(2, 4), "sum(B2:B4)");
		CalcProbe.setExp(rd.getCell(2, 5), "ds1.sum(qty)");
		CalcProbe.setExp(rd.getCell(2, 6), "sum(ds1.select(qty))");
		CalcProbe.setExp(rd.getCell(2, 7), "ds2.sum(qty)");
		CalcProbe.setExp(rd.getCell(2, 8), "sum(ds2.select(qty))");
		CalcProbe.setExp(rd.getCell(2, 9), "ds2.count()");
		CalcProbe.attachFileDataSet(rd, "ds1", csvName("engine_sum"));
		CalcProbe.attachFileDataSet(rd, "ds2", csvName("engine_sum") + "2");
		return rd;
	}

	/**
	 * engine_ext -- one dataset row set, three cells whose extend modes are
	 * pinned to 17/18/19 on THREE SEPARATE ROWS so each owns its own
	 * extension envelope.
	 *
	 * The previous layout put all four modes on one row; there the leftmost
	 * cell's vertical extension swallows the row band and 17/18/19 all came
	 * out with sub_cells=1, i.e. indistinguishable. Observed behaviour with
	 * the split layout (ProbeP0 S7):
	 *   17  -> one cell holding an ArrayList of every row, sub_cells=1
	 *   18  -> five sub-cells down column 1
	 *   19  -> five sub-cells across one row (columns grow)
	 */
	private static ReportDefine2 extFixture() {
		ReportDefine2 rd = new ReportDefine2(4, 3);
		lit(rd, 1, 1, "none17");
		lit(rd, 1, 2, "vertical18");
		lit(rd, 1, 3, "horiz19");
		INormalCell c17 = rd.getCell(2, 1);
		c17.setExtendMode(INormalCell.EXTEND_NONE);
		CalcProbe.setExp(c17, "ds1.select(region)");
		INormalCell c18 = rd.getCell(3, 1);
		c18.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(c18, "ds1.select(region)");
		INormalCell c19 = rd.getCell(4, 1);
		c19.setExtendMode(INormalCell.EXTEND_HORIZONTAL);
		CalcProbe.setExp(c19, "ds1.select(region)");
		CalcProbe.attachFileDataSet(rd, "ds1", csvName("engine_ext"));
		return rd;
	}

	/**
	 * engine_expr -- arithmetic and cell references. Column C uses a
	 * relative ref, D an absolute-column ref, E a mixed expression.
	 */
	private static ReportDefine2 exprFixture() {
		ReportDefine2 rd = new ReportDefine2(6, 5);
		lit(rd, 1, 1, "region");
		lit(rd, 1, 2, "qty");
		lit(rd, 1, 3, "ref");
		lit(rd, 1, 4, "absCol");
		lit(rd, 1, 5, "mixed");
		CalcProbe.setExp(rd.getCell(2, 1), "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
		CalcProbe.setExp(rd.getCell(2, 3), "B2");
		CalcProbe.setExp(rd.getCell(2, 4), "$B2*3");
		CalcProbe.setExp(rd.getCell(2, 5), "B2+$B2");
		CalcProbe.attachFileDataSet(rd, "ds1", csvName("engine_expr"));
		return rd;
	}

	/**
	 * engine_pager -- 40 data rows so the report spans several A4 pages,
	 * with row 2 marked as a group-header row (rowType -90, level 1) so the
	 * pager repeats it at the top of every page.
	 */
	private static ReportDefine2 pagerFixture() {
		ReportDefine2 rd = new ReportDefine2(4, 3);
		lit(rd, 1, 1, "region");
		lit(rd, 1, 2, "qty");
		lit(rd, 1, 3, "doubled");
		lit(rd, 2, 1, "group: R*");
		rd.getRowCell(2).setRowType(IRowCell.TYPE_GROUP_HEADER);
		rd.getRowCell(2).setGroupHeaderLevel((byte) 1);
		CalcProbe.setExp(rd.getCell(3, 1), "ds1.select(region)");
		CalcProbe.setExp(rd.getCell(3, 2), "ds1.select(qty)");
		CalcProbe.setExp(rd.getCell(3, 3), "B3*2");
		for (int r = 1; r <= 4; r++) {
			rd.getRowCell(r).setRowHeight(20f);
		}
		CalcProbe.attachFileDataSet(rd, "ds1", csvName("engine_pager"));
		return rd;
	}

	/**
	 * engine_disp -- the dispValue contract (M2-14). Every cell here carries a
	 * display value that differs from String.valueOf(value), which is the only
	 * way the DISPVALUE slot shows up in expected.json. Four shapes:
	 *
	 *   (2,1) literal "North"  + disp "北区"      value survives, display differs
	 *   (2,2) literal 100      + disp "100.00"   Integer stays Integer (no widening)
	 *   (2,3) value=null       + disp "仅显示值"  非空但无值：value==null 不得当空格丢弃
	 *   (2,4) 6*1 -> 6         + disp "6 件"      计算值与显示值并存
	 *   (3,1) ds1.select(qty)  + disp "Q"        纵向扩展锚点：3 个附属格都要带 "Q"
	 *
	 * Row 4 is empty on purpose: the extension of row 3 pushes it to the last
	 * calculated row, so template_rows(4) + 3 - 1 == calculated rows(6).
	 */
	private static ReportDefine2 dispFixture() {
		ReportDefine2 rd = new ReportDefine2(4, 4);
		lit(rd, 1, 1, "A");
		lit(rd, 1, 2, "B");
		lit(rd, 1, 3, "C");
		lit(rd, 1, 4, "D");
		lit(rd, 2, 1, "North");
		disp(rd, 2, 1, "北区");
		lit(rd, 2, 2, Integer.valueOf(100));
		disp(rd, 2, 2, "100.00");
		disp(rd, 2, 3, "仅显示值");
		CalcProbe.setExp(rd.getCell(2, 4), "6*1");
		disp(rd, 2, 4, "6 件");
		CalcProbe.setExp(rd.getCell(3, 1), "ds1.select(qty)");
		disp(rd, 3, 1, "Q");
		CalcProbe.attachFileDataSet(rd, "ds1", csvName("engine_disp"));
		return rd;
	}

	/** Writes a display value that differs from String.valueOf(value). */
	static void disp(ReportDefine2 rd, int r, int c, String dv) {
		INormalCell cell = rd.getCell(r, c);
		if (cell == null) {
			throw new IllegalStateException("getCell(" + r + "," + c + ") == null");
		}
		cell.setDispValue(dv);
	}

	// =================================================================
	// Data
	// =================================================================

	static String csvName(String fixture) {
		return fixture + ".csv";
	}

	static String[][] rows(String fixture) {
		if ("engine_pager".equals(fixture)) {
			String[][] r = new String[40][2];
			for (int i = 0; i < 40; i++) {
				r[i][0] = "R" + (i + 1);
				r[i][1] = Integer.toString((i + 1) * 10);
			}
			return r;
		}
		if ("engine_ext".equals(fixture)) {
			return new String[][] { { "R1", "10" }, { "R2", "20" }, { "R3", "30" },
					{ "R4", "40" }, { "R5", "50" } };
		}
		if ("engine_rowheight".equals(fixture)) {
			return EngineModelsM2.rows();
		}
		if ("engine_head_top".equals(fixture) || "engine_head_left".equals(fixture)) {
			return EngineModelsHead.rows();
		}
		if ("engine_samemerge".equals(fixture)) {
			return EngineModelsP0.sameRows();
		}
		if ("engine_avgrange".equals(fixture)) {
			return EngineModelsRange.rows();
		}
		if ("engine_listrange".equals(fixture)) {
			return EngineModelsRange.rows();
		}
		return new String[][] { { "North", "100" }, { "East", "200" },
				{ "South", "150" } };
	}

	static void lit(ReportDefine2 rd, int r, int c, Object v) {
		rd.getCell(r, c).setValue(v);
	}
}
