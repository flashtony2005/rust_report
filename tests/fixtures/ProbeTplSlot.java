package rpx.fixtures;

import com.scudata.common.Area;
import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;

/** ProbeTplSlot -- what does the TEMPLATE matrix hold at merged-area
 *  continuation slots, and what does ecs.getSource() return there. */
public class ProbeTplSlot {

	public static void main(String[] args) {
		ReportDefine2 rd = EngineModelsP0.mergeFixture();
		System.out.println("--- template slots ---");
		slot(rd, 2, 1);
		slot(rd, 3, 1);
		slot(rd, 2, 3);
		slot(rd, 2, 4);
		System.out.println("--- after calc: ecs.getSource identity ---");
		IReport rpt = CalcProbe.calc(rd, null);
		ExtCellSet ecs = (ExtCellSet) rpt;
		srcSlot(ecs, 3, 1);
		srcSlot(ecs, 2, 4);
	}

	static void slot(ReportDefine2 rd, int r, int c) {
		INormalCell cell = rd.getCell(r, c);
		if (cell == null) {
			System.out.println("tpl(" + r + "," + c + ")=null");
			return;
		}
		Area ar = cell.getMergedArea();
		System.out.println("tpl(" + r + "," + c + ")=" + cell.getClass().getSimpleName()
				+ "@" + Integer.toHexString(System.identityHashCode(cell))
				+ " merged=" + cell.isMerged()
				+ " area=" + (ar == null ? "null" : ar.getBeginRow() + "," + ar.getBeginCol()
						+ "->" + ar.getEndRow() + "," + ar.getEndCol())
				+ " value=" + cell.getValue()
				+ " extMode=" + cell.getExtendMode());
	}

	static void srcSlot(ExtCellSet ecs, int r, int c) {
		ExtCell s = ecs.getSource(r, c);
		if (s == null) {
			System.out.println("src(" + r + "," + c + ")=null");
			return;
		}
		System.out.println("src(" + r + "," + c + ")=" + s.getClass().getSimpleName()
				+ "@" + Integer.toHexString(System.identityHashCode(s))
				+ " row=" + s.getRow() + " col=" + s.getCol()
				+ " src=(" + s.getSourceRow() + "," + s.getSourceCol() + ")"
				+ " rm=" + s.getRowMerge() + " cm=" + s.getColMerge()
				+ " ext=" + s.getExtensible());
	}
}
