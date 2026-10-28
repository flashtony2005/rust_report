# rpx 预制报表模板目录与制作规范

> 目标：基于现有 rpx 报表引擎，建立可检索、可组合、可验证的预制业务报表模板库。
>
> 重要边界：当前 `rpx-spec/v1` 编译器支持 `list`（明细表）、`grouped`（分组汇总）、`crosstab`（交叉表）三种布局；本目录中的模板均以这三种布局组合实现。仪表盘、多图表、多工作表等复杂展示不假设已由 `rpx-spec/v1` 直接支持，须在现有模板能力验证通过后另行实现。
>
> 本文定义的是“预制模板产品目录 + 数据契约 + 规格草案”。正式入库前，需按项目当前 `template.json` 结构和真实样例数据编译、lint、运行及核对，不应仅凭本文中的规格草案直接认定模板已经可运行。

---

## 1. 预制模板设计原则

### 1.1 模板的组成

每个可交付模板至少应包含：

```text
<template-id>/
├── manifest.json       # 人工维护的业务名称、描述、tags
├── template.json       # rpx 引擎可执行模板
└── data/
    └── sample.csv      # 可复现的示例数据
```

如果模板由 `rpx-spec/v1` 生成，建议开发期间另外保存：

```text
spec/
└── <template-id>.spec.json
```

`spec` 是源规格；`template.json` 是编译结果。若仓库约定不允许模板目录增加 `spec/`，则将源规格统一放到独立的开发目录，正式模板目录仍遵循引擎现有结构。

### 1.2 两层元数据必须分开

- **手写业务语义层**：名称、描述、业务域、关键词、口语例句、难度。由人维护。
- **程序推导结构层**：布局形态、扩展方向、能力标签、引用字段、SQL 依赖。由 `scripts/template_metadata.py` 从模板推导，并由 `check` 验证。
- `manifest.tags` 只放能力与形态短标签，例如 `list`、`grouped`、`crosstab`、`agg_sum`；业务域放 `meta.domain`，不要重复维护两份业务域标签。

### 1.3 模板质量门槛

每个预制模板必须满足：

1. 有清楚、可验证的业务问题和适用范围。
2. 有字段契约：字段名、含义、类型、是否必需、示例。
3. 指标有明确口径，不允许只写“金额”“利润”等模糊名称。
4. 至少有一份小型、可复现的样例数据。
5. 能通过模板校验和 lint；有数据时必须实际运行。
6. 核对明细数量、分组汇总、总计和边界情况。
7. `manifest.json` 中的手写元数据完整，推导元数据通过 `check`。
8. 不把尚未支持的布局、函数或导出能力写成已实现能力。

---

## 2. 预制模板总目录

本版建议先建立 **30 个预制模板**，分为 8 个业务域。30 是本项目的建设目标，不是引擎能力数量或行业标准。

