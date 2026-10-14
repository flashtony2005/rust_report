/*
 * ProbeParamFull.java -- M3-01 参数全景探针（补 ProbeParamList 的四个缺口）
 *
 * 为什么需要它：spec-m3.md §8 的「参数全景」待验证项。ProbeParamList 已经把
 * 「值是不是 List 决定 isExtend」钉死（RUNLOG §15），但以下四条**仍是推断或空白**，
 * 而 M3-01 的参数夹具与 Rust 实现都要踩上去：
 *
 *   A. 空 List `List[]` 的 calc **完整后果** —— §15.3 只说「成为扩展格却没有子格」，
 *      但没跑过 calc：计算矩阵会怎样（塌陷 / 留一格 / 报错）？Rust 扩展算法从未接过
 *      这个形态（M2 无参数入口），必须先看产品真机。
 *   B. `@@q` **未登记**的 calc 后果 —— 源码推断是「不查 isParamName，直接取参值」，
 *      未登记时 getParamValue 返回 null ⇒ `new Constant(null)`，**不报错**。
 *      这是推断，真机没跑过（ProbeParamList 只测了登记的情况）。
 *   C. `@p` 未登记的**完整错误消息** —— ProbeParamList 只打到异常类名 `ReportError`，
 *      消息原文没落盘；Rust 侧的错误变体设计要对照它（等价信息，不逐字复刻）。
 *   D. `Types.getProperData` 的**类型转换矩阵** —— 静态参数默认值（Param.value 是
 *      String）注入 ctx 前要过的转换（CalcReport.java:44-47）。M3 的 server 层与夹具
 *      的 params 段编码都靠它。含空串 / 前后空格 / 各 dataType 的实测。
 *
 * 用法（与 GenEngineFixtures 同规）：
 *   CP="F:/project/report/reportlite/lib/*;F:/project/report/rpx/tests/fixtures/out"
 *   java -cp "$CP" rpx.fixtures.ProbeParamFull > probe_param_full_out.txt
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.Context;
import com.raqsoft.reportl.usermodel.Engine;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

public class ProbeParamFull {

	/** 参数名（三种写法共用）。 */
	private static final String P = "p";

	public static void main(String[] args) {
		header();
		emptyListCase();
		unregisteredMacroCase();
		unregisteredAtCase();
		typesMatrix();
		System.out.println();
		System.out.println("=== E. 完成 ===");
	}

	private static void header() {
		System.out.println("=== 0. 前置事实（与 ProbeParamList 一致，此探针不重复验证） ===");
		System.out.println("  值∈{标量,List} 决定 isExtend（RUNLOG §15.2）；本探针补四个缺口。");
	}

	// =================================================================
	// A. 空 List 的 calc 完整后果
	// =================================================================

	private static void emptyListCase() {
		System.out.println();
		System.out.println("=== A. 空 List `List[]` 的 calc 完整后果 ===");
		calcDump("空List", new ArrayList<Object>());
		// 对照：单元素 List（ProbeParamList 已测过，这里同框再跑一遍便于并排读）
		calcDump("单元素List[9]", list(Integer.valueOf(9)));
		calcDump("三元素List[1,2,3]", list(1, 2, 3));
	}

	/** 建一张 3x2 报表：格(2,1) 用 `@p`，按参数值 calc 后全量 dump。 */
	private static void calcDump(String note, Object paramValue) {
		ReportDefine2 rd = new ReportDefine2(3, 2);
		EngineModels.lit(rd, 1, 1, "值");
		EngineModels.lit(rd, 1, 2, "旁注");
		CalcProbe.setExp(rd.getCell(2, 1), "@" + P);
		EngineModels.lit(rd, 2, 2, note);
		Context ctx = new Context();
		ctx.setParamValue(P, paramValue);
		try {
			IReport rpt = new Engine(rd, ctx).calc();
			dump(note, rpt);
		} catch (Throwable t) {
			CalcProbe.fail("calc[" + note + "]", t);
		}
	}

	/**
	 * 全量 dump：尺寸 + 每格的 值/类型/扩展结构（含 source 的 extend/subCells）。
	 * 格坐标是计算态坐标（1 起），明说以防与模板坐标混淆。
	 */
	private static void dump(String note, IReport rpt) {
		if (!(rpt instanceof ExtCellSet)) {
			System.out.println("  [" + note + "] calc 未返回 ExtCellSet");
			return;
		}
		ExtCellSet ecs = (ExtCellSet) rpt;
		System.out.println("  [" + note + "] 计算态矩阵 = " + ecs.getRowCount() + "x"
				+ ecs.getColCount());
		for (int r = 1; r <= ecs.getRowCount(); r++) {
			for (int c = 1; c <= ecs.getColCount(); c++) {
				INormalCell cell = ecs.getCell(r, c);
				ExtCell src = ecs.getSource(r, c);
				System.out.println("    (" + r + "," + c + ") value="
						+ (cell == null ? "<无格>" : CalcProbe.repr(cell.getValue()))
						+ "  source=" + (src == null ? "null"
								: "extend=" + src.getExtensible() + " subCells="
										+ src.extCellSize()));
			}
		}
	}

	// =================================================================
	// B. `@@q` 未登记的 calc 后果（推断：Constant(null)，不报错）
	// =================================================================

	private static void unregisteredMacroCase() {
		System.out.println();
		System.out.println("=== B. `@@q` 未登记（ctx 无 q）的 calc ===");
		ReportDefine2 rd = new ReportDefine2(3, 2);
		EngineModels.lit(rd, 1, 1, "值");
		CalcProbe.setExp(rd.getCell(2, 1), "@@q");
		EngineModels.lit(rd, 2, 2, "@@q 未登记");
		Context ctx = new Context();
		try {
			IReport rpt = new Engine(rd, ctx).calc();
			dump("@@q 未登记", rpt);
		} catch (Throwable t) {
			CalcProbe.fail("calc[@@q 未登记]", t);
		}
		// 表达式层的独立对照（不跑 calc）：isExtend 与建树
		try {
			com.raqsoft.reportl.model.expression.Expression e =
					new com.raqsoft.reportl.model.expression.Expression(
							null, null, ctx, "@@q", false);
			System.out.println("  @@q 独立建树 isExtend=" + e.isExtend()
					+ " 树根=" + e.getHome().getClass().getSimpleName()
					+ " 值=" + CalcProbe.repr(e.calculate(ctx)));
		} catch (Throwable t) {
			CalcProbe.fail("表达式[@@q 未登记]", t);
		}
	}

	// =================================================================
	// C. `@p` 未登记的完整错误消息
	// =================================================================

	private static void unregisteredAtCase() {
		System.out.println();
		System.out.println("=== C. `@p` 未登记（ctx 无 p）的报错原文 ===");
		ReportDefine2 rd = new ReportDefine2(3, 2);
		EngineModels.lit(rd, 1, 1, "值");
		CalcProbe.setExp(rd.getCell(2, 1), "@" + P);
		EngineModels.lit(rd, 2, 2, "@p 未登记");
		Context ctx = new Context();
		try {
			IReport rpt = new Engine(rd, ctx).calc();
			System.out.println("  ⚠ 未报错！calc 返回 " + rpt.getClass().getSimpleName());
		} catch (Throwable t) {
			System.out.println("  异常类 = " + t.getClass().getName());
			System.out.println("  消息   = " + t.getMessage());
			if (t.getCause() != null) {
				System.out.println("  cause  = " + t.getCause().getClass().getName()
						+ " / " + t.getCause().getMessage());
			}
		}
	}

	// =================================================================
	// D. Types.getProperData 类型转换矩阵
	// =================================================================

	/** dataType 常量（Types2.java:22-44）与测试值。 */
	private static void typesMatrix() {
		System.out.println();
		System.out.println("=== D. Types.getProperData(dataType, 输入) 转换矩阵 ===");
		byte DT_INT = 1;
		byte DT_LONG = 2;
		byte DT_SHORT = 3;
		byte DT_FLOAT = 5;
		byte DT_DOUBLE = 6;
		byte DT_BIGDEC = 7;
		byte DT_STRING = 11;
		byte DT_BOOL = 12;
		byte DT_INT_ARR = 51;
		byte DT_STRING_ARR = 61;
		row(DT_INT, "10");
		row(DT_INT, " 10 ");
		row(DT_INT, "abc");
		row(DT_LONG, "42");
		row(DT_SHORT, "7");
		row(DT_FLOAT, "1.5");
		row(DT_DOUBLE, "3.14");
		row(DT_BIGDEC, "2.500");
		row(DT_STRING, "abc");
		row(DT_STRING, " pad ");
		row(DT_STRING, "");
		row(DT_INT, "");
		row(DT_BOOL, "true");
		row(DT_BOOL, "TRUE");
		row(DT_BOOL, "yes");
		row(DT_INT_ARR, "1,2,3");
		row(DT_STRING_ARR, "a,b,c");
		row(DT_STRING_ARR, "a, ,c");
	}

	private static void row(byte type, String val) {
		try {
			Object o = com.scudata.common.Types.getProperData(type, val);
			System.out.println("  type=" + pad(type) + " 输入=" + pad("[" + val + "]")
					+ " ⇒ " + CalcProbe.repr(o) + "  类="
					+ (o == null ? "null" : o.getClass().getName()));
		} catch (Throwable t) {
			System.out.println("  type=" + pad(type) + " 输入=" + pad("[" + val + "]")
					+ " ⇒ 抛 " + t.getClass().getName() + " / " + t.getMessage());
		}
	}

	private static String pad(Object s) {
		StringBuilder sb = new StringBuilder(String.valueOf(s));
		while (sb.length() < 12) {
			sb.append(' ');
		}
		return sb.toString();
	}

	private static List<Object> list(Object... vs) {
		List<Object> l = new ArrayList<Object>();
		l.addAll(Arrays.asList(vs));
		return l;
	}
}
