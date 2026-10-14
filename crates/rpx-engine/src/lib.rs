//! rpx-engine：ReportLite 计算态/分页态引擎（纯 std，零第三方依赖）。
//!
//! # 这一层做什么
//!
//! M1 的 `rpx-core` 只把 `.rpx` 解到**模板态**（`ReportDefine`：矩阵 + 行高列宽 +
//! 单元格属性）。M2 把它推进到**计算态**：按扩展方式放大矩阵、按表达式求值、
//! 按同值规则合并、并按行高分页，产出可直接渲染的扁平矩阵。
//!
//! # 对拍纪律
//!
//! 计算态的**唯一事实源**是产品引擎自己跑出来的 `tests/fixtures/engine_*.expected.json`
//! （12 个引擎夹具，`GenEngineFixtures --verify .` 记 `VERIFY OK: 12 engine fixtures`）。
//! 禁止手搓期望值：手写出来的只会是"我以为的语义"，而不是基准。
//!
//! # 当前进度（诚实声明）
//!
//! 已落地（有源码级证据 + 单测）：
//!
//! | 模块 | 对应 | 状态 |
//! |------|------|------|
//! | [`error`] | M2-07 循环引用的对外错误形状 | ✅ |
//! | [`value`] | M2-14 类型保真（`int`/`long`/`double`/`float`/`decimal`/`string`/`boolean`/`list`） | ✅ |
//! | [`literal`] | 模板值 → 计算态值（`ObjValue`→[`Value`]）+ `BigDecimal.toString` 复刻 | ✅ |
//! | [`arena`] | 计算态的**两张矩阵**（引擎矩阵只读 + 计算矩阵可变）与来源引用（`source` 是可选键 → 一律 `Option`） | ✅ |
//! | [`main_cell`] | M2-01 主格判定（`MainCell.java:21-39` 忠实移植） | ✅ |
//! | [`extend`] | M2-15 `ExtendMode` + 运行时改写 + 扩展行数公式 | ✅ |
//! | [`expr`] | 表达式切片：词法 + Pratt 语法分析 + `is_extend()` 谓词（**不含求值**） | ✅ |
//! | [`heads`] | 第 5/6 步 head 图：命中即返查找（`find.rs`）+ 双索引网格（`grid.rs`） | ✅ |
//! | [`eval`] | 第 7 步求值调度：四通道 trampoline（附件 K.1 三条通道 + 统一入口） | ✅ 已接入顶层 `calculate()` |
//! | [`ext_algo`] | 第 8 步扩展算法：包络 / 部分平移插行列 / expand*Head / extendRow·Col 两趟（附件 I） | ✅ 12 夹具全网格对拍 |
//! | [`stage_b`] | 阶段 B 驱动：role 判定 → head 赋值（含登记）→ 拓扑改写 → 归一化（§4）→ 字面量取值 | ✅ 引擎夹具对拍（改写全量 + 未移动区 head + `engine_types` 全量值） |
//! | [`calc`] | 顶层 `calculate()` 驱动：`ExtCellSet._$2:1483-1543` 逐行移植 + 导出（channel4/5/6 + chain 诊断） | ✅ 12 引擎夹具逐格对拍 |
//! | [`merge`] | M2-08/M2-21 同值合并 `mergeSame` + 空格合并 `mergeNull`（各四趟 + 两道互斥守卫） | ⚠ 见下「合并的覆盖缺口」 |
//! | [`pager`] | M2-11 分页：`ReportUtils2.getPagerInfo` + `SimplePager.createPages`（纵向按行高断页 + 分组头每页重复 + 横向按列宽断页） | ✅ 三个多页夹具的切点全对；见下「分页的覆盖缺口」 |
//!
//! ## 合并的覆盖缺口（诚实声明）
//!
//! `mergeSame` 的 P3/P4（横向 99/100）与**整个 `mergeNull`** 在夹具上**无可观测面**：
//! 12 个夹具里 `EngineModelsP0.java` 只设过 `mergeSame = 98`（纵向），既没设 99/100、
//! 也没设 `mergeSameMode` 的 119/120，更没设过 `mergeNull`。已落地的对拍证据只有
//! `engine_samemerge` 的 P1（模式 118、`_$6` 守卫）与 `engine_merge`/`engine_ext`
//! 的**模板合并区**（不走 `mergeSame`）。P3 还逐字复刻了 `ExtCellSet.java:1968`
//! 的残值行为（见 [`merge`] 模块文档）—— 它同样无夹具可辨，**照抄**是刻意的。
//!
//! ## 分页的覆盖缺口（诚实声明）
//!
//! [`pager`] 的纵向切点由计算态行高决定；M2-22 的内置 RowCell key 1 表达式已在分页前
//! 求值。`engine_rowheight` 的 17 项行高 `[10,40×12,50,27,10,10]` 与产品夹具逐值一致，
//! 分页 `[8,9]` 也已全段对拍。行列不可见与 `breakPage` 已接入分页输入；
//! `pagerStyle != 2`、跨页合并区拆分、文本换行回填行高仍未实现，边界见 [`pager`]。
//!
//! M2-22 的剩余范围是 Java 独立 `customPropEngMap`、ExtCellSet/cell-graph 属性对象及 state 8
//! 属性队列，这些目前没有本地模型或夹具证据；故**M2 整体仍未闭合**。
//! ⓘ 2026-10-13 更正：旧版此处把「顶层 `calculate()` 驱动」「附属格 `sub_cells` 落盘」
//! 「同值合并 `merge`」也列为未落地 —— 三项**均已落地**（这次双向漂移记在
//! `docs/completeness-audit.md` §2.5②）。
//! ⓘ 同日再更正：**分页 `pager`（M2-11）也已落地**（旧版把它与 M2-22 并列列为未落地）。
//!
//! 按 `docs/m2-engine-design.md` §12.1 的 11 步执行顺序：第 1 步（两张矩阵，
//! [`arena`]）、第 2 步（阶段 A 建格，[`stage_a`]）、第 3 步（`is_extend()`，
//! [`expr`]）、第 5/6 步（head 图，[`heads`]）、第 7 步（求值调度，[`eval`]）、
//! 第 8 步（扩展算法，[`ext_algo`]）已落地；阶段 B 的组装（[`stage_b`]）与顶层
//! `calculate()`（[`calc`]）也已接齐，12 个引擎夹具逐格对拍全绿。
//!
//! # 门禁
//!
//! **本 crate 零第三方依赖** —— `[dependencies]` 只许 path 依赖。这是 G5 的
//! 「零依赖层」，`rpx-core` 与 `rpx-engine` 同守，**不许为省事挪出去**；
//! 其余 crate 走「受审白名单」（`scripts/dep_allowlist.txt` + 登记理由）。
//! 另有：每个 `.rs` ≤500 行；`clippy --all-targets -- -D warnings` 零告警；
//! **全中文注释**。
//!
//! 以上由 `scripts/gate.sh` 的 G2 / G4 / G5 自动守（另有 G1 格式、G3 测试、
//! G6 shell 陷阱、G7 文档引用完整性）。G5 的分层规则与故障注入记录见
//! `docs/code-review.md` §3.2，G7 见 §3.3。完整标准与流程见同一份文档。
//!
//! ⓘ 文件长度上限 2026-10-13 由 300 上调到 500：本仓原先靠手写高密度排版守住 300 行，
//! 全仓 `cargo fmt --all` 后 rustfmt 的竖向展开一次性把 11 个文件撑过线。
//! 500 不是"可以写到 500"的许可；理由与代价写在 `docs/code-review.md` §3.1，
//! **改阈值必须回去改那一节，不许只改 gate.sh 里的数字**。
//!
//! # 算法文档（本项目约定：算法写进 md，代码只留指针）
//!
//! | 算法 | 正文位置 |
//! |------|----------|
//! | 三阶段总览 / 阶段 A 模板→计算矩阵 | `docs/m2-engine-design.md` §2、§3 |
//! | head 图赋值顺序 | `docs/m2-engine-design.md` §4.2、§4.4 |
//! | `extensible` 的两条写入路径（[`extend`]） | `docs/m2-engine-design.md` §4.3 |
//! | 求值调度状态机 | `docs/m2-engine-design.md` §5 |
//! | **DS current 生命周期**（`ds.current` 何时变 `Row`、何时留 `Root`） | `docs/m2-engine-design.md` **§5.2** |
//! | `extendRow` / `extendCol` / 合并区×扩展 | `docs/format-contract.md` **附件 I**（算法正文，逐行）、`docs/m2-engine-design.md` §6 |
//! | 两张矩阵与 `source` 契约（[`arena`]） | `docs/m2-engine-design.md` §0.1、§1 |
//! | 表达式切片：token/优先级/AST/`is_extend` 真值表（[`expr`]） | `docs/m2-engine-design.md` **§13** |
//! | `sub_cells` 语义 | `docs/m2-engine-design.md` §7 |
//! | **同值合并 `mergeSame` 四趟 + `_$5`/`_$6` 守卫 / `mergeNull` 四趟** | `docs/format-contract.md` **附件 K.2、K.3**；方向码与 `:1968` 残值见 [`merge`] 模块文档 |
//! | **分页：纸张口径 / 断页判据 / 分组头重复 / 横断** | [`pager`] 模块文档（含逐行出处）；设计说明见 `docs/m2-engine-design.md` **§15** |
//! | 扩展方式改写与行数口径（[`extend`]） | `docs/format-contract.md` 附件 H.4、H.7.1 |
//! | 求值语义 / 聚合 / 类型保真 | `docs/format-contract.md` 附件 H.1、H.2、H.3 |
//! | **`Text-Text` 字典序 `ChineseComparator` 移植**（[`collation`]） | `docs/m2-collation.md` |
//! | 阶段 B 三段顺序（role → head → 改写 → 归一） | `docs/m2-engine-design.md` §4 |
//! | 字面量取值 / `BigDecimal.toString`（[`literal`]） | `docs/m2-engine-design.md` §4、`docs/format-contract.md` 附件 H.3 |
//! | `$` 语义 / 循环引用检测 | `docs/format-contract.md` 附件 G.1、G.3、H.8 |

