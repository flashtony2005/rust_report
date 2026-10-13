/*
 * CalcJson.java -- renders the calculated state (ExtCellSet) as JSON.
 *
 * This is the M2 ground truth document: for every cell of the calculated
 * grid it records the computed value, the display value, and the extend
 * structure (source cell, row/col merge, extend mode, sub-cell values).
 * The Rust engine (rpx-engine) is asserted against exactly this.
 *
 * Values are typed, not stringified: JSON string => text cell, JSON number
 * => numeric cell, null => empty. The type name is kept alongside because
 * Integer vs Double vs String is itself part of the contract (the product
 * returns Integer(100) where a naive engine would return Double(100.0)).
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.usermodel.PagerInfo;
import com.raqsoft.reportl.usermodel.PageBuilder;
import com.raqsoft.reportl.util.ReportUtils2;

import java.math.BigDecimal;
import java.util.List;

final class CalcJson {

	private CalcJson() {
	}

	/** Renders the calculated grid: dimensions plus every non-empty cell. */
	static String renderCalc(IReport rpt) {
		if (!(rpt instanceof ExtCellSet)) {
			return "null";
		}
		ExtCellSet ecs = (ExtCellSet) rpt;
		JsonArr cells = new JsonArr();
		int rows = ecs.getRowCount();
		int cols = ecs.getColCount();
		// Cells live at rows 1..rows and cols 1..cols (getRowCount is the
		// DATA count, i.e. matrix size minus the header row/col).
		for (int r = 1; r <= rows; r++) {
			for (int c = 1; c <= cols; c++) {
				INormalCell cell = ecs.getCell(r, c);
				if (cell == null) {
					continue;
				}
				Object v = cell.getValue();
				String dv = cell.getDispValue();
				if (v == null && dv == null) {
					continue;
				}
				cells.add(cellJson(ecs, r, c, cell, v, dv));
			}
		}
		JsonObj o = new JsonObj();
		o.put("rows", rows);
		o.put("cols", cols);
		o.put("cells", cells);
		return o.done();
	}

	private static String cellJson(ExtCellSet ecs, int r, int c, INormalCell cell,
			Object v, String dv) {
		JsonObj o = new JsonObj();
		o.put("r", r);
		o.put("c", c);
		o.put("value", value(v));
		o.putString("type", typeName(v));
		if (dv != null && !dv.equals(String.valueOf(v))) {
			o.putString("disp_value", dv);
		}
		// ---- head 图（§12.1 第 5/6 步）：left_head / top_head ----
		// 用 `{src_row, src_col}` 对象而不是裸坐标：`{0,0}` 唯一表示 cell00
		// （cell00 的 getSourceRow()/getSourceCol() 就是 0/0），裸数字没有这个约定。
		// ⛔ 可选键：`getCellLeftHead()` 返回 null 时**省略该键**，不写 null
		//    （与 `source` 的处理一致，也避免 loader 的 Option 语义打架）。
		// ⚠ head 目标**可以没有 source**（engine_head_left 的 (11,1) 就是），
		//    所以这里从格对象自身取 head，而不是从 `source` 推。
		ExtCell self = cell instanceof ExtCell ? (ExtCell) cell : null;
		if (self != null) {
			ExtCell lh = self.getCellLeftHead();
			if (lh != null) {
				o.put("left_head", headJson(lh));
			}
			ExtCell th = self.getCellTopHead();
			if (th != null) {
				o.put("top_head", headJson(th));
			}
		}
		ExtCell src = ecs.getSource(r, c);
		if (src != null) {
			JsonObj s = new JsonObj();
			s.put("src_row", src.getSourceRow());
			s.put("src_col", src.getSourceCol());
			s.put("row_merge", src.getRowMerge());
			s.put("col_merge", src.getColMerge());
			s.put("extend", src.getExtensible());
			int subs = src.extCellSize();
			s.put("sub_cells", subs);
			if (subs > 1) {
				JsonArr arr = new JsonArr();
				for (int i = 0; i < subs; i++) {
					ExtCell sub = src.getExtCell(i);
					JsonObj so = new JsonObj();
					so.put("r", sub.getRow());
					so.put("c", sub.getCol());
					Object sv = sub.getValue(false);
					so.put("value", value(sv));
					so.putString("type", typeName(sv));
					arr.add(so);
				}
				s.put("sub_values", arr);
			}
			o.put("source", s);
		}
		return o.done();
	}


	/**
	 * head 目标的 `{src_row, src_col}`。
	 *
	 * <p>用 **模板出处**（`getSourceRow()/getSourceCol()`）而不是它在计算态里的
	 * `getRow()/getCol()`：167 个被引用的 head 目标里两者有 149 个不同
	 * （插入行会把它放到别处），因此这个选择**有判别力**、必须显式钉住。
	 */
	private static String headJson(ExtCell head) {
		JsonObj h = new JsonObj();
		h.put("src_row", head.getSourceRow());
		h.put("src_col", head.getSourceCol());
		return h.done();
	}
	/**
	 * Pagination ground truth. Each page records its row count and the
	 * first/last non-empty column-1 value, which is what the Rust pager
	 * must reproduce. Returns null when the report cannot be paginated.
	 */
	static String renderPages(IReport rpt, float w, float h, int cols) {
		try {
			PagerInfo pi = ReportUtils2.getPagerInfo(rpt, (int) w, (int) h, cols);
			PageBuilder pb = new PageBuilder(rpt, pi);
			pb.createPages();
			JsonArr pages = new JsonArr();
			IReport[] all = pb.getAllPages();
			for (int i = 0; i < all.length; i++) {
				IReport pg = all[i];
				int n = pg.getRowCount();
				String first = null;
				String last = null;
				JsonArr vals = new JsonArr();
				for (int r = 1; r <= n; r++) {
					INormalCell c = pg.getCell(r, 1);
					Object v = c == null ? null : c.getValue();
					if (v == null) {
						continue;
					}
					if (first == null) {
						first = String.valueOf(v);
					}
					last = String.valueOf(v);
					vals.add(value(v));
				}
				JsonObj po = new JsonObj();
				po.put("page", i + 1);
				po.put("rows", n);
				if (first != null) {
					po.putString("first_value", first);
				}
				if (last != null) {
					po.putString("last_value", last);
				}
				po.put("col1_values", vals);
				pages.add(po);
			}
			JsonObj o = new JsonObj();
			o.put("paper_w", w);
			o.put("paper_h", h);
			o.put("columns", cols);
			o.put("page_count", pb.getPageCount());
			o.put("x_pages", pb.getXPageCount());
			o.put("y_pages", pb.getYPageCount());
			o.put("row_num_per_page", pi.getRowNumPerPage());
			o.put("pages", pages);
			return o.done();
		} catch (Throwable t) {
			JsonObj o = new JsonObj();
			o.put("error", t.getClass().getSimpleName() + ": " + t.getMessage());
			return o.done();
		}
	}

	/** Renders a dataset literal so the Rust side can rebuild it. */
	static String renderDataSet(String[] cols, String[][] rows) {
		JsonArr c = new JsonArr();
		for (String col : cols) {
			c.add(Json.quote(col));
		}
		JsonArr r = new JsonArr();
		for (String[] row : rows) {
			JsonArr ro = new JsonArr();
			for (String v : row) {
				ro.add(Json.quote(v));
			}
			r.add(ro.done());
		}
		JsonObj o = new JsonObj();
		o.put("columns", c);
		o.put("rows", r);
		o.put("row_count", rows.length);
		return o.done();
	}

	// =================================================================
	// Value typing
	// =================================================================

	static String value(Object o) {
		if (o == null) {
			return "null";
		}
		if (o instanceof String) {
			return Json.quote((String) o);
		}
		if (o instanceof Boolean) {
			return o.toString();
		}
		if (o instanceof Integer || o instanceof Long || o instanceof Short
				|| o instanceof Byte) {
			return o.toString();
		}
		if (o instanceof BigDecimal) {
			return o.toString();
		}
		if (o instanceof Double || o instanceof Float) {
			double d = ((Number) o).doubleValue();
			if (Double.isNaN(d) || Double.isInfinite(d)) {
				return Json.quote(String.valueOf(d));
			}
			if (d == Math.floor(d) && Math.abs(d) < 1e15) {
				return Long.toString((long) d);
			}
			return o.toString();
		}
		if (o instanceof List) {
			JsonArr a = new JsonArr();
			for (Object e : (List<?>) o) {
				a.add(value(e));
			}
			return a.done();
		}
		return Json.quote(o.toString());
	}

	static String typeName(Object o) {
		if (o == null) {
			return "null";
		}
		if (o instanceof String) {
			return "string";
		}
		if (o instanceof Boolean) {
			return "boolean";
		}
		if (o instanceof Integer) {
			return "int";
		}
		if (o instanceof Long) {
			return "long";
		}
		if (o instanceof BigDecimal) {
			return "decimal";
		}
		if (o instanceof Double) {
			return "double";
		}
		if (o instanceof Float) {
			return "float";
		}
		if (o instanceof List) {
			return "list";
		}
		return o.getClass().getSimpleName();
	}
}