# rpx-query/v1 —— 语义查询契约（查询 IR）

> 状态：P0 定稿（2026-10-23）。架构依据：`docs/report_studio_business_model_sql_architecture.html` §04 L3。
>
> **独立契约**：从第一天定死，不混进 rpx-spec。它表达「查什么数据」，不表达「怎么摆报表」。
> 语言无关（报表、代码生成、API 都可消费）；LLM 的产出物就是这个 JSON，规划器/编译器是确定性程序。

## 0. 与相邻契约的边界

```
自然语言需求
   ↓ 理解层（LLM，只做概念绑定）
rpx-query/v1        ← 本契约：查什么（指标、维度、过滤、时间、排序、限制）
   ↓ 查询规划（L4，确定性：JOIN 路径 / 粒度 / 去重）
   ↓ SQL 编译（L5，确定性：方言 / AST 校验 / 护栏）
SQL
   ↓
结果数据 → rpx-spec/v1（怎么摆：版式 / 样式 / 分页）→ 报表产物
```

- **rpx-query ≠ rpx-spec**：query 说「按区域取净销售额」，spec 说「交叉表、表头蓝色、分 3 页」。
- **rpx-query ≠ SQL**：query 不含 JOIN、不含方言、不含物理表名 —— 那些是 L4/L5 的职责。
- **rpx-query 引用 bizmodel 的 id**（`net_sales`、`Region`），不引用物理列名 —— 概念必须已注册。

## 1. 顶层结构

```json
{
  "schema": "rpx-query/v1",
  "intent": "按地区统计最近三个月每月销售额和订单数",
  "metrics": [
    { "ref": "net_sales" },
    { "ref": "order_count" }
  ],
  "dimensions": [
    { "ref": "region" },
    { "ref": "order_month", "time_grain": "month" }
  ],
  "filters": [
    { "ref": "order_status", "op": "eq", "value": "approved" }
  ],
  "time_range": {
    "field": "order_created_at",
    "preset": "last_3_months"
  },
  "sort": [
    { "by": { "metric": "net_sales" }, "direction": "desc" }
  ],
  "limit": 100,
  "ambiguities": []
}
```

## 2. 字段语义

### `intent`（可选）
用户原始需求文本，留痕用。规划器不依赖它。

### `metrics[]`（必填，至少 1）
```json
{ "ref": "net_sales", "label": "净销售额" }
```
- `ref`：bizmodel 里注册的指标 id。**未注册的指标不得出现在这里** —— 未注册意味着口径未定义。
- `label`：可选显示名，缺省取 bizmodel 的 label。

### `dimensions[]`（可选）
```json
{ "ref": "region" }
{ "ref": "order_created_at", "time_grain": "month" }
```
- `ref`：bizmodel 注册的维度 id。
- `time_grain`：仅对 date 维度有效：`day | week | month | quarter | year`。

### `filters[]`（可选）

过滤条件，**按 `scope` 分三档**（business_report_agent.md §4.1「过滤发生在哪个阶段」）：

| scope | 作用位置 | 写法 | SQL |
|---|---|---|---|
| `detail`（缺省） | 明细行 | `{"ref": "<维度>", "op", "value"}` | WHERE |
| `aggregate` | 最终分组 | `{"metric": "<指标>", "op", "value"}` | HAVING |
| `partition` | 先按 `partition_by` 聚合再筛 | `{"metric", "op", "value", "partition_by": ["<维度>"]}` | 半连接子查询 |

```json
"filters": [
  {"ref": "region", "op": "eq", "value": "华北"},
  {"metric": "sales_amount", "op": "gt", "value": 100000},
  {"metric": "sales_amount", "op": "gt", "value": 100000,
   "scope": "partition", "partition_by": ["region"]}
]
```

⚠ `aggregate` 与 `partition` 的区别是**语义**，不是性能：
「今年各地区月度销售额，只看全年总额超 10 万的地区」必须用 `partition`；
用 `aggregate`（HAVING）会变成「某个月的总额超 10 万」，结论完全不同。

`op` 词表：`eq|ne|gt|gte|lt|lte|in|not_in|like|between|is_null|not_null`
（聚合筛只接数值比较 `eq|ne|gt|gte|lt|lte`）。

### `time_range`（可选）
```json
{ "field": "order_created_at", "preset": "last_3_months" }
{ "field": "order_created_at", "start": "2026-07-01", "end": "2026-10-01" }
```
- `preset` 词表（封闭）：`today | yesterday | last_7_days | last_30_days | this_month | last_month |
  this_quarter | this_year | last_year | last_3_months | last_6_months | last_12_months`
- `preset` 与 `start/end` 二选一；同时给 ⇒ 报错（不允许隐式优先级）。
- `field` 必须是 bizmodel 里注册的时间维度。

### `comparison`（可选，需配合 `time_range`）

同比 / 环比 / 上一期。**对比期由本期窗口平移得出**（编译期算，SQL 里只有参数）：

```json
"comparison": {"type": "year_over_year", "metric": "sales_amount"}
```

| type | 对比期 |
|---|---|
| `year_over_year` | 本期两端各 −1 年（2/29 落 2/28） |
| `month_over_month` | 两端各 −1 月 |
| `previous_period` | 整段等长前移（窗口长度不变） |

