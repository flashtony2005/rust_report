package rpx.fixtures;

import com.raqsoft.commonl.ChineseComparator;
import java.io.BufferedReader;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.text.CollationElementIterator;
import java.text.Normalizer;
import java.text.RuleBasedCollator;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Extracts the collation tables and PROVES the port model against the
 * product's real behavior (M2-12 Text-Text closure).
 *
 * Model under test:
 *   CE(text) = walk(NFD(text)) where at each position:
 *     a) longest contraction key match  -> probed CE of the key
 *     b) else per-char table: empty | single(W) | fallback [7FFF,cp]
 *
 * Inputs:  args[0] = _collation_dump.tsv (produced by ProbeCollationDump)
 * Outputs (outDir = args[1]):
 *   _collation_nfd.tsv       cp -> NFD code point sequence (only when != cp)
 *   _collation_contract.tsv  key | probed CE | source cp
 *   _collation_model.txt     per-BMP model-vs-product comparison + summary
 *   _collation_ccc.tsv       cp -> combining class (reflection, best effort)
 */
public class ProbeCollationTables {

	public static void main(String[] args) throws Exception {
		final String dumpPath = args[0];
		final String outDir = args.length > 1 ? args[1] : ".";

		RuleBasedCollator c = (RuleBasedCollator) ChineseComparator
				.getInstance();

		// ---- read the product CE dump ----
		Map<Integer, String> dump = new HashMap<Integer, String>();
		BufferedReader r = new BufferedReader(new InputStreamReader(
				new FileInputStream(dumpPath), "UTF-8"));
		String ln;
		while ((ln = r.readLine()) != null) {
			String[] p = ln.split("\t", -1);
			dump.put(Integer.valueOf(Integer.parseInt(p[0], 16)),
					p.length > 1 ? p[1] : "");
		}
		r.close();

		// ---- 1) NFD table + contraction key candidates ----
		Map<String, Integer> candidates = new LinkedHashMap<String, Integer>();
		PrintWriter nfdW = new PrintWriter(new OutputStreamWriter(
				new FileOutputStream(outDir + "/_collation_nfd.tsv"), "UTF-8"));
		int nfdCount = 0;
		for (int cp = 0; cp <= 0xFFFF; cp++) {
			if (cp >= 0xD800 && cp <= 0xDFFF) {
				continue;
			}
			String s = new String(Character.toChars(cp));
			String nfd = Normalizer.normalize(s, Normalizer.Form.NFD);
			if (!nfd.equals(s)) {
				nfdW.println(String.format("%04X\t%s", cp, cps(nfd)));
				nfdCount++;
			}
			String d = dump.get(Integer.valueOf(cp));
			if (d != null && nfd.length() > 1 && d.indexOf(',') < 0
					&& d.length() > 0) {
				if (!candidates.containsKey(nfd)) {
					candidates.put(nfd, Integer.valueOf(cp));
				}
			}
		}
		nfdW.close();

		// ---- 2) probe each contraction key ----
		final Map<String, String> contract = new HashMap<String, String>();
		PrintWriter cW = new PrintWriter(new OutputStreamWriter(
				new FileOutputStream(outDir + "/_collation_contract.tsv"),
				"UTF-8"));
		for (Map.Entry<String, Integer> e : candidates.entrySet()) {
			String key = e.getKey();
			String probed = ceOf(c, key);
			String src = dump.get(e.getValue());
			contract.put(key, probed);
			cW.println(String.format("%s\t%s\t%s", cps(key), probed, src));
		}
		cW.close();

		// ---- 3) model check over all BMP ----
		PrintWriter mW = new PrintWriter(new OutputStreamWriter(
				new FileOutputStream(outDir + "/_collation_model.txt"),
				"UTF-8"));
		int checked = 0, mismatched = 0;
		int maxKeyLen = 0;
		for (String k : contract.keySet()) {
			if (k.length() > maxKeyLen) {
				maxKeyLen = k.length();
			}
		}
		for (int cp = 0; cp <= 0xFFFF; cp++) {
			if (cp >= 0xD800 && cp <= 0xDFFF) {
				continue;
			}
			String s = new String(Character.toChars(cp));
			String actual = dump.get(Integer.valueOf(cp));
			String model = modelCE(s, dump, contract, maxKeyLen);
			checked++;
			if (!actual.equals(model)) {
				mismatched++;
				if (mismatched <= 400) {
					mW.println(String.format("MISMATCH U+%04X actual=[%s] model=[%s] nfd=%s",
							cp, actual, model, cps(Normalizer.normalize(s,
									Normalizer.Form.NFD))));
				}
			}
		}
		mW.println(String.format(
				"checked=%d mismatched=%d contractions=%d maxKeyLen=%d nfdCount=%d",
				checked, mismatched, contract.size(), maxKeyLen, nfdCount));
		mW.close();

		// ---- 3b) multi-char spot checks: adjacent pairs over key ranges ----
		int pairBad = 0, pairChecked = 0;
		StringBuilder pBad = new StringBuilder();
		List<Integer> interest = new ArrayList<Integer>();
		for (int cp = 0x20; cp <= 0x7E; cp++) {
			interest.add(Integer.valueOf(cp));
		}
		int[] marks = { 0x300, 0x301, 0x302, 0x303, 0x304, 0x306, 0x307, 0x308,
				0x30A, 0x30B, 0x30C, 0x327, 0x328, 0x340, 0x341, 0x344, 0x345 };
		for (int m : marks) {
			interest.add(Integer.valueOf(m));
		}
		// sample of listed CJK + latin-1 accented
		for (int cp = 0xC0; cp <= 0x17F; cp++) {
			interest.add(Integer.valueOf(cp));
		}
		for (int cp = 0x4E00; cp <= 0x9FA5; cp += 0x37) {
			interest.add(Integer.valueOf(cp));
		}
		for (Integer i1 : interest) {
			for (Integer i2 : interest) {
				String s = new String(Character.toChars(i1.intValue()))
						+ new String(Character.toChars(i2.intValue()));
				String actual = ceOf(c, s);
				String model = modelCE(s, dump, contract, maxKeyLen);
				pairChecked++;
				if (!actual.equals(model)) {
					pairBad++;
					if (pairBad <= 20) {
						pBad.append(String.format("PAIR BAD [%s]: actual=[%s] model=[%s]%n",
								cps(s), actual, model));
					}
				}
			}
		}
		PrintWriter pW = new PrintWriter(new OutputStreamWriter(
				new FileOutputStream(outDir + "/_collation_model.txt", true),
				"UTF-8"));
		pW.print(pBad.toString());
		pW.println(String.format("pairCheck: checked=%d mismatched=%d (interestSize=%d)",
				pairChecked, pairBad, interest.size()));
		pW.close();

		// ---- 4) ccc via reflection (best effort) ----
		int cccDumped = 0;
		try {
			Class<?> nz = Class.forName("sun.text.Normalizer");
			java.lang.reflect.Method m = nz.getDeclaredMethod(
					"getCombiningClass", int.class);
			m.setAccessible(true);
			PrintWriter w = new PrintWriter(new OutputStreamWriter(
					new FileOutputStream(outDir + "/_collation_ccc.tsv"),
					"UTF-8"));
			for (int cp = 0; cp <= 0xFFFF; cp++) {
				int cc = ((Integer) m.invoke(null, Integer.valueOf(cp)))
						.intValue();
				if (cc != 0) {
					w.println(String.format("%04X\t%d", cp, cc));
					cccDumped++;
				}
			}
			w.close();
		} catch (Throwable t) {
			System.out.println("ccc reflection failed: " + t);
		}

		System.out.println("model summary: checked=" + checked + " mismatched="
				+ mismatched + " contractions=" + contract.size()
				+ " maxKeyLen=" + maxKeyLen + " pairBad=" + pairBad
				+ " pairChecked=" + pairChecked + " ccc=" + cccDumped);
	}

