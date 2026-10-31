# rpx-bizmodel/v1 —— 业务模型契约（核心资产）

> 状态：P0 定稿（2026-10-23）。架构依据：`docs/report_studio_business_model_sql_architecture.html` §03。
>
> **定位**：业务模型是系统的核心资产 —— 独立于物理数据库、LLM Prompt、报表版式。
> 它可以版本化（文件 diff）、审查（每条结论带证据与状态）、测试（口径可回归）。
> **报表生成和代码生成是它的两个下游消费者**，不为任何一方特化。

## 0. 三条原则（对应架构文档 KPI）

1. **事实与推断分离**：`evidence` 说明结论从哪来，`status` 说明能不能直接用。未经确认的口径不得伪装成事实。
2. **查询与展示分离**：本契约只描述「业务世界里有什么」；「查什么」在 `rpx-query/v1`，「怎么摆」在 `rpx-spec/v1`。
3. **验证优先于自信**：执行成功 ≠ 语义正确。`draft` 的东西进入生成链必须显式提示或拦截。

## 1. 顶层结构

```json
{
  "schema": "rpx-bizmodel/v1",
  "version": 1,
  "source": { "dsn": "sales-db", "generated_at": "2026-10-23T21:00:00+08:00",
              "generator": "discovery/P0" },
  "entities":      [ ... ],
  "relationships": [ ... ],
  "metrics":       [ ... ],
  "dimensions":    [ ... ],
  "rules":         [ ... ],
  "open_questions": [ ... ]
}
```

## 2. status 词表（全局）

| status | 含义 | 谁能改 | 下游行为 |
|---|---|---|---|
| `confirmed` | 结构事实（主键、声明外键）或人工确认过的口径 | 人工确认晋升 | 可直接使用 |
| `candidate` | 从命名/类型/样本推导，未确认 | 人工确认晋升 / 重发现覆盖 | 可用但必须显式标注 |
| `draft` | 指标/规则的初始形态，业务含义待定义 | 人工定义后晋升 | **不得**静默进生成链 |

晋升规则：`candidate|draft → confirmed` 只能由人工确认（或业务验收用例通过）触发，不允许自动晋升。

## 3. Entity（实体 = 表的业务化）

```json
{
  "id": "orders",
  "table": "orders",
  "label": "订单",
  "grain": {
    "key": ["id"],
    "description": "一行一个订单",
    "status": "confirmed",
    "evidence": ["primary_key(id)"]
  },
  "status": "candidate",
  "evidence": ["table_exists", "comment:订单主表", "row_count:10000"]
}
```

- `grain` 是**最重要的字段**（架构文档 §05）：连接后重复计数的根源就是粒度不明。
  声明主键 ⇒ `grain.status=confirmed`；只有命名猜测（有 `id` 列）⇒ `candidate`；都没有 ⇒ 记 open_question。
- `description` 是给业务人员看的一句话，推导自 COMMENT/表名，确认后可改写。

## 4. Relationship（关系 = 不只是 JOIN 条件）

```json
{
  "from": { "entity": "order_items", "key": ["order_id"] },
  "to":   { "entity": "orders",      "key": ["id"] },
  "cardinality": "many_to_one",
  "status": "confirmed",
  "evidence": ["declared_foreign_key"]
}
```

- `cardinality`：`many_to_one | one_to_one | one_to_many | many_to_many`。外键侧为 many_to_one；
  从键即主键 ⇒ one_to_one。
- **证据分级**（决定 status）：
  - `declared_foreign_key`（information_schema / PRAGMA）⇒ `confirmed`
  - `column_naming:order_id→orders.id`（命名推断）⇒ `candidate`
  - `data_profile:100% 值域覆盖`（采样验证）⇒ 可把命名推断晋升半级（仍 candidate，直到人工确认）
- ⚠ 外键只能证明**结构关联**，不能决定「有效订单」这类业务定义（架构文档 §02 注脚）。

## 5. Metric（指标 = 不是字段名）

