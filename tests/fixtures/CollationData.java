package rpx.fixtures;

import java.lang.reflect.Method;
import java.text.Normalizer;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;

/**
 * Live extraction of the collation tables the Rust port needs (M2-12
 * Text-Text dictionary order).
 *
 * Sections 0..12 mirror -- one to one -- the frozen `collation.bin` layout
 * documented in the task brief. Nothing here is hand-transcribed: every
 * number comes from the product's own {@code RuleBasedCollator} (CE dump)
 * and the product JRE's {@code java.text.Normalizer}.
 *
 * Section recap (see GenCollationData / the brief for the byte layout):
 *   0 cjk_primary (u16)  1 singles (u32 cp|primary&lt;&lt;16)  2 nfd_cp (u32)
 *   3 nfd_off (u32)      4 nfd_len (u8)     5 nfd_pool (u32)
 *   6 ccc_cp (u32)       7 ccc_val (u8)
 *   8 contract_starter (u32 cp|key_index&lt;&lt;16)  9 contract_count (u8)
 *  10 contract_key (u32 cp0|cp1&lt;&lt;16)  11 contract_key_ce (u16)  12 ignorable (u8)
 *
 * NOTE on [8]: `key_index` is the **offset into section 10** where this
 * starter's keys begin (so `[10][key_index .. key_index+count)` is the
 * group). The brief calls this field `first_key`; offset is the only
 * reading under which the (starter, count) pair is self-sufficient.
 */
final class CollationData {

	static final int CJK_LO = 0x4E00;
	static final int CJK_HI = 0x9FA5;
	static final int HANGUL_LO = 0xAC00;
	static final int HANGUL_HI = 0xD7A3;
	static final int MAX_CP = 0x10FFFF;
	static final int SEC_CNT = 13;
	/** Element width in bytes, index == section number. */
	static final int[] WIDTH = { 2, 4, 4, 4, 1, 4, 4, 1, 4, 1, 4, 2, 1 };

	private final int[][] sec = new int[SEC_CNT][];

	/** How the ccc table was obtained (reflection vs black-box). */
	String cccHow = "";
	/** nfd entries whose full NFD is a single code point (e.g. U+0340 -> U+0300). */
	int nfdSingleLen = 0;
	int maxContractKeyLen = 0;
	/** Set of cp with a non-empty, single-CE dump, for the starter-primary check. */
	private final Map<Integer, Integer> ownPrimary = new HashMap<Integer, Integer>();

	int[] section(int i) {
		return sec[i];
	}

	static int primaryOf(String ce) {
		return Integer.parseInt(ce.substring(0, ce.indexOf('/')), 16);
	}

	static boolean isSurrogate(int cp) {
		return cp >= 0xD800 && cp <= 0xDFFF;
	}

	static String hex(int cp) {
		return cp <= 0xFFFF ? String.format("%04X", cp)
				: String.format("%06X", cp);
	}

	// =================================================================
	// extraction
	// =================================================================

	static CollationData extract(Map<Integer, String> dump,
			Map<Integer, ProbeCollationModel.Contract> contracts,
			Map<String, String> entryCE) throws Exception {
		CollationData d = new CollationData();

		// ---- 0 cjk_primary + 1 singles ----
		int[] cjk = new int[CJK_HI - CJK_LO + 1];
		Arrays.fill(cjk, -1);
		List<Integer> singles = new ArrayList<Integer>();
		for (int cp = 0; cp <= 0xFFFF; cp++) {
			if (isSurrogate(cp)) {
				continue;
			}
			String ce = dump.get(Integer.valueOf(cp));
			if (ce == null || ce.length() == 0 || ce.indexOf(',') >= 0) {
				continue; // empty (ignorable) / unmapped fallback / multi-CE
			}
			int p = primaryOf(ce);
			d.ownPrimary.put(Integer.valueOf(cp), Integer.valueOf(p));
			if (cp >= CJK_LO && cp <= CJK_HI) {
				cjk[cp - CJK_LO] = p;
			} else {
				singles.add(Integer.valueOf(cp | (p << 16)));
			}
		}
		d.sec[0] = cjk;
		d.sec[1] = toArr(singles);

		// ---- 2 nfd_cp / 3 nfd_off / 4 nfd_len / 5 nfd_pool ----
		List<Integer> ncp = new ArrayList<Integer>();
		List<Integer> noff = new ArrayList<Integer>();
		List<Integer> nlen = new ArrayList<Integer>();
		List<Integer> pool = new ArrayList<Integer>();
		for (int cp = 0; cp <= MAX_CP; cp++) {
			if (isSurrogate(cp)) {
				continue;
			}
			if (cp >= HANGUL_LO && cp <= HANGUL_HI) {
				continue; // Hangul syllables: the Rust side decomposes
				          // them algorithmically, they are NOT table entries
			}
			String s = new String(Character.toChars(cp));
			String n = Normalizer.normalize(s, Normalizer.Form.NFD);
			if (n.equals(s)) {
				continue;
			}
			int[] pts = n.codePoints().toArray();
			ncp.add(Integer.valueOf(cp));
			noff.add(Integer.valueOf(pool.size()));
			nlen.add(Integer.valueOf(pts.length));
			if (pts.length == 1) {
				d.nfdSingleLen++;
			}
			for (int x : pts) {
				pool.add(Integer.valueOf(x));
			}
		}
		d.sec[2] = toArr(ncp);
		d.sec[3] = toArr(noff);
		d.sec[4] = toArr(nlen);
		d.sec[5] = toArr(pool);

		// ---- 6 ccc_cp / 7 ccc_val ----
		Method m = CollationCcc.resolve();
		List<Integer> ccp = new ArrayList<Integer>();
		List<Integer> cval = new ArrayList<Integer>();
		if (m != null) {
			d.cccHow = m.getDeclaringClass().getName() + "." + m.getName()
					+ "(int) via reflection";
			for (int cp = 0; cp <= MAX_CP; cp++) {
				if (isSurrogate(cp)) {
					continue;
				}
				int v = ((Integer) m.invoke(null, Integer.valueOf(cp)))
						.intValue();
				if (v != 0) {
					ccp.add(Integer.valueOf(cp));
					cval.add(Integer.valueOf(v));
				}
			}
		} else {
			d.cccHow = "black-box: NFD '" + "m1 m2" + "' swap order";
			CollationCcc.deriveBlackBox(ccp, cval);
		}
		d.sec[6] = toArr(ccp);
		d.sec[7] = toArr(cval);

		// ---- 8..12 contraction table ----
		List<Integer> starters = new ArrayList<Integer>(contracts.keySet());
		Collections.sort(starters);
		List<Integer> cst = new ArrayList<Integer>();
		List<Integer> cct = new ArrayList<Integer>();
		List<Integer> cig = new ArrayList<Integer>();
		List<Integer> ckey = new ArrayList<Integer>();
		List<Integer> cke = new ArrayList<Integer>();
		int idx = 0;
		for (Integer cpI : starters) {
			ProbeCollationModel.Contract c = contracts.get(cpI);
			cst.add(Integer.valueOf(cpI.intValue() | (idx << 16)));
			cct.add(Integer.valueOf(c.keys.size()));
			cig.add(Integer.valueOf(c.ignorable ? 1 : 0));
			for (String k : c.keys) {
				if (k.length() > d.maxContractKeyLen) {
					d.maxContractKeyLen = k.length();
				}
				ckey.add(Integer.valueOf(k.charAt(0) | (k.charAt(1) << 16)));
				cke.add(Integer.valueOf(primaryOf(entryCE.get(k))));
			}
			idx += c.keys.size();
		}
		d.sec[8] = toArr(cst);
		d.sec[9] = toArr(cct);
		d.sec[10] = toArr(ckey);
		d.sec[11] = toArr(cke);
		d.sec[12] = toArr(cig);
		return d;
	}