| ID | 业务域 | 预制模板 | 推荐布局 | 关键输出 |
|---|---|---|---|---|
| FIN-001 | 财务 | 收支明细表 | list | 日期、类别、摘要、收入、支出 |
| FIN-002 | 财务 | 月度收支汇总表 | grouped | 按月份汇总收入/支出 |
| FIN-003 | 财务 | 费用类别分析表 | grouped | 按部门/类别汇总费用 |
| FIN-004 | 财务 | 应收账款账龄明细表 | list | 客户、余额、账龄天数 |
| FIN-005 | 财务 | 应收账款账龄分组表 | grouped | 按账龄区间汇总余额 |
| FIN-006 | 财务 | 预算执行对比表 | crosstab | 部门 × 预算项目的预算/实际 |
| SAL-001 | 销售 | 销售订单明细表 | list | 订单、日期、客户、销售人员、金额 |
| SAL-002 | 销售 | 销售人员业绩汇总表 | grouped | 销售人员销售额、订单数 |
| SAL-003 | 销售 | 月度销售趋势表 | grouped | 按月份汇总销售额 |
| SAL-004 | 销售 | 产品销售汇总表 | grouped | 按产品汇总销量、销售额 |
| SAL-005 | 销售 | 区域销售交叉表 | crosstab | 区域 × 产品类别销售额 |
| SAL-006 | 销售 | 客户销售排行表 | grouped | 客户销售额排序 |
| INV-001 | 库存 | 库存现状明细表 | list | SKU、仓库、现存数量 |
| INV-002 | 库存 | 商品库存汇总表 | grouped | 按商品汇总库存 |
| INV-003 | 库存 | 仓库库存交叉表 | crosstab | 仓库 × 商品类别库存 |
| INV-004 | 库存 | 出入库流水表 | list | 日期、SKU、入库量、出库量 |
| INV-005 | 库存 | 低库存预警清单 | list | 当前库存、安全库存、差额 |
| INV-006 | 库存 | 库存盘点差异表 | list | 账面数量、实盘数量、差异 |
| HR-001 | 人事 | 员工花名册 | list | 员工编号、姓名、部门、岗位 |
| HR-002 | 人事 | 部门人数汇总表 | grouped | 部门人数 |
| HR-003 | 人事 | 考勤异常明细表 | list | 员工、日期、异常类型 |
| HR-004 | 人事 | 部门薪酬汇总表 | grouped | 部门人数、薪酬总额 |
| PRJ-001 | 项目 | 项目任务跟踪表 | list | 任务、负责人、计划日期、状态 |
| PRJ-002 | 项目 | 项目进度状态汇总表 | grouped | 项目/状态任务数 |
| PRJ-003 | 项目 | 项目成本汇总表 | grouped | 项目预算、实际成本 |
| OPS-001 | 运营 | 门店经营汇总表 | grouped | 门店销售额、订单数 |
| OPS-002 | 运营 | KPI 指标明细表 | list | 指标、目标、实际、单位 |
| OPS-003 | 运营 | 问题整改跟踪表 | list | 问题、责任人、截止日期、状态 |
| DAT-001 | 通用分析 | 分类占比基础表 | grouped | 分类数量/金额，可供外层计算占比 |
| DAT-002 | 通用分析 | 多维交叉统计表 | crosstab | 行维度 × 列维度的聚合指标 |

### 2.1 第一批优先级

先落地以下 10 个模板，覆盖三种编译布局和最常见的数据处理路径：

1. `SAL-001` 销售订单明细表（list）
2. `SAL-002` 销售人员业绩汇总表（grouped）
3. `SAL-003` 月度销售趋势表（grouped）
4. `SAL-005` 区域销售交叉表（crosstab）
5. `FIN-001` 收支明细表（list）
6. `FIN-005` 应收账款账龄分组表（grouped）
7. `INV-001` 库存现状明细表（list）
8. `INV-003` 仓库库存交叉表（crosstab）
9. `HR-002` 部门人数汇总表（grouped）
10. `OPS-003` 问题整改跟踪表（list）

---

## 3. 模板字段契约与指标口径

字段名是模板契约的一部分。上传文件的列名不一致时，应先通过字段映射或数据预处理对齐，不要让模板默默猜测字段。

### 3.1 销售订单数据集 `sales_orders`

| 字段 | 类型 | 含义 | 必需 | 示例 |
|---|---|---|---|---|
| `order_id` | string | 订单唯一编号 | 是 | SO-1001 |
| `order_date` | date/string | 订单日期 | 是 | 2026-01-03 |
| `customer` | string | 客户名称 | 是 | 客户 A |
| `salesperson` | string | 销售人员 | 是 | 张三 |
| `region` | string | 区域 | 是 | 华东 |
| `category` | string | 产品类别 | 是 | 办公用品 |
| `product` | string | 产品名称 | 是 | 产品 X |
| `quantity` | number | 销售数量 | 是 | 10 |
| `amount` | number | 订单销售金额 | 是 | 1200 |
| `cost` | number | 成本金额 | 否 | 800 |

口径约定：
- 销售额：`SUM(amount)`。
- 订单数：如果引擎聚合能力不能保证去重计数，不得把普通 `count(order_id)` 宣称为去重订单数；应保证样例一行一订单，或先预处理去重。
- 毛利：只有 `cost` 字段存在且口径明确时才计算；可定义为销售额减成本，但需确认 `amount` 与 `cost` 的含税/未税口径一致。
- 月度分组：只有在当前编译器支持日期提取表达式时才直接按日期月份分组；否则预处理生成 `month` 字段（如 `2026-01`），再将其作为普通维度。

### 3.2 财务收支数据集 `finance_transactions`

