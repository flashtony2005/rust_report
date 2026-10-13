/*
 * ProbeFunPriority.java -- 取「函数节点 priority 到底影响不影响树形」的真机证据
 *
 * 为什么需要它：架构师指出 `lefthead`/`tophead`=17、`mcorder`=16、其余=19
 * （LeftHead.java:22 / TopHead.java:22 / McOrder.java:110 / Node.java:54），
 * 而我们 Rust 侧的 `fun_priority` 表目前**没人消费** —— 独立验证方的变异 M6
 * （把 lefthead 17 改成 19）杀不死任何测试，因为那张表只被它自己的单测读。
 *
 * 那"接线"到底要不要做？不能靠推理下结论，因为产品用的是**线性扫描 + 右脊下探**
 * （Expression.java:442-450），不是 Pratt：
 *
 *   for (right = home; right != null && right.getPriority() < node.getPriority(); right = right.getRight())
 *       parent = right;
 *   node.setLeft(right);
 *   if (parent != null) { parent.setRight(node); continue; }
 *   home = node;
 *
 * 而 `Node.setLeft` 是**空实现**（Node.java:64-65），只有 `Operator`（left+right，
 * Operator.java:19-25）与 `SpecialFunction`（只有 left，:19-21）真的存。
 * ⇒ 若 `right` 没被存下而 `parent == null`，`home` 被覆盖，**前驱子树就被孤立丢弃**。
 *
 * 本探针直接读产品自己建出来的树（`Expression.getHome()` 是 public，:66），
 * 回答"17 与 19 在真实表达式里到底差在哪"。用法：
 *   java -cp "lib/*;out" rpx.fixtures.ProbeFunPriority
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.expression.Expression;
import com.raqsoft.reportl.model.expression.Node;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.Context;
import com.raqsoft.reportl.usermodel.IReport;

public class ProbeFunPriority {

	/** 一元运算符优先级 18（Negative.java:21），后缀 17（Offset/Move/DSOffset）。 */
	private static final int NEG = 18;

	public static void main(String[] args) {
		// `lefthead`/`tophead` 的 setParameter 会把 cs 转成 ExtCellSet 并调 getSource
		// （LeftHead.java:26,37）⇒ 传 null 必 NPE。要判 17 优先级的分叉必须带真实计算矩阵。
		IReport cs = buildCs();
		String[][] csCases = {
				{ "lefthead(A1)", "带 cs：左头函数基线（应为 17）" },
				{ "-lefthead(A1)", "带 cs：前缀 18 后接 17（分叉形状）" },
				{ "-sum(A1)", "带 cs 对照：前缀 18 后接 19" },
				{ "not lefthead(A1)", "带 cs 对照：前缀 16 后接 17" },
				{ "lefthead(A1)+2", "带 cs 对照：中缀 13 后接 17" },
				{ "-lefthead(A1)*2", "带 cs：前缀 18 → 17 → 中缀 14" },
		};
		for (String[] c : csCases) {
			dumpWithCs(cs, c[0], c[1]);
		}
		runWithoutCs();
	}

	/** 造一个最小计算矩阵：2 行 2 列全字面量，够 lefthead/mcorder 取 getSource。 */
	private static IReport buildCs() {
		ReportDefine2 rd = new ReportDefine2(2, 2);
		EngineModels.lit(rd, 1, 1, "R1C1");
		EngineModels.lit(rd, 1, 2, "R1C2");
		EngineModels.lit(rd, 2, 1, "R2C1");
		EngineModels.lit(rd, 2, 2, "R2C2");
		IReport rpt = CalcProbe.calc(rd, null);
		System.out.println("=== 带 cs 的形状（cs=" + (rpt == null ? "null" : rpt.getClass().getSimpleName())
				+ "）===");
		return rpt;
	}

	private static void dumpWithCs(IReport cs, String src, String note) {
		System.out.println("--- " + src + "    （" + note + "）");
		Node home;
		try {
			home = new Expression(cs, null, new Context(), src, false).getHome();
		} catch (Throwable t) {
			System.out.println("    建表达式失败：" + t.getClass().getName()
					+ " msg=" + CalcProbe.repr(t.getMessage()));
			StackTraceElement[] st = t.getStackTrace();
			for (int i = 0; i < 3 && i < st.length; i++) {
				System.out.println("      at " + st[i]);
			}
			return;
		}
		if (home == null) {
			System.out.println("    home == null");
			return;
		}
		System.out.println("    树根 priority=" + home.getPriority()
				+ "  isOperator=" + home.isOperator() + "  isExtended=" + home.isExtended());
		print(home, 2);
	}

	private static void runWithoutCs() {
		System.out.println("=== 不带 cs 的形状（只有不需要引擎矩阵的函数能建成）===");
		// (表达式, 说明)
		String[][] cases = {
				{ "lefthead(1)", "基线：左头函数 17" },
				{ "tophead(1)", "基线：顶头函数 17" },
				{ "mcorder(1)", "基线：mcorder 16" },
				{ "sum(1)", "基线：普通函数 19（对照组）" },
				{ "-lefthead(1)", "分叉形状：前缀 18 后接 17" },
				{ "-sum(1)", "对照组：前缀 18 后接 19" },
				{ "-mcorder(1)", "分叉形状：前缀 18 后接 16" },
				{ "not lefthead(1)", "对照组：前缀 16 后接 17" },
				{ "lefthead(1)+2", "对照组：中缀 13 后接 17" },
				{ "-lefthead(1)+2", "组合：前缀 18 → 17 → 中缀 13" },
				{ "-mcorder(1)*2", "组合：前缀 18 → 16 → 中缀 14" },
				{ "mcorder(1)+2", "对照组：中缀 13 后接 16" },
				{ "not mcorder(1)", "对照组：前缀 16 后接 16" },
				{ "lefthead(A1)", "左头函数换真格引用" },
		};
		for (String[] c : cases) {
			dump(c[0], c[1]);
		}
	}

	private static void dump(String src, String note) {
		System.out.println("--- " + src + "    （" + note + "）");
		Node home;
		try {
			// cs/ds 都传 null：本探针只用不依赖引擎矩阵的函数，避免把 CellRef 混进来
			home = new Expression(null, null, new Context(), src, false).getHome();
		} catch (Throwable t) {
			System.out.println("    建表达式失败：" + t.getClass().getName()
					+ " msg=" + CalcProbe.repr(t.getMessage()));
			StackTraceElement[] st = t.getStackTrace();
			for (int i = 0; i < 3 && i < st.length; i++) {
				System.out.println("      at " + st[i]);
			}
			return;
		}
		if (home == null) {
			System.out.println("    home == null（表达式没建出节点）");
			return;
		}
		System.out.println("    树根 priority=" + home.getPriority()
				+ "  isOperator=" + home.isOperator()
				+ "  isExtended=" + home.isExtended());
		print(home, 2);
	}

	/** 递归打印子树，深度缩进对齐。 */
	private static void print(Node n, int depth) {
		if (n == null) {
			return;
		}
		StringBuilder pad = new StringBuilder();
		for (int i = 0; i < depth; i++) {
			pad.append("  ");
		}
		System.out.println(pad + n.getClass().getSimpleName()
				+ "  pri=" + n.getPriority()
				+ "  op=" + n.isOperator()
				+ "  ext=" + n.isExtended()
				+ "  left=" + (n.getLeft() == null ? "null" : n.getLeft().getClass().getSimpleName())
				+ "  right=" + (n.getRight() == null ? "null" : n.getRight().getClass().getSimpleName()));
		print(n.getLeft(), depth + 1);
		print(n.getRight(), depth + 1);
	}

	/** 供报告引用：一元运算符优先级常量（本探针未直接用，防 unused 警告）。 */
	static int negPriority() {
		return NEG;
	}
}