---
name: rpx-bizmodel-management
description: 指导企业报表语义层（rpx-bizmodel/v1）从接入到维护的全生命周期业务知识管理 SOP。涵盖数据源冷启动自动发现、业务专家人工口径核准晋升（draft/candidate → confirmed）、长尾别名同义词沉淀、全局业务规则治理，以及基于 Goldens 回归评测集验证防漂移的标准作业程序。当用户要求管理业务知识、扩充报表指标口径、处理二义性、发现新库或沉淀评测用例时使用。
---

# 业务知识管理 SOP (BizModel Management)

> 适用标准：`rpx-bizmodel/v1` 业务模型契约与 Report Studio 语义层架构。
> 核心定位：业务知识是企业的核心语义资产，**独立于物理数据库、LLM Prompt 和报表版式**。它是一份可版本化、可审查、可回归测试的事实契约。

---

## 核心三铁律

1. **事实与推断严格分离**：未经人工核准的口径（`candidate`/`draft`）**绝不直接**进入自动出表决策链，必须经人工审查带 evidence 晋升为 `confirmed`。
2. **别名只做检索，口径只认表达式**：同义词（`aliases`）只负责用户口语与标准指标/维度的意图链接，**绝对不改变**指标内部的 SQL 聚合公式。
3. **无回归不出库**：任何新增指标、修改规则或追加长尾别名后，**必须运行 Goldens 回归测试集**，确保已有问答不被意外破坏或漂移。

---

## 阶段一：新数据源冷启动与自动发现 (Cold Start & Discovery)

当接入一个新数据库（PostgreSQL / MySQL / SQLite）时，系统自动执行反向工程与语义发现，产出初始的 `candidate` 与 `draft` 资产。

### 1. 实体与粒度识别 (Grain)
- **物理元数据提取**：扫描表名、`COMMENT` 注释、物理主键与唯一键。
- **声明实体粒度**：
  ```json
  "grain": {
    "key": ["order_id"],
    "description": "一行代表一笔结算销售记录",
    "status": "confirmed",
    "evidence": ["declared_primary_key(order_id)"]
  }
  ```
- **警惕笛卡尔积陷阱**：多表关联时，必须依据主从关系定义 cardinality（如 `many_to_one`）。若没有明确主键，粒度标记为 `candidate` 并记入 `open_questions`。

### 2. 候选维度提取 (Dimensions)
- 提取低基数（distinct $\le 50$）分类列，收录其真实值域（`values`）作为实体链接白名单。
- 时间字段自动标注 `kind: "date"`，并列出可用时间粒度（`day / week / month / quarter / year`）。
- 初始状态一律设为 `candidate`，推导依据记入 `evidence: ["column_comment:大区", "distinct=3"]`。

### 3. 候选指标生成 (Metrics)
- **度量启发式规则**：
  - 字段名含“率 / 均 / 比 / 单价” $\rightarrow$ 生成 `AVG(...)`；
  - 字段名含“数 / 量 / 次数” $\rightarrow$ 生成 `COUNT(...)`；
  - 其余金额、数值度量 $\rightarrow$ 生成 `SUM(...)`。
- **状态约束**：自动推断出的指标统一设为 `candidate`；若缺少字段注释且数值含义不明，设为 `draft`，并触发未决问题。

---

## 阶段二：业务专家人工核准与晋升 (Promotion SOP)

自动推断只能解决“结构可用”，无法替代“业务正确”（例如：销售额是否扣除优惠券？是否扣除退款？测试门店是否计入？）。

### 1. 指标公式审查与修正
业务分析师在「业务知识」工作台或直接在 `bizmodel.json` 中审查：
- **原始推断**：`SUM(amount)`
- **修正为真实财务口径**：`SUM(amount) - SUM(COALESCE(discount, 0))`
- **时间锚定**：明确指定 `time_field: "month"`（确定以创建时间还是结算时间统计）。

