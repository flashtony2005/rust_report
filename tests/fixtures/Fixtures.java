/*
 * Fixtures.java -- the three .rpx fixture models.
 *
 * Each builder returns an in-memory ReportDefine2. The caller serialises it
 * with the product writer and renders expected.json from the same object, so
 * the binary and the golden document can never disagree.
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.DataSetMetaData;
import com.raqsoft.reportl.usermodel.FileDataSetConfig;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.Param;
import com.raqsoft.reportl.usermodel.ParamMetaData;
import com.raqsoft.reportl.usermodel.PrintSetup;
import com.raqsoft.reportl.usermodel.Types2;
import com.scudata.common.Area;

/** Builds the simple / merged / full fixture models. */
final class Fixtures {
	private Fixtures() {
	}

	static ReportDefine2 build(String name) {
		if ("simple".equals(name)) {
			return simple();
		}
		if ("merged".equals(name)) {
			return merged();
		}
		return full();
	}

	// =====================================================================
	// simple -- 5x4 matrix, mixed string/number values, per-row heights,
	// per-column widths, header font + alignment.
	// =====================================================================
	private static ReportDefine2 simple() {
		ReportDefine2 rd = new ReportDefine2(5, 4);
		INormalCell c;

		c = rd.getCell(1, 1);
		c.setValue("ReportLite");
		c.setFontName("Arial");
		c.setFontSize((short) 14);
		c.setBold(true);
		c.setHAlign(INormalCell.HALIGN_CENTER);

		c = rd.getCell(1, 2);
		c.setValue("Sales");
		c.setFontName("Arial");
		c.setBold(true);
		c.setHAlign(INormalCell.HALIGN_CENTER);

		c = rd.getCell(1, 3);
		c.setValue("Region");
		c.setBold(true);
		c.setHAlign(INormalCell.HALIGN_CENTER);

		c = rd.getCell(1, 4);
		c.setValue("Qty");
		c.setBold(true);
		c.setHAlign(INormalCell.HALIGN_RIGHT);

		c = rd.getCell(2, 1);
		c.setValue("North");
		c = rd.getCell(2, 2);
		c.setValue(Integer.valueOf(100));
		c = rd.getCell(2, 3);
		c.setValue("East");
		c = rd.getCell(2, 4);
		c.setValue(Integer.valueOf(200));

		c = rd.getCell(3, 1);
		c.setValue("South");
		c = rd.getCell(3, 2);
		c.setValue(Integer.valueOf(150));
		c = rd.getCell(3, 3);
		c.setValue("West");
		c = rd.getCell(3, 4);
		c.setValue(Integer.valueOf(120));

		c = rd.getCell(4, 1);
		c.setValue("Total");
		c.setBold(true);
		c = rd.getCell(4, 2);
		c.setValue(Integer.valueOf(370));
		c.setBold(true);

		c = rd.getCell(5, 1);
		c.setValue("Avg");
		c = rd.getCell(5, 2);
		c.setValue(Double.valueOf(92.5));
		c.setHAlign(INormalCell.HALIGN_RIGHT);

		// Row heights: rows 3 and 5 stay at the model default of 8.0.
		rd.getRowCell(1).setRowHeight(24f);
		rd.getRowCell(2).setRowHeight(16f);
		rd.getRowCell(4).setRowHeight(20f);
		// Column widths: column 4 stays at the model default of 25.0.
		rd.getColCell(1).setColWidth(80f);
		rd.getColCell(2).setColWidth(60f);
		rd.getColCell(3).setColWidth(60f);
		return rd;
	}

	// =====================================================================
	// merged -- one 1x3 horizontal, one 2x2 block, one 3x1 vertical merge
	// (anchors carry the values), plus plain cells.
	// =====================================================================
	private static ReportDefine2 merged() {
		ReportDefine2 rd = new ReportDefine2(6, 5);
		INormalCell c;

		// 1x3 horizontal merge, anchor (1,1) .. (1,3)
		c = rd.getCell(1, 1);
		c.setValue("Title");
		c.setFontName("Arial");
		c.setFontSize((short) 14);
		c.setBold(true);
		c.setHAlign(INormalCell.HALIGN_CENTER);
		setArea(c, 1, 1, 1, 3);

		// 2x2 block merge, anchor (2,2) .. (3,3)
		c = rd.getCell(2, 2);
		c.setValue("Merged2x2");
		c.setHAlign(INormalCell.HALIGN_CENTER);
		c.setVAlign(INormalCell.VALIGN_MIDDLE);
		setArea(c, 2, 2, 3, 3);

		// 3x1 vertical merge, anchor (4,4) .. (6,4)
		c = rd.getCell(4, 4);
		c.setValue("Side");
		setArea(c, 4, 4, 6, 4);

		// plain cells outside every merge
		c = rd.getCell(2, 5);
		c.setValue(Integer.valueOf(99));
		c = rd.getCell(5, 1);
		c.setValue("A");
		c = rd.getCell(5, 2);
		c.setValue("B");
		c = rd.getCell(5, 3);
		c.setValue("C");

		rd.getRowCell(1).setRowHeight(22f);
		rd.getRowCell(2).setRowHeight(18f);
		rd.getColCell(1).setColWidth(70f);
		rd.getColCell(4).setColWidth(40f);
		rd.getColCell(5).setColWidth(50f);
		return rd;
	}

