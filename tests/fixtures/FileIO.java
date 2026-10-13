/*
 * FileIO.java -- UTF-8 text read/write for the fixture sidecars.
 *
 * Kept separate from GenFixtures so neither file has to grow past the
 * 300-line gate just to own a string<->bytes helper.
 */
package rpx.fixtures;

import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStreamWriter;
import java.io.Writer;

final class FileIO {

	private FileIO() {
	}

	static void write(String path, String content) throws Exception {
		Writer w = new OutputStreamWriter(new FileOutputStream(path), "UTF-8");
		try {
			w.write(content);
		} finally {
			w.close();
		}
	}

	static String read(String path) throws Exception {
		InputStream in = new FileInputStream(path);
		try {
			byte[] buf = new byte[Math.max(in.available(), 1)];
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