/*
 * EngineModelsCollate.java -- engine_collate: M2-12 Text-Text dictionary
 * order (the product's `ChineseComparator`, a RuleBasedCollator built from
 * `com/raqsoft/commonl/rule.txt`).
 *
 * WHY a dedicated fixture: until this one NO fixture compared two text
 * values with `<`/`>`/`<=`/`>=`, nor aggregated text with `max`/`min`. The
 * engine slice answers Text-Text compare with UnsupportedExpr, so nothing
 * pinned the product's real order.
 *
 * Grid = 8x6, no cell extends (every operand is a literal or a non-extending
 * aggregate), so template == calculated and every branch is visible.
 *
 *   R2  张 vs 阿   > T  < F  >= T  <= F
 *   R3  阿 vs 张   > F  < T  >= F  <= T
 *   R4  b  vs a    (pure ASCII)  > T  "b"<"a" F  "b">="a" T  "a">"b" F
 *   R5  一 vs 丁   > T  "一"<"丁" F  "一">="丁" T  <= F
 *       ^ code point order is 一(U+4E00) < 丁(U+4E01) but the PRODUCT order
 *         is 丁(primary 3322) < 一(primary 18183). The two DISAGREE, so a
 *         naive UTF-16 compare answers `false` where the product says `true`.
 *   R6  fully-ignorable U+0301 (a contract starter with value -1)
 *       CE("\u0301") truncates to EMPTY, so it compares EQUAL to "" -- but
 *       that means `<` is false and `>=` is TRUE (never both false):
 *       `"́"<""`  F / `"́">=""` T   (compare == 0)
 *       `"́"=="́"` T                 (== is EXACT String.equals, not collation)
 *       `"́"==""`  F                 (== again: the two strings differ)
 *       `"́">"A"` F / `"́"<"A"` T    (empty stream is a prefix of [A] ⇒ smaller)
 *   R7  contraction + canonical reordering
 *       `"Á">"À"`   T   (contraction A+0301 → CE 00C1 vs A+0300 → 00C0)
 *       `"Á"<="Á"`  T   (decomposed ≡ precomposed under collation)
 *       `"Á"=="Á"`  F   (== is exact: A+0301 ≠ U+00C1 as strings)
 *       reordered `A 0301 0327` vs canonical `A 0327 0301`:
 *         `>=` T, `>` F, `==` F  -- NFD reorders by ccc(0301)=230 > ccc(0327)=202
 *         so both collapse to CE 00C7, i.e. collation-equal but string-unequal.
 *   R8  max/min over text (dataset region column = 张 / 阿 / 丁)
 *       max(ds1.select(region)) = 张, min(...) = 阿 (genuine order, not row 1)
 *       plus the two `==` cross-checks.
 *
 * NOTE (evidence, not opinion): the brief proposed `max("张","阿")` and
 * `"\u0301"="\u0301"`. Both are WRONG shapes for this product:
 *   - `max(A,B)` is the 2-operand "region" form; for cell operands the
 *     product takes Max.java:161-183's else branch that just evaluates
 *     operand 1 (`max(A2,B2)` and `min(A2,B2)` BOTH return A2).
 *     With literals it throws `max函数参数类型错误` (measured).
 *     The genuine text aggregate is `max(<one List expr>)`.
 *   - single `=` is Operator `Evaluate` (priority 2): `Evaluate.java:36`
 *     throws unless `left` is UnknownSymbol/ParamNode. Exact equality is
 *     `==` (Equals.java:47 → Variant2.equals). Measured `=` throws
 *     `"="操作符号左操作数应为变量名称`.
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;

final class EngineModelsCollate {

	private EngineModelsCollate() {
	}

	static ReportDefine2 collateFixture() {
		ReportDefine2 rd = new ReportDefine2(8, 6);
		lit(rd, 1, "A", "B", "gt", "lt", "ge", "le");
		// ---- R2/R3: pinyin order, both branches of every operator -----
		pair(rd, 2, "张", "阿", "A2>B2", "A2<B2", "A2>=B2", "A2<=B2");
		pair(rd, 3, "阿", "张", "A3>B3", "A3<B3", "A3>=B3", "A3<=B3");
		// ---- R4: pure ASCII ------------------------------------------
		pair(rd, 4, "b", "a", "A4>B4", "\"b\"<\"a\"", "\"b\">=\"a\"", "\"a\">\"b\"");
		// ---- R5: code point order disagrees with the product order ----
		pair(rd, 5, "一", "丁", "A5>B5", "\"一\"<\"丁\"", "\"一\">=\"丁\"", "A5<=B5");
		// ---- R6: fully-ignorable U+0301 truncates the CE stream -------
		String q = "\"";
		String m = "\u0301"; // COMBINING ACUTE ACCENT, value == -1 in rule.txt
		exp(rd, 6, 1, q + m + q + "<" + q + q);
		exp(rd, 6, 2, q + m + q + ">=" + q + q);
		exp(rd, 6, 3, q + m + q + "==" + q + m + q);
		exp(rd, 6, 4, q + m + q + "==" + q + q);
		exp(rd, 6, 5, q + m + q + ">" + q + "A" + q);
		exp(rd, 6, 6, q + m + q + "<" + q + "A" + q);
		// ---- R7: contraction + canonical (ccc) reordering -------------
		exp(rd, 7, 1, "\"A\u0301\">\"A\u0300\"");
		exp(rd, 7, 2, "\"A\u0301\"<=\"\u00C1\"");
		exp(rd, 7, 3, "\"A\u0301\"==\"\u00C1\"");
		exp(rd, 7, 4, "\"A\u0301\u0327\">=\"A\u0327\u0301\"");
		exp(rd, 7, 5, "\"A\u0301\u0327\">\"A\u0327\u0301\"");
		exp(rd, 7, 6, "\"A\u0301\u0327\"==\"A\u0327\u0301\"");
		// ---- R8: max/min over text (a value list of literals) --------
		// `max(A,B)` is the 2-operand region form (returns operand 1);
		// `max(<one List expr>)` is the genuine aggregate. A `[...]` value
		// list gives a List of Strings WITHOUT needing a dataset -- and a
		// dataset column cannot be used here: the product writes fixture
		// CSVs in GBK (CalcProbe.writeCsv) while the Rust loader requires
		// ASCII/UTF-8, so non-ASCII CSV content is not loadable.
		exp(rd, 8, 1, "max([\"张\",\"阿\"])");
		exp(rd, 8, 2, "min([\"张\",\"阿\"])");
		exp(rd, 8, 3, "max([\"一\",\"丁\"])");
		exp(rd, 8, 4, "min([\"一\",\"丁\"])");
		exp(rd, 8, 5, "max([\"张\",\"阿\"])==\"张\"");
		exp(rd, 8, 6, "min([\"张\",\"阿\"])==\"阿\"");
		return rd;
	}

	/** header literals in one row */
	private static void lit(ReportDefine2 rd, int r, String... v) {
		for (int i = 0; i < v.length; i++) {
			rd.getCell(r, i + 1).setValue(v[i]);
		}
	}

	/** a row of two operand literals plus four comparison formulas */
	private static void pair(ReportDefine2 rd, int r, String a, String b,
			String c3, String c4, String c5, String c6) {
		rd.getCell(r, 1).setValue(a);
		rd.getCell(r, 2).setValue(b);
		exp(rd, r, 3, c3);
		exp(rd, r, 4, c4);
		exp(rd, r, 5, c5);
		exp(rd, r, 6, c6);
	}

	private static void exp(ReportDefine2 rd, int r, int c, String e) {
		CalcProbe.setExp(rd.getCell(r, c), e);
	}
}