| 字段 | 类型 | 含义 | 必需 | 示例 |
|---|---|---|---|---|
| `date` | date/string | 发生日期 | 是 | 2026-01-03 |
| `department` | string | 部门 | 否 | 销售部 |
| `category` | string | 收支类别 | 是 | 差旅费 |
| `summary` | string | 摘要 | 否 | 客户拜访 |
| `income` | number | 收入金额 | 是 | 0 |
| `expense` | number | 支出金额 | 是 | 350 |
| `account` | string | 账户 | 否 | 银行账户 A |

约定：收入与支出分别求和；净额可在数据预处理层生成 `net_amount = income - expense`，除非模板表达式能力已验证支持所需运算。

### 3.3 库存数据集 `inventory`

| 字段 | 类型 | 含义 | 必需 | 示例 |
|---|---|---|---|---|
| `sku` | string | 商品编码 | 是 | SKU-001 |
| `product` | string | 商品名称 | 是 | 产品 X |
| `category` | string | 商品类别 | 是 | 原材料 |
| `warehouse` | string | 仓库 | 是 | 一号仓 |
| `quantity` | number | 当前库存数量 | 是 | 100 |
| `safety_stock` | number | 安全库存 | 否 | 30 |
| `unit_cost` | number | 单位成本 | 否 | 12.5 |

约定：库存金额只有在单位成本口径一致时才能计算；低库存清单需要 `quantity < safety_stock` 的过滤能力。若当前模板规格不支持过滤表达式，应先生成预过滤数据集，不能把未过滤的明细表命名为“低库存预警清单”。

### 3.4 员工数据集 `employees`

| 字段 | 类型 | 含义 | 必需 | 示例 |
|---|---|---|---|---|
| `employee_id` | string | 员工唯一编号 | 是 | E-001 |
| `name` | string | 姓名 | 是 | 张三 |
| `department` | string | 部门 | 是 | 销售部 |
| `position` | string | 岗位 | 否 | 销售专员 |
| `hire_date` | date/string | 入职日期 | 否 | 2024-05-01 |
| `status` | string | 在职状态 | 是 | 在职 |
| `salary` | number | 薪酬金额 | 否 | 8000 |

约定：部门人数应基于一行一员工的数据集计数；如果员工表存在历史变更记录，应先按业务规则筛选当前有效记录。

### 3.5 通用任务数据集 `tasks`

| 字段 | 类型 | 含义 | 必需 | 示例 |
|---|---|---|---|---|
| `task_id` | string | 任务编号 | 是 | T-001 |
| `project` | string | 项目名称 | 否 | 报表改造 |
| `task_name` | string | 任务名称 | 是 | 完成字段映射 |
| `owner` | string | 负责人 | 是 | 李四 |
| `status` | string | 状态 | 是 | 进行中 |
| `priority` | string | 优先级 | 否 | 高 |
| `plan_start` | date/string | 计划开始日期 | 否 | 2026-10-20 |
| `due_date` | date/string | 截止日期 | 否 | 2026-10-25 |
| `actual_cost` | number | 实际成本 | 否 | 1200 |
| `budget` | number | 预算金额 | 否 | 2000 |

---

## 4. `rpx-spec/v1` 规格模板

以下是规格构造模式。`dataset.file`、字段名和布局必须与实际样例数据一致；生成后应先 `spec analyze`，再 `spec compile`，最后 lint 和运行。没有经过真实引擎验证的 JSON 不应直接放进正式模板库。

### 4.1 明细表（list）

适用于订单明细、收支明细、员工花名册、任务清单等。

```json
{
  "schema": "rpx-spec/v1",
  "title": "销售订单明细表",
  "dataset": {
    "name": "d",
    "file": "sales-orders.csv"
  },
  "layout": "list",
  "measures": [
    { "field": "amount", "agg": "sum", "label": "销售金额", "format": "#,##0.00" }
  ]
}
```

说明：该示例只展示当前规格的基础形状。`list` 对字段投影、明细列配置的实际行为应以当前引擎 schema 和现有模板为准；如果 `list` 的编译结果不能直接输出全部原始字段，应从已有可运行模板复制其明细布局结构，不要猜测未记录的字段。

### 4.2 分组汇总（grouped）

适用于销售人员业绩、按月销售、部门人数、费用分类汇总等。

```json
{
  "schema": "rpx-spec/v1",
  "title": "销售人员业绩汇总表",
  "dataset": {
    "name": "d",
    "file": "sales-orders.csv"
  },
  "layout": "grouped",
  "measures": [
    { "field": "amount", "agg": "sum", "label": "销售额", "format": "#,##0.00" }
  ],
  "row_dims": [
    { "field": "salesperson", "label": "销售人员", "sort": "asc" }
  ],
  "totals": {
    "grand": true,
    "subtotal": true
  }
}
```

