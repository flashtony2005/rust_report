# SQL 复杂示例：月度 TOP3 产品经营看板

一个可运行的复杂 SQL 报表示例，覆盖：

- CTE 分层聚合（`base` → `ranked`）
- 窗口函数：`SUM(...) OVER (PARTITION BY month)`、`ROW_NUMBER()`
- 多表 JOIN（事实表 + 区域/产品维度）
- 条件过滤（`status='paid'`）
- SQL 侧计算：毛利、毛利率、收入占比
- 第二个 SQL 数据集：退货按月份 + 区域关联
- 报表格间计算：净收入 = 收入 - 成本 - 退货
- 纵向扩展、月份同值合并、`TOP3 合计` 行、A3 横向打印设置

## 文件

| 文件 | 说明 |
|---|---|
| `data.sql` | 建表 + 样例数据 |
| `template.json` | 报表定义（已生成，可修改） |
| `scripts/build_sql_complex_example.py` | 建 SQLite 库 + `datasources.json` |
| `scripts/build_sql_complex_template.py` | 重新生成 `template.json` |

## 运行

在仓库根目录：

```sh
python3 scripts/build_sql_complex_example.py
python3 scripts/build_sql_complex_template.py   # 仅在修改生成器后需要

rm -rf /tmp/sql-complex-run
rpx run examples/sql-complex/template.json \
  --datasources output/sql-complex/datasources.json \
  --out-dir /tmp/sql-complex-run \
  --export xlsx --export csv
```

查看结果：

```sh
rpx result inspect /tmp/sql-complex-run/artifacts/result.json --range A1:K20
```

## 预期

- 数据集行数：3 个月 × 每月 TOP3 = 9 行
- `TOP3 合计`：数量 55、收入 38,700、成本 19,200、退货 410、净收入 19,090
- 合计只统计 TOP3 结果行，**不是全量销售额**
- 结果行从第 4 行开始，合计行被扩展到第 13 行，说明行第 14 行

## 复杂度

```sql
WITH base AS (... GROUP BY ...),
ranked AS (
  SELECT *,
    SUM(revenue) OVER (PARTITION BY month) AS month_revenue,
    ROW_NUMBER() OVER (PARTITION BY month ORDER BY revenue DESC, product) AS rn
  FROM base
)
SELECT ... FROM ranked WHERE rn<=3
```

对应报表把每个字段映射为一列：`summary.select(month)` 纵向扩展，
其余列 `left_head=A4` 跟随；退货用 `returns.select(...)` 按
`month==A4 and region==B4` 关联。

## 修改

- 改 SQL：编辑 `data.sql` 后重建库，并同步 `template.json` 里的 `sql`。
- 改报表：直接改 `template.json`，或改生成器后重跑。
- 换数据库：改 `datasources.json` 的 URL；SQL 使用 SQLite 语法，
  PostgreSQL/MySQL 需按方言调整 `substr` 等函数。
