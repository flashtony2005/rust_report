#!/usr/bin/env python3
"""季度 → 月份 + 组内小计 报表的逐格对拍。

结构：1 个标签列 + 4 组 ×（3 个月份列 + 1 个小计列）= 17 列，3 行。
校验：月份金额 = SUM(amount) GROUP BY month；小计 = SUM(amount) GROUP BY quarter；
并校验列序（季度组序、组内月份序、小计列紧跟本组）与各组列位。
"""
import csv
import sqlite3
import sys

DB = "/Users/lushaohui/project/rust_report/rust_report/demo_data/report_demo.db"
CSV = sys.argv[1]

con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
cur = con.cursor()

QUARTERS = ["Q1", "Q2", "Q3", "Q4"]
months_of = {q: [f"{qi * 3 + k + 1}月" for k in range(3)] for qi, q in enumerate(QUARTERS)}

cur.execute("SELECT (month || '月'), ROUND(SUM(amount), 2) FROM sales GROUP BY month")
month_sum = dict(cur.fetchall())
cur.execute("SELECT quarter, ROUND(SUM(amount), 2) FROM sales GROUP BY quarter")
quarter_sum = dict(cur.fetchall())

with open(CSV, encoding="utf-8-sig") as f:
    data = list(csv.reader(f))

hdr = data[1]          # 月份表头行
val = data[2]          # 金额行
bad = 0
checked = 0


def chk(tag, got, want, tol=0.005):
    global bad, checked
    checked += 1
    try:
        ok = abs(float(got) - float(want)) <= tol
    except (ValueError, TypeError):
        ok = False
    if not ok:
        bad += 1
        print(f"MISMATCH {tag}: got={got!r} want={want}")


# 期望布局：col1 = 标签；其后 4 组，每组 = 3 月份列 + 1 小计列
want_hdr = ["月份"]
want_cols = []  # (col_index_0based, kind, key)
c = 1
for q in QUARTERS:
    for m in months_of[q]:
        want_hdr.append(m)
        want_cols.append((c, "month", m))
        c += 1
    want_hdr.append("小计")
    want_cols.append((c, "quarter", q))
    c += 1

if len(hdr) != len(want_hdr):
    bad += 1
    print(f"COL-COUNT MISMATCH: got={len(hdr)} want={len(want_hdr)}")
for i, (got, want) in enumerate(zip(hdr, want_hdr)):
    if got.strip() != want:
        bad += 1
        print(f"HEADER MISMATCH col{i + 1}: got={got!r} want={want!r}")

# 季度表头行：**只覆盖本组的 3 个月份列**。
#
# 这是「显式声明主格」的直接后果：小计表头 `C2` 的 `top_head` 被声明为季度格 `B1`
# （而不是让引擎自动上溯），于是季度锚点的包络不再把小计列吞进去 ⇒
# 季度表头跨 3 列（参考图的画法），小计列在季度行上**留空**。
#
# ⛔ 这条断言是**双向**的：既要求月份列上必须是本组季度值，也要求小计列上**必须为空**。
# 只写前一半的话，「季度又跨 4 列」这个回退会静默通过 —— 那正是本脚本要拦的。
hdr0 = data[0]
for ci, kind, key in want_cols:
    if kind == "month":
        q = next(x for x in QUARTERS if key in months_of[x])
        want = q
    else:
        want = ""  # 小计列：不在季度锚点的包络内
    if hdr0[ci].strip() != want:
        bad += 1
        print(f"QUARTER-HEADER MISMATCH col{ci + 1}: got={hdr0[ci]!r} want={want!r}")

for ci, kind, key in want_cols:
    want = month_sum[key] if kind == "month" else quarter_sum[key]
    chk(f"{kind}({key})", val[ci].strip(), want)

print(f"cols={len(hdr)} checked={checked} mismatched={bad}")
sys.exit(1 if bad else 0)
