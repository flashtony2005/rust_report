---
name: rpx-report
description: Report Studio 报表引擎操作技能。当用户要分析报表需求、查业务模型/指标口径、把自然语言转成查询、跑数据查询、生成 SQL、执行报表查询、把确认过的查询存为金样例、管理数据源（含数据库连接）时使用。触发词：报表、需求分析、指标、口径、维度、业务模型、bizmodel、understand、金样例、golden、跑数、查询销售额、数据源、rpx、Report Studio、执行查询。
---

# Report Studio（报表引擎）操作

本机报表引擎 Studio 在 **http://127.0.0.1:8778**（纯 HTTP JSON API，用 `curl` 调）。
你的角色是**需求分析与查询起草的协作者**：把用户说的业务问题翻成结构化查询、
调 Studio 的确定性管线取数、把用户确认过的查询归档为金样例。

## 管线地图（每一层职责固定，不要跳层）

```
自然语言需求
  → /understand   理解层：LLM 草案 rpx-query/v1（必须过验算）
  → /plan         L4 规划：JOIN 路径 + 粒度重复计数检查
  → /sql          L5 编译：方言 SQL + 只读护栏 + 口径回执
  → /execute      一条龙：校验→计划→编译→跑→错误分类/形状核对
  → /golden       金样例归档（人确认过的才入）
```

## 铁律（违反 = 帮倒忙）

1. **歧义必须人拍板**。`/understand` 返回的 `ambiguities[].resolved_as: null` 表示
   口径未定 —— 把它念给用户问清楚，**不许替用户猜**（含税吗？同比还是环比？）。
   未决歧义时下游会拒绝编译，这是设计如此，不是 bug。
2. **口径以业务模型为准**（`status=confirmed` 的是人工确认过的事实；
   `candidate`/`draft` 是机器推断）。用户和模型说的口径若与 confirmed 冲突，
   指出来，不要直接采用。
3. **只建议不代改**。SQL 报错里给的列名建议、口径修正建议，都念给用户决定。
4. 取数失败先看 `error.code`（UNKNOWN_COLUMN / SQL_SYNTAX / TIMEOUT / CONNECTION…），
   带建议的把 `suggest_columns` 一起念出来。

## 常用 API（base = http://127.0.0.1:8778）

| 用途 | 调用 |
|---|---|
| 健康检查 | `curl -s --noproxy '*' $base/api/health` |
| 数据源列表 | `curl -s --noproxy '*' $base/api/datasources` |
| 测试连接 | `curl -s --noproxy '*' -X POST $base/api/datasources/<id>/test` |
| 列库表 | `curl -s --noproxy '*' $base/api/datasources/<id>/tables` |
| **生成/重建业务模型** | `curl -s --noproxy '*' -X POST $base/api/datasources/<id>/bizmodel` |
| 读业务模型 | `curl -s --noproxy '*' $base/api/datasources/<id>/bizmodel` |
| **需求 → 查询草案** | `curl -s --noproxy '*' -X POST $base/api/datasources/<id>/understand -H 'Content-Type: application/json' -d '{"intent":"按区域统计销售额"}'` |
| 查询 → 计划 | `... -X POST .../plan -d '{"query":{...}}'` |
| 查询 → SQL | `... -X POST .../sql -d '{"query":{...}}'` |
| **查询 → 执行取数** | `... -X POST .../execute -d '{"query":{...}}'` |
| 金样例检索 | `curl -s --noproxy '*' --get "$base/api/datasources/<id>/golden" --data-urlencode "q=各区域销售额"` |
| 金样例归档 | `... -X POST .../golden -d '{"intent":"…","query":{…},"sql":"…","result_shape":{…},"note":"口径说明"}'` |

⚠ 所有 curl 都带 `--noproxy '*'`（本机环回地址不要走代理）。

## 标准工作流（用户报需求 → 出数）

```bash
base=http://127.0.0.1:8778; ds=<数据源id>

# 1) 确认有业务模型（没有就生成）
curl -s --noproxy '*' -X POST $base/api/datasources/$ds/bizmodel

# 2) 自然语言 → 查询草案（理解层）
curl -s --noproxy '*' -X POST $base/api/datasources/$ds/understand \
  -H 'Content-Type: application/json' -d '{"intent":"用户的原话"}'

# 3) 有 ambiguities 就问用户；没有就直接执行
curl -s --noproxy '*' -X POST $base/api/datasources/$ds/execute \
  -H 'Content-Type: application/json' -d '{"query":<第2步返回的 query>}'
```

`/execute` 返回里值得念给用户的字段：
- `result.columns` / `result.preview` —— 取数结果
- `sql.sql` + `sql.params` —— 实际执行的 SQL（可审计）
- `sql.definitions` —— **口径回执**：每个数字怎么算出来的（用户问「这个数怎么来的」就念它）
- `warnings` —— 空结果 / 形状不符 / 粒度风险（执行成功≠语义正确，有警告要念）

## 金样例（few-shot 记忆，核心资产）

用户确认某个查询结果正确时，归档进金样例库（`rpx/goldens/<dsn>/`）：

```bash
curl -s --noproxy '*' -X POST $base/api/datasources/$ds/golden \
  -H 'Content-Type: application/json' -d '{
    "intent": "用户原话",
    "query": <确认过的 rpx-query/v1>,
    "sql": "<执行过的 SQL>",
    "result_shape": {"columns": [...], "rows": 4},
    "note": "口径说明：销售额=sum(amount)-sum(discount)，按降序",
    "dialect": "sqlite"
  }'
```

只归档**人确认过**的。以后 `/understand` 会自动召回相似金样例当 few-shot ——
你归档的每一条都会让下次起草更准。

## 查询契约速查（rpx-query/v1，/understand 的输出形状）

```json
{
  "schema": "rpx-query/v1",
  "intent": "按区域统计销售额",
  "metrics": [{"ref": "<指标id>"}],
  "dimensions": [{"ref": "<维度id>"}],
  "filters": [{"ref": "<维度id>", "op": "eq|ne|gt|gte|lt|lte|in|like", "value": "..."}],
  "time_range": {"field": "<时间维度id>", "preset": "last_30_months|this_month|..."},
  "sort": [{"by": {"metric": "<指标id>"}, "direction": "desc"}],
  "limit": 1000,
  "ambiguities": [{"text": "拿不准的点", "candidates": ["A","B"], "resolved_as": null}]
}
```

ref **只能**用业务模型里注册的 id（`GET .../bizmodel` 的 `metrics[].id` / `dimensions[].id`）。
未注册的 ref 会被校验器拒绝 —— 那是口径未定义，先问用户或先在业务模型里确认口径。

## 理解层的 LLM 配置（Studio 自己也会调）

Studio 的 `/understand` 后端接的是 OpenAI 兼容端点（默认 DeepSeek）。
配置在 `POST /api/llm`（`{"api_key":"sk-...","model":"deepseek-v4-flash"}`），
或环境变量 `DEEPSEEK_API_KEY` / `LLM_API_KEY`。没配 key 时 `/understand` 会明确报
`NO_API_KEY` —— 你仍然可以走 `/plan`、`/sql`、`/execute`（查询 JSON 可以由你起草）。
