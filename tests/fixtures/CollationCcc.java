package rpx.fixtures;

import java.lang.reflect.Method;
import java.text.Normalizer;
import java.util.ArrayList;
import java.util.List;

/**
 * Canonical combining class (ccc) acquisition for {@link CollationData}.
 *
 * Strategy: reflect the product JRE's own table first; only if that fails,
 * derive an order-isomorphic re-encoding from black-box NFD behaviour. The
 * Rust port only needs the ccc *ordering*, not the literal UCD numbers, so
 * either source is sufficient evidence.
 *
 * Split out of CollationData purely to keep every file under the 300-line
 * gate -- it has no coupling to the rest of the extraction.
 */
final class CollationCcc {

	private CollationCcc() {
	}

	/** `getCanonicalClass` on JDK≥9 internals, `getCombiningClass` on JDK 8. */
	static Method resolve() {
		String[][] tries = {
				{ "sun.text.normalizer.NormalizerImpl", "getCanonicalClass" },
				{ "sun.text.normalizer.NormalizerImpl", "getCombiningClass" },
				{ "sun.text.Normalizer", "getCanonicalClass" },
				{ "sun.text.Normalizer", "getCombiningClass" } };
		for (String[] t : tries) {
			try {
				Class<?> k = Class.forName(t[0]);
				Method m = k.getDeclaredMethod(t[1], int.class);
				m.setAccessible(true);
				((Integer) m.invoke(null, Integer.valueOf(0x0301))).intValue();
				return m;
			} catch (Throwable ignore) {
				// try the next spelling
			}
		}
		return null;
	}

	/**
	 * Black-box ccc: for non-decomposing combining marks, `ccc(a) > ccc(b)`
	 * iff NFD(a+b) swaps them (canonical ordering only swaps strictly
	 * descending non-zero classes). Rank(cp) = 1 + #{ q : ccc(cp) > ccc(q) }
	 * -- an order-isomorphic re-encoding, which is all the reorder pass needs.
	 */
	static void deriveBlackBox(List<Integer> outCp, List<Integer> outVal) {
		List<Integer> cand = new ArrayList<Integer>();
		for (int cp = 0; cp <= 0xFFFF; cp++) {
			if (CollationData.isSurrogate(cp)) {
				continue;
			}
			int t = Character.getType(cp);
			if (t != Character.NON_SPACING_MARK && t != Character.ENCLOSING_MARK) {
				continue;
			}
			String s = new String(Character.toChars(cp));
			if (s.equals(Normalizer.normalize(s, Normalizer.Form.NFD))) {
				cand.add(Integer.valueOf(cp));
			}
		}
		for (int i = 0; i < cand.size(); i++) {
			int cp = cand.get(i).intValue();
			int rank = 1;
			for (int j = 0; j < cand.size(); j++) {
				if (i != j && swaps(cp, cand.get(j).intValue())) {
					rank++;
				}
			}
			outCp.add(Integer.valueOf(cp));
			outVal.add(Integer.valueOf(Math.min(rank, 254)));
		}
	}

	/** Canonical ordering swapped `a b` into `b a`? (true ⇒ ccc(a) > ccc(b) > 0) */
	private static boolean swaps(int a, int b) {
		String ab = str(a) + str(b);
		return !Normalizer.normalize(ab, Normalizer.Form.NFD).equals(ab);
	}

	private static String str(int cp) {
		return new String(Character.toChars(cp));
	}
}
