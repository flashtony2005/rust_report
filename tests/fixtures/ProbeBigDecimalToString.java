/*
 * ProbeBigDecimalToString.java -- 用真实 JVM 实跑 java.math.BigDecimal.toString()，
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
 * 产物与出处（2026-10-13）：本类的 FQCN 是 `rpx.fixtures.ProbeBigDecimalToString`，
 * 由 `scripts/gen_decimal_oracle.sh` 编译运行，产出
 * `tests/fixtures/_bdo/out_jre8.txt`（Zulu JDK 8.0.504）与
 * `tests/fixtures/_bdo/out_jdk21.txt`（Zulu JDK 21.0.12.1）。两份正文除版本头行外
 * 逐字节相同，这一点由 `decimal_oracle.rs::the_two_jvm_oracles_agree_byte_for_byte` 断言。
 *
 * ⚠ 仓库自带的 `common/jre` 是 Windows 版 JRE，macOS 上无法运行，故未在该 JRE 上复跑。
 *
 * 手动跑一行版（等价于脚本内部做的事）：
 *   javac -d out -source 8 -target 8 ProbeBigDecimalToString.java
 *   java -cp out rpx.fixtures.ProbeBigDecimalToString > out_jre8.txt
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
