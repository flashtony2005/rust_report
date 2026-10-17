//! 参数矩阵解码（`DSSelect.calculate:36-145`）—— 从 `IParam` 树到
//! [`SelectArgs`] 全字段。排序段（`;` 第二段）的键/次序对解码复刻
//! `ParamInfo2.parse`（`ParamInfo2.java:29-113`）。
//!
//! 本文件由 `dsselect/mod.rs` 的 [`decode`] 消费；错误出口用产品消息键
//! （`missingParam` / `invalidParam` / `paramTypeError`）。

use crate::error::{EngineError, Result};
use crate::expr::ast::{Call, Expr, Param, ParamKind};
use crate::expr::eval::EvalCtx;
use crate::value::Value;

use super::eval_bound;

/// 产品 `ReportError`（`function.missingParam` / `invalidParam` / `paramTypeError`）
/// 的本仓出口：没有专用变体，统一走 [`EngineError::UnsupportedExpr`] 并在消息里
/// 点名产品消息键（与 `classify::err` 同策略）。
fn param_err(key: &str, detail: &str) -> EngineError {
    EngineError::UnsupportedExpr {
        expr: format!("数据集函数 {key}: {detail}"),
    }
}

/// 解码后的 select 参数（`DSSelect.java:36-145` 的逐字段对应物）。
pub(crate) struct SelectArgs {
    /// 选列表达式（`selectExp`，必填）。
    pub(crate) select_exp: Expr,
    /// 过滤表达式（`fltExp`，可空）。
    pub(crate) flt_exp: Option<Expr>,
    /// 限量（`num`；**-1 = 不限**，产品初值 `:40`）。
    pub(crate) num: i32,
    /// 去重表达式（`distinctExp`，可空）。
    pub(crate) distinct_exp: Option<Expr>,
    /// `:` 次序（`order`；>0 升 / <0 降 / 0 不排序）。
    pub(crate) order: i32,
    /// `;` 多排序键（`sortExps`）与逐键降序标记（`isDescs`）。
    pub(crate) sort_exps: Vec<Expr>,
    pub(crate) is_descs: Vec<bool>,
}

/// `IParam.getLeafExpression()` 的安全版：产品对**非叶子**节点调它是**抛
/// `invalidParam`**（`lIllIlIlIIIlIlIl.java:56-60`），不是返 null ——
/// `select(x, a:b)` 这类写法的过滤段会走到这里。
fn leaf_or_err(p: Option<&Param>, what: &str) -> Result<Option<Expr>> {
    match p {
        None => Ok(None),
        Some(node) if node.is_leaf() => Ok(Some(node.leaf_expr().expect("叶子").clone())),
        Some(_) => Err(param_err(
            "invalidParam",
            &format!("{what}段不是叶子（产品 getLeafExpression 抛 invalidParam）"),
        )),
    }
}

