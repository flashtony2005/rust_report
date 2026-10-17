#!/usr/bin/env python3
"""交叉表渲染结果与 sqlite 直查逐格对拍（区域/季度 × 产品 两版通用）。"""
import csv
import sqlite3
import sys

DB = "/Users/lushaohui/project/rust_report/rust_report/demo_data/report_demo.db"
CSV = sys.argv[1]
N_DATA = int(sys.argv[2]) if len(sys.argv) > 2 else 80  # 明细行数
N_TAIL = int(sys.argv[3]) if len(sys.argv) > 3 else 2   # 末尾统计行数

con = sqlite3.connect(DB)
cur = con.cursor()

# 明细真值：行维度 × 产品
cur.execute(
    "SELECT s.quarter, e.name, s.product, s.amount FROM sales s "
    "JOIN emp e ON e.id = s.emp_id ORDER BY s.quarter, e.id, s.product"
)
rows = cur.fetchall()
# 产品列序 = 引擎 `select(product:1,,,product)` 的首现序（按 quarter,emp,product 排）
prod_order = []
for _, _, p, _ in rows:
    if p not in prod_order:
        prod_order.append(p)

truth = {}
for q, n, p, a in rows:
    truth[(q, n, p)] = a

with open(CSV, encoding="utf-8") as f:
    data = list(csv.reader(f))

# 定位表头行（含"季度"与产品名）
hdr_i = next(i for i, r in enumerate(data) if "季度" in r[:2])
data_start = hdr_i + 1
cols = data[hdr_i]
prod_cols = {c: i for i, c in enumerate(cols) if c in prod_order}

bad = 0
checked = 0
for r in data[data_start : data_start + N_DATA]:
    q, n = r[0], r[1]
    for p, ci in prod_cols.items():
        got = r[ci].strip()
        want = truth.get((q, n, p))
        checked += 1
        if want is None:
            ok = got == ""
        else:
            try:
                ok = abs(float(got) - want) < 0.005
            except ValueError:
                ok = False
        if not ok:
            bad += 1
            if bad <= 10:
                print(f"MISMATCH ({q},{n},{p}): got={got!r} want={want}")

# 合计 / 平均 行
tail = data[data_start + N_DATA : data_start + N_DATA + N_TAIL]
cur.execute("SELECT product, SUM(amount) FROM sales GROUP BY product")
sums = dict(cur.fetchall())
cur.execute("SELECT product, AVG(amount) FROM sales GROUP BY product")
avgs = dict(cur.fetchall())
for r in tail:
    for p, ci in prod_cols.items():
        got = r[ci].strip()
        label = r[0]
        want = sums[p] if label == "合计" else avgs[p]
        checked += 1
        try:
            ok = abs(float(got) - want) < 0.005
        except ValueError:
            ok = False
        if not ok:
            bad += 1
            if bad <= 10:
                print(f"MISMATCH tail ({label},{p}): got={got!r} want={want}")

print(f"checked={checked} mismatched={bad}")
sys.exit(1 if bad else 0)
