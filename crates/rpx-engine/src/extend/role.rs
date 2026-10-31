//! [`CellRole`]：一个格走哪条 `extensible` 写入路径（M2-15 的两条路径）。
//!
//! **为什么必须单独建模**：`extend.rs` 早期版本只实现了扩展格那条拓扑改写，
//! 于是"因为错的原因得出对的答案"—— 夹具恰好也对，但换个 heads 就错。
//! 详见 [`CellRole`] 的文档与 `docs/m2-engine-design.md` §4.3。

use crate::arena::CellId;

use super::{topology_default, ExtendMode};

/// 一个格在 `prepareCalculate` 眼里属于哪一类 —— **它决定走哪条写入路径**。
///
/// ⛔ **只实现 `Extender` 是一条会"因为错的原因得出对的答案"的陷阱**（本项目实测踩到）：
/// `engine_sum` 第 2 行里 `(2,3)..(2,9)`（`B2*2` / `sum(B2:B4)` / `ds2.sum(qty)` …）计算态确实是
/// `extend=17`，但**不是**拓扑推出来的 —— 它们的 heads 是 `null`，恰好也让
/// `topology_default` 给出 17。真因是它们**不是扩展格**，走的是 [`CellRole::PlainFormula`]
/// 那条「无条件置 17」的分支。若不建模这条路径，只要某天来一个 heads 落在
/// `(cell00, cell00)` 的普通公式格，就会判成 18 而错。
///
/// 角色判定的源码依据是 `ExtNormalCell.checkExtend`（`ExtNormalCell.java:492-514`）：
///
/// ```java
/// public boolean checkExtend(Context ctx) {
///     byte extend = this.getExtensible();
///     byte type = this.getCellType();
///     if (type == -61 || type == -57 || type == -60) { return false; }   // 表达式格类型直接排除
///     if (this.engine.expression != null) {
///         ...构建 Expression...
///         if (16 == extend) { return ((Expression)this.engine.expression).isExtend(); }
///     }
///     return 18 == extend || 19 == extend;                               // 声明 18/19 恒为真
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellRole {
    /// **没有表达式**：两条路径都只在 `expression != null` 时改写，所以声明值原样保留。
    /// 实测残留：`engine_sum` 27 格、`engine_expr` 15 格、`engine_merge` 10 格仍是 `16`。
    Literal,
    /// **扩展格**（会被包成 `ExtExtendCell`）：走拓扑改写（`ExtExtendCell.java:315-329`）。
    Extender,
    /// **有表达式但不会成为扩展格的普通格**（仍是 `ExtNormalCell`）：
    /// `ExtNormalCell.java:539-541` 的 `if (expression != null) this.setExtensible((byte)17);`
    /// —— **无条件置 17**，与 heads 拓扑无关。
    PlainFormula,
}

/// `ExtNormalCell.checkExtend` **开头**的类型短路（`ExtNormalCell.java:494-496`）：
/// 图形 / 子报表 / 条形码 这三种格**永远不是扩展格**，无论 extendMode 声明成什么。
///
/// 常量取自 `INormalCell.java:24-28`：`TYPE_GRAPH=-61` / `TYPE_SUBREPORT=-60` / `TYPE_BARCODE=-57`。
///
/// ⛔ 这句短路排在**任何** extend 判断之前，所以它必须优先于"声明 18/19 恒为扩展格"。
/// 漏掉它的后果是**可构造的**：`setCellType((byte)-57)` + `setExtendMode(18)` + 任意表达式
/// → Java 计算态 `extend=17`（落成 `ExtNormalCell` 后走无条件置 17），
/// 而只看声明的实现会判成 `Extender` → 18。当前 7 个夹具都不含这类格，
/// 所以**没有任何测试会失败** —— 这正是它危险的地方。
pub const EXTEND_EXCLUDED_CELL_TYPES: [i8; 3] = [-61, -57, -60];

/// 该 cellType 是否落在 [`EXTEND_EXCLUDED_CELL_TYPES`] 里。
pub fn is_extend_excluded_type(cell_type: i8) -> bool {
    EXTEND_EXCLUDED_CELL_TYPES.contains(&cell_type)
}