/// `ParamInfo2.parse(param, "select", has1=true, has2=false)`
/// （`ParamInfo2.java:29-113`）—— `;` 第二段的排序键/次序对解码。
///
/// 产品 `DSSelect.java:51-65` 只用它解析 sort 段：`exps1` = 排序键、
/// `exps2` = 次序表达式（`None` = 缺省升序）。本函数返回对齐的两个 Vec。
fn param_info2(param: &Param) -> Result<(Vec<Expr>, Vec<Option<Expr>>)> {
    match param.kind() {
        // `,` 层：逐子段（`:37-77`）
        ParamKind::Comma => {
            let n = param.sub_size();
            let mut exps1 = Vec::with_capacity(n);
            let mut exps2 = Vec::with_capacity(n);
            for i in 0..n {
                // sub == null ⇒ has1=true ⇒ invalidParam（`:46-49`）
                let sub = param
                    .sub(i)
                    .ok_or_else(|| param_err("invalidParam", "排序段含空子段"))?;
                if sub.is_leaf() {
                    // 叶子且 has2=false ⇒ 只有排序键、次序缺省升序（`:50-56`）
                    exps1.push(sub.leaf_expr().expect("叶子").clone());
                    exps2.push(None);
                } else {
                    if sub.sub_size() != 2 {
                        return Err(param_err("invalidParam", "排序子段不是两层"));
                    }
                    // sub(0) == null ⇒ has1 ⇒ invalidParam（`:62-70`）；
                    // `:` 节点的子段只可能是叶子（ParamParser 层级递进）
                    let s0 = sub
                        .sub(0)
                        .ok_or_else(|| param_err("invalidParam", "排序子段缺排序键"))?;
                    exps1.push(s0.leaf_expr().expect("`:` 子段是叶子").clone());
                    // sub(1) == null ⇒ has2=false ⇒ 容许（升序，`:71-76`）
                    exps2.push(
                        sub.sub(1)
                            .map(|s| s.leaf_expr().expect("`:` 子段是叶子").clone()),
                    );
                }
            }
            Ok((exps1, exps2))
        }
        // `:` 层：单个排序对（`:78-102`）
        ParamKind::Colon => {
            if param.sub_size() != 2 {
                return Err(param_err("invalidParam", "排序段不是两层"));
            }
            let s0 = param
                .sub(0)
                .ok_or_else(|| param_err("invalidParam", "排序段缺排序键"))?;
            let s1 = param
                .sub(1)
                .ok_or_else(|| param_err("invalidParam", "排序段缺次序"))?;
            Ok((
                vec![s0.leaf_expr().expect("`:` 子段是叶子").clone()],
                vec![Some(s1.leaf_expr().expect("`:` 子段是叶子").clone())],
            ))
        }
        // 叶子：单排序键、无次序（`:103-109`，has2=false 才容许）
        ParamKind::Leaf => Ok((vec![param.leaf_expr().expect("叶子").clone()], vec![None])),
        ParamKind::Semicolon => Err(param_err("invalidParam", "排序段不该再含 `;`")),
    }
}

/// `Variant2.getValue(x)` 后取整（产品 `((Number)obj).intValue()`，
/// Double 截断同构）—— 次序/限量参数的公共出口；非数值报 `paramTypeError`。
fn as_param_int(v: &Value) -> Result<i32> {
    match v {
        Value::Int(i) => Ok(*i),
        Value::Long(l) => Ok(*l as i32),
        Value::Double(d) => Ok(*d as i32),
        other => Err(param_err(
            "paramTypeError",
            &format!("期望数值，得到 {other:?}"),
        )),
    }
}