### 4.3 交叉表（crosstab）

适用于区域 × 品类、仓库 × 商品类别等多维交叉统计。

```json
{
  "schema": "rpx-spec/v1",
  "title": "区域销售交叉表",
  "dataset": {
    "name": "d",
    "file": "sales-orders.csv"
  },
  "layout": "crosstab",
  "measures": [
    { "field": "amount", "agg": "sum", "label": "销售额", "format": "#,##0.00" }
  ],
  "row_dims": [
    { "field": "region", "label": "区域", "sort": "asc" }
  ],
  "col_dims": [
    { "field": "category", "label": "产品类别", "sort": "asc" }
  ],
  "totals": {
    "grand": true,
    "subtotal": true
  }
}
```

### 4.4 过滤、同比、环比、百分比与预警

这些需求不能仅凭模板名称就假设已被 `rpx-spec/v1` 支持。制作前先确认当前 spec schema、表达式目录和模板实例是否支持对应能力。

- **过滤**：如果 spec 没有正式的过滤字段，先在数据准备阶段过滤并生成专用 CSV。
- **同比/环比**：需要时间对齐、上一期数据、零分母处理和明确的期间边界；基础三布局不自动等同于完整同比引擎。
- **占比**：需要分组总计和分母口径。若当前表达式无法稳定计算，先由数据准备步骤生成占比字段。
- **低库存预警**：先过滤 `quantity < safety_stock`，再使用 `list` 输出。
- **预算执行对比**：先将预算与实际整理成统一数据集，并明确“预算”和“实际”的字段/类别映射；交叉表只负责已支持的维度聚合，不自动完成所有差异计算。

---

## 5. 30 个模板的逐项规格说明

下表是模板制作任务单。每个模板至少要补齐对应字段、样例 CSV、spec、编译产物、manifest 元数据和验证记录。

### 财务（FIN）

#### FIN-001 收支明细表
- ID：`fin-income-expense-detail`
- 布局：`list`
- 数据集：`finance_transactions`
- 维度/展示字段：`date, category, summary, income, expense, account`
- 指标：收入合计、支出合计；如需净额，预处理生成 `net_amount`
- 口语需求：列出本月所有收支记录，分别显示收入和支出。
- 边界：收入/支出为空值如何处理；币种是否统一。

#### FIN-002 月度收支汇总表
- ID：`fin-monthly-summary`
- 布局：`grouped`
- 字段：`month, income, expense`
- 指标：`SUM(income)`、`SUM(expense)`
- 口语需求：按月份汇总每月收入和支出。
- 前置：若引擎不能直接提取月份，数据准备阶段生成 `month`。

#### FIN-003 费用类别分析表
- ID：`fin-expense-by-category`
- 布局：`grouped`
- 字段：`department, category, expense`
- 指标：`SUM(expense)`
- 口语需求：按部门和费用类别统计支出金额。
- 边界：部门为空值的归类规则。

#### FIN-004 应收账款账龄明细表
- ID：`fin-ar-aging-detail`
- 布局：`list`
- 字段：`customer, invoice_id, due_date, amount, days_past_due`
- 指标：应收余额；账龄天数由数据源提供或预处理。
- 口语需求：列出所有未收款发票和逾期天数。
- 边界：未到期账款、负数余额、币种。

#### FIN-005 应收账款账龄分组表
- ID：`fin-ar-aging-summary`
- 布局：`grouped`
- 字段：`aging_bucket, amount`
- 指标：`SUM(amount)`
- 口语需求：按未到期、1–30 天、31–60 天、61–90 天、90 天以上统计应收余额。
- 前置：由数据准备阶段生成 `aging_bucket`，避免假设编译器自动支持区间分桶。

#### FIN-006 预算执行对比表
- ID：`fin-budget-vs-actual`
- 布局：`crosstab`
- 字段：`department, budget_item, value_type, amount`
- 行维度：`department, budget_item`
- 列维度：`value_type`（预算/实际）
- 指标：`SUM(amount)`
- 口语需求：按部门和预算项目对比预算与实际发生额。
- 前置：先把预算和实际数据规范化到同一数据集。

### 销售（SAL）

