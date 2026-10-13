/*
 * ProbeParamList.java -- #17 消歧 1：多选参数（`@名` 绑 List）是否让该格变成扩展格
 *
 * 为什么必须真机验证（源码推断 vs 真机）：
 * 源码侧的推断是 —— 产品建参数节点时给的是**带值**的 Constant
 * （`Expression.java:185` 的 `@@p`、`:188` 的 `@p`、`:233` 的裸参数名 → `ParamNode`），
 * 而 `Constant.isExtended() = value instanceof List`（`Constant.java:22-24`），
 * ⇒ 多选参数（值是 List）**应该**让该格被包成扩展格。
 * 但这是**推断**：值到底会不会是 List、`isExtend()` 在这条路径上会不会真读到它，
 * 只有真机能回答。
 *
 * 走的是产品**唯一**的 isExtend 调用点那条路径（`ExtNormalCell.java:510`，
 * 构造表达式时 ds 恒为 null，见 `ExtNormalCell.java:498-504`）：
 *     new Expression(cs, null, ctx, expStr, false).isExtend()
 * 再补一层全量 calc：看该格在计算态里到底有没有被包成扩展格。
 *
 * 用法：java -cp "lib/*;out" rpx.fixtures.ProbeParamList
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCell;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.model.expression.Expression;
import com.raqsoft.reportl.model.expression.Node;
import com.raqsoft.reportl.usermodel.Context;
import com.raqsoft.reportl.usermodel.Engine;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

public class ProbeParamList {

	/** 参数名，三种写法都用它（`@@p` 不查 isParamName，`@p` 要查）。 */
	private static final String P = "p";

	public static void main(String[] args) {
		header();
		// ---------- 第一部分：isExtend() 的 A/B（产品的唯一调用点）----------
		System.out.println("=== A. isExtend()：标量参数 vs List 参数 ===");
		Object[][] values = {
				{ "Integer(7)", Integer.valueOf(7) },
				{ "String(\"x\")", "x" },
				{ "List[1,2,3]（多选）", list(1, 2, 3) },
				{ "List[9]（单元素）", list(9) },
				{ "List[]（空）", new ArrayList<Object>() },
		};
		for (Object[] v : values) {
			row("@" + P, v[1], String.valueOf(v[0]));
			row("@@" + P, v[1], String.valueOf(v[0]));
		}
		// ---------- 第二部分：对照组 ----------
		System.out.println();
		System.out.println("=== B. 对照组 ===");
		row("ds1.select(qty)", null, "无参数（已知为 true：DS 函数覆写 isExtended）");
		row("@" + P, null, "**未登记的参数**（应报错：unknownParam）");
		row("1+1", null, "纯字面量算术（无参数）");

		// ---------- 第三部分：全量 calc，看格是否真被包成扩展格 ----------
		System.out.println();
		System.out.println("=== C. 全量 calc：该格在计算态里是什么 ===");
		calcCase("标量 Integer(7)", Integer.valueOf(7));
		calcCase("List[1,2,3]（多选）", list(1, 2, 3));
		calcCase("List[9]（单元素）", list(9));
	}

	private static void header() {
		System.out.println("=== 0. 前置事实 ===");
		Context probe = new Context();
		probe.setParamValue(P, Integer.valueOf(7));
		System.out.println("  Context.isParamName(\"p\") = " + probe.isParamName("p")
				+ "   （setParamValue 走 params map，Context.java:163-168；"
				+ "isParamName 查 containsKey，:133-135）");
		System.out.println("  产品建参数节点的三条入口：Expression.java:185(@@p) / :188(@p) / :233(裸名)");
		System.out.println("  Constant.isExtended() = value instanceof List   （Constant.java:22-24）");
	}

	/** 第一/二部分的一行：给表达式配一个参数值，打印 isExtend 与建树过程。 */
	private static void row(String expr, Object paramValue, String note) {
		Context ctx = new Context();
		if (paramValue != null) {
			ctx.setParamValue(P, paramValue);
		}
		String shown = paramValue == null ? "(无 p)" : describe(paramValue);
		try {
			Expression e = new Expression(null, null, ctx, expr, false);
			Node dump = e.getHome();
			System.out.println("  " + pad(expr) + " 参数=" + pad(shown)
					+ " isExtend=" + pad5(String.valueOf(e.isExtend()))
					+ " 树根=" + (dump == null ? "null" : dump.getClass().getSimpleName()
							+ "(pri=" + dump.getPriority() + ")")
					+ "   // " + note);
		} catch (Throwable t) {
			System.out.println("  " + pad(expr) + " 参数=" + pad(shown)
					+ " 建表达式抛异常 " + t.getClass().getSimpleName()
					+ "   // " + note);
		}
	}

	/** 第三部分：整张报表算一遍，看格 2/1 的扩展结构。 */
	private static void calcCase(String note, Object paramValue) {
		ReportDefine2 rd = new ReportDefine2(3, 2);
		EngineModels.lit(rd, 1, 1, "值");
		EngineModels.lit(rd, 1, 2, "旁注");
		CalcProbe.setExp(rd.getCell(2, 1), "@" + P);
		EngineModels.lit(rd, 2, 2, note);
		Context ctx = new Context();
		if (paramValue != null) {
			ctx.setParamValue(P, paramValue);
		}
		try {
			// ⚠ 不能用 CalcProbe.calc —— 它自建 Context，参数登记就丢了（CalcProbe.java:126-131）
			IReport rpt = new Engine(rd, ctx).calc();
			if (!(rpt instanceof ExtCellSet)) {
				System.out.println("  " + pad(note) + " calc 未返回 ExtCellSet");
				return;
			}
			ExtCellSet ecs = (ExtCellSet) rpt;
			INormalCell cell = ecs.getCell(2, 1);
			ExtCell src = ecs.getSource(2, 1);
			System.out.println("  " + pad(note)
					+ " grid=" + ecs.getRowCount() + "x" + ecs.getColCount()
					+ "  格(2,1) value=" + CalcProbe.repr(cell == null ? null : cell.getValue())
					+ "  source=" + (src == null ? "null"
							: "extend=" + src.getExtensible() + " subCells=" + src.extCellSize())
					+ "   ⇒ " + (src != null && src.extCellSize() > 1 ? "★ 被包成扩展格" : "未扩展"));
		} catch (Throwable t) {
			CalcProbe.fail("calc[" + note + "]", t);
		}
	}

	private static List<Object> list(Object... vs) {
		List<Object> l = new ArrayList<Object>();
		l.addAll(Arrays.asList(vs));
		return l;
	}

	private static String describe(Object v) {
		if (v instanceof List) {
			return "List" + ((List<?>) v).toString();
		}
		return v.getClass().getSimpleName() + "(" + v + ")";
	}

	private static String pad(String s) {
		StringBuilder sb = new StringBuilder(s);
		while (sb.length() < 18) {
			sb.append(' ');
		}
		return sb.toString();
	}

	private static String pad5(String s) {
		StringBuilder sb = new StringBuilder(s);
		while (sb.length() < 5) {
			sb.append(' ');
		}
		return sb.toString();
	}
}