产出列：`m0`（本期）/ `m0_prev`（对比期）/ `m0_growth`（增长率，`NULLIF` 防零除）。
⚠ 编译时 WHERE 会**同时放行本期与对比期**（只留本期的话对比列永远为空）；
⚠ 没有 `time_range` 时拒绝（无处平移）。

### `sort[]`（可选）
```json
{ "by": { "metric": "net_sales" }, "direction": "desc" }
{ "by": { "dimension": "region" }, "direction": "asc" }
```
- `by` 二选一：`{"metric": <ref>}` 或 `{"dimension": <ref>}`。
- `direction`：`asc | desc`。

### `limit`（可选，整数 > 0）
结果行数上限。缺省由规划器按成本策略决定（护栏会强制兜底）。

### `ambiguities[]`（强烈建议）
理解层遇到歧义时**必须**落这里，而不是猜一个：
```json
{
  "text": "「销售额」可能是含税或不含税",
  "candidates": ["net_sales", "gross_sales"],
  "resolved_as": null
}
```
- `resolved_as: null` ⇒ 需要人拍板；规划器对未决歧义**拒绝编译**或走保守默认并在产物上标注。
- 这是「事实与推断分离」原则在查询层的体现。

## 3. 校验规则（编译前，确定性）

1. `schema` 必须是 `rpx-query/v1`；未知字段报错（不静默丢弃）。
2. `metrics` ≥ 1；所有 `ref` 必须在 bizmodel 注册表中存在。
3. `filters[].op` 与 value 形状匹配（数组类 op 给标量 ⇒ 报错，指到 JSON pointer）。
3b. `filters[].scope` 只允许 `detail|aggregate|partition`；`partition` 必须给非空
   `partition_by`（且每个 id 都必须在业务模型里注册）。
3c. `comparison.type` 只允许三种；`comparison` **必须**配 `time_range`。
4. `time_range.field` 必须是 date 维度；`preset` 与 `start/end` 互斥。
5. `dimensions` 里 date 维度的 `time_grain` 只允许词表值。
6. `sort.by` 引用必须出现在 metrics 或 dimensions 中（排序一个没选的列没有意义）。
7. **粒度检查**（L4 的输入侧）：metrics 的 grain 与 dimensions 的组合粒度冲突时出 warning
   （如对「一行一订单」的金额 metric 用订单明细维度做维度 ⇒ 会重复计数）。
8. `ambiguities` 里 `resolved_as: null` 的条目存在 ⇒ 校验输出 `NEEDS_CLARIFICATION`，
   不产出 SQL（对齐架构文档「未确认的口径不能伪装成确定事实」）。

## 4. 与 rpx-spec/v1 的映射（报表消费者的编译边界）

| rpx-query/v1 | rpx-spec/v1 | 说明 |
|---|---|---|
| `metrics[].ref` | `measures[].expr` | 经 bizmodel 的 expression 编译（SUM/AVG…） |
| `dimensions[]`（非时间） | `row_dims[]` / `col_dims[]` | 由规划器决定放行/列 |
| `dimensions[]`+`time_grain` | `row_dims[]` 的时间分组 | 时间粒度即分组键 |
| `filters[]` | 无对应 | **过滤在 SQL 层生效**，不进报表 spec |
| `time_range` | 无对应 | 同上，进 SQL WHERE |
| `sort[]` | `row_dims[].sort` / 表内排序 | 报表内排序与查询排序分开 |
| `limit` | 无对应 | 分页是报表（print/paginate）的事，与查询限制无关 |

注意方向性：**query → spec 是编译（一对多）**；反向（spec → query）不存在 —— 报表模板是查询结果的展示壳。

## 5. 示例

### 例 1：简单聚合
需求：「统计各区销售额」
```json
{
  "schema": "rpx-query/v1",
  "intent": "统计各区销售额",
  "metrics": [{ "ref": "net_sales" }],
  "dimensions": [{ "ref": "region" }],
  "sort": [{ "by": { "metric": "net_sales" }, "direction": "desc" }],
  "ambiguities": []
}
```

### 例 2：时间趋势 + 歧义登记
需求：「看下最近卖得怎么样」（含糊）
```json
{
  "schema": "rpx-query/v1",
  "intent": "看下最近卖得怎么样",
  "metrics": [{ "ref": "net_sales" }],
  "dimensions": [{ "ref": "order_created_at", "time_grain": "month" }],
  "time_range": { "field": "order_created_at", "preset": "last_6_months" },
  "ambiguities": [
    {
      "text": "「最近」未指定时间范围，暂按近 6 个月；「卖得怎么样」可能指销售额或订单数",
      "candidates": ["net_sales", "order_count"],
      "resolved_as": "net_sales"
    }
  ]
}
```
（`resolved_as` 已填 ⇒ 可编译，但产物上标注假设；为 null ⇒ 拒绝编译等人拍板。）

## 6. 版本化

- 文件名/字段 `schema: "rpx-query/v1"` 固定；破坏性变更升 v2 并提供迁移说明。
- 词表（op / preset / time_grain）是契约的一部分 —— 扩词表 = minor 变更，改词义 = major。