#### SAL-001 销售订单明细表
- ID：`sales-order-detail`
- 布局：`list`
- 字段：`order_id, order_date, customer, salesperson, product, quantity, amount`
- 指标：销售金额；订单明细以一行一条记录为约定。
- 口语需求：列出销售订单明细，按日期查看客户、产品、数量和金额。
- 边界：订单头表与订单明细表不可未经去重直接连接汇总订单总额。

#### SAL-002 销售人员业绩汇总表
- ID：`sales-by-salesperson`
- 布局：`grouped`
- 字段：`salesperson, amount`
- 指标：`SUM(amount)`
- 口语需求：按销售人员统计销售额，并按销售人员名称排序。
- 注意：若需按销售额降序排名，应确认排序能力是否支持指标排序；不支持时由数据准备步骤预排序或增加排名字段。

#### SAL-003 月度销售趋势表
- ID：`sales-monthly-trend`
- 布局：`grouped`
- 字段：`month, amount`
- 指标：`SUM(amount)`
- 口语需求：统计每个月的销售额，按月份顺序展示。
- 前置：生成可排序的 `month` 字段（例如 `2026-01`），并显式指定排序。

#### SAL-004 产品销售汇总表
- ID：`sales-by-product`
- 布局：`grouped`
- 字段：`product, quantity, amount`
- 指标：`SUM(quantity)`、`SUM(amount)`
- 口语需求：按产品统计销量和销售额。
- 边界：数量单位是否一致、退货是否以负数记录。

#### SAL-005 区域销售交叉表
- ID：`sales-region-category-crosstab`
- 布局：`crosstab`
- 行维度：`region`
- 列维度：`category`
- 指标：`SUM(amount)`
- 口语需求：行按区域、列按产品类别，交叉格统计销售额，并显示总计。
- 边界：区域或类别空值、列维度基数过高。

#### SAL-006 客户销售排行表
- ID：`sales-by-customer`
- 布局：`grouped`
- 字段：`customer, amount`
- 指标：`SUM(amount)`
- 口语需求：统计每个客户的累计销售额，找出主要客户。
- 注意：真正的 Top N 需要过滤/排名能力；若未支持，输出完整汇总表并在后处理中排名，不要把完整列表称为 Top 10。

### 库存（INV）

#### INV-001 库存现状明细表
- ID：`inventory-current-detail`
- 布局：`list`
- 字段：`sku, product, category, warehouse, quantity, unit_cost`
- 指标：当前库存数量；库存金额需有统一单位成本。
- 口语需求：列出每个仓库的商品库存现状。
- 边界：同一 SKU 在多个仓库是否应分行。

#### INV-002 商品库存汇总表
- ID：`inventory-by-product`
- 布局：`grouped`
- 字段：`product, quantity`
- 指标：`SUM(quantity)`
- 口语需求：汇总每种商品在所有仓库的库存数量。
- 边界：同一 SKU 不同计量单位不可直接相加。

#### INV-003 仓库库存交叉表
- ID：`inventory-warehouse-category-crosstab`
- 布局：`crosstab`
- 行维度：`warehouse`
- 列维度：`category`
- 指标：`SUM(quantity)`
- 口语需求：按仓库和商品类别交叉统计库存。
- 边界：列维度类别数量应有限。

#### INV-004 出入库流水表
- ID：`inventory-movement-detail`
- 布局：`list`
- 字段：`date, sku, product, warehouse, inbound_qty, outbound_qty, reference_id`
- 指标：出入库流水明细。
- 口语需求：查询某段时间内的出入库记录。
- 前置：日期过滤若未被模板规格支持，应由数据准备阶段完成。

#### INV-005 低库存预警清单
- ID：`inventory-low-stock`
- 布局：`list`
- 字段：`sku, product, warehouse, quantity, safety_stock, shortage`
- 指标：`shortage = safety_stock - quantity`
- 口语需求：列出低于安全库存的商品，并显示缺口数量。
- 前置：数据集必须预先过滤 `quantity < safety_stock`，并生成 `shortage`。

#### INV-006 库存盘点差异表
- ID：`inventory-stocktake-diff`
- 布局：`list`
- 字段：`sku, product, warehouse, book_qty, counted_qty, diff_qty`
- 指标：`diff_qty = counted_qty - book_qty`
- 口语需求：对比账面库存与实盘数量，列出差异。
- 前置：如表达式运算能力未经验证，预处理生成 `diff_qty`。

