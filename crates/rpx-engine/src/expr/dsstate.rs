//! `Expr` 上的**两个树谓词**：`is_ds_function()` 与 `get_state()`（自 `ast.rs` 拆出，门禁行数）。
//!
//! 为什么与 `is_extend()` 分开放：三条谓词虽然都是"解析树上的一次纯递归"，但
//! **消费方完全不同**，混在一个文件里最容易互相抄错传播规则：
//!
//! | 谓词 | 谁消费 | 决定什么 |
//! |------|--------|----------|
//! | [`Expr::is_extend`]（在 [`super::ast`]） | `ExtNormalCell.checkExtend:510` | 要不要把格包成 `ExtExtendCell` |
//! | [`Expr::is_ds_function`] | `ExtNormalCell.java:648` | 要不要重算 DS 指针（`setDSCurrent`） |
//! | [`Expr::get_state`] | `ExtNormalCell.java:508/637` | 调度器换格的轮次门槛 |
//!
//! # ⛔ 三条链的传播规则**两两不同**，互相近似必错
//!
//! - `is_extend`：`Function` **不**向参数透传（`Function.java` 全文无 `isExtended()`）；
//! - `is_ds_function`：`Function` **向参数透传**（`Function.java:55-60`）；
//! - `get_state`：`Function` **向参数取最大**（`Function.java:48-53`）。
//!
//! 所以 `sum(ds1.select(qty))` 是「`is_extend` 假 / `is_ds_function` 真 / `get_state` 4」。
//! 两条回归钉子分别在 `super::tests::ds_function_and_extend_disagree_on_the_same_expression`
//! 与 `super::tests::get_state_is_four_for_every_expression_this_slice_supports`。

use super::ast::Expr;

impl Expr {
    /// `Expression.isDSFunction()` 的逐节点实现
    /// （`Expression.java:568-572` → `home.isDSFunction()` + 各节点覆写）。
    ///
    /// # ⛔ 与 [`Expr::is_extend`] 的传播规则**相反**（本项目最容易踩的一处）
    ///
    /// | 节点 | `is_ds_function` | 出处 |
    /// |------|------------------|------|
    /// | `Node` 默认 | `false` | `Node.java:103-105` |
    /// | `Constant` / `CSVariable`（`B2`） | `false` | 二者都 `extends Node` 且**未**覆写 |
    /// | `DSVariable`（`ds.col`） | **`true`**（覆写） | `DSVariable.java:54-56` |
    /// | `UnknownSymbol`（DS 参数里的裸列名） | **`true`**（覆写） | `UnknownSymbol.java:39-41` |
    /// | `DSFunction`（`ds.xxx(...)`） | **`true`**（覆写） | `DSFunction.java:25-27` |
    /// | `Function`（裸调用 `sum(...)`） | **向参数透传** `param.isDSFunction()` | `Function.java:55-60` |
    /// | `ValueList`（`[...]`/`list(...)`） | 同上（`ValueList extends Function`） | `ValueList.java:21` |
    /// | `Operator`（一元/二元） | `left \|\| right` | `Operator.java:91-96` |
    /// | `SpecialFunction` | `left \|\| super` | `SpecialFunction.java:71-76` |
    ///
    /// ⛔ **回归钉子**：`sum(ds1.select(qty))` 的 `is_ds_function() == true`
    /// 而 `is_extend() == false` —— 这个差别**有值级后果**：前者触发 DS 指针重算
    /// （`ExtNormalCell.java:648`），后者决定格的角色（`checkExtend:510`）。
    /// `engine_sum` 的 `(2,6)` 逐行 100/200/150 就是前者在起作用（§5.2.5）。
    ///
    /// ⚠ **与 `is_extend` 的另一处不对称**：`SpecialFunction` 的 `super` 是 `Function`
    /// （`SpecialFunction extends Function`，类声明已核），所以 `Select{arg}` 的**参数也查**
    /// （`arg.is_ds_function()`）；而 `is_extend()` 只看 `left`（几乎恒 null）⇒ 恒 false。
    /// 即 `{ds1.select(qty)}` 在两条链上一个真一个假。
    ///
    /// ⚠ `Index{target}` 的 Java 形态是 `Move`/`Offset`/`DSOffset`（都 `extends
    /// SpecialFunction`）：`is_ds_function = left || super`。本切片的 `target` 就是 `left`，
    /// 而**参数（`(...)` 的内容）未被 AST 建模** ⇒ 这里只查 `target`，是一条**已声明的缺口**。
    pub fn is_ds_function(&self) -> bool {
        match self {
            Expr::Const(_) | Expr::CellRef { .. } | Expr::Param { .. } | Expr::CurrentCell => false,
            Expr::DsColumn { .. } => true,
            Expr::Unknown(_) => true,
            Expr::ValueList { items, .. } => items.iter().any(Self::is_ds_function),
            Expr::Select { arg } => arg.is_ds_function(),
            Expr::Index { target } => target.is_ds_function(),
            Expr::Call(c) => c.qualifier.is_some() || c.args.iter().any(Self::is_ds_function),
            Expr::Unary { rhs, .. } => rhs.is_ds_function(),
            Expr::Binary { lhs, rhs, .. } => lhs.is_ds_function() || rhs.is_ds_function(),
        }
    }

