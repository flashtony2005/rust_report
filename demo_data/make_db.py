#!/usr/bin/env python3
"""演示库构建器：区域销售综合报表（2026 年度）的 SQLite 数据源。

自创数据（`random.seed(20261016)` 固定 ⇒ 可复现）：

* ``emp``：20 名员工，5 个区域 × 4 人（华东/华北/华南/西南/东北），
  每人一个职位；``region_order`` 只用于排序（中文按 UTF-8 字节序排不顺眼）。
* ``sales``：640 行明细 —— 20 员工 × 8 产品 × 4 季度，
  ``amount = qty × 标价 × uniform(0.92, 1.0)``（保留 2 位小数）。
  ``month``（1..12）由**插入下标**确定性派生（``qi*3 + (id%3) + 1``，不消耗 rng），
  供「季度 → 月份 + 小计」嵌套横向扩展报表使用。

列类型纪律（`rpx-ds/src/sql_source.rs` 实测）：sqlite 的 Any 层**不认
BOOLEAN** 声明列，故全部用 INTEGER/TEXT/REAL。

用法：``python3 demo_data/make_db.py``（幂等：先 DROP 再建）。
"""

import os
import random
import sqlite3
import sys

SEED = 20261016
DB_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "report_demo.db")

# 5 个区域（order 只服务 ORDER BY，让报表按业务顺序而不是字节序展开）。
REGIONS = [
    ("华东", 1),
    ("华北", 2),
    ("华南", 3),
    ("西南", 4),
    ("东北", 5),
]

# 20 个不重名的员工姓名（按区域 4 人一组）。
NAMES = {
    "华东": ["林晓峰", "苏婉清", "顾明远", "沈书瑶"],
    "华北": ["赵子墨", "钱雨桐", "孙浩然", "周若曦"],
    "华南": ["吴俊杰", "郑思远", "王梦琪", "冯启铭"],
    "西南": ["陈静姝", "褚翰林", "卫子昂", "蒋文昊"],
    "东北": ["韩雪莹", "杨大伟", "朱一鸣", "秦淑芬"],
}

TITLES = ["销售经理", "高级销售代表", "销售代表", "客户经理"]

# 8 个产品：标价 + 每笔数量区间（数量区间让单价高的产品笔数少，符合直觉）。
PRODUCTS = [
    ("笔记本电脑", 6800, 5, 40),
    ("显示器", 1599, 10, 80),
    ("机械键盘", 459, 20, 120),
    ("无线鼠标", 129, 30, 200),
    ("激光打印机", 2299, 3, 25),
    ("机架服务器", 29800, 1, 8),
    ("千兆交换机", 1899, 5, 30),
    ("高清投影仪", 4599, 3, 15),
]

QUARTERS = ["Q1", "Q2", "Q3", "Q4"]


def build() -> None:
    rng = random.Random(SEED)
    if os.path.exists(DB_PATH):
        os.remove(DB_PATH)
    conn = sqlite3.connect(DB_PATH)
    try:
        cur = conn.cursor()
        cur.execute("DROP TABLE IF EXISTS emp")
        cur.execute("DROP TABLE IF EXISTS sales")
        cur.execute(
            """
            CREATE TABLE emp (
                id          INTEGER PRIMARY KEY,
                name        TEXT NOT NULL,
                region      TEXT NOT NULL,
                region_order INTEGER NOT NULL,
                title       TEXT NOT NULL
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE sales (
                id      INTEGER PRIMARY KEY,
                emp_id  INTEGER NOT NULL REFERENCES emp(id),
                product TEXT NOT NULL,
                quarter TEXT NOT NULL,
                month   INTEGER NOT NULL,
                qty     INTEGER NOT NULL,
                amount  REAL NOT NULL
            )
            """
        )

        # ── emp：20 行（5 区域 × 4 人）──
        emp_id = 0
        for region, order in REGIONS:
            for i, name in enumerate(NAMES[region]):
                emp_id += 1
                cur.execute(
                    "INSERT INTO emp VALUES (?, ?, ?, ?, ?)",
                    (emp_id, name, region, order, TITLES[i]),
                )

        # ── sales：640 行（20 员工 × 8 产品 × 4 季度）──
        # `month` 由下标**确定性**派生（不消耗 rng）⇒ 金额序列与加月维度前逐字节相同，
        # 已有报表（交叉表 / 区域×季度）的数字不受影响。
        sales_id = 0
        for emp in range(1, emp_id + 1):
            for product, price, lo, hi in PRODUCTS:
                for qi, quarter in enumerate(QUARTERS):
                    sales_id += 1
                    qty = rng.randint(lo, hi)
                    amount = round(qty * price * rng.uniform(0.92, 1.0), 2)
                    month = qi * 3 + (sales_id % 3) + 1
                    cur.execute(
                        "INSERT INTO sales VALUES (?, ?, ?, ?, ?, ?, ?)",
                        (sales_id, emp, product, quarter, month, qty, amount),
                    )
        conn.commit()

        # ── 自检（把验证值打出来，渲染断言用同一口径）──
        n_emp = cur.execute("SELECT COUNT(*) FROM emp").fetchone()[0]
        n_sales = cur.execute("SELECT COUNT(*) FROM sales").fetchone()[0]
        total_qty = cur.execute("SELECT SUM(qty) FROM sales").fetchone()[0]
        total_amt = cur.execute("SELECT SUM(amount) FROM sales").fetchone()[0]
        print(f"[make_db] {DB_PATH}")
        print(f"[make_db] emp {n_emp} 行 / sales {n_sales} 行")
        print(f"[make_db] SUM(qty) = {total_qty}")
        print(f"[make_db] SUM(amount) = {total_amt:.2f}")
        per_region = cur.execute(
            "SELECT e.region, COUNT(*), ROUND(SUM(s.amount), 2) "
            "FROM sales s JOIN emp e ON e.id = s.emp_id "
            "GROUP BY e.region ORDER BY e.region_order"
        ).fetchall()
        for region, rows, amt in per_region:
            print(f"[make_db]   {region}：{rows} 笔，{amt:.2f} 元")
    finally:
        conn.close()


if __name__ == "__main__":
    try:
        build()
    except Exception as e:  # noqa: BLE001 —— 顶层兜底，打印后非零退出
        print(f"[make_db] 失败：{e}", file=sys.stderr)
        sys.exit(1)
