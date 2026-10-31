# completeness-audit — M1/M2 完成度审计与工程反省

| 项 | 值 |
|---|---|
| 审计对象 | M1（`rpx-core` 解析器 + 模型层）、M2（`rpx-engine` 扩展 + 计算 + 合并） |
| 审计日期 | 2026-10-13 |
| 审计基线 | `HEAD = edd368f`（提交信息：*M2 里程碑达成 — 引擎主干接通，12 引擎夹具逐格对拍全绿*） |
| 性质 | **只审计，不开发**。未改任何代码 |
| 依据 | `docs/spec-m1.md` v1.4（379 行）、`docs/spec-m2.md` v0.9（319 行）、`docs/format-contract.md`（1732 行）、`docs/m2-engine-design.md`（1648 行）+ 代码实测 |

---

## 0. 结论速览

| 问题 | 结论 |
|---|---|
| **设计到位吗？** | **到位，且质量罕见地高。** 但有三处结构性缺口：M2 规格至今是「草案待用户确认」、验收口径与实现形态脱钩、两份规格的验证步骤整章是 Windows 的（本机跑不了）。 |
| **代码完成度？** | **M1 完成；M2 的 P0 已全部落地**（M2-08 同值合并、M2-21 合并四趟于 **2026-10-13** 补齐），**剩 3 项 P1 未实现**（分页 / 函数面 / 属性表达式）。逐条状态见 `docs/spec-m2.md` §6.1。 |
| **最大的口径问题** | 「M2 里程碑达成」与 `rpx-engine/src/lib.rs:33-35` 的自述「**在它们落地前，本 crate 不构成一个可用的 M2**」直接冲突。 |
| **最值得肯定的一点** | 全仓 `todo!` / `unimplemented!` / `TODO` / `FIXME` **各 0 处**；不支持时一律显式报错（`UnsupportedExpr` 31 处）。**没有静默缺失。** |

---

## 1. 设计到位吗

### 1.1 到位的地方（这是本仓最强的资产）

1. **规格是源码级的，逐条带 `文件:行号`。** spec-m1 有 AC-01..AC-10，spec-m2 有 M2-01..M2-23，契约正文在 `format-contract.md` 附件 A–L（1732 行）。
2. **所有"按直觉写必挂且不报错"的结论都被写死在契约层**，不是留在实验记录里：

   | 反直觉结论 | 出处 |
   |---|---|
   | `sum(B2:B4)` = **100**，不是 450 —— 不是 Excel 区域聚合 | spec-m2 §8、附件 H.1 |
   | `$` 是**整格**前缀（`$B2` 可用），**不支持** Excel 的 `$B$2` | spec-m2 §8、附件 G.1 |
   | `extendMode=16` **不是**"不扩展"，运行时按拓扑改写成 **18** | spec-m2 §8、附件 H.4 |
   | `src_row`/`src_col` 是**模板态**坐标，⛔ 不得用于索引计算态网格 | spec-m2 §5、M2-23 |
   | `mergeSame` **不在** `Engine.calc()` 里，只在**分页后**可见 | spec-m2 §8 |
   | `PropQueue` 是**死代码**，不属于 M2 结构 | spec-m2 §8、附件 F.2 |
   | `lefthead`/`tophead` priority=**17**、`mcorder`=**16**，其余 136 个是 19 | spec-m2 §5.1.4 |
   | 类型**禁止**统一提升 f64（`Integer(100)` 必须产出 `int`） | 附件 H.3、M2-14 |

3. **规格自己记录自己的勘误**（spec-m1 v1.1–v1.4、spec-m2 v0.2–v0.9 变更记录），并严格区分「已核实（附证据）」与「⚠ 待验证（禁止当结论用）」。这个诚实度在真实项目里很少见。

### 1.2 设计上的三处真实缺口

**① M2 的规格从未被正式确认，却已作为验收基线在用。**

`docs/spec-m2.md:6` 状态行仍写：**「草案待用户确认」**。而代码已按它实现、提交已宣称里程碑达成。

> **2026-10-13 已处置**：状态行改为「已作为 M2 落地基线使用（未做正式评审确认）」，并新增
> §6.1 逐条落地状态表 —— 规格正文（规范）与落地事实（实测）现在分开陈述，不再互相冒充。