/// 参数矩阵解码（`DSSelect.java:36-145`）。
pub(crate) fn decode(c: &Call, ctx: &mut dyn EvalCtx, max_args: usize) -> Result<SelectArgs> {
    let dsn = c.qualifier.as_deref().unwrap_or_default();
    // `this.param == null` ⇒ missingParam（`:32-35`）。DS 调用空参数串时
    // `build_param` 返回 None（`ParamParser.java:39-41`）。
    let tree = c
        .param_tree
        .as_ref()
        .ok_or_else(|| param_err("missingParam", "参数为空"))?;

    // ── `;` 两层（`:43-67`）：第一段是主体，第二段是多排序规格 ──
    let (p, sort_exps, is_descs) = if tree.kind() == ParamKind::Semicolon {
        if tree.sub_size() != 2 {
            return Err(param_err("invalidParam", "`;` 段数不为 2"));
        }
        let first = tree
            .sub(0)
            .ok_or_else(|| param_err("invalidParam", "`;` 第一段为空"))?;
        let mut sort_exps = Vec::new();
        let mut is_descs = Vec::new();
        if let Some(seg) = tree.sub(1) {
            let (exps1, exps2) = param_info2(seg)?;
            for (key, ord) in exps1.into_iter().zip(exps2) {
                // 次序缺省（sub(1)==null）⇒ 升序（`:57-58` 的 continue）
                let desc = match ord {
                    None => false,
                    // `Variant2.getValue(...)`，负数 ⇒ 降序（`:58-64`）
                    Some(ord) => as_param_int(&eval_bound(&ord, dsn, ctx)?)? < 0,
                };
                sort_exps.push(key);
                is_descs.push(desc);
            }
        }
        (first, sort_exps, is_descs)
    } else {
        // `tree` 是 `&Box<Param>`（断环层）⇒ 脱一层拿 `&Param`
        (tree.as_ref(), Vec::new(), Vec::new())
    };

    // ── 主体段 p 的三种类型（`:68-145`）──
    // select_exp 各分支**无条件**赋值 ⇒ 延迟初始化（不写死 None，G2 会报
    // unused_assignments）；其余四槽保持产品初值（None / -1 / 0）。
    let select_exp;
    let mut flt_exp = None;
    let mut num: i32 = -1;
    let mut distinct_exp = None;
    let mut order = 0i32;
    match p.kind() {
        // 叶子：只有选列（`:68-69`）
        ParamKind::Leaf => {
            select_exp = Some(p.leaf_expr().expect("叶子").clone());
        }
        // `:` 两层：选列 + 次序（`:70-89`）
        ParamKind::Colon => {
            if !sort_exps.is_empty() || p.sub_size() != 2 {
                return Err(param_err("invalidParam", "`:` 与 `;` 排序不能混用"));
            }
            let sel = p
                .sub(0)
                .ok_or_else(|| param_err("invalidParam", "`:` 第一段为空"))?;
            select_exp = Some(sel.leaf_expr().expect("`:` 子段是叶子").clone());
            if let Some(ord) = p.sub(1) {
                order = as_param_int(&eval_bound(
                    ord.leaf_expr().expect("`:` 子段是叶子"),
                    dsn,
                    ctx,
                )?)?;
            }
        }
        // `,` 层：选列(+次序) + 过滤 + 限量 + 去重（`:90-145`）
        ParamKind::Comma => {
            let sub_size = p.sub_size();
            if sub_size > max_args {
                return Err(param_err("invalidParam", &format!("参数段数 > {max_args}")));
            }
            let sub0 = p
                .sub(0)
                .ok_or_else(|| param_err("invalidParam", "第一段为空"))?;
            if sub0.is_leaf() {
                select_exp = Some(sub0.leaf_expr().expect("叶子").clone());
            } else {
                // 第一段是 `:` 嵌套（`x:1, f, n, d` 混合形态，`:105-125`）
                if !sort_exps.is_empty() || sub0.sub_size() != 2 {
                    return Err(param_err("invalidParam", "第一段 `:` 形态非法"));
                }
                let sel = sub0
                    .sub(0)
                    .ok_or_else(|| param_err("invalidParam", "第一段 `:` 缺选列"))?;
                select_exp = Some(sel.leaf_expr().expect("`:` 子段是叶子").clone());
                if let Some(ord) = sub0.sub(1) {
                    order = as_param_int(&eval_bound(
                        ord.leaf_expr().expect("`:` 子段是叶子"),
                        dsn,
                        ctx,
                    )?)?;
                }
            }
            flt_exp = leaf_or_err(p.sub(1), "过滤")?;
            if sub_size > 2 {
                if let Some(np) = leaf_or_err(p.sub(2), "限量")? {
                    num = as_param_int(&eval_bound(&np, dsn, ctx)?)?;
                    // `num < 1` ⇒ invalidParam（`:137-140`）—— 0 与负数都拒
                    if num < 1 {
                        return Err(param_err("invalidParam", &format!("num={num} < 1")));
                    }
                }
            }
            if sub_size > 3 {
                distinct_exp = leaf_or_err(p.sub(3), "去重")?;
            }
        }
        ParamKind::Semicolon => unreachable!("`;` 已在上面展开"),
    }
    Ok(SelectArgs {
        select_exp: select_exp.expect("各分支都已给 select_exp 赋值"),
        flt_exp,
        num,
        distinct_exp,
        order,
        sort_exps,
        is_descs,
    })
}
