package rpx.fixtures;

import com.raqsoft.commonl.ChineseComparator;
import java.io.BufferedReader;
import java.io.FileInputStream;
import java.io.InputStreamReader;
import java.lang.reflect.Field;
import java.text.CollationElementIterator;
import java.text.Normalizer;
import java.text.RuleBasedCollator;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.Vector;

/**
 * Model v2 for the collation port (M2-12 Text-Text), validated against the
 * product's real behavior.
 *
 * Model (matches sun.text/java.text RBC + rule.txt):
 *   CE(text): walk over NFD(text) units:
 *     - contract char: try greedy longest fwd entry match (2-char keys);
 *       a) matched -> emit probed CE, advance key length
 *       b) else terminal == -1 (ignorable) -> TRUNCATE stream
 *       c) else emit terminal CE, advance 1
 *     - other char: single(W) -> emit; else -> emit [7FFF],[ch]
 *
 * The three self-checks (BMP vs dump, interest pairs vs live iterator, fuzz
 * CE + fuzz compare) and the consolidated-table export live in
 * {@link CollationSelfCheck}; this file holds the shared state and the model
 * itself. The split exists to keep every file under the 300-line gate.
 */
public class ProbeCollationModel {

	static RuleBasedCollator collator;
	static Map<Integer, String> dump = new HashMap<Integer, String>();
	static Map<Integer, Contract> contracts = new HashMap<Integer, Contract>();
	static Map<String, String> entryCE = new HashMap<String, String>();
	static int maxKeyLen = 1;

	static class Contract {
		boolean ignorable;
		String terminal; // CE string, null when ignorable
		List<String> keys = new ArrayList<String>(); // fwd entries, len >= 2
	}

	public static void main(String[] args) throws Exception {
		String outDir = args.length > 1 ? args[1] : ".";
		collator = (RuleBasedCollator) ChineseComparator.getInstance();
		readDump(args[0]);
		buildContracts();
		CollationSelfCheck.run(outDir);
	}

	// ================= model =================

	static String modelCE(String s) {
		String nfd = Normalizer.normalize(s, Normalizer.Form.NFD);
		StringBuilder sb = new StringBuilder();
		int i = 0;
		while (i < nfd.length()) {
			int cp = nfd.codePointAt(i);
			int adv = Character.charCount(cp);
			if (cp > 0xFFFF) {
				// supplementary: [7FFF],[hi],[lo] (Normalizer yields code points)
				append(sb, String.format("7FFF/0000/0000,%04X/0000/0000,%04X/0000/0000",
						(int) Character.highSurrogate(cp), (int) Character.lowSurrogate(cp)));
				i += adv;
				continue;
			}
			char ch = (char) cp;
			Contract ct = contracts.get(Integer.valueOf((int) ch));
			if (ct != null) {
				// greedy longest fwd entry match
				String hit = null;
				int hi = Math.min(maxKeyLen, nfd.length() - i);
				for (int L = hi; L >= 2; L--) {
					String key = nfd.substring(i, i + L);
					if (ct.keys.contains(key)) {
						hit = key;
						break;
					}
				}
				if (hit != null) {
					append(sb, entryCE.get(hit));
					i += hit.length();
					continue;
				}
				if (ct.ignorable) {
					return sb.toString(); // truncate
				}
				append(sb, ct.terminal);
				i++;
				continue;
			}
			String d = dump.get(Integer.valueOf((int) ch));
			if (d == null || d.length() == 0) {
				return sb.toString(); // empty dump = truncate (defensive)
			}
			append(sb, d); // stable unit: single(W) or 7FFF-pair, self-contained
			i++;
		}
		return sb.toString();
	}

	static int modelCompare(String a, String b) {
		List<int[]> ca = parseCE(modelCE(a));
		List<int[]> cb = parseCE(modelCE(b));
		int n = Math.min(ca.size(), cb.size());
		for (int i = 0; i < n; i++) {
			int[] x = ca.get(i), y = cb.get(i);
			for (int lv = 0; lv < 3; lv++) {
				if (x[lv] != y[lv]) {
					return x[lv] < y[lv] ? -1 : 1;
				}
			}
		}
		if (ca.size() != cb.size()) {
			return ca.size() < cb.size() ? -1 : 1;
		}
		return 0;
	}