### 人事（HR）

#### HR-001 员工花名册
- ID：`hr-employee-roster`
- 布局：`list`
- 字段：`employee_id, name, department, position, hire_date, status`
- 口语需求：生成员工花名册，按部门和姓名查看员工信息。
- 边界：隐私字段按最小必要原则提供。

#### HR-002 部门人数汇总表
- ID：`hr-headcount-by-department`
- 布局：`grouped`
- 字段：`department, employee_id`
- 指标：`COUNT(employee_id)`；前提是一行一名当前有效员工。
- 口语需求：统计各部门在职人数。
- 前置：先过滤 `status=在职`，除非过滤已由引擎规格支持。

#### HR-003 考勤异常明细表
- ID：`hr-attendance-exceptions`
- 布局：`list`
- 字段：`date, employee_id, name, department, exception_type, minutes`
- 口语需求：列出迟到、早退、缺勤等考勤异常。
- 前置：数据源应已筛选异常记录。

#### HR-004 部门薪酬汇总表
- ID：`hr-payroll-by-department`
- 布局：`grouped`
- 字段：`department, salary`
- 指标：`SUM(salary)`；如需平均工资，确认 `avg` 口径与空值规则。
- 口语需求：按部门汇总薪酬总额。
- 权限：薪酬属于敏感业务数据，模板本身不代表用户获得查看权限。

### 项目（PRJ）

#### PRJ-001 项目任务跟踪表
- ID：`project-task-tracker`
- 布局：`list`
- 字段：`project, task_id, task_name, owner, priority, status, plan_start, due_date`
- 口语需求：查看各项目任务、负责人、优先级、截止日期和状态。
- 边界：日期格式统一；逾期标记应在数据准备阶段生成，除非过滤/日期函数已验证。

#### PRJ-002 项目进度状态汇总表
- ID：`project-status-summary`
- 布局：`grouped`
- 字段：`project, status, task_id`
- 指标：`COUNT(task_id)`；前提是一行一任务。
- 口语需求：按项目和状态统计任务数量。
- 边界：取消任务是否计入总任务数需明确。

#### PRJ-003 项目成本汇总表
- ID：`project-cost-summary`
- 布局：`grouped`
- 字段：`project, budget, actual_cost`
- 指标：分别汇总预算与实际成本。
- 口语需求：按项目查看预算和实际成本。
- 注意：预算若在任务表重复出现，不能直接对每行预算求和；应先规范到项目粒度。

### 运营（OPS）

#### OPS-001 门店经营汇总表
- ID：`store-performance-summary`
- 布局：`grouped`
- 字段：`store, amount`
- 指标：`SUM(amount)`
- 口语需求：按门店汇总营业额。
- 扩展：订单数、客单价需要订单粒度和除零规则。

#### OPS-002 KPI 指标明细表
- ID：`kpi-metric-detail`
- 布局：`list`
- 字段：`period, department, metric_name, target_value, actual_value, unit`
- 口语需求：列出各部门每项 KPI 的目标值和实际值。
- 注意：达成率需要确认目标值为零或缺失时的处理规则。

#### OPS-003 问题整改跟踪表
- ID：`issue-action-tracker`
- 布局：`list`
- 字段：`issue_id, issue, owner, priority, status, due_date, resolution`
- 口语需求：查看问题、责任人、截止日期、处理状态和整改措施。
- 扩展：逾期筛选与状态预警可由数据准备步骤生成。

### 通用分析（DAT）

#### DAT-001 分类汇总基础表
- ID：`generic-category-summary`
- 布局：`grouped`
- 字段：`category, value`
- 指标：`SUM(value)`
- 口语需求：按类别汇总数量或金额。
- 用途：作为占比、排名和图表的基础数据源。

#### DAT-002 多维交叉统计表
- ID：`generic-multidimensional-crosstab`
- 布局：`crosstab`
- 字段：由调用方指定行维度、列维度和度量字段。
- 指标：`sum`、`count` 或 `avg`，以当前 schema 支持项为准。
- 口语需求：行按指定维度展开，列按另一维度展开，交叉格聚合指定指标。
- 边界：字段必须存在；维度排序、空值、列基数和总计口径必须经分析确认。

---

## 6. 从目录项制作成真正可运行的模板

对每一个模板按以下流水线执行：