	/** The port model: NFD -> greedy longest contraction -> per-char table.
	 *  Completely-ignorable chars (empirically: 12 BMP code points) emit no CE. */
	private static final java.util.HashSet<Integer> IGNORABLE = new java.util.HashSet<Integer>(
			java.util.Arrays.asList(Integer.valueOf(0x0000), Integer.valueOf(0x0001),
					Integer.valueOf(0x0300), Integer.valueOf(0x0301), Integer.valueOf(0x0302),
					Integer.valueOf(0x0303), Integer.valueOf(0x0308), Integer.valueOf(0x030A),
					Integer.valueOf(0x0327), Integer.valueOf(0x0340), Integer.valueOf(0x0341),
					Integer.valueOf(0x0344)));

	private static String modelCE(String s, Map<Integer, String> dump,
			Map<String, String> contract, int maxKeyLen) {
		String nfd = Normalizer.normalize(s, Normalizer.Form.NFD);
		StringBuilder sb = new StringBuilder();
		int i = 0;
		while (i < nfd.length()) {
			// greedy longest contraction match
			boolean hit = false;
			int hi = Math.min(maxKeyLen, nfd.length() - i);
			for (int L = hi; L >= 2; L--) {
				String key = nfd.substring(i, i + L);
				String ce = contract.get(key);
				if (ce != null) {
					append(sb, ce);
					i += L;
					hit = true;
					break;
				}
			}
			if (hit) {
				continue;
			}
			char ch = nfd.charAt(i);
			if (IGNORABLE.contains(Integer.valueOf((int) ch))) {
				i++;
				continue;
			}
			String d = dump.get(Integer.valueOf((int) ch));
			if (d != null && d.length() > 0) {
				append(sb, d);
			} else {
				append(sb, String.format("7FFF/0000/0000,%04X/0000/0000",
						(int) ch));
			}
			i++;
		}
		return sb.toString();
	}

	private static void append(StringBuilder sb, String seq) {
		if (sb.length() > 0) {
			sb.append(',');
		}
		sb.append(seq);
	}

	private static String ceOf(RuleBasedCollator c, String s) {
		CollationElementIterator it = c.getCollationElementIterator(s);
		StringBuilder sb = new StringBuilder();
		while (true) {
			int ce = it.next();
			if (ce == CollationElementIterator.NULLORDER) {
				break;
			}
			if (sb.length() > 0) {
				sb.append(',');
			}
			sb.append(String.format("%04X/%04X/%04X",
					CollationElementIterator.primaryOrder(ce),
					CollationElementIterator.secondaryOrder(ce),
					CollationElementIterator.tertiaryOrder(ce)));
		}
		return sb.toString();
	}

	private static String cps(String s) {
		StringBuilder sb = new StringBuilder();
		for (int i = 0; i < s.length(); i++) {
			if (i > 0) {
				sb.append(' ');
			}
			sb.append(String.format("%04X", (int) s.charAt(i)));
		}
		return sb.toString();
	}
}
