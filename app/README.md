# `app/` — React 产品壳（**只连 Rust 服务端**）

来源：`origin/master` @ `efbe488`（作者 `Lucas <letlucas50@gmail.com>`，2026-10-28），
按 `docs/merge-master-analysis.md` 方案 C（squash 落盘）收编，未合并其孤儿历史。

**2026-10-29 起这一层已经不含任何 mock**：原 `reportEngine.ts`（928 行假引擎）、
`seedData.ts`（636 行假数据）、`ontologyResolver.ts`（466 行假向量）、`UniverViewer.tsx`
（909 行手写 `<table>` 仿 Univer）全部删除 —— 9,239 行 → **约 1,300 行**，只留真功能。

## 现在能做什么（全部走真引擎）

| 页面 | 状态 | 后端 |
|---|---|---|
| `#/` 报表库 | ✅ 真 | `GET /reports`（59 份，服务端 CJK 检索 + 标签过滤） |
| `#/report/<id>` 报表详情 | ✅ 真 | `GET /reports/{id}` + `GET …/runs` |
| `#/ontology` 本体大脑 · 解析器 | 🎨 界面真 / 数据示例 | 四步（识别 → 定主表 → 分头聚合 → 组装输出）；本体来自 `src/data/ontology.ts` 的固定夹具，**Rust 端点未就绪**，页面顶部挂「示例数据 · 待后端」 |
| 运行 | ✅ 真 | `POST /reports/{id}/runs`（可勾 `csv/xlsx/print-xlsx/html/pdf`） |
| 产物 | ✅ 真 | `GET …/runs/{run}/artifacts/{name}`（直链，服务端读盘） |
| `#/datasources` 数据源 | ✅ 真 | `GET/POST /datasources`、`POST …/probe`、`GET …/tables` —— 真连库探测（列画像/抽样/主键），口令只写不读 |
| `#/bizmodel` 业务知识 | ✅ 真 | 六个页签全部接后端：本体与关系（实体/粒度/关系图）、指标口径库（新建/编辑/核准，表达式先过编译器白名单）、维度与字典（类型/值域/别名）、业务规则（新建/删除，**只有已核准规则进 SQL**）、同义词映射（加/删，不动口径）、问答回归（金样例，真跑 SQL 评通过/不通过） |
| `#/analysis` | ⏳ 待后端 | 端点还没实现，页面只列缺口（不放假数字） |
| 设计器 | ⏳ 未接 | 计划复用 `web/` 的真 Univer 1.2.0，不用手写表格 |

已实测（1440×1000，headless Chromium）：库内 59 份 / 结果最新 57 / 过期 0 / 未运行 2；
点「运行」后 21 → 22 次、新 run 置顶、勾 `csv` 后产物多出 `report.csv`（HTTP 200，表头正确）；
无 console 错误。

## 跑起来

```bash
# 1) Rust 服务端（必须先起，否则侧栏灯是黄的、页面只有错误原因）
#    数据源注册表默认落在 <library>/.datasources.json（--datasources FILE 可覆盖）
./target/release/rpx-server.exe serve --library examples/library --addr 127.0.0.1:8090

# 2) 前端（5289；5288 是另一套 Univer 设计器 web/，8778 是已冻结的 Python studio）
cd app && npm install && npm run dev
```

类型检查 / 构建：

```bash
cd app
node node_modules/typescript/bin/tsc -p tsconfig.json --noEmit
node node_modules/vite/bin/vite.js build      # 产物 dist/（不入库）
```

## 与 Rust 的唯一出口：`src/services/api.ts`

只封装 `crates/rpx-server/src/http.rs:31-40` 的 10 条路由。路径一律 `/api/...`，
由 `vite.config.ts` 的 proxy `rewrite` 剥掉前缀转发到 8090 —— **漏了 rewrite 就全 404**
（响应体是 `{"error":{"code":"NOT_FOUND"}}`，很容易被当成"没数据"）。

