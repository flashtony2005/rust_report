/*
 * ExpectedJson.java -- maps an in-memory ReportDefine2 to the expected.json
 * golden document consumed by the Rust parser tests.
 *
 * Every value is read back through the public model getters; nothing is
 * reverse-engineered from the serialized bytes.
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.DataSetConfig;
import com.raqsoft.reportl.usermodel.DataSetMetaData;
import com.raqsoft.reportl.usermodel.FileDataSetConfig;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.Param;
import com.raqsoft.reportl.usermodel.ParamMetaData;
import com.raqsoft.reportl.usermodel.PrintSetup;
import com.scudata.common.Area;

/** Renders the golden expected.json for one report model. */
final class ExpectedJson {

	// ---- NormalCell model defaults (NormalCell.java field initialisers) ----
	private static final String DEF_FONT_NAME = "\u5b8b\u4f53"; // Song
	private static final short DEF_FONT_SIZE = 12;
	private static final byte DEF_EXTEND_MODE = 16;  // EXTEND_DEFAULT
	private static final byte DEF_HALIGN = -48;      // HALIGN_LEFT
	private static final byte DEF_VALIGN = -31;      // VALIGN_MIDDLE
	private static final int DEF_FORE = 0xFF000000;  // Color.black.getRGB()
	private static final int DEF_BACK = 0xFFFFFFFF;  // Color.white.getRGB()

	private ExpectedJson() {
	}

	static String render(ReportDefine2 rd) {
		int matrixRows = rd.getRowCount() + 1; // includes the 0-index border
		int matrixCols = rd.getColCount() + 1;

		JsonObj root = new JsonObj();
		root.put("rows", (long) matrixRows);
		root.put("cols", (long) matrixCols);
		root.put("row_count", (long) rd.getRowCount());
		root.put("col_count", (long) rd.getColCount());
		root.put("row_heights", rowHeights(rd));
		root.put("col_widths", colWidths(rd));
		root.put("cells", cells(rd));
		root.put("params", params(rd));
		root.put("datasets", datasets(rd));
		root.put("print_setup", printSetup(rd));
		root.put("scalars", scalars(rd));
		return root.done();
	}

	private static JsonArr rowHeights(ReportDefine2 rd) {
		JsonArr a = new JsonArr();
		for (int i = 1; i <= rd.getRowCount(); i++) {
			a.add(rd.getRowCell(i).getRowHeight());
		}
		return a;
	}

	private static JsonArr colWidths(ReportDefine2 rd) {
		JsonArr a = new JsonArr();
		for (int j = 1; j <= rd.getColCount(); j++) {
			a.add(rd.getColCell(j).getColWidth());
		}
		return a;
	}

	private static JsonArr cells(ReportDefine2 rd) {
		JsonArr a = new JsonArr();
		for (int i = 1; i <= rd.getRowCount(); i++) {
			for (int j = 1; j <= rd.getColCount(); j++) {
				INormalCell nc = rd.getCell(i, j);
				if (nc == null) {
					continue;
				}
				// fillRecord spreads the anchor object across the whole merge
				// area, so a re-read model has the anchor sitting on every
				// merged slot. Only the anchor slot is recorded (per spec).
				if (nc.isMerged()) {
					Area ma = nc.getMergedArea();
					if (ma.getBeginRow() != i || ma.getBeginCol() != j) {
						continue;
					}
				}
				if (isBlank(nc)) {
					continue;
				}
				a.add(emitCell(i, j, nc));
			}
		}
		return a;
	}

	private static JsonObj emitCell(int row, int col, INormalCell nc) {
		JsonObj o = new JsonObj();
		o.put("row", (long) row);
		o.put("col", (long) col);
		o.put("value", valueToJson(nc.getValue()));
		if (nc.getDispValue() != null) {
			o.putString("dispValue", nc.getDispValue());
		}
		if (nc.getExtendMode() != DEF_EXTEND_MODE) {
			o.put("extendMode", (long) nc.getExtendMode());
		}
		if (nc.isMerged()) {
			Area a = nc.getMergedArea();
			JsonObj m = new JsonObj();
			m.put("begin_row", (long) a.getBeginRow());
			m.put("begin_col", (long) a.getBeginCol());
			m.put("end_row", (long) a.getEndRow());
			m.put("end_col", (long) a.getEndCol());
			o.put("merged", m);
		}
		if (nc.getHAlign() != DEF_HALIGN) {
			o.put("hAlign", (long) nc.getHAlign());
		}
		if (nc.getVAlign() != DEF_VALIGN) {
			o.put("vAlign", (long) nc.getVAlign());
		}
		if (!DEF_FONT_NAME.equals(nc.getFontName())) {
			o.putString("fontName", nc.getFontName());
		}
		if (nc.getFontSize() != DEF_FONT_SIZE) {
			o.put("fontSize", (long) nc.getFontSize());
		}
		if (nc.isBold()) {
			o.put("bold", true);
		}
		if (nc.isItalic()) {
			o.put("italic", true);
		}
		if (nc.isUnderline()) {
			o.put("underline", true);
		}
		if (!nc.isVisible()) {
			o.put("visible", false);
		}
		if (nc.getTextWrap()) {
			o.put("textWrap", true);
		}
		if (nc.getForeColor() != DEF_FORE) {
			o.put("foreColor", (long) nc.getForeColor());
		}
		if (nc.getBackColor() != DEF_BACK) {
			o.put("backColor", (long) nc.getBackColor());
		}
		if (nc.getRotationAngle() != 0f) {
			o.put("rotationAngle", nc.getRotationAngle());
		}
		if (nc.getIndent() != 0f) {
			o.put("indent", nc.getIndent());
		}
		if (nc.getFormat() != null) {
			o.putString("format", nc.getFormat());
		}
		return o;
	}

