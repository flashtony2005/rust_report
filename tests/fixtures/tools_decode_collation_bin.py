#!/usr/bin/env python3
"""Independent decoder + verifier for crates/rpx-engine/data/collation.bin (M2-12).

Pure Python (no Java). Reverse-engineers the frozen binary format, re-implements
the collation model *from the decoded tables only*, then cross-checks it against
the product-derived goldens:
  * _collation_dump.tsv        (63,488 BMP single chars + 384 supplementary)
  * _collation_golden.tsv      (SORT 3200 + PAIR 58)
  * _collation_nfd_golden.tsv  (NFD, incl. sequence/reordering rows)

Layout (little-endian):
  u32 magic=0x314C4352 | u32 version=1 | u32 sec_cnt=13 | u32 reserved=0
  13 x (u32 byte_offset, u32 count)               (sections 4-byte aligned)
  [0] cjk_primary u16[20902]  index=cp-0x4E00     [7]  ccc_val    u8[653]
  [1] singles     u32[933]    = cp|(primary<<16)  [8]  contract_starter u32[25]=cp|(first<<16)
  [2] nfd_cp      u32[2053]                       [9]  contract_count   u8[25]
  [3] nfd_off     u32[2053]                       [10] contract_key     u32[54]=cp0|(cp1<<16)
  [4] nfd_len     u8[2053]                        [11] contract_key_ce  u16[54]
  [5] nfd_pool    u32[3390]  (elements are u32)   [12] ignorable        u8[25]
  [6] ccc_cp      u32[653]
Stdout ends with: SUMMARY bmp=?,? supp=?,? sort=?,? pair=?,? nfd=?,?
"""
import os
import struct
import sys
from functools import cmp_to_key

HERE = os.path.dirname(os.path.abspath(__file__))
BIN = os.environ.get("COLLATION_BIN", os.path.join(
    HERE, "..", "..", "crates", "rpx-engine", "data", "collation.bin"))
if len(sys.argv) > 1:
    BIN = sys.argv[1]

MAGIC, VER, SEC_CNT = 0x314C4352, 1, 13
CJK_LO, CJK_HI = 0x4E00, 0x9FA5
UNMAPPED_PRIMARY = 0x7FFF


def load_sections(path):
    data = open(path, "rb").read()
    magic, ver, sec_cnt, resv = struct.unpack_from("<IIII", data, 0)
    assert magic == MAGIC, "bad magic %08X" % magic
    assert ver == VER, "bad version %d" % ver
    assert sec_cnt == SEC_CNT, "bad sec_cnt %d" % sec_cnt
    assert resv == 0, "reserved != 0"
    secs = []
    for i in range(sec_cnt):
        off, cnt = struct.unpack_from("<II", data, 16 + 8 * i)
        secs.append((off, cnt))
    # structural self-consistency: 4-byte aligned, non-overlapping, in-file
    for i, (off, cnt) in enumerate(secs):
        assert off % 4 == 0, "sec[%d] offset not aligned" % i
        assert off + cnt >= off and off + cnt <= len(data) + 3, "sec[%d] out of file" % i
        if i + 1 < sec_cnt:
            assert off + cnt <= secs[i + 1][0], "sec[%d] overlaps sec[%d]" % (i, i + 1)
    return data, secs


def unpack(data, off, fmt, n):
    return list(struct.unpack_from("<%d%s" % (n, fmt), data, off))