/// [`CellRole`] 的判定，逐条对齐 `ExtNormalCell.checkExtend`（`:492-514`）。
///
/// # 参数
///
/// - `has_expression`：该格有没有表达式。
/// - `declared`：模板里声明的 extendMode。
/// - `expr_is_extend`：表达式的 `Expression.isExtend()` —— 「这个表达式会不会驱动扩展」。
///   ⚠ **本 crate 尚未实现它**（需要表达式解析，属后续切片）：调用方必须传入，
///   不许在这里猜（例如按「是不是 DS 函数」近似 —— 那是未经验证的替代）。
/// - `cell_type`：`getCellType()` 的原始字节（图形/子报表/条形码见 [`EXTEND_EXCLUDED_CELL_TYPES`]）。
pub fn cell_role(
    has_expression: bool,
    declared: ExtendMode,
    expr_is_extend: bool,
    cell_type: i8,
) -> CellRole {
    // 第 1 步（Java 顺序相同）：类型短路 —— 这类格不会被包成 ExtExtendCell，
    // 于是退回 ExtNormalCell 的规则：有表达式就置 17，没有就保留声明值。
    if is_extend_excluded_type(cell_type) {
        return if has_expression {
            CellRole::PlainFormula
        } else {
            CellRole::Literal
        };
    }
    // 第 2 步：`checkExtend` 末行 `return 18 == extend || 19 == extend;` —— 无条件为真
    if declared.drives_extension() {
        return CellRole::Extender;
    }
    if !has_expression {
        return CellRole::Literal;
    }
    // 走到这里：声明 16 或 17，且有表达式
    if declared == ExtendMode::Default16 && expr_is_extend {
        CellRole::Extender
    } else {
        // 声明 17，或声明 16 但表达式不驱动扩展
        CellRole::PlainFormula
    }
}

/// 从**表达式原文**直接判定 [`CellRole`] —— [`cell_role`] 的便捷入口。
///
/// # 为什么需要它
///
/// [`cell_role`] 要一个 `expr_is_extend: bool`，而它来自 `Expression.isExtend()`
/// —— 那是**要解析整棵语法树**才能算的谓词（`spec-m2.md` §5.1：全包 17 处覆写，
/// 且 `Operator` 走 `left || right`、`SpecialFunction` 只看 `left` + `super` 恒假）。
/// 本函数把 [`crate::expr::is_extend`] 接进来，调用方不必自己串两段逻辑，
/// 也就**不会**出现「忘了传 `expr_is_extend` 于是默认 false」这种静默错误。
///
/// # 参数
///
/// `expression` 传 `None` 等价于「该格没有表达式」；传 `Some("")` 则是「有表达式但是空串」
/// —— 两者**不等价**，别混。
///
/// `params` 是报表参数值表（M3-01）：`is_extend` 会对表达式**整树重建**，其中
/// `@名`/裸参数名要在建树期绑定值（`@名` 未登记 ⇒ 直接报错，见 [`crate::params`]）。
///
/// # 错误
///
/// 表达式解析失败时**返回错误**而不是退回 `false`：静默假设「不扩展」会把
/// 本该扩展的格判成 `PlainFormula`，导致扩展行数算少 —— 属于会静默算错的分叉。
/// `EngineError` 变体见 [`crate::error::EngineError::UnsupportedExpr`]。
pub fn cell_role_of(
    expression: Option<&str>,
    declared: ExtendMode,
    cell_type: i8,
    params: &crate::params::ParamTable,
) -> crate::error::Result<CellRole> {
    let Some(src) = expression else {
        return Ok(cell_role(false, declared, false, cell_type));
    };
    let expr_is_extend = crate::expr::is_extend(src, params)?;
    Ok(cell_role(true, declared, expr_is_extend, cell_type))
}

impl ExtendMode {
    /// `prepareCalculate` 的**完整**语义 —— 两条写入路径合起来才是契约。
    ///
    /// | 角色 | 改写规则 | 源码 |
    /// |------|----------|------|
    /// | [`CellRole::Literal`] | 不改写，原样保留声明值 | 两条路径的 `else` 分支 |
    /// | [`CellRole::Extender`] | 声明 16 → [`topology_default`]；17/18/19 不动 | `ExtExtendCell.java:315-329` |
    /// | [`CellRole::PlainFormula`] | **无条件** → 17 | `ExtNormalCell.java:539-541` |
    pub fn resolve(
        self,
        role: CellRole,
        left_head: CellId,
        top_head: CellId,
        cell00: CellId,
    ) -> ExtendMode {
        match role {
            CellRole::Literal => self,
            CellRole::PlainFormula => ExtendMode::None17,
            CellRole::Extender => {
                if self == ExtendMode::Default16 {
                    topology_default(left_head, top_head, cell00)
                } else {
                    self
                }
            }
        }
    }
}
