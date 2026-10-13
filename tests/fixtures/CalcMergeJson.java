/*
 * CalcMergeJson.java -- the merged-cell dump, split out of CalcJson so both
 * files stay under the 300-line gate.
 *
 * This is the only place mergeSame can be observed: ExtCellSet.mergeSame()
 * is called from SimplePager (:133,:179) and CalcSubRpt, and nowhere in
 * Engine.calc() -- verified by grepping the whole decompiled tree. So the
 * dump must run the pager first and then read the page grid.
 *
 * The same key also carries template merges that survived extension, so one
 * dump covers M2-04 (merged area vs extension) and M2-08 (mergeSame).
 * Cells with row_merge==1 and col_merge==1 are omitted, so a fixture without
 * merges gets {"page_count":n,"cells":[]}.
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.engine.ExtNormalCell;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.usermodel.PageBuilder;
import com.raqsoft.reportl.usermodel.PagerInfo;
import com.raqsoft.reportl.util.ReportUtils2;

final class CalcMergeJson {

	private CalcMergeJson() {
	}

	/** Merged cells across every page produced by SimplePager. */
	static String renderMerged(IReport rpt, float w, float h, int cols) {
		try {
			PagerInfo pi = ReportUtils2.getPagerInfo(rpt, (int) w, (int) h, cols);
			PageBuilder pb = new PageBuilder(rpt, pi);
			pb.createPages();
			IReport[] all = pb.getAllPages();
			JsonArr cells = new JsonArr();
			for (int p = 0; p < all.length; p++) {
				IReport pg = all[p];
				for (int r = 1; r <= pg.getRowCount(); r++) {
					for (int c = 1; c <= pg.getColCount(); c++) {
						INormalCell cell = pg.getCell(r, c);
						if (cell == null) {
							continue;
						}
						ExtNormalCell ec = (ExtNormalCell) cell;
						int rm = ec.getRowMerge();
						int cm = ec.getColMerge();
						if (rm <= 1 && cm <= 1) {
							continue;
						}
						JsonObj o = new JsonObj();
						o.put("page", p + 1);
						o.put("r", r);
						o.put("c", c);
						o.put("value", CalcJson.value(cell.getValue()));
						o.put("row_merge", rm);
						o.put("col_merge", cm);
						cells.add(o);
					}
				}
			}
			JsonObj o = new JsonObj();
			o.put("page_count", pb.getPageCount());
			o.put("cells", cells);
			return o.done();
		} catch (Throwable t) {
			JsonObj o = new JsonObj();
			o.put("error", t.getClass().getSimpleName() + ": " + t.getMessage());
			return o.done();
		}
	}
}