**② 验收标准的表述与可执行判据脱钩。**

M2 的验收写作「**CLI** 渲染 sample.rpx → **JSON/CSV**，与 Java 版导出对拍一致」，但实现形态完全不同（见 §2.3）。M1 是同一个病：`spec-m1.md:307` 自己标注「CLI 实跑（人工核对；⚠ 尚无自动化断言——遗留项③）」。

**③ 两份规格的「端到端验证步骤」整章已失效。**

`spec-m1.md:293-317` 与 `spec-m2.md:276-306` 全部是 Windows：`F:/project/report/...`、`common/jre/bin/java.exe`、MSVC `LIB` 环境变量、`D:/Tools/jdk-21.0.2`、`rpx-cli.exe`。本机是 macOS ⇒ **这两章现在一条都跑不了**，且没有任何标注说明它们已过时。

> **2026-10-13 已处置**：两节均已重写为 macOS 可跑（`spec-m1.md` §9 / `spec-m2.md` §9），
> 并显式写明能力边界与日常入口 `./scripts/gate.sh`。
> ⓘ **仍存同类残留（本轮未动，属证据/出处而非"验证步骤"）**：`docs/m2-engine-design.md` 的
> 探针编译/运行命令（`:1339-1346`、`:1383-1385`）仍是 Windows 路径；`docs/format-contract.md`
> 与 `docs/m2-engine-design.md` 顶部的 `F:/project/report/re_work/src/...` 是**反编译证据的出处**，
> 不是可执行步骤，保留。`spec-m1.md` §3 的 workspace 根 `F:/…` 与 §8 的 MSVC `LIB` 坑同理。

### 1.3 一个结构性观察

`spec-m2.md:10-15` 自己写了可信度声明：M2 规格**不是**架构师源码级核实后产出的（架构师 429 限流失败，总监接管）。而 M2 的 P0 里恰好塞满了「按直觉写必挂且不报错」的语义（`is_extend`、`src_row`、`calcProperties`）。

**但这条风险被补回来了** —— 补法很对：每个反直觉结论都配了一个**产品实测夹具**去钉（`engine_ext` 重排以区分 17/18/19、`engine_sum` 双数据集、`engine_cycle`、`engine_samemerge`）。所以 M2 规格的**可靠度最终不低**，只是路径比 M1 曲折。

---

## 2. 代码完成度

### 2.1 总览（实测）

| 项 | 值 |
|---|---|
| `.rs` 文件 | 131（core 14 / engine 86 / cli 1，其余为集成测试） |
| 源码行 | core 2,070 / engine 14,317 / cli 182 |
| `todo!` / `unimplemented!` / `TODO` / `FIXME` | **0 / 0 / 0 / 0** |
| 显式不支持出口 | `UnsupportedExpr` 31 处、`UnsupportedFormat` 7 处 |
| 测试 | **283 passed / 0 failed** |
| 门禁 | `scripts/gate.sh` G1–G6 全绿 |

### 2.2 M1（rpx-core）—— 完成

- AC-01..AC-10 均有实现与测试；独立验证方做过变异测试，**夹具变异 24/24 全杀**（含曾经存活的 M3）。
- `spec-m1.md:357-379` 的 5 项遗留**全部是「测试/夹具覆盖」而非功能缺陷**，已移交 M2 回归清单。判定：**M1 可以算完成**。
- 一处需注意：`crates/rpx-core/tests/golden.rs` = **483 行**，距 G4 上限（500）仅 17 行。

### 2.3 M2（rpx-engine）—— P0 已完成，剩 3 项 P1

> **2026-10-13 更新**：本节初稿判「两项 P0 未实现（M2-08 / M2-21）」。同日已把这两项落地
> （`crates/rpx-engine/src/merge.rs` + `merge/same.rs` + `merge/null.rs`），并把 `merged`
> 段接进 `emit::render_document`、把 `Dim::Merged` 摘出豁免篮。下面的表格与证据已按现状改写。

按 `spec-m2.md` 的 23 条 AC 逐条判定：

