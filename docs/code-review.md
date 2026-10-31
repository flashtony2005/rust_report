# code-review — 代码审查标准与流程

> 状态：v1（2026-10-13 立稿）
> 适用范围：`rust_report` 工作区全部 crate（`rpx-core` / `rpx-engine` / `rpx-cli`）
> 不管什么：**算法契约**归 `docs/m2-engine-design.md` / `docs/format-contract.md` /
> `docs/spec-m1.md` / `docs/spec-m2.md` 管；本文只接管"一次改动如何被接受"。

---

## 0. 一句话原则

**机器能判的，不许人来判；人只判"违反也不报错"的那一类。**

本仓的语义/契约质量极高（每条复刻规则都带 `file:line`、每个"按直觉写会挂"的点都有
tripwire 钉子），但机械层（格式、测试可复现、CI）**完全没有闸门**——这就是
"代码质量参差不齐"的真实来源：不是人不行，是**形而上者没有机器守**。

---

## 1. 基线体检（2026-10-13 实测，非估算）

| 门禁项 | 实测 | 结论 |
|---|---|---|
| 零第三方依赖 | 3 个 crate 全是 path 依赖，pure std | ✅ |
| `clippy --workspace --all-targets -- -D warnings` | 强制重跑（`touch` 后），**零告警** | ✅ |
| 每个 `.rs` ≤ 500 行 | 0 个超限，最长 483（`rpx-core/tests/golden.rs`） | ✅ 阈值 2026-10-13 由 300 上调，见 §3.1 |
| 全中文注释 + `//!` 模块文档 | 仅 `rpx-core/src/error.rs` 缺 `//!` | ✅ |
| `cargo test --workspace --no-fail-fast` | **280 passed / 3 failed** / 19 个测试二进制 | ❌ 无绿基线 |
| `cargo fmt --all --check` | **108 / 131 个文件不合规** | ❌ 无闸门 |
| CI | 无 `.github/` | ❌ 无闸门 |
| README / CONTRIBUTING / rustfmt.toml | 均无 | ❌ |

### 1.1 四个必须知道的既有缺陷（都是"审查标准"存在的理由）

**① `decimal_oracle` 的 3 条断言在新克隆上永久红。**
`crates/rpx-engine/tests/decimal_oracle.rs:44-49` 读
`tests/fixtures/_bdo/out_jre8.txt`，而 `_bdo` 曾被整条写进 `.gitignore`、oracle 也从未入库
（`git log --all -- '*out_jre8*'` 空 —— 它在 Windows 上生成后没被带过来）。
→ **判据本身没入库** ⇒ 任何"全绿才能合并"的规则现在都无法成立。

已做（2026-10-13）：`.gitignore` 收窄为 `/tests/fixtures/_bdo/*.class`；
新增 `scripts/gen_decimal_oracle.sh`（+ `scripts/_selftest_gen_oracle.sh` 故障注入自测 8/8 过）；
装了两个 JVM 并**真的生成了 oracle**，`decimal_oracle` 3/3 绿，**G3 已转绿**。

oracle 实际出处（如实记录，注释写在 `tests/decimal_oracle.rs` 模块文档里）：

| 文件 | 生成者 | 版本头 |
|---|---|---|
| `out_jre8.txt` | Azul Zulu OpenJDK 8.0.504（macOS aarch64） | `java.version=1.8.0_504` |
| `out_jdk21.txt` | Azul Zulu OpenJDK 21.0.12.1（macOS aarch64） | `java.version=21.0.12.1` |

⚠ **不是**产品自带的那个 JRE：仓库自带的 `common/jre` 是 **Windows** 版
（`java.exe` + `.dll`），macOS 上跑不了。这不影响判据有效性 ——
`BigDecimal.toString()` 是 JDK 规范行为，且 `the_two_jvm_oracles_agree_byte_for_byte`
这条断言**本身就是用来证明"换个 JVM 结果一样"的**。 brew 装不了 JDK
（brew 内部要再套一层 `sandbox-exec`，macOS 不允许嵌套沙箱），JDK 是直接下
Azul CDN 的 tarball 解到 `~/.workbuddy-ai/binaries/java/` 的。

