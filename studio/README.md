# Report Studio（MVP）

「**需求 → 可选的模板 → 结果**」的工作台。设计稿见 Ardot *Report Studio 设计稿 v0.1*
（fileId `739460766593958`），本目录是它的可运行实现。

引擎还是 `rpx.exe`：分析走 `rpx spec analyze`、出稿走 `rpx spec compile`、
跑表走 `rpx run --export …`、选模板走 `rpx report list`（实例库检索）。
Studio 不自己算数，只做**对象化 + 编排 + 取舍**。

## 跑起来

```bash
# 前置：引擎在 target/release（或设 RPX_BIN 指到别处）
cargo build --release -p rpx-cli

cd studio
python server.py            # 默认 127.0.0.1:8778
```

打开 <http://127.0.0.1:8778>。只依赖 Python 标准库，无需 pip。

## 三步走（页面就是这三步）

1. **明确需求与数据源** —— 标题 + 一句话需求；数据源三选一：示例（90 行门店销售）、
   上传 CSV、**连数据库**（PostgreSQL / Kingbase / MySQL / SQLite）。库形态在
   「数据源」页登记连接 → 需求里选数据源 + 写 SQL（可「列出库表」点表名自动填）
   → 预览前 20 行 → 用这条 SQL。列画像由 `dataset inspect` 给出（列名/角色推断/样例值）。
2. **自动给出可选的模板** —— 按「描述关键词 + 列名重合 + 形态匹配」从实例库
   （`examples/library`，59 份）里挑 6 个候选，每条都带理由与「缺哪些列」；
   外加一张保底的「按需求生成新模板」。
3. **可调整的选项** —— 生成路径可调：**报表形态**（明细/分组/交叉/趋势/排名/对比）、
   行维度与排序、度量与聚合、列维度、总计/小计；库模板路径可确认数据绑定。
   下方「对话补充」用规则把口语落回这些开关（不生成文案）。

接下来：**分析**（口径/画像/决策日志/未决问题，右侧栏）→ **编译出稿**（rev N）→
**运行**（csv/xlsx/html/print-xlsx 四件产物）→ **设计器**微调（内嵌 5288 的 Univer
画布，可改字号/字形/对齐/边框/行列尺寸，「保存」写回模板定义 rev + 1；「结果」
交给 Studio 重算，**不需要库内 id**）→ 再运行。

## 查询能力（rpx-query/v1 的两处扩展）

按 `business_report_agent.md` §4.1/§4.2 补齐：

| 能力 | 写法 | 编译结果 |
|---|---|---|
| **聚合后筛选** | `filters[].scope = aggregate` | `HAVING <聚合> op 值` |
| **分区筛选**（region_total 语义） | `scope = partition` + `partition_by` | 半连接子查询（先按维度汇总再筛） |
| **同比 / 环比 / 上一期** | `comparison.type` + `time_range` | `m0` / `m0_prev` / `m0_growth` 三列（CASE WHEN 分桶，WHERE 放宽两窗口） |

区别是**语义**不是性能：「全年总额超 10 万的地区」必须用 `partition`；
用 `aggregate` 会变成「某个月超 10 万」。详见 `docs/rpx-query-v1.md`。

## 报表形态（六种，`shapes.py`）

「怎么算对」在查询里（时间窗、粒度、`ORDER BY` 指标、`LIMIT`），「怎么摆对」在形态里。
引擎只有 `list / grouped / crosstab` 三档 layout，六形态是**适配层**（不改引擎契约）：

| 形态 | layout | 摆位 | 对查询的要求（缺了直接拒绝，不静默降级） |
|---|---|---|---|
| `detail` 明细 | `list` | 维度平铺、不带合计 | — |
| `grouped` 分组 | `grouped` | 维度依次分组 | 至少一个维度 |
| `crosstab` 交叉 | `crosstab` | 首个维度放行、次个放列 | 两个维度 |
| `trend` 趋势 | `grouped` | 时间维提前并升序，分类维放列 | 日期维度（`kind=date` 或 `time_grain`） |
| `rank` 排名 | `grouped` | 只留首个维度、关合计 | 查询里 `sort.by.metric`（名次由 SQL 的 ORDER BY + LIMIT 决定） |
| `compare` 对比 | `grouped` | 度量拆成 本期 / 上期 / 增幅 三列 | `comparison` 产出的 `m<i>_prev` 列 |

