# 报表形态契约（六种）— `rpx-report-shape/v1`

> 定位：`business_report_agent.md` §9 / §12 的 **Report Spec 适配层**。
> 一句话：**查询负责算对，形态负责摆对。**
> 实现：`studio/shapes.py`（纯函数，无 IO）；唯一出 spec 的入口是 `spec_for()`。

---

## 1. 为什么要这一层

同一份数据可以摆成完全不同的表，而「摆错」不会报错 —— 它会安静地出一张
看着正常、读起来是错的表（排名没有名次、趋势按乱序、对比缺上期）。
所以形态必须**显式判定**，缺条件就**明确拒绝**，而不是尽量凑一张。

引擎已经有的 layout 只有三档（`list` / `grouped` / `crosstab`，
见 `crates/rpx-runtime/src/compile/parse.rs`）。六形态**不是**新规格版本：
它把 trend / rank / compare 映射回三档 layout，靠**维度摆位 + 度量拆列 + 查询侧
ORDER BY/LIMIT** 表达。这样不动机芯，也不会出现两套平行标准。

---

## 2. 六种形态

| `shape` | 中文 | layout | 摆位规则 | 对查询的要求 |
|---|---|---|---|---|
| `detail` | 明细清单 | `list` | 维度平铺；关闭合计 | — |
| `grouped` | 分组汇总 | `grouped` | 维度依次分组；带总计行 | 至少 1 个维度、1 个指标 |
| `crosstab` | 交叉表 | `crosstab` | 首个维度放行、第二个放列；开行末小计 | 2 个维度 |
| `trend` | 趋势表 | `grouped` | 时间维提前并 `sort=asc`；其余维度放列（多序列） | 日期维度：`kind=date` 或带 `time_grain` |
| `rank` | 排名表 | `grouped` | 只留第一个维度；关合计 | 查询 `sort.by.metric`（名次 = SQL 的 `ORDER BY` + 可选 `LIMIT`） |
| `compare` | 对比表 | `grouped` | 每个度量拆成「本期 / 上期 / 增幅」三列，增幅 `0.0%` | `comparison` 产出的 `m<i>_prev` 列 |

**为什么 rank 不在 spec 里排序**：spec 的 `sort` 只作用于维度字段（`ds.group(d;d:1)`），
表达不了「按聚合值排序」。名次属于**计算**，所以放在 `sqlgen` 的 `ORDER BY m<i>`
（`limit` 同源）。同理 **compare 的增幅**在 SQL 里用 `CASE WHEN` 分桶 + `NULLIF` 算，
spec 只负责把三列并排显示。

---

## 3. 判定（`detect`）

证据优先级：**查询 IR > 计划里的维度/排序 > 需求角色 > 版式与文案**。

1. `comparison` 存在 ⇒ `compare`（结果列形状已被它决定）；
2. `sort` 里出现 `by.metric` ⇒ `rank`（有 `limit` = Top-N，没有 = 全量名次）；
3. 唯一维度是日期 ⇒ `trend`；日期 + 一个分类维 ⇒ `trend`（多序列，confidence=medium）；
4. ≥2 个非日期维度且明确说了「交叉/透视/矩阵」⇒ `crosstab`；
5. 一个维度 ⇒ `grouped`；没有维度 ⇒ `detail`（单行汇总也走这里）。

**显式版式不覆盖语义判定**：`layout=crosstab` 但只有一个维度时，判定仍是 `grouped`，
交叉表只出现在 `alternatives` 里。备选**不是**自动降级 —— 要用户点头。

返回结构：

```json
{
  "shape": "trend", "label": "趋势表", "layout": "grouped",
  "confidence": "high",
  "reasons": ["唯一维度是日期（月） ⇒ 按时间顺序展开"],
  "blockers": [], "alternatives": [{"shape": "crosstab", "why": "..."}],
  "ok": true
}
```

`blockers` 非空 = **摆不出来**：调用方必须拒绝（HTTP 400 / 需求页红字），
不许「先出一张再说」。

---

## 4. 从查询计划出规格（`from_plan`）

`rpx-plan/v1` → `rpx-spec/v1`。关键一条：**列名用编译器给的别名**。

| 计划 | 结果集列 | spec 引用 |
|---|---|---|
| `dimensions[i]` | `d0`、`d1`… | `row_dims[].field = d<i>` |
| `facts[i]` | `m0`、`m1`… | `measures[].field = m<i>` |
| `facts[i]` + `comparison` | `m0_prev`、`m0_growth` | compare 形态展开成三列 |

引用业务 `ref`（如 `sales_amount`）会让引擎报未知列 —— 别名是两边唯一的共同语言。

`agg` 一律 `sum`：SQL 已经聚合过，同维度分组后每组只剩一行，再聚合是**幂等**的。

---

## 5. 接口

| 方法 | 路径 | 用途 |
|---|---|---|
| GET | `/api/shapes` | 六形态目录（`label/layout/needs/template_shapes`） |
| GET | `/api/requirements/<id>/shape` | 当前形态决策（含理由与阻断） |
| POST | `/api/requirements/<id>/shape` | 换形态；`{shape, force}`，已出稿需 `force=true` |
| POST | `/api/datasources/<id>/report` | 查询 IR → 报表需求（含 `spec` 与 `shape_decision`） |
| POST | `/api/datasources/<id>/execute` | 执行；响应附 `report_spec` / `report_shape` |

`/report` 落盘的需求带追溯三件套：`query_ir`、`plan`、`compiled`
（`sql`/`params`/`dialect`/`guardrails`）—— 同一条记录即可复现。

---

## 6. 测试

`studio/test_shapes.py`（60+ 断言，纯离线）：

- 判定：六形态各一例 + comparison 优先 + 维度排序不算排名 + 显式版式不覆盖；
- 阻断：交叉缺维、趋势缺日期、对比缺 `m<i>_prev`；
- 摆位：layout / row_dims / col_dims / sort / grand / 增幅格式；
- 别名：`d0/m0`、`m0_prev/m0_growth`、时间桶标签「月份（月）」。

端到端：`tests/fixtures/out/report_flow_e2e.py`（需要 Studio 在 8778）——
IR → execute → report → analyze → compile → run → 换形态 → 重编译 → 再运行。

---

## 7. 边界（写清楚，不装）

- **趋势只接一个分类维**：多出来的维度不摆（`notes` 里会说），因为三序列以上
  在一张表里读不了，应该拆成多张。
- **排名不做并列名次**：名次是行序，不是计算列；并列需要窗口函数 `RANK()`，
  当前查询 IR 不支持。
- **对比表依赖 `comparison`**：没有对比列的普通数据摆不出对比表（不会硬造）。
- **形态不改数据**：换形态只重出 spec，不重跑查询；查询侧要改（排序/Top-N/对比期）
  得回查询 IR。