⚠ **这两个 oracle 文件按用户决定「不落库」**（2026-10-13）。本机 `cargo test`
全绿靠它们，但**全新克隆上 `decimal_oracle` 仍会红**（3 条）——
复跑命令见 `tests/decimal_oracle.rs` 模块文档。当前 `.gitignore` 只忽略
`_bdo/*.class`，所以 `git add .` **会把这两个 .txt 一起带进索引**，
提交前务必 `git status` 看一眼。

**② `tests/fixtures/probe_out.txt` 是永久假脏。**
`.gitattributes` 写死 `* text=auto eol=lf` / `*.txt text eol=lf`，而该 blob 本身就是
CRLF 入库（Windows 上 Java probe 的原始输出，含 `.\data.csv` 与 GBK 中文报错）。
HEAD blob / index / 工作区三者 sha256 完全相同，git 却报 150 增 150 删。
→ **直接 `git add` 会真的提交 150 行换行符噪音。**

**③ 格式债 108 个文件。**
任何 diff 里约七成行是格式噪声 ⇒ 真实改动被埋 ⇒ 审查成本被无效放大。

---

## 2. 分层

| 层 | 谁执行 | 判什么 | 失败怎么办 |
|---|---|---|---|
| **Tier-0 机器闸** | CI / `scripts/gate.sh` | 格式、lint、测试、文件长度、依赖 | 不许开 PR |
| **Tier-1 作者自查** | 作者（PR 描述即 checklist） | 出处、夹具可区分性、故障注入记录 | 不许进审查 |
| **Tier-2 人审** | 审查者 | 语义、契约、tripwire 有效性 | 见 §6 |

---

## 3. Tier-0 机器闸（7 个 gate，合并前必须全绿）

| # | gate | 命令 | 当前状态 |
|---|---|---|---|
| G1 | 格式 | `cargo fmt --all --check` | ✅ 绿（2026-10-13 已全仓 fmt） |
| G2 | lint | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ 绿 |
| G3 | 测试 | `cargo test --workspace --no-fail-fast` | ✅ 283 绿（本机） |
| G4 | 文件长度 | ≤ 500 行（450 行起预警，见 §3.1） | ✅ 绿（最大 483） |
| G5 | 依赖策略 | 零依赖层只许 path + 受审层登记（见 §3.2） | ✅ 绿 |
| G6 | shell 陷阱 | `python3 scripts/scan_sh_trap.py scripts` | ✅ 绿 |
| G7 | 文档引用 | `python3 scripts/check_doc_refs.py`（见 §3.3） | ✅ 绿（125 个引用） |
| G8 | crate 分层 | `gate.sh` 内置 awk：path 依赖方向对照允许矩阵（core ← {engine, ds} ← {cli, server}，M3-03 定案 §3.4 / spec-m3 §3.1）；越界/未登记 crate 均 FAIL；**变异测试闭环**（cli 加依赖 rpx-ds 必红、撤后必绿） | ✅ 绿（2026-10-15 M3-07 立） |

`scripts/gate.sh` 已落地，一键跑 G1–G7 并按 gate 报 PASS/FAIL。
**2026-10-13 起 G1–G7 全绿**（G4 在文件长度阈值调到 500 之后转绿，见 §3.1；
G7 于同日立闸，见 §3.3）。

### 3.1 文件长度阈值：300 → 500（2026-10-13 定案）

**决定**：每 `.rs` 上限 **500 行**（原 300）。

**为什么不是 300**：本仓长期守住 300 行靠的是**手写高密度排版**（紧凑表格、长链单行）。
2026-10-13 全仓 `cargo fmt --all` 之后，rustfmt 默认 `max_width = 100` 的竖向展开
一次性把 **11 个** `.rs` 撑破 300。这是"格式化规则 vs 行数规则"的结构性冲突，不是偶发。
用户选择调阈值，而不是拆 10 个文件。

**这个决定的代价，写在这里以免以后忘**：

1. 500 **不是"从此可以写到 500"的许可**。它的初衷仍是"模块要小"；
2. 当前最大文件 `rpx-core/tests/golden.rs` = **483 行**，距上限只剩 17 行 ⇒
   headroom 极小，**下次再 fmt 或加内容就可能再顶格**；