    /// `Expression.getState()`（`Expression.java:554-558` → `home.getState()`）。
    ///
    /// # 语义：**子树取最大、下限 4**
    ///
    /// | 节点 | 值 | 出处 |
    /// |------|----|------|
    /// | `Node` 默认 | `4` | `Node.java:107-109` |
    /// | `Constant` / `CSVariable` / `DSVariable` / `UnknownSymbol` | `4` | 均 `extends Node` 且**未**覆写 |
    /// | `Function` / `DSFunction` / `ValueList` | `max(4, 各参数)` | `Function.java:48-53` + 参数树累积（`lIllIlIlIIIlIlIl.java:108-116` 逐子 `getState(state)`、`lllIlIlIIlIIlIll.java:73-76` 单子取大） |
    /// | `Operator`（一元/二元） | `max(4, left, right)` | `Operator.java:39-49` |
    /// | `SpecialFunction` | `max(super, left)` = `max(4, 参数, left)` | `SpecialFunction.java:62-69` |
    ///
    /// # ⛔ 本 crate 的可达值域 = `{4}`（有边界的结论，不是"没实现"）
    ///
    /// 上表每一层都只是「取大 + 下限 4」，叶子全部返回 4 ⇒ **没有任何表达式能产出 > 4**，
    /// **除非**用到显式覆写 `getState` 的函数。源码里覆写它的只有
    /// `page()` / `pageCount()` / `cellRowNo()` / `cellColNo()` / `cellCount()` /
    /// `cellSum()` / `sequence()` / `mcOrder()` / `getCell()` 与 `CalcGraph` /
    /// `CalcSubGantt` / `CalcCustomGraph` / `CalcSubRpt` 这批（`grep -rn getState` 的完整清单），
    /// 12 个引擎夹具**一个都没用**：`EngineModels*.java` 的公式只有
    /// `ds*.select/sum/count`、`sum(A:B)`、`B2*2`、`6*1` 这些形态。
    ///
    /// 后果（**接线时必须知道**的一条）：
    /// `ExtNormalCell.calculate:637` 的 `engine.state > cs.getState()` 在现有夹具上**恒假**
    /// ⇒ **state 5 / 6 / 8 的挂起路径恒不可达**，通道 5/6 对夹具只会走"chase + 完成"两支。
    /// 前提由两层钉子守住：`super::tests` 的表驱动单测
    /// + `crates/rpx-engine/tests/expr_state_tripwire.rs`（全 40 条夹具公式）。
    pub fn get_state(&self) -> u8 {
        /// 表格里"下限 4"的统一表达：取 `4` 与各子节点状态的最大值。
        fn max_of<'a>(subs: impl Iterator<Item = &'a Expr>) -> u8 {
            subs.fold(4u8, |acc, e| acc.max(e.get_state()))
        }
        match self {
            Expr::Const(_)
            | Expr::CellRef { .. }
            | Expr::Param { .. }
            | Expr::CurrentCell
            | Expr::DsColumn { .. }
            | Expr::Unknown(_) => 4,
            Expr::ValueList { items, .. } => max_of(items.iter()),
            Expr::Select { arg } => 4u8.max(arg.get_state()),
            Expr::Index { target } => 4u8.max(target.get_state()),
            Expr::Call(c) => max_of(c.args.iter()),
            Expr::Unary { rhs, .. } => 4u8.max(rhs.get_state()),
            Expr::Binary { lhs, rhs, .. } => 4u8.max(lhs.get_state()).max(rhs.get_state()),
        }
    }
}
