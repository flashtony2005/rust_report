# rpx-mcp 实施记录：报表引擎的 MCP 服务（P0-M）

> 日期：2026-10-21　状态：**M1–M5 已完成（32/32 验收通过）；M6 待用户在宿主注册后做对话级演练**
> 目标：把六步旅程（接数据源 → 选模板 → 讲需求 → 分析 → 确认规格 → 引擎出报表）封装为 MCP 工具，
> 让任意 AI 宿主（WorkBuddy / Claude / Cursor）通过结构化调用驱动引擎，无需 shell、无编码坑。

## 0. 两条 ADR（已按此实施）

| # | 决策 | 理由 |
|---|---|---|
| ADR-1 | **零依赖手写 stdio JSON-RPC**（`scripts/mcp/server.mjs`，ESM，~420 行含注释） | 本机 `npx` 被沙箱拦、npm 安装链路不可靠；MCP 必需面只有 `initialize / tools/list / tools/call` 三方法。SDK的 progress/resources 等能力本期用不上 |
| ADR-2 | **启动期 env 定白名单根，调用期传相对路径** | `RPX_ROOT` 限定 AI 可触达的目录（模板库 + 数据目录），调用只收相对路径 ⇒ 权限收敛，且比每次传绝对路径更不易越界 |

## 1. 架构（与计划一致，已落地）

```
任意 AI 宿主
  ↕ MCP stdio（结构化参数，绕开 shell 编码）
scripts/mcp/server.mjs（零依赖薄壳）
  ↕ spawn rpx.exe（无 shell、逐参数传递，天然免疫注入）
rpx.exe：report list · dataset inspect · spec analyze/compile · run
```

薄壳只做三件事：协议编解码、参数→命令行翻译、JSON 信封透传。
`rpx.exe` 路径取 `RPX_BIN`（缺省 `<repo>/target/release/rpx.exe`）。
另增 `RPX_LIBRARY`（缺省 `$RPX_ROOT/library`；本机实际库在 `examples/library`，注册时显式给）。
再增 `RPX_PRESETS_LIBRARY`（缺省 `$RPX_ROOT/examples/presets`）：**第二模板库**，与 `RPX_LIBRARY`
在 `list_templates` 里合并检索（见 §2.6）。两库路径都可在注册时显式覆盖。

## 2. 工具面（5 个，全部映射已有 CLI）

| 工具 | 参数 | 底层命令 | 返回 |
|---|---|---|---|
| `list_templates` | `query?`, `tag?`, `limit?`, `offset?` | 合并 `report list --library $RPX_LIBRARY` + `--library $RPX_PRESETS_LIBRARY` | 跨库去重后的分页条目（含 manifest 与 meta） |
| `inspect_dataset` | `file`, `limit?=5` | `dataset inspect <file>`（**M0 新增裸文件模式**） | 列名、行数、样本行、编码、分隔符 |
| `analyze_spec` | `spec`(对象), `data_dir?` | `spec analyze`（spec 落 `$RPX_ROOT/.specs/`） | `analysis` 全文（口径/发现/决策/未决问题） |
| `compile_spec` | `spec`, `name?`, `data_dir?` | `spec compile --out` | geometry + 模板路径（落 `$RPX_ROOT/compiled/`） |
| `run_report` | `template`, `data_dir?`, `exports?=["csv"]` | `run --out-dir --export` | run 摘要 + products 清单（含存在性与字节数） |

边界规则（已实现）：
- `exports` 白名单 `csv/xlsx/html`，非法值在薄壳层拒绝（不出子进程，T6.3 验证不落 run 目录）；
- `run` 的 out-dir 由薄壳自动生成唯一目录名且**只起名不创建**（引擎 `create_dir` 是 CREATE_NEW 语义，规避 os error 183）；
- 子进程超时 120s（`RPX_MCP_TIMEOUT_MS` 可调），超时 kill 并返回 `RPX_TIMEOUT`；
- spec/data/产物文件全部约束在 `RPX_ROOT` 内（含 `spec.dataset.file` 相对 data_dir 解析后的收口）；
- 工具名未知/参数形状错 → JSON-RPC -32602；方法未知 → -32601；坏 JSON → -32700 且进程继续服务。

