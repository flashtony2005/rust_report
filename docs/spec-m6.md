# Spec — ReportLite Rust 重实现 · M6（导出面）v0.5

> 状态：**M6-01 / M6-02 / M6-03 / M6-04 已闭合**（2026-10-15；蓝图 §10 既有设计，同日补契约与任务表）。
> `rpx-export`：`to_csv` / `to_xlsx`；server `POST /export`；CLI `render --out`
> （审计遗留③闭合）。M6-05 未启动。
> 上游：M1–M4 已收官，M5（多数据集关联）进行中（`docs/spec-m5.md`）。
> 蓝图：`docs/reportlite-rust全链路设计.html` §10（导出设计）/ §8.1（路由表 `/export`）。
> 本文件定义 **M6 做什么、输入是什么、怎么验收**；实现细节以夹具与源码为准。

## 0. 本文件的可信度声明

凡未附证据的断言一律标注「待验证」。与 M5 不同，M6 **没有 Java 导出真值可对拍**
（ReportLite 开源版不含 xlsx 导出，蓝图 §5 已记录该外部商业事实待复核），
故 M6 的对拍以**自洽对拍**为主、Java 探针真值为带账（§5.1）。

## 1. 立项依据：为什么是 M6

当前所有交付面都是「**看**」：`.rpx` → 引擎计算 → Univer 浏览器预览（M4）。
**没有任何一个字节是导出的文件**：

| 出口 | 现状 |
| --- | --- |
| CSV | **完全不存在**（仓里 CSV 只作数据集输入；`completeness-audit.md` §2.5③ 早已点名） |
| xlsx | 不存在（无 `rpx-export` crate） |
| PDF / 打印 | 不存在（蓝图 §11 的 `rpx-print`，后置） |
| CLI 导出 | `rpx-cli` 仍只有 `print` 一个子命令（审计遗留项③） |

报表工具的终局交付物是「**可再编辑的数据文件**」与「**可归档打印的版式文件**」。
蓝图 §10 已定方向：**导出与打印都走 Rust 侧，不依赖 Univer Pro**（Pro 的
xlsx 导入导出/打印是付费能力，免费档有页数与水印限制）。本里程碑先做
**数据保真面（CSV → xlsx）**；版式保真面（PDF/打印）另立 `rpx-print`，不在 M6。

## 2. 设计决策

### 2.1 新 crate `rpx-export`（受审层）

```
rpx-core / rpx-engine（零依赖层，不动）
        ↑
rpx-export（新增，受审层）── 吃 &CalcGrid，吐文件字节
        ↑
rpx-server（加 /export 路由）  +  rpx-cli（加 render --out）
```

- **输入是 `&rpx_engine::arena::CalcGrid`**（`crates/rpx-engine/src/arena/calc_grid.rs:176`：
  `rows/cols: i32` 数据格数、`cells: Vec<CalcCell>` 稀疏、`MergedCell{row_merge,col_merge}`）。
  **不经过** server 的 Univer snapshot JSON——那是给 canvas 看的中间形态，
  导出要的是计算态矩阵本身（蓝图 §10「Java BaseExcel 的锚点+跨度逻辑可直接复用」）。
- **零对拍依赖**：导出器不读 `.rpx`、不跑 SQL，只吃已经算好的矩阵。
  这与 core/engine 的零依赖不变式不冲突（导出器本就是受审层）。

### 2.2 出口顺序：CSV 先行，xlsx 跟进，PDF 后置

蓝图 §10 原话：「**CSV：先做，用于对拍验证**（rpx-cli 输出 CSV vs Java 版导出对比）」。
本 spec 采纳，并按本仓纪律细化：

