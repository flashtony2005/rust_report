//! 坐标语言 —— 层次坐标 / 位移坐标 / 根坐标 / 格集管辖范围 / `&` 位置运算。
//!
//! **事实源是润乾官方教程**（`http://d.raqsoft.com.cn:6999/report/tutorial/`，
//! 全量 91 页抓取后逐页核对；落盘 `/tmp/rq_tut/pages/`）。本文的每条语义都点名页码，
//! 不引"设计文档"—— 2010 年的 `报表引擎设计.md` 把两对括号写反了（见 §记号）。
//!
//! # 记号（⛔ 这是全项目最容易记反的一处）
//!
//! ```text
//! Cellx[层次坐标或位移坐标]{条件}      ← 产品 / 官方教程
//! Cellx{坐标}[EXP]                     ← 2010 设计文档（**写反了**）
//! ```
//!
//! 原始 HTML（非 OCR）铁证（`061_pm137`）：
//! `` =count(B2[`0]{B2>$B2})+1 ``、`=count(C2[A2]{C2>$C2})+1`；
//! `058_fzhz128`：`C2[`0]{}` / `C2[A2:2]{}` / `D2[A2:1,B2:2]{}`；
//! `059_tjhz131`：`count(D2{D2>10000})`（无中括号 = 缺省坐标 + 条件）。
//! 产品侧同结论：`Select{…}` 与 `Move x[…]` 都是**后缀**、优先级 17
//! （`docs/m2-engine-design.md` 的优先级表，`Select.java:26` / `Move.java:54`）。
//!
//! # 四个原语
//!
//! | 原语 | 记号 | 语义 | 教程 |
//! |------|------|------|------|
//! | 层次坐标（绝对） | `Cellx[Lk:lk, ...; Tk:tk, ...]` | 树上一条**路径** | `056_bzbb121` |
//! | 位移坐标（相对） | `Cellx[Lk:±lk, ...]` / `cell[±n]` | 相对当前格的**偏移** | `057_tqb124`、`055_ljbb117` |
//! | 格集 / 条件 | `Cellx[坐标]{条件}` | 坐标界定的集合**再过滤** | `058_fzhz128`、`059_tjhz131` |
//! | 根坐标 | `` `0 ``（设计文档写 `{00}`） | 00 格下的**全部**目标实例 | `061_pm137` |
//! | `&` 位置运算 | `&Cellx` | 层次坐标的**逆运算**：排第几 | `063_xh141` |
//!
//! # 五条不能"顺手统一"的原文口径
//!
//! 1. **序号是"下属"口径**：`C1[A1:2,B1:1]` = 「先找 A1 的第 2 格，再找**第二个 A1 下属**的
//!    B1 的第 1 个」（`056_bzbb121` 说明 c）。⇒ 本模块的序号在**父节点子树内**计。
//!    而 `&Cellx` 依教程 `063_xh141` 的「组内序号」⇒ **在同一父主格实例内**计。
//!    `seq(Cellx)` 则按全部同源实例连续计数。
//! 2. **`+` 向下/右、`-` 向上/左**（`057_tqb124` 说明）；`cell[±n]` 是"没有复杂主格关系"
//!    时的简写，`[1]`（无符号）是**绝对序号**、`[-1]`（带符号）是**位移**。
//!    本模块按"是不是一元 `+`/`-` 节点"判，与产品的树形一致。
//! 3. **缺省坐标**：`lk` 不写或为 0 ⇒ 当前表达式所在格所属的那个主格实例（`056` 说明 a）。
//! 4. **`{}` 缺省管辖范围 = 当前格与目标格的共同主格**（`058_fzhz128` 的三连：
//!    D3 随 B2 ⇒ 每组学历；D4 随 A2 ⇒ 每组部门；D5 固定 ⇒ 全体）。本模块用
//!    [`super::resolve`] 的 `common_left_head`/`common_top_head` 求它 —— 与
//!    `CSVariable` 用的是同一对原语，不是另起一套。
//! 5. **定位到多格 ⇒ 返回第一个**（`056_bzbb121`：「如果定位到多个单元格，
//!    那么返回这些单元格中的第一个单元格的值」）；`{}` 才返回整个集合。
//!
//! # 与产品的已知差异（诚实清单）
//!
//! - `?Cellx`（消歧义运算符，`060_fztqb134`）**未实现**；`$Cellx` 已可用，
//!   因为它就是既有的 `$` 绝对引用（`resolve_cell(.., saved=true)`）——
//!   `{}` 求值期 `saveCurrent()` 已把 `_$20` 钉在写表达式的那个格上。
//! - `seq()`（同源号）未实现；它要"同源格"概念，属另一条线。
//! - 目标格不是**格号**的坐标（如 `ds1.select(x)[1]`）在语法层显式拒绝。
//! - 被动复制格的实例序列靠**主格子格列表**（`left_subs`/`top_subs`）展开；
//!   若某格既非主动扩展格、其主格子格列表里也没有它（理论不可达），回退成 `[自身]`。

