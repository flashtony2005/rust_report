package rpx.fixtures;

import com.raqsoft.commonl.ChineseComparator;
import java.io.BufferedWriter;
import java.io.OutputStreamWriter;
import java.io.FileOutputStream;
import java.util.Arrays;
import java.util.Comparator;
import java.util.Random;

/**
 * Generates _collation_golden.tsv for the Rust port of M2-12 (Text-Text).
 * Independent seed (collation-verifier): 314159265 (NOT the vendor's 20261014).
 *
 * Format:
 *   # comment lines (metadata; parser should skip lines starting with '#')
 *   SORT &lt;idx&gt; &lt;hex code points space separated&gt; &lt;raw UTF-8 string&gt;
 *   PAIR &lt;hexA&gt; &lt;hexB&gt; &lt;-1|0|1&gt;
 *
 * Determinism of ties: the array is first ordered by code-point sequence, then
 * stable-sorted (Arrays.sort(Object[]) is stable) with the product collator.
 * Elements whose compare()==0 therefore keep ascending code-point order.
 * Tab/CR/LF inside strings are written as \t \r \n.
 */
public class GenCollationGolden {

	static final int SEED = 314159265;
	static final int N_SORT = 3200;

	public static void main(String[] args) throws Exception {
		Comparator<String> coll = ChineseComparator.getInstance();
		Random r = new Random(SEED);
		String[] arr = new String[N_SORT];
		int[] lens = new int[N_SORT];
		for (int i = 0; i < N_SORT; i++) {
			int len = (i < 20) ? 0 : 1 + r.nextInt(8);   // 0..8 code points
			lens[i] = len;
			arr[i] = randStr(r, len);
		}
		// tie-break basis: ascending code points (stable pre-order)
		Arrays.sort(arr, BY_CODEPOINT);
		Arrays.sort(arr, coll);               // stable -> preserves cp order on ties

		String path = "tests/fixtures/_collation_golden.tsv";
		BufferedWriter w = new BufferedWriter(new OutputStreamWriter(
				new FileOutputStream(path), "UTF-8"));
		w.write("# M2-12 collation gold standard (product com.raqsoft.commonl.ChineseComparator)\n");
		w.write("# generator=GenCollationGolden seed=" + SEED + " sortCount=" + arr.length
				+ " pairCount=" + PAIRS.length + "\n");
		w.write("# sorted with Arrays.sort(arr, ChineseComparator.getInstance()); ties resolved by\n");
		w.write("# ascending code-point order (stable sort over a code-point pre-sorted array)\n");
		for (int i = 0; i < arr.length; i++) {
			w.write("SORT\t" + i + "\t" + hex(arr[i]) + "\t" + esc(arr[i]) + "\n");
		}
		for (String[] p : PAIRS) {
			int c = Integer.signum(coll.compare(s(p[0]), s(p[1])));
			w.write("PAIR\t" + hex(s(p[0])) + "\t" + hex(s(p[1])) + "\t" + c + "\n");
		}
		w.close();
		verify(path, coll);
		System.out.println("wrote " + path + " sorts=" + arr.length + " pairs=" + PAIRS.length
				+ " seed=" + SEED);
		System.out.println("tie check: compare(\"A\\u0327\",\"A\")=" + coll.compare("A\u0327", "A")
				+ " compare(\"\\u0301\",\"\")=" + coll.compare("\u0301", ""));
	}

	/** Re-read the written file and assert it is exactly what the product collator ordered. */
	static void verify(String path, Comparator<String> coll) throws Exception {
		java.io.BufferedReader r = new java.io.BufferedReader(new java.io.InputStreamReader(
				new java.io.FileInputStream(path), "UTF-8"));
		String prev = null;
		int n = 0, ties = 0, badSort = 0, badTie = 0, badPair = 0;
		String line;
		while ((line = r.readLine()) != null) {
			if (line.startsWith("#") || line.isEmpty()) continue;
			String[] f = line.split("\t", -1);
			if (f[0].equals("SORT")) {
				String s = s(f[2]);
				if (prev != null) {
					int c = coll.compare(prev, s);
					if (c > 0) badSort++;
					else if (c == 0) { ties++; if (BY_CODEPOINT.compare(prev, s) > 0) badTie++; }
				}
				prev = s; n++;
			} else if (f[0].equals("PAIR")) {
				if (Integer.signum(coll.compare(s(f[1]), s(f[2]))) != Integer.parseInt(f[3])) badPair++;
			}
		}
		r.close();
		System.out.println("VERIFY sorts=" + n + " tiePairs=" + ties
				+ " badSortOrder=" + badSort + " badTieOrder=" + badTie + " badPair=" + badPair);
	}

	static final Comparator<String> BY_CODEPOINT = new Comparator<String>() {
		public int compare(String a, String b) {
			int[] x = a.codePoints().toArray(), y = b.codePoints().toArray();
			int m = Math.min(x.length, y.length);
			for (int i = 0; i < m; i++) if (x[i] != y[i]) return x[i] < y[i] ? -1 : 1;
			return x.length - y.length;
		}
	};