判定证据序：**查询 IR > 计划里的维度/排序 > 需求角色 > 版式与文案**。
显式版式不覆盖语义判定（只有一个维度时不会硬摆交叉表）。

两个入口：

- `POST /api/datasources/<id>/report` —— 自然语言查询 IR → **报表需求**（SQL 内联成字面量、
  取样落 `sample.csv`、`spec` 由 `shapes.from_plan` 生成），返回的需求可继续
  排版 / 换形态 / 编译 / 运行。追溯三件套（`query_ir` / `plan` / `compiled`）随需求落盘。
- `POST /api/requirements/<id>/shape {shape, force}` —— 换形态。已出稿（有 rev）默认
  409，要 `force=true`：重新出 spec 会盖掉手工排版。

## 多步骤归因分析（§8 L2）

`shapes` 管「怎么摆」，`analysis` 管「拆几层」：整体 → 按维度 1 拆 → 对最大贡献者按维度 2 再拆。
**每一步都是 IR → 计划 → 编译 → 执行**，执行器只接受 IR（没有自由 SQL 入口），
步数/行数/`top_k` 有硬顶（8 / 500 / 5）。

- 窗口取「上一个完整周期」（半月对半月是假下降）；对比期等长前移，日历对齐时按日历平移；
- 结论分 `observed`（看到的）与 `computed`（算出来的贡献），**没有 causal**；
- `caveats` 固定写「算术分解 ≠ 因果」、对比期口径、预算触顶；
- 页面：`#/analysis`（数据源 / 指标 / 时间维 / 粒度 / 基准日 → 拆解）。
- 契约 `docs/rpx-analysis-v1.md`；测试 `test_analysis.py`（真 SQL 跑内存 sqlite）。

## 保存与自动化（§8 L3）

| 能力 | 落点 | 关键取舍 |
|---|---|---|
| **语义版本** | `versions.py` → `data/versions/<req>/v<N>.json` | 指纹只认口径（改 label 不算）；版本只增不改 |
| **定时执行** | `schedules.py` → `data/schedules.json` + 30s tick | 错过不补跑；本地 naive 时间；单轮 5 个 |
| **异常提醒** | `alerts.py` → `data/alerts/YYYY-MM.jsonl` | 与归因同一口径；算不出增幅要说出来；按天去重 |

排期与手动运行共用 `run_requirement()`：跑报表 → 顺带体检 → 记审计。
外部 cron 可用 `python server.py --once` 或 `POST /api/schedules/tick`。
契约 `docs/rpx-automation.md`；测试 `test_automation.py`。

## 审计（§5.3）

每次 `understand` / `compile` / `execute` 落一条 `data/audit/YYYY-MM-DD.jsonl`
（append-only，一行一条完整记录：需求、query、plan、SQL、参数、状态、行数、耗时、
口径回执）。`GET /api/audit?limit=50&dsn=` 读最近记录。

**不记**口令与 API key（`params` 是查询参数，必须留 —— 否则复现不了）。

## 业务问题回归集（§10 P0 验收）

`questions.json` + `test_questions.py`：**24 条**真实业务说法 + 对应查询 + 可核对的
结果形状（`python test_questions.py` 离线跑，不需要 Studio / LLM）。

覆盖：基础聚合 / 多维交叉 / HAVING / partition / 明细筛（ne·in·like）/ 排序 TopN /
空结果 / 时间左闭右开 / 空时间窗 / 同比 / 环比 / 多指标 + 三类**应拒绝**
（未决歧义、未注册 ref、对比缺时间窗）。

期望值不硬编码具体金额 —— 由 SQL 现算，样本换了不会假红。

## MCP 工具面（§6.1）

`scripts/mcp/server.mjs` 从 5 个工具扩到 **10 个**，新增 5 个语义层工具
（全部转发 Studio HTTP，与 Web 共用同一套核心，不在 MCP 里重复实现）：

| 工具 | 作用 |
|---|---|
| `inspect_data_model` | 看业务模型（实体/关系/指标/维度/规则/未决） |
| `search_business_terms` | 按中文说法检索指标/维度（走 `aliases`） |
| `build_report_plan` | 自然语言 → 查询草案（过验算）→ 执行计划；有歧义返回 `needs_clarification` |
| `compile_report_query` | 查询 → 方言 SQL + 参数 + 口径回执（不执行） |
| `explain_report` | 解释「这个数怎么算的」：口径/警告/粒度风险/护栏 |