mod parse;

pub use parse::parse_coord;

use crate::arena::CellId;
use crate::error::{EngineError, Result};
use crate::expr::eval::EvalCtx;
use crate::expr::resolve::{common_left_head, common_top_head, resolve_cell, walk_to_source};
use crate::value::Value;

/// 链遍历防御上限；与 `resolve.rs`/`normalize.rs` 同值。
const CHAIN_LIMIT: usize = 4096;

/// 一个坐标段的值（`056_bzbb121` 的 `lk` / `057_tqb124` 的 `±lk`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordVal {
    /// 缺省或 `0` —— **当前格所属的该主格实例**（`056` 说明 a）。
    Current,
    /// 绝对序号，1 起（`C2[1]`、`C2[A2:2]`）。
    Index(i64),
    /// 相对偏移（`C2[-1]`、`C2[A2:+1]`）。`+` = 下/右，`-` = 上/左。
    Offset(i64),
    /// 根坐标 `` `0 ``（`061_pm137`）—— 从**根格**起算。
    Root,
}

/// 一个坐标段：`主格:值`（主格可缺省）。
#[derive(Debug, Clone, PartialEq)]
pub struct CoordSeg {
    /// 主格格号；`None` = 缺省，作用在**目标格自己**的实例序列上。
    pub master: Option<String>,
    /// 序号 / 偏移 / 根。
    pub value: CoordVal,
}

/// `[...]` 里解析出的完整坐标。
///
/// `left` = 纵展（左主格）链，`top` = 横伸（上主格）链，两边都**从远到近**
/// （`056_bzbb121` 说明 c：越上级的主格越靠前）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Coord {
    /// 纵展链。
    pub left: Vec<CoordSeg>,
    /// 横伸链。
    pub top: Vec<CoordSeg>,
    /// 是否显式出现了 `;` —— 只有上主格时**分号不可省**（`056` 说明 b），
    /// 所以这个标志决定 `[T2:1]`（缺分号、被当纵展链）是否要报错。
    pub has_semi: bool,
}

/// `` `0 `` —— 根坐标的原文记号（`061_pm137`：「而 `` `0 `` 则代表根坐标」）。
///
/// ⛔ 反引号**不在** `Expression.scanId` 的分隔符集合里（`Expression.java:74`，
/// 见 `token.rs` 的 `is_delimiter`）⇒ 它天然被吃成一个 `Word`，本函数只是认形状。
/// 因此**不需要动词法层**。
pub fn is_root_marker(w: &str) -> bool {
    w == "`0"
}

/// `{00}` —— 2010 设计文档的根坐标记号（`报表引擎设计.md` §四 根坐标表示法）。
///
/// 保留为**兼容别名**：教程/产品的真记号是 `` Cellx[`0] ``，设计文档写成
/// `Cellx{00}`。两者语义相同（00 格下的全部目标实例），但花括号在教程里装的是
/// **条件** ⇒ 本别名只在「花括号内容恰好是 `00`」时生效，且**必须紧跟在格号后面**。
/// 这是有意的窄口径：`{0}` / `{00+0}` 都不认，避免把合法条件吞掉。
pub fn is_root_alias(w: &str) -> bool {
    w == "00"
}

