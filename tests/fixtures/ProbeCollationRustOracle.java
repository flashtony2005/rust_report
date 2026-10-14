package rpx.fixtures;

import com.raqsoft.commonl.ChineseComparator;
import java.io.BufferedWriter;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.text.CollationElementIterator;
import java.text.RuleBasedCollator;
import java.util.Comparator;
import java.util.Random;

/**
 * Independent oracle for the Rust port of M2-12 (Text-Text).
 *
 * Verifier-owned: seed 0x5EED2A17L (deliberately NOT 20261014 / 314159265 /
 * any implementer/vendor seed). Produces tests/fixtures/_collation_oracle.tsv,
 * a fresh random pool of string pairs compared by the PRODUCT collator.
 *
 * Encoding discipline (critical):
 *   The key for each side is the Java string's own code-point sequence as
 *   returned by String.codePoints(). That is exactly the unit sequence the
 *   product RuleBasedCollator consumes (surrogate pairs -> one supplementary
 *   code point; unpaired surrogates -> themselves). The Rust side feeds these
 *   same code points into compare_text (all scalars) or compare_cps (contains
 *   an unpaired surrogate), so the two sides see identical inputs.
 *
 * Format: <hexA>\t<hexB>\t<-1|0|1>   ('#' comment lines first)
 */
public class ProbeCollationRustOracle {

	static final long SEED = 0x5EED2A17L;
	static final int N_RANDOM = 2400;

	public static void main(String[] args) throws Exception {
		RuleBasedCollator coll = (RuleBasedCollator) ChineseComparator.getInstance();
		Random r = new Random(SEED);

		String path = "tests/fixtures/_collation_oracle.tsv";
		BufferedWriter w = new BufferedWriter(new OutputStreamWriter(
				new FileOutputStream(path), "UTF-8"));
		w.write("# M2-12 independent oracle (verifier collation-rust-verify)\n");
		w.write("# generator=ProbeCollationRustOracle seed=" + Long.toHexString(SEED)
				+ " randomPairs=" + N_RANDOM + "\n");
		w.write("# keys = Java String.codePoints() of each operand (product unit sequence)\n");

		int n = 0;
		for (int i = 0; i < N_RANDOM; i++) {
			String a = randStr(r);
			String b = randStr(r);
			int sign = Integer.signum(coll.compare(a, b));
			w.write(hexCp(a) + "\t" + hexCp(b) + "\t" + sign + "\n");
			n++;
		}
		// explicit coverage pairs (pool corner cases the random draw may under-sample)
		String[][] fixed = FIXED;
		for (int i = 0; i < fixed.length; i++) {
			String a = s(fixed[i][0]);
			String b = s(fixed[i][1]);
			int sign = Integer.signum(coll.compare(a, b));
			w.write(hexCp(a) + "\t" + hexCp(b) + "\t" + sign + "\n");
			n++;
		}
		w.close();
		System.out.println("wrote " + path + " pairs=" + n + " random=" + N_RANDOM
				+ " fixed=" + fixed.length + " seed=" + Long.toHexString(SEED));

		// point-2 evidence: supplementary singleton decomposition + isolated surrogates
		ce(coll, "\uFA6C");                 // U+FA6C -> U+242EE (plane 2)
		ce(coll, "\uD800");                 // isolated high surrogate
		ce(coll, "\uDC00");                 // isolated low surrogate
		ce(coll, "\u00C1");                 // A + U+0301 (contraction hit)
		System.out.println("cmp(A+301, C1)=" + Integer.signum(coll.compare("A\u0301", "\u00C1")));
		System.out.println("cmp(0301, empty)=" + Integer.signum(coll.compare("\u0301", "")));
	}

