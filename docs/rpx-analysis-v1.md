# 多步骤归因分析契约 — `rpx-analysis/v1`

> 对应 `business_report_agent.md` §8 L2。实现：`studio/analysis.py`（纯编排，无 IO）。
> 一句话：**把「为什么降了」拆成几步受控查询，每一步都留 SQL，结论分「看到的」与「算出来的」。**

---

## 1. 为什么不是「让模型多问几次」

L2 最容易做歪的地方是把 Agent 放到循环里自由发挥。这里的约束是硬的：

| 约束 | 落地 |
|---|---|
| 每步都是正规查询 | 只产 `rpx-query/v1` IR，执行由注入的 `run_query` 完成（校验→计划→编译→执行） |
| 没有自由 SQL | 执行器只接受 IR，**没有 SQL 字符串入口** |
| 有预算 | `max_steps ≤ 8`、`max_rows ≤ 500`、`top_k ≤ 5`（硬顶，调用方只能调小） |
| 可复现 | 每步存 `query / plan / sql / params / columns / rows / row_count` |
| 不装懂 | `status = ok / partial / failed`，`summary` 直说为什么拆不动 |

---

## 2. 拆解逻辑

```
step-1  baseline   整体：本期 vs 上期（无维度）        → Δ、增幅
   │  Δ ≥ 0 ⇒ 停（说「没有下降，不用拆」）
   ▼
step-2  split      按维度 1 分组（同窗口 + comparison） → 每个成员的 Δ
   │  取 Δ<0 的前 top_k 个（降得最多的在前）
   ▼
step-3+ drill      对每个被点名的成员，按维度 2 再拆一层（filter: dim1 in [...]）
```

- **排序**：`(Δ 升序, 成员名)` 双键 —— 不依赖数据库返回顺序，同输入同输出。
- **占比**：分母是**上一层**的变化量（整体降幅或该成员降幅），不是「所有下降成员之和」。
  有成员增长时占比之和会 >100%，此时 `coverage > 1` 并附一条 caveat 说明。
- **深层**：最多两层维度（再深一层用户也读不动）。

---

## 3. 窗口口径（`timewin.py`，与编译期共用）

| 名词 | 含义 |
|---|---|
| `window="last_complete"`（缺省） | 上一个**走完**的周期（拿半个月对半个月出的「下降」是假的） |
| `window="current"` | 用当期（未走完） |
| 对比期 | 与本期**等长**的前一个窗口；日历对齐时按日历平移（9 月 → 8 月），对不齐才减天数 |
| `as_of` | 以指定日期为基准（复盘 / 测试）。缺省 = 今天 |

`period_bounds(today, grain, window)` 的粒度：`day / week / month / quarter / year`。

---

## 4. 文档结构

```json
{
  "schema": "rpx-analysis/v1",
  "id": "an-261027014052-aa1e",
  "dsn": "decline-demo", "metric": "sales_amount", "metric_label": "销售额",
  "grain": "month", "dimensions": ["sales_region", "sales_category"],
  "windows": {"cur_start": "2026-02-01", "cur_end": "2026-03-01",
              "prev_start": "2026-01-01", "prev_end": "2026-02-01"},
  "totals": {"cur": 1950.0, "prev": 2400.0, "delta": -450.0, "pct": -0.1875},
  "steps": [{"id": "step-1", "kind": "baseline", "query": {}, "plan": {}, "sql": "SELECT …",
             "params": [], "columns": ["m0", "m0_prev", "m0_growth"], "rows": [[…]],
             "row_count": 1, "ok": true}],
  "findings": [
    {"kind": "observed", "what": "销售额：本期 1,950，上期 2,400，减少 450（-18.8%）", "evidence": ["step-1"]},
    {"kind": "computed", "what": "区域「A」贡献 -600（占整体降幅 133.3%）", "share": 1.333, "evidence": ["step-2"]},
    {"kind": "computed", "what": "区域「A」内部，品类「X」贡献 -500（占该成员降幅 83.3%）",
     "member": "A", "sub_member": "X", "evidence": ["step-3"]}
  ],
  "coverage": 1.333,
  "caveats": ["贡献是**算术分解**（Δ = 本期 − 上期），不是因果结论：…"],
  "summary": "整体 销售额 -450（-18.8%）。降幅集中在 区域「A」：合计占 133.3%。…",
  "status": "ok"
}
```

`kind` 只有两种，对应文档 §11.4 的要求：
- `observed` —— 看到的变化（本期 / 上期 / 差值）；
- `computed` —— 算出来的贡献（Δ 分解）。
**没有 `causal`**：相关不是因果，不给就是不给。

---

## 5. 接口

| 方法 | 路径 | 说明 |
|---|---|---|
| POST | `/api/datasources/<id>/analysis` | 跑一次分解。body：`{metric?, time_field?, grain?, dimensions?, window?, as_of?, budget?}` |
| GET | `/api/analyses?limit=` | 最近的分析（摘要） |
| GET | `/api/analyses/<id>` | 完整文档（含每步 SQL） |

`metric / time_field / dimensions` 缺省时从业务模型挑（第一个非 draft 指标、第一个日期维、
前两个文本维）；模型的日期维标 `day` 时**默认按月**并在 caveats 里说明（可显式传 `grain` 覆盖）。

审计：一次分析落一条 `action="analysis"`（含 `steps` 摘要：n/kind/rows/status/sql 前 400 字）。

---

## 6. 测试

- `studio/test_analysis.py`：**用真 SQL 跑内存 sqlite** —— 注入的 runner 就是
  planner + sqlgen + sqlite3，所以「IR→计划→编译→执行→分解」每一环都是真的。
  断言到具体数字（Δ=-450、区域 A=-600、品类 X=-500、占比分母是整体而非成员之和）。
- 另钉：窗口算术（1/31→2/28、季度/年度对齐）、确定性（跑两次同一结果）、
  预算硬顶、无维度/失败/空窗口的降级文案。

---

## 7. 边界（写清楚，不装）

- **两层维度封顶**：第三层就该换一张报表，不是继续钻。
- **只找下降**：`Δ ≥ 0` 直接停并说明；要拆增长得改 `direction`（未做）。
- **不解释原因**：只给「哪里一起降了」的算术证据，因果要靠业务判断。
- **不做异常检测**：那是 `alerts.py` 的活（同一口径，见 `docs/rpx-automation.md`）。
- **不自动建报表**：分析是分析，落成需求要显式调 `/report`（P1 的桥）。