3. gate.sh 因此在 **450 行** 处加了一条 `NEAR` 预警（不改变 PASS/FAIL），
   让 creep 在超标前先被喊一声。

**已修 1 个、并立了先例**：`expr/names.rs`（210→333）的 139 行新增**全是**炸开的
函数名表 ⇒ 加 `#[rustfmt::skip]` + 理由注释，两张表压回原样（现 216 行）。
⇒ 规则见 §5.9 的补充：**「密集字面量表」可以用 `#[rustfmt::skip]`，但必须写理由注释**。

⛔ **不许做的事**：把 G4 从 gate.sh 删掉、给超标文件加白名单、或只把阈值抬到
"刚好让当前文件过"。改阈值必须回到本文这一节写清理由，不许只改 gate.sh 里的数字。

**遗留**：下列 10 个文件仍在 300–483 之间，属于"当时决定不拆"，
若其中某个开始频繁改动，优先拆它而不是继续抬阈值：

| 文件 | 行数 | | 文件 | 行数 |
|---|---|---|---|---|
| `rpx-core/tests/golden.rs` | 483 | | `rpx-engine/src/stage_b.rs` | 309 |
| `rpx-engine/tests/m09_compare.rs` | 333 | | `rpx-core/src/cell.rs` | 307 |
| `rpx-engine/tests/common/loader.rs` | 324 | | `rpx-engine/src/expr/tests.rs` | 302 |
| `rpx-engine/tests/disp_assert.rs` | 323 | | `rpx-engine/src/calc/ctx.rs` | 302 |
| `rpx-engine/src/literal.rs` | 319 | | `rpx-core/src/report.rs` | 302 |

### 3.2 G5 依赖策略：从「全仓零依赖」到「分层白名单」（2026-10-13 定案）

**决定**：G5 不再要求「全仓零依赖」，改为**分层**——

| 层 | 成员 | 规则 |
|---|---|---|
| **零依赖层** | `rpx-core`、`rpx-engine` | `[dependencies]` **必须全是 path 依赖**，任何第三方依赖直接 FAIL |
| **受审层** | `rpx-cli` 及将来的 `rpx-ds`/`rpx-export`/`rpx-server`/`rpx-print` | 允许第三方依赖，但每个都必须在 `scripts/dep_allowlist.txt` 登记 |
| **死条目** | — | 白名单里登记了、代码里却没有的依赖，**也 FAIL** |

**为什么改（触发点）**：`docs/reportlite-rust全链路设计.html` 要求 M3+ 引入
libsql / sqlx / axum / rust_xlsxwriter / printpdf / nom / rust_decimal。旧 G5 写死
「只许 path」⇒ **M3 第一天就打红，M3–M6 一个都开不了工**。详见 `docs/design-review.md` §1。

**为什么不是删掉 G5**：`rpx-core` / `rpx-engine` 的零依赖是**有真实价值的不变式** ——
`.rpx` 解析与扩展算法是可审计、可对拍的核心资产，零依赖才守得住（无供应链面、
无版本漂移、构建最简）。删掉闸门等于把这条资产一起放弃。所以是**分层**，不是放开。

**代价与约束**（写在这里以免以后忘）：

1. 零依赖层**不许**为了省事把某个 crate 挪出去；挪出必须回到本节写理由。
2. 白名单每加一条，都要在下面的登记表里写「谁 + 为什么 + 替代方案为何不行」。
   **只改 `scripts/dep_allowlist.txt` 不写这张表 = 违规。**
3. G5 **只覆盖 `[dependencies]`**。`[dev-dependencies]` / `[build-dependencies]`
   不在闸门范围内 —— 这是**已知缺口**（dev/build 依赖不进产物）。若发现有人把
   生产依赖藏进 dev-deps 规避闸门，应扩大范围。
4. 闸门有效性已用**故障注入**证明（下表）。

**故障注入记录（2026-10-13，全部实测）**：

| 注入 | 期望 | 实得 |
|---|---|---|
| 基线 | 绿 | ✅ 绿 |
| `rpx-core` 加 `serde = "1.0"` | FAIL | ✅ `零依赖层违规 rpx-core: serde` |
| `rpx-cli` 加 `serde = "1.0"`（未登记） | FAIL | ✅ `未登记依赖 rpx-cli: serde` |
| 把 `rpx-cli serde` 登记进白名单 | 绿 | ✅ 绿 |
| 白名单留一条死条目 | FAIL | ✅ `白名单死条目` |

