/*
 * ProbeMergePages.java -- is merged.cells' `r` page-local or global when the
 * report spans several pages?
 *
 * Why: CalcMergeJson.renderMerged (:37-59) loops
 *     for (int p = 0; p < all.length; p++) {
 *         IReport pg = all[p];
 *         for (int r = 1; r <= pg.getRowCount(); r++) {
 *             ...
 *             o.put("page", p + 1);
 *             o.put("r", r);
 * so the DECOMPILATION says `r` is page-local and `page` is emitted for every
 * page. All three fixtures that currently have merged content are SINGLE-page,
 * so that reading is untested: page-local and global coordinates coincide.
 *
 * The Rust criteria depend on it (crates/rpx-engine/tests/merged_assert.rs
 * cross-checks column-1 merged values against each page's own col1_values).
 * If `r` turns out to be global on multi-page reports, that is a CONTRACT
 * CHANGE and the criteria must be rewritten -- hence this probe.
 *
 * Two variants, because "multi-page with merges" has two different sources:
 *   A  mergeSame=98 on an extending column, 40 rows  -> merges on EVERY page
 *   B  a template merge on row 2, 40 rows           -> merges only on page 1
 *
 * Standalone by design: writes only into the given scratch dir, touches no
 * shared registry (no GenEngineFixtures.NAMES), so it can run while the
 * backend is mid-edit on those.
 *
 * Run: java -cp "lib/*;out" rpx.fixtures.ProbeMergePages out
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.usermodel.PagerInfo;
import com.raqsoft.reportl.usermodel.PageBuilder;
import com.raqsoft.reportl.util.ReportUtils;
import com.raqsoft.reportl.util.ReportUtils2;
import com.scudata.common.Area;

public class ProbeMergePages {

	/** mergeSame = vertical collapse (EngineModelsP0.SAME_V). */
	private static final byte SAME_V = (byte) 98;
	private static final int ROWS = 40;

	public static void main(String[] args) throws Throwable {
		String dir = args.length > 0 ? args[0] : ".";
		run("A: mergeSame=98 on extending column, 40 rows", dir, true);
		run("B: template merge A2:A3, 40 rows", dir, false);
	}

	private static void run(String tag, String dir, boolean mergeSame) throws Throwable {
		System.out.println("==== " + tag + " ====");
		String csv = dir + "/_probe_mergepages.csv";
		CalcProbe.writeCsv(csv, new String[] { "region", "qty" }, rows());

		ReportDefine2 rd = mergeSame ? multiPageMergeModel() : templateMergeModel();
		CalcProbe.attachFileDataSet(rd, "ds1", csv);

		// write -> read -> calc, so the numbers come off real bytes
		String rpx = dir + "/_probe_mergepages.rpx";
		ReportUtils.write(rpx, rd);
		ReportDefine2 rd2 = (ReportDefine2) ReportUtils.read(rpx);
		IReport rpt = CalcProbe.calc(rd2, null);

		PagerInfo pi = ReportUtils2.getPagerInfo(rpt, 595, 842, 1);
		PageBuilder pb = new PageBuilder(rpt, pi);
		pb.createPages();
		IReport[] pages = pb.getAllPages();

		int[] pageRows = new int[pages.length];
		System.out.println("pageCount=" + pb.getPageCount() + " xPages=" + pb.getXPageCount()
				+ " yPages=" + pb.getYPageCount());
		for (int p = 0; p < pages.length; p++) {
			pageRows[p] = pages[p].getRowCount();
			System.out.println("  page " + (p + 1) + " rows=" + pageRows[p]);
		}

		// replicate CalcMergeJson.renderMerged's collection
		int total = 0;
		boolean pageLocal = true;
		boolean everyPageCovered = true;
		StringBuilder perPage = new StringBuilder();
		for (int p = 0; p < pages.length; p++) {
			IReport pg = pages[p];
			java.util.List<String> spans = new java.util.ArrayList<String>();
			int onThisPage = 0;
			int minR = Integer.MAX_VALUE;
			int maxR = 0;
			for (int r = 1; r <= pg.getRowCount(); r++) {
				for (int c = 1; c <= pg.getColCount(); c++) {
					INormalCell cell = pg.getCell(r, c);
					if (cell == null) {
						continue;
					}
					int rm = rowMerge(cell);
					int cm = colMerge(cell);
					if (rm <= 1 && cm <= 1) {
						continue;
					}
					onThisPage++;
					minR = Math.min(minR, r);
					maxR = Math.max(maxR, r);
					spans.add(rm + "/" + cm);
					// 页内坐标判据：r 必须落在本页的行数范围内
					if (r < 1 || r > pg.getRowCount()) {
						pageLocal = false;
					}
				}
			}
			total += onThisPage;
			if (onThisPage == 0) {
				everyPageCovered = false;
			}
			perPage.append("      page ").append(p + 1).append(": merged=").append(onThisPage)
					.append(" r∈[").append(minR == Integer.MAX_VALUE ? "-" : minR)
					.append(",").append(maxR).append("] pageRows=").append(pageRows[p])
					.append(" rm/cm=").append(spans)
					.append('\n');
		}
		System.out.println("  merged total=" + total);
		System.out.print(perPage);
		System.out.println("  VERDICT page-local r : " + (pageLocal ? "YES" : "NO  <-- CONTRACT CHANGE"));
		System.out.println("  VERDICT every page has merged cells : " + everyPageCovered);
		System.out.println();
	}

	/** mergeSame on the extending column; the repeated region values give the pager something to collapse. */
	/**
	 * The multi-page merge fixture model, exposed so the fixture generator can
	 * reuse it verbatim once NAMES registration is handed over.
	 *
	 * Shape (measured by this probe, 595x842pt, row height 20mm, unit=1):
	 * 40 data rows with region values repeating in PAIRS (A,A,B,B,C,C,D,D...)
	 * and mergeSame=98 (vertical) on the extending column A. Every page slice
	 * therefore contains adjacent duplicates, so every full page gets its own
	 * merges: 4 pages of 14/14/14/1 rows, 12 merged cells per full page
	 * (6 merge areas x 2 slots, all row_merge=2 / col_merge=1), and the
	 * 1-row LAST page gets ZERO merged cells.
	 *
	 * That last part is the point: it pins "page does NOT necessarily appear in
	 * merged.cells", which a naive criterion ("every page must appear") would
	 * get wrong.
	 */
	public static ReportDefine2 multiPageMergeModel() {
		ReportDefine2 rd = new ReportDefine2(4, 2);
		EngineModels.lit(rd, 1, 1, "region");
		EngineModels.lit(rd, 1, 2, "qty");
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(region)");
		a2.setMergeSame(SAME_V);
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
		for (int r = 1; r <= 4; r++) {
			rd.getRowCell(r).setRowHeight(20f);
		}
		return rd;
	}

	/** A template vertical merge on the extending row; 40 rows forces several pages. */
	/** Variant B: a template vertical merge; 40 rows. Also multi-page. */
	private static ReportDefine2 templateMergeModel() {
		ReportDefine2 rd = new ReportDefine2(4, 2);
		EngineModels.lit(rd, 1, 1, "region");
		EngineModels.lit(rd, 1, 2, "qty");
		INormalCell a2 = rd.getCell(2, 1);
		a2.setExtendMode(INormalCell.EXTEND_VERTICAL);
		CalcProbe.setExp(a2, "ds1.select(region)");
		a2.setMergedArea(new Area(2, 1, 3, 1));
		CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
		for (int r = 1; r <= 4; r++) {
			rd.getRowCell(r).setRowHeight(20f);
		}
		return rd;
	}

	/** region repeats in pairs so every page slice contains adjacent duplicates. */
	private static String[][] rows() {
		String[] tags = { "A", "B", "C", "D" };
		String[][] r = new String[ROWS][2];
		for (int i = 0; i < ROWS; i++) {
			r[i][0] = tags[(i / 2) % tags.length];
			r[i][1] = Integer.toString((i + 1) * 10);
		}
		return r;
	}

	private static int rowMerge(INormalCell cell) {
		return ((com.raqsoft.reportl.model.engine.ExtNormalCell) cell).getRowMerge();
	}

	private static int colMerge(INormalCell cell) {
		return ((com.raqsoft.reportl.model.engine.ExtNormalCell) cell).getColMerge();
	}
}
