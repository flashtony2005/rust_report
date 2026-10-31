# design-review — 全链路设计文档评审

| 项 | 值 |
|---|---|
| 评审对象 | `docs/reportlite-rust全链路设计.html`（772 行 / 13 节 / 落款 2026-10-12） |
| 评审日期 | 2026-10-13 |
| 评审基线 | `HEAD = edd368f`（M2 里程碑达成），工作区未提交改动 111 文件 |
| 性质 | **只出意见，不改任何规则**。G5、§9 路线图、阈值一律未动 |
| 严重度词表 | Blocker / Major / Minor / Nit（与 `code-review.md` §10 一致） |

---

## 0. 一句话结论

设计文档的**技术判断基本站得住**（Univer 只当渲染终端、出口必是扁平矩阵、分页语义引擎里本来就有——这三条都对），
但它有 **1 个 Blocker**：**它要求的依赖清单与本仓 G5 门禁互斥，M3 第一天就开不了工**；
另有 **2 个 Major**：文档写于 M1/M2 之前、且**不引用仓内任何文档**，导致它无法与现状同步（已经漂了）。

---

## 1. Blocker：设计文档的依赖清单与 G5 门禁互斥

### 事实

`scripts/gate.sh` 的 G5 原文：

```sh
# ---- G5 依赖纯净（[dependencies] 里只许出现 path 依赖）--------------------
```

判定方式是 awk 扫 `crates/*/Cargo.toml` 的 `[dependencies]` 段，**任何一条不含 `path =` 的依赖即 FAIL**。

而设计文档要求的依赖：

| 依赖 | 设计文档出处 | 用途 |
|---|---|---|
| `libsql` | §7.1（HTML:348-381） | 平台库（报表资产 / 参数 / 缓存） |
| `sqlx`（any + postgres/mysql/sqlite/mssql） | §7.2（HTML:383-391） | 业务库取数 |
| `axum` | §8（HTML:483-494） | HTTP 服务 |
| `rust_xlsxwriter` | §10（HTML:598） | xlsx 导出 |
| `printpdf` | §11.3（HTML:641-655） | PDF 打印 |
| `nom` | §12（HTML:743） | 表达式解析（文档自己也写了"或手写"） |
| `rust_decimal` | §12（HTML:746） | BigDecimal 精度 |

### 影响

**M3（`rpx-ds` + axum 主链路）第一步就会把 G5 打红。** 当前 G5 之所以是绿的，
唯一原因是 M3 还没开始——**绿的是"还没写"，不是"规则对"**。这条不解决，
`code-review.md` §11 第 1 条（"gate 非全绿不予合并"）会立刻变成拦路石。

### 建议（供决策，本评审不代改）

> ✅ **已定案（2026-10-13）**：用户批准了本节的建议。G5 已改为**分层白名单**——
> 零依赖层（`rpx-core` / `rpx-engine`）+ 受审白名单（`scripts/dep_allowlist.txt`）
> + 死条目 FAIL，并以 4 组故障注入证明有牙齿。
> 规则正文与代价见 `docs/code-review.md` **§3.2**。本节以下保留原文，作为决策依据。

把 G5 从"全仓零依赖"改成**分层白名单**，而不是删掉它：

```
rpx-core / rpx-engine   → 零依赖（path only），不可放宽
rpx-ds / rpx-export / rpx-server / rpx-print
                        → 允许依赖，但必须在 code-review.md §3 登记
                          「谁 + 为什么 + 替代方案为何不行」
```

理由：`rpx-core`/`rpx-engine` 的零依赖是有真实价值的不变式（.rpx 解析与扩展算法
是可审计、可对拍的核心资产，零依赖才守得住）；新 crate 的依赖是产品必需的。
**改 G5 时必须同时改 `code-review.md` §3 的 G5 一节并写明理由**（照 §3.1 阈值先例）。

---

## 2. Major：文档写于 M1/M2 之前，部分内容已过时

### 事实

文档落款 **2026-10-12**，早于 M1/M2 完成。具体漂移：

- §12 风险表把表达式引擎列为待做：「自研递归下降子集解析器（nom 或手写）」
  （HTML:743）。**实际已完成**：`crates/rpx-engine/src/expr/parser.rs:1` 写明是
  **手写 Pratt 解析器**，全仓 `grep -rnE 'use nom|nom::' crates/` **无命中** ⇒
  已按"手写"这一支落地，且零依赖。
- 但**函数求值只做了一部分**：`expr/dsfn.rs` 只实现 `select` / `sum` / `count`
  三条 DS 函数，其余走 `dsfn.rs:168` 的 `Err(EngineError::UnsupportedExpr { … })`。
  （未实现时**显式报错**，不是静默给空值——这点是对的。）

### 影响

风险表的"待做"与仓库实际状态不符。照它排期会重复劳动（重写一个已有的 parser）。

### 建议

文档加一节「现状对照」，逐条标注 已完成 / 部分完成 / 未开始，并指向 `file:line`。

---

## 3. Major：文档不引用仓内任何文档 ⇒ 无法保持同步

### 事实

在 772 行里搜 `docs/`、`m2-engine-design`、`format-contract`、`spec-m1`、`spec-m2`、
`code-review`：**0 命中**（完全没引用）。

而仓内已有：