**受审依赖登记表**（M3 起逐条填写）：

| crate | 依赖 | 为什么需要 | 替代方案为何不行 |
|---|---|---|---|
| `rpx-server` | `serde_json` | 快照映射（M3-08 v1）：`CalcGrid` → Univer `IWorkbookData` 的 **JSON 文本**（契约 `docs/spec-m4.md` §4；`/render` 响应体同源） | ① 手写 JSON 序列化：转义 / 浮点 / 大整数边界全靠自证；且 M3-05 的 axum 栈**必然**引入 serde 生态，现在省下的依赖下一个任务就还回去；② 无 std 内建 JSON，替代只有自研 |
| `rpx-server` | `axum`（features = `multipart`） | M3-05 `POST /render` 主链路：HTTP 路由 + multipart 表单解析（`report`/`dataset`/`params` 三字段，`form.rs`）。蓝图 §8.1 路由表的既定选型 | ① 手写 HTTP/1.1 + multipart 编解码：multipart 边界/CRLF/Content-Disposition 解析是典型的"写起来简单、边界全是坑"，无基准可对拍；② `spec-m3.md` §3.1 已定 axum，另起炉灶等于违背架构文档 |
| `rpx-server` | `tokio`（features = `rt-multi-thread`/`macros`/`net`/`io-util`） | axum 0.8 的运行时底座：`#[tokio::main]` + `TcpListener`（`serve` 子命令）+ 集成测试的异步 socket 客户端（`tests/render_http.rs`） | axum 的 `serve`/`Router` 全部构建在 tokio 之上，没有"用 axum 但不引 tokio"的路径；std 无异步运行时，替代只有自研 executor（axum 不接受） |
| `rpx-ds` | `sqlx` 0.9（default-features=false，features = `any`/`postgres`/`mysql`/`sqlite`/`runtime-tokio`） | M3-03 取数主链路：`AnyPool` 按 URL 前缀选驱动（`postgres://`/`mysql://`/`sqlite://`，换库只换 URL——蓝图 §7.2 的既定选型）；sqlite 内存库冒烟 5/5 | ① 每库一个原生客户端（postgres/mysql/sqlite 三套协议）：三份连接池/类型映射/错误处理，维护面 ×3 且无对拍基准；② 自研协议：不可行；③ `libsql` 只覆盖 SQLite 一族，PG/MySQL 无解 |
| `rpx-ds` | `tokio`（features = `rt-multi-thread`/`macros`） | sqlx 0.9 的 runtime-tokio 底座 + `#[tokio::test]`（sqlite 冒烟） | 同 rpx-server 行：sqlx async 只跑在 tokio 上，std 无异步运行时 |
| `rpx-ds` | `calamine` 0.36 | Excel 工作表物化为类型化 RowSet，复用工作区已有读回验证依赖 | 自写 ZIP/OOXML、XLS 二进制解析与缓存公式处理复杂且易丢失类型；解析器留在数据源层，core/engine 保持零外部依赖 |
| `rpx-ds` | `csv` 1.4 | 显式分隔符、引号、转义和引号内换行的 CSV/TSV 解析 | 字符串 split 无法正确解析实际业务 CSV，手写状态机重复成熟解析器 |
| `rpx-export` | `rust_xlsxwriter` 0.99（默认 features） | M6-02 xlsx 写器：`CalcGrid` → xlsx 字节（`merge_range` + 类型化 `write_*`）；zip 容器 + OOXML 由库负责 | ① 手写 xlsx（zip + OOXML + `_xHHHH_` 控制字符转义 + 共享字符串表）是几百节点的格式工程，边界全靠自证且**无产品真值可对拍**（spec-m6 §0）；② `umya-spreadsheet` 偏读改场景、写从零场景更重；③ 零依赖层（rpx-core/rpx-engine）不能为它引——本 crate 是受审层，正是为这类出口依赖而分层 |

