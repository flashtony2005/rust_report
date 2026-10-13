package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;

/**
 * ProbeMergeAlias -- settles, by object identity, which calculated cells are
 * merged-area ALIASES of their anchor and which are independent cells.
 * Ground truth for the Rust engine's export loop (engine_merge mismatch).
 */
public class ProbeMergeAlias {

	public static void main(String[] args) {
		ReportDefine2 rd = EngineModelsP0.mergeFixture();
		IReport rpt = CalcProbe.calc(rd, null);
		if (!(rpt instanceof ExtCellSet)) {
			System.out.println("calc failed: " + rpt);
			return;
		}
		ExtCellSet ecs = (ExtCellSet) rpt;
		System.out.println("rows=" + ecs.getRowCount() + " cols=" + ecs.getColCount());
		for (int r = 1; r <= ecs.getRowCount(); r++) {
			for (int c = 1; c <= ecs.getColCount(); c++) {
				INormalCell cell = ecs.getCell(r, c);
				if (cell == null) {
					continue;
				}
				Object v = cell.getValue();
				if (v == null && cell.getDispValue() == null) {
					continue;
				}
				System.out.println("(" + r + "," + c + ") " + id(cell, ecs));
			}
		}
		System.out.println("--- identity checks ---");
		same(ecs, "(2,1)~(3,1)", 2, 1, 3, 1);
		same(ecs, "(2,1)~(4,1)", 2, 1, 4, 1);
		same(ecs, "(2,3)~(2,4)", 2, 3, 2, 4);
		same(ecs, "(4,1)~(5,1)", 4, 1, 5, 1);
		same(ecs, "(4,3)~(4,4)", 4, 3, 4, 4);
		same(ecs, "(6,3)~(6,4)", 6, 3, 6, 4);
		same(ecs, "(4,1)~(6,1)", 4, 1, 6, 1);
	}

	static void same(ExtCellSet ecs, String tag, int r1, int c1, int r2, int c2) {
		INormalCell a = ecs.getCell(r1, c1);
		INormalCell b = ecs.getCell(r2, c2);
		System.out.println(tag + " same=" + (a == b));
	}

	static String id(INormalCell cell, ExtCellSet ecs) {
		if (cell == null) {
			return "null";
		}
		StringBuilder sb = new StringBuilder();
		sb.append(cell.getClass().getSimpleName());
		sb.append(" value=").append(cell.getValue());
		if (cell instanceof ExtCell) {
			ExtCell ec = (ExtCell) cell;
			sb.append(" row=").append(ec.getRow()).append(" col=").append(ec.getCol());
			sb.append(" rm=").append(ec.getRowMerge()).append(" cm=").append(ec.getColMerge());
			sb.append(" ext=").append(ec.getExtensible());
			sb.append(" src=(").append(ec.getSourceRow()).append(",").append(ec.getSourceCol()).append(")");
			ExtCell lh = ec.getCellLeftHead();
			ExtCell th = ec.getCellTopHead();
			sb.append(" lh=").append(head(lh)).append(" th=").append(head(th));
		}
		return sb.toString();
	}

	static String head(ExtCell h) {
		if (h == null) {
			return "null";
		}
		return "(" + h.getSourceRow() + "," + h.getSourceCol() + ")";
	}
}
