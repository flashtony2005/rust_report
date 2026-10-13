//! 表达式的 AST 与 `is_extend()` 谓词。
//!
//! 算法正文见 `docs/m2-engine-design.md` §13.2（节点清单）与 §13.3（`is_extend` 真值表）。
//!
//! # `is_extend()` 是什么
//!
//! `Expression.isExtend()`（`Expression.java:561-566`）就是 `home.isExtended()` ——
//! **解析树上的一次纯递归谓词，不做任何求值**。它唯一的调用点是
//! `ExtNormalCell.checkExtend`（`ExtNormalCell.java:510`，全产品唯一，已 grep 确认）。
//!
//! ⛔ **不许用 `isDSFunction()` 近似**：两条链的传播规则**相反**。
//! `Function.isDSFunction()`（`Function.java:55-57`）**向参数透传**，而
//! `Function`/`Sum`/`DSSum` 都**没有**覆写 `isExtended()`（`Sum extends Function extends Node`，
//! `Function.java:17-18`），所以 `sum(ds1.select(qty))` 的 `isDSFunction()=true` 而
//! `isExtended()=false`。这条反例是回归单测（`tests.rs`）。
//!
//! # 树形与 Java 的对应（每个变体点名了源码原类）
//!
//! | 本切片 | Java 原类 | `isExtended()` 出处 |
//! |--------|-----------|---------------------|
//! | [`Expr::Const`] | `Constant` | `Constant.java:22-24`（`value instanceof List`） |
//! | [`Expr::CellRef`] | `CSVariable` | `CSVariable.java:379-381`（恒 false） |
//! | [`Expr::Param`] | `Constant` / `ParamNode extends Constant` | 同 `Constant`；值未知 ⇒ 取 false |
//! | [`Expr::CurrentCell`] | `CSCurrent extends Function` | 继承 `Node.java:82-84`（false） |
//! | [`Expr::DsColumn`] | `DSVariable extends Node` | 继承 `Node.java:82-84`（false） |
//! | [`Expr::Unknown`] | `UnknownSymbol extends Node` | 继承 `Node.java:82-84`（false），但 `isDSFunction()=true`（`:39-41`） |
//! | [`Expr::ValueList`] | `ValueList extends Function` | `ValueList.java:30-32`（**自己**重写） |
//! | [`Expr::Select`] | `Select extends SpecialFunction` | `SpecialFunction.java:27-32` 读 **`left`**，而 `left` 几乎恒 null ⇒ **恒 false** |
//! | [`Expr::Index`] | `Move`/`DSOffset`/`Offset` | 同上，读 `left`；本切片的 `target` 就是 `left` |
//! | [`Expr::Call`] | `Function` / `DSFunction` | 见 [`CallKind::is_extend`]（`Function` **未**重写） |
//! | [`Expr::Unary`] / [`Expr::Binary`] | `Operator`（前缀/中缀） | `Operator.java:51-56`，**`Link` 例外** |
//!
//! # ⛔ 四条已逐行核过、最容易写错的类层次事实
//!
//! 1. **`SpecialFunction` 只挂 `left`**（`SpecialFunction.java:17-18`），
//!    `Node.setLeft/setRight` 在基类是**空实现** ⇒ 往 `Constant`/`Function` 上挂子链
//!    **静默丢弃**。
//! 2. **`Select` 的 `left` 不是花括号里的表达式**：`Expression.java:131-134` 只调
//!    `setParameter(...)`，`left` 要靠 `Expression.java:445` 的通用 `node.setLeft(right)`
//!    才可能非 null，而那要求**前驱节点优先级 ≥ 17**（`Select.java:26` 自己是 17）。
//!    常规写法（前驱是 `B2` / `sum(...)` / 运算符）拿到的 `left` 要么是那些节点的
//!    `isExtended()`（CSVariable/Function 恒 false），要么是 null。
//! 3. **`Function` 不向参数透传 `isExtended()`**（`Function.java` 全文无该方法），
//!    而 `Function.isDSFunction()` **向参数透传**（`Function.java:55-57`）—— 两条链相反。
//! 4. **`setLeft` 只有一个调用点**（`Expression.java:445`），`priority` 还会在
//!    括号内 `+= 20`（`Node.java:57`）。

