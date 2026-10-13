/*
 * GenRealMerge.java -- the "real merge" .rpx fixture (M1 gap, M2 prerequisite).
 *
 * Why this fixture exists
 * ----------------------
 * The M1 fixtures (simple/merged/full) only ever called setMergedArea on the
 * ANCHOR cell. In ReportDefine.serialize() a non-anchor slot is written as
 * writeRecord(null) -- i.e. int32 -1, four bytes FF FF FF FF -- only when that
 * slot's OWN cell reports isMerged() == true AND is not the anchor. With the
 * anchor-only approach every other slot still holds a plain default NormalCell,
 * so it serialises as a full default record and the -1 branch is never taken.
 *
 * The product's real merge operation is AtomicReport case 23
 * (02-ide-core/com/raqsoft/reportl/ide/AtomicReport.java:509-556):
 *
 *     INormalCell firstCell = (INormalCell) report.getCell(firstPos).deepClone();
 *     firstCell.setMergedArea(rect.getArea());
 *     for (r = beginRow..endRow) for (c = beginCol..endCol)
 *         report.setCell(r, c, firstCell);      // SAME object in every slot
 *
 * Because one shared object lands in every slot of the rectangle, every slot
 * reports isMerged() == true and only the anchor's coordinates match the area's
 * begin, so the writer emits -1 for all the others. That is the byte shape the
 * rpx-core parser's empty-slot branch needs to be exercised against.
 *
 * This generator reproduces that exact call sequence using only model-level
 * public API -- no bytes are handcrafted.
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.util.ReportUtils;
import com.scudata.common.Area;

import java.io.File;

/** Builds realmerge.rpx plus its expected.json, then proves the round trip. */
public class GenRealMerge {

	public static void main(String[] args) throws Exception {
		String dir = args.length > 0 ? args[0] : ".";
		ReportDefine2 rd = build();

		String rpxPath = dir + File.separator + "realmerge.rpx";
		String jsonPath = dir + File.separator + "realmerge.expected.json";
		ReportUtils.write(rpxPath, rd);
		System.out.println("wrote " + rpxPath + " (" + new File(rpxPath).length() + " bytes)");

		String expected = ExpectedJson.render(rd).trim();
		writeFile(jsonPath, expected + "\n");
		System.out.println("wrote " + jsonPath);

		IReport read = ReportUtils.read(rpxPath);
		String actual = ExpectedJson.render((ReportDefine2) read).trim();
		if (!expected.equals(actual)) {
			System.out.println("FAIL round-trip mismatch");
			System.out.println("  built  : " + expected);
			System.out.println("  re-read: " + actual);
			System.exit(1);
		}
		System.out.println("PASS round-trip: realmerge.rpx -> reader -> identical expected.json");
	}

	private static void writeFile(String path, String content) throws Exception {
		java.io.Writer w = new java.io.OutputStreamWriter(
				new java.io.FileOutputStream(path), "UTF-8");
		try {
			w.write(content);
		} finally {
			w.close();
		}
	}

	/** 7x5 report with one 1x3, one 2x2 and one 3x1 real merge. */
	private static ReportDefine2 build() {
		ReportDefine2 rd = new ReportDefine2(7, 5);

		// ---- anchors (values must be set BEFORE merging: the merge clones) ----
		INormalCell c = rd.getCell(1, 1);
		c.setValue("WideTitle");
		c.setFontName("Arial");
		c.setFontSize((short) 14);
		c.setBold(true);
		c.setHAlign(INormalCell.HALIGN_CENTER);

		c = rd.getCell(3, 2);
		c.setValue("Block");
		c.setHAlign(INormalCell.HALIGN_CENTER);
		c.setVAlign(INormalCell.VALIGN_MIDDLE);

		c = rd.getCell(2, 4);
		c.setValue("Side");
		c.setVAlign(INormalCell.VALIGN_MIDDLE);

		// ---- the three merges, via the product's own call sequence ----
		mergeRect(rd, 1, 1, 1, 3); // 1x3 horizontal
		mergeRect(rd, 3, 2, 4, 3); // 2x2 block
		mergeRect(rd, 2, 4, 4, 4); // 3x1 vertical

		// ---- plain cells outside every merge ----
		c = rd.getCell(2, 1);
		c.setValue("r2c1");
		c = rd.getCell(2, 2);
		c.setValue("r2c2");
		c = rd.getCell(2, 3);
		c.setValue("r2c3");

		c = rd.getCell(5, 1);
		c.setValue("r5c1");
		c = rd.getCell(5, 2);
		c.setValue("r5c2");
		c = rd.getCell(5, 3);
		c.setValue("r5c3");
		c = rd.getCell(5, 4);
		c.setValue("r5c4");

		c = rd.getCell(6, 1);
		c.setValue("r6c1");
		c = rd.getCell(6, 2);
		c.setValue(Integer.valueOf(42));
		c = rd.getCell(7, 1);
		c.setValue("r7c1");

		rd.getRowCell(1).setRowHeight(24f);
		rd.getRowCell(3).setRowHeight(20f);
		rd.getRowCell(5).setRowHeight(18f);
		rd.getRowCell(7).setRowHeight(22f);
		rd.getColCell(1).setColWidth(80f);
		rd.getColCell(2).setColWidth(60f);
		rd.getColCell(3).setColWidth(60f);
		rd.getColCell(4).setColWidth(40f);
		rd.getColCell(5).setColWidth(50f);
		return rd;
	}

	/**
	 * Mirrors AtomicReport.java:532-542 -- one shared cloned object is stored
	 * into every slot of the rectangle, which is what makes the writer emit
	 * int32 -1 for the non-anchor slots.
	 */
	private static void mergeRect(ReportDefine2 rd, int br, int bc, int er, int ec) {
		INormalCell anchor = (INormalCell) rd.getCell(br, bc).deepClone();
		Area area = new Area();
		area.setArea(br, bc, er, ec);
		anchor.setMergedArea(area);
		for (int r = br; r <= er; r++) {
			for (int c = bc; c <= ec; c++) {
				rd.setCell(r, c, anchor);
			}
		}
	}
}
