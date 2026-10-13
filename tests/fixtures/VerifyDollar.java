package rpx.fixtures;

import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtNormalCell;
import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.usermodel.INormalCell;

/**
 * Independent audit of the `$` (isSaved) reference semantics. M2-06 evidence.
 * actually reach the calculated grid, and do the four "$"/range variants
 * really discriminate?
 */
public class VerifyDollar {

    private static ReportDefine2 listReport(int cols) {
        ReportDefine2 rd = new ReportDefine2(6, cols);
        rd.getCell(1, 1).setValue("Region");
        rd.getCell(1, 2).setValue("Qty");
        rd.getCell(1, 3).setValue("Doubled");
        rd.getCell(1, 4).setValue("Running");
        CalcProbe.setExp(rd.getCell(2, 1), "ds1.select(region)");
        CalcProbe.setExp(rd.getCell(2, 2), "ds1.select(qty)");
        CalcProbe.setExp(rd.getCell(2, 3), "B2*2");
        CalcProbe.setExp(rd.getCell(2, 4), "sum($B2:B2)");
        CalcProbe.attachFileDataSet(rd, "ds1", "data.csv");
        return rd;
    }

    private static String probe(String label, String dExp) {
        try {
            ReportDefine2 rd = listReport(4);
            CalcProbe.setExp(rd.getCell(2, 4), dExp);
            String readBack = CalcProbe.expOf(rd.getCell(2, 4));
            IReport rpt = CalcProbe.calc(rd, CalcProbe.stdDataSet());
            ExtCellSet cs = (ExtCellSet) rpt;
            StringBuilder sb = new StringBuilder();
            for (int r = 1; r <= 4; r++) {
                INormalCell c = cs.getCell(r, 4);
                Object v = c == null ? null : ((ExtNormalCell) c).getValue(false);
                sb.append("(").append(r).append(",4)=").append(v).append("  ");
            }
            return String.format("%-18s setExp->%-18s  %s", label, readBack, sb);
        } catch (Throwable t) {
            return String.format("%-18s setExp->%-18s  THROW %s: %s",
                    label, dExp, t.getClass().getName(), t.getMessage());
        }
    }

    public static void main(String[] args) {
        System.out.println("== VerifyF: template-cell expression override on the D column ==");
        String[][] cases = {
                { "baseline", "sum($B2:B2)" },
                { "constant", "999" },
                { "plain-arith", "B2*2" },
                { "plain-ref", "B2" },
                { "dollar-ref", "$B2" },
                { "dollar-arith", "$B2*3" },
                { "dollar-both", "$B$2" },
                { "range-rel", "sum(B2:B4)" },
                { "range-dollar", "sum($B2:B4)" },
                { "range-rel22", "sum(B2:B2)" },
                { "range-dollar22", "sum($B2:B2)" },
        };
        for (String[] c : cases) {
            System.out.println(probe(c[0], c[1]));
        }
    }
}