	// =====================================================================
	// full -- extend modes 16/17/18/19, value != dispValue, a 2x2 merge,
	// row heights, column widths, params, a file dataset, print setup, and
	// the full scalar tail.
	// =====================================================================
	private static ReportDefine2 full() {
		ReportDefine2 rd = new ReportDefine2(7, 6);

		// setUnit rescales every row height / column width, so it must run
		// BEFORE the explicit dimensions below (which then win outright).
		rd.setUnit((byte) 0); // inch; the model default is 1 (mm)

		// ---- scalar tail ----
		rd.setDispRatio(100); // model default is 133
		rd.setTip("full fixture tip");
		rd.setNotes("full fixture notes");
		rd.setChartStyleName("chartStyle1");
		rd.setFutureCellNumExp("=1");
		rd.setImageMaxWidth(800);
		rd.setImageMaxHeight(600);
		rd.setPageBorderMode((byte) 1);
		rd.setReportStyleName("reportStyle1");
		rd.setValidityScript("validityScript1");
		rd.setBigDataSetName("bigDataSet1");
		rd.setPassword("fixture-secret");
		rd.setHighPrecision(false); // model default is true
		rd.setThreshold(5);         // model default is -1
		rd.setSubmit((byte) 1);

		INormalCell c;

		// extend modes: all four constants, one cell each
		c = rd.getCell(1, 1);
		c.setValue("ExtendDefault");
		c.setExtendMode((byte) 16);
		c = rd.getCell(2, 1);
		c.setValue("ExtendNone");
		c.setExtendMode(INormalCell.EXTEND_NONE);
		c = rd.getCell(3, 1);
		c.setValue("ExtendVertical");
		c.setExtendMode(INormalCell.EXTEND_VERTICAL);
		c = rd.getCell(4, 1);
		c.setValue("ExtendHoriz");
		c.setExtendMode(INormalCell.EXTEND_HORIZONTAL);

		// value != dispValue
		c = rd.getCell(1, 2);
		c.setValue("1000");
		c.setDispValue("1,000");
		c.setHAlign(INormalCell.HALIGN_RIGHT);
		c = rd.getCell(2, 2);
		c.setValue(Integer.valueOf(2000));
		c.setDispValue("2,000");
		c.setHAlign(INormalCell.HALIGN_RIGHT);

		// full style coverage
		c = rd.getCell(1, 3);
		c.setValue("Styled");
		c.setFontName("Arial");
		c.setFontSize((short) 16);
		c.setBold(true);
		c.setItalic(true);
		c.setUnderline(true);
		c.setForeColor(0xFF0000);
		c.setBackColor(0xFFFF00);
		c.setTextWrap(true);
		c.setFormat("#,##0");
		c = rd.getCell(2, 3);
		c.setValue("Rotated");
		c.setRotationAngle(45f);
		c.setIndent(2f);
		c = rd.getCell(3, 3);
		c.setValue("Hidden");
		c.setVisible(false);

		// 2x2 merge, anchor (5,2) .. (6,3)
		c = rd.getCell(5, 2);
		c.setValue("MergedFull");
		c.setHAlign(INormalCell.HALIGN_CENTER);
		setArea(c, 5, 2, 6, 3);

		// value-type coverage
		c = rd.getCell(5, 4);
		c.setValue(Double.valueOf(3.14159));
		c = rd.getCell(6, 4);
		c.setValue(Boolean.TRUE);
		c = rd.getCell(7, 1);
		c.setValue("Footer");

		// Every dimension is set explicitly (unit=0 already rescaled the
		// defaults, so nothing here is left as a derived float).
		rd.getRowCell(1).setRowHeight(28f);
		rd.getRowCell(2).setRowHeight(18f);
		rd.getRowCell(3).setRowHeight(18f);
		rd.getRowCell(4).setRowHeight(14f);
		rd.getRowCell(5).setRowHeight(16f);
		rd.getRowCell(6).setRowHeight(14f);
		rd.getRowCell(7).setRowHeight(22f);
		rd.getColCell(1).setColWidth(90f);
		rd.getColCell(2).setColWidth(70f);
		rd.getColCell(3).setColWidth(70f);
		rd.getColCell(4).setColWidth(50f);
		rd.getColCell(5).setColWidth(50f);
		rd.getColCell(6).setColWidth(60f);

		// ---- parameters ----
		ParamMetaData pmd = new ParamMetaData();
		pmd.addParam(new Param("startDate", "Start date", (byte) 0,
				Types2.DT_STRING, "2024-01-01"));
		pmd.addParam(new Param("topN", "Top N rows", (byte) 0,
				Types2.DT_INT, "10"));
		rd.setParamMetaData(pmd);

		// ---- one file dataset ----
		DataSetMetaData dsmd = new DataSetMetaData();
		FileDataSetConfig ds = new FileDataSetConfig();
		ds.setName("ds1");
		ds.setFileName("data.csv");
		ds.setDataSourceName("ds1");
		dsmd.addDataSetConfig(ds);
		rd.setDataSetMetaData(dsmd);

		// ---- print setup (A4, portrait, explicit margins) ----
		PrintSetup ps = rd.getPrintSetup();
		ps.setPaper((short) 9); // A4
		ps.setPaperWidth(595f);
		ps.setPaperHeight(842f);
		ps.setOrientation((byte) 1); // portrait
		ps.setLeftMargin(20f);
		ps.setRightMargin(20f);
		ps.setTopMargin(25f);
		ps.setBottomMargin(25f);
		ps.setCopyCount(1);
		return rd;
	}

	/** setMergedArea via the unambiguous setArea(br, bc, er, ec) ordering. */
	private static void setArea(INormalCell cell, int br, int bc, int er, int ec) {
		Area a = new Area();
		a.setArea(br, bc, er, ec);
		cell.setMergedArea(a);
	}
}