```text
确认业务口径
   ↓
定义字段契约
   ↓
准备 sample.csv
   ↓
编写 rpx-spec/v1（仅使用已支持能力）
   ↓
rpx dataset inspect
   ↓
rpx spec analyze
   ↓
修订规格并再次 analyze
   ↓
rpx spec compile
   ↓
rpx lint
   ↓
rpx run 导出 xlsx + csv
   ↓
核对输出与预期结果
   ↓
编写 manifest 业务元数据
   ↓
template_metadata.py apply / check
   ↓
进入正式模板库
```

### 建议开发目录

```text
presets-work/
├── specs/
├── data/
├── expected/
└── validation/
```

每个模板在 `expected/` 中保存人工计算的预期结果，在 `validation/` 中记录：

- 模板 ID、版本、日期
- 输入数据文件与字段
- analyze 的 findings / open_questions 处理结果
- lint 的 issues / gate
- 输出行列数
- 总计与关键分组的预期值、实际值
- 导出文件是否可打开
- 未覆盖的能力与已知限制

### 单模板验收清单

- [ ] ID 唯一，名称清楚，描述可检索。
- [ ] 数据契约完整，字段名与样例一致。
- [ ] `rpx schema` / 现有样例确认规格结构正确。
- [ ] `rpx spec analyze` 无未处理的关键问题。
- [ ] `rpx spec compile` 成功。
- [ ] `rpx lint --strict` 通过，或已书面批准例外。
- [ ] `rpx run` 至少生成 CSV 和 XLSX。
- [ ] 总计、分组值、空值、零值均有测试。
- [ ] 元数据 `check` 通过。
- [ ] 在目录中记录模板能力边界。

---

## 7. 命名、标签与检索约定

### 7.1 ID 规范

使用稳定、英文、小写、连字符分隔的 ID：

```text
sales-order-detail
sales-by-salesperson
sales-region-category-crosstab
fin-ar-aging-summary
inventory-low-stock
```

ID 不应随显示名称轻微调整而变化；若业务口径发生不兼容变化，应提升模板版本或创建新 ID。

### 7.2 manifest 业务元数据示例

下面是概念示例，实际字段必须遵循当前仓库的 manifest schema：

```json
{
  "name": "销售人员业绩汇总表",
  "description": "按销售人员汇总销售金额，用于日常业绩检查。",
  "tags": ["grouped", "agg_sum"],
  "meta": {
    "schema": "rpx-template-meta/v1",
    "kind": "business",
    "domain": "sales",
    "keywords": ["销售人员", "业绩汇总", "销售额", "销售统计"],
    "examples": [
      "统计每个销售人员的销售额",
      "做一张销售业绩汇总表"
    ],
    "difficulty": 1
  }
}
```

`shape`、`extends`、`capabilities`、`datasets.fields` 等结构事实由工具推导；不要手工编造后写入。

### 7.3 推荐标签

- 形态：`list`、`grouped`、`crosstab`
- 聚合：`agg_sum`、`agg_count`、`agg_avg`（仅在模板实际使用时添加）
- 业务域：放 `meta.domain`，例如 `finance`、`sales`、`inventory`、`hr`、`project`、`operations`、`generic`
- 业务关键词：使用用户实际会说的短语，不要只放开发术语。

---

## 8. 后续扩展原则

1. **先复用三种布局，再增加引擎能力。** 先证明 30 个目录项中哪些能用现有能力直接完成，哪些需要预处理。
2. **计算与展示分离。** 业务指标、筛选、时间分桶尽可能在明确的数据准备层或已验证的表达式中完成；模板只承诺实际执行的逻辑。
3. **预制模板必须带数据契约。** 没有字段定义与样例数据的模板只是一个名字，不是可复用资产。
4. **复杂仪表盘作为后续能力。** 当前文档只把 list/grouped/crosstab 作为已确认的 spec 布局，不将图表、仪表盘、同比环比等扩展能力冒充为已实现。
5. **模板增长以验证通过数衡量。** 不以目录项数量作为完成标准。第一阶段目标是 10 个模板完整通过端到端验证，再扩展到 30 个。

---

## 9. 本目录与 `usage.md` 的关系

- 本文件负责：预制模板分类、业务口径、字段契约、模板制作规范和验收清单。
- `usage.md` 负责：环境、CLI、`rpx-spec/v1`、MCP、完整执行流程和排错。
- 新增模板时先按本目录定义业务和字段，再遵循 `usage.md` 的命令链执行。