// ─────────────────────────── 求值：坐标 → 格集 ───────────────────────────

/// 目标格 `src` 的**实例序列**（树序）。
///
/// - **主动扩展格**（`_$7` 已填）⇒ 直接用它（`ExtExtendCell.extCellSize`）；
/// - **被动复制格** ⇒ 沿**最近主格**展开：主格的每个实例 `m`，在 `m` 的
///   子格列表里找 `source_of == src` 的那一个。该登记是 `setCellLeftHead` 的
///   副作用（`ExtExtendCell.java:357-367`，本仓 `heads/grid.rs` 的 `set_left_head`），
///   扩展算法的第 2 趟也会给克隆登记（`ext_algo/row.rs:108`）。
///
/// ⛔ 不能拿 `ext_cells` 当通解：`_$7` **只填在锚点上**（`row.rs:30/72-73`），
/// 被动格（如 `055_ljbb117` 的 `E2`）读到的是 `[自身]` ⇒ `E2[-1]` 会静默越界。
pub fn instances_of(src: CellId, ctx: &dyn EvalCtx) -> Vec<CellId> {
    fn collect(src: CellId, ctx: &dyn EvalCtx, depth: usize) -> Vec<CellId> {
        if depth > CHAIN_LIMIT {
            return Vec::new();
        }
        for top in [false, true] {
            let head = if top {
                ctx.top_head(src)
            } else {
                ctx.left_head(src)
            };
            if head.is_none() || head == ctx.root() {
                continue;
            }
            let mut out = Vec::new();
            for parent in collect(ctx.source_of(head), ctx, depth + 1) {
                let children = if top {
                    ctx.top_subs(parent)
                } else {
                    ctx.left_subs(parent)
                };
                for child in children {
                    if ctx.source_of(child) != src {
                        continue;
                    }
                    let ext = ctx.ext_cells(child);
                    let members: Vec<CellId> = if ext.is_empty() {
                        vec![child]
                    } else {
                        ext.iter().copied().collect()
                    };
                    for c in members {
                        if ctx.source_of(c) == src && !out.contains(&c) {
                            out.push(c);
                        }
                    }
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
        let direct = ctx.ext_cells(src);
        if direct.is_empty() {
            vec![src]
        } else {
            direct.iter().copied().collect()
        }
    }
    collect(src, ctx, 0)
}

/// `c` 是否挂在节点 `node` 的同一轴子树下（`node` 为 [`CellId::NONE`] ⇒ 不设限）。
///
/// 实现 = 沿该轴 head 链上溯，看能不能走到 `node` 本身。`node` 取的是**实例**
/// 而不是出处，所以比较的是 `CellId` 相等（同一实例）而非 `source_of` 相等。
fn under(ctx: &dyn EvalCtx, c: CellId, node: CellId, top: bool) -> bool {
    if node.is_none() {
        return true;
    }
    let mut h = c;
    for _ in 0..CHAIN_LIMIT {
        if h.is_none() {
            return false;
        }
        if h == node {
            return true;
        }
        h = if top {
            ctx.top_head(h)
        } else {
            ctx.left_head(h)
        };
    }
    false
}

/// `{}` 的**缺省管辖节点**（`058_fzhz128` 的 D3/D4/D5 三连）。
///
/// = 当前格与目标格的**共同主格**在**当前格这一侧**的实例：
/// 左链用 `common_left_head` + `walk_to_source`，上链对称 —— 与 `CSVariable`
/// 用的是同一对原语（`resolve.rs`），不另起一套。
fn default_jurisdiction(src: CellId, ctx: &dyn EvalCtx) -> (CellId, CellId) {
    let cur = ctx.current();
    let cur_src = ctx.source_of(cur);
    let lsrc = common_left_head(ctx, src, cur_src);
    let tsrc = common_top_head(ctx, src, cur_src);
    let lnode = if lsrc.is_none() {
        ctx.root()
    } else {
        walk_to_source(ctx, cur, lsrc, false)
    };
    let tnode = if tsrc.is_none() {
        ctx.root()
    } else {
        walk_to_source(ctx, cur, tsrc, true)
    };
    (lnode, tnode)
}

/// 走一步坐标段：在 `parent` 的子树里按段选出一个节点。
///
/// * `master = Some(M)` ⇒ 池子是 **M 的实例**，基准实例由 [`resolve_cell`] 给出
///   （"当前格所属的那个 M"，与普通格引用同一条解析链）；
/// * `master = None` ⇒ 池子是**目标格自己**的实例（`C2[1]` / `C2[-1]`）；
/// * `Root` ⇒ 直接回到根格，不设限。
fn step(
    seg: &CoordSeg,
    parent: CellId,
    target: &str,
    src: CellId,
    top: bool,
    ctx: &dyn EvalCtx,
) -> Result<CellId> {
    if seg.value == CoordVal::Root {
        return Ok(ctx.root());
    }
    let (pool, base_id) = match &seg.master {
        Some(m) => {
            let msrc = ctx
                .source_by_id(m)
                .ok_or_else(|| EngineError::UnknownCell { id: m.clone() })?;
            (instances_of(msrc, ctx), m.clone())
        }
        None => (instances_of(src, ctx), target.to_string()),
    };
    let cands: Vec<CellId> = pool
        .into_iter()
        .filter(|&c| under(ctx, c, parent, top))
        .collect();
    match seg.value {
        // 当前格所属的那个实例（缺省坐标）
        CoordVal::Current => resolve_cell(ctx, &base_id, false)
            .filter(|c| cands.contains(c))
            .ok_or_else(|| missing(&base_id, "当前上下文里解析不到")),
        // 绝对序号（1 起；0 已在语法层折成 Current）
        CoordVal::Index(n) => cands
            .get(usize::try_from(n - 1).unwrap_or(usize::MAX))
            .copied()
            .ok_or_else(|| missing(&base_id, &format!("第 {n} 个实例不存在"))),
        // 位移：以"当前格所属实例"为原点
        CoordVal::Offset(d) => {
            let base = resolve_cell(ctx, &base_id, false)
                .filter(|c| cands.contains(c))
                .ok_or_else(|| missing(&base_id, "当前上下文里解析不到，无法位移"))?;
            let i = cands.iter().position(|&c| c == base).unwrap_or(0) as i64;
            Ok(cands
                .get(usize::try_from(i + d).unwrap_or(usize::MAX))
                .copied()
                .unwrap_or(CellId::NONE))
        }
        CoordVal::Root => unreachable!("Root 已在函数开头返回"),
    }
}

fn missing(id: &str, why: &str) -> EngineError {
    EngineError::UnknownCell {
        id: format!("{id}（坐标 {why}）"),
    }
}

/// 坐标 → 格集（按 [`instances_of`] 的树序）。
///
/// # 管辖范围的两条路（**互斥**）
///
/// * **`Cellx{}`（无中括号）** ⇒ 管辖范围 = [`default_jurisdiction`]
///   —— 当前格与目标格的**共同主格**（教程 `058_fzhz128` 的 D3/D4/D5 三连）。
/// * **给了中括号** ⇒ 管辖范围**完全由坐标决定**，缺省管辖范围**整条丢弃**：
///   有段的轴从**根**起走（`` `0 `` 是链首），没段的轴不设限。
///
/// ⛔ 第二条不能写成"只覆盖给了段的那个轴"：`D2[1]` 只给左链，若右链仍用缺省，
/// 就会拿到 `common_top_head` 退化成"目标格自己"（`is_top_head_of` 自含）
/// ⇒ 过滤后集合恒空 ⇒ `D2[1]` 静默变 `Null`。**实测踩过**（`tests_coord`）。
pub fn coord_cells(
    target: &str,
    src: CellId,
    coord: &Coord,
    ctx: &dyn EvalCtx,
) -> Result<Vec<CellId>> {
    let (mut lnode, mut tnode) = if coord.left.is_empty() && coord.top.is_empty() {
        default_jurisdiction(src, ctx)
    } else {
        (ctx.root(), ctx.root())
    };
    if !coord.left.is_empty() {
        let mut node = ctx.root();
        for seg in &coord.left {
            node = step(seg, node, target, src, false, ctx)?;
            if node.is_none() {
                return Ok(Vec::new());
            }
        }
        lnode = node;
    }
    if !coord.top.is_empty() {
        let mut node = ctx.root();
        for seg in &coord.top {
            node = step(seg, node, target, src, true, ctx)?;
            if node.is_none() {
                return Ok(Vec::new());
            }
        }
        tnode = node;
    }
    Ok(instances_of(src, ctx)
        .into_iter()
        .filter(|&c| under(ctx, c, lnode, false) && under(ctx, c, tnode, true))
        .collect())
}

/// `&Cellx`（`063_xh141`）—— 当前格所属的 Cellx 主格在**当前组**内排第几（1 起）。
///
/// 口径三条（逐条来自 `063_xh141` 与 `068_bzkbh146`）：
/// * 「Cellx 必须是当前格的主格」⇒ 从当前格沿 head 链上溯找 `source_of == Cellx` 的节点；
/// * 「在**所有**扩展出来的格子中的排序」⇒ 序号是相同父主格实例下的下标
///   （跨组连续编号使用 `seq()`）；
/// * 当前格**自己**就是 Cellx 的实例时（`A3: =&A3`）序号即自身下标。
pub fn address_of(target: &str, ctx: &dyn EvalCtx) -> Result<Value> {
    let src = ctx
        .source_by_id(target)
        .ok_or_else(|| EngineError::UnknownCell {
            id: target.to_string(),
        })?;
    let cur = ctx.current();
    let inst = ancestor_instance(ctx, cur, src).ok_or_else(|| EngineError::UnknownCell {
        id: format!("&{target}：当前格不挂在 {target} 之下（& 要求目标是当前格的主格）"),
    })?;
    let all: Vec<_> = instances_of(src, ctx)
        .into_iter()
        .filter(|&c| {
            ctx.left_head(c) == ctx.left_head(inst) && ctx.top_head(c) == ctx.top_head(inst)
        })
        .collect();
    let i = all
        .iter()
        .position(|&c| c == inst)
        .ok_or_else(|| EngineError::UnknownCell {
            id: format!("&{target}：实例不在 {target} 的扩展数组里"),
        })?;
    Ok(Value::Int(i as i32 + 1))
}

/// 从 `from` 沿**两条** head 链上溯，找 `source_of == src` 的节点（自含）。
fn ancestor_instance(ctx: &dyn EvalCtx, from: CellId, src: CellId) -> Option<CellId> {
    let mut stack = vec![from];
    let mut seen: Vec<CellId> = Vec::new();
    while let Some(c) = stack.pop() {
        if c.is_none() || seen.contains(&c) {
            continue;
        }
        seen.push(c);
        if ctx.source_of(c) == src {
            return Some(c);
        }
        stack.push(ctx.left_head(c));
        stack.push(ctx.top_head(c));
    }
    None
}

/// `` `0 `` 的坐标 —— 只含一个根段的坐标（`Cellx{00}` 兼容别名与
/// `` Cellx[`0] `` 都用它，见 [`is_root_alias`]）。
pub fn root_coord() -> Coord {
    Coord {
        left: vec![CoordSeg {
            master: None,
            value: CoordVal::Root,
        }],
        top: Vec::new(),
        has_semi: false,
    }
}