	private static int[] toArr(List<Integer> l) {
		int[] a = new int[l.size()];
		for (int i = 0; i < a.length; i++) {
			a[i] = l.get(i).intValue();
		}
		return a;
	}

	// =================================================================
	// invariants
	// =================================================================

	void assertInvariants() {
		// [0] complete, all primaries distinct (permutation of the pinyin ranks)
		HashSet<Integer> seen = new HashSet<Integer>();
		for (int i = 0; i < sec[0].length; i++) {
			int p = sec[0][i];
			if (p < 0) {
				fail("cjk_primary missing for U+" + hex(CJK_LO + i));
			}
			if (!seen.add(Integer.valueOf(p))) {
				fail("cjk primary duplicated: " + hex(p));
			}
		}
		if (sec[0].length != CJK_HI - CJK_LO + 1) {
			fail("cjk_primary length " + sec[0].length);
		}
		// [1] must not contain any cp inside [0x4E00,0x9FA5]
		for (int v : sec[1]) {
			int cp = v & 0xFFFF;
			if (cp >= CJK_LO && cp <= CJK_HI) {
				fail("singles contains CJK cp " + hex(cp));
			}
		}
		// [10] every key is exactly 2 chars, [11] single CE and never 7FFF
		for (int i = 0; i < sec[10].length; i++) {
			if (sec[11][i] > 0x7FFE) {
				fail("contract_key_ce out of range: " + hex(sec[11][i]));
			}
		}
		if (maxContractKeyLen != 2) {
			fail("contract key length " + maxContractKeyLen + " (expected 2)");
		}
		// non-ignorable starters must expose their own primary in [0] or [1]
		for (int i = 0; i < sec[8].length; i++) {
			if (sec[12][i] != 0) {
				continue;
			}
			int cp = sec[8][i] & 0xFFFF;
			Integer p = ownPrimary.get(Integer.valueOf(cp));
			if (p == null) {
				fail("non-ignorable starter " + hex(cp)
						+ " has no single-CE primary in [0]/[1]");
			}
		}
		// [9] count == number of keys in the starter's group
		int total = 0;
		for (int c : sec[9]) {
			total += c;
		}
		if (total != sec[10].length) {
			fail("contract_count sum " + total + " != key count "
					+ sec[10].length);
		}
		// [7] ccc in 1..254
		for (int v : sec[7]) {
			if (v < 1 || v > 254) {
				fail("ccc out of range: " + v);
			}
		}
		// [4] every decomposition has at least one code point
		for (int v : sec[4]) {
			if (v < 1) {
				fail("nfd_len == 0");
			}
		}
		// [3]+[4] must index inside the pool
		for (int i = 0; i < sec[3].length; i++) {
			if (sec[3][i] + sec[4][i] > sec[5].length) {
				fail("nfd entry " + i + " runs past the pool");
			}
		}
	}

	private static void fail(String msg) {
		throw new IllegalStateException("collation invariant violated: " + msg);
	}
}
