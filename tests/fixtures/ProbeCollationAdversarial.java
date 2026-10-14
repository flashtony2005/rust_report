package rpx.fixtures;

import com.raqsoft.commonl.ChineseComparator;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.lang.reflect.Field;
import java.text.CollationElementIterator;
import java.text.Normalizer;
import java.text.RuleBasedCollator;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Random;
import java.util.Vector;
/**
 * Adversarial probe for M2-12 Text-Text collation.  INDEPENDENT (collation-verifier):
 * the model below was derived from rule.txt (product data) + the JDK contract table
 * read via reflection -- NOT from any ProbeCollation*Model/Table source. Every case
 * compares the verifier model against the live product collator.
 * Output: one "PASS/FAIL desc actual=.. expected=.." line per case, then a SUMMARY.
 */
public class ProbeCollationAdversarial {
	static RuleBasedCollator COLL;
	static final HashMap<Integer, Integer> PRIMARY = new HashMap<Integer, Integer>();
	static final HashMap<Long, Integer> CONTRACT = new HashMap<Long, Integer>();
	static final HashSet<Integer> STARTERS = new HashSet<Integer>();
	static final HashSet<Integer> SELF_UNMAPPED = new HashSet<Integer>();
	static int cases = 0, mism = 0;
	static final List<String> FAILS = new ArrayList<String>();
	public static void main(String[] args) throws Exception {
		COLL = (RuleBasedCollator) ChineseComparator.getInstance();
		loadModel();
		structuralClaims();
		referenceValues();

		System.out.println("== boundary cases (model vs product) ==");
		String[] singles = {
			"A\u0301", "\u00C1", "a\u0308", "\u00E4", "C\u0327", "\u00C7",
			"\u0000\u0001", "A\u0301\u0302", "\u0301", "\u0300", "\u0327", "",
			"\u0301A", "A\u0301", "\u0301\u0300\u0327", "\u0301\u0300",
			"A\u0327\u0301", "A\u0301\u0327", "A\u0301\u0300", "A\u0300\u0301",
			"A\u0304", "A\u0327B", "\u0000A", "\uD800", "\uDC00", "\uD800\uD800",
			"\u0391", "\u0410", "\uE000", "\uFFFD", "\uFFFE", "\uFFFF",
			"\uD800\uDC00", "\uD840\uDC00", "\uD83D\uDE00", "\uD87E\uDD00",
			"\u4E2D", "\u963F", "\uD83D\uDE00\u4E2D A\u0301",
			"\u0301\uD83D\uDE00A\u0301\u4E2D",
			"\u0001", "\u0001A", "\u0340", "\u0341", "\u0344", "\u0340A",
			"\u9FA6", "\uFA1F", "\u2E80", "\u00B5", "s\u0323\u0307"
		};
		for (String s : singles) checkCE("CE(" + hex(s) + ")", s);
		String[][] pairs = {
			{"\u00C1", "A\u0301"}, {"\u00E4", "a\u0308"}, {"\u00C7", "C\u0327"},
			{"\u0000\u0001", "\u0001"}, {"\u0301", "\u0300"}, {"\u0301", ""},
			{"\u0301A", "A"}, {"A\u0301", "A"}, {"\u0301\u0300", ""},
			{"A\u0327\u0301", "A\u0301\u0327"}, {"A\u0301\u0300", "A\u0300\u0301"},
			{"A\u0304", "A"}, {"A\u0327B", "AB"}, {"A\u0327B", "A\u0327C"},
			{"", "a"}, {"ab", "abc"}, {"A", "A\u0301"}, {"\u4E2D", "\u963F"},
			{"\uD83D\uDE00", "\u4E2D"}, {"\uE000", "\u4E2D"}, {"\uE000", "\uFFFD"},
			{"\uFFFF", "\uFFFE"}, {"\uD840\uDC00", "\uE000"}, {"\u0391", "\u0410"},
			{"\u0000", "\u0001"}, {"A\u0000B", "AB"}, {"\u0301A", "\u0300A"}
		};
		for (String[] p : pairs) checkCMP("CMP(" + hex(p[0]) + " , " + hex(p[1]) + ")", p[0], p[1]);

		System.out.println("== long random strings (seed=777001) ==");
		Random r = new Random(777001L);
		for (int t = 0; t < 12; t++) {
			String a = rnd(r, 100 + r.nextInt(60)), b = rnd(r, 100 + r.nextInt(60));
			checkCE("longCE#" + t, a, false);
			checkCE("longCE#" + t + "b", b, false);
			checkCMP("longCMP#" + t, a, b, false);
		}
		System.out.println("SUMMARY cases=" + cases + " mismatched=" + mism);
		for (String f : FAILS) System.out.println("  MINREPRO " + f);
	}
	static void checkCE(String desc, String s) { checkCE(desc, s, true); }
	static void checkCE(String desc, String s, boolean verbose) {
		cases++;
		int[] act = productCE(s), exp = modelCE(s);
		boolean ok = eq(act, exp);
		if (!ok) { mism++; FAILS.add(desc + " actual=" + arr(act) + " expected=" + arr(exp)); }
		System.out.println((ok ? "PASS" : "FAIL") + "  " + desc
				+ (verbose ? "  actual=" + arr(act) + "  expected=" + arr(exp)
						: "  actual.len=" + act.length + " expected.len=" + exp.length
								+ (ok ? "" : "  firstDiff=" + diff(act, exp))));
	}
	static void checkCMP(String desc, String a, String b) { checkCMP(desc, a, b, true); }
	static void checkCMP(String desc, String a, String b, boolean verbose) {
		cases++;
		int act = Integer.signum(COLL.compare(a, b));
		int exp = modelCompare(a, b);
		boolean ok = act == exp;
		if (!ok) { mism++; FAILS.add(desc + " actual=" + act + " expected=" + exp); }
		System.out.println((ok ? "PASS" : "FAIL") + "  " + desc
				+ "  actual=" + act + "  expected=" + exp
				+ (verbose ? "  ceA=" + arr(productCE(a)) + "  ceB=" + arr(productCE(b)) : ""));
	}
	static String diff(int[] a, int[] b) {
		int m = Math.min(a.length, b.length);
		for (int i = 0; i < m; i++) if (a[i] != b[i])
			return "@" + i + " actual=" + String.format("%04X", a[i]) + " expected=" + String.format("%04X", b[i]);
		return "prefix (len " + a.length + " vs " + b.length + ")";
	}
	// ---------------- model ----------------
	static void loadModel() throws Exception {
		StringBuilder sb = new StringBuilder();
		InputStream is = ChineseComparator.class.getClassLoader()
				.getResourceAsStream("com/raqsoft/commonl/rule.txt");
		InputStreamReader rd = new InputStreamReader(is, "UTF-8");
		char[] buf = new char[8096];
		int k;
		while ((k = rd.read(buf)) > 0) sb.append(buf, 0, k);
		String rules = sb.toString();
		int n = rules.length(), i = 0, idx = 0;
		List<String> elems = new ArrayList<String>();
		while (i < n) {
			char ch = rules.charAt(i);
			if (ch == '\'') {
				int j = i + 1;
				while (j < n && !(rules.charAt(j) == '\'' && (j + 1 >= n || rules.charAt(j + 1) == '<'))) j++;
				elems.add(rules.substring(i + 1, j)); i = j + 1;
			} else if (ch == '<') i++;
			else { elems.add(String.valueOf(ch)); i++; }
		}
		for (String e : elems) {
			if (e.length() == 1) PRIMARY.put(e.codePointAt(0), idx);
			else throw new IllegalStateException("multi-char rule element");
			idx++;
		}
		// contraction keys = NFD-2 expansions of listed characters
		HashSet<Integer> seconds = new HashSet<Integer>();
		for (String e : elems) {
			String d = Normalizer.normalize(e, Normalizer.Form.NFD);
			if (d.codePointCount(0, d.length()) == 2) {
				int c0 = d.codePointAt(0), c1 = d.codePointAt(Character.charCount(c0));
				CONTRACT.put(key(c0, c1), PRIMARY.get(e.codePointAt(0)));
				seconds.add(c1);
			}
		}
		// one JDK-internal key on C0/C1 recovered via reflection on the live table.
		// (0001,0000) is a dead fwd=false mirror -- NOT a usable key (see verdict R2).
		CONTRACT.put(key(0x0000, 0x0001), PRIMARY.get(0x0001));
		for (Long kk : CONTRACT.keySet()) STARTERS.add((int) (kk >>> 32));
		// every contraction second char is itself a starter (its own mapping is
		// absent/UNMAPPED, so an unmatched hit truncates the iteration)
		for (int c : seconds) { STARTERS.add(c); SELF_UNMAPPED.add(c); }
		SELF_UNMAPPED.add(0x0000);
		SELF_UNMAPPED.add(0x0001);
		// a char whose own mapping is UNMAPPED still acts as a truncating starter
		// even when it is no longer a live contraction key (e.g. 0001, a dead key)
		STARTERS.addAll(SELF_UNMAPPED);
		System.out.println("model: ruleChars=" + PRIMARY.size() + " contractKeys=" + CONTRACT.size()
				+ " starters=" + STARTERS.size() + " selfUnmapped=" + SELF_UNMAPPED.size());
	}
	static long key(int a, int b) { return ((long) a << 32) | (b & 0xFFFFFFFFL); }
	static int[] modelCE(String s) {
		String n = Normalizer.normalize(s, Normalizer.Form.NFD);
		int[] cp = n.codePoints().toArray();
		List<Integer> out = new ArrayList<Integer>();
		int i = 0;
		while (i < cp.length) {
			int c = cp[i];
			if (STARTERS.contains(c)) {
				if (i + 1 < cp.length && CONTRACT.containsKey(key(c, cp[i + 1]))) {
					out.add(CONTRACT.get(key(c, cp[i + 1]))); i += 2; continue;
				}
				if (SELF_UNMAPPED.contains(c)) break;   // truncation
			}
			Integer p = PRIMARY.get(c);
			if (p != null) { if (p != 0) out.add(p); }
			else if (c > 0xFFFF) { out.add(0x7FFF); out.add(high(c)); out.add(low(c)); }
			else { out.add(0x7FFF); out.add(c); }
			i++;
		}
		return toArr(out);
	}
	static int modelCompare(String a, String b) {
		int[] x = modelCE(a), y = modelCE(b);
		int m = Math.min(x.length, y.length);
		for (int i = 0; i < m; i++) if (x[i] != y[i]) return x[i] < y[i] ? -1 : 1;
		return x.length == y.length ? 0 : (x.length < y.length ? -1 : 1);
	}
	static int[] productCE(String s) {
		CollationElementIterator it = COLL.getCollationElementIterator(s);
		List<Integer> out = new ArrayList<Integer>();
		while (true) {
			int ce = it.next();
			if (ce == CollationElementIterator.NULLORDER) break;
			out.add((ce >>> 16) & 0xFFFF);
		}
		return toArr(out);
	}
	// ---------------- evidence: structural claims ----------------
	static void structuralClaims() throws Exception {
		Field tf = RuleBasedCollator.class.getDeclaredField("tables");
		tf.setAccessible(true);
		Object tb = tf.get(COLL);
		Field cf = tb.getClass().getDeclaredField("contractTable");
		cf.setAccessible(true);
		Vector<?> ct = (Vector<?>) cf.get(tb);
		int multi = 0, baseFirst = 0, markFirst = 0, lenNot2 = 0, fwdTrue = 0, fwdFalse = 0;
		for (int i = 0; i < ct.size(); i++) {
			Vector<?> row = (Vector<?>) ct.get(i);
			for (int j = 0; j < row.size(); j++) {
				Object ep = row.get(j);
				String nm = fld(ep, "entryName").toString();
				boolean fwd = (Boolean) fld(ep, "fwd");
				if (nm.length() == 1) continue;
				multi++;
				if (nm.length() != 2) lenNot2++;
				if (mark(nm.charAt(0))) markFirst++; else baseFirst++;
				if (fwd) fwdTrue++; else fwdFalse++;
			}
		}
		claim("contract rows == 25", ct.size() == 25, "" + ct.size());
		claim("all contract keys length 2", lenNot2 == 0, "len!=2 count=" + lenNot2);
		claim("2-char entryPairs == 108 (54 fwd + 54 dead)", multi == 108,
				"multi=" + multi + " fwd=true=" + fwdTrue + " fwd=false=" + fwdFalse);
		claim("live fwd=true keys == 54", fwdTrue == 54, "measured=" + fwdTrue);
		claim("model contractKeys == fwd=true keys", CONTRACT.size() == fwdTrue && CONTRACT.size() == 54,
				"model=" + CONTRACT.size() + " fwd=true=" + fwdTrue
						+ " | base-first=" + baseFirst + " mark-first=" + markFirst);
		claim("truncating starters (empty-CE own mapping) == 9", SELF_UNMAPPED.size() == 9,
				"measured=" + SELF_UNMAPPED.size() + " -> " + SELF_UNMAPPED
						+ " [12 = empty-CE BMP cps; 0340/0341/0344 fold away by NFD]");
	}
	static Object fld(Object o, String name) throws Exception {
		Field f = o.getClass().getDeclaredField(name);
		f.setAccessible(true);
		return f.get(o);
	}
	static boolean mark(char c) {
		int t = Character.getType(c);
		return t == Character.NON_SPACING_MARK || t == Character.COMBINING_SPACING_MARK
				|| t == Character.ENCLOSING_MARK;
	}
	static void referenceValues() {
		int[][] ref = {{0x963F, 294}, {0x4E01, 3322}, {0x4E00, 18183}, {0x5F20, 19926}};
		for (int[] r : ref) {
			int got = productCE(new String(Character.toChars(r[0])))[0];
			claim("ref U+" + String.format("%04X", r[0]) + " primary==" + r[1], got == r[1], "" + got);
		}
		int distinct = 0, single = 0;
		for (int cp = 0x4E00; cp <= 0x9FA5; cp++) {
			int[] ce = productCE(new String(Character.toChars(cp)));
			if (ce.length == 1 && ce[0] != 0x7FFF) { single++; distinct = cp; }
		}
		claim("CJK 4E00..9FA5 all single CE", single == 20902, "single=" + single);
		HashSet<Integer> set = new HashSet<Integer>();
		for (int cp = 0x4E00; cp <= 0x9FA5; cp++) set.add(productCE(new String(Character.toChars(cp)))[0]);
		claim("CJK primaries distinct", set.size() == 20902, "distinct=" + set.size());
	}
	static void claim(String d, boolean ok, String measured) {
		cases++;
		if (!ok) { mism++; FAILS.add("CLAIM " + d + " measured=" + measured); }
		System.out.println((ok ? "PASS" : "FAIL") + "  CLAIM " + d + "  measured=" + measured);
	}
	// ---------------- util ----------------
	static String rnd(Random r, int len) {
		StringBuilder b = new StringBuilder();
		for (int i = 0; i < len; i++) {
			int k = r.nextInt(9);
			int cp;
			if (k == 0) cp = 0x20 + r.nextInt(95);
			else if (k == 1) cp = 0x00A0 + r.nextInt(0x180);
			else if (k == 2) cp = 0x0391 + r.nextInt(0x100);
			else if (k == 3) cp = 0x4E00 + r.nextInt(0x1000);
			else if (k == 4) cp = 0x0300 + r.nextInt(0x46);
			else if (k == 5) cp = 0xAC00 + r.nextInt(0x400);
			else if (k == 6) cp = 0xE000 + r.nextInt(0x100);
			else if (k == 7) cp = 0x10000 + r.nextInt(0x1000);
			else cp = 0x20000 + r.nextInt(0x2000);
			b.appendCodePoint(cp);
		}
		return b.toString();
	}
	static int high(int cp) { return 0xD800 + ((cp - 0x10000) >> 10); }
	static int low(int cp) { return 0xDC00 + ((cp - 0x10000) & 0x3FF); }

	static int[] toArr(List<Integer> l) {
		int[] a = new int[l.size()];
		for (int i = 0; i < a.length; i++) a[i] = l.get(i);
		return a;
	}
	static boolean eq(int[] a, int[] b) { return java.util.Arrays.equals(a, b); }

	static String arr(int[] a) {
		StringBuilder b = new StringBuilder("[");
		for (int i = 0; i < a.length; i++) { if (i > 0) b.append(','); b.append(String.format("%04X", a[i])); }
		return b.append(']').toString();
	}
	static String hex(String s) {
		StringBuilder b = new StringBuilder();
		for (int i = 0; i < s.length(); i++) {
			if (i > 0) b.append(' ');
			b.append(String.format("%04X", (int) s.charAt(i)));
		}
		return b.toString();
	}
}