class Tables:
    def __init__(self, data, secs):
        self.bytes_len = len(data)
        self.sha256 = __import__("hashlib").sha256(data).hexdigest()
        o, c = secs[0]
        self.cjk = unpack(data, o, "H", c)
        o, c = secs[1]
        single = unpack(data, o, "I", c)
        self.single = {v & 0xFFFF: v >> 16 for v in single}
        o, c = secs[2]
        self.nfd_cp = unpack(data, o, "I", c)
        o, c = secs[3]
        self.nfd_off = unpack(data, o, "I", c)
        o, c = secs[4]
        self.nfd_len = list(data[o:o + c])
        o, c = secs[5]
        self.nfd_pool = unpack(data, o, "I", c)
        o, c = secs[6]
        self.ccc_cp = unpack(data, o, "I", c)
        o, c = secs[7]
        self.ccc = {cp: v for cp, v in zip(self.ccc_cp, list(data[o:o + c]))}
        o, c = secs[8]
        st = unpack(data, o, "I", c)
        self.starter_cp = [v & 0xFFFF for v in st]
        self.starter_first = [v >> 16 for v in st]
        o, c = secs[9]
        self.starter_cnt = list(data[o:o + c])
        o, c = secs[10]
        self.key = unpack(data, o, "I", c)
        o, c = secs[11]
        self.key_ce = unpack(data, o, "H", c)
        o, c = secs[12]
        self.ignorable = list(data[o:o + c])
        self.nfd_idx = {cp: i for i, cp in enumerate(self.nfd_cp)}
        self.starter_idx = {cp: i for i, cp in enumerate(self.starter_cp)}

    def check_structure(self):
        p = []
        p.append(("cjk.count", len(self.cjk), 20902))
        p.append(("singles.count", len(self.single), 933))
        p.append(("nfd.count", len(self.nfd_cp), 2053))
        p.append(("nfd_pool.count", len(self.nfd_pool), 3390))
        p.append(("ccc.count", len(self.ccc), 653))
        p.append(("starters", len(self.starter_cp), 25))
        p.append(("sum(contract_count)", sum(self.starter_cnt), len(self.key)))
        p.append(("keys", len(self.key), 54))
        p.append(("sum(ignorable)", sum(self.ignorable), 9))
        p.append(("singles_sorted", self.single_keys_sorted(), True))
        p.append(("nfd_cp_sorted", self.nfd_cp == sorted(self.nfd_cp), True))
        p.append(("ccc_cp_sorted", self.ccc_cp == sorted(self.ccc_cp), True))
        p.append(("keys_in_range", all(
            f + n <= len(self.key) for f, n in zip(self.starter_first, self.starter_cnt)), True))
        p.append(("nfd off/len in pool", all(
            o + l <= len(self.nfd_pool) for o, l in zip(self.nfd_off, self.nfd_len)), True))
        bad = [x for x in p if x[1] != x[2]]
        for name, got, exp in p:
            print("  %-22s %s (got=%s expect=%s)" % (name, "OK" if got == exp else "BAD", got, exp))
        return len(bad)

    def single_keys_sorted(self):
        return sorted(self.single.keys()) == list(self.single.keys())

    # ---- NFD (from the decoded tables only) ----
    def decompose_cp(self, cp):
        if 0xAC00 <= cp <= 0xD7A3:                       # Hangul syllable
            s = cp - 0xAC00
            l, v, t = 0x1100 + s // 588, 0x1161 + (s % 588) // 28, s % 28
            return [l, v] if t == 0 else [l, v, 0x11A7 + t]
        i = self.nfd_idx.get(cp)
        if i is None:
            return [cp]
        o, l = self.nfd_off[i], self.nfd_len[i]
        return self.nfd_pool[o:o + l]           # transitive closure (verified)

    def nfd(self, cps):
        out = []
        for cp in cps:
            out.extend(self.decompose_cp(cp))
        for i in range(1, len(out)):
            cc = self.ccc.get(out[i], 0)
            if cc == 0:
                continue
            j = i
            while j > 0:
                pc = self.ccc.get(out[j - 1], 0)
                if pc == 0 or pc <= cc:
                    break
                out[j], out[j - 1] = out[j - 1], out[j]
                j -= 1
        return out

    # ---- CE sequence / compare ----
    def ce(self, cps):
        out = []
        i, n = 0, len(cps)
        while i < n:
            c = cps[i]
            si = self.starter_idx.get(c)
            if si is not None:
                first, cnt = self.starter_first[si], self.starter_cnt[si]
                if i + 1 < n and cnt > 0:
                    want = c | (cps[i + 1] << 16)
                    for j in range(first, first + cnt):
                        if self.key[j] == want:
                            out.append(self.key_ce[j])
                            i += 2
                            break
                    else:
                        if self.ignorable[si]:
                            break
                        i = self._emit(out, c, i)
                    continue
                if self.ignorable[si]:
                    break
            i = self._emit(out, c, i)
        return out

    def _emit(self, out, c, i):
        p = self.single.get(c)
        if p is not None:
            out.append(p)
        elif CJK_LO <= c <= CJK_HI:
            out.append(self.cjk[c - CJK_LO])
        elif c <= 0xFFFF:
            out.extend((UNMAPPED_PRIMARY, c))
        else:
            out.extend((UNMAPPED_PRIMARY, 0xD800 + ((c - 0x10000) >> 10),
                        0xDC00 + ((c - 0x10000) & 0x3FF)))
        return i + 1

    def cmp(self, a, b):
        x, y = self.ce(self.nfd(a)), self.ce(self.nfd(b))
        return (x > y) - (x < y)
    def total_cmp(self, a, b):
        c = self.cmp(a, b)
        return c if c else (a > b) - (a < b)


def parse_hex_cps(field):
    return tuple(int(t, 16) for t in field.split()) if field.strip() else ()