| 判定 | 条目 |
|---|---|
| ✅ 已实现（有对拍或单测） | M2-01 主格图 · M2-02 扩展算法 · M2-03 非扩展格 · M2-04 合并区扩展 · M2-05 算术 · M2-06 相对/绝对引用 · M2-07 循环引用 · **M2-08 同值合并** · M2-09 对拍链路 · M2-14 类型保真（**仅合成单测**） · M2-15 extendMode 改写 · M2-16 `is_extend` · M2-17 分类顺序 · M2-18 函数表 · M2-19 不实现位运算 · M2-20 求值调度 · **M2-21 合并四趟与守卫** · M2-23 源坐标语义 |
| ❌ 未实现（P1） | M2-11 分页 · M2-12 函数面 · M2-22 属性表达式 |
| ⚠ 实现有缺口 | M2-10 零依赖与门禁（阈值 300→500 已改，**规格未同步**） |

**M2-08 / M2-21 落地后的覆盖现状（诚实边界）：**

- `mergeSame`：**第 1 趟**（纵向正、mode 118、`_$6` 守卫）有夹具对拍 —— `engine_samemerge`。
- `mergeSame` **第 2/3/4 趟**（反向、横向）与**整个 `mergeNull`**：**无夹具**。
  全部夹具只设过 `mergeSame = 98`，没有 99/100、没有 mode 119/120、没有 `mergeNull`
  （唯一出处 `tests/fixtures/EngineModelsP0.java`）。第 3 趟还逐字复刻了
  `ExtCellSet.java:1968` 的残值行为，同样**无夹具可辨**（证据链见
  `docs/java-reference-map.md` §6.4 / §6.6）。
- `merged.page_count`：属分页器（M2-11），本 crate 恒发 1 ⇒ 三个多页夹具的页数**对不上**，
  已单列成 `Dim::MergedPageCount` 并**照常报差异**（仍豁免，但不静默）。
- 分页：**无 `pager.rs`**，**无 `paginate()`**（`lib.rs` 只导出 `calculate`）。
- 属性表达式 `calcProperties`：`calc/run.rs:16` 标注为「未建模（缺口 1）」。

**M2-18 函数表实测通过**（我独立数过）：`REGULAR` = **124**、`DS` = **15**，合计 **139**，与规格完全一致；两表交集恰为规格点名的 6 个（`avg`/`count`/`max`/`min`/`plot`/`sum`）。`priority` 三档例外也已落地。

**M2-19 位运算**实测通过：`lexer.rs:143-148` 把 `&`/`|` 词法化为 `Tok::Amp`/`Tok::Pipe`，注释明写「交给语法层报'不支持'」。

### 2.4 对拍的真实边界（这一节最关键）

唯一的**真对拍**是 `crates/rpx-engine/tests/engine_grid_compare.rs`：12 个夹具各跑一遍
「`.rpx` + `.csv` → `rpx_engine::calculate()` → `emit::render_calculated()`」，逐格比
值 / 类型 / `disp_value` / `source` 六子字段 / `sub_values` / `left_head` / `top_head` / 尺寸。

**它显式豁免两维，并且自己断言"被豁免的只能是这两维"**（`:84-90`、`:128-134`）：

```rust
fn split_exempt(r: &Report) -> (Vec<Diff>, Vec<Diff>) {
    r.diffs.iter().cloned()
        .partition(|d| !matches!(d.dim, Dim::Pagination | Dim::Merged))
}
```

而 `emit.rs` **根本不产出 `pagination` / `merged` 两个键** ⇒ **豁免是结构性的，不是偶发**。

> ⚠ **必须点破的推论**：`spec-m2.md:230` 明确写「`merged` 是 **`mergeSame` 的唯一可观测位置**」。
> 所以 M2-08 未实现 ⇒ 这条判据**当前没有任何测试在管**。两个 P0 正好都落在豁免篮子里。

### 2.5 三处「声称完成 vs 实际」的口径差

**① 提交信息 vs crate 自述。**

提交写「M2 里程碑达成 — 引擎主干接通」。而 `crates/rpx-engine/src/lib.rs:33-35` 自述：

> 尚未落地（`docs/spec-m2.md` §4.1 的其余模块）：顶层 `calculate()` 驱动…、附属格 `sub_cells` 落盘、
> 同值合并 `merge`、分页 `pager`。**在它们落地前，本 crate 不构成一个可用的 M2。**

**② 而 lib.rs 的这份"未落地"清单本身也已经过时**（双向漂移，不只是"文档落后于代码"）：