| `rpx-export` | `base64` 0.22 | 解码图形 data URI，复用已进入锁文件的成熟实现；实际图片验证仍交给 `rust_xlsxwriter::Image` | 自写 base64 解码会重复处理填充、非法字符和截断边界，不能降低整体复杂度 |
| `rpx-runtime` | `serde` 1（2026-10-22 加） | **流式序列化**（snapshot 页快照 / `result.json`）：50k 明细实测「整棵 `serde_json::Value` 树 + `to_string`」把峰值推到 **1,723 MB**（产物 34.6 MB 的 ~26 倍）。用 `serde::ser::Serialize` 手写 `SerializeMap`/`SerializeSeq`，把 `sheets`（逐页）、`cells`（逐格）改成边生成边写、写完即弃，格式仍由 `serde_json` 的 formatter 输出 ⇒ 与旧产物**逐字节一致**（钉子 `snapshot::stream_tests` + `result::tests::stream_matches_build`） | ① 手拼 JSON 标点/缩进：pretty 输出的缩进深度、浮点最短表示、字符串转义全要自证，正是「禁止手写字节」那条纪律要避免的；② 不加 serde 只能退回 `to_writer_pretty(&build())`：省掉 74.7 MB 的整串缓冲，但整棵 J 树照建，峰值不降 —— 治不了病根；③ `serde_json` 已依赖 `serde`，直接声明**不新增供应链面**，只把已有的传递依赖提升为直接依赖 |

前端新增 `echarts`：同一成熟库提供交互图、SVG 打印、自定义区间图和 JS 模板运行时。
服务端新增 dev-dependency `zip`：读取实际 XLSX 的图表 XML、图片与 drawing 条目，验证导出包含真正图形，不仅是单元格数据；不进入服务端产物。

> （dev-dep，不经 G5）`rpx-export` 的 `calamine`（读回校验）：xlsx 是 zip 容器、**字节级对拍无意义**（时间戳/条目序），M6-02 的验收是「读回自洽」——`to_xlsx` 产出 → `calamine` 读回 → 与 `CalcGrid` 逐格比值/类型 + 合并区全等。只进 `[dev-dependencies]`（不进产物，`code-review.md` §3.2 注 3 的已知缺口的**正当用法**，与「藏生产依赖」相对）；正文见 `docs/spec-m6.md` §2.4。

⛔ **不许做的事**：删掉 G5、把 `rpx-core`/`rpx-engine` 挪进受审层、
或往白名单里塞没写理由的条目。

### G3 的前置修复（二选一，**先定这个再谈流程**）

- **A（推荐，已选定）**：`out_jre8.txt` / `out_jdk21.txt` 入库；`.gitignore` 把
  `/tests/fixtures/_bdo` 收窄成 `/tests/fixtures/_bdo/*.class`（已改）。
  理由：oracle **是**判据本身，不入库等于判据不存在；而 `.class` 才是真·构建副产物。
  **落地方式**（禁止手写 oracle）：
  ```bash
  scripts/gen_decimal_oracle.sh \
    --javac <任一版本 javac> \
    --jre8  <产品自带 JRE 1.8 的 java> \
    --jdk21 <JDK 21 的 java>
  ```
  脚本会自检（≥25 条用例、5 段格式、两份版本头必须不同）并在任一不过时**两份一起删掉**，
  杜绝"半成品冒充基准"。
- **B（不改库）**：`decimal_oracle` 文件缺失时打印 skip 而非 panic，CI 里单开一个
  "有 oracle" 的 job。代价：**默认 `cargo test` 会长绿 3 条本来该管的断言**。

### G4 脚本

```bash
find crates -name '*.rs' -exec wc -l {} + | awk -v m=500 '$1>m && $2!="total" {print "OVER:",$1,$2; bad=1} END{exit bad}'
```

### G7 脚本

```bash
python3 scripts/check_doc_refs.py      # 单跑；从任何目录调用均可
```

### 3.3 G7 文档引用完整性：代码指向的 `*.md §N.M` 必须存在（2026-10-13 立闸）

**决定**：新增 G7 —— 凡形如 `` `<path>.md` §N.M `` 的引用，其编号必须在目标文档里
有一个同名标题（`#`..`####` 后跟该编号）；**指向不存在的文档同样 FAIL**。

**为什么需要它**（一天之内查出的三类同款静默失败）：

1. **8 处**代码头注释指向 `docs/m2-engine-design.md` **§5.4**，而 §5.4 **从未写出**
   （章节只到 §5.3）；