## 2.5 M0：`dataset inspect` 裸文件模式（计划外新增，偏差说明）

计划假设 `dataset inspect <file>` 可直接查裸 CSV；**真实 CLI 原签名是「模板文件 + `--name`」**
（按模板内声明口径查，SQL 数据集走这条）。为支撑旅程第一步「刚上传数据、还没有模板」，
`--name` 改为可选：省略时按数据文件直查（`rpx-runtime/src/run.rs::inspect_data_file`）：

- 解析复用执行链同一套 `rpx_ds::file_source`，**探查看到的列就是引擎将来看到的列**；
- 分隔符嗅探（`,`/`\t`/`;`/`|` 看首行计数，默认逗号）—— 不嗅探时 TSV 会静默切成 1 列；
- 编码按**产品口径**：默认 GBK（`FileDataSetConfig.java:25`），GBK 解不动才兜底 UTF-8，
  并把命中的编码写进返回 JSON。不反过来是因为 `run` 模板链缺省就是 GBK，
  「探查说能读、一跑就炸」比报错更糟。spec 链路（analyze/compile）本来就固定发 `UTF-8`；
- Excel 走 `file_source::excel`，与编码之争无关。
- 钉子：`crates/rpx-runtime/tests/dataset_inspect_raw.rs`（9 条，含 GBK 夹具与 TSV 嗅探回归）。

## 2.6 `list_templates` 双库合并检索（第二批预制模板接入）

`report list` 只扫**单个** `--library` 根目录的直接子目录（不递归），没有"一键跨多库"的能力。
为把 `examples/presets`（30 个预制模板）接进检索面，又不污染现有 29 个线上模板，
在**薄壳层**做合并，而非改磁盘结构或引擎：

- 两个库 `RPX_LIBRARY` + `RPX_PRESETS_LIBRARY` 各自以 `--limit 500 --offset 0` 取全量
  （CLI 把 `--limit` 约束在 `1..=500`，用上限一次取尽；分页在合并后重做）；
- 跨库按 `id` 去重（保留首次出现）；当前两库 id 零重叠（29 + 30 = 59）；
- 合并后按 `id` 排序，再按调用方 `limit`/`offset` 重新分页；
- **单库故障**（目录缺失 / 清单损坏 / 引擎异常）被 `try/catch` 吞掉、归入 `diagnostics`，
  不拖垮整体检索——已验证：预设库路径不存在时，主库 29 条照常返回、不报错；
- 返回信封与 `report list` **逐字段一致**：`items` / `returned`(数字) / `total`(数字) /
  `next_offset` / `diagnostics`（不额外塞 `offset`/`limit`，避免消费方按 CLI 契约解析踩类型坑）；
- 合并后默认 `limit` 提到 500，否则无 limit 调用会静默截掉后半库模板（如 `sales-*` / `store-*` 排在字典序末尾）。
- 验收：`T2.1` 总数 59、`T2.1b` 预制 id 可达、`T2.2` 账龄跨库命中主库 `ar-aging` 与预制库 `fin-ar-aging-summary`、
  `T2.3` `tag=crosstab` 共 11 条且全带标签、`T2.4` `query=预算` 只命中预制库 `fin-budget-vs-actual`、
  `T2.5` 空结果非错误、`T2.6/T2.7` 分页与翻页生效。

## 3. 里程碑（实际完成情况）

