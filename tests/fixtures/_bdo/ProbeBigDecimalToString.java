/*
 * ProbeBigDecimalToString.java -- 用产品自带的 JRE 实跑 java.math.BigDecimal.toString()，
 * 为 rpx-engine 的 `literal/decimal.rs`（`decimal_to_string` 的复刻）取 golden。
 *
 * 为什么必须实跑而不是"按 JavaDoc 推"：`scale < 0` 走指数形态这条规则与直觉相反
 * （直觉是"补零写全"），独立验证方用 JDK 21 实测发现 rpx 侧原实现把 `scale=-3, unscaled=1`
 * 写成了 `1000` 而 Java 给 `1E+3`。
 *
 * 输入形态刻意与 `rpx_core::reader::ObjValue::BigDecimal { scale, unscaled }` 对齐：
 * unscaled 是**大端补码字节**（即 Java `BigInteger.toByteArray()` 的形态），
 * 所以这里用 `new BigDecimal(new BigInteger(bytes), scale)` 还原，再取 `toString()`。
 *
 * 用法（JRE 无编译器，故用 JDK 编译、两个 JVM 分别跑）：
 *   /d/Tools/jdk-21.0.2/bin/javac -d out -source 8 -target 8 ProbeBigDecimalToString.java
 *   <jre>/bin/java -cp out ProbeBigDecimalToString
 */
package rpx.fixtures;

import java.math.BigDecimal;
import java.math.BigInteger;

final class ProbeBigDecimalToString {

	private ProbeBigDecimalToString() {
	}

	/** cases: {scale, unscaled}; unscaled 用 long 表示（超过 long 的另测）。 */
	private static final long[][] CASES = {
			// ---- scale == 0 ----
			{ 0, 0 }, { 0, 5 }, { 0, -5 }, { 0, 100 }, { 0, -100 }, { 0, 1234567890123L },
			// ---- scale > 0，adjusted >= -6（普通形态）----
			{ 2, 10000 },      // 100.00
			{ 1, 5 },          // 0.5
			{ 3, 1 },          // 0.001
			{ 2, 0 },          // 0.00
			{ 6, 1 },          // 0.000001（adjusted == -6 边界）
			{ 1, 100 },        // 10.0
			{ 4, 12345 },      // 1.2345
			{ 2, -10000 },     // -100.00
			{ 0, 16777216 },   // 跨字节
			// ---- scale > 0，adjusted < -6（指数形态）----
			{ 7, 1 },          // 1E-7
			{ 10, 1 },         // 1E-10
			{ 30, 1 },         // 1E-30
			{ 10, 0 },         // 0E-10（零带精度）
			{ 8, 12345 },      // adjusted = -8 + 4 = -4 → 普通？边界探一下
			// ---- scale < 0（指数形态）----
			{ -3, 1 },         // 1E+3
			{ -1, 5 },         // 5E+1
			{ -3, 0 },         // 0E+3
			{ -10, 12345 },    // 1.2345E+14
			{ -10, 1 },        // 1E+10
			{ -1, 12300 },     // 1.2300E+5（尾零是否保留？）
			{ -2, 1000 },      // 1.000E+5
			{ -1, -12300 },    // 负 scale 且负值
			{ -5, 1 },         // 1E+5
	};

	public static void main(String[] args) {
		System.out.println("java.version=" + System.getProperty("java.version"));
		for (long[] c : CASES) {
			int scale = (int) c[0];
			BigInteger unscaled = BigInteger.valueOf(c[1]);
			String bytes = hex(unscaled.toByteArray());
			BigDecimal bd = new BigDecimal(unscaled, scale);
			System.out.println(scale + "|" + bytes + "|" + unscaled + "|" + bd.toString()
					+ "|" + bd.toPlainString());
		}
	}

	private static String hex(byte[] bs) {
		StringBuilder sb = new StringBuilder();
		for (int i = 0; i < bs.length; i++) {
			if (i > 0) {
				sb.append(',');
			}
			sb.append(String.format("%02X", bs[i] & 0xFF));
		}
		return sb.toString();
	}
}