use crate::value::Value;

use super::names::CallKind;

/// 一个表达式节点。
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// 字面量。`Constant`（`Constant.java:11`）。值为 `List` 时 `is_extend()` 为真。
    Const(Value),
    /// 格引用 `B2` / `$B2`。`CSVariable`（`CSVariable.java:19`）。
    /// `absolute = true` 表示带 `$` 前缀（产品的"整格绝对引用"，见附件 G.1）。
    ///
    /// ⚠ `provisional = true` 表示**这是按形态猜的**：产品的判定是
    /// `((ExtCellSet)cs).getSource(id) != null`（`Expression.java:201`），本切片没有
    /// 引擎矩阵只能暂定。`qty1` / `amount2` 这类**数据集列名会被误判成本变体**。
    /// 对 `is_extend()` 无影响（`CSVariable` 与 `UnknownSymbol` 都是 false），
    /// 但求值期**必须**用引擎矩阵重新解析。未验证 ⇒ §13.5。
    CellRef {
        /// 格号原文（已剥 `$`），如 `B2`。
        id: String,
        /// 是否带 `$` 前缀。
        absolute: bool,
        /// 是否只是「形态上像格号」的**暂定**判定（见上文）。
        provisional: bool,
    },
    /// 参数引用 `@p` / `@@p`。产品建的是 **`Constant(ctx.getParamValue(id))`**
    /// （`Expression.java:185` 的 `@@`、`:188` 的 `@`）与
    /// `ParamNode(id, ctx.getParamValue(id))`（`:233`，裸参数名形态）。
    /// 三者都继承 `Constant.isExtended() = value instanceof List`（`Constant.java:22-24`）。
    ///
    /// ✅ **真机已验证**（`tests/fixtures/probe_param_list_out.utf8.txt`，2026-10）：
    /// 多选参数（值是 `List`）⇒ `isExtend()=true` ⇒ 该格**真的**被包成扩展格 ——
    /// `List[1,2,3]` 时计算矩阵从 3×2 长到 **5×2**，锚点 `extend=18 subCells=3`。
    /// 标量参数（`Integer(7)` / `String("x")`）⇒ `false`。
    /// ⚠ 两个边界同样为 **true**（判的是 `instanceof List`，不看内容）：
    /// **单元素** `List[9]` 与 **空** `List[]`。空 List 会让格"成为扩展格却没有子格"。
    ///
    /// 本切片没有 `Context`，拿不到参数值 ⇒ `value` 恒 `None` ⇒ 取 **false**。
    /// 这个默认值现在**由证据支撑**而非假设：真机上标量参数确实是 false，
    /// 所以"未知时取 false"不误判标量。求值期（M2-13）必须用真实参数值重判。
    Param {
        /// 参数名（已剥 `@`/`@@`）。
        name: String,
        /// 是否 `@@`（宏参数；`@@` **不查** `isParamName`，`Expression.java:183-186`）。
        is_macro: bool,
        /// 参数值（产品在建节点时就有；本切片恒 `None`）。
        value: Option<Value>,
    },
    /// `@`（当前格）。`CSCurrent`（`function/CSCurrent.java:13`）。
    CurrentCell,
    /// `数据集.列名`。`DSVariable`（`DSVariable.java:16`）。
    DsColumn {
        /// 数据集名。
        ds: String,
        /// 列名。
        col: String,
    },
    /// 未识别符号。`UnknownSymbol`（`UnknownSymbol.java:15`），产品 `isExtended()` = false。
    Unknown(String),
    /// `[...]` 值列表/区间。`ValueList`（`ValueList.java:22`）。
    ValueList {
        /// 顶层分隔符类别（决定 `is_extend()`）。
        kind: ParamKind,
        /// 顶层切出的子表达式（供求值期使用；本切片不解释语义）。
        items: Vec<Expr>,
    },
    /// `{...}` 单元格选择。`Select`（`Select.java:21`）。
    ///
    /// ⚠ `arg` 是花括号里的**参数**（产品：`Expression.java:132` 的 `setParameter`），
    /// **不是** `SpecialFunction.left`。`is_extend()` 不读它 —— 见 [`Expr::is_extend`]。
    Select {
        /// `{...}` 内层表达式（产品的 `param`）。
        arg: Box<Expr>,
    },
    /// 后缀 `(...)` / `[...]` 挂在某个表达式上。`Offset` / `Move` / `DSOffset`
    /// （都 `extends SpecialFunction`）。`is_extend()` 只看 `target`（= 产品的 `left`）。
    Index {
        /// 被作用的表达式（Java 的 `SpecialFunction.left`）。
        target: Box<Expr>,
    },
    /// 函数调用。`Function`（`function/*`）或 `DSFunction`（`function/dsfunction/*`）。
    Call(Call),
    /// 前缀运算符。`Not`/`Negative`/`Plus`/`Address`（都 `extends Operator`，只挂 `right`）。
    Unary {
        /// 运算符。
        op: UnOp,
        /// 操作数（Java 的 `right`）。
        rhs: Box<Expr>,
    },
    /// 中缀运算符。`Operator` 的 `left`/`right` 两侧。
    Binary {
        /// 运算符。
        op: BinOp,
        /// 左操作数。
        lhs: Box<Expr>,
        /// 右操作数。
        rhs: Box<Expr>,
    },
}

