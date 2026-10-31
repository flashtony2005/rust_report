"""Cross-check rpx-cli print output against a fixture's expected.json.

Usage: python cli_crosscheck.py <fixture-name-without-extension>
Exits 0 when every asserted field matches, 1 otherwise.
"""
import json
import re
import subprocess
import sys

name = sys.argv[1]
cli = "target/debug/rpx-cli.exe"
out = subprocess.run([cli, "print", "tests/fixtures/%s.rpx" % name],
                     capture_output=True, text=True, encoding="utf-8")
if out.returncode != 0:
    print("FAIL: cli exit %d\n%s" % (out.returncode, out.stderr))
    sys.exit(1)
txt = out.stdout

d = json.load(open("tests/fixtures/%s.expected.json" % name, encoding="utf-8"))
errs = []

m = re.search(r"matrix: (\d+)x(\d+) \(logical (\d+)x(\d+)\)", txt)
if not m:
    errs.append("no matrix line")
else:
    r, c, rc, cc = (int(x) for x in m.groups())
    if (r, c) != (d["rows"], d["cols"]):
        errs.append("rows/cols %s vs %s" % ((r, c), (d["rows"], d["cols"])))
    if (rc, cc) != (d["row_count"], d["col_count"]):
        errs.append("row/col_count %s vs %s" % ((rc, cc), (d["row_count"], d["col_count"])))

for key, pat in (("row_heights", r"row_heights: \[([^\]]*)\]"),
                 ("col_widths", r"col_widths:\s*\[([^\]]*)\]")):
    mm = re.search(pat, txt)
    if not mm:
        errs.append("no " + key)
        continue
    got = [float(x) for x in mm.group(1).split(",")]
    exp = d[key]
    if len(got) != len(exp) or any(abs(a - b) > 1e-5 for a, b in zip(got, exp)):
        errs.append("%s %s vs %s" % (key, got, exp))

exp_merges = set()
for cell in d["cells"]:
    if "merged" in cell:
        g = cell["merged"]
        exp_merges.add((g["begin_row"], g["begin_col"], g["end_row"], g["end_col"]))

head = txt.split("cells:")[0]
got_merges = set(tuple(int(x) for x in mm.groups())
                 for mm in re.finditer(r"\((\d+),(\d+)\)\.\.\((\d+),(\d+)\)", head))
if got_merges != exp_merges:
    errs.append("merges %s vs %s" % (sorted(got_merges), sorted(exp_merges)))
else:
    print("merges match: %d" % len(exp_merges))

fill_total = 0
for cell in d["cells"]:
    if "merged" not in cell:
        continue
    g = cell["merged"]
    for rr in range(g["begin_row"], g["end_row"] + 1):
        for cc2 in range(g["begin_col"], g["end_col"] + 1):
            fill_total += 1
            pat = (r"\(%d,%d\): value=\"%s\" merged=\(%d,%d\)\.\.\(%d,%d\)"
                   % (rr, cc2, re.escape(str(cell["value"])),
                      g["begin_row"], g["begin_col"], g["end_row"], g["end_col"]))
            if not re.search(pat, txt):
                errs.append("grid fill missing at (%d,%d)" % (rr, cc2))
print("grid fill verified: %d slots" % fill_total)

# every expected non-default cell must appear in the cli listing
missing = []
for cell in d["cells"]:
    pat = r"\(%d,%d\): " % (cell["row"], cell["col"])
    if not re.search(pat, txt):
        missing.append((cell["row"], cell["col"]))
if missing:
    errs.append("cells absent from cli output: %s" % missing)

print("RESULT:", "ALL OK" if not errs else "FAIL " + "; ".join(errs))
sys.exit(0 if not errs else 1)
