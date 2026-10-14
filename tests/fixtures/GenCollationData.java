package rpx.fixtures;

import com.raqsoft.commonl.ChineseComparator;
import java.io.BufferedReader;
import java.io.ByteArrayOutputStream;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.text.RuleBasedCollator;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * M2-12 Text-Text: builds the Rust-side collation data table + goldens.
 *
 * Reproduces (does not re-implement) the validated model by calling
 * {@link ProbeCollationModel#main}: that probe cross-checks its CE model
 * against the live product collator over 63,488 BMP single chars, 595,984
 * interest pairs, 30,000 fuzz CE strings and 8,000 fuzz compares. We re-run
 * it, require every `mismatched` to be 0, then add the **supplementary
 * plane** (384 dumped code points) on top.
 *
 * On any failure it prints the counterexample and exits non-zero WITHOUT
 * writing collation.bin.
 *
 * The golden/summary writers live in {@link CollationGoldens}; the table
 * extraction and its invariants live in {@link CollationData}.
 *
 * Usage:
 *   java rpx.fixtures.GenCollationData &lt;dump.tsv&gt; &lt;fixturesDir&gt; &lt;bin&gt;
 */
public class GenCollationData {

	/** Cross-check totals the model must reproduce (from ProbeCollationModel). */
	private static final String[] EXPECT = { "bmp=0/63488", "pairs=0/595984",
			"fuzzCE=0/30000", "fuzzCMP=0/8000" };
	private static final Pattern BAD = Pattern.compile("(\\w+)=(\\d+)/(\\d+)");

	private static String dumpPath;
	private static String fixturesDir;
	private static String binPath;

	public static void main(String[] args) throws Exception {
		dumpPath = args.length > 0 ? args[0] : "tests/fixtures/_collation_dump.tsv";
		fixturesDir = args.length > 1 ? args[1] : "tests/fixtures";
		binPath = args.length > 2 ? args[2]
				: "crates/rpx-engine/data/collation.bin";
		try {
			run();
		} catch (Throwable t) {
			System.err.println("GenCollationData FAILED: " + t);
			t.printStackTrace();
			System.exit(1);
		}
	}

	private static void run() throws Exception {
		RuleBasedCollator collator = (RuleBasedCollator) ChineseComparator
				.getInstance();
		if (collator == null) {
			throw new IllegalStateException("no product collator");
		}

		// ---- 1) re-run the validated model + full cross-check ----
		String probeOut = runProbe();
		System.out.print(probeOut);
		String summary = readSummaryLine();
		for (String tok : EXPECT) {
			if (!summary.contains(tok)) {
				throw new IllegalStateException(
						"cross-check failed; want `" + tok + "` in [" + summary
								+ "]\n" + probeOut);
			}
		}
		Matcher mm = BAD.matcher(summary);
		while (mm.find()) {
			if (!"0".equals(mm.group(2))) {
				throw new IllegalStateException(
						"cross-check mismatch: " + mm.group());
			}
		}

		Map<Integer, String> dump = ProbeCollationModel.dump;
		Map<Integer, ProbeCollationModel.Contract> contracts =
				ProbeCollationModel.contracts;

		// ---- 2) CE dump: purity + grand total ----
		String ceNote = checkCeDump();

		// ---- 3) supplementary plane (the new coverage) ----
		int supChecked = 0, supBad = 0;
		for (Map.Entry<Integer, String> e : dump.entrySet()) {
			int cp = e.getKey().intValue();
			if (cp <= 0xFFFF) {
				continue;
			}
			supChecked++;
			String model = ProbeCollationModel
					.modelCE(new String(Character.toChars(cp)));
			if (!e.getValue().equals(model)) {
				supBad++;
				if (supBad <= 20) {
					System.err.println(String.format(
							"SUPPLEMENTARY BAD U+%X actual=[%s] model=[%s]", cp,
							e.getValue(), model));
				}
			}
		}
		if (supChecked == 0 || supBad != 0) {
			throw new IllegalStateException("supplementary cross-check: checked="
					+ supChecked + " mismatched=" + supBad);
		}

		// ---- 4) extract tables + invariants ----
		CollationData d = CollationData.extract(dump, contracts,
				ProbeCollationModel.entryCE);
		d.assertInvariants();
		checkReferences(d);

		// ---- 5) write the binary ----
		byte[] bin = CollationBin.write(d);
		Files.createDirectories(Paths.get(binPath).getParent());
		FileOutputStream fo = new FileOutputStream(binPath);
		fo.write(bin);
		fo.close();
		String sha = CollationGoldens.sha256(bin);

		// ---- 6) goldens + summary ----
		int nfdRows = CollationGoldens.writeNfdGolden(
				fixturesDir + "/_collation_nfd_golden.tsv", d);
		String contractNote = CollationGoldens.writeContractGolden(
				fixturesDir + "/_collation_contract_golden.tsv", d, dump);
		CollationGoldens.writeSummary(fixturesDir + "/_collation_summary.txt", d,
				binPath, summary, ceNote, supChecked, nfdRows, contractNote, sha,
				bin.length);

		// ---- 7) one-line report ----
		StringBuilder counts = new StringBuilder();
		for (int i = 0; i < CollationData.SEC_CNT; i++) {
			if (i > 0) {
				counts.append(' ');
			}
			counts.append(i).append('=').append(d.section(i).length);
		}
		System.out.println("GEN OK sections[" + counts + "]");
		System.out.println("GEN OK cross-check: " + summary);
		System.out.println("GEN OK supplementary: checked=" + supChecked
				+ " mismatched=0");
		System.out.println("GEN OK " + ceNote);
		System.out.println("GEN OK ccc: " + d.cccHow);
		System.out.println("GEN OK nfd golden rows=" + nfdRows
				+ " | contract " + contractNote);
		System.out.println("GEN OK bin bytes=" + bin.length + " sha256=" + sha);
	}

	// =================================================================

	/** Runs the validated probe, capturing its stdout, and return it. */
	private static String runProbe() throws Exception {
		PrintStream old = System.out;
		ByteArrayOutputStream cap = new ByteArrayOutputStream();
		String text;
		try {
			System.setOut(new PrintStream(cap, true, "UTF-8"));
			ProbeCollationModel.main(new String[] { dumpPath, fixturesDir });
			System.out.flush();
		} finally {
			System.setOut(old);
		}
		text = cap.toString("UTF-8");
		return text;
	}

	private static String readSummaryLine() throws Exception {
		BufferedReader r = new BufferedReader(new InputStreamReader(
				new FileInputStream(fixturesDir + "/_collation_tables/model_check.txt"),
				"UTF-8"));
		String ln, hit = null;
		while ((ln = r.readLine()) != null) {
			if (ln.startsWith("SUMMARY ")) {
				hit = ln;
			}
		}
		r.close();
		if (hit == null) {
			throw new IllegalStateException("no SUMMARY line in model_check.txt");
		}
		return hit;
	}

	/** All 151,485 CEs are pure-primary; 384 supplementary rows are dumped. */
	private static String checkCeDump() throws Exception {
		BufferedReader r = new BufferedReader(new InputStreamReader(
				new FileInputStream(dumpPath), "UTF-8"));
		String ln;
		long total = 0, sup = 0, rows = 0;
		while ((ln = r.readLine()) != null) {
			if (ln.length() == 0) {
				continue;
			}
			rows++;
			String[] p = ln.split("\t", -1);
			if (p[0].length() > 4) {
				sup++;
			}
			if (p.length < 2 || p[1].length() == 0) {
				continue;
			}
			for (String ce : p[1].split(",")) {
				String[] t = ce.split("/");
				total++;
				if (!"0000".equals(t[1]) || !"0000".equals(t[2])) {
					throw new IllegalStateException("non-primary CE at " + p[0]
							+ ": " + ce);
				}
			}
		}
		r.close();
		if (total != 151485 || sup != 384 || rows != 63872) {
			throw new IllegalStateException("CE dump totals: rows=" + rows
					+ " ce=" + total + " supplementary=" + sup);
		}
		return "CE dump: rows=63872 ce=151485 supplementary=384"
				+ " (all secondary/tertiary == 0)";
	}

	/** Three published reference ranks must come out of the CJK table. */
	private static void checkReferences(CollationData d) {
		int[] cjk = d.section(0);
		int a = cjk[0x963F - CollationData.CJK_LO];
		int yi = cjk[0x4E00 - CollationData.CJK_LO];
		int zhang = cjk[0x5F20 - CollationData.CJK_LO];
		if (a != 294 || yi != 18183 || zhang != 19926) {
			throw new IllegalStateException("reference ranks: 阿=" + a + " 一="
					+ yi + " 张=" + zhang);
		}
	}
}
