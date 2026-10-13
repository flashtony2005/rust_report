/*
 * ProbeEscape.java -- #17 消歧 3：`Escape.remove` 的完整转义规则（真机取证）
 *
 * 为什么必须真机验证：源码侧已逐行读出规则（`Escape.java:173-214`），但"源码写了"
 * 不等于"这条路径真的会被走到"。要证明差异**可观测**，得让产品自己算一遍。
 *
 * 关键差异（我们旧实现写错了）：**结尾的孤立反斜杠被丢弃**，不是保留。
 * 机制：`:188` 的 `if (++i == len) break;` —— 反斜杠后没有下一个字符就跳出循环。
 *
 * 走两条真实入口：
 *   A. 字符串字面量   `Expression.java:121-123` → `new Constant(Escape.remove(str))`
 *   B. `$[...]` 常量   `Expression.java:169-171` → `Escape.remove(内文.trim())`
 * 然后 `Engine.calc()` 读回计算态的值。
 *
 * 用法：java -cp "lib/*;out" rpx.fixtures.ProbeEscape
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.model.engine.ExtCellSet;
import com.raqsoft.reportl.usermodel.Context;
import com.raqsoft.reportl.usermodel.Engine;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IReport;

public class ProbeEscape {

	public static void main(String[] args) {
		System.out.println("=== 0. 源码规则（Escape.java:173-214，已逐行读）===");
		System.out.println("  \\t→9  \\r→13  \\n→10");
		System.out.println("  其它 \\x → 只留 x（丢反斜杠）        :203-206 default");
		System.out.println("  非 escape 字符原样                     :208-210");
		System.out.println("  ⛔ 结尾的孤立 \\ → 整段丢弃（不是保留）   :188 if (++i == len) break;");

		System.out.println();
		System.out.println("=== A. 字符串字面量（Escape.remove 直调）===");
		for (String lit : new String[] { "'a\\tb'", "'a\\nb'", "'a\\xb'", "'a\\\\b'", "'a\\\"b'" }) {
			report("字面量 " + lit, lit);
		}

		System.out.println();
		System.out.println("=== B. `$[...]` 常量（先 trim 再 Escape.remove）===");
		for (String src : new String[] { "$[abc]", "$[ a b ]", "$[a\\tb]", "$[a\\xb]", "$[a\\]" }) {
			report("表达式 " + src, src);
		}
	}

	/** 把表达式放进一个格、算一遍、读回 (2,1) 的值。 */
	private static void report(String label, String expr) {
		try {
			ReportDefine2 rd = new ReportDefine2(2, 1);
			EngineModels.lit(rd, 1, 1, "头");
			CalcProbe.setExp(rd.getCell(2, 1), expr);
			IReport rpt = new Engine(rd, new Context()).calc();
			if (!(rpt instanceof ExtCellSet)) {
				System.out.println("  " + pad(label) + " → calc 未返回 ExtCellSet");
				return;
			}
			INormalCell cell = ((ExtCellSet) rpt).getCell(2, 1);
			Object v = cell == null ? null : cell.getValue();
			System.out.println("  " + pad(label) + " → 值=" + CalcProbe.repr(v)
					+ "  类型=" + (v == null ? "null" : v.getClass().getSimpleName()));
		} catch (Throwable t) {
			System.out.println("  " + pad(label) + " → 抛异常 "
					+ t.getClass().getSimpleName() + " " + CalcProbe.repr(t.getMessage()));
		}
	}

	private static String pad(String s) {
		StringBuilder sb = new StringBuilder(s);
		while (sb.length() < 22) {
			sb.append(' ');
		}
		return sb.toString();
	}
}