2. `merge.rs` 指向 `docs/java-reference-map.md` **§6.6**（不存在）；
3. `expr/names.rs` 指向 `docs/code-review.md` **§5.9**（当时不存在），另有 3 处指向 §5.12。

这类错误的性质：**编译过、测试绿，只有人真的去翻那一节时才发现**。它就是
`docs/completeness-audit.md` §3 问题 2 说的「文档与代码双向漂移」的**第 4 种形态**
（代码引用文档、文档没有）；根因是「G1–G6 里没有任何一条检查文档」。
⇒ 判据 `scripts/check_doc_refs.py` 已用**两次故障注入**证明有牙齿：
注入 `§9.9` → 红；注入 `docs/nope.md` → 红；回滚 → 绿（125 个引用全解析）。

**立闸时顺手把存量清成 0**：新写 §5.4 与 §6.6；把 `code-review.md` §5 的 12 条由
`**N.**` 粗体改成 `### 5.N` 标题，使 §5.9/§5.12 这类引用可被机器解析。

**判据的边界（诚实声明）**：G7 只认「路径与 `§` 紧邻」的引用（中间只允许空白/星号/
反引号）。`见 §5.4 的建模缺口` 这种**不带文档名**的裸引用**不在覆盖范围**——
无法判断它指哪份文档。故 G7 是"必要不充分"的闸。

⛔ **不许做的事**：为了让 G7 转绿而**删掉引用注释**（那是把判据删掉，不是修好）。
正确做法是补写被引用的那一节，或把引用改指到真实存在的节。

---

## 4. Tier-1 作者自查（PR 描述必填 4 项）

1. **改了哪个契约节 / 出处 `file:line`**——给不出行的，不许开 PR。
2. **新增/修改了哪个夹具，它和已有夹具的区别是什么**——不许同值（见 §5 规则 5）。
3. **故障注入记录**：故意改反了哪一行、哪条测试因此变红、改回来后又变绿。
4. **G1/G2/G3 三条的实际输出尾部**（贴原文，不许贴"通过"两个字）。

---

## 5. Tier-2 审查者判据（本项目特有 12 条）

### 5.1 复刻行为必须带 `file:line` 出处，且必须可复现
注释写"产品如此"但给不出行的 → Blocker。

### 5.2 docs 是契约，改代码必改 docs
`crates/rpx-engine/src/lib.rs` 顶部有"算法 → 文档"指针表。改了某模块行为而指针表 /
`docs/` 对应节没动 → Blocker。

### 5.3 禁「刚才还在」式 `expect`/`unwrap`
现成反例：`crates/rpx-engine/src/expr/dsfn.rs` 有 **9 处**
`ctx.ds(dsn).expect("上面刚取到")`。
规则：取值点与使用点相距 > 5 行，必须改成 `?` 透传；`Ok(...unwrap_row(i))` 同理。

### 5.4 每个新判据必须能被弄红
写完判据先做一次故障注入：改反实现 → 测试必须红 → 改回 → 必须绿。
**固定报 N 条噪声的检查等于没有检查。**

### 5.5 夹具必须能「区分」实现
反例在 `docs/spec-m2.md` 里有案可查：`engine_ext` 旧布局把 17/18/19 放同一行，
竖向扩展吞掉整条行带，三者 `sub_cells` 全为 1，**根本不可区分**；重排成"一行一模式"后才分开。
新夹具加完先自问：把某条规则改反，这条测试会红吗？不会 = 夹具白加。

### 5.6 「违反不会有测试失败」的项必须显式建清单
照 `docs/m2-engine-design.md` §5.3 的体例成节落盘，不许散落在个人注释里。

### 5.7 测试不得依赖 `.gitignore` 产物
现成违例：`decimal_oracle`（见 §1.1①）。

### 5.8 新增 `pub` 项必须同 Commit 进 `lib.rs` 的 `pub use` 与模块状态表

