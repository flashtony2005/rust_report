# 保存与自动化契约 — 版本 / 定时 / 提醒

> 对应 `business_report_agent.md` §8 L3（保存报表及其语义版本、定时生成、指标变化提醒）。
> 实现：`studio/versions.py`、`studio/schedules.py`、`studio/alerts.py`（都无 IO 依赖，可单测）。

三件事共用一条原则：**口径只有一份**。版本记的是它，定时跑的是它，提醒比的也是它。

---

## 1. 语义版本 — `rpx-report-version/v1`

只存模板等于只存了壳子：同一份模板换了指标口径就出不同的数。所以一次「保存」写三样：

| 段 | 内容 | 为什么 |
|---|---|---|
| `spec` | `rpx-spec/v1` + `template.rev` | 怎么摆 |
| `semantic` | `{version, hash, metrics[], rules[]}` | **当时的口径** |
| `provenance` | `query_ir` / `plan` | 怎么来的（可复现） |

### 指纹只认口径

```json
"semantic": {"version": 3, "hash": "sha256:ba26f6782fa4be80",
             "metrics": [{"id": "...", "label": "销售额", "expression": "SUM(amount)-SUM(discount)"}],
             "rules": []}
```

`hash` 只覆盖**会改变数字**的字段（指标表达式、维度列/kind/time_grain、关系、规则），
并按 id 排序。改 `label` / `aliases` 指纹不变 —— 改显示名不算「口径变了」。
（实测踩过：整个 metric 对象扔进哈希，改个中文名就让所有历史版本「不可比」。）

### 差异（`diff`）

口径优先，摆放其次。只比人能核对的：指纹、业务模型版本、指标表达式、模板 rev、形态、版式、度量列。
不做逐格 diff —— 没人读，而且模板 rev 已经够定位。

| 接口 | 说明 |
|---|---|
| `POST /api/requirements/<id>/publish {note}` | 保存一版（版本号 = 已有 + 1，不覆盖旧的） |
| `GET /api/requirements/<id>/versions` | 摘要列表（新→旧） |
| `GET /api/requirements/<id>/versions/<n>` | 完整文档 |
| `GET /api/requirements/<id>/versions?a=1&b=2` | 差异（`semantic_changed` 是关键位） |

落盘：`data/versions/<req>/v<N>.json`。审计：`action="publish"`。

---

## 2. 定时执行 — `rpx-schedule/v1`

不支持 cron 表达式，只支持四种说法（多了没人配得对）：

```json
{"kind": "daily",  "at": "08:30"}
{"kind": "weekly", "weekday": 0, "at": "09:00"}      // 0=周一
{"kind": "monthly","day": 31, "at": "07:00"}         // 超月天数夹到月末，并留 note
{"kind": "hourly", "interval_hours": 6}              // 硬顶 168（7 天）
```

三条取舍：

1. **错过不补跑**：关机三天再开机，只跑**一次**然后重算下次 —— 补跑出来的是一堆过期快照。
2. **本机本地时区（naive）**：单人工作区，跨时区排期是伪需求；存 `2026-10-28T08:30:00` 人看得懂。
3. **单轮上限 5 个**：一次 tick 不把服务拖垮。

外壳两种，逻辑同一套（`schedules.run_due`）：

- 进程内线程：启动 `server.py` 后每 30s 检查一次（`STUDIO_SCHEDULER=0` 关）；
- 外部驱动：`python studio/server.py --once` 或 `POST /api/schedules/tick`。

| 接口 | 说明 |
|---|---|
| `GET /api/schedules` | 全部排期（含 `next_run` / `last_status` / `runs`） |
| `POST /api/schedules` | 新建或改（带 `id` 即改） |
| `POST /api/schedules/<id>/run` | 立刻跑一次（并推 `next_run`） |
| `POST /api/schedules/<id>/toggle` | 暂停 / 启用 |
| `POST /api/schedules/<id>/delete` | 删除 |
| `POST /api/schedules/tick` | 跑所有到期的（给外部 cron） |

排期触发的运行走 `run_requirement(req, trigger="schedule")`：**跑报表 → 顺带体检 → 记审计**，
与手动运行同一个入口，不写第二条执行路径。

---

## 3. 指标异常提醒 — `rpx-alert/v1`

```json
{"level": "warn", "metric": "sales_amount", "metric_label": "销售额",
 "current": 1950.0, "previous": 2400.0, "pct": -0.1875,
 "rule": {"pct": 0.1, "direction": "down", "level": "warn", "label": "较上期下降 ≥10%"},
 "message": "销售额 较上期 -18.8%（较上期下降 ≥10%）"}
```

- **口径与归因分解第一步完全一致**（`analysis.observe`）：告警说降 30%、分析说降 12% 是灾难。
- **需求自己声明了时间范围就用它**（`time_range`），不换成「相对今天的上一个整月」——
  换窗口等于换了一个问题（实测踩过：报表的 window 是 2 月，体检却去看 9 月，永远 0 命中）。
- **算不出来要说出来**：对比期为 0/空 ⇒ `pct` 是 NULL，不当作「没异常」，
  写一条 note「这段时间的监控是瞎的」。
- **一个指标一条**：命中多条规则时取最高级别（`info < warn < critical`）。
- **append-only + 按天去重**：`data/alerts/YYYY-MM.jsonl`，同一天同库同指标同级别只留一条
  （否则排期每天跑一遍，列表会被同一个异常刷满）。

| 接口 | 说明 |
|---|---|
| `POST /api/requirements/<id>/alerts {rules?, as_of?, persist?}` | 体检一次（`rules` 会存进需求的 `alert_rule`） |
| `POST /api/datasources/<id>/alerts` | 临时体检（不落需求） |
| `GET /api/alerts?limit=&level=` | 最近提醒（当月 + 上月） |

`run_requirement` 返回 `alerts`（新入库条数）与 `alerts_found`（本次判定命中条数）：
去重导致的 0 不等于漏报，两个数都要有。

---

## 4. 测试

`studio/test_automation.py`（离线，tempdir）：

- 指纹：改 label/别名不变、改表达式/规则变；版本不覆盖（v1/v2 并存）；列表新→旧；
- 差异：只改版式 ⇒ `semantic_changed=false` 但列出模板 rev / 版式；
- 排期：每日/每周/每月/每小时的下一次时间；月末夹取 + note；错过三天只跑一次；
  失败落 `failed` 且不影响其它排期；单轮上限 5；
- 提醒：25%/50% 分级、默认只盯下降、涨不提醒、对比期为 0 不出假提醒、
  同一天去重、观测失败要说明。

端到端：`tests/fixtures/out/automation_e2e.py`（自带两月 sqlite 夹具）——
注册数据源 → 发现模型 → 归因 → 落需求 → 编译 → 保存两版 → 差异 → 体检 → 排期执行。

---

## 5. 边界（写清楚，不装）

- **没有并发队列**：排期串行跑，一次 tick 最多 5 个；多人同时点会互相等。
- **没有通知渠道**：提醒只落库 + 列表展示，邮件/IM 推送未接（文档 §8 L3 的「通知」留待下一步）。
- **版本不可回滚**：只增不改，回滚要手工把旧 spec 复制回去（下一阶段做）。
- **提醒只看单指标**：不支持复合条件（如「销售额降且订单数升」）。
- **排期只有四种说法**：不支持 cron，也不支持「每月最后一个工作日」这类日历规则。