#![forbid(unsafe_code)]

pub mod arena;
pub mod calc;
pub mod collation;
pub mod dataset;
pub mod emit;
pub mod error;
pub mod eval;
pub mod expr;
pub mod ext_algo;
pub mod extend;
pub mod heads;
pub mod literal;
pub mod main_cell;
pub mod merge;
pub mod pager;
pub mod params;
pub mod stage_a;
pub mod stage_b;
pub mod value;

pub use arena::{
    CalcCell, CalcGrid, CalcMatrix, CellId, EngineMatrix, MergedCell, SourceRef, SubValue,
};
pub use calc::{calculate, calculate_with};
pub use calc::{DsInstance, HeadPairTracker};
pub use error::{EngineError, Result};
pub use eval::{run_channels, state as eval_state, Flow, SchedGrid, Suspend};
pub use expr::{is_extend, parse, parse_with, Expr};
pub use ext_algo::{extend_col, extend_row};
pub use extend::{
    calc_row_count, cell_role, is_extend_excluded_type, topology_default, CellRole, ExtendMode,
    EXTEND_EXCLUDED_CELL_TYPES,
};
pub use heads::{find_left_head, find_top_head, normalize_heads, HeadGrid};
pub use literal::{decimal_to_string, literal_value};
pub use main_cell::MainCell;
pub use merge::{merge_null, merge_same};
pub use pager::{paginate, PageInfo, Pagination};
pub use params::ParamTable;
pub use stage_a::{StageA, ROW_TYPE_GROUP_HEADER};
pub use stage_b::{cell_expression, prepare_calculate, StageB};
pub use value::Value;

#[cfg(test)]
mod stage_a_tests;
#[cfg(test)]
mod stage_b_tests;