`RPX_STUDIO_BASE` 可改 Studio 地址（默认 `http://127.0.0.1:8778`）。

## 数据源（数据库连接）

`#/datasources` 页管理连接：

* **记录层** `data/datasources.json`：`{id, label, driver, host, port, database, user,
  password, options}`。口令只在这里，**不回传前端**（列表只给 `has_password`）。
* **注册表层** `data/runtime/datasources.json`：每次调引擎前由记录层生成，
  形如 `{"ds-x": {"url": "postgres://user:pass@host:5432/db"}}`（凭据按 URL 编码），
  以 `--datasources` 交给 `rpx run`。
* 三种驱动：`postgres`（含 Kingbase）/ `mysql` / `sqlite`（填 .db 文件路径，免服务端，
  本机 `sample/sales-demo.db` 就是拿来验链路的）。

**取数口径**：SQL 原样交给数据库（不做改写）。需求的 spec 里 `dataset.sql = {dsn, sql}`，
编译出的模板 `datasets[0].sql` 同样是它 —— 运行期现取，**不落数据文件**。
画像要读盘（`spec analyze` 只认文件），所以每份 SQL 需求会额外存一份
`data/req/<id>/sample.csv`（前 200 行取样），**只用于分析**。

## 理解层（LLM：自然语言 → 查询草案）

`POST /api/datasources/<id>/understand` `{"intent": "按区域统计销售额"}` →
`rpx-query/v1` 草案 + 事实层验算。接 **DeepSeek**（OpenAI 兼容，stdlib urllib 直连，
不引 SDK）：

* 配置：`POST /api/llm` `{"api_key": "sk-...", "model": "deepseek-v4-flash"}`，
  或环境变量 `DEEPSEEK_API_KEY` / `LLM_API_KEY`（优先级：env > `data/llm.json` > 默认）。
  key 只落文件，**不回传前端**（`GET /api/llm` 只给 `has_key`）。
* 模型：`deepseek-v4-flash`（默认，便宜快）/ `deepseek-v4-pro`（复杂推理）。
  请求级覆盖：`{"intent": ..., "model": "deepseek-v4-pro"}`。
  ⚠ 旧名 `deepseek-chat`/`deepseek-reasoner` 已于 2026-07-24 退役。
* `temperature=0`：查询生成要确定性，同样的需求不能今天一个口径明天另一个。

**三条铁律**（对应架构原则 01/03）：

1. **LLM 说的话不作数** —— 输出一律过 `query_ir.validate`：未注册的 ref、形状不对的
   filter 全部挡下，绝不静默改写。LLM 是起草人不是权威。
2. **不许猜** —— 拿不准的口径进 `ambiguities`（`resolved_as: null`）由人拍板；
   有未决歧义 ⇒ `needs_clarification`，下游拒绝编译。
3. **上下文是证据** —— prompt 里业务模型条目带 `status` 与库备注（COMMENT），
   让模型知道哪些是事实哪些是推断。

prompt 组装 = 业务模型（实体/关系/指标/维度/规则/未决问题）+ 金样例 few-shot
（`goldens.search` 召回 top-3）+ `rpx-query/v1` 输出契约。

## Pi + DeepSeek（终端 agent 接入）

**Pi**（开源终端编程 agent，`github.com/badlogic/pi-mono`）当协作者：配 DeepSeek 为
模型 + 装 `rpx-report` skill，它就能做多轮需求分析、起草查询、调管线取数、归档金样例。
配置见 [`../pi-skill/README.md`](../pi-skill/README.md)：

* `~/.pi/agent/models.json` 并入 `pi-skill/models.deepseek.json`（DeepSeek 为自定义 provider）
* `cp -r pi-skill/rpx-report ~/.pi/skills/`（技能：API 清单 + 工作流 + 铁律）

两条路互为备份：Studio 自己的理解层直连 DeepSeek（不依赖 Pi）；Pi 也可以不靠
`/understand` 直接起草查询 JSON 走 `/plan`→`/execute`。

## 目录

```
studio/
  server.py        服务端（stdlib，~700 行）：对象存储 + rpx 调用 + 23 个 API
  web/index.html   单页壳
  web/app.js       前端（hash 路由 + 四页视图，无构建）
  web/styles.css   设计稿同款样式（深青灰 #2C4A5A / 浅灰底）
  sample/          示例数据（90 行门店销售，确定性生成）
  data/            工作区（gitignore）：需求/模板/运行/产物都在这里
```