	static String randStr(Random r, int len) {
		StringBuilder b = new StringBuilder();
		for (int i = 0; i < len; i++) b.appendCodePoint(randCp(r));
		return b.toString();
	}

	static int randCp(Random r) {
		int k = r.nextInt(10);
		switch (k) {
			case 0: return 0x20 + r.nextInt(0x5F);            // ASCII printable
			case 1: return 0x00A0 + r.nextInt(0x60);          // Latin-1
			case 2: return 0x0100 + r.nextInt(0x80);          // Latin Ext-A
			case 3: return 0x0391 + r.nextInt(0x39);          // Greek
			case 4: return 0x0410 + r.nextInt(0x40);          // Cyrillic
			case 5: return 0x4E00 + r.nextInt(0x51A6);        // CJK
			case 6: return 0xAC00 + r.nextInt(0x2BA4);        // Hangul
			case 7: return 0xE000 + r.nextInt(0x1900);        // PUA
			case 8: return 0x0300 + r.nextInt(0x46);          // combining marks
			default:
				int[] sup = {0x10000 + r.nextInt(0x1000), 0x20000 + r.nextInt(0xA6E0)};
				return sup[r.nextInt(2)];
		}
	}

	static final String[][] PAIRS = {
		// contraction hit
		{"00C1", "0041 0301"}, {"00E4", "0061 0308"}, {"00C7", "0043 0327"},
		{"00C5", "0041 030A"}, {"00D1", "004E 0303"}, {"00FF", "0079 0308"},
		{"0100", "0041 0304"}, {"00C0", "0300 0041"},
		// contraction miss / truncation
		{"0061", "0062"}, {"0301", ""}, {"0301 0041", "0041"}, {"0041 0304", "0041"},
		{"0041 0327 0042", "0041 0042"}, {"0000 0041", "0041"}, {"0041 0301 0302", "00C1"},
		// ccc reorder
		{"0041 0327 0301", "0041 0301 0327"}, {"0041 0301 0300", "0041 0300 0301"},
		{"0041 0308 0301", "0041 0301 0308"},
		// unmapped BMP
		{"0391", "0410"}, {"0391", "0041"}, {"E000", "4E2D"}, {"FFFD", "FFFE"},
		{"FFFF", "FFFE"}, {"E000", "E001"}, {"0391", "0392"},
		// supplementary
		{"D800 DC00", "E000"}, {"D840 DC00", "E000"}, {"D83D DE00", "E000"},
		{"D87E DD00", "4E3D"}, {"D83D DE00", "4E2D"}, {"D800 DC00", "D800 DC01"},
		// surrogates unpaired
		{"D800", "DC00"}, {"D800 D800", "D800"}, {"DC00", "D800"},
		// prefix
		{"", "61"}, {"0061 0062", "0061 0062 0063"}, {"41", "41 0301"}, {"", "4E2D"},
		{"0301", "0301 0041"},
		// CJK / pinyin order
		{"963F", "554A"}, {"4E00", "4E59"}, {"4E2D", "56FD"}, {"9FA5", "4E00"},
		{"5317 4EAC", "4E0A 6D77"}, {"5F20", "738B"},
		// case / ASCII
		{"41", "61"}, {"61", "42"}, {"0061 0062 0063", "0041 0042 0043"}, {"41", "7A"}, {"7E", "20"},
		// Hangul vs others
		{"AC00", "4E2D"}, {"D7A3", "E000"}, {"1100", "AC00"},
		// Latin-1 vs its NFD
		{"00E9", "0065 0301"}, {"00FC", "0075 0308"}, {"00DF", "0073 0073"},
		// combined
		{"4E2D 0041 0301", "4E2D 00C1"}, {"D83D DE00 0301", "D83D DE00 0301 0041"}
	};

	static String s(String hex) {
		if (hex.isEmpty()) return "";
		StringBuilder b = new StringBuilder();
		for (String t : hex.trim().split("\\s+")) {
			int cp;
			try { cp = Integer.parseInt(t, 16); }
			catch (NumberFormatException e) { throw new IllegalArgumentException("bad hex token [" + t + "] in [" + hex + "]"); }
			if (cp < 0 || cp > 0x10FFFF) throw new IllegalArgumentException("cp out of range [" + t + "]=" + cp + " in [" + hex + "]");
			b.appendCodePoint(cp);
		}
		return b.toString();
	}

	static String hex(String s) {
		StringBuilder b = new StringBuilder();
		int i = 0;
		while (i < s.length()) {
			int cp = s.codePointAt(i);
			if (b.length() > 0) b.append(' ');
			b.append(cp <= 0xFFFF ? String.format("%04X", cp) : String.format("%X", cp));
			i += Character.charCount(cp);
		}
		return b.toString();
	}

	static String esc(String s) {
		StringBuilder b = new StringBuilder();
		for (int i = 0; i < s.length(); i++) {
			char c = s.charAt(i);
			if (c == '\t') b.append("\\t");
			else if (c == '\r') b.append("\\r");
			else if (c == '\n') b.append("\\n");
			else b.append(c);
		}
		return b.toString();
	}
}
