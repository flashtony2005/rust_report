---
name: rpx-report-authoring
description: 用 rpx CLI 从数据（CSV/Excel/SQL）准确产出一份可打印报表的端到端 SOP。当用户要求"用 CLI 生成/做一个报表"、"把这份数据做成 Excel/打印稿"、或问"为什么 CLI 跑出来的表不对"时使用。覆盖最小模板骨架、扩展与主格的硬约束、六类静默失败、以及一条必跑的验证闸（verify_report.py）。不适用于只查库内既有报表（用 rpx 技能）。
---

# 用 CLI 准确生成报表

目标不是"跑通"，而是**产物可交付**：行数对、数字不溢出、页数可控、宏生效、能打印。

## 三条铁律

1. **先数据，后版式**。数据集没被证实能加载（`dataset inspect` 看到正确列名）之前，不要写任何单元格。
2. **`validate` 通过 ≠ 能出报表**。它的 `deferred` 明确包含 `data_loading/calculation/pagination/export`。必须真跑。
3. **每次改动都过同一道闸**：`verify_report.py`。不看它的 PASS，不算完成。

## 第 1 步：确认工具链

```sh
command -v rpx && rpx --version
rpx doctor
```

- 版本必须与你依据的文档一致。**已安装二进制可能落后于源码**（本例曾出现装 0.1.0、源码 0.13.0，导致文档里的命令直接报 `USAGE`）。
- 版本不一致时先 `make install-local`，不要靠猜行为。

## 第 2 步：先证数据

```sh
rpx dataset inspect template.json --name ds --data-dir ./data --limit 5
```

判据三条，缺一不可：

- `columns` 是**真正的列名列表**（`["customer","amount"]`），不是 `["customer,amount"]` 这种一整串。
- `sample` 的值类型对（金额是 int/decimal，不是 string）。
- `rows` 与源文件行数一致。

> 列名粘成一串 = CSV 没按逗号切分 = 没写 `"delimiter": ","`。
> 这时 `validate` 仍然返回 `valid: true`，要到 `run` 才炸，且错误是
> `CALCULATION_FAILED: Engine(UnknownCell { id: "ds.customer" })`——**它不会告诉你是分隔符问题**。

## 第 3 步：抄最小骨架，不要从空白开始

```json
{
  "version": 1,
  "grid": { "rows": 6, "cols": 4 },
  "datasets": [{ "name": "ds", "file": { "file_name": "data.csv", "encoding": "UTF-8", "delimiter": "," } }],
  "col_props": [{ "col": 1, "width": 40 }, { "col": 2, "width": 30 }, { "col": 3, "width": 20 }, { "col": 4, "width": 20 }],
  "row_props": [{ "row": 1, "height": 12 }, { "row": 3, "height": 8 }, { "row": 4, "height": 8 }, { "row": 5, "height": 8 }],
  "printing": { "auto_size": false },
  "cells": [
    { "row": 1, "col": 1, "text": "标题", "font_size": 16, "bold": true, "h_align": "center", "merge": { "rows": 1, "cols": 4 } },
    { "row": 3, "col": 1, "text": "客户", "bold": true, "back_color": "#2C4A5A", "fore_color": "#FFFFFF" },
    { "row": 3, "col": 2, "text": "金额", "bold": true, "back_color": "#2C4A5A", "fore_color": "#FFFFFF" },
    { "row": 4, "col": 1, "extend_mode": "vertical", "expr": "ds.group(customer)" },
    { "row": 4, "col": 2, "extend_mode": "none", "left_head": "A4", "expr": "ds.sum(amount,customer==A4)", "format": "#,##0" },
    { "row": 5, "col": 1, "text": "合计", "bold": true },
    { "row": 5, "col": 2, "expr": "ds.sum(amount)", "format": "#,##0" }
  ]
}
```

从现成模板起步更快：

```sh
rpx template create --example complex-master --out-dir /tmp/r
rpx catalog examples
```

## 第 4 步：写版式（硬约束）

| 约束 | 规则 | 违反的后果 |
| --- | --- | --- |
| 扩展列 | 同排可以有多个 `vertical`，但必须串成**父子链**（`B4` 的 `left_head=A4`）；并列且互不指向的多个 vertical 会相乘 | 6 行明细炸成数百行（真实案例 798 行） |
| 扩展块 | 扩展行与合计行之间**不能有空行** | 空行被算进块内，合计被推下去，页面上多一条空白 |
| 计算列 | 同行四则运算 `B4*0.1` 可用；**不要把 `select(...)` 放非扩展列**（返回逗号串）；跨行累计 `E4[-1]+C4-D4` | 引用另一扩展列做 `C4+D4` 会被当整列集合相加 → `UnsupportedExpr`；`select` 会算成 0 且不报错 |
| 宏 | `${period}` **只能写在 `expr`** | 写在 `text` 里原样上屏（`${period} 标题`） |
| 边框 | 当前版本 `border` 不落地（快照与 xlsx 都不带） | 写了等于没写，且无提示 |
| 列宽 | 总宽 ≤ 纸张可用宽（默认 A4 约 172mm；有 `print_setup` 时按 `paper_width - 左右边距`），单列放 `#,##0.00` 至少 18mm | 超宽横向分页；窄列显示 `####` |
| 跨组占比 | 用 `sum(B4[\`0]{})` 取整根格集 | `ds.sum(amount,cond)` 只汇总当前组，占比恒 100% |

