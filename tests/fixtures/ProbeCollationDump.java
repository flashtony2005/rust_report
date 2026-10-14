package rpx.fixtures;

import com.raqsoft.commonl.ChineseComparator;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.text.CollationElementIterator;
import java.text.RuleBasedCollator;

/**
 * Dumps the collation-element sequence for every BMP code point (and a
 * sample of supplementary planes) as produced by the product's
 * ChineseComparator (rule.txt from reportlite.jar).
 *
 * Output: TSV, one line per code point:  <hex cp>\t<p/s/t triplets>
 * The triplet sequence is comma-separated, e.g. "0041/0000/0000".
 * Ignorable code points produce an empty sequence.
 *
 * Purpose: cross-check the "weight = 2 + chain position" hypothesis
 * against a from-scratch parse of rule.txt, and provide golden data for
 * the Rust port (M2-12 Text-Text).
 */
public class ProbeCollationDump {

	public static void main(String[] args) throws Exception {
		String out = args.length > 0 ? args[0] : "_collation_dump.tsv";
		RuleBasedCollator c = (RuleBasedCollator) ChineseComparator
				.getInstance();
		PrintWriter w = new PrintWriter(new OutputStreamWriter(
				new FileOutputStream(out), "UTF-8"));
		int n = 0;
		for (int cp = 0; cp <= 0xFFFF; cp++) {
			if (cp >= 0xD800 && cp <= 0xDFFF) {
				continue; // lone surrogates are not valid input
			}
			w.println(String.format("%04X\t%s", cp, dump(c, cp)));
			n++;
		}
		// supplementary: sample every 0x200 across planes 1..3
		for (int cp = 0x10000; cp <= 0x3FFFF; cp += 0x200) {
			if (cp > 0x10FFFF) {
				break;
			}
			w.println(String.format("%06X\t%s", cp, dump(c, cp)));
			n++;
		}
		w.close();
		System.out.println("dumped " + n + " code points to " + out);
	}

	private static String dump(RuleBasedCollator c, int cp) {
		String s = new String(Character.toChars(cp));
		CollationElementIterator it = c.getCollationElementIterator(s);
		StringBuilder sb = new StringBuilder();
		int k = 0;
		while (true) {
			int ce = it.next();
			if (ce == CollationElementIterator.NULLORDER) {
				break;
			}
			if (k > 0) {
				sb.append(',');
			}
			sb.append(String.format("%04X/%04X/%04X",
					CollationElementIterator.primaryOrder(ce),
					CollationElementIterator.secondaryOrder(ce),
					CollationElementIterator.tertiaryOrder(ce)));
			k++;
		}
		return sb.toString();
	}
}