| 函数 | 路由 | 备注 |
|---|---|---|
| `health()` / `authStatus()` | `GET /health`、`GET /auth/status` | 侧栏那颗灯 |
| `listReports()` | `GET /reports?query=&tag=&limit=&offset=` | `limit` 上限 500 |
| `getReport(id)` | `GET /reports/{id}` | 带 `revision` 与完整 `template` |
| `saveTemplate(id, rev, tpl)` | `PUT /reports/{id}/template` | **必须**带 `If-Match`，否则 428 |
| `listRuns()` / `getRun()` | `GET /reports/{id}/runs[/{run}]` | 分页体 `{items,returned,total,next_offset}` |
| `runReport(id, opts)` | `POST /reports/{id}/runs` | 只认 `params`/`data_dir`/`exports`，多一个字段 400 |
| `artifactUrl()` / `sourceUrl()` | `…/artifacts/{name}`、`…/source/{path}` | 直链 |
| `listDatasources()` / `saveDatasource()` / `deleteDatasource()` | `/datasources` | 响应里**没有口令**，只有 `url_masked` |
| `probeDatasource(id, {table\|sql, limit})` | `POST /datasources/{id}/probe` | 空体 = `select 1`（只验连通）；表名优先于 sql |
| `probeRecord(record, …)` | `POST /datasources/probe` | 表单「测试连接」（还没保存） |
| `listTables(id)` | `GET /datasources/{id}/tables` | 拿不到时页面退化为「自己写 SQL」，不挡流程 |
| `upsertMetric` / `patchDimension` / `upsertRule` / `deleteRule` | `/bizmodels/{dsn}/{metric,dimension,rule}` | 人改即 `confirmed` + `human_edited:<日期>`；表达式先过函数白名单 |
| `editAlias` | `POST /bizmodels/{dsn}/alias` | 只动 `aliases`，**不动 status**（改别名不该让历史结果不可比） |
| `listGoldens` / `upsertGolden` / `deleteGolden` / `runGoldens` | `…/goldens*` | 期望至少一项（rows/columns/first_cell），否则后端 400 拒收 |
| `runQuery` / `compileQuery` | `/query/run`、`/query/compile` | **只收 IR**，服务端校验→计划→编译→执行；没有自由 SQL 通道 |

## 取舍记录（改之前先看这里）

1. **删掉 mock 而不是留着**：`executeReport()` 当年用内置 `RAW_SAMPLE_CSV` 在浏览器算，
   页面看着完整、数字全是假的。"看着对但不算"比"空着"危险 —— 假数据会被人当结论。
   **唯一例外**是 `#/ontology`：它演示的是"四步推理长什么样"，数据是 8 客户 / 11 订单 / 9 贷款的
   固定夹具，页面顶部常驻「示例数据 · 待后端」徽章，且所有数字由 `aggregate()` 现算（不是写死的对照表）。
   端点到位后只换 `resolve()` 的入参，界面不动。
2. **没有 Toast**：错误就地显示（页头下方红字卡片），不搞 5 秒消失的提示。
3. **没有"需求"这一层**：`requirement` 是 Python studio 的概念（`studio/query_ir.py`），
   Rust 侧没有对应端点。等 `rpx-semantic` 的 `query_ir` 落地再加，现在不加假的。
4. **快照必须是稳定引用**：`useSyncExternalStore` 的 `getSnapshot` 每次返回新对象会被
   React 判成"一直在变"（报 "The result of getSnapshot should be cached"）。
   空态用同一个冻结常量 `EMPTY_REPORT`，别改回 `{...}`。
5. **不在前端估页数/耗时**：`layout.page_count`、`started_at`/`finished_at` 全由引擎给。

## 本体解析器这一页的排版约定

- **一步一张卡**：标题（衬线）+ 一句说明 + 一张表/一段代码 + 一条结论，不在同一屏堆两件事；
- **步骤条固定四格**：序号 → 标题 → 副标题，当前步用铜色下缘 + 实心序号，走过的步打 ✓；
- **第 3、4 步不要侧栏**：对照表要横向空间，挤在 356px 的右栏里会把客户名压成一列竖排字（踩过）；
- **数字列右对齐 + `tabular-nums`**，表头等宽小字大写，列才对得齐；
- **不写 markdown**：JSX 里 `**粗体**` 和 `` `代码` `` 都是字面量，要用 `<b>` / `<code className="sql-inline">`。

## 下一步

1. 设计器：复用 `web/` 的 Univer 1.2.0 集成（含 `live-layout.ts` 的活态差量），删掉"手写表格"思路；
2. 语义层只剩 `analysis`（多步归因）没做 —— 依赖已就位（`POST /query/run` 能真出数），
   缺的是三步编排与结论文案，见 `docs/rpx-semantic-port.md` §三；
3. 模板保存：`saveTemplate` 已封装但没接 UI —— 需要先有"编辑模板"的界面（与设计器同一件事）。