### 5.9 `.rs` 超 500 行必须拆文件，不许加 `#[allow(...)]` 蒙混
现有 3 处 allow（`ext_algo.rs:181`、`expr/resolve.rs:136`、`calc/ctx.rs:213`）
须逐条补"为什么必须 allow"的理由注释，否则清掉。
补充先例（`expr/names.rs`）：**密集字面量表**（函数名表、码值字典）可以
`#[rustfmt::skip]`，因为 rustfmt 会把一个名字拆一行、白白撑破行数上限，
而逐行读表毫无收益；但**必须在属性上方写清理由**，否则按 §11 一票否决。
ⓘ 阈值 300 → 500 的来龙去脉见 §3.1；**不许只改 gate.sh 里的数字而不过本文**。

### 5.10 测试代码一律走 `#[cfg(test)] mod`，不许进生产路径
`lib.rs` 里 `#[cfg(test)] mod stage_a_tests;` / `stage_b_tests` 是正确体例，照此办理。

### 5.11 审 diff 前先排除 CRLF 假象
看到整文件 100% 行改动（如 `probe_out.txt` 150/150），先跑
`git diff --ignore-cr-at-eol` 再判。**谁直接 `git add` 提交了它，记 Major。**

### 5.12 shell 脚本里 `$var` 后面严禁紧跟全角中文标点
UTF-8 locale 下 bash 会**把紧跟的全角标点吃进变量名**：

```bash
set -u; v=HELLO
echo "$v：文字"      # ← 变量名变成「v：文字」，unbound ⇒ 脚本被杀，报错还是乱码
echo "${v}：文字"    # ← 正确
```

判据：`\$[A-Za-z_][A-Za-z0-9_]*` 后紧跟非 ASCII 字符即为违规。
本仓已有 **G6 闸门**自动扫（不许手写扫描命令，也不许用 `grep -P` —— macOS 的 BSD grep
没有 `-P`，这条命令在 CI 上根本不存在）：

```bash
./scripts/gate.sh          # G6 自动扫 scripts/ 下全部 .sh
python3 scripts/scan_sh_trap.py scripts   # 单跑
```

**所有新增 `.sh` 必须过这一条**；`scripts/gen_decimal_oracle.sh` 与
`scripts/_selftest_gen_oracle.sh` 已全部改成 `${var}` 形式（G6 当前 PASS）。

---

## 6. 严重度与处置

| 级别 | 定义 | 处置 |
|---|---|---|
| **Blocker** | 契约破坏、判据失效、新引入 CI 永久红 | 必须改；不改不许合并 |
| **Major** | 期望值错、tripwire 可被绕、CRLF 误提交 | 必须改 |
| **Minor** | 可改进但不破坏契约 | 开 issue，可带 TODO 合并 |
| **Nit** | 措辞、注释 | 不阻塞 |

---

## 7. 流程与 SLA

```
作者：gate.sh 全绿 → 填 PR 4 项自查 → 请求审查
审查者：只看 §5 的 12 条（不重复机器已判的）→ 按 §6 定级
AI 助手：跑 gate、汇总输出、起草 §5.3 式清单、准备夹具复现命令
        ❌ 不代替人判断"这次期望值对不对"
SLA：Blocker 24h 内响应；Blocker 未清不得合并
月度：把当月 Blocker 回写进 §9 反模式清单（用实例，不用形容词）
```

**可以不审查的**：docs 纯笔误、由生成器确定性重产的夹具（须附"重产后逐字节相同"的证明）。
**必须双人看的**：任何动 `arena/`、`calc/`、`expr/` 公共契约的改动；任何动 `.gitattributes` 的改动。

---

## 8. 反模式清单（已出现实例，持续追加）

| 反模式 | 实例 | 防线 |
|---|---|---|
| 测试依赖未入库产物 | `decimal_oracle` → `_bdo/out_jre8.txt` | §5.7 / G3-A |
| 夹具不可区分 | `engine_ext` 旧布局 17/18/19 全 `sub_cells=1` | §5.5 |
| CRLF 假改动 | `probe_out.txt` 150/150 | §5.11 |
| 格式无闸门 | 108/131 文件不合规 | G1 |
| CI 缺失 | 无 `.github/` | §9 路线图 |
| "刚才还在"式 expect | `expr/dsfn.rs` 9 处 | §5.3 |
| **`$var` 后紧跟全角标点** | bash 把标点吃进变量名，2026-10-13 在自测脚本里连抓两次 | §5.12 |

---

## 9. 落地路线图（按价值排序）