	static List<int[]> parseCE(String seq) {
		List<int[]> out = new ArrayList<int[]>();
		if (seq == null || seq.length() == 0) {
			return out;
		}
		for (String part : seq.split(",")) {
			String[] t = part.split("/");
			out.add(new int[] { Integer.parseInt(t[0], 16),
					Integer.parseInt(t[1], 16),
					Integer.parseInt(t[2], 16) });
		}
		return out;
	}

	// ================= helpers =================

	static void readDump(String path) throws Exception {
		BufferedReader r = new BufferedReader(new InputStreamReader(
				new FileInputStream(path), "UTF-8"));
		String ln;
		while ((ln = r.readLine()) != null) {
			String[] p = ln.split("\t", -1);
			dump.put(Integer.valueOf(Integer.parseInt(p[0], 16)), p.length > 1 ? p[1] : "");
		}
		r.close();
	}

	@SuppressWarnings("unchecked")
	static void buildContracts() throws Exception {
		Field f = RuleBasedCollator.class.getDeclaredField("tables");
		f.setAccessible(true);
		Object tables = f.get(collator);
		Field cf = tables.getClass().getDeclaredField("contractTable");
		cf.setAccessible(true);
		Vector<Object> ct = (Vector<Object>) cf.get(tables);
		Field nameF = null, valueF = null, fwdF = null;
		for (Object listObj : ct) {
			Vector<Object> list = (Vector<Object>) listObj;
			if (list.size() == 0) {
				continue;
			}
			Object first = list.elementAt(0);
			if (nameF == null) {
				nameF = first.getClass().getDeclaredField("entryName");
				valueF = first.getClass().getDeclaredField("value");
				fwdF = first.getClass().getDeclaredField("fwd");
				nameF.setAccessible(true);
				valueF.setAccessible(true);
				fwdF.setAccessible(true);
			}
			String fn = (String) nameF.get(first);
			int fv = ((Integer) valueF.get(first)).intValue();
			int fcp = fn.charAt(0);
			Contract c = new Contract();
			contracts.put(Integer.valueOf(fcp), c);
			if (fv == -1) {
				c.ignorable = true;
			} else {
				// terminal CE = the public single-char CE (dump)
				String d = dump.get(Integer.valueOf(fcp));
				c.terminal = (d == null || d.length() == 0) ? "" : d;
			}
			for (int j = 1; j < list.size(); j++) {
				Object ep = list.elementAt(j);
				String en = (String) nameF.get(ep);
				boolean fwd = fwdF == null ? true : ((Boolean) fwdF.get(ep)).booleanValue();
				if (!fwd || en.length() < 2) {
					continue;
				}
				c.keys.add(en);
				if (en.length() > maxKeyLen) {
					maxKeyLen = en.length();
				}
				entryCE.put(en, ceOf(en));
			}
		}
	}

	static String ceOf(String s) {
		CollationElementIterator it = collator.getCollationElementIterator(s);
		StringBuilder sb = new StringBuilder();
		while (true) {
			int ce = it.next();
			if (ce == -1) {
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

	static void append(StringBuilder sb, String seq) {
		if (seq == null || seq.length() == 0) {
			return;
		}
		if (sb.length() > 0) {
			sb.append(',');
		}
		sb.append(seq);
	}

	static int sign(int v) {
		return v < 0 ? -1 : v > 0 ? 1 : 0;
	}

	static String randStr(Random r, int[] pools) {
		int len = r.nextInt(7);
		StringBuilder sb = new StringBuilder();
		for (int i = 0; i < len; i++) {
			int seg = (r.nextInt(pools.length / 2)) * 2;
			int lo = pools[seg], hi = pools[seg + 1];
			int cp = lo + r.nextInt(hi - lo + 1);
			sb.appendCodePoint(cp);
		}
		return sb.toString();
	}

	static String cps(String s) {
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