## 数据模型

```
需求 requirement ── spec（rpx-spec/v1，生成路径）
  ├─ analysis（rpx spec analyze 的输出，带口径/决策/未决问题）
  ├─ template_choice（library：复用实例库模板 / generated：按需求编译）
  ├─ template（rev-N.json，可逐格补丁）
  └─ runs[]（run-…：状态/耗时/形状/产物/输入 sha）
```

状态机：`draft → analyzed（口径待确认）→ compiled（已出稿）→ ran（待验收）→ accepted（已验收）`。

## API（节选）

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/bootstrap` | 工作台：KPI + 需求列表 + 最近运行 |
| POST | `/api/requirements` | 新建需求（示例数据或上传 token） |
| POST | `/api/upload?name=` | 上传 CSV/TSV，返回 token + 列画像 |
| POST | `/api/requirements/<id>/dataset` | 换数据源（示例或上传 token） |
| POST | `/api/requirements/<id>/recommend` | **自动给出可选模板**（带理由） |
| POST | `/api/requirements/<id>/choose` | 选定模板（library / generated） |
| GET | `/api/requirements/<id>/shape` | 形态决策（判定/理由/阻断/六形态目录） |
| POST | `/api/requirements/<id>/shape` | 换形态（已出稿需 `force`） |
| POST | `/api/datasources/<id>/execute` | 一条龙：校验→计划→编译→跑（附带 `report_spec`/`report_shape`） |
| POST | `/api/datasources/<id>/report` | **自然语言 → 报表需求**（查询只算数，报表能继续加工） |
| GET | `/api/shapes` | 六形态目录（UI 按钮与 MCP 说明共用） |
| POST | `/api/datasources/<id>/analysis` | 多步骤归因（IR→计划→编译→执行，带预算） |
| GET | `/api/analyses` `/api/analyses/<id>` | 分析列表 / 完整文档（含每步 SQL） |
| POST | `/api/requirements/<id>/publish` | 保存语义版本（spec + 口径指纹 + 来源） |
| GET | `/api/requirements/<id>/versions[?a=&b=]` | 版本列表 / 差异（`semantic_changed`） |
| GET/POST | `/api/schedules` | 排期列表 / 新建或修改 |
| POST | `/api/schedules/<id>/{run,toggle,delete}` | 立即执行 / 暂停 / 删除 |
| POST | `/api/schedules/tick` | 跑所有到期的（给外部 cron） |
| POST | `/api/requirements/<id>/alerts` | 体检一次（口径与归因同源） |
| GET | `/api/alerts?limit=&level=` | 最近提醒 |
| POST | `/api/requirements/<id>/analyze` | 口径/画像/决策/未决问题 |
| POST | `/api/requirements/<id>/compile` | 出稿（库模板=复制并改数据绑定） |
| POST | `/api/requirements/<id>/run` | 运行并留档（四格式产物） |
| POST | `/api/requirements/<id>/chat` | 对话补充（规则式改开关） |
| GET/POST | `/api/requirements/<id>/template` | 读/逐格改模板（设计器用） |
| GET | `/api/requirements/<id>/artifact?run=&name=` | 产物下载 / `inline=1` 预览 |

## 已知边界（写清楚，不装）

- **对话是规则不是模型**：只认「合计/交叉表/明细/升序降序/列名」这些说法，
  改的是结构化开关；开放式需求要落到 spec 才能编译。
- **库模板只换数据不改结构**：列名对不上会在运行时给出诊断；要改维度请走
  「按需求生成」。
- **设计器只改样式**（字号/字形/对齐/边框），合并与表达式列不在这里改 ——
  它们由编译期决定，改了会破坏报表语义。
- **单人工作区**：没有账号/并发队列；`rpx run` 的 out-dir 是 CREATE_NEW，
  Studio 每次运行都开新目录。
- 模板 revision 只做「每次编译 +1」；**版本不可回滚**（只增不改，回滚要手工拷回旧 spec）。
- **提醒没有通知渠道**：只落库 + 列表展示，邮件/IM 未接。
- **分析只找下降**、最多两层维度、不解释因果（只给算术证据）。
- **排期四种说法**（每小时/每天/每周/每月），不支持 cron 与「最后一个工作日」这类日历规则。