两级分组（部门 → 费用项）的最小写法：

```json
{ "row": 4, "col": 1, "extend_mode": "vertical", "expr": "exp.group(dept)" },
{ "row": 4, "col": 2, "extend_mode": "vertical", "left_head": "A4", "expr": "exp.group(item)" },
{ "row": 4, "col": 3, "extend_mode": "none", "left_head": "B4", "expr": "exp.sum(amount)" }
```

> 把 `group(item)` 换成 `select(item)` 放在非扩展列上，得到的是 `"差旅,招待"` 这样的**逗号串**，
> 金额会算成 0 —— 行数不变、不报错、闸门也可能 PASS，只有 `--expect-cell` 能抓住。

表达式速查（`rpx catalog functions` 是权威）：

| 目的 | 写法 |
| --- | --- |
| 分组驱动 | `ds.group(field)` / `ds.group(field;desc)` |
| 组内聚合 | `ds.sum(field, 条件)`、`ds.count()`、`ds.avg(field)` |
| 明细展开 | `ds.select(field, 条件)` |
| 跨行累计 | `E4[-1] + C4 - D4`（列号必须真实存在，否则 `CircularRef`） |
| 整根格集汇总 | `sum(B4[\`0]{})` |
| 序号 | `seq(A4)` |

## 第 5 步：过闸（必跑）

```sh
python3 scripts/verify_report.py template.json --data-dir ./data \
  --expect-rows 10 --max-pages 1 --print \
  --expect-cell C4=12000 --expect-cell C9=71000
```

- `--print` 才验证 `print-xlsx`（图形画布类模板不需要）。
- 页数上限也可以从 `rpx run` 的 `data.layout.page_count` 直接读到，不必开 `workbook.json`。
- `--expect-cell` 是**唯一能证明"算对了"的检查**：行数对不代表值对。作用域写错时值会静默错位。

它做四件事：`rpx lint`（静态）→ `validate` → `run`（csv + xlsx，`--print` 再加 print-xlsx）→ 读产物断言。
静态规则**只有一份实现**（在 CLI 里），脚本只是调用方——不要在别处再养一套。

L 码来自 `rpx lint`，可以单独跑：

```sh
rpx lint template.json --data-dir ./data --strict   # 有告警即退出 3，可进 CI
```

| 码 | 含义 | 处理 |
| --- | --- | --- |
| L001 | `text` 里写了宏 | 移到 `expr` |
| L002 | 同排多个纵向扩展有多个根（未收敛到唯一根） | 给子级加 `left_head` 指向父格；兄弟可共享父格 |
| L003 | 扩展块内有空行 | 删掉空行，合计行紧贴扩展行 |
| L004 | 声明了 border | 导出不落地，改用底色分层 |
| L005 | 数据集文件不存在 | 修 `--data-dir` 或文件名 |
| L006 | CSV 未写 delimiter | 加 `"delimiter": ","` |
| L007 | 列宽合计超可用宽度（按 print_setup 纸张算，默认 190mm） | 压列宽或减列 |
| L008 | 两位小数配窄列 | 改 `#,##0` 或加宽 |
| R000 | `rpx lint` 不可用（二进制过旧） | `make install-local` |
| R001/R002 | validate / run 失败 | 看 `error.code` / `error.hint` + `run.json` 的 diagnostics |
| R004 | CSV 行数不符 | 优先怀疑空行/多扩展列 |
| R005 | 产物里漏出 `${` | 宏没生效 |
| R006 | 页数超上限 | 收列宽/行高/边距 |
| R007 | 边框未落地（动态确认） | 同 L004 |
| R008 | `--expect-cell` 断言不符 | 九成是 `left_head` 指错父格，或 `select` 返回了集合 |

退出码 0 = 可交付；1 = 有 error。**只认退出码，不认"看起来没问题"。**

## 第 6 步：交付与回归

```sh
rpx result inspect RUN/artifacts/result.json --origin C6      # 按模板坐标核对某个格
rpx result inspect RUN/artifacts/result.json --range A1:D10   # 按结果坐标扫一片
```

- 结果值是 `{type,value}`；long/decimal 的 `value` 是**字符串**，不要强转 number。
- 产物：`result.json`（值）/`workbook.json`（快照 + `pagination`）/`report.csv`/`report.xlsx`/`report-print.xlsx`。
- 页数直接读 `workbook.json.pagination.page_count`，横向看 `x_pages`。
- 要 PDF 不要用 CLI（没有该导出）：走 Web 打印预览另存。

## 反模式

- ❌ 先写 20 个单元格再跑 → 一次报 1 个错，来回 20 轮。正确做法：先 1 列明细 + 1 个合计跑通，再长版式。
- ❌ 用 `validate` 的 `valid:true` 当作"能交付"。它只证明结构合法。
- ❌ 看到 `UnknownCell` 就去加字段 → 先 `dataset inspect`，九成是分隔符/编码/表头问题。
- ❌ 改完只跑 `run` 不检查产物 → 行数、`####`、字面量 `${` 都不会报错。
- ❌ 在共享库里直接覆盖 `template.json` → 必须 `report show` 取 revision + `--if-match`，先 `--dry-run`。
