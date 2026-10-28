# rpx 需求分析平台 · 完整使用文档

> 适用版本：rpx v0.16（Rust 重写）/ MCP 服务 `rpx-mcp`（P0-M 已完成）
> 更新日期：2026-10-21
> 受众：终业务用户（对话式出表）、AI 宿主开发者（MCP 工具集成）、引擎运维（CLI 排错）

---

## 目录

1. [平台定位与核心旅程](#1-平台定位与核心旅程)
2. [环境准备与构建](#2-环境准备与构建)
3. [六步旅程详解](#3-六步旅程详解)
4. [端到端示例（命令级）](#4-端到端示例命令级)
5. [CLI 使用手册](#5-cli-使用手册)
6. [规格 `rpx-spec/v1` 参考](#6-规格-rpx-specv1-参考)
7. [模板元数据：两层分离设计](#7-模板元数据两层分离设计)
8. [MCP 工具参考](#8-mcp-工具参考)
9. [常见问题与排错](#9-常见问题与排错)
10. [已知边界与路线图](#10-已知边界与路线图)

---

## 1. 平台定位与核心旅程

rpx rust 写的报表引擎。本平台在引擎之上，提供一条**「数据 → 报表」**的低门槛路径：

```
接数据源 → 选模板 → 讲需求 → 分析 → 确认规格 → 引擎出报表
```

- **接数据源**：上传 CSV / Excel，或登记数据库链接（P1 待接入 MCP）。
- **选模板**：从模板库按业务关键词 / 能力标签检索，系统推荐最贴近的表样。
- **讲需求**：用自然语言描述「要什么表」，由助手把它落成结构化规格（spec）。
- **分析**：编译前先给出可审阅事实——口径、数据画像、默认决策、待拍板项。
- **确认规格**：人核对分析，必要时改 spec，再进入编译。
- **引擎出报表**：确定性编译器把 spec 转成可执行模板，引擎跑出 CSV/XLSX/HTML/PDF。

这条旅程**既能在终端用 CLI 一步步走，也能在 WorkBuddy/dsh 里用一句话（文本对话）完成**——后者的底层就是 MCP 工具（见第 8 节）。

### 组件一览

| 组件 | 形态 | 用途 |
|---|---|---|
| `rpx.exe` | Rust 二进制（CLI） | 报表定义管理、执行、规格编译/分析、数据集探查 |
| `examples/library` | 目录库 | 29 份内置业务/技术模板，含 `manifest.json` + `template.json` + 数据 |
| `scripts/mcp/server.mjs` | Node 脚本（零依赖） | MCP 服务壳，把 CLI 暴露成 5 个结构化工具 |
| `scripts/template_metadata.py` | Python 脚本 | 模板检索元数据的补写与校验 |

---

## 2. 环境准备与构建

### 2.1 依赖

- **Rust 工具链**（编译引擎）：需要 MSVC 链接器 + Windows SDK 10.0.26100.0。
  - 本机若报 `LNK1181`，先设好 `LIB` / `INCLUDE` 环境变量（见 §9）。
- **Node.js ≥ 22**（仅 MCP 服务需要）：`server.mjs` 是 ESM，零第三方依赖，但需 Node 直连（本机 `npx` 被沙箱拦截，勿用）。
- **Python 3**（仅 `template_metadata.py` 需要）：标准库即可，无需 `pip install`。

### 2.2 构建引擎

```bash
# 位置：仓库根 F:\project\report\rpx
cargo build --release -p rpx-cli
# 产物：target/release/rpx.exe
```

调试构建：`cargo build -p rpx-cli` → `target/debug/rpx.exe`。

### 2.3 自测（Doctor / Capabilities）

```bash
# 环境与能力自检（无需文档即可让 AI 了解能力面）
rpx doctor --library examples/library
rpx capabilities
```

`capabilities` 会输出命令面、支持的导出格式、字段数量限制、错误码清单——AI 宿主可直接读它，不必硬编码。

---

## 3. 六步旅程详解

下面以「区域 × 品类交叉统计，求销售额合计」为例，逐步展开。数据用内置夹具 `examples/r-model/crosstab-merge-cells.csv`：

```
region,store,month,cat,amount
华东,南京东路店,1月,空调,12000
...（12 行，含大区/门店/月份/品类四级重复）
```

### 步骤 1 · 接数据源

上传文件后，先**探查**它——确认列名、行数、编码、分隔符与引擎实际读取一致：

```bash
rpx dataset inspect examples/r-model/crosstab-merge-cells.csv --limit 3
```

返回（节选）：

```json
{
  "file": "crosstab-merge-cells.csv",
  "format": "delimited",
  "encoding": "GBK",
  "delimiter": ",",
  "columns": ["region", "store", "month", "cat", "amount"],
  "rows": 12,
  "sample": { "returned": "3", "total": "12", "items": [ ... ] }
}
```

> ⚠️ 编码按**产品口径默认 GBK**，GBK 解不动才兜底 UTF-8，并在返回里写明实际命中的编码。`spec analyze/compile` 链路固定按 UTF-8 读数据——若你的 CSV 是 UTF-8，告诉探查工具/规格没关系，但**模板链路的 `run` 仍是 GBK 默认**（参见 §9.4）。

### 步骤 2 · 选模板

从库里按业务词或能力标签检索：

```bash
# 按业务关键词（短词，substring 匹配——见 §10）
rpx report list --library examples/library --query 交叉表

# 按能力标签（精确匹配，更稳）
rpx report list --library examples/library --tag crosstab --limit 10

# 分页
rpx report list --library examples/library --tag crosstab --offset 0 --limit 5
```

返回条目含 `manifest`（名称、描述、tags）+ `meta`（业务域、关键词、难度、推导出的形态/能力/字段）。`tags` 在 `manifest.tags` 下，是「能力 + 形态」短标签集合。

> 选模板的目的有两层：① 给业务用户一个现成表样直接填数据；② 给助手一个**编译基底**——下面的 spec 流程可以基于某份模板的能力，让编译器只产出对应版式。

### 步骤 3 · 讲需求

用自然语言描述，助手把它落成 `rpx-spec/v1` 规格（可读、可审、可存档）。

示例需求：「**行按 大区→门店→月份 三级，列按 品类 展开，每格是销售额求和，末尾加总计和行小计**」→

```json
{
  "schema": "rpx-spec/v1",
  "title": "区域品类交叉统计",
  "dataset": { "name": "d", "file": "crosstab-merge-cells.csv" },
  "layout": "crosstab",
  "measures": [{ "field": "amount", "agg": "sum", "label": "金额" }],
  "totals": { "grand": true, "subtotal": true },
  "row_dims": [
    { "field": "region", "label": "大区" },
    { "field": "store", "label": "门店" },
    { "field": "month", "label": "月份" }
  ],
  "col_dims": [{ "field": "cat", "label": "品类" }]
}
```

### 步骤 4 · 分析

编译前先跑**分析**，把「这份规格会产出什么」摆在人面前——这是可核对的事实，不是 AI 文案：

```bash
rpx spec analyze my-spec.json --data-dir examples/r-model
```

返回（节选，`schema: rpx-spec-analysis/v1`）：

```json
{
  "layout": "crosstab",
  "dataset": { "name": "d", "file": "crosstab-merge-cells.csv", "rows": 12,
               "columns": ["region","store","month","cat","amount"] },
  "estimate": { "data_rows": 12, "cross_columns": 4,
                "how": "交叉表：行维度组合 × 列维度取值 的交叉格" },
  "calibers": [
    { "label": "金额", "agg": "sum", "expr": "amount",
      "format": "#,##0", "scope": "行列交集聚合：每个格带全部行维度与列维度条件",
      "source": "$.measures[0]" },
    { "label": "总计", "agg": "sum", "expr": "d.sum(amount)",
      "scope": "全表口径（不带任何条件）", "source": "$.totals.grand" }
  ],
  "findings": [ ... ],
  "decisions": [
    { "what": "crosstab 版式：行维度纵向、列维度横向、交集格聚合", "source": "$.layout" },
    { "what": "维度 region 未指定排序，按数据首次出现序分组",
      "source": "$.row_dims[0].sort",
      "evidence": "rpx-engine/src/expr/dsgroup.rs:65-71 无 sort 参数时按扫描序建组键" }
  ],
  "open_questions": [
    { "question": "维度 month 未指定 sort，将按数据首次出现序分组（1月→...）；与字典序不同。若需特定顺序请指定 asc/desc",
      "source": "$.row_dims[2].sort", "evidence": "..." }
  ]
}
```

四类产出：
- **口径（calibers）**：每个度量怎么算、作用域是什么，带 `source` 指回规格。
- **探查发现（findings）**：数据画像——行数、字段取值、非数值、空值。
- **决策日志（decisions）**：编译将采用的每个默认值，带 `file:line` 实证。
- **未决问题（open_questions）**：需要人拍板的分歧（如排序序、空值处理），**只报确实存在**的。

> 排序口径提醒：rpx 在无 `sort` 时按**数据首次出现序**（扫描序），不是字典序——这是已实证的引擎行为（`dsgroup.rs:65-71`）。若业务要字典序，务必在 spec 里写 `sort: "asc" | "desc"`。

### 步骤 5 · 确认规格

人读分析报告。若发现问题（例如发现维度有空值、或想按字典序排），**改 spec 重跑 analyze**，直到分析干净。这一步是「确认」而非「盲信」。

### 步骤 6 · 引擎出报表

规格确认后，确定性编译成模板，再出报表：

```bash
# 编译（确定性：同输入必同输出）
rpx spec compile my-spec.json --out compiled/cross.json --data-dir examples/r-model

# 跑引擎出 CSV（一次一个导出格式）
rpx run compiled/cross.json --out-dir out/cross --export csv
```

`run` 返回（节选）：

```json
{
  "run": { "report_id": "...", "revision": "...", "layout": { "rows": 12, "cols": 8 } },
  "out_dir": "F:\\project\\report\\rpx\\out\\cross",
  "result":  "F:\\project\\report\\rpx\\out\\cross\\artifacts\\result.json",
  "workbook": "F:\\project\\report\\rpx\\out\\cross\\artifacts\\workbook.json",
  "layout": { "rows": 12, "cols": 8 }
}
```

产物在 `out_dir/artifacts/`：
- `result.json` / `workbook.json`：结构化结果（程序消费）
- `report.csv` / `report.xlsx`：表格导出
- `report.html` / `report.pdf`：打印视图（HTML/PDF 走同一快照契约）
- `report-print.xlsx`：套打版式

> `out-dir` 必须不存在（引擎 `CREATE_NEW` 语义）——若目录已存在会报 `IO_ERROR (os error 183)`。每次跑换个新目录即可。

---

## 4. 端到端示例（命令级）

把第 3 节串成一条可复现的命令链（在 `F:\project\report\rpx` 下执行）：

```bash
# 0) 准备规格（写入 my-spec.json）
cat > my-spec.json <<'EOF'
{
  "schema": "rpx-spec/v1",
  "title": "区域品类交叉统计",
  "dataset": { "name": "d", "file": "crosstab-merge-cells.csv" },
  "layout": "crosstab",
  "measures": [{ "field": "amount", "agg": "sum", "label": "金额" }],
  "totals": { "grand": true, "subtotal": true },
  "row_dims": [
    { "field": "region", "label": "大区" },
    { "field": "store",  "label": "门店" },
    { "field": "month",  "label": "月份" }
  ],
  "col_dims": [{ "field": "cat", "label": "品类" }]
}
EOF

# 1) 探查数据
rpx dataset inspect examples/r-model/crosstab-merge-cells.csv --limit 2

# 2) 选模板（看交叉表类有哪些）
rpx report list --library examples/library --tag crosstab

# 3) 分析规格（人确认口径/排序/空值）
rpx spec analyze my-spec.json --data-dir examples/r-model

# 4) 编译 + 出表
rpx spec compile my-spec.json --out compiled/cross.json --data-dir examples/r-model
rpx run compiled/cross.json --out-dir out/cross --export csv
rpx run compiled/cross.json --out-dir out/cross --export xlsx
```

### 库内报表（免编译）路径

若直接用库里现成模板（如账龄分析表 `ar-aging`）：

```bash
# 库内报表直接跑（数据在报表子目录的 data/ 下）
rpx report run ar-aging --library examples/library --export csv

# 查看某报表源码/状态
rpx report show ar-aging --library examples/library

# 查看运行历史
rpx report runs ar-aging --library examples/library --limit 10
```

数据集 `ar-aging/data/ar-aging.csv` 表头：`customer, amount, days`。

---

## 5. CLI 使用手册

所有命令默认输出 JSON（`--output json`，可加 `--output text` 看人读摘要）。总入口：`rpx <command> [--output json|text]`。

### 5.1 `rpx doctor` — 环境自检

```bash
rpx doctor [--library DIR] [--datasources FILE]
```
检查目录库可读写、数据源注册表可解析（不建连接）。

### 5.2 `rpx capabilities` — 能力自描述

```bash
rpx capabilities
```
输出命令面、导出格式、行/列限制、错误码清单。AI 宿主应读它了解能力，而非背文档。

### 5.3 `rpx schema` — 内置 JSON Schema

```bash
rpx schema template        # 输出模板 JSON schema
```

### 5.4 `rpx lint` — 静态检查

```bash
rpx lint <template.json> [--data-dir DIR] [--strict]
```
拦截「不报错但产物错」的写法（如聚合缺 `维度==主格` 条件）。`--strict` 时即便只有 warning 也返回退出码 3（CI 闸门用）。

### 5.5 `rpx report list` — 检索模板库

```bash
rpx report list --library DIR [--query 词] [--tag 标签] [--limit N] [--offset M]
```
- `--query`：**中文二元滑窗 + 多字段 OR + 命中计数**（2026-10-22 起）。长句按 bigram 拆词（「应收账款账龄」→ 应收/收账/账款/款账/账龄），候选文本 = manifest 里全部字符串值（name/description/tags/**meta 关键词/域/例句/能力**），按命中词数排序、同分按 id；整串原样出现加倍。口语长句实测 **12/12 命中且排第一**（旧整串 substring 为 3/12）。见 §10。
- `--tag`：精确匹配 `manifest.tags`（能力 + 形态短标签，如 `crosstab` `grouped` `agg_sum`）。
- 返回分页条目（含 `manifest` + `meta`）。

> 其他 `report` 子命令：`show` / `runs` / `copy` / `patch` / `validate` / `run` / `update` / `import-json` / `delete` / `migrate`（v0→v1）/ `prune`（清旧运行记录）。

### 5.6 `rpx dataset inspect` — 探查数据集

```bash
rpx dataset inspect <file> [--name 数据集名] [--data-dir DIR] \
                        [--datasources FILE] [--limit N]
```
- **省略 `--name`** → 把 `file` 当裸数据文件直查（M0 新增）：嗅探分隔符、判定编码（GBK 优先/UTF-8 兜底并上报）、返回列名/行数/样本。
- **带 `--name`** → 按模板内声明的数据集口径查（SQL 数据集走这条，配合 `--datasources`）。

返回：`file / format(delimited|excel) / encoding / delimiter / columns / rows / sample{returned,total,items}`。

### 5.7 `rpx spec analyze` — 规格分析

```bash
rpx spec analyze <spec.json> --data-dir DIR [--out analysis.json]
```
吃规格 + 数据集字节，吐可审阅分析 JSON（`rpx-spec-analysis/v1`）：`estimate / fields / calibers / decisions / findings / open_questions`。详见 §3 步骤 4。

错误：字段名拼错 → `SPEC_INVALID` + `json_pointer`（如 `$.row_dims[0].field`）。

### 5.8 `rpx spec compile` — 规格编译

```bash
rpx spec compile <spec.json> [--out template.json] [--dry-run] [--data-dir DIR]
```
确定性编译，输出模板 JSON。返回：

```json
{
  "geometry": { "rows": 12, "cols": 8 },
  "lint": { "issues": [...], "summary": {...}, "gate": {...} },
  "out": "F:\\...\\compiled\\cross.json",     // 仅 --out 且无 --dry-run 时
  "template": { ... }                          // 否则内联完整模板
}
```

错误码：
- `$.layout` 非法（如 `pivot`）→ `SPEC_INVALID` + `json_pointer=$.layout`，message 列出受支持形态（`list`/`grouped`/`crosstab`）。
- `$.dataset.file` 缺失 → `SPEC_INVALID`。

> 三种受支持形态：`list`（明细表）/`grouped`（分组汇总）/`crosstab`（交叉表）。`pivot` 等不在本编译器范围内。

### 5.9 `rpx run` — 直接执行模板

```bash
rpx run <template.json> --out-dir DIR \
    [--data-dir DIR] [--params-file FILE] [--datasources FILE] \
    --export csv|xlsx|print-xlsx|html|pdf
```
- `--out-dir`：**必须不存在**（CREATE_NEW 语义）。
- `--export`：**可重复**，一次可导出多个格式（如 `--export csv --export xlsx`），循环逐个产出到 `artifacts/` 下各自的文件（`report.csv` / `report.xlsx` / `report.html` / `report.pdf` / `report-print.xlsx`）。
- 产物见 §3 步骤 6。

### 5.10 `rpx template` / `rpx result` / `rpx catalog` / `rpx library`

- `rpx template inspect|validate|create|patch`：模板级查看/校验/从示例创建/打补丁。
- `rpx result inspect <result.json>`：分页查看运行结果。
- `rpx catalog functions [--name 名]` / `rpx catalog examples`：内置函数与示例清单。
- `rpx library init DIR [--presets]`：初始化一个空目录库。

---

## 6. 规格 `rpx-spec/v1` 参考

规格是「自然语言需求 → 模板」的中间层，可读、可审、可存档。**版本不对直接拒绝**，不做尽力兼容。

### 顶层字段

| 字段 | 必填 | 说明 |
|---|---|---|
| `schema` | ✅ | 固定 `"rpx-spec/v1"` |
| `title` | ✅ | 人类可读标题 |
| `dataset.name` | ✅ | 数据集名（编译后作为 `ds.<agg>(...)` 前缀） |
| `dataset.file` | ✅ | 数据文件名，相对 `--data-dir` 解析 |
| `layout` | ✅ | `list` / `grouped` / `crosstab` |
| `measures` | ✅ | 度量数组（≥1） |
| `row_dims` | ⭕ | 行维度（grouped/crosstab 用） |
| `col_dims` | ⭕ | 列维度（crosstab 用） |
| `totals` | ⭕ | `{ grand?: bool, subtotal?: bool }` |

### 度量子段 `measures[i]`

| 字段 | 说明 |
|---|---|
| `field` | 聚合目标字段（也可写表达式如 `qty*price`，编译器会提取字段逐一校验存在性） |
| `agg` | `sum` / `count` / `avg` |
| `label` | 输出列/格标签 |
| `format` | 可选，默认 `#,##0`（千分位、0 位小数） |
| `expr` | 高级：直接写表达式（替代 `field`） |

### 维度子段 `row_dims[i]` / `col_dims[i]`

| 字段 | 说明 |
|---|---|
| `field` | 维度字段名（必须存在于数据列） |
| `label` | 维度标签 |
| `sort` | 可选：`asc` / `desc` / 省略。省略 → **按数据首次出现序**（非字典序，见 §3.4 提示） |

### 三条硬约束（写错不报错，只出错数）

1. 只有**根列**是 `vertical`；其余列一律 `none` + `left_head`。多列各自 `vertical` 不串父子链，N 行会乘成数百行。
2. 聚合必须显式带 `维度==主格` 条件（编译器自动加，但你写的自定义 expr 要留意）。
3. 扩展行与合计行之间不留空行，否则空行被算进块内。

---

## 7. 模板元数据：两层分离设计

模板库检索/推荐依赖一份**元数据**。它故意分成两层，各自唯一来源：

### 7.1 手写语义层（`TABLE`，人维护）

业务意义，机器推不出来：`title` / `kind`(business|pattern) / `domain`(受控词表) / `keywords`(2–6 短词) / `examples`(口语需求句) / `difficulty`(1–3)。

### 7.2 自动推导结构事实层（`derive()`，程序根据模板实测）

形态 `shape`、扩展方向 `extends`、能力 `capabilities`、引用字段 `datasets.fields`、是否依赖 SQL。这些写进 `manifest.meta` 只是**缓存**，`check` 时重新推导比对——模板一改，元数据不同步就被抓住。

### 7.3 为什么分两层

1. **变质速度不同**：结构事实随编辑立刻变，手写副本只会静默撒谎；业务语义永不随模板改。
2. **可判定性不同**：推导层 100% 可重算，`check` 能钉死；手写层程序只验低保真度（关键词≥3、每份至少一个独有关键词、受控词表）。**机器负责判得死的，人负责判不死的。**
3. **消费方不同**：检索/推荐只吃手写层（业务语言），编译器能力判断只吃推导层（事实）。故 `tags` 只放能力+形态，域走 `meta.domain`——同一信息两种写法必然漂移。

### 7.4 操作

```bash
# 补写（把 TABLE 的手写层 + derive 的推导层写进每个 manifest.meta）
python scripts/template_metadata.py apply --library examples/library

# 校验（退出码 0 通过，1 有缺漏/漂移，可直接进 CI）
python scripts/template_metadata.py check --library examples/library
```

`check` 校验项：meta.schema、shape/extends/capabilities/datasets 与实测一致、受控词表、关键词≥3、examples≥1、difficulty∈{1,2,3}、每份模板至少一个独有关键词、manifest.name 已设人类可读名。

---

## 8. MCP 工具参考

`scripts/mcp/server.mjs` 把引擎封装成 **5 个 MCP 工具**。任意 AI 宿主（WorkBuddy / Claude / Cursor）通过结构化调用驱动引擎，**无需 shell、无中文编码坑**。

### 8.1 架构

```
任意 AI 宿主
  ↕ MCP stdio（结构化参数，绕开 shell 编码）
scripts/mcp/server.mjs（零依赖薄壳）
  ↕ spawn rpx.exe（无 shell、逐参数传递，天然免疫注入）
rpx.exe：report list · dataset inspect · spec analyze/compile · run
```

薄壳只做：协议编解码、参数→命令行翻译、JSON 信封透传。

### 8.2 注册（WorkBuddy）

编辑 `~/.workbuddy/mcp.json`，**保留既有条目**追加 `rpx`：

```json
{
  "mcpServers": {
    "rea": { "...": "既有条目保持不变" },
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
  }
}
```

> 写盘后，须在**连接器管理页右上角「自定义连接器」**对新服务器点**信任**才会生效（MCP 不自动激活）。

环境变量：
- `RPX_ROOT`：AI 可触达的根目录（模板库 + 数据目录），调用只收相对路径 ⇒ 权限收敛。
- `RPX_LIBRARY`：主目录库路径（缺省 `$RPX_ROOT/library`；本机库在 `examples/library`）。
- `RPX_PRESETS_LIBRARY`：第二目录库（预制模板集，缺省 `$RPX_ROOT/examples/presets`）。
  `list_templates` 把两个库合并检索（跨库去重、单库故障不拖垮整体）；见 §8.4。
- `RPX_BIN`：引擎路径（缺省 `<repo>/target/release/rpx.exe`）。
- `RPX_MCP_TIMEOUT_MS`：子进程超时（缺省 120000ms），超时 kill 并返回 `RPX_TIMEOUT`。

### 8.3 工具面

| 工具 | 参数 | 底层命令 | 返回 |
|---|---|---|---|
| `list_templates` | `query?` `tag?` `limit?` `offset?` | 合并 `report list`（`RPX_LIBRARY` + `RPX_PRESETS_LIBRARY`） | 跨库去重后的分页条目（manifest + meta） |
| `inspect_dataset` | `file` `limit?=5` | `dataset inspect` | 列名/行数/样本/编码/分隔符 |
| `analyze_spec` | `spec`(对象) `data_dir?` | `spec analyze` | `analysis` 全文 |
| `compile_spec` | `spec` `name?` `data_dir?` | `spec compile --out` | geometry + 模板路径 |
| `run_report` | `template` `data_dir?` `exports?=["csv"]` | `run --out-dir --export` | run 摘要 + products 清单 |

### 8.4 边界规则（已实现）

- `exports` 白名单 `csv/xlsx/html`：非法值在薄壳层拒绝（**不出子进程**，不落 run 目录）。
- `run` 的 `out-dir` 由薄壳自动生成唯一目录名且**只起名不创建**（规避引擎 CREATE_NEW 的 os error 183）。
- 所有 spec/data/产物文件约束在 `RPX_ROOT` 内（含 `spec.dataset.file` 相对 `data_dir` 解析后的收口）。
- 工具名未知/参数形状错 → JSON-RPC **-32602**；方法未知 → **-32601**；坏 JSON → **-32700** 且进程继续服务。
- `list_templates` 双库合并：`RPX_LIBRARY` 与 `RPX_PRESETS_LIBRARY` 各自以 `--limit 500` 取全量，跨库按 `id` 去重、排序后重新分页；**单库故障（目录缺失/清单损坏）归入 `diagnostics`，不报错、不拖垮整体**。合并后默认 `limit=500`，否则无 limit 调用会静默截掉字典序靠后的模板（如 `sales-*`/`store-*`）。返回信封字段与 `report list` 一致（`items`/`returned`/`total` 为数字 + `next_offset`）。

### 8.5 对话级演练（T8）

注册信任后，给 AI 一句：

> 「用 rpx 的工具，把 `examples/library/ar-aging/data/ar-aging.csv` 出一张账龄分析表」

观察它是否自觉走完：
`inspect_dataset → list_templates → analyze_spec →（等你在分析结果上拍板）→ compile_spec → run_report`。

---

## 9. 常见问题与排错

### 9.1 中文 CLI 参数在 Git Bash 下检索为空

Git Bash 会对中文参数做编码转换，导致 `--query 账龄` 落空。**验证中文检索用 Python `subprocess` 直接传参**更可靠：

```python
import subprocess, json
out = subprocess.run(
    ["target/release/rpx.exe", "report", "list",
     "--library", "examples/library", "--query", "账龄"],
    capture_output=True)
print(json.loads(out.stdout))
```

MCP 路径天然无此问题（走结构化 JSON-RPC）。

### 9.2 构建报 `LNK1181` 找不到库

本机 Community 版链接器路径需显式设 `LIB` / `INCLUDE`：

```powershell
$env:LIB = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.43.34808\lib\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\ucrt\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64"
$env:CARGO_TARGET_DIR = "F:\project\report\rpx\target"
cargo build --release -p rpx-cli
```

### 9.3 `run` 报 `IO_ERROR (os error 183)`

`--out-dir` 已存在。换一个新目录（或删掉旧的）再跑。MCP 的 `run_report` 已自动处理（每次生成唯一目录名）。

### 9.4 编码：探查说 UTF-8 能读，一跑 `run` 就乱码

`dataset inspect` / `spec analyze` / `spec compile` 链路**固定 UTF-8**；但 `run` 的**模板链路默认 GBK**。若数据是 UTF-8 又走 `run`，需确保模板/数据集声明里指定 `encoding: "UTF-8"`（或把数据另存 GBK）。探查返回的 `encoding` 字段即引擎实际用的解码口径，照它对齐即可。

### 9.5 `layout=pivot` 编译失败

`pivot` 不在本编译器范围。受支持形态：`list` / `grouped` / `crosstab`。错误为 `SPEC_INVALID` + `json_pointer=$.layout`，message 会列出受支持形态。

### 9.6 排序结果和预期不同

未指定 `sort` 时按**数据首次出现序**（非字典序，实证于 `dsgroup.rs:65-71`）。要字典序就在 spec 写 `sort: "asc" | "desc"`。

---

## 10. 已知边界与路线图

| 项 | 状态 | 说明 |
|---|---|---|
| `--query` 口语检索 | **已修（2026-10-22）** | 旧实现整串 substring，口语长句实测 **3/12**。改为：①**CJK 二元滑窗**分词（无词典，bigram 是零依赖下最稳的近似）；②**多字段 OR**（haystack = manifest 全部字符串值，含 `meta.keywords`/`examples`/`domain`）；③**命中计数**排序（整串原样出现加倍）。实测 **12/12 命中且期望项全排第一**（判据 `tests/fixtures/out/query_hitrate.py`；单测 `library::search::tests::*` 6 条，含端到端排序钉子）。实现在 `library/search.rs`（新增，~110 行），`library.rs::list` 只接线 |
| 数据库数据源 | P1 | MySQL/PostgreSQL 尚未进 MCP 工具面；需数据源注册表传入 + 连接参数收口。本期 MCP 只支持文件数据集（CLI 的 `dataset inspect --name --datasources` 已支持 SQL 数据集探查） |
| `exports` 白名单 | 收口中 | 壳层暂只放 `csv/xlsx/html`；`print-xlsx`/`pdf` CLI 支持但壳层未暴露 |
| 详情报表内存/耗时 O(n²) | **已修（2026-10-22，两轮）** | **内存侧**：5 万行明细原峰值 **9,900 MB**（n²×4 字节、与列数无关）。根因：`ext_algo.rs::set_ext_cells` 对每个克隆 `members.to_vec()` 独立深拷贝。修复：`HeadCell.ext_cells: Option<Arc<[CellId]>>`，`extend_*` 只建一次 Arc、逐克隆 `Arc::clone`；实测 50k：9,900 → 1,485 MB（10k/20k/50k = 311/605/1,485 MB ⇒ 线性）。变异钉子 `ext_algo::tests::extend_row_shares_one_ext_cells_allocation`（`Arc::ptr_eq`）+ `scripts/bench_detail_mem.py`。**耗时侧**：旧归因「`members_of` 扫描是耗时主因」**已证伪**（实测 2000 行明细 400 万次调用装入 0 个实例；真正病灶在分页与快照）。当日续定位并修掉三处：① `set_ds_current_left_head` 成员扫描**惰性短路**（`ds_filled` 派生判据，不用宿主计数器；关闭短路 A/B 对拍 `report.csv`/`workbook.json` 逐字节一致，钉子 `ds_current::tests::left_head_scan_*` 三条）；② `col_values → CalcGrid::get` 是 O(cells) 线性扫描，分页器每页每行查首列 ⇒ O(pages×rpp×cells)（50k：62.5 亿次比较、38,677 ms）⇒ 新增 `pager::CellIndex` 一次性 `(row,col)→下标` 索引（`entry().or_insert()` 首现胜，与 `iter().find` 同语义；钉子 `cell_index_*` 三条），50k PAGINATE 38,677 → **23.72 ms**；③ `project()` 每页重建全网格 HashMap（O(pages×cells)，50k/1429 页 ≈ 3.57 亿次插入 ≈ 21 s）⇒ 索引下沉为 `project_page_with` 参数、两热路径循环外只建一次。快照侧另拆 `sheet_json`/`workbook_json` 免掉每页 `to_string`→`from_str` 往返（serde_json 无 `preserve_order` ⇒ Map 是 BTreeMap，输出逐字节不变）。**50k 端到端 259.9 s → 7.6 s**，产物与修复前逐字节一致。**残余（已清零，2026-10-22 第三轮）**：sheets J 树与 `result.json` 的整树/整串开销已用**流式序列化**消掉 —— `snapshot::write` 逐页 `to_writer`（sheet 树写完即弃）、`result::write` 逐格 `serialize_element`，二者都按 BTreeMap 字母序 `serialize_entry`、格式仍由 `serde_json` 产出 ⇒ 产物**逐字节不变**。50k 实测峰值 **1,711.7 → 234.9 MB（7.3×）**、耗时 8.66 → 6.02 s；10k 峰值 353.6 → 56.5 MB。钉子 `snapshot::stream_tests::paged_stream_matches_legacy_assembly`（含「`sheets` 按 id 字典序、`sheetOrder` 按页序」那条坑）+ `result::tests::stream_matches_build_bytes`。CLI 用 `files::create_with`（1 MiB `BufWriter`，否则逐 token 直写 `File` 会 2.6 s → 37.5 s） |
| MCP 对话演练 T8 | 待用户 | 需在宿主注册信任后人工走一遍六步对话 |

---

## 附：校验命令速查

```bash
# 引擎能力自检
rpx doctor --library examples/library
rpx capabilities

# 模板元数据（CI 闸门）
python scripts/template_metadata.py check --library examples/library

# MCP 服务验收（32 用例）
node scripts/mcp/test_server.mjs

# 全量回归
cargo test --manifest-path Cargo.toml
```

> 文档与代码同版本管理；命令签名以 `rpx --help` 与各子命令 `--help` 为准，本文档如有出入以源码（`crates/rpx-cli/src/args.rs`、`crates/rpx-runtime/src/{analyze,compile,run}.rs`）为最终依据。