/// 参数（`IParam`）的顶层分隔符类别。
///
/// `ParamParser._$1`（`ParamParser.java:66-79`）的层级是 `'\0' → ';' → ',' → ':'`：
/// 取**第一个出现**的分隔符作为该层的 `getType()`；都不出现则是叶子（`getType()='\0'`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// 叶子：没有顶层分隔符（`lllIlIlIIlIIlIll.getType()` 返回 `'\0'`，`lllIlIlIIlIIlIll.java:47-49`）。
    Leaf,
    /// 顶层 `,`（`Comma`）。
    Comma,
    /// 顶层 `:`（`Link`）。
    Colon,
    /// 顶层 `;`。
    Semicolon,
}

impl ParamKind {
    /// `ValueList.isExtended()`（`ValueList.java:30-32`）：
    /// `param == null || param.getType() != ':'`。
    ///
    /// 叶子（`param == null` 等价于空 `[]`）与 `,`/`;` 都返回 true，**只有顶层 `:` 返回 false**。
    pub fn is_extend(self) -> bool {
        self != ParamKind::Colon
    }
}

/// 前缀运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `!` / `not`（`Not`，优先级 16）。
    Not,
    /// 一元 `-`（`Negative`，优先级 18）。
    Neg,
    /// 一元 `+`（`Plus`，优先级 18）。
    Pos,
    /// `&`（`Address`，优先级 16）。
    Address,
}

/// 中缀运算符 —— 优先级数值逐条取自 `Node.java:13-44` 的 `PRI_*` 常量与各运算符类的
/// `this.priority = N`（见 §13.1 的优先级表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `,`（`Comma.java:16`，优先级 1）。`Operator` → 两侧或。
    Comma,
    /// 单个 `=`（`Evaluate.java:18`，优先级 2）。赋值/求值。
    Eval,
    /// `:`（`Link.java:25`，优先级 4）。⛔ **本运算符显式覆写 `isExtended()=false`**
    /// （`operator/Link.java:100-102`）。
    Link,
    /// `or` / `||`（`Or.java:17`，优先级 5）。
    Or,
    /// `and` / `&&`（`And.java:18`，优先级 6）。
    And,
    /// `==`（`Equals.java:17`，优先级 10）。`=` 与 `=` 相连时走这里（`Expression.java:349-352`）。
    Eq,
    /// `!=`（`NotEquals.java:17`，优先级 10）。
    Ne,
    /// `in`（`In.java:18`，优先级 10）。
    In,
    /// `>`（`Greater.java:17`，优先级 11）。
    Gt,
    /// `>=`（`NotSmaller.java:17`，优先级 11）。
    Ge,
    /// `<`（`Smaller.java:17`，优先级 11）。
    Lt,
    /// `<=`（`NotGreater.java:17`，优先级 11）。
    Le,
    /// 二元 `+`（`Add.java:23`，优先级 13）。
    Add,
    /// 二元 `-`（`Subtract.java:22`，优先级 13）。
    Sub,
    /// `*`（`Multiply.java:21`，优先级 14）。
    Mul,
    /// `/`（`Divide.java:21`，优先级 14）。
    Div,
    /// `%`（`Mod.java:21`，优先级 14）。
    Mod,
}

