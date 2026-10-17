#!/usr/bin/env python3
"""Generate examples/sql-complex/template.json (kept in sync with data.sql)."""
import json
from pathlib import Path

cells = []


def add(**kw):
    cells.append(kw)


def title(row, text, size, color, merge=11):
    add(row=row, col=1, text=text, font_size=size, bold=size >= 18,
        fore_color=color, v_align="middle", merge={"rows": 1, "cols": merge},
        extend_mode="none")


def header(col, text):
    add(row=3, col=col, text=text, font_size=10, bold=True, fore_color="#FFFFFF",
        back_color="#1B505B", h_align="center", v_align="middle",
        extend_mode="none")


def detail(col, expr, align, fmt=None, merge_same=None):
    kw = dict(row=4, col=col, expr=expr, font_size=10, v_align="middle",
              h_align=align, left_head="A4")
    if fmt:
        kw["format"] = fmt
    if merge_same:
        kw["merge_same"] = merge_same
        kw["extend_mode"] = "vertical"
        kw["left_head"] = "`0"
    else:
        kw["extend_mode"] = "none"
    add(**kw)


def total(col, expr=None, text=None, fmt=None):
    kw = dict(row=5, col=col, font_size=10, bold=True, fore_color="#163D46",
              back_color="#E8F2F1", h_align="right", v_align="middle",
              extend_mode="none")
    if text is not None:
        kw["text"] = text
    if expr is not None:
        kw["expr"] = expr
    if fmt:
        kw["format"] = fmt
    add(**kw)


title(1, "SQL 复杂示例 · 月度 TOP3 产品经营看板", 18, "#163D46")
title(2, "CTE + 窗口排名 + 条件聚合 + 退货关联；数据来自 SQLite，QTY/收入/成本/毛利/占比/净额。", 9, "#52666E")
for i, text in enumerate(["月份", "区域", "类别", "产品", "数量", "收入", "成本",
                          "毛利率%", "收入占比%", "退货", "净收入"], 1):
    header(i, text)

detail(1, "summary.select(month)", "center", "@", merge_same="vertical")
detail(2, "summary.region", "left")
detail(3, "summary.category", "left")
detail(4, "summary.product", "left")
detail(5, "summary.qty", "right", "#,##0")
detail(6, "summary.revenue", "right", "#,##0.00")
detail(7, "summary.cost", "right", "#,##0.00")
detail(8, "summary.margin_pct", "right", "0.0")
detail(9, "summary.share_pct", "right", "0.0")
detail(10, "sum(returns.select(return_amount,month==A4 and region==B4))", "right", "#,##0.00")
detail(11, "F4-G4-J4", "right", "#,##0.00")

total(1, text="TOP3 合计")
# merge total label across the first four columns
cells[-1]["merge"] = {"rows": 1, "cols": 4}
total(5, "summary.sum(qty)", fmt="#,##0")
total(6, "summary.sum(revenue)", fmt="#,##0.00")
total(7, "summary.sum(cost)", fmt="#,##0.00")
total(8, "if(F5==0,0,(F5-G5)*100/F5)", fmt="0.0")
total(9, text="100.0", fmt="0.0")
total(10, "returns.sum(return_amount)", fmt="#,##0.00")
total(11, "F5-G5-J5", fmt="#,##0.00")

add(row=6, col=1, text="说明：毛利=收入-成本；净收入=收入-成本-退货。SQL 先按 CTE 聚合，再用窗口函数取每月收入 TOP3；合计只统计 TOP3 结果行，不是全量销售额。",
    font_size=9, fore_color="#52666E", v_align="middle",
    merge={"rows": 1, "cols": 11}, extend_mode="none")

doc = {
    "format": "rpx-report",
    "format_version": 1,
    "schema": "rpx-report/v1",
    "version": 1,
    "grid": {"rows": 20, "cols": 11},
    "datasets": [
        {"name": "summary", "sql": {"dsn": "demo-db", "sql": """WITH base AS (
 SELECT substr(s.order_date,1,7) AS month,
        s.region, p.category, p.product,
        SUM(s.qty) AS qty,
        ROUND(SUM(s.qty*s.price),2) AS revenue,
        ROUND(SUM(s.qty*s.cost),2) AS cost
 FROM fact_sales s
 JOIN dim_product p ON p.product_id=s.product_id
 WHERE s.status='paid'
 GROUP BY 1,2,3,4
), ranked AS (
 SELECT *,
        SUM(revenue) OVER (PARTITION BY month) AS month_revenue,
        ROW_NUMBER() OVER (PARTITION BY month ORDER BY revenue DESC, product) AS rn
 FROM base
)
SELECT month,region,category,product,qty,revenue,cost,
       ROUND(revenue-cost,2) AS margin,
       ROUND((revenue-cost)*100.0/NULLIF(revenue,0),1) AS margin_pct,
       ROUND(revenue*100.0/NULLIF(month_revenue,0),1) AS share_pct
FROM ranked
WHERE rn<=3
ORDER BY month,rn"""}},
        {"name": "returns", "sql": {"dsn": "demo-db", "sql": """SELECT substr(r.return_date,1,7) AS month,
       s.region,
       ROUND(SUM(r.amount),2) AS return_amount
FROM fact_returns r
JOIN fact_sales s ON s.id=r.sale_id
GROUP BY 1,2
ORDER BY 1,2"""}},
    ],
    "cells": cells,
    "print_setup": {"paper": "A3", "orientation": "landscape",
                    "pager_style": "paper", "table_column_num": 11},
}
root = Path(__file__).resolve().parent.parent
(root / "examples/sql-complex/template.json").write_text(
    json.dumps(doc, ensure_ascii=False, indent=2) + "\n")
