/*
 * GenEngineFixtures.java -- M2 engine fixtures, produced by the product.
 *
 * M1 fixtures pin the STRUCTURAL contract (what rpx-core parses). These
 * fixtures pin the ENGINE contract (what rpx-engine must compute): the
 * calculated grid, the extend structure, the pagination split and the
 * merged cells, all dumped from the product's own ExtCellSet.
 *
 * Every .rpx is written by com.raqsoft.reportl.util.ReportUtils.write -- no
 * hand-rolled bytes. Each fixture ships the CSV its FileDataSetConfig points
 * at, so the calculation is reproducible from the fixtures alone.
 *
 *   engine_sum        sum()/aggregate forms; two datasets, one referenced by
 *                     the extension and one not, so the two scopes differ
 *   engine_ext        extendMode 17/18/19, one per row so each owns its own
 *                     envelope (17 -> ArrayList, 18 -> down, 19 -> across)
 *   engine_expr       arithmetic + relative/absolute cell references
 *   engine_pager      40 data rows -> multi-page, plus a -90 group-header row
 *   engine_merge      M2-04 merged area crossing the extension
 *   engine_cycle      M2-07 reference cycle -> ReportError (no grid)
 *   engine_samemerge  M2-08 mergeSame=98 collapsing equal adjacent values
 *   engine_disp       M2-14 dispValue: survives calculation, keeps a null-valued
 *                     cell alive, and is inherited by every sub-cell
 *
 * Compile:
 *   javac --release 8 -cp "F:/project/report/reportlite/lib/*" *.java -d out
 * Run:
 *   java -cp "...;out" rpx.fixtures.GenEngineFixtures <outDir>
 *   java -cp "...;out" rpx.fixtures.GenEngineFixtures --verify <outDir>
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.usermodel.IRowCell;
import com.raqsoft.reportl.util.ReportUtils;

import java.io.File;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class GenEngineFixtures {

	private static final String[] NAMES = { "engine_sum", "engine_ext",
			"engine_expr", "engine_pager", "engine_merge", "engine_cycle",
			"engine_samemerge", "engine_disp", "engine_types",
			"engine_rowheight", "engine_head_top", "engine_head_left",
			"engine_avgrange", "engine_collate", "engine_listrange",
			"engine_param" };

	public static void main(String[] args) throws Exception {
		String dir = args.length > 0 ? args[0] : ".";
		boolean verify = "--verify".equals(args[0]);
		if (verify) {
			dir = args.length > 1 ? args[1] : ".";
		}
		int bad = 0;
		for (String name : NAMES) {
			bad += verify ? verifyOne(dir, name) : gen(dir, name);
		}
		System.out.println(bad == 0
				? (verify ? "VERIFY OK: " : "OK: ") + NAMES.length
						+ " engine fixtures generated"
				: "FAILED: " + bad + " differ");
		if (bad != 0) {
			System.exit(1);
		}
	}

	private static int gen(String dir, String name) throws Exception {
		ReportDefine2 rd = EngineModels.build(name);
		String csv = EngineModels.csvName(name);
		writeCsv(dir, csv, EngineModels.rows(name));

		String rpx = dir + File.separator + name + ".rpx";
		ReportUtils.write(rpx, rd);
		// calculate through the real FileDataSetFactory, reading the CSV we
		// just wrote -- no in-memory shortcut, so the fixture is the input.
		ExtCellSet ecs = null;
		Throwable err = null;
		try {
			// M3-01: parameter injection is the CALLER's job (the engine never
			// registers defaults -- CalcReport.java); engine_param is the only
			// fixture with parameters, the rest pass null.
			ecs = (ExtCellSet) CalcProbe.calc(rd, null, EngineModelsM3.params(name));
		} catch (Throwable t) {
			err = t;
		}
		String json = render(name, csv, rd, ecs, err);

		FileIO.write(dir + File.separator + name + ".expected.json", json + "\n");
		System.out.println("wrote " + rpx + " (" + new File(rpx).length() + " bytes)");
		System.out.println("wrote " + csv + " + " + name + ".expected.json"
				+ (err != null ? " (expected failure: " + err.getClass()
						.getSimpleName() + ")"
						: " (calc grid " + ecs.getRowCount() + "x"
								+ ecs.getColCount() + ")"));
		return 0;
	}

	/** Round-trip gate: re-read the binary, re-calculate, re-render, compare. */
	private static int verifyOne(String dir, String name) throws Exception {
		String rpx = dir + File.separator + name + ".rpx";
		ReportDefine2 rd = (ReportDefine2) ReportUtils.read(rpx);
		ExtCellSet ecs = null;
		Throwable err = null;
		try {
			ecs = (ExtCellSet) CalcProbe.calc(rd, null, EngineModelsM3.params(name));
		} catch (Throwable t) {
			err = t;
		}
		String actual = render(name, EngineModels.csvName(name), rd, ecs, err);
		String expected = FileIO.read(dir + File.separator + name
				+ ".expected.json").trim();
		if (!expected.equals(actual.trim())) {
			System.out.println("FAIL " + name + ": expected.json does not match the"
					+ " binary + calculation");
			return 1;
		}
		System.out.println("PASS " + name + ": " + new File(rpx).length()
				+ " bytes round-trip to expected.json");
		return 0;
	}

	// =================================================================
	// expected.json
	// =================================================================

	private static String render(String name, String csv, ReportDefine2 rd,
			ExtCellSet ecs, Throwable err) {
		JsonArr formulas = new JsonArr();
		for (int r = 1; r <= rd.getRowCount(); r++) {
			for (int c = 1; c <= rd.getColCount(); c++) {
				INormalCell cell = rd.getCell(r, c);
				String exp = CalcProbe.expOf(cell);
				if (exp == null) {
					continue;
				}
				JsonObj o = new JsonObj();
				o.put("r", r);
				o.put("c", c);
				o.putString("exp", exp);
				if (cell.getExtendMode() != 16) {
					o.put("extend_mode", cell.getExtendMode());
				}
				if (cell.getMergeSame() != 97) {
					o.put("merge_same", cell.getMergeSame());
				}
				formulas.add(o);
			}
		}
		JsonArr rowTypes = new JsonArr();
		boolean wroteAll = false;
		for (int r = 1; r <= rd.getRowCount(); r++) {
			IRowCell rc = rd.getRowCell(r);
			JsonObj o = new JsonObj();
			o.put("r", r);
			o.put("row_type", rc.getRowType());
			o.put("group_header_level", rc.getGroupHeaderLevel());
			o.put("row_height", rc.getRowHeight());
			// ---- 行高属性表达式（M2-22）----
			// `row_height` 始终是**模板**值；计算值另存 `computed_row_height`，
			// 二者不同即证明 `calcProperties` 真的跑过。**只在带表达式的行写出**，
			// 这样其余夹具的 expected.json 保持逐字节不变。
			// 覆盖的通道 = 内置行属性通道（ExtRowCell 的 prop expMap），key 1；
			// ⛔ customPropEngMap 那条通道（ExtRowCell.java:93-95/:406-437，取值走
			// getSingleValue）**未覆盖**，需要时另造夹具。
			String hExp = rc.getRowHeightExp();
			if (hExp != null && hExp.length() > 0) {
				o.putString("height_exp", hExp);
				o.putString("exp_channel", "row-builtin-key1");
				// ⚠ **不能拿模板行号去索引计算态网格** —— 扩展会把行号整体下移。
				//   第一版就踩了这个坑：三行的 computed 全读成 40（模板行 2 的值）。
				// 正确做法：用每个计算态格的 `source.src_row` 反查它来自哪个模板行。
				if (!wroteAll) {
					// 只写一次：把**全部计算态行高按行序**倒出来。
					// ⛔ 不做「模板行 → 计算态行」的映射：扩展插入的行其 `source.src_row`
					//    报的是**自己的**行号（engine_sum 里 (3,1) 的 src_row 就是 3），
					//    所以按 src_row 反查模板行是错的（第一版就踩了）。
					//    逐行数组不需要任何映射，对 M2 的分页实现反而更直接。
					o.put("computed_anchor_row", r);
					o.put("computed_row_heights", computedHeights(ecs));
					// ⛔ **显式**列出本夹具**没有**覆盖的属性通道。
					// 只有 `exp_channel`（正向标记）是不够的：读的人无法从"覆盖了哪条"
					// 推出"产品还有哪条没测"。这句话必须是**机器可读**的，否则边界只活在文档里。
					// 为什么不放顶层：顶层是 loader 的 12 键白名单（common/loader.rs:54-69），
					// 多写一个键会让 **所有** 夹具的 load 直接 panic；`row_cells[]` 的
					// 未建模键会被收进 `RowCellInfo.extra`，所以放这里既安全又可断言。
					o.put("not_covered", NOT_COVERED_ROW_CHANNEL);
					wroteAll = true;
				}
			}
			rowTypes.add(o);
		}
		JsonObj o = new JsonObj();
		o.putString("name", name);
		o.put("unit", rd.getUnit());
		o.putString("unit_note", "1=mm 0=inch 2=point; row/column sizes use this unit");
		o.putString("dataset_file", csv);
		// M3-01 参数段：**只对 engine_param 写**。其余夹具没有这个键 ——
		// 顶层键集合是 loader 的白名单（`common/loader.rs` 的 expect_keys），
		// 无条件写 `params: []` 会让 15 个既有夹具的 expected.json 全部变动。
		if ("engine_param".equals(name)) {
			o.put("params", EngineModelsM3.paramsJson(name));
		}
		o.put("dataset", CalcJson.renderDataSet(new String[] { "region", "qty" },
				EngineModels.rows(name)));
		o.put("template_rows", rd.getRowCount());
		o.put("template_cols", rd.getColCount());
		o.put("formulas", formulas);
		o.put("row_cells", rowTypes);
		if (err != null) {
			o.put("calculated", errorJson(err));
			o.put("pagination", "null");
			o.put("merged", "null");
		} else {
			o.put("calculated", CalcJson.renderCalc(ecs));
			o.put("pagination", CalcJson.renderPages(ecs, 595, 842, 1));
			o.put("merged", CalcMergeJson.renderMerged(ecs, 595, 842, 1));
		}
		return o.done();
	}

	/**
	 * A circular reference is a legitimate expected outcome, so the fixture
	 * records the failure instead of a grid. chain is the cell sequence the
	 * engine prints (e.g. "B2-C2-B2-C2"), extracted so the Rust side does not
	 * have to parse a localised message.
	 */
	/**
	 * 本夹具**未覆盖**的属性通道（写进 `row_cells[].not_covered`，机器可读）。
	 *
	 * <p>`customPropEngMap` 是产品里的第三条属性通道（`ExtRowCell.java:93-95` 建表、
	 * `:406-437` 求值，取值走 `Variant2.getSingleValue` **拆 List**），与内置通道
	 * 「不拆 List + `((Number)).floatValue()`」规则不同。本夹具只碰 key 1（行高）。
	 */
	private static final String NOT_COVERED_ROW_CHANNEL =
			"[\"customPropEngMap\"]";

	/** 全部计算态行高（按计算态行序，`ecs.getRowCount()` 个）。 */
	private static String computedHeights(ExtCellSet ecs) {
		JsonArr a = new JsonArr();
		for (int r = 1; r <= ecs.getRowCount(); r++) {
			a.add(((IRowCell) ecs.getRowCell(r)).getRowHeight());
		}
		return a.done();
	}

	private static String errorJson(Throwable err) {
		Throwable cause = err.getCause();
		JsonObj e = new JsonObj();
		e.putString("class", err.getClass().getName());
		e.putString("message", err.getMessage());
		if (cause != null) {
			e.putString("cause_class", cause.getClass().getName());
			e.putString("cause_message", cause.getMessage());
		}
		String chain = chainOf(err.getMessage());
		if (chain == null && cause != null) {
			chain = chainOf(cause.getMessage());
		}
		if (chain != null) {
			e.putString("chain", chain);
		}
		JsonObj o = new JsonObj();
		o.put("error", e.done());
		return o.done();
	}

	private static final Pattern CHAIN = Pattern
			.compile("[A-Z]+[0-9]+(?:-[A-Z]+[0-9]+)+");

	private static String chainOf(String msg) {
		if (msg == null) {
			return null;
		}
		Matcher m = CHAIN.matcher(msg);
		return m.find() ? m.group() : null;
	}

	// =================================================================
	// Fixture models -- built with the product API only
	// =================================================================

	private static void writeCsv(String dir, String name, String[][] data)
			throws Exception {
		CalcProbe.writeCsv(dir + File.separator + name,
				new String[] { "region", "qty" }, data);
		// engine_sum carries a second dataset with identical content but no
		// extending cells referencing it, so the two aggregate scopes can be
		// compared side by side in one fixture.
		if ("engine_sum.csv".equals(name)) {
			CalcProbe.writeCsv(dir + File.separator + name + "2",
					new String[] { "region", "qty" }, data);
		}
	}

}
