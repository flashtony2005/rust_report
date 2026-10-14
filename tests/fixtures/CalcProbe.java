/*
 * CalcProbe.java -- shared helpers for the headless calculation probes.
 *
 * Everything the probes share lives here: report/dataset builders, the
 * calculated-grid dumper and the pagination dumper. Keeping them out of the
 * probe entry points is what lets each file stay under the 300-line gate.
 *
 * Ground-truth rules discovered while building this (see RUNLOG §10):
 *  - a cell expression lives in expMap slot 40 (AtomicCell.setExp)
 *  - only a DS FUNCTION returning a List extends a cell, i.e. ds.select(col)
 *    NOT ds.col -- the bare field is a DSVariable returning one value
 *  - "$B2" is absolute-column/relative-row; "$B$2" is NOT a valid cell id
 */
package rpx.fixtures;

import com.raqsoft.reportl.dataset.DataSet;
import com.raqsoft.reportl.dataset.Row;
import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.Context;
import com.raqsoft.reportl.usermodel.DataSetConfig;
import com.raqsoft.reportl.usermodel.DataSetMetaData;
import com.raqsoft.reportl.usermodel.Engine;
import com.raqsoft.reportl.usermodel.FileDataSetConfig;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.usermodel.PagerInfo;
import com.raqsoft.reportl.usermodel.PageBuilder;
import com.raqsoft.reportl.util.ReportUtils2;
import com.scudata.common.ByteMap;
import com.scudata.common.IByteMap;

import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.Writer;

final class CalcProbe {

	private CalcProbe() {
	}

	// =================================================================
	// Model construction
	// =================================================================

	/** Expression cells live in expMap slot 40 (see AtomicCell/JPanelDataSet). */
	static void setExp(INormalCell cell, String exp) {
		IByteMap map = cell.getExpMap(true);
		if (map == null) {
			map = new ByteMap((short) 4);
		}
		map.put((byte) 40, exp);
		cell.setExpMap(map);
	}

	static String expOf(INormalCell cell) {
		IByteMap map = cell == null ? null : cell.getExpMap(false);
		Object v = map == null ? null : map.get((byte) 40);
		return v == null ? null : v.toString();
	}

	/**
	 * Adds a FileDataSetConfig (the real product dataset channel). Appends
	 * to an existing DataSetMetaData so a report can carry several datasets.
	 */
	static void attachFileDataSet(ReportDefine2 rd, String dsName, String fileName) {
		DataSetMetaData dsmd = rd.getDataSetMetaData();
		if (dsmd == null) {
			dsmd = new DataSetMetaData();
			rd.setDataSetMetaData(dsmd);
		}
		FileDataSetConfig ds = new FileDataSetConfig();
		ds.setName(dsName);
		ds.setFileName(fileName);
		ds.setDataSourceName(dsName);
		dsmd.addDataSetConfig(ds);
	}

	static void attachDataSetConfig(ReportDefine2 rd, DataSetConfig dsc) {
		DataSetMetaData dsmd = new DataSetMetaData();
		dsmd.addDataSetConfig(dsc);
		rd.setDataSetMetaData(dsmd);
	}

	/**
	 * In-memory DataSet. NOTE: addCol, not setColNames -- the col-info list
	 * carries a phantom slot at index 0 and setColNames wipes it, which
	 * silently corrupts getColNo (leads to ArrayIndexOutOfBounds on setData).
	 */
	static DataSet memDataSet(String name, String[] regions, int[] qtys) {
		DataSet ds = new DataSet(name);
		ds.addCol("region");
		ds.addCol("qty");
		for (int i = 0; i < regions.length; i++) {
			Row row = ds.addRow();
			row.setData("region", regions[i]);
			row.setData("qty", Integer.valueOf(qtys[i]));
		}
		return ds;
	}

	/** The canonical 3-row / 3-column dataset used by the probes. */
	static DataSet stdDataSet() {
		return memDataSet("ds1",
				new String[] { "North", "East", "South" },
				new int[] { 100, 200, 150 });
	}

	/** A dataset of n synthetic rows: region R1..Rn, qty 10,20,...,10n. */
	static DataSet tallDataSet(String name, int n) {
		String[] regions = new String[n];
		int[] qtys = new int[n];
		for (int i = 0; i < n; i++) {
			regions[i] = "R" + (i + 1);
			qtys[i] = (i + 1) * 10;
		}
		return memDataSet(name, regions, qtys);
	}

	// =================================================================
	// Calculation
	// =================================================================

	/** Runs the product engine headlessly. ds may be null (no dataset). */
	static IReport calc(ReportDefine2 rd, DataSet ds) {
		return calc(rd, ds, null);
	}

	/**
	 * Runs the product engine headlessly with report parameters injected into
	 * the Context (M3-01: the caller owns parameter injection -- the engine
	 * itself never registers defaults, see CalcReport.java). null = none.
	 */
	static IReport calc(ReportDefine2 rd, DataSet ds,
			java.util.Map<String, Object> params) {
		Context ctx = new Context();
		if (ds != null) {
			ctx.getDataSetMap(true).put(ds.getDataSetName(), ds);
		}
		if (params != null) {
			for (java.util.Map.Entry<String, Object> e : params.entrySet()) {
				ctx.setParamValue(e.getKey(), e.getValue());
			}
		}
		return new Engine(rd, ctx).calc();
	}

