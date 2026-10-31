//! 函数名表与「名字 → 函数类别」的解析 —— `FunctionLib.loadSystemFunctions` 的忠实转录。
//!
//! 算法正文（本项目约定：算法写进 md，代码只留指针）：
//! `docs/m2-engine-design.md` §13.4 与 §4.1（V25 链）。
//!
//! # 为什么必须**照抄**两张名字表，而不是「是不是 DS 函数」的近似
//!
//! `Expression.createNode`（`Expression.java:210-228`）对一次 `名字(` 的解析顺序是：
//!
//! 1. `FunctionLib.isFunName(name)`（`FunctionLib.java:64-66`，查私有表 `_$4`）→ **普通函数**；
//! 2. 只有第 1 步不中、且 `ds != null` 时才回退到 `FunctionLib.isDSFunName(name)`
//!    （`:68-70`，查 `_$3`）→ **DS 函数**。
//!
//! 于是 `avg / count / max / min / plot / sum` 这 6 个**两张表都登记过的名字**在裸调用时
//! 一律解析成**普通函数**：`plot(x)` 是常规 `Plot`（`isExtended()` 未覆写 → `false`），
//! 只有 `ds1.plot(x)` 才是 `DSPlot`（`isExtended()` → `true`）。
//! 按「名字像 DS 就当 DS」实现会把 `plot(x)` 判成可扩展 —— 这就是本模块存在的理由。
//!
//! # 而且：`isExtend()` 那条路径上 `ds` **恒为 null**
//!
//! `ExtNormalCell.checkExtend`（`ExtNormalCell.java:498-504`）构造表达式时写死第二参数：
//!
//! ```java
//! this.engine.expression = new Expression(this.engine.cs, null, ctx, (String)this.engine.expression, false);
//! //                                                       ^^^^  (IReport cs, DataSet ds, Context ctx, String, boolean)
//! ```
//!
//! 所以上面第 2 步的 `ds != null` 恒假 ⇒ **裸 `select(qty)` 不会解析成 `DSSelect`**，
//! 它会掉到 `Variant2.parse`（`Expression.java:235`）成为 `UnknownSymbol`（`isExtended()` 也是 false）。
//! `ds1.select(qty)` 走的是另一条路（`Expression.java:173-177`：`id.lastIndexOf('.')` +
//! `ctx.getDataSet(前缀)`），**不受该 `null` 影响**，所以它是真 DS 调用。
//!
//! 证据侧：`isExtend()` 在整个产品里**只有一个调用点**（`ExtNormalCell.java:510`，
//! 全 `re_work/src` grep 确认），且就在上面那个 `ds = null` 的表达式上 ——
//! 所以本模块的 `classify_bare` 不把裸调用解析成 DS 函数是**有据**的，不是保守估计。
//!
//! ⚠ **本切片不做 `ctx.getDataSet(前缀) != null` 的校验**（切片没有 `Context`）：
//! 只要形态是 `前缀.名(` 且 `名 ∈ DS 表`，就按 DS 函数建模。求值期（M2-06/M2-13）
//! 必须补上前缀的数据集存在性校验，否则 `nods.select(x)` 也会被当成 `DSSelect`。

/// 普通函数表 `FunctionLib._$4`（`FunctionLib.java:121-254` 的 `addFunction` 全量）。
///
/// ⚠ 大小写敏感（Java `HashMap.containsKey`）：产品登记的是全小写名。
// 名字表是**密集字面量表**，与 Java 侧 `addFunction` 调用序列逐条对齐、按主题分组注释。
// rustfmt 会把每个名字拆成独立一行（+139 行、文件直接破 300 行上限），
// 逐行读名字表没有任何收益，故此处显式跳过格式重排（见 docs/code-review.md §5.9）。
#[rustfmt::skip]
pub const REGULAR: &[&str] = &[
    // 基础 / 单元格 / 数据集
    "avg", "subreport", "call", "case", "col", "row", "count", "value", "disp", "eval", "esproc",
    "spl", "cell", "ds", "if", "lefthead", "max", "maxc", "maxwidth", "mcorder", "min", "minc",
    "ifn", "plot", "query", "seq", "sort", "sublist", "sum", "throwe", "to", "tophead", "valueat",
    "map", "fread", "fexists", "barcode", "graph", "graph2", "list",
    // datetime
    "after", "elapse", "age", "deq", "datetime", "day", "dayname", "days", "hour", "interval",
    "millisecond", "minute", "month", "now", "pdate", "second", "date", "time", "year",
    // math
    "abs", "acos", "asin", "atan", "ceil", "cos", "exp", "fact", "floor", "ln", "lg", "pi",
    "power", "rand", "round", "sign", "sin", "sqrt", "tan",
    // string
    "fill", "link", "left", "len", "like", "lower", "mid", "pos", "rmquote", "replace", "right",
    "space", "split", "trim", "upper", "urlencode", "wordcap",
    // convert
    "ifdate", "iftime", "ifnumber", "ifstring", "isalpha", "isdigit", "islower", "isupper",
    "isempty", "number", "int", "long", "bigint", "float", "double", "decimal", "string",
    "hexstring", "asc", "char", "rgb", "chn", "rmb", "sequence", "json",
    // 分页
    "pno", "pcount", "pccount", "pcsum",
];