### 2. 全局业务规则绑定 (Business Rules)
定义全局排他条件，避免用户每次提问都要重复“排除已取消订单”：
```json
{
  "id": "valid_orders",
  "label": "有效结算订单规则",
  "expression": "amount > 0 AND status <> 'cancelled'",
  "applies_to": ["sales"],
  "status": "confirmed"
}
```
在指标的 `filters` 数组中引用该规则 ID：`"filters": ["valid_orders"]`。

### 3. 核准晋升操作 (Promotion)
- 将指标或维度的 `status` 从 `candidate`/`draft` 晋升为 `confirmed`。
- 追加操作凭证：`evidence.push("human_confirmed:YYYY-MM-DD")`。
- **结果**：该指标正式进入可直接用于报表出稿与归因分析的事实层。

---

## 阶段三：长尾同义词扩展与长效增量维护 (Aliases)

业务人员日常提问充满行话、缩写与口语，需要不断扩充同义词图谱。

### 1. 别名扩展规范
- 当业务人员提问：“*看下各店本月流水*”、“*大区 GMV 排行*”：
  1. 识别“流水”、“GMV”未命中现有标准词；
  2. 在「业务知识」页面的 **同义词映射图谱** 或指标卡片中，为 `sales_amount` 点击「＋ 添加同义词」；
  3. 登记别名：`["销售额", "流水", "业绩", "GMV", "进账", "入账金额"]`。
- 为维度枚举值补充俗称：
  - `sales_region = "华东"` 补充别名：`["江浙沪", "华东大区", "东部区域"]`。

### 2. 二义性拦截原则 (Disambiguation)
- **禁止交叉别名**：同一别名不得同时绑定两个不同指标（例如“利润”不能同时作为“毛利”和“净利”的别名）。
- **人机协同确认**：若出现同义词冲突，自然语言解析器必须抛出 `needs_clarification`，向用户弹出选项确认，并在确认后落盘为确定的意图。

---

## 阶段四：业务问题金样例回归与防漂移 (Goldens Regression)

语义层不是静态文档，而是会随业务频繁变更的系统。必须通过持续回归测试保证质量。

### 1. 沉淀标准业务用例
在测试集（如 `studio/questions.json`）中沉淀标准问答用例：
```json
{
  "id": "q01",
  "intent": "按区域看销售额",
  "query": {
    "metrics": [{ "ref": "sales_amount" }],
    "dimensions": [{ "ref": "sales_region" }],
    "sort": [{ "by": { "metric": "sales_amount" }, "direction": "desc" }]
  },
  "expect": {
    "rows": 3,
    "cols": 2,
    "shape": "分组汇总",
    "note": "应按大区分组并按销售额降序排列"
  }
}
```

### 2. 回归执行标准
- 每当发生以下操作时，**必须一键触发全量回归评测**：
  1. 修改了某个指标的计算公式；
  2. 调整了全局过滤规则；
  3. 新增了指标或同义词别名。
- **验收判据**：
  - 指标召回匹配率必须达到 100%；
  - 报表形态判定（明细/分组/交叉/趋势/排名/对比）与期望一致；
  - 核心数值经过独立 SQL 验算一致。

---

## 常见失误与排错对照表

| 现象 / 错误 | 根本原因 | 标准修正措施 |
|---|---|---|
| 多表关联后，销售金额汇总值翻倍 | 实体粒度（Grain）丢失，出现一对多笛卡尔积 | 检查 `relationships` 中的 cardinality，在 SQL 编译时采用半连接子查询或先聚合后关联 |
| AI 自动出的报表包含了测试门店和已退款数据 | 未在指标中绑定全局业务规则 | 在 `rules` 中建立 `valid_orders` 规则，并在指标 `filters` 中显式引用 |
| 用户提问“业绩”时系统报错“未识别指标” | 同义词图谱缺失长尾别名 | 在指标 `sales_amount` 的 `aliases` 中加入 `"业绩"` |
| 调整了一个指标别名，导致另一个报表查询变错 | 别名产生交叉冲突（多对一重叠） | 分离通用别名，将歧义项交由人机交互弹窗确认，不可静默替罪 |