| 序 | 出口 | 为什么这个顺序 |
| --- | --- | --- |
| M6-01 | **CSV 文本** | 格式最简单、无第三方依赖（engine 纯 std 即可写）；既是对拍基准，又是 CLI 第一个真导出 |
| M6-02 | **xlsx** | `rust_xlsxwriter`；含 `merge_range`、行高列宽后置 |
| M6-03 | server `/export` 路由 | 把前两个接到 HTTP 文件流 |
| M6-04 | CLI `render --out` | 补审计遗留③（凡验收写了 `CLI X` 就必须有真跑 CLI 的断言） |
| — | PDF / Word / HTML | **后置**，`rpx-print` 另立里程碑（蓝图 §11） |

### 2.3 CSV 格式契约（M6-01）

无 Java 真值时，**自己定 RFC 4180 子集**，并写死：

- 分隔符 `,`；文本字段含 `,` `"` `\n` 时双引号包裹、内部 `"` 双写；
- **稀疏格落空前**：`CalcGrid` 的 `cells` 只装非空格（emit.rs:26-28 同纪律），
  CSV 必须按 `(row, col)` 展开成**满矩阵**——空格落为空串，不留洞；
- `disp_value` 优先于 `value`（与前端展示一致）；`value` 为 null → 空串；
- 行尾 `\r\n`（RFC 4180）；BOM **不写**（Excel 中文乱码问题由导出 xlsx 通道解决，
  CSV 定位是对拍与数据交换，不是给 Excel 双击打开）。

> ⚠ 这套格式是**自定**，不是 Java 产品格式。Java 产品导出 CSV 的真实记法
> （空值记 `""` 还是 `null`、是否带表头、数字是否带千分位）**未探针**——
> 见 §5.1 带账。

### 2.4 xlsx 对拍方式（M6-02）

xlsx 是 zip 容器，**字节级对拍无意义**（时间戳/zip 条目序都不同）。
验收用**读回自洽**：

```
CalcGrid ──rpx-export──> out.xlsx ──calamine 读回──> (r,c)→值表 + 合并区列表
                                                          ↓
                              与 CalcGrid 逐格比值/类型 + 合并区与 MergedCell 全等
```

`calamine` 只作 **dev-dependency**（读回校验用，不进导出器本身）——
G5 只查 `[dependencies]`（`code-review.md` §3.2 注 3），dev-deps 不进产物。

### 2.5 G5 白名单登记（开工前先登记）

| crate | 依赖 | 为什么需要 | 替代方案为何不行 |
| --- | --- | --- | --- |
| `rpx-export` | `rust_xlsxwriter`（默认 features） | xlsx 写器：`worksheet.write_string/number` + `merge_range` | ① 手写 xlsx（zip + OOXML 都是几百个 XML 节点，格式边界全靠自证，且无对拍基准）；② 引擎是零依赖层，不能为它引 |

> ⛔ 按 `code-review.md` §3.2：只改 `scripts/dep_allowlist.txt` 不写这张表 = 违规。
> 本格即登记正文。
> **（v0.3 补记）已登记**：`scripts/dep_allowlist.txt` 加 `rpx-export rust_xlsxwriter`
> 一行；`docs/code-review.md` §3.2 登记表同步加行（含 calamine dev-dep 注记）；
> 实际版本由 Cargo 解析为 **0.99.1**（`"0.99"`）；G5 全绿。

## 3. 任务表（M6）