| 步 | 动作 | 影响面 | 先决 |
|---|---|---|---|
| 3 | `out_jre8.txt` 入库 + 修 `.gitignore` | 1 文件 + 1 配置 | 定 A/B 方案 |
| 4 | 落 `rustfmt.toml` + `Cargo.toml` `[workspace.lints]` + `.github/workflows/ci.yml` | 3 新文件 | — |
| 5 | 跑一次 `cargo fmt --all` | 100+ 文件 | 步 4 |
| 6 | 本文档生效 + `lib.rs` 的 `# 门禁` 改为指向本文 | 1 处注释 | — |

步 1 / 步 3 会动大量文件，**须显式授权后执行**。

---

## 10. 团队能力提升（立闸门之外的另一半）

闸门兜底下限，上限靠练。按本仓特性设计的四周计划：

| 周 | 主题 | 动作 | 验收 |
|---|---|---|---|
| 1 | **读懂既有判据** | 每人挑一个 `tests/*.rs` 读出"它防的是什么、怎么防的" | 能说出该测试"改反哪行会红" |
| 2 | **故障注入实操** | 对任一模块故意注入 1 个 bug，看测试是否抓到；抓不到就补 | 每人产出 1 条被证明有效的判据 |
| 3 | **trace 回源头** | 从一条 `expected.json` 反查到 Java 侧 `file:line` | 溯源笔记进 `docs/` |
| 4 | **审查轮值** | 每人审 1 个真实 PR，按 §5 的 12 条出具 Blocker/Major/Minor/Nit | 审查意见被作者采纳或能说清为何不采纳 |

配套约定：
- 资深开发者**每周一次** code-reading（30–45 min，只读不改，读"这里为什么这么写"）；
- 所有"我以为是"一律写成"⚠ 待验证 + 验证方法"，禁止当结论用（照 §9.2 体例）；
- 新人也从"跑 gate + 修红"入门，不从"读全部 docs"入门。

### 10.1 本周就能开始的三件事（不需要任何人先"学会"）

1. **跑一次 `./scripts/gate.sh` 并把输出贴进群里** —— 让所有人看见"红在哪"，
   比读十页文档有用。当前应看到 G1/G3 红、G2/G4/G5/G6 绿。
2. **全组跑一次 G6**（`python3 scripts/scan_sh_trap.py scripts`），确认 0 命中；
   以后每个新增 `.sh` 都会由它自动扫。这是**零成本、零风险、立刻消掉一类
   静默崩脚本**的动作。
3. **每人认领一条 `tests/*.rs`，用一句话说清"它防什么、改反哪行会红"**，
   说不上来的那条判据就是待补的。

---

## 11. 一票否决

以下任一情况，PR 一律不予合并，无例外：

1. `scripts/gate.sh` 非全绿；
2. PR 描述缺 §4 的 4 项自查；
3. 新增 `#[allow(...)]` 而无理由注释；
4. 新增 pub 项未同步 `lib.rs`；
5. 未做故障注入的"新判据"。

## Agent CLI 运行层依赖（2026-10-17）

`rpx-runtime` 提取已有 JSON 定义、Univer 快照和 Excel/CSV/SQL 供数编排，依赖 core、engine、ds、export。
CLI 依赖 runtime；server 只调用其目录库定义保存与固定产物读取接口。core/engine 保持零第三方生产依赖。

| 依赖 | 使用方 | 理由 |
| --- | --- | --- |
| serde_json | runtime、CLI | 复用项目已有 JSON 表示，统一源码、协议、Schema 与运行记录，避免第二套转换模型 |
| clap | CLI | 成熟命令树、参数约束与 try_parse 诊断；删除旧手写解析器 |
| json-patch | runtime | 使用 RFC 6902 的完整操作和 test 条件，避免自写补丁实现 |
| sha2 | runtime | 源码修订与真实输入内容的 SHA-256；避免修改时间和自制摘要 |
| time | runtime | UTC RFC 3339 运行时间，避免自写日历/时区算法 |
| tokio | CLI | 与项目已有 SQL 供数器共享异步执行环境 |

模板库的跨进程锁使用标准库 File::lock。要求 Rust 1.89 或以上；依赖锁文件确定实际构建版本。