| lib.rs 声称未落地 | 实际 |
|---|---|
| 顶层 `calculate()` 驱动 | **已落地** —— `calc/run.rs` 完整实现（channel4/5/6 + chain 诊断），`engine_grid_compare.rs` 真跑通 |
| 附属格 `sub_cells` 落盘 | **已落地** —— `emit.rs:113`、`:137-145` 产出 `sub_values`，且 `compare/cells.rs:174` 真的在比 |
| 同值合并 `merge` | **已落地** —— `merge.rs` + `merge/same.rs` + `merge/null.rs`（2026-10-13） |
| 分页 `pager` | 确实未落地 ✅ 自述正确 |

> **2026-10-13 已修正**：上表所列的 `lib.rs` 漂移已按 §4 第 2 项处理 —— 清单现只剩
> `pager`（M2-11）+ `calcProperties`（M2-22）。ⓘ 本表是**审计当时的快照**，保留以记录这次双向漂移。

**③ M2 验收口径 vs 实现形态。**

| 验收标准（用户给的） | 实际 |
|---|---|
| 「**CLI** 渲染 sample.rpx」 | CLI 只有 `print` 一个子命令（`rpx-cli/src/main.rs:27`），**没有 render/json/csv** |
| 「→ **JSON**」 | 有，但在**库函数** `emit::render_calculated` 里，由**集成测试**调用，不经 CLI |
| 「→ **CSV**」 | **完全不存在** —— 仓里 CSV 只作为**数据集输入**，没有任何 CSV 输出代码 |
| 「与 Java 版导出对拍一致」 | 确实逐格对拍，但只在测试内，且豁免 pagination/merged |

### 2.6 已完成部分的质量证据（值得肯定）

- **反证纪律到位**：`engine_grid_compare.rs:159-187` 有正/负控制 —— 先断言未变异文本干净，再断言变异必被报出并精确定位到 `(2,1)`；`:195-229` 还钉住「空 `cells` 必须报**恰好 36** 条 Missing」，用来证明对拍器**没有**退化成按 72 个位置逐点比。
- **变异纪律写进了测试文档**：`subcell_assert.rs:43-51` 明确写「批量变异得到的'哪几个测试红了'不可信，必须单因、逐个跑」，并记录了上一轮被掩盖的真实事故。
- **"未消费的能力"被显式标出**：`expr/names.rs:110-149` 给 `FUN_PRIORITY_*` 加了 ⛔ 横幅，说明它「当前无人消费」、变异 M6 存活是**被接受**的，并写明接线时必须同时做的三件事。这正是"不许留'看起来已接线'的假象"。

### 2.7 仓库自己已经诚实记录的风险（不是我的发现）

| 风险 | 出处 |
|---|---|
| **M2-14 类型保真无真实数据覆盖**：夹具类型面只有 `string`/`int`/`list`，`double`/`float`/`long`/`decimal`/`boolean` 零覆盖 ⇒ **只能声明"接口就位"，不得声明"已验证"** | `spec-m2.md:257` |
| **`cell_type` 短路无夹具**：图形/子报表/条形码被排除出扩展集合，12 个夹具一个都没有 ⇒ **漏掉它没有任何测试会失败** | `spec-m2.md:255` |
| **二维合并对角槽**（`rm>1` 且 `cm>1`）无夹具实证，按纵向规则处理 | `stage_b/merge_alias.rs:29` |
| **横向扩展锚点的行号机制未核实**，只有单点拟合，断言层只钉"同属一行 + 列走向" | `subcell_assert.rs:32-38` |
| **按分组键动态生成分组头**未验证 | `spec-m2.md:262` |

---

## 3. 反省：三个真正的问题

### 问题 1 —— 里程碑的「达成」没有可判定的定义

M2 宣称达成，但它的两个 P0（M2-08 / M2-21）**正好落在对拍豁免篮子里**，同时 crate 自述「不构成一个可用的 M2」。
根因：**豁免机制有"不许静默"的纪律，却没有配套的"豁免清单必须清零"的退出条件。**

> 建议：把「豁免清单」升级为里程碑的**显式待办** —— 豁免非空即不算达成；至少在里程碑记录里写明「达成 = 除 X/Y 外的全部」。

