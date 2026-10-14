package rpx.fixtures;

import com.raqsoft.commonl.ChineseComparator;
import java.text.CollationElementIterator;
import java.text.RuleBasedCollator;

/**
 * Settles two behavioral questions for the collation port:
 *  1) a completely-ignorable char returns -1 from next(): does iteration
 *     STOP there (all following chars unobservable) or skip and continue?
 *  2) does compare() treat the -1 stop as \"end of stream\"?
 * Prints raw next() values (including -1) with call indexes.
 */
public class ProbeCollationQuirk {

	public static void main(String[] args) throws Exception {
		RuleBasedCollator c = (RuleBasedCollator) ChineseComparator
				.getInstance();

		raw(c, "\u0300 "); // ignorable then space
		raw(c, "\u0300Z");
		raw(c, "a\u0300z");
		raw(c, "\u0000\u0001");
		raw(c, "\u0000X");
		raw(c, "\u0001\u0000");
		raw(c, "\u0300\u0300A");
		raw(c, "A\u0300B"); // contraction consumes the mark
		raw(c, "\u0344A"); // NFD -> 0308 0301
		raw(c, "\u1F82"); // 03B1 0313 0300 0345

		System.out.println("== compare quirks ==");
		cmp(c, "a\u0300z", "az");
		cmp(c, "a\u0300z", "a");
		cmp(c, "a\u0300z", "a\u0300z");
		cmp(c, "a\u0301b", "ab");
		cmp(c, "a\u0300b", "a\u0301b");
		cmp(c, "\u0300", "");
		cmp(c, "\u0300A", "A");
		cmp(c, "\u0000\u0001", "\u0001");
	}

	private static void raw(RuleBasedCollator c, String s) {
		CollationElementIterator it = c.getCollationElementIterator(s);
		StringBuilder sb = new StringBuilder();
		for (int i = 0; i < 8; i++) {
			int ce = it.next();
			sb.append(String.format(" #%d=%s", i, ce == -1 ? "-1(NULL?)" : String
					.format("p%04X",
							CollationElementIterator.primaryOrder(ce))));
			if (ce == -1) {
				continue; // keep calling to see whether iteration resumes
			}
		}
		System.out.println("RAW [" + esc(s) + "]" + sb);
	}

	private static void cmp(RuleBasedCollator c, String a, String b) {
		int r = c.compare(a, b);
		System.out.println("CMP [" + esc(a) + "] vs [" + esc(b) + "] = " + r);
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