/// 登记在普通函数表里、但**显式覆写 `isExtended() = true`** 的四个名字。
///
/// - `sort` → `function/Sort.java:19`
/// - `sublist` → `function/SubList.java:18`
/// - `to` → `function/To.java:17`
/// - `split` → `function/string/SplitString.java:18`
pub const REGULAR_EXTEND: &[&str] = &["sort", "sublist", "to", "split"];

/// DS 函数表 `FunctionLib._$3`（`FunctionLib.java:106-120` 的 `addDSFunction` 全量）。
// 同上：密集字面量表，跳过 rustfmt 重排。
#[rustfmt::skip]
pub const DS: &[&str] = &[
    "avg", "bfind", "fcount", "count", "dcount", "field", "fname", "filtercross", "group",
    "isfield", "max", "min", "plot", "select", "sum",
];

/// 登记在 DS 函数表里、且**显式覆写 `isExtended() = true`** 的四个名字。
///
/// 其余 DS 函数（`DSSum` / `DSCount` / `DSAverage` / `DSMax` / `DSMin` / `DSField` …）
/// **都没有覆写**，继承 `DSFunction → Function → Node` 的 `false`。
///
/// - `select` → `function/dsfunction/DSSelect.java:23`
/// - `group` → `function/dsfunction/DSGroup.java:23`
/// - `filtercross` → `function/dsfunction/DSFilterCross.java:38`
/// - `plot` → `function/dsfunction/DSPlot.java:21`
pub const DS_EXTEND: &[&str] = &["select", "group", "filtercross", "plot"];

/// 函数节点的优先级 —— **只有三个类改过默认值**，其余全部是 `Node.java:54` 的 `19`。
///
/// | 函数 | 优先级 | 出处 |
/// |------|--------|------|
/// | `lefthead` | **17** | `function/LeftHead.java:22`（`this.priority = 17;`） |
/// | `tophead` | **17** | `function/TopHead.java:22`（`this.priority = 17;`） |
/// | `mcorder` | **16** | `function/McOrder.java:110`（`this.priority = 16;`） |
/// | 其余全部 | 19 | `Node.java:54`（`protected int priority = 19;`） |
///
/// ⚠ 默认 19 与"数字字面量"同值（`Node.PRI_NUM = 19`），这不是笔误。
/// ⚠ 括号内的节点还会 `priority += inBrackets * 20`（`Node.java:57`）。
///
/// # ⛔ 本表当前**无人消费**（P1 能力），而且是**刻意**的
///
/// 这三个数字是真的，但 [`super::parser`] 把**所有函数调用当原子（19）**，所以查不到这张表。
/// 独立验证方的变异 M6（把 `lefthead` 从 17 改成 19）杀不死任何测试 —— 因为**没有任何
/// 解析路径读它**。我们**接受**这个变异存活：前提是这里没有留下「看起来已被接线」的假象。
///
/// ## 为什么 P0 不接（不是"懒得接"，是接了也不忠实）
///
/// 差异只在一种形状上出现：**优先级低于前缀/后缀的函数紧跟其后**
/// （前缀 `-`/`+`=18、`not`/`&`=16，后缀 `[...]`/`(...)`=17）。产品真机实测
/// （证据：`tests/fixtures/probe_fun_priority_out.utf8.txt`）：
///
/// ```text
/// -sum(A1)        →  Negative(18) right=Sum          （19：前缀正常挂上）
/// -lefthead(A1)   →  LeftHead(17)                   （17：⛔ Negative 被孤立丢弃）
/// -mcorder(1)     →  McOrder(16)                    （16：⛔ Negative 被孤立丢弃）
/// -lefthead(A1)*2 →  Multiply(14) left=LeftHead     （⛔ Negative 被丢弃，乘号正常）
/// not lefthead(A1)→  Not(16) right=LeftHead          （16<17 下探成功 ⇒ 无差异）
/// lefthead(A1)+2  →  Add(13) left=LeftHead          （13<17 与 13<19 都成立 ⇒ 无差异）
/// ```
///
/// 机制：产品是线性右脊下探（`Expression.java:442-450`），而
/// `Node.setLeft` 是**空实现**（`Node.java:64-65`），只有 `Operator`（`Operator.java:19-25`）
/// 与 `SpecialFunction`（`SpecialFunction.java:19-21`）真的存链。所以当
/// `parent == null` 且 `node` 是 `Function` 时，`home` 被覆盖、**前驱子树被静默丢弃**
/// —— 这本身是产品的缺陷（`-lefthead(A1)` 的减号没了）。
///
/// **Pratt 在结构上没有"丢弃已建子树"这个操作**（递归下降只会往上挂）。所以把本表接进
/// parser 只会造出**第三种**树形（我们的 `-(lefthead(A1))` vs 产品的 `lefthead(A1)`），
/// 既不忠实，又把一个 P1 级语义差异混进 P0 路径。**要忠实复刻只能移植线性右脊算法。**
///
/// ## 何时接线（§12.1 第 8 步）
///
/// 接线时**必须同时**做三件事，缺一不可：
/// 1. 决定是否连 orphaning 一起复刻（复刻 ⇒ 改用线性算法；不复刻 ⇒ 在文档里显式记差异）；
/// 2. 补**能观测接线**的回归（断言树形随优先级变化，而不是断言"查表返回表里的值"）；
/// 3. 把本节的 ⛔ 横幅删掉 —— 它存在的唯一目的就是提醒"这里有未消费的能力"。
///
/// ⛔ **本表不参与 `is_extend()`**：上面所有形状的产品 `isExtended` 都是 `false`
/// （这三个函数类都没覆写），我们也是 false ⇒ 对 M2 扩展算法**零影响**。
pub const FUN_PRIORITY_LEFTHEAD: i32 = 17;
/// 见 [`FUN_PRIORITY_LEFTHEAD`] 的说明（`TopHead.java:22`）。
pub const FUN_PRIORITY_TOPHEAD: i32 = 17;
/// 见 [`FUN_PRIORITY_LEFTHEAD`] 的说明（`McOrder.java:110`）。
pub const FUN_PRIORITY_MCORDER: i32 = 16;
/// 其余全部函数的默认值（`Node.java:54`）。
pub const FUN_PRIORITY_DEFAULT: i32 = 19;