| 编号 | 任务 | 落点 | 验收 |
| --- | --- | --- | --- |
| M6-01 | `rpx-export` crate 骨架 + `to_csv(&CalcGrid) -> String` | 新 crate；workspace member | ✅ **已闭合**（2026-10-15）：`crates/rpx-export`（`lib.rs` 契约文档 + `csv.rs` 规则 + `text.rs` 文本口径，G8 矩阵登记 `rpx-export -> rpx-engine`）；10 条钉测 = 满矩阵展开/零维网格/越界忽略/disp 优先/null 空串/五组转义/List 引号/数值 shortest/非有限数保留/读回逐格自洽；`full_matrix` 等以**字面量**断言（非自证） |
| M6-02 | `to_xlsx(&CalcGrid) -> Vec<u8>`（rust_xlsxwriter） | 同 crate；`merge_range` 写合并 | ✅ **已闭合**（2026-10-15）：`xlsx.rs`（口径表 + 锚点重建 + 边界 5 条）＋`xlsx/tests.rs` 16 钉测；dev-dep `calamine` 读回自洽（逐格比值/**类型** + 合并区矩形全等）；3 条变异实测（disp 优先/延续槽判定/非有限数拦截，改坏必红）；确定性（同输入同字节）有钉 |
| M6-03 | server `POST /export?fmt=csv\|xlsx` | `rpx-server/src/http.rs` | ✅ **已闭合**（2026-10-15）：`rpx-server/src/export.rs`（格式枚举=严格小写解析 / 响应组装 / 文件名双形态 ASCII fallback + RFC 5987）+ `http.rs` 拆出 `calculate_grid` **共享段**（`/render` 与 `/export` 同一次调用，出口分家不分叉口径）；真 socket 跑通（`tests/export_http.rs` 7 条：csv/xlsx **逐字节对拍库函数直调** + `Content-Disposition` 双形态 + 缺省 xlsx + 非法 fmt 400 + 中文报表名过网 + GBK 中文链路 + 缺文件 400）；3 条变异实测（消毒 / 编码 / 缺省，改坏必红） |
| M6-04 | CLI `render sample.rpx --out out.csv\|xlsx` | `rpx-cli`（现在只有 print） | ✅ **已闭合**（2026-10-15）：`render_cmd`（子命令分派重构 + `--out` 扩展名判格式（不分大小写）/ `--data-dir` 缺省 = `.rpx` 同目录）+ `tests/cli_render.rs` **7 条真跑 CLI 二进制的断言**（`CARGO_BIN_EXE_rpx-cli`）：csv/xlsx 与库直调**逐字节一致**、默认目录三段（缺 CSV 失败 / 同目录成功 / flag 生效）、未知扩展名 exit 2、缺 `--out` usage、大写扩展名、print 回归；3 条变异实测（扩展名互换 / exit code / 默认目录，改坏必红） |
| M6-05 | 20 个既有引擎夹具各导出一份 CSV/xlsx 落 `tests/fixtures/out/` | 夹具资产 | 作为回归基线；任一导出改动导致读回 diff 即红（立项时 12 个，M6-01 时已增至 20——`engine_cross`/`engine_cross_root` 系 M5-04 新增） |

依赖序：M6-01 → M6-02 → M6-03/M6-04 并行 → M6-05 收口。

## 4. 与既有里程碑的边界

- **不动 core/engine 的零依赖**：导出是受审层新增依赖，`rpx-core`/`rpx-engine` 的
  `[dependencies]` 仍只许 path（G5 不变）。
- **不重算**：导出器吃已经 `calculate()` 完的 `CalcGrid`，不重跑引擎。
- **分页**：导出默认导出**整个计算矩阵**（不分页、不按纸切）；
  「按页导出一个 sheet/分 sheet」是 M6-05 之后的带账（§5.2）。

## 5. 边界与带账

### 5.1 新带账（本立项核出）

1. **Java 产品导出真值未探针**：CSV 空值/表头/数字格式、xlsx 样式（边框/字体/
   列宽/冻结窗格）均未拿产品真值。M6 先做**数据保真**（值+合并区），
   **样式保真**（边框/字体/对齐/行高列宽/冻结）整体带账——蓝图 §10 说
   「边框/字体/对齐逐格写」是目标，但样式来自模板属性（`props.rs` 那套），
   需要另起探针把模板样式位 → xlsx 样式的映射钉死，不在 M6-01..04。
2. **`calamine` 只作 dev-dep**：G5 目前不查 dev-dependencies（`code-review.md` §3.2
   注 3），这是已知缺口；若以后发现有人把生产依赖藏进 dev-deps，再扩 G5 范围。

### 5.2 后置（不在 M6）

- PDF / 打印（`rpx-print`，蓝图 §11）：版式保真，另立里程碑；
- 按纸分页导出 / 多 sheet 分页导出 / 重复冻结行；
- Word / HTML 出口；
- 样式保真（边框/字体/列宽/冻结窗格）；
- **CLI 的参数支持**（v0.5 补记）：`render` 目前只收无参模板（`ParamTable::EMPTY`），
  `@p` 模板报「未登记参数」。缺的不是管道（`calculate_with` 直接吃 `ParamTable`），
  而是**值推断口径**——HTTP 面用 JSON（`form.rs::json_to_value`），CLI 侧若做
  `--param k=v` 则 List/数组值不可表达、若做 `--params '<json>'` 又要引 serde_json
  且与 server 口径镜像（两处实现）。无基准前不拍板 ⇒ 整体后置。

### 5.3 继承带账

分页高级 / 跨页合并拆分 / 文本换行 / M2-22 残余 / M5-07 `cross()` 等
沿用 `spec-m3.md` §5 / `spec-m5.md` §6.2，本 spec 不重复。

## 6. 变更记录

- **v0.1（2026-10-15）**：立项。出口顺序 CSV→xlsx→server/CLI；输入定 `&CalcGrid`；
  CSV RFC 4180 自定格式契约 §2.3；xlsx calamine 读回自洽对拍 §2.4；
  G5 白名单登记 `rust_xlsxwriter` §2.5；任务表 M6-01..05 §3；新带账 2 项 §5.1。

- **v0.2（2026-10-15）**：**M6-01 闭合**。
  - 落地：`crates/rpx-export`（新 crate，workspace member；G8 矩阵登记
    `rpx-export -> rpx-engine` 一行——「新 crate 进仓必须登记，默认拒绝」）；
    `src/lib.rs`（三条边界 + 出口顺序）／`src/csv.rs`（`to_csv` + 转义，
    规则即 §2.3 逐条）／`src/text.rs`（值文本口径：数值 Rust shortest Display、
    Decimal 原样、Bool 小写、List 逗号连接、**非有限数保留 NaN/inf 文本**而非
    落空——比 server 快照的「落 null」更保守，有测试钉）。
  - 独立实现而不复用 server 的 `snapshot/value.rs::scalar_text`：G8 分层矩阵
    方向是 `server -> export`（快照要吃导出/映射），反向依赖会被 G8 拒绝。
  - 测试：10 条钉测全绿（满矩阵/零维/越界/disp 优先/null/五组转义/List 引号/
    数值/非有限数/读回自洽）；关键形态用**字面量**断言（`"a,,\r\n,5,"`），
    不用 `value_text` 自证。
  - 验证：`cargo test -p rpx-export` 10/10；G1–G8 全绿。
  - 契约补充（实现期钉死、本文 §2.3 未写到的两条）：① 末尾**不留**换行
    （join 语义）；② 越界格（row/col 超出 `rows`/`cols`）忽略不 panic。

- **v0.3（2026-10-15）**：**M6-02 闭合**。
  - 落地：`src/xlsx.rs`（`to_xlsx(&CalcGrid) -> Vec<u8>`，单 sheet、无样式）
    ＋ `src/xlsx/tests.rs`（16 钉测）。依赖：`rust_xlsxwriter`（解析为 **0.99.1**，
    受审层登记见 §2.5 补记）＋ dev-dep `calamine` **0.36.1**（读回自洽）。
  - **写值口径表**（模块文档，逐条有测试钉）：数值族 → number（f64，Excel
    数字的存储本体；大整数/超精度 Decimal 带损是格式物理边界）；`Decimal`
    解析失败/溢出 → 文本原样；**非有限数 → 文本 `NaN`/`inf`/`-inf`**（拦截
    写库会把 NaN/inf 落成 `NAN`/`INF` 全大写文本的默认行为，统一到 text.rs
    口径）；`Bool` → 原生布尔；`List` → 逗号连接文本；`disp_value` 优先。
  - **合并区锚点重建**（新问题、新解法）：`CalcGrid::merged` 是**逐槽**视图
    （每槽报锚点整段跨度），`merge_range` 要**每区一条锚点矩形** ⇒ 重建
    规则 = 「延续槽 = 存在另一条同跨度记录覆盖本槽」。锚点值：cells 里有该格
    则走统一取值口径（含 disp 优先），否则用 merged 记录带的锚点值。
  - **边界 5 条**（有测试钉）：Excel 硬上限裁剪（行 1,048,576 / 列 16,384）；
    超长文本截断到 32,767 **字符**（整字截断防破 UTF-8，写库判据同粒度）；
    越界格忽略；空网格仍产出合法 xlsx；同输入同字节（写库 zip 时间戳固定
    → M6-04 的逐字节断言依赖此条）。
  - **病态输入防御**（引擎不产出、测试钉不 panic）：重叠合并区保首见
    （否则写库报 `MergeRangeOverlaps`）。
  - 测试：16 条全绿 —— 类型全谱读回 / Decimal 回退 / 非有限数文本 /
    disp 优先 / List 文本 / 空文本 / 合并区值+延续槽空 / 逐槽重建去重 /
    锚点 disp / 病态重叠 / 超长截断 / 空网格 / 越界格 / 超上限裁剪 /
    合并区跨界裁剪 / 同输入同字节。
  - **变异实测 3 条**（改坏必红、还原必绿）：① disp 优先改走 value
    → `disp_value_wins` 红；② 延续槽判定改 `j == i` → `continuation` 红；
    ③ 非有限数不拦截 → 读回 `NAN`（全大写）≠ `NaN` 红。
  - 验证：`cargo test -p rpx-export` 26/26（csv 10 + xlsx 16）；
    `cargo clippy -p rpx-export --all-targets -- -D warnings` 干净；
    G1–G8 全绿。
  - 坑（记录在案）：写库 `ColNum` 是 **u16**（`RowNum` 才 u32），坐标换算
    必须按列类型转换（`pos()` 注释钉死）；clippy `approx_constant` 会把
    测试里的 `3.14` 当 π 近似（换精确二进制小数 `3.5`）。

- **v0.4（2026-10-15）**：**M6-03 闭合**。
  - 落地：`crates/rpx-server/src/export.rs`（`ExportFormat` 严格小写解析 /
    `export_request` 纯函数 + `export_request_ds` async 版 / `export_response`
    响应组装 / 文件名双形态）+ `src/export/tests.rs`（5 条单元钉测）+
    `tests/export_http.rs`（7 条真 socket 集成钉测）。
  - **共享段重构**：`http.rs` 的 `render_core` 拆出 `calculate_grid`
    （主链路 ②-⑨ 步，原 M3-06 口径注释原样搬入）——`/render` 与 `/export`
    用同一趟计算（导出不分页/不按纸切，但走同一 pager 守卫与页数覆盖，
    出口分家不产生口径分叉）。
  - **文件名口径**（HTTP 层新增契约）：`Content-Disposition` 双形态 —
    ASCII fallback（白名单外全 `_`，全空 → `export`）+ RFC 5987
    `filename*=UTF-8''<pct>`（attr-char 集外全编码 ⇒ 控制字符/引号天然
    无注入面，不需要单独 sanitize）。中文报表名「月度报表」→
    `filename="export.csv"; filename*=UTF-8''%E6%9C%88%E5%BA%A6%E6%8A%A5%E8%A1%A8.csv`。
  - **fmt 口径**：严格小写 `csv`/`xlsx`（`CSV` 也拒，不静默回落）；
    缺省 = `xlsx`（蓝图 §8.1 示例口径）；非法值 400 且错误消息列可选值。
  - **body 合法性证据**：与库函数直调**逐字节一致**（xlsx 载体合法性
    「zip 魔数 + calamine 读回」已在 M6-02 全量钉死，此处不重复构建；
    集成层另断言 xlsx 首 4 字节 = `PK\x03\x04`）。
  - **测试脚手架抽取**：新建 `tests/common/mod.rs`（夹具读取 / multipart
    拼装 / 真 socket HTTP 客户端，**响应体保留原始字节**——xlsx 二进制不得
    走 lossy 转换），`render_http.rs` 改为接线共用（体例同 engine 的
    `tests/common/loader.rs`）。
  - 测试：server 单测 25 + export_http 7 + render_http 11 + sql 4 全绿；
    **变异实测 3 条**：① `ascii_filename` 不消毒（直返原名）→ 注入测试红；
    ② `rfc5987_encode` 不编码 → utf8 头测试红；③ 缺省格式改 csv → 缺省
    测试红。clippy `-D warnings` 干净。
  - 验证：`cargo test -p rpx-server` 47 条全绿（25+7+11+4）；G1–G8 全绿。
  - 坑（记录在案）：`mod common;` 的共用脚手架在两个测试 crate 各编译一份，
    「各自只取所需」⇒ 未用项 dead_code 预警，`common/mod.rs` 顶部
    `#![allow(dead_code)]` + 理由注释（体例：engine loader）。
  - **对拍抓到的 M6-02 缺陷（已修）**：M6-03 的「socket × 库直调逐字节对拍」
    在全量门禁时红——定位到 `docProps/core.xml` 单条目差异：写库
    `DocProperties::new()` 默认取**当前 UTC 时刻**写入，**跨秒**两次调用
    字节即不同（M6-02 的 `same_input_same_bytes` 是同进程连算两次（<1ms），
    碰巧跨不过秒界 ⇒ 未抓到；**单包跑绿、全量跑红**）。
    修复：`to_xlsx` 里 `set_creation_datetime(&ExcelDateTime::from_ymd(1980,1,1))`
    钉死（对齐 zip 条目时间戳的 DOS 纪元）；xlsx.rs 模块文档 §确定性 5 更新。
    回归网：新增 `bytes_stable_across_second_boundary`（sleep 1.1s 再算第二遍；
    变异实测：去掉固定时间戳 → 必红）。

- **v0.5（2026-10-15）**：**M6-04 闭合**（审计遗留③闭合）。
  - 落地：`crates/rpx-cli/src/main.rs` 的 `render_cmd`（子命令分派从「扫全参数
    找 print」重构为标准「首参 = 子命令」；错误统一 `Result<(), String>`）
    + `tests/cli_render.rs`（7 条**真跑 CLI 二进制**的断言）。
  - **CLI 口径**：`render <file.rpx> --out <out.csv|out.xlsx> [--data-dir <dir>]`
    —— 格式由扩展名决定（**不区分大小写**，文件系统惯例）；未知扩展名/缺
    `--out`/未知选项 = 用法错误 **exit 2**，运行期错误（读模板/计算/写文件）
    = **exit 1**；成功打印 `exported: <path> (<n> bytes)`。
  - **数据集**：`--data-dir` 缺省 = `.rpx` 同目录（模板 `file_name` 是相对名）；
    缺失 → `MissingDataset` 显式报错；SQL 数据集 → `SqlDatasetNoConnection`
    （CLI 无连接通道，不静默跳过）。
  - **与 server `/export` 同构**：`calculate_with` + `paginate_with_setup`
    （守卫复核）→ `rpx-export`；导出字节不分页。参数支持整体后置（§5.2）。
  - 测试：8 条（单测 1 + 集成 7）；**对拍基准 = 测试侧不经 CLI 代码的
    同构链**（`rpx_core::open_bytes + load_datasets_full + calculate_with +
    paginate_with_setup + to_csv|to_xlsx`）——CLI 产物逐字节一致。
  - **变异实测 3 条**：① csv/xlsx 扩展名判定互换 → csv 对拍红；② 未知扩展名
    exit 2→1 → usage 测试红；③ 默认数据目录改 CWD → 默认目录测试红。
  - 验证：`cargo test -p rpx-cli` 8/8；clippy `-D warnings` 干净；G1–G8 全绿。