def parse_dump(path):
    rows = []
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if not line:
                continue
            cp_s, _, ce_s = line.partition("\t")
            ce = [int(x[:4], 16) for x in ce_s.split(",") if x]
            rows.append((int(cp_s, 16), ce))
    return rows


def main():
    print("bin      = %s" % os.path.normpath(BIN))
    data, secs = load_sections(BIN)
    t = Tables(data, secs)
    print("bytes    = %d  sha256=%s" % (t.bytes_len, t.sha256))
    bad = t.check_structure()

    # --- singles 933 vs 935: which dump single-CE code points are absent? ---
    rows = parse_dump(os.path.join(HERE, "_collation_dump.tsv"))
    dump_single = {cp for cp, ce in rows
                   if len(ce) == 1 and ce[0] != UNMAPPED_PRIMARY and not (CJK_LO <= cp <= CJK_HI)}
    missing = sorted(dump_single - set(t.single))
    print("singles investigation: dump non-CJK single-CE=%d bin singles=%d missing=%s"
          % (len(dump_single), len(t.single), [hex(x) for x in missing]))
    for cp in missing:
        i = t.nfd_idx.get(cp)
        exp = None if i is None else t.nfd_pool[t.nfd_off[i]:t.nfd_off[i] + t.nfd_len[i]]
        print("   %s nfd_len=%s nfd->%s ; model CE=%s" % (
            hex(cp), None if i is None else t.nfd_len[i],
            None if exp is None else [hex(x) for x in exp], [hex(x) for x in t.ce(t.nfd([cp]))]))

    # --- BMP + supplementary CE ---
    bmp_bad, supp_bad = [], []
    n_bmp = n_supp = 0
    for cp, expected in rows:
        got = t.ce(t.nfd([cp]))
        if cp <= 0xFFFF:
            n_bmp += 1
            if got != expected:
                bmp_bad.append((cp, expected, got))
        else:
            n_supp += 1
            if got != expected:
                supp_bad.append((cp, expected, got))
    # --- golden SORT + PAIR ---
    sort_rows, pair_rows = [], []
    for line in open(os.path.join(HERE, "_collation_golden.tsv"), encoding="utf-8"):
        line = line.rstrip("\n")
        if not line or line.startswith("#"):
            continue
        f = line.split("\t")
        if f[0] == "SORT":
            sort_rows.append(parse_hex_cps(f[2]))
        elif f[0] == "PAIR":
            pair_rows.append((parse_hex_cps(f[1]), parse_hex_cps(f[2]), int(f[3])))
    sorted_rows = sorted(sort_rows, key=cmp_to_key(t.total_cmp))
    sort_bad = [i for i, (a, b) in enumerate(zip(sorted_rows, sort_rows)) if a != b]
    pair_bad = [(a, b, e, t.cmp(a, b)) for a, b, e in pair_rows if t.cmp(a, b) != e]

    # --- nfd golden ---
    nfd_bad, n_nfd = [], 0
    for line in open(os.path.join(HERE, "_collation_nfd_golden.tsv"), encoding="utf-8"):
        line = line.rstrip("\n")
        if not line:
            continue
        src, _, exp = line.partition("\t")
        n_nfd += 1
        got = t.nfd(parse_hex_cps(src))
        if got != list(parse_hex_cps(exp)):
            nfd_bad.append((src, exp, got))

    for name, lst in (("BMP", bmp_bad), ("SUPP", supp_bad), ("NFD", nfd_bad)):
        for it in lst[:10]:
            print("  %s MISMATCH %s exp=%s got=%s" % (
                name, hex(it[0]) if name != "NFD" else it[0],
                it[1] if name == "NFD" else [hex(v) for v in it[1]],
                [hex(v) for v in it[2]]))
    for i in sort_bad[:5]:
        print("  SORT MISMATCH at %d" % i)
    for a, b, e, g in pair_bad[:5]:
        print("  PAIR MISMATCH %s vs %s exp=%d got=%d" % (a, b, e, g))

    print("structure bad=%d" % bad)
    print("SUMMARY bmp=%d/%d supp=%d/%d sort=%d/%d pair=%d/%d nfd=%d/%d" % (
        len(bmp_bad), n_bmp, len(supp_bad), n_supp,
        len(sort_bad), len(sort_rows), len(pair_bad), len(pair_rows),
        len(nfd_bad), n_nfd))
    ok = (bad == 0 and not bmp_bad and not supp_bad and not sort_bad and not pair_bad and not nfd_bad)
    print("RESULT %s" % ("PASS" if ok else "FAIL"))


if __name__ == "__main__":
    main()