/// 查表取函数优先级。**当前无调用方**（见上方 ⛔ 横幅），保留它是为了让这三个数字
/// 在代码里有一个可被 grep、可被文档引用的落点。
pub fn fun_priority(name: &str) -> i32 {
    match name {
        "lefthead" => FUN_PRIORITY_LEFTHEAD,
        "tophead" => FUN_PRIORITY_TOPHEAD,
        "mcorder" => FUN_PRIORITY_MCORDER,
        _ => FUN_PRIORITY_DEFAULT,
    }
}

/// 一次调用的解析结果类别 —— **只有两个 `*Extend` 会贡献 `is_extend()`**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    /// 普通函数、未覆写 `isExtended()` → false。
    Regular,
    /// 普通函数、显式覆写为 true（`sort`/`sublist`/`to`/`split`）。
    RegularExtend,
    /// DS 函数、未覆写 `isExtended()` → false（`ds1.sum(qty)`、`ds2.count()` 等）。
    Ds,
    /// DS 函数、显式覆写为 true（`select`/`group`/`filtercross`/`plot`）。
    DsExtend,
    /// 名字不在任何表里：产品会落成 `UnknownSymbol`（`isExtended()=false`）。
    Unknown,
}

impl CallKind {
    /// `isExtended()` 在这一类调用上的取值（`Function`/`DSFunction` 不向参数透传，见 §13.3）。
    pub fn is_extend(self) -> bool {
        matches!(self, CallKind::RegularExtend | CallKind::DsExtend)
    }
}

/// **裸**调用 `名(...)` 的分类（`Expression.java:219-224`，只查普通表；DS 回退因 `ds == null` 不可达）。
pub fn classify_bare(name: &str) -> CallKind {
    if REGULAR.contains(&name) {
        if REGULAR_EXTEND.contains(&name) {
            CallKind::RegularExtend
        } else {
            CallKind::Regular
        }
    } else {
        // 裸 DS 名（select/group/… 不在普通表里）走不到 DS 回退：`ds == null`。
        CallKind::Unknown
    }
}

/// **DS 限定**调用 `ds.名(...)` 的分类（`Expression.java:173-177` + `createDSNode:253-273`）。
pub fn classify_ds(name: &str) -> CallKind {
    if DS.contains(&name) {
        if DS_EXTEND.contains(&name) {
            CallKind::DsExtend
        } else {
            CallKind::Ds
        }
    } else {
        CallKind::Unknown
    }
}