	/**
	 * A cell is "blank" when every readable attribute still holds its model
	 * default. Mirrors emitCell exactly, so the two never drift apart.
	 */
	private static boolean isBlank(INormalCell nc) {
		return nc.getValue() == null
				&& nc.getDispValue() == null
				&& !nc.isMerged()
				&& nc.getExtendMode() == DEF_EXTEND_MODE
				&& nc.getHAlign() == DEF_HALIGN
				&& nc.getVAlign() == DEF_VALIGN
				&& DEF_FONT_NAME.equals(nc.getFontName())
				&& nc.getFontSize() == DEF_FONT_SIZE
				&& !nc.isBold() && !nc.isItalic() && !nc.isUnderline()
				&& nc.isVisible() && !nc.getTextWrap()
				&& nc.getForeColor() == DEF_FORE
				&& nc.getBackColor() == DEF_BACK
				&& nc.getRotationAngle() == 0f
				&& nc.getIndent() == 0f
				&& nc.getFormat() == null;
	}

	private static String valueToJson(Object v) {
		if (v == null) {
			return "null";
		}
		if (v instanceof Boolean) {
			return v.toString();
		}
		if (v instanceof String) {
			return Json.quote((String) v);
		}
		if (v instanceof Number) {
			return v.toString();
		}
		return Json.quote(v.toString());
	}

	private static JsonArr params(ReportDefine2 rd) {
		JsonArr a = new JsonArr();
		ParamMetaData pmd = rd.getParamMetaData();
		if (pmd == null) {
			return a;
		}
		for (int i = 0; i < pmd.getParamCount(); i++) {
			Param p = pmd.getParam(i);
			JsonObj po = new JsonObj();
			po.putString("name", p.getParamName());
			po.putString("description", p.getDescription());
			po.put("param_type", (long) p.getParamType());
			po.put("data_type", (long) p.getDataType());
			po.putString("value", p.getValue());
			a.add(po);
		}
		return a;
	}

	private static JsonArr datasets(ReportDefine2 rd) {
		JsonArr a = new JsonArr();
		DataSetMetaData dsmd = rd.getDataSetMetaData();
		if (dsmd == null) {
			return a;
		}
		for (int i = 0; i < dsmd.getDataSetConfigCount(); i++) {
			DataSetConfig d = dsmd.getDataSetConfig(i);
			JsonObj dObj = new JsonObj();
			dObj.putString("name", d.getName());
			dObj.putString("data_source_name", d.getDataSourceName());
			dObj.putString("factory_class", d.getFactoryClass());
			if (d instanceof FileDataSetConfig) {
				dObj.putString("file_name", ((FileDataSetConfig) d).getFileName());
			}
			a.add(dObj);
		}
		return a;
	}

	private static JsonObj printSetup(ReportDefine2 rd) {
		JsonObj pso = new JsonObj();
		PrintSetup ps = rd.getPrintSetup();
		if (ps == null) {
			return pso;
		}
		pso.put("paper", (long) ps.getPaper());
		pso.put("paper_width", ps.getPaperWidth());
		pso.put("paper_height", ps.getPaperHeight());
		pso.put("orientation", (long) ps.getOrientation());
		pso.put("left_margin", ps.getLeftMargin());
		pso.put("right_margin", ps.getRightMargin());
		pso.put("top_margin", ps.getTopMargin());
		pso.put("bottom_margin", ps.getBottomMargin());
		pso.put("copy_count", (long) ps.getCopyCount());
		return pso;
	}

	private static JsonObj scalars(ReportDefine2 rd) {
		JsonObj sc = new JsonObj();
		sc.put("disp_ratio", (long) rd.getDispRatio());
		sc.put("unit", (long) rd.getUnit());
		sc.putString("tip", rd.getTip());
		sc.putString("notes", rd.getNotes());
		sc.putString("chart_style_name", rd.getChartStyleName());
		sc.putString("future_cell_num_exp", rd.getFutureCellNumExp());
		sc.put("image_max_width", (long) rd.getImageMaxWidth());
		sc.put("image_max_height", (long) rd.getImageMaxHeight());
		sc.put("page_border_mode", (long) rd.getPageBorderMode());
		sc.putString("report_style_name", rd.getReportStyleName());
		sc.putString("validity_script", rd.getValidityScript());
		sc.putString("big_data_set_name", rd.getBigDataSetName());
		sc.putString("password", rd.getPassword());
		sc.put("submit", (long) rd.getSubmit());
		sc.put("high_precision", rd.isHighPrecision());
		sc.put("threshold", (long) rd.getThreshold());
		return sc;
	}
}
