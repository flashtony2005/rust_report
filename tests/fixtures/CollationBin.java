package rpx.fixtures;

/**
 * Serialises {@link CollationData} into the frozen `collation.bin` layout.
 *
 * Layout (all little-endian):
 *   Header (16 bytes)
 *     u32 magic = 0x314C4352 ("RCL1"), u32 version = 1,
 *     u32 sec_cnt = 13, u32 reserved = 0
 *   Section table: sec_cnt x (u32 byte_offset, u32 count)  -- offset from
 *   file start; every section start is 4-byte aligned.
 *   Then the 13 sections' payloads, each 4-byte aligned.
 *
 * Kept apart from {@link CollationData} (extraction) and
 * {@code GenCollationData} (orchestration) so all three stay under the
 * 300-line gate.
 */
final class CollationBin {

	private CollationBin() {
	}

	static final int MAGIC = 0x314C4352; // bytes 52 43 4C 31 == "RCL1"

	static byte[] write(CollationData d) {
		int n = CollationData.SEC_CNT;
		int[] off = new int[n];
		int[] cnt = new int[n];
		int pos = 16 + n * 8;
		for (int i = 0; i < n; i++) {
			int[] v = d.section(i);
			cnt[i] = v.length;
			pos = align4(pos);
			off[i] = pos;
			pos += v.length * CollationData.WIDTH[i];
		}
		pos = align4(pos);

		byte[] out = new byte[pos];
		putU32(out, 0, MAGIC);
		putU32(out, 4, 1);
		putU32(out, 8, n);
		putU32(out, 12, 0);
		for (int i = 0; i < n; i++) {
			putU32(out, 16 + i * 8, off[i]);
			putU32(out, 16 + i * 8 + 4, cnt[i]);
		}
		for (int i = 0; i < n; i++) {
			int[] v = d.section(i);
			int w = CollationData.WIDTH[i];
			int p = off[i];
			for (int k = 0; k < v.length; k++) {
				if (w == 1) {
					out[p++] = (byte) (v[k] & 0xFF);
				} else if (w == 2) {
					putU16(out, p, v[k]);
					p += 2;
				} else {
					putU32(out, p, v[k]);
					p += 4;
				}
			}
		}
		return out;
	}

	private static int align4(int p) {
		return (p + 3) & ~3;
	}

	private static void putU16(byte[] b, int p, int v) {
		b[p] = (byte) (v & 0xFF);
		b[p + 1] = (byte) ((v >>> 8) & 0xFF);
	}

	private static void putU32(byte[] b, int p, int v) {
		b[p] = (byte) (v & 0xFF);
		b[p + 1] = (byte) ((v >>> 8) & 0xFF);
		b[p + 2] = (byte) ((v >>> 16) & 0xFF);
		b[p + 3] = (byte) ((v >>> 24) & 0xFF);
	}
}