| # | 内容 | 出口判据 | 状态 |
|---|---|---|---|
| M1 | 协议壳：initialize / tools/list / tools/call + 错误信封 | 管道喂 JSON-RPC 手工报文，握手与报错正确 | ✅ T1 全过 |
| M2 | 转发层：spawn + JSON 信封解析 | `list_templates` 真调返回 29 条 | ✅ T2.1 → 双库合并后 T2.1 校验返回 59 条 |
| M3 | 5 工具注册 + 参数校验 | 每个工具 happy path + error path 各过 | ✅ T2–T6 |
| M4 | 安全与健壮：白名单/超时/自动 out-dir | T7 全过 | ✅ |
| M5 | 自动化测试脚本 `test_server.mjs` 落库 | T1–T7 全绿 | ✅ **32/32** |
| M6 | WorkBuddy 注册 + 对话演练 + 记忆归档 | T8 六步对话级走通 | ⏳ 待用户（见 §6） |

## 4. 测试结果（`node scripts/mcp/test_server.mjs`，2026-10-21）

**32/32 通过（MCP_SERVER_OK）**。其中三处**实测行为优于计划原文**，已按实测修订：

| 用例 | 计划写法 | 实测行为（已按此断言） |
|---|---|---|
| T2.3 `tag=crosstab` | 「7 条」 | 确认 7 条；条目里 tags 在 `manifest.tags` 下（非顶层字段） |
| T5.2 `layout=pivot` | `SPEC_UNSUPPORTED` + hint | `SPEC_INVALID` + `json_pointer=$.layout` + message 列出受支持形态（list/grouped/crosstab）——指针更精确 |
| T1.4b 批量数组 | （计划未列） | `-32600` 且 id=null（JSON-RPC 惯例：请求不合法时 id 无法回认） |

T8 为人工对话级用例，依赖宿主注册，无法在脚本内闭环。

## 5. 交付物

- `scripts/mcp/server.mjs` —— MCP 服务壳（零依赖 ESM；被 import 时不启动服务，供测试复用 spawn 层）
- `scripts/mcp/test_server.mjs` —— 验收脚本（T1–T7，含 T6.5 假慢命令超时验证）
- `crates/rpx-runtime/src/run.rs::inspect_data_file` + `crates/rpx-cli` 的 `--name` 可选化
- `crates/rpx-runtime/tests/dataset_inspect_raw.rs` —— M0 钉子 9 条
- `~/.workbuddy/mcp.json` 的 `rpx` 条目（与既有 `rea` 合并，未覆盖）

## 6. 注册与演练（M6）

配置片段（node 用绝对路径，不走 npx）：

```json
"rpx": {
  "command": "C:\\Users\\Administrator\\.workbuddy\\binaries\\node\\versions\\22.22.2-6\\node.exe",
  "args": ["F:\\project\\report\\rpx\\scripts\\mcp\\server.mjs"],
  "env": {
    "RPX_ROOT": "F:\\project\\report\\rpx",
    "RPX_LIBRARY": "F:\\project\\report\\rpx\\examples\\library",
    "RPX_PRESETS_LIBRARY": "F:\\project\\report\\rpx\\examples\\presets",
    "RPX_BIN": "F:\\project\\report\\rpx\\target\\release\\rpx.exe"
  }
}
```

写盘后需在连接器管理页右上角「自定义连接器」对新服务器点**信任**才会生效（MCP 不会自动激活）。
演练口令（T8）：只给 AI 一句「用 rpx 的工具，examples/library/ar-aging/data/ar-aging.csv 出一张账龄分析」，
观察它是否自觉走 `inspect_dataset → list_templates → analyze_spec →（人等拍板）→ compile_spec → run_report`。

## 7. 已知边界（后续 P1 候选）

- ~~`--query` 检索仍是整串 substring 匹配，口语长句命中率约 6/12~~ **已修（2026-10-22）**：
  改为中文二元滑窗分词 + 多字段 OR + 命中计数排序（`library/search.rs`），口语长句实测
  12/12 命中且期望项排第一；MCP 的 `list_templates` 走同一入口，天然受益。
- 数据库数据源（MySQL/PostgreSQL）尚未进 MCP 工具面：需要数据源注册表传入 + 连接参数收口，
  本期只支持文件数据集。
- `exports` 白名单暂不含 `print-xlsx`/`pdf`（CLI 支持，壳层按最小面收口）。