	// =================================================================
	// Dumpers
	// =================================================================

	static void dump(String tag, IReport rpt) {
		dump(tag, rpt, false);
	}

	/**
	 * Dumps the calculated grid: computed value, display value (when
	 * different) and the extend structure (source cell, row/col merge,
	 * sub-cells). With all=true every cell is printed, nulls included.
	 *
	 * Bound note: getRowCount()/getColCount() are DATA counts (matrix size
	 * minus the header row/col), so cells live at rows 1..rows, cols 1..cols.
	 */
	static void dump(String tag, IReport rpt, boolean all) {
		if (rpt == null) {
			System.out.println("[" + tag + "] calc returned null");
			return;
		}
		if (!(rpt instanceof ExtCellSet)) {
			System.out.println("[" + tag + "] calc returned " + rpt.getClass().getName()
					+ " (not ExtCellSet)");
			return;
		}
		ExtCellSet ecs = (ExtCellSet) rpt;
		int rows = ecs.getRowCount();
		int cols = ecs.getColCount();
		System.out.println("[" + tag + "] calculated grid: " + rows + " rows x "
				+ cols + " cols");
		for (int r = 1; r <= rows; r++) {
			for (int c = 1; c <= cols; c++) {
				INormalCell cell = ecs.getCell(r, c);
				if (cell == null) {
					continue;
				}
				Object v = cell.getValue();
				String dv = cell.getDispValue();
				if (v == null && dv == null && !all) {
					continue;
				}
				StringBuilder sb = new StringBuilder();
				sb.append("[").append(tag).append("] (").append(r).append(",")
						.append(c).append(") value=").append(repr(v));
				if (dv != null && !dv.equals(String.valueOf(v))) {
					sb.append(" disp=").append(repr(dv));
				}
				ExtCell src = ecs.getSource(r, c);
				if (src != null) {
					sb.append(" src=(").append(src.getSourceRow()).append(",")
							.append(src.getSourceCol()).append(") rowMerge=")
							.append(src.getRowMerge()).append(" colMerge=")
							.append(src.getColMerge()).append(" extend=")
							.append(src.getExtensible());
					int subs = src.extCellSize();
					if (subs > 0) {
						sb.append(" subCells=").append(subs);
						for (int i = 0; i < subs; i++) {
							ExtCell sub = src.getExtCell(i);
							sb.append(" {(").append(sub.getRow()).append(",")
									.append(sub.getCol()).append(") ")
									.append(repr(sub.getValue(false))).append("}");
						}
					}
				}
				System.out.println(sb.toString());
			}
		}
	}

	/**
	 * Pagination ground truth: page count plus, for every page, its row
	 * count and the first/last non-empty cell value of column 1.
	 */
	static void dumpPages(String tag, IReport rpt, float w, float h, int cols)
			throws Throwable {
		PagerInfo pi = ReportUtils2.getPagerInfo(rpt, (int) w, (int) h, cols);
		PageBuilder pb = new PageBuilder(rpt, pi);
		pb.createPages();
		System.out.println("[" + tag + "] pageCount=" + pb.getPageCount()
				+ " xPages=" + pb.getXPageCount() + " yPages=" + pb.getYPageCount()
				+ " rowNumPerPage=" + pi.getRowNumPerPage());
		IReport[] pages = pb.getAllPages();
		for (int i = 0; i < pages.length; i++) {
			IReport pg = pages[i];
			int n = pg.getRowCount();
			StringBuilder sb = new StringBuilder();
			sb.append("[").append(tag).append("] page ").append(i + 1)
					.append(": rows=").append(n).append(" | ");
			for (int r = 1; r <= n; r++) {
				INormalCell c = pg.getCell(r, 1);
				Object v = c == null ? null : c.getValue();
				if (v == null) {
					continue;
				}
				sb.append("r").append(r).append("=").append(repr(v)).append(" ");
			}
			System.out.println(sb.toString());
		}
	}

	// =================================================================
	// Misc
	// =================================================================

	static String repr(Object o) {
		if (o == null) {
			return "null";
		}
		if (o instanceof String) {
			return "\"" + o + "\"";
		}
		return o.getClass().getSimpleName() + "(" + o + ")";
	}

	static int fail(String tag, Throwable t) {
		System.out.println("[" + tag + "] FAIL: " + t.getClass().getName() + ": "
				+ t.getMessage());
		StackTraceElement[] st = t.getStackTrace();
		for (int i = 0; i < st.length && i < 6; i++) {
			System.out.println("[" + tag + "]   at " + st[i]);
		}
		return 1;
	}

	/** Tab-separated, GBK, first row = column names (FileDataSetFactory). */
	static void writeCsv(String path, String[] header, String[][] rows) throws Exception {
		Writer w = new OutputStreamWriter(new FileOutputStream(path), "GBK");
		try {
			StringBuilder sb = new StringBuilder();
			for (int i = 0; i < header.length; i++) {
				if (i > 0) {
					sb.append('\t');
				}
				sb.append(header[i]);
			}
			w.write(sb + "\r\n");
			for (String[] row : rows) {
				sb = new StringBuilder();
				for (int i = 0; i < row.length; i++) {
					if (i > 0) {
						sb.append('\t');
					}
					sb.append(row[i]);
				}
				w.write(sb + "\r\n");
			}
		} finally {
			w.close();
		}
	}
}