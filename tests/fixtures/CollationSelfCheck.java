package rpx.fixtures;

import static rpx.fixtures.ProbeCollationModel.dump;
import static rpx.fixtures.ProbeCollationModel.contracts;
import static rpx.fixtures.ProbeCollationModel.entryCE;
import static rpx.fixtures.ProbeCollationModel.collator;
import static rpx.fixtures.ProbeCollationModel.modelCE;
import static rpx.fixtures.ProbeCollationModel.modelCompare;
import static rpx.fixtures.ProbeCollationModel.ceOf;
import static rpx.fixtures.ProbeCollationModel.cps;
import static rpx.fixtures.ProbeCollationModel.randStr;
import static rpx.fixtures.ProbeCollationModel.sign;

import java.io.File;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Random;

/**
 * The three cross-checks + the consolidated-table export of
 * {@link ProbeCollationModel}, split out purely to keep every file under the
 * 300-line gate.
 *
 * The model's live state (`dump` / `contracts` / `entryCE` / `collator`) and the
 * model functions are reached through static imports, so this file is a
 * verbatim lift of the former `ProbeCollationModel.main` body.
 */
final class CollationSelfCheck {

	private CollationSelfCheck() {
	}

	static void run(String outDir) throws Exception {
		// ---------- check 1: all BMP vs dump ----------
		int checked = 0, mism;
		mism = 0;
		StringBuilder bad = new StringBuilder();
		for (int cp = 0; cp <= 0xFFFF; cp++) {
			if (cp >= 0xD800 && cp <= 0xDFFF) {
				continue;
			}
			String s = new String(Character.toChars(cp));
			String actual = dump.get(Integer.valueOf(cp));
			String model = modelCE(s);
			checked++;
			if (!actual.equals(model)) {
				mism++;
				if (mism <= 25) {
					bad.append(String.format("BMP BAD U+%04X actual=[%s] model=[%s]%n", cp, actual, model));
				}
			}
		}
		System.out.println("check1 BMP: checked=" + checked + " mismatched=" + mism);

		// ---------- check 2: interest pairs vs live iterator ----------
		List<Integer> interest = new ArrayList<Integer>();
		for (int cp = 0x20; cp <= 0x7E; cp++) {
			interest.add(Integer.valueOf(cp));
		}
		interest.add(Integer.valueOf(0));
		interest.add(Integer.valueOf(1));
		for (int cp = 0x300; cp <= 0x345; cp++) {
			interest.add(Integer.valueOf(cp));
		}
		for (int cp = 0xC0; cp <= 0x17F; cp++) {
			interest.add(Integer.valueOf(cp));
		}
		for (int cp = 0x1E00; cp <= 0x1EFF; cp += 3) {
			interest.add(Integer.valueOf(cp));
		}
		for (int cp = 0x386; cp <= 0x3CE; cp++) {
			interest.add(Integer.valueOf(cp));
		}
		for (int cp = 0x4E00; cp <= 0x9FA5; cp += 0x53) {
			interest.add(Integer.valueOf(cp));
		}
		interest.add(Integer.valueOf(0x9FFF));
		interest.add(Integer.valueOf(0xE000));

		int pairChecked = 0, pairBad = 0;
		StringBuilder pBad = new StringBuilder();
		for (Integer i1 : interest) {
			for (Integer i2 : interest) {
				String s = new String(Character.toChars(i1.intValue()))
						+ new String(Character.toChars(i2.intValue()));
				String actual = ceOf(s);
				String model = modelCE(s);
				pairChecked++;
				if (!actual.equals(model)) {
					pairBad++;
					if (pairBad <= 15) {
						pBad.append(String.format("PAIR BAD [%s] actual=[%s] model=[%s]%n",
								cps(s), actual, model));
					}
				}
			}
		}
		System.out.println("check2 pairs: checked=" + pairChecked + " mismatched=" + pairBad);

		// ---------- check 3: random fuzz strings (CE) + pairs (compare) ----------
		Random rnd = new Random(20261014L);
		int[] pools = { 0x20, 0x7E, 0xC0, 0x24F, 0x300, 0x345, 0x1E00, 0x1EFF,
				0x4E00, 0x9FA5, 0xAC00, 0xAC10, 0xE000, 0xE010, 0x9FFF, 0x9FFF };
		int fuzzBad = 0, fuzzChecked = 0;
		StringBuilder fBad = new StringBuilder();
		for (int t = 0; t < 30000; t++) {
			String s = randStr(rnd, pools);
			String actual = ceOf(s);
			String model = modelCE(s);
			fuzzChecked++;
			if (!actual.equals(model)) {
				fuzzBad++;
				if (fuzzBad <= 15) {
					fBad.append(String.format("FUZZ BAD [%s] actual=[%s] model=[%s]%n",
							cps(s), actual, model));
				}
			}
		}
		System.out.println("check3 fuzz CE: checked=" + fuzzChecked + " mismatched=" + fuzzBad);

		int cmpBad = 0, cmpChecked = 0;
		StringBuilder cBad = new StringBuilder();
		for (int t = 0; t < 8000; t++) {
			String s1 = randStr(rnd, pools);
			String s2 = t % 4 == 0 ? s1 : randStr(rnd, pools);
			int actual = sign(collator.compare(s1, s2));
			int model = sign(modelCompare(s1, s2));
			cmpChecked++;
			if (actual != model) {
				cmpBad++;
				if (cmpBad <= 15) {
					cBad.append(String.format("CMP BAD [%s] vs [%s] actual=%d model=%d%n",
							cps(s1), cps(s2), actual, model));
				}
			}
		}
		System.out.println("check3 fuzz CMP: checked=" + cmpChecked + " mismatched=" + cmpBad);

		// ---------- export ----------
		File dir = new File(outDir + "/_collation_tables");
		dir.mkdirs();
		PrintWriter sw = new PrintWriter(new OutputStreamWriter(
				new FileOutputStream(dir + "/single.tsv"), "UTF-8"));
		int nSingle = 0;
		for (Map.Entry<Integer, String> e : dump.entrySet()) {
			String[] parts = e.getValue().split(",");
			if (parts.length == 1 && parts[0].indexOf("7FFF") < 0 && parts[0].length() > 0) {
				sw.println(String.format("%04X\t%s", e.getKey(), parts[0].split("/")[0]));
				nSingle++;
			}
		}
		sw.close();
		PrintWriter cw = new PrintWriter(new OutputStreamWriter(
				new FileOutputStream(dir + "/contract.tsv"), "UTF-8"));
		for (Map.Entry<Integer, ProbeCollationModel.Contract> e : contracts.entrySet()) {
			ProbeCollationModel.Contract ct = e.getValue();
			cw.println(String.format("%04X\t%s", e.getKey(),
					ct.ignorable ? "IGNORABLE" : ct.terminal));
			for (String k : ct.keys) {
				cw.println(String.format("  %s\t%s", cps(k), entryCE.get(k)));
			}
		}
		cw.close();

		PrintWriter rep = new PrintWriter(new OutputStreamWriter(
				new FileOutputStream(dir + "/model_check.txt"), "UTF-8"));
		rep.print(bad.toString());
		rep.print(pBad.toString());
		rep.print(fBad.toString());
		rep.print(cBad.toString());
		rep.println(String.format(
				"SUMMARY bmp=%d/%d pairs=%d/%d fuzzCE=%d/%d fuzzCMP=%d/%d singles=%d contracts=%d",
				mism, checked, pairBad, pairChecked, fuzzBad, fuzzChecked, cmpBad,
				cmpChecked, nSingle, contracts.size()));
		rep.close();
		System.out.println("tables exported to " + dir);
	}
}
