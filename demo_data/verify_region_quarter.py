#!/usr/bin/env python3
"""区域×季度×产品 交叉表渲染结果与 sqlite 直查逐格对拍。

校验三部分：
1. 明细行序与 ds1 的 (region_order, quarter) 排序严格一致（20 行）；
2. 每行 8 个产品金额格 = SUM(amount) GROUP BY (region, quarter, product)；
3. 行末「数量」「金额（元）」两列 + 合计行的列合计与总计，全部直查 DB 对拍。
"""
import csv
import sqlite3
import sys

DB = "/Users/lushaohui/project/rust_report/rust_report/demo_data/report_demo.db"
CSV = sys.argv[1]

con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
cur = con.cursor()

# ── 真值 1：明细行序（与 ds1 同序）──
cur.execute(
    "SELECT e.region, s.quarter FROM sales s "
    "JOIN emp e ON e.id = s.emp_id "
    "GROUP BY e.region_order, e.region, s.quarter "
    "ORDER BY e.region_order, s.quarter"
)
expected_rows = cur.fetchall()
N_DATA = len(expected_rows)

# ── 真值 2：(区域,季度,产品) 金额 / (区域,季度) 数量与金额 ──
cur.execute(
    "SELECT e.region, s.quarter, s.product, s.amount, s.qty FROM sales s "
    "JOIN emp e ON e.id = s.emp_id"
)
cell_amount = {}
row_qty = {}
row_amount = {}
for reg, q, prod, amt, qty in cur.fetchall():
    cell_amount[(reg, q, prod)] = cell_amount.get((reg, q, prod), 0.0) + amt
    row_qty[(reg, q)] = row_qty.get((reg, q), 0.0) + qty
    row_amount[(reg, q)] = row_amount.get((reg, q), 0.0) + amt

# ── 真值 3：列合计与总计 ──
cur.execute("SELECT product, SUM(amount) FROM sales GROUP BY product")
prod_sums = dict(cur.fetchall())
cur.execute("SELECT SUM(qty), SUM(amount) FROM sales")
grand_qty, grand_amount = cur.fetchone()

with open(CSV, encoding="utf-8-sig") as f:
    data = list(csv.reader(f))

hdr_i = next(i for i, r in enumerate(data) if "区域" in r[:2] and "季度" in r[:2])
cols = data[hdr_i]
prod_cols = {c: i for i, c in enumerate(cols) if c in prod_sums}
qty_ci = cols.index("数量")
amt_ci = cols.index("金额（元）")
data_start = hdr_i + 1

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
        if bad <= 10:
            print(f"MISMATCH {tag}: got={got!r} want={want}")


# ── 明细行 ──
for k, (reg, q) in enumerate(expected_rows):
    r = data[data_start + k]
    if (r[0], r[1]) != (reg, q):
        bad += 1
        if bad <= 10:
            print(f"ROW-ORDER MISMATCH row {k}: got={(r[0], r[1])!r} want={(reg, q)!r}")
        continue
    for p, ci in prod_cols.items():
        chk(f"({reg},{q},{p})", r[ci].strip(), cell_amount.get((reg, q, p)))
    chk(f"({reg},{q}) 数量", r[qty_ci].strip(), row_qty[(reg, q)])
    chk(f"({reg},{q}) 金额", r[amt_ci].strip(), row_amount[(reg, q)])

# ── 合计行 ──
tail = data[data_start + N_DATA]
if tail[0] != "合计":
    bad += 1
    print(f"TAIL LABEL MISMATCH: got={tail[0]!r}")
for p, ci in prod_cols.items():
    chk(f"(合计,{p})", tail[ci].strip(), prod_sums[p])
chk("(合计) 数量", tail[qty_ci].strip(), grand_qty)
chk("(合计) 金额", tail[amt_ci].strip(), grand_amount)

print(f"rows={N_DATA} checked={checked} mismatched={bad}")
sys.exit(1 if bad else 0)