	static final String[][] FIXED = {
		// Hangul with final consonant (L/V/T) vs plain syllables / others
		{"AC01", "AC00"}, {"D7A3", "D7A2"}, {"AC01", "4E2D"}, {"D7A3", "E000"},
		// isolated surrogates (must go through compare_cps)
		{"D800", "DC00"}, {"D800", "D801"}, {"DC00", "DC01"}, {"D800", "4E2D"},
		{"DC00", "E000"}, {"D800", "D800 0041"}, {"0041 D800", "0041 DC00"},
		// supplementary plane (valid pairs, compare_text path)
		{"1F600", "E000"}, {"20000", "4E2D"}, {"1F600", "1F601"}, {"2A6E0", "4E00"},
		// contraction hit
		{"00C1", "0041 0301"}, {"00E4", "0061 0308"}, {"00C7", "0043 0327"},
		{"00C5", "0041 030A"}, {"0100", "0041 0304"},
		// contraction miss / ignorable truncation
		{"0301", ""}, {"0301 0041", "0041"}, {"0041 0327 0042", "0041 0042"},
		{"0000 0041", "0041"}, {"0301", "0301 0041"},
		// canonical reorder (ccc different / equal)
		{"0041 0327 0301", "0041 0301 0327"}, {"0041 0301 0300", "0041 0300 0301"},
		{"0041 0301 0301", "0041 0301"}, {"0061 0301 0327 0300", "0061 0300 0327 0301"},
		// CJK / pinyin order
		{"4E00", "4E01"}, {"963F", "767D"}, {"4E00", "9FA5"}, {"5317 4EAC", "4E0A 6D77"},
		// case / ASCII / prefix
		{"41", "61"}, {"61", "42"}, {"", "61"}, {"0061 0062", "0061 0062 0063"},
		// plain combining char vs letter
		{"0301", "41"}, {"41", "0301"},
	};

	static String randStr(Random r) {
		int len = r.nextInt(11); // 0..10 code points
		StringBuilder b = new StringBuilder();
		for (int i = 0; i < len; i++) b.appendCodePoint(randCp(r));
		return b.toString();
	}

	static int randCp(Random r) {
		int k = r.nextInt(14);
		switch (k) {
			case 0: return 0x20 + r.nextInt(0x5F);        // ASCII printable
			case 1: return r.nextInt(0x20);               // ASCII control (incl. ignorable)
			case 2: return 0x00A0 + r.nextInt(0x60);      // Latin-1
			case 3: return 0x0100 + r.nextInt(0x80);      // Latin Ext-A
			case 4: return 0x0391 + r.nextInt(0x39);      // Greek
			case 5: return 0x0410 + r.nextInt(0x40);      // Cyrillic
			case 6: return 0x4E00 + r.nextInt(0x51A6);    // CJK
			case 7: return 0xAC00 + r.nextInt(0x2BA4);    // Hangul syllables (all have L/V/T)
			case 8: return 0xE000 + r.nextInt(0x1900);    // PUA
			case 9: return 0x0300 + r.nextInt(0x46);      // combining marks (ccc 0..240)
			case 10:                                      // supplementary
				int[] sup = {0x10000 + r.nextInt(0x1000), 0x20000 + r.nextInt(0xA6E0)};
				return sup[r.nextInt(2)];
			case 11: return 0x1F600 + r.nextInt(0x100);   // emoji plane
			case 12: return (r.nextBoolean() ? 0xD800 : 0xDC00) + r.nextInt(0x40); // isolated surrogate
			default: return 0xAC01 + r.nextInt(0x28);     // extra Hangul with final consonant
		}
	}

	static String s(String hex) {
		if (hex.isEmpty()) return "";
		StringBuilder b = new StringBuilder();
		for (String t : hex.trim().split("\\s+")) {
			int cp = Integer.parseInt(t, 16);
			b.appendCodePoint(cp);
		}
		return b.toString();
	}

	/** hex of the string's own code-point sequence (product unit sequence). */
	static String hexCp(String s) {
		StringBuilder b = new StringBuilder();
		int[] cps = s.codePoints().toArray();
		for (int i = 0; i < cps.length; i++) {
			if (i > 0) b.append(' ');
			b.append(Integer.toHexString(cps[i]).toUpperCase());
		}
		return b.toString();
	}

	static void ce(RuleBasedCollator c, String s) {
		CollationElementIterator it = c.getCollationElementIterator(s);
		StringBuilder sb = new StringBuilder();
		int n = 0;
		while (true) {
			int ce = it.next();
			if (ce == CollationElementIterator.NULLORDER) break;
			n++;
			sb.append(String.format(" [0x%04X,0x%04X,0x%04X]",
					CollationElementIterator.primaryOrder(ce),
					CollationElementIterator.secondaryOrder(ce),
					CollationElementIterator.tertiaryOrder(ce)));
			if (n > 6) { sb.append(" ..."); break; }
		}
		System.out.println("CE [" + hexCp(s) + "] ->" + sb + " (count=" + n + ")");
	}
}
