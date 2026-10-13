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
//! | [`eval`] | 第 7 步求值调度：四通道 trampoline（附件 K.1 三条通道 + 统一入口） | ✅（顶层 `calculate()` 序列未接齐） |
//! | [`ext_algo`] | 第 8 步扩展算法：包络 / 部分平移插行列 / expand*Head / extendRow·Col 两趟（附件 I） | ✅（夹具级全网格回归待顶层接线） |
//! | [`stage_b`] | 阶段 B 驱动：role 判定 → head 赋值（含登记）→ 拓扑改写 → 归一化（§4）→ 字面量取值 | ✅ 引擎夹具对拍（改写全量 + 未移动区 head + `engine_types` 全量值） |
//!
//! 尚未落地（`docs/spec-m2.md` §4.1 的其余模块）：顶层 `calculate()` 驱动（阶段 B
//! 已就绪，缺生产版 `EvalCtx`/`SchedGrid` 与扩展—求值的接线）、附属格 `sub_cells` 落盘、
//! 同值合并 `merge`、分页 `pager`。**在它们落地前，本 crate 不构成一个可用的 M2。**
//!
//! 按 `docs/m2-engine-design.md` §12.1 的 11 步执行顺序：第 1 步（两张矩阵，
//! [`arena`]）、第 2 步（阶段 A 建格，[`stage_a`]）、第 3 步（`is_extend()`，
//! [`expr`]）、第 5/6 步（head 图，[`heads`]）、第 7 步（求值调度，[`eval`]）、
//! 第 8 步（扩展算法，[`ext_algo`]）已落地，阶段 B 的组装（[`stage_b`]）
//! 也已按 §4 的三段顺序接齐；其余为顶层 `calculate()` 拼装与收尾对拍。
//!
//! # 门禁
//!
//! 零第三方依赖；每个 `.rs` ≤300 行；`clippy --all-targets -- -D warnings` 零告警；
//! **全中文注释**。
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
//! | 扩展方式改写与行数口径（[`extend`]） | `docs/format-contract.md` 附件 H.4、H.7.1 |
//! | 求值语义 / 聚合 / 类型保真 | `docs/format-contract.md` 附件 H.1、H.2、H.3 |
//! | 阶段 B 三段顺序（role → head → 改写 → 归一） | `docs/m2-engine-design.md` §4 |
//! | 字面量取值 / `BigDecimal.toString`（[`literal`]） | `docs/m2-engine-design.md` §4、`docs/format-contract.md` 附件 H.3 |
//! | `$` 语义 / 循环引用检测 | `docs/format-contract.md` 附件 G.1、G.3、H.8 |

#![forbid(unsafe_code)]

pub mod arena;
pub mod calc;
pub mod dataset;
pub mod emit;
pub mod error;
pub mod eval;
pub mod expr;
pub mod extend;
pub mod ext_algo;
pub mod heads;
pub mod literal;
pub mod main_cell;
pub mod stage_a;
pub mod stage_b;
pub mod value;

pub use arena::{CalcCell, CalcGrid, CalcMatrix, CellId, EngineMatrix, SourceRef, SubValue};
pub use calc::calculate;
pub use calc::{DsInstance, HeadPairTracker};
pub use error::{EngineError, Result};
pub use eval::{run_channels, state as eval_state, Flow, SchedGrid, Suspend};
pub use expr::{is_extend, parse, Expr};
pub use ext_algo::{extend_col, extend_row};
pub use extend::{
    calc_row_count, cell_role, is_extend_excluded_type, topology_default, CellRole, ExtendMode,
    EXTEND_EXCLUDED_CELL_TYPES,
};
pub use heads::{find_left_head, find_top_head, normalize_heads, HeadGrid};
pub use literal::{decimal_to_string, literal_value};
pub use main_cell::MainCell;
pub use stage_a::{StageA, ROW_TYPE_GROUP_HEADER};
pub use stage_b::{cell_expression, prepare_calculate, StageB};
pub use value::Value;

#[cfg(test)]
mod stage_a_tests;
#[cfg(test)]
mod stage_b_tests;