/// 一次函数调用。
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// `ds.名(...)` 里的 `ds`；裸调用为 `None`。
    pub qualifier: Option<String>,
    /// 函数名（已剥 `ds.` 前缀与 `@选项`）。
    pub name: String,
    /// `名@选项(...)` 的选项串（`Expression.java:212-218` 的 `indexOf('@')` 切法）。
    pub option: Option<String>,
    /// 名字解析结果（决定 `is_extend()`）。
    pub kind: CallKind,
    /// 顶层参数类别（`list(...)` 走 `ValueList`，见 parser）。
    pub param_kind: ParamKind,
    /// 顶层切出的子表达式（⚠ 未建模嵌套 `IParam` 树，见 §13.5）。
    pub args: Vec<Expr>,
}

impl Expr {
    /// `Expression.isExtend()` 的逐节点实现（`Expression.java:561-566` + 各节点的 `isExtended()`）。
    ///
    /// **纯递归、不做求值。** 每个分支的来源见模块文档的真值表。
    pub fn is_extend(&self) -> bool {
        match self {
            // Constant.isExtended() = value instanceof List（Constant.java:22-24）
            Expr::Const(v) => matches!(v, Value::List(_)),
            // ⛔ ParamNode/Constant 的值本切片拿不到（None）⇒ 取 false。
            //    真机上多选参数（值是 List）会为真 —— 真实分叉，零夹具覆盖，§13.5 未验证。
            Expr::Param { value, .. } => matches!(value, Some(Value::List(_))),
            // CSVariable / CurrentCell / DSVariable / UnknownSymbol
            Expr::CellRef { .. } | Expr::CurrentCell | Expr::DsColumn { .. } | Expr::Unknown(_) => {
                false
            }
            // ValueList.isExtended() = param == null || param.getType() != ':'（ValueList.java:30-32）
            Expr::ValueList { kind, .. } => kind.is_extend(),
            // SpecialFunction.isExtended() = left.isExtended() || super(false)
            //   （SpecialFunction.java:27-32；super 是 Function，**未重写** ⇒ Node 的 false）
            //
            // ⛔ `Select` 的 `left` **不是**花括号里的表达式（`Expression.java:131-134`
            //    只 setParameter）。`left` 只能由通用 `Expression.java:445` 的
            //    `node.setLeft(right)` 填上，且要求前驱节点优先级 ≥ 17（`Select.java:26`）。
            //    常规写法下前驱是 CSVariable / Function / 运算符（isExtended 全 false）
            //    或压根没有前驱 ⇒ **恒 false**。
            //    ⚠ 未覆盖：形如 `[1,2]{x}` 的相邻原子（前驱是 ValueList(19)、isExtended=true）
            //    会让产品判 true，本切片仍判 false。零夹具覆盖，§13.5 已登记。
            Expr::Select { .. } => false,
            // Move / DSOffset / Offset 的 `left` 就是本切片的 `target`（前一个表达式）
            Expr::Index { target } => target.is_extend(),
            // Function/DSFunction：不向参数透传（Function 未覆写 isExtended）→ 只看类别
            Expr::Call(c) => c.kind.is_extend(),
            // Operator.isExtended() = left || right（Operator.java:51-56）——前缀只挂 right
            Expr::Unary { rhs, .. } => rhs.is_extend(),
            // ⛔ Link 是唯一的例外：它是 Operator 却覆写成 false（operator/Link.java:100-102）
            Expr::Binary { op: BinOp::Link, .. } => false,
            Expr::Binary { lhs, rhs, .. } => lhs.is_extend() || rhs.is_extend(),
        }
    }
}