### 问题 2 —— 文档与代码是**双向**漂移的

不是「文档落后于代码」这么简单，**四种形态**同时存在：

- 文档说没做、**代码做了** → lib.rs 的 `calculate()` / `sub_cells`
- 文档说做了、**代码没做** → spec-m2 的「状态：已确认」、M2-10 的「≤300 行」
- **两边都以为对方知道** → 验收口径写 CLI+CSV，实现是库函数 + 测试
- **代码引用文档、文档没有**（最隐蔽）→ **8 处**代码头注释指向
  `docs/m2-engine-design.md` **§5.4**，而 §5.4 **从未写出**（章节只到 §5.3）。
  引用点：`calc/ctx.rs:3`、`calc/run.rs:3`、`calc/cell_eval.rs:4`、`expr/eval.rs:94`、
  `expr/dsfn.rs:121`、`calc/cell_eval/extend.rs:22/29`、`stage_a.rs:184`、
  `stage_b/merge_alias.rs:27`。**2026-10-13 已补写 §5.4**（接线设计 + 三条已声明缺口：
  DS 值归属旁路 / `_$14<=4` 幂等偏离 / 二维合并对角槽）。

根因：**G1–G6 里没有任何一条检查文档**。文档一致性完全靠人记得。

> **2026-10-13 已补 G7**：新增 **文档引用完整性** 闸（`scripts/check_doc_refs.py`，
> 规则见 `docs/code-review.md` §3.3，已两次故障注入：注入 `§9.9` → 红、注入
> `docs/nope.md` → 红、回滚 → 绿）。它只覆盖**第 4 种形态**（代码/文档指向 `*.md §N.M`）；
> 其余三种（说没做 / 说做了 / 两边都不知道）**仍靠人** ⇒ G7 是**必要不充分**的闸。

### 问题 3 —— 验收标准的表述与可执行判据脱钩

「CLI 渲染 → JSON/CSV 与 Java 对拍一致」在实现时被拆成「库函数 + 集成测试」，但**规格没有回写**。此后任何人照规格验收，都会得出"没做"的结论。
这与 M1 遗留③（AC-07 CLI 无自动化断言）是**同一个病**：**凡验收标准里写了"CLI X"，就必须有一条真的跑 CLI 的自动化断言。**

---

## 4. 建议的处置顺序（本审计不含任何开发动作）

> **执行进度（2026-10-13）**：第 1–4 项**已完成**，第 5 项**已拍板**（补实现）。逐项落点：

1. ✅ **先修口径**：`docs/spec-m2.md` 状态行已改为「已作为 M2 落地基线使用（未做正式评审确认）」，
   并新增 **§6.1 落地状态**（M2-01..M2-23 逐条标 ✅/⚠/❌ + 证据或缺口）。
2. ✅ **同步 `lib.rs` 的"未落地"清单**：现只剩 `pager`（M2-11）+ `calcProperties`（M2-22）；
   旧版多列的 `calculate()` 驱动 / `sub_cells` 落盘 / `merge` 三项均已删去（都落地了）。
3. ✅ **两份规格的 Windows 验证步骤改 macOS**：`spec-m1.md` §9、`spec-m2.md` §9 已重写为 macOS 可跑，
   并写明能力边界（产品 JAR 在 Windows ⇒ 本机不能重跑夹具生成器；日常入口是 `./scripts/gate.sh`）。
   ⚠ 原建议的落点「`docs/code-review.md` §9 的 macOS 路径」**是错的** —— §9 是"落地路线图"，
   不含 macOS 路径；实际入口是 `scripts/gate.sh`（见 `docs/code-review.md` §3）。
4. ✅ **同步 M2-10 的行数阈值**：`spec-m2.md` M2-10 已改为 ≤500（附理由出处）；
   `spec-m1.md` §8 的同名行一并改，并在该节留下「其余 `≤300` 是 M1 交付时事实、保留不改」的说明。
5. ✅ **已拍板：补实现**（不是降级为 M3）—— M2-08/M2-21 于 2026-10-13 落地：
   `merge.rs` + `merge/same.rs` + `merge/null.rs`；`emit` 补 `merged` 段；对拍豁免收窄为
   `pagination` + `merged.page_count`（两者都是分页器 M2-11 的产物）。

