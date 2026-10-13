/*
 * EngineModelsTypes.java -- the engine_types fixture (M2-14 value-type
 * coverage). Kept in its own file so EngineModels.java stays under the
 * 300-line gate.
 *
 * Established empirically by ProbeTypes (RUNLOG §12), not by guessing:
 *
 *  - A LITERAL cell's boxed type survives .rpx write->read unchanged.
 *    ByteArrayOutputRecord.writeObject:202-266 gives every type its own tag
 *    byte (1 String, 2 BigDecimal, 4 Integer, 5 Long, 6 Boolean, 9 Double,
 *    10 Float, 11 Byte, 12 Short) and ByteArrayInputRecord.readObject:265-343
 *    rebuilds the exact boxed type, so CalcJson.typeName reports the
 *    original type after a full write->read->calc round trip.
 *  - Expressions additionally yield double/long/boolean/decimal
 *    (ProbeTypes PART 2), and dataset aggregates yield int/double
 *    (ProbeTypes PART 3) -- but those need a dataset and are non-deterministic
 *    to reason about, so this fixture pins the vocabulary with LITERALS only.
 *
 * The shipped 7 engine_* fixtures only ever produced string/int/list
 * (RUNLOG §11 / _shape_audit.py), which left M2-14 -- Integer(100) must not
 * come back as Double(100.0) -- with no product backing. This fixture is
 * that backing.
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;

import java.math.BigDecimal;

final class EngineModelsTypes {

	private EngineModelsTypes() {
	}

	/**
	 * engine_types -- one cell per value type the calculated grid can carry.
	 *
	 * 4x4, no dataset, no extension: the calculated grid equals the template,
	 * so every cell also carries a `source` (the invariant the Rust loader
	 * asserts) and nothing depends on a CSV.
	 *
	 * Layout (labels are the string-type controls):
	 *
	 *   row 1  int        long         double-frac  double-integral
	 *   row 2  100        1234567890123 92.5        100.0
	 *   row 3  decimal    boolean-true boolean-false float
	 *   row 4  100.00     true         false        1.5
	 *
	 * (2,1) is the M2-14 control: Integer(100) must stay `type=int`.
	 * (2,4) is Double(100.0): CalcJson.value() collapses it to the bare
	 * literal `100` (CalcJson.java:209-211), so an engine that infers the
	 * type from the JSON literal would wrongly report `int` -- only the
	 * `type` field says `double`.
	 * (4,1) is BigDecimal("100.00"): value() emits `100.00` verbatim
	 * (CalcJson.java:201-203) with `type=decimal`.
	 */
	static ReportDefine2 typesFixture() {
		ReportDefine2 rd = new ReportDefine2(4, 4);
		// row 1 -- labels (string), also the string-type controls
		EngineModels.lit(rd, 1, 1, "int");
		EngineModels.lit(rd, 1, 2, "long");
		EngineModels.lit(rd, 1, 3, "double-frac");
		EngineModels.lit(rd, 1, 4, "double-integral");
		// row 2 -- integral and floating literals
		EngineModels.lit(rd, 2, 1, Integer.valueOf(100));
		EngineModels.lit(rd, 2, 2, Long.valueOf(1234567890123L));
		EngineModels.lit(rd, 2, 3, Double.valueOf(92.5));
		EngineModels.lit(rd, 2, 4, Double.valueOf(100.0));
		// row 3 -- labels
		EngineModels.lit(rd, 3, 1, "decimal");
		EngineModels.lit(rd, 3, 2, "boolean-true");
		EngineModels.lit(rd, 3, 3, "boolean-false");
		EngineModels.lit(rd, 3, 4, "float");
		// row 4 -- the remaining types
		EngineModels.lit(rd, 4, 1, new BigDecimal("100.00"));
		EngineModels.lit(rd, 4, 2, Boolean.TRUE);
		EngineModels.lit(rd, 4, 3, Boolean.FALSE);
		EngineModels.lit(rd, 4, 4, Float.valueOf(1.5f));
		return rd;
	}
}
