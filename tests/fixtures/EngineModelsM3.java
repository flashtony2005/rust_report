/*
 * EngineModelsM3.java -- engine_param: report parameters (M3-01), pinned by
 * the product.
 *
 * WHY this fixture: M2's fifteen fixtures contain no report parameter at all,
 * so the parameter channel had no end-to-end evidence. The Rust side binds
 * parameter values at parse time (crates/rpx-engine/src/params.rs), which is
 * only meaningful if the product's own calculation grid can confirm it. The
 * parameter semantics themselves were probed first (ProbeParamList.java and
 * ProbeParamFull.java; results in RUNLOG §15 and §24).
 *
 * Layout (3 rows x 4 cols; row 1 is the header):
 *   A2  @p       the List parameter -- isExtend=true drives a vertical
 *                extension (product: the matrix grows and A2 becomes
 *                extend=18 subCells=3; RUNLOG §24.3)
 *   B2  literal  the neighbouring literal column: its continuation slots get
 *                the template value copied down the extension (probe A shape)
 *   C2  sum(@p)  the SAME parameter wrapped in an aggregate -- isExtend does
 *                NOT propagate through Sum (m2-engine-design §13.3), so this
 *                cell stays 17 and evaluates to 6
 *   D2  max(@p)  same, evaluates to 3
 *   B3  @s       a scalar parameter: PlainFormula (17), value 7
 *
 * ⚠ **Why there is no `@@p` cell here** (2026-10-14): the first version of
 * this fixture had E2 = `@@p` beside A2 = `@p` -- two 18 extensions on one
 * row -- and the product produced an 11x5 grid with a row-band interaction
 * (A2's sub-cell anchors at rows 2/5/8, `@@p`'s at 2/3/4) that the Rust
 * engine's extension algorithm (M2-15) does not model. The product's
 * behaviour is recorded in RUNLOG §24.4 as an explicit boundary; this
 * fixture keeps only the single-extension shape the Rust side must
 * reproduce. `@@p`'s parse-time semantics are pinned separately
 * (probe_param_full B group + expr/tests/deviation.rs).
 *
 * Injected parameters (params()): p = ArrayList[1,2,3], s = Integer(7).
 * ⚠ The value MUST be a java.util.List -- the product's judgement is
 * `instanceof List` (Constant.java:22-24), so an int[] would NOT extend even
 * though it holds the same numbers.
 */
package rpx.fixtures;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import com.raqsoft.reportl.model.ReportDefine2;

final class EngineModelsM3 {

	private EngineModelsM3() {
	}

	static ReportDefine2 paramFixture() {
		ReportDefine2 rd = new ReportDefine2(3, 4);
		rd.getCell(1, 1).setValue("值");
		rd.getCell(1, 2).setValue("旁注");
		rd.getCell(1, 3).setValue("求和");
		rd.getCell(1, 4).setValue("最大");
		CalcProbe.setExp(rd.getCell(2, 1), "@p");
		rd.getCell(2, 2).setValue("三元素List[1,2,3]");
		CalcProbe.setExp(rd.getCell(2, 3), "sum(@p)");
		CalcProbe.setExp(rd.getCell(2, 4), "max(@p)");
		CalcProbe.setExp(rd.getCell(3, 2), "@s");
		// The dataset is attached but never referenced by any cell: it keeps the
		// CSV/loader path identical to the other fixtures and proves an unused
		// dataset does not perturb a parameter-driven calculation.
		CalcProbe.attachFileDataSet(rd, "ds1", EngineModels.csvName("engine_param"));
		return rd;
	}

	/** Parameters injected into the Context before Engine.calc (null elsewhere). */
	static Map<String, Object> params(String fixture) {
		if (!"engine_param".equals(fixture)) {
			return null;
		}
		Map<String, Object> m = new LinkedHashMap<String, Object>();
		List<Object> p = new ArrayList<Object>(Arrays.asList(Integer.valueOf(1),
				Integer.valueOf(2), Integer.valueOf(3)));
		m.put("p", p);
		m.put("s", Integer.valueOf(7));
		return m;
	}

	/** The parameter map as a JSON array of {name, value} entries (raw JSON). */
	static String paramsJson(String fixture) {
		Map<String, Object> params = params(fixture);
		if (params == null || params.isEmpty()) {
			return "[]";
		}
		StringBuilder b = new StringBuilder("[");
		boolean first = true;
		for (Map.Entry<String, Object> e : params.entrySet()) {
			if (!first) {
				b.append(',');
			}
			first = false;
			b.append("{\"name\":").append(Json.quote(e.getKey()))
					.append(",\"value\":").append(paramValueJson(e.getValue()))
					.append('}');
		}
		return b.append(']').toString();
	}

	/** A parameter value as JSON: List -> array, Number/Boolean verbatim. */
	private static String paramValueJson(Object v) {
		if (v instanceof List) {
			StringBuilder b = new StringBuilder("[");
			boolean first = true;
			for (Object item : (List<?>) v) {
				if (!first) {
					b.append(',');
				}
				first = false;
				b.append(paramValueJson(item));
			}
			return b.append(']').toString();
		}
		if (v instanceof Number || v instanceof Boolean) {
			return v.toString();
		}
		return Json.quote(v == null ? null : v.toString());
	}
}
