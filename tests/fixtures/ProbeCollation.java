package rpx.fixtures;

import com.raqsoft.commonl.ChineseComparator;
import java.text.CollationElementIterator;
import java.text.RuleBasedCollator;
import java.util.Arrays;

/**
 * Probe for the collation gap (M2-12 Text-Text).
 *
 * ChineseComparator loads com/raqsoft/commonl/rule.txt from the product jar
 * and builds a java.text.RuleBasedCollator. This probe records:
 *   1) the class's own sample sort,
 *   2) key pair comparisons (case / pinyin / unlisted chars / supplementary),
 *   3) collation-element dumps for representative single chars,
 *   4) collator configuration (strength / decomposition mode).
 *
 * All findings feed docs/ + the Rust port design. Output goes to stdout.
 */
public class ProbeCollation {

	public static void main(String[] args) throws Exception {
		RuleBasedCollator c = (RuleBasedCollator) ChineseComparator.getInstance();

		System.out.println("== config ==");
		System.out.println("strength      = " + c.getStrength()
				+ " (0=PRI 1=SEC 2=TER 3=IDEN)");
		System.out.println("decomposition = " + c.getDecomposition()
				+ " (0=OFF 1=CANON 2=FULL)");

		System.out.println("== sample sort (class main) ==");
		String[] arr = { "\u5317  \u4eac", "\u4e0a\u6d77",
				"\u5e7f\u5dde", "abc", "\u5317 \u4eac", "bd", "ABC",
				"AB C", "BD" };
		Arrays.sort(arr, c);
		for (String s : arr) {
			System.out.println("SORT: [" + s + "]");
		}

		System.out.println("== key pairs ==");
		pair(c, "A", "a");
		pair(c, "a", "B");
		pair(c, "a", "A");
		pair(c, "abc", "ABC");
		pair(c, "AB C", "ABC");
		pair(c, "\u5317\u4eac", "\u4e0a\u6d77"); // 北京 vs 上海
		pair(c, "\u4e00", "\u4e59"); // 一 vs 乙
		pair(c, "\u963f", "\u554a"); // 阿 vs 啊
		pair(c, "A", "\u4e2d"); // listed ASCII vs listed CJK
		pair(c, "a", "\u4e2d");
		pair(c, "\u4e2d", "A");
		pair(c, "", "A");
		pair(c, "", "\u4e2d");

		// unlisted chars: private use, emoji (supplementary), rare plane-2
		System.out.println("== unlisted vs listed ==");
		pair(c, "\uE000", "\u4e2d"); // PUA  vs 中
		pair(c, "\uE000", "A");
		pair(c, "\uE000", "z");
		pair(c, "\uE000", "\uE001");
		pair(c, "\uD840\uDC00", "\uE000"); // U+20000 vs PUA
		pair(c, "\uD83D\uDE00", "\uE000"); // U+1F600 vs PUA
		pair(c, "\u9FFF", "\uE000"); // U+9FFF vs PUA
		// decomposition check: A + combining ring vs precomposed
		pair(c, "\u00C5", "A\u030A"); // Å vs A + U+030A

		System.out.println("== collation elements ==");
		ce(c, "A");
		ce(c, "a");
		ce(c, "0");
		ce(c, " ");
		ce(c, "\u4e2d");
		ce(c, "\u4e00");
		ce(c, "\uE000");
		ce(c, "\u9FFF");
		ce(c, "\uD840\uDC00");
		ce(c, "\uD83D\uDE00");
		ce(c, "A\u030A");
		ce(c, "\u00C5");

		System.out.println("== weights spread (sanity) ==");
		// spot-print primary weights of first/last listed elements inferred
		// via CE dump of boundary chars
		ce(c, "\u0000");
		ce(c, "\u0001");
		ce(c, " ");
		ce(c, "~");
		ce(c, "\uE7E5"); // plain char used by earlier hex dump (kept as-is)
		System.out.println("done");
	}

	private static void pair(RuleBasedCollator c, String a, String b) {
		int r = c.compare(a, b);
		System.out.println("CMP [" + esc(a) + "] vs [" + esc(b) + "] = " + r
				+ (r < 0 ? "  (<)" : r > 0 ? "  (>)" : "  (=)"));
	}

	private static void ce(RuleBasedCollator c, String s) {
		CollationElementIterator it = c.getCollationElementIterator(s);
		StringBuilder sb = new StringBuilder();
		int n = 0;
		while (true) {
			int ce = it.next();
			if (ce == CollationElementIterator.NULLORDER) {
				break;
			}
			n++;
			sb.append(String.format(" [p=0x%04X s=0x%04X t=0x%04X]",
					CollationElementIterator.primaryOrder(ce),
					CollationElementIterator.secondaryOrder(ce),
					CollationElementIterator.tertiaryOrder(ce)));
			if (n > 12) {
				sb.append(" ...");
				break;
			}
		}
		System.out.println("CE  [" + esc(s) + "] ->" + sb + "  (count=" + n
				+ ")");
	}

	private static String esc(String s) {
		StringBuilder sb = new StringBuilder();
		for (int i = 0; i < s.length(); i++) {
			char ch = s.charAt(i);
			if (ch < 0x20 || ch > 0x7E) {
				sb.append(String.format("\\u%04X", (int) ch));
			} else {
				sb.append(ch);
			}
		}
		return sb.toString();
	}
}