```json
{
  "id": "net_sales",
  "label": "净销售额",
  "entity": "orders",
  "expression": "SUM(order_amount)",
  "filters": [],
  "time_field": "created_at",
  "grain": "order",
  "format": "#,##0.00",
  "status": "draft",
  "evidence": ["column_comment:销售额", "column_type:numeric"]
}
```

- `expression` 引用**实体内字段**（`SUM(order_amount)`），不写跨表 SQL —— join 由查询规划层决定。
- `filters` 是业务规则（如「排除已取消订单」），引用 `rules[]` 的 id。
- 发现管线只能产出 `draft`（字段名/COMMENT 猜不出退款、折扣、含税口径）；**正式口径必须人工确认**。
- 度量函数启发式：率/均/单价 ⇒ `AVG`，其余数值 ⇒ `SUM`；计数类字段名（count/次数）⇒ `COUNT`。
- 与 `rpx-spec/v1` 的关系：**registry 是源，spec 是产物**。`expression` 可编译成 spec 的
  `measures[].expr`，`label/format` 直通 —— 不允许两套口径各写各的。

### 5b. `aliases`（业务别名，2026-10-27 增）

```json
"aliases": ["amount", "销售额", "金额", "收入"]
```

用途**只有两个**：检索（业务人员说「销售额」要能命中 `sales_amount`）与展示。
⚠ 它**不参与口径计算** —— 口径永远以 `expression` + `status` 为准。

发现管线按词表（`bizmodel.COL_ALIASES`）从列名/COMMENT 生成，因此
`merge_confirmed` 对 `aliases`/`label`/`column`/`kind`/`values` 这几项**取新**
（元数据随库结构与词表更新），口径相关字段（`expression`/`filters`/`time_field`/
`status`/`evidence`）仍以人工版为准。

## 6. Dimension（维度）

```json
{
  "id": "region",
  "entity": "customers",
  "label": "区域",
  "kind": "enum",
  "values": ["华北", "华东", "华南"],
  "time_grain": null,
  "status": "candidate",
  "evidence": ["column_comment:区域", "distinct=3"]
}
```

- `kind`：`enum | text | date | number`。date 带 `time_grain` 候选（`day|week|month|quarter|year`）。
- `values` 仅在 distinct 小（≤50）时收录，作为 Schema Mapper 的词表。

## 7. Rule（业务规则 / 口径）

```json
{
  "id": "exclude_cancelled",
  "label": "排除已取消订单",
  "expression": "status <> 'cancelled'",
  "applies_to": ["orders"],
  "status": "draft",
  "evidence": []
}
```

P0 不自动发现规则；人工编写后 `confirmed`，供 metrics.filters 引用。

## 8. open_question（未决问题）

```json
{
  "question": "amount 的业务含义需要确认（金额？数量？含税？）",
  "source": "entity:orders.column:amount",
  "evidence": ["column_type:numeric", "comment_missing"]
}
```

与 `spec analyze` 的 open_questions 同形 —— 只报**确实存在**的分歧，不凑数。

## 9. 版本化与审查

- 落盘位置（Studio）：`data/bizmodel/<dsn>.json` —— 一个数据源一份，git 可 diff。
- 发现管线每次运行**只生成不覆盖 confirmed**：`confirmed` 条目以人工版为准，`candidate/draft` 可被重发现刷新。
- 人工编辑 = 对该文件的 PR 式修改；晋升 status 必须留 evidence 追加（如 `human_confirmed:2026-10-23`）。

## 10. 下游消费

| 消费者 | 用什么 | 不用什么 |
|---|---|---|
| Report Studio 报表生成 | entities/grain + metrics/dimensions → 编译进 rpx-spec | 不直接读物理列猜口径 |
| 代码生成（任意语言） | relationships（join 路径）+ metrics（聚合表达式）+ rules | 不依赖报表版式 |
| 查询规划（L4） | relationships 做 JOIN 计划，grain 做重复计数检查 | — |
| Schema Mapper | dimensions.values + labels 做术语绑定 | — |