| 文档 | 行数 | 与设计文档重叠的部分 |
|---|---|---|
| `docs/format-contract.md` | 1732 | 设计文档 §4（.rpx 解析器） |
| `docs/m2-engine-design.md` | 1648 | 设计文档 §5 / §6（模型与引擎算法） |
| `docs/spec-m1.md` | 379 | M1 验收 |
| `docs/spec-m2.md` | 319 | M2 验收 |

设计文档 §4/§5/§6 是这三份已有文档的**摘要重述**，但既不引用也不标明"以哪份为准"。

### 影响

**两份事实源**，必然漂移——第 2 节的过时就是它的第一次发作。

### 建议

设计文档定位收敛为「**跨 crate 的架构与选型决策**」（前端/数据源/服务端/导出/打印），
§4–§6 改为**指针**（"详见 `format-contract.md` §X"），不重述细节。

---

## 4. Minor：文档内部 6 处不一致

| # | 问题 | 证据 |
|---|---|---|
| 1 | **`rpx-print` 不在 crate 表里。** KPI 写「6 个 crate」，§3 表 6 行；但 §11.3/§11.5/§13 M5 反复使用 `rpx-print` ⇒ 实际 **7 个** | KPI:HTML:53；表:HTML:166-171；`rpx-print`:HTML:600,607,641,654,748,763 |
| 2 | **「八阶段」对不上 10 步。** §8.1 标题写「对应 Java 八阶段」，配图编号到 **⑩** | HTML:496 vs HTML:500-520 |
| 3 | **平台库两套 SQLite 栈。** §7.1 明确「裸 libSQL，不走 sqlx」，§8.2 标题却写「sqlx migration」；而 §7.2 业务库必须用 sqlx ⇒ 同一二进制内 libsql + sqlx-sqlite 两个 SQLite 实现 | HTML:348-350 vs HTML:545 |
| 4 | §9 样例代码用了 `UniverSheetsCorePresetZhCN`，但 import 里没有它（照抄即报未定义） | 使用:HTML:580；import:HTML:575 |
| 5 | §7.3 代码块 `fn async fn fetch(...)` 不是合法 Rust 语法（应为 `async fn`） | HTML:467 |
| 6 | §5 图里字段名 `main_cells`，代码里是 `main_of` | 图:HTML:251；代码:HTML:272 |

第 3 条不只是笔误：**两套 SQLite 是可以砍掉一个的**。§7.1 自己承认
"只跑本地模式时，libSQL 的增量价值主要是 async API"——若平台库也用 sqlx，
平台库与业务库共用一套驱动，依赖数从 2 降到 1。

---

## 5. 待验证（不许当结论）

**§11 整节的架构决策建立在一个商业事实上**：文档称 Univer Pro
「新购与 30 天试用均已暂停」「免费档 ≤3 页/次 + 激活水印」（HTML:613-614）。
这两条是 2026-10 的**外部商业状态**，本机离线无法核实，而且**会变**。

> ⚠ 待验证 + 验证方法：在真正开工 M5（`rpx-print`）之前，
> 重新核一次 `pro.univer.ai` 的公告与版本对比表。若 Pro 恢复供货且价格可接受，
> 「自建 PDF」与「买 Pro」需要重新做一次取舍。
> **不要把一条会变的商业状态当成不可逆的硬架构驱动。**

其余未核实项（文档声称、本评审未独立验证）：Univer 的 star 数 / v1.0.x 版本、
`printpdf` 与 `rust_xlsxwriter` 的成熟度、`sqlx AnyPool` 的 mssql 支持现状。

---

## 6. 建议的处置顺序

1. **先定 G5**（第 1 节）。这是唯一阻塞开工的项，且必须先改规则再写代码。
2. **设计文档加「现状对照」节 + 交叉引用**仓内四份文档（第 2、3 节）。
   这一步能防住后续所有漂移，成本最低。
3. **修 6 处内部不一致**（第 4 节），其中第 3 条顺带做一次"要不要两套 SQLite"的决策。
4. **M5 开工前重核 §11 的商业前提**（第 5 节）。

---

## 附录：复现命令

```sh
# 1. G5 的判定逻辑
sed -n '/G5/,/G6/p' scripts/gate.sh

# 2. 设计文档要求的依赖出处
grep -nE 'libsql|sqlx|axum|rust_xlsxwriter|printpdf|nom|rust_decimal' \
     docs/reportlite-rust全链路设计.html | head -30

# 3. rpx-print 是否在 crate 表里（注意必须用 -E）
grep -nE 'rpx-print' docs/reportlite-rust全链路设计.html
grep -nE 'rpx-ds|rpx-export|rpx-server' docs/reportlite-rust全链路设计.html | head

# 4. 表达式引擎是否用了 nom / 是否手写
grep -rnE 'use nom|nom::' crates/ ; head -20 crates/rpx-engine/src/expr/parser.rs

# 5. 文档是否引用仓内其他文档（0 命中 = 完全不引用）
grep -nE 'm2-engine-design|format-contract|spec-m1|spec-m2|code-review' \
     docs/reportlite-rust全链路设计.html
```

> ⚠ **第 3、5 条必须用 `grep -E`**。本机是 BSD grep，BRE 下 `\|` 被当字面量，
> `grep 'rpx-ds\|rpx-export'` 会**静默返回 0 行**（本评审过程中已实际踩到一次，
> 差点得出"crate 名不在文档里"的错误结论）。见 `code-review.md` §5.12。