> **仍未闭合的一项（本轮未动）**：§2.5③ 的「验收口径写 CLI+JSON/CSV，实现是库函数 + 集成测试」。
> §3 问题 3 给的判据是"凡验收标准里写了 `CLI X`，就必须有一条真的跑 CLI 的自动化断言"——
> 这需要**新增代码**（CLI 子命令或断言），不在"修口径"范围内，故留作待办。已记入 `spec-m2.md` §9。

> **✅ 已闭合（2026-10-15，M6-04）**：上项所需的新增代码已落地 —— `rpx-cli render
> <file.rpx> --out <out.csv|out.xlsx>` 子命令 + `crates/rpx-cli/tests/cli_render.rs`
> 7 条**真跑 CLI 二进制**的断言（`CARGO_BIN_EXE_rpx-cli`），含 `print_still_works`
> 回归（M1 遗留③同为闭）；产出与库函数直调逐字节一致。契约与验收见
> `docs/spec-m6.md` §3（M6-04）/ §6（v0.5），复盘见 `tests/fixtures/RUNLOG.md` §45。

### 状态补记（2026-10-14）

本审计主体与附录命令是 2026-10-13 快照，保留其历史口径。此后 M2-11 分页已落地；M2-22 的 RowCell/ColCell/NormalCell 内置 `expMap` 属性计算第一阶段也已接入，`engine_rowheight` 的计算行高与分页逐项全段对拍，分页豁免删除；M2-12 的 `Text-Text` 字典序也已于 2026-10-14 落地（移植产品 `ChineseComparator`，见 `docs/m2-collation.md`），`M2-14` 凭 `engine_types` 产品夹具升级 ✅。M2 仍未整体闭合：自定义属性表达式/ExtCellSet-cellGraph 对象路径及 state 8 属性队列、M2-12 的 `A:B` List 逐格分支仍待处理。详见 `docs/spec-m2.md` §6.2 与 `docs/m2-engine-design.md` §5.4.7。

---

## 附录：复现命令

```sh
# 1. 零占位符 / 不支持出口统计
for p in 'todo!' 'unimplemented!' 'TODO' 'FIXME'; do
  printf '%-16s %s\n' "$p" "$(grep -rnF "$p" crates/ --include='*.rs' | wc -l | tr -d ' ')"
done
grep -rhoE 'Unsupported[A-Za-z]*' crates/ --include='*.rs' | sort | uniq -c

# 2. mergeSame / mergeNull / pager 的落地状态（2026-10-13 更新）
grep -rnE 'mergeSame|mergeNull' crates/rpx-engine/src/merge/  # 已落地：merge/same.rs + merge/null.rs
find crates/rpx-engine/src -name 'pager*'                     # 无 —— M2-11 未做
grep -rn 'paginate' crates/                                   # 无 —— M2-11 未做

# 3. 对拍豁免了哪两维、以及 emit 是否产出这两维（2026-10-13 更新）
grep -nE 'split_exempt' -A4 crates/rpx-engine/tests/engine_grid_compare.rs  # 现豁免 pagination + merged.page_count
grep -rnE '"pagination"|"merged"' crates/rpx-engine/src/emit.rs
#    现在 emit **产出 `merged` 段**（render_merged）；`pagination` 仍不产出（M2-11 未做）。
#    ⓘ 审计当时此处实跑只得 1 行注释 ⇒ 结论是"两个键都不产出"。该结论已随 M2-08/M2-21 落地而改变。

# 4. 函数表规模（应得 124 + 15 = 139）
awk '/pub const REGULAR: &\[&str\] = &\[/,/^\];/' crates/rpx-engine/src/expr/names.rs \
  | grep -oE '"[a-z0-9_]+"' | wc -l
awk '/pub const DS: &\[&str\] = &\[/,/^\];/' crates/rpx-engine/src/expr/names.rs \
  | grep -oE '"[a-z0-9_]+"' | wc -l

# 5. CLI 到底有几个子命令
grep -nE '"(print|render|json|csv|export)"' crates/rpx-cli/src/main.rs
```

> ⚠ 全部命令用 `grep -E` / `-F`。本机是 BSD grep，BRE 下 `\|` 被当字面量，会**静默返回 0 行**
> （见 `docs/code-review.md` §5.12）。
