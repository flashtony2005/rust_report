/*
 * GenFixtures.java -- ReportLite (runqian) .rpx golden-fixture generator.
 *
 * Produces real .rpx binaries through the product's own writer
 * (com.raqsoft.reportl.util.ReportUtils.write) plus a sibling
 * <name>.expected.json holding the authoritative expected values, rendered
 * from the in-memory model through the public getters (NOT reverse-engineered
 * from the serialized bytes).
 *
 * The .rpx bytes are the golden input for the Rust parser (rpx-core);
 * the .expected.json is the golden assertion target.
 *
 * Compile (Java 8 target, product jars on classpath):
 *   javac --release 8 -cp "F:/project/report/reportlite/lib/*" *.java -d out
 * Run (product JRE 1.8):
 *   "F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
 *       rpx.fixtures.GenFixtures <outDir>
 * Round-trip gate:
 *   ... rpx.fixtures.GenFixtures --verify <dir>
 */
package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.IReport;
import com.raqsoft.reportl.util.ReportUtils;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStreamWriter;
import java.io.Writer;

/** Entry point: builds each fixture, writes it, and renders expected.json. */
public class GenFixtures {

	private static final String[] NAMES = {"simple", "merged", "full"};

	public static void main(String[] args) throws Exception {
		String dir = args.length > 0 ? args[0] : ".";
		boolean verify = "--verify".equals(dir);
		if (verify) {
			dir = args.length > 1 ? args[1] : ".";
		}
		int failures = 0;
		for (String name : NAMES) {
			ReportDefine2 built = Fixtures.build(name);
			failures += verify ? verifyOne(dir, name) : gen(dir, name, built);
		}
		if (verify) {
			System.out.println(failures == 0
					? "VERIFY OK: every .rpx round-trips to its expected.json"
					: "VERIFY FAILED: " + failures + " fixture(s) differ");
			if (failures != 0) {
				System.exit(1);
			}
		} else {
			System.out.println("OK: 3 fixtures generated");
		}
	}

	/** Writes the .rpx binary and its expected.json sidecar. */
	private static int gen(String dir, String name, ReportDefine2 rd) throws Exception {
		String rpxPath = dir + File.separator + name + ".rpx";
		String jsonPath = dir + File.separator + name + ".expected.json";
		ReportUtils.write(rpxPath, rd);
		writeFile(jsonPath, ExpectedJson.render(rd) + "\n");
		System.out.println("wrote " + rpxPath + " (" + new File(rpxPath).length() + " bytes)");
		System.out.println("wrote " + jsonPath);
		return 0;
	}

	/**
	 * Round-trip gate: re-read the written .rpx through the product's own
	 * reader and confirm it still yields identical expected.json. This is what
	 * proves the Rust parser can reproduce the golden values from the binary
	 * alone.
	 */
	private static int verifyOne(String dir, String name) throws Exception {
		String rpxPath = dir + File.separator + name + ".rpx";
		String jsonPath = dir + File.separator + name + ".expected.json";
		IReport read = ReportUtils.read(rpxPath);
		if (!(read instanceof ReportDefine2)) {
			System.out.println("FAIL " + name + ": read back " + read.getClass().getName());
			return 1;
		}
		String actual = ExpectedJson.render((ReportDefine2) read).trim();
		String expected = readFile(jsonPath).trim();
		if (!expected.equals(actual)) {
			System.out.println("FAIL " + name + ": round-trip mismatch");
			System.out.println("  on disk : " + expected);
			System.out.println("  re-read : " + actual);
			return 1;
		}
		System.out.println("PASS " + name + ": " + new File(rpxPath).length()
				+ " bytes round-trip to expected.json");
		return 0;
	}

	private static void writeFile(String path, String content) throws Exception {
		Writer w = new OutputStreamWriter(new FileOutputStream(path), "UTF-8");
		try {
			w.write(content);
		} finally {
			w.close();
		}
	}

	private static String readFile(String path) throws Exception {
		InputStream in = new FileInputStream(path);
		try {
			byte[] buf = new byte[in.available()];
			int off = 0;
			while (off < buf.length) {
				int n = in.read(buf, off, buf.length - off);
				if (n < 0) {
					break;
				}
				off += n;
			}
			return new String(buf, 0, off, "UTF-8");
		} finally {
			in.close();
		}
	}
}
