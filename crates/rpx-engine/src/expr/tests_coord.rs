//! 坐标语言的求值单测 —— 手工 head 图 + 内存数据集。
//!
//! # 为什么用假网格而不是真 `stage_a/stage_b`
//!
//! 坐标的语义**全部落在 head 图上**（`instances_of` 走主格子格列表、管辖范围走
//! `common_left_head`、`&` 走 `ext_cells`），与扩展算法的行列搬运无关。手工搭 head 图
//! 能逐条对准教程的说明，也能把"哪条链在起作用"写进断言 —— 与 `tests_eval.rs` 的
//! CSVariable 用例同一手法。
//!
//! # 模型（教程 `058_fzhz128` 的 部门 → 学历 → 姓名 结构）
//!
//! ```text
//! A2 部门（2 组）        B2 学历（每部门 2 组 ⇒ 共 4 组）      D2 工资（被动复制，1 行/组）
//! a2_1 ──┬── b2_1 ── d2_1 = 1000
//!        └── b2_2 ── d2_2 = 2000
//! a2_2 ──┬── b2_3 ── d2_3 = 3000
//!        └── b2_4 ── d2_4 = 4000
//! ```
//!
//! `D3`（左主格 = B2）、`D4`（左主格 = A2）、`D5`（左主格 = root）三格的
//! `=sum(D2{})` 必须给出**三个不同的值** —— 这是教程 `058_fzhz128` 的核心判据。
//!
//! 事实源：`http://d.raqsoft.com.cn:6999/report/tutorial/` 的
//! `055_ljbb117`（位移）、`056_bzbb121`（绝对层次坐标）、`057_tqb124`（位移坐标）、
//! `058_fzhz128`（格集）、`059_tjhz131`（条件）、`061_pm137`（根坐标 + 排名）、
//! `063_xh141`（`&`）。

use crate::arena::CellId;
use crate::expr::eval::evaluate;
use crate::expr::parse;
use crate::value::Value;

use super::tests_eval::Fake;

/// 一整套假网格 + 常用句柄。
struct Grid {
    f: Fake,
    /// `A2`（部门）的 2 个实例。
    a2i: [CellId; 2],
    /// `B2`（学历）的 4 个实例。
    b2i: [CellId; 4],
    /// `D2`（工资，被动复制）的 4 个实例。
    d2i: [CellId; 4],
    /// `D3`（左主格 = B2）的 4 个实例。
    d3i: [CellId; 4],
    /// `D4`（左主格 = A2）的 2 个实例。
    d4i: [CellId; 2],
    /// `D5`（左主格 = root）的 1 个实例。
    d5: CellId,
}

const WAGE: [i32; 4] = [1000, 2000, 3000, 4000];
/// B2 的值刻意与行号错位，用来分辨"序号"与"值"（`061_pm137` 的排名要的就是这个）。
const DEGREE: [i32; 4] = [100, 300, 200, 400];

fn grid() -> Grid {
    let mut f = Fake::new();
    // ── 引擎矩阵本体格（模板）──
    let a2 = f.put("A2", 18, Value::Null);
    let b2 = f.put("B2", 18, Value::Null);
    let d2 = f.put("D2", 16, Value::Null);
    let d3 = f.put("D3", 16, Value::Null);
    let d4 = f.put("D4", 16, Value::Null);
    let d5 = f.put("D5", 16, Value::Null);
    // 模板态的 head 链（归一化后的形态：只指向 ExtExtendCell）
    f.set_left_head(b2, a2);
    f.set_left_head(d2, b2);
    f.set_left_head(d3, b2);
    f.set_left_head(d4, a2);
    f.set_left_head(d5, CellId::ROOT);
    f.reg_left(a2, b2);
    f.reg_left(b2, d2);
    f.reg_left(b2, d3);
    f.reg_left(a2, d4);

    // ── A2 的两个实例 ──
    let mut a2i = [CellId::NONE; 2];
    for slot in a2i.iter_mut() {
        *slot = f.put_inst(a2, 18, Value::Null);
        f.add_ext(a2, *slot);
    }
    // ── B2 的四个实例（每部门 2 个）──
    let mut b2i = [CellId::NONE; 4];
    for i in 0..4 {
        b2i[i] = f.put_inst(b2, 18, Value::Int(DEGREE[i]));
        f.add_ext(b2, b2i[i]);
        f.set_left_head(b2i[i], a2i[i / 2]);
        f.reg_left(a2i[i / 2], b2i[i]);
    }
    // ── D2 的四个实例（被动复制：`_$7` 不填，靠主格子格列表展开）──
    let mut d2i = [CellId::NONE; 4];
    for i in 0..4 {
        d2i[i] = f.put_inst(d2, 16, Value::Int(WAGE[i]));
        f.set_left_head(d2i[i], b2i[i]);
        f.reg_left(b2i[i], d2i[i]);
    }
    // ── D3 随 B2 扩展（4 个实例）──
    let mut d3i = [CellId::NONE; 4];
    for i in 0..4 {
        d3i[i] = f.put_inst(d3, 16, Value::Null);
        f.set_left_head(d3i[i], b2i[i]);
        f.reg_left(b2i[i], d3i[i]);
    }
    // ── D4 随 A2 扩展（2 个实例）──
    let mut d4i = [CellId::NONE; 2];
    for i in 0..2 {
        d4i[i] = f.put_inst(d4, 16, Value::Null);
        f.set_left_head(d4i[i], a2i[i]);
        f.reg_left(a2i[i], d4i[i]);
    }
    // ── D5 不随任何格扩展（1 个实例，左主格 = root）──
    let d5i = f.put_inst(d5, 16, Value::Null);
    f.set_left_head(d5i, CellId::ROOT);

    Grid {
        f,
        a2i,
        b2i,
        d2i,
        d3i,
        d4i,
        d5: d5i,
    }
}

/// 在 `home` 格里求值一条表达式。
///
/// ⛔ 用宏而不是函数调用：`at(&mut g, g.d3i[0], ..)` 会同时可变借用 `g`
/// 与读取 `g.d3i[0]`（E0503）。宏先把 home 拷出来（`CellId` 是 `Copy`）再借用。
macro_rules! ev {
    ($g:expr, $home:expr, $src:expr) => {{
        let home = $home;
        at(&mut $g, home, $src)
    }};
}

fn at(g: &mut Grid, home: CellId, src: &str) -> Value {
    g.f.set_cur(home);
    let e = parse(src).unwrap_or_else(|e| panic!("解析 `{src}` 失败：{e}"));
    evaluate(&e, &mut g.f).unwrap_or_else(|e| panic!("求值 `{src}` 失败：{e}"))
}

fn ints(v: &Value) -> Vec<i32> {
    match v {
        Value::List(items) => items
            .iter()
            .map(|x| match x {
                Value::Int(n) => *n,
                other => panic!("列表里出现非 Int：{other:?}"),
            })
            .collect(),
        other => panic!("期望 List，实得 {other:?}"),
    }
}

/// ⛔ **核心判据**（教程 `058_fzhz128`）：同一段 `=sum(D2{})` 写在三个位置 ⇒ 三个值。
///
/// * `D3` 随 **B2**（学历）⇒ 该学历下的工资之和；
/// * `D4` 随 **A2**（部门）⇒ 该部门下的工资之和；
/// * `D5` 固定 ⇒ 全体之和。
///
/// 「管辖范围由**写表达式的那个格**决定」这条如果实现错了（比如把管辖范围写成
/// 目标的自身主格、或写成 root），三格会退化成一两个值 —— 这正是本用例的判别力。
#[test]
fn empty_braces_jurisdiction_follows_the_writing_cell() {
    let mut g = grid();
    // D3_1 挂在 b2_1 下 ⇒ 只有 d2_1
    assert_eq!(ev!(g, g.d3i[0], "sum(D2{})"), Value::Int(1000));
    // D3_3 挂在 b2_3 下 ⇒ 只有 d2_3
    assert_eq!(ev!(g, g.d3i[2], "sum(D2{})"), Value::Int(3000));
    // D4_1 挂在 a2_1 下 ⇒ d2_1 + d2_2
    assert_eq!(ev!(g, g.d4i[0], "sum(D2{})"), Value::Int(3000));
    // D4_2 挂在 a2_2 下 ⇒ d2_3 + d2_4
    assert_eq!(ev!(g, g.d4i[1], "sum(D2{})"), Value::Int(7000));
    // D5 挂在 root 下 ⇒ 全体
    assert_eq!(ev!(g, g.d5, "sum(D2{})"), Value::Int(10000));
    // 三格三个值，且都非零（零意味着空 `{}` 被当成"恒假条件"处理了）
    let three = [
        ev!(g, g.d3i[0], "sum(D2{})"),
        ev!(g, g.d4i[0], "sum(D2{})"),
        ev!(g, g.d5, "sum(D2{})"),
    ];
    assert_ne!(three[0], three[1]);
    assert_ne!(three[1], three[2]);
}

/// 条件过滤（教程 `059_tjhz131` 的 `count(D2{D2>10000})` 形态）。
///
/// ⚠ 本仓 `count()` **未实现**（`eval.rs` 只分发 `sum/avg/max/min`），所以观测口
/// 换成 `sum(...)` —— 过滤本身是可断言的部分。教程原式里的 `count` 见
/// `docs/report-def-json.md` 的未实现清单。
#[test]
fn a_condition_filters_the_set() {
    let mut g = grid();
    // 全体中大于 1500 的：2000 + 3000 + 4000
    assert_eq!(ev!(g, g.d5, "sum(D2{D2>1500})"), Value::Int(9000));
    // 部门 1 内大于 1500 的：2000
    assert_eq!(ev!(g, g.d4i[0], "sum(D2{D2>1500})"), Value::Int(2000));
    // 条件为真则全集、为假则空集（`sum(空)` = Double(0.0)，`Sum.java:111-113`）
    assert_eq!(ev!(g, g.d5, "sum(D2{D2>0})"), Value::Int(10000));
    assert_eq!(ev!(g, g.d5, "sum(D2{D2>9999})"), Value::Double(0.0));
}

/// 绝对层次坐标（教程 `056_bzbb121` 的 `=C2[1]-C2`）：`[n]` 取**第 n 个实例**。
#[test]
fn absolute_index_picks_the_nth_instance() {
    let mut g = grid();
    // 写在 D3_3（当前第 3 个实例）里：`D2[1]` 恒是第 1 个实例
    assert_eq!(ev!(g, g.d3i[2], "D2[1]"), Value::Int(1000));
    assert_eq!(ev!(g, g.d3i[2], "D2[4]"), Value::Int(4000));
    // `D2[1]-D2`：与"当前格"的差（`C2` 无主格时按当前实例解析）
    assert_eq!(ev!(g, g.d2i[2], "D2[1]-D2"), Value::Int(-2000));
    // 越界 ⇒ 显式报错（不静默给 Null）
    g.f.set_cur(g.d5);
    let err = evaluate(&parse("D2[9]").unwrap(), &mut g.f).unwrap_err();
    assert!(err.to_string().contains("第 9 个实例不存在"), "{err}");
}

/// 位移坐标（教程 `057_tqb124` 的 `=C2/C2[-1]`、`055_ljbb117` 的 `=D2+E2[-1]`）。
///
/// ⛔ 判据的要点是**被动复制格**（`D2` 的 `_$7` 是空的）也能拿到实例序列 ——
/// 只认 `ext_cells` 的实现会在这里静默越界。
#[test]
fn signed_offset_moves_along_the_instance_sequence() {
    let mut g = grid();
    // 当前 D2_3 ⇒ 上一个实例是 D2_2
    assert_eq!(ev!(g, g.d2i[2], "D2[-1]"), Value::Int(2000));
    assert_eq!(ev!(g, g.d2i[2], "D2[-2]"), Value::Int(1000));
    assert_eq!(ev!(g, g.d2i[0], "D2[+1]"), Value::Int(2000));
    // `=D2+E2[-1]` 的累积形态：本行 + 上一行
    assert_eq!(ev!(g, g.d2i[1], "D2+D2[-1]"), Value::Int(3000));
    // 首行往上位移 ⇒ 越界显式报错
    g.f.set_cur(g.d2i[0]);
    assert_eq!(
        evaluate(&parse("D2[-1]").unwrap(), &mut g.f).unwrap(),
        Value::Null
    );
}

/// 根坐标（教程 `061_pm137`）：`` D2[`0]{} `` = 00 格下的**全部**实例，
/// 与写在哪一格无关。
#[test]
fn root_coordinate_spans_every_instance_regardless_of_home_cell() {
    let mut g = grid();
    for home in [g.d3i[0], g.d4i[1], g.d5] {
        assert_eq!(
            ev!(g, home, "sum(D2[`0]{})"),
            Value::Int(10000),
            "根坐标必须无视写在哪一格"
        );
    }
    // 对照：不带 `[`0]` 的 `D2{}` 随宿主格变（上面那条已断言）
    assert_eq!(ev!(g, g.d3i[0], "sum(D2[`0]{D2>1500})"), Value::Int(9000));
}

/// 2010 设计文档的 `Cellx{00}` 记号 = `` Cellx[`0]{} ``（兼容别名，见 `coord.rs`）。
#[test]
fn design_doc_root_alias_equals_the_product_notation() {
    let mut g = grid();
    assert_eq!(ev!(g, g.d3i[0], "sum(D2{00})"), Value::Int(10000));
    assert_eq!(ev!(g, g.d3i[0], "sum(D2[`0]{})"), Value::Int(10000));
}

/// `&Cellx`（教程 `063_xh141`）：当前格所属的 Cellx 主格在**所有**实例里排第几。
#[test]
fn address_operator_returns_the_flat_ordinal() {
    let mut g = grid();
    // D2_3 挂在 b2_3（第 3 个 B2 实例）、a2_2（第 2 个 A2 实例）之下
    assert_eq!(ev!(g, g.d2i[2], "&B2"), Value::Int(1));
    assert_eq!(ev!(g, g.d2i[2], "&A2"), Value::Int(2));
    // 组内序号（`A2: =&C2` 的形态）：D3_1 挂在 b2_1 下 ⇒ 1
    assert_eq!(ev!(g, g.d3i[0], "&B2"), Value::Int(1));
    // 当前格自己就是该主格的实例（`A3: =&A3`）⇒ 自身下标
    assert_eq!(ev!(g, g.b2i[3], "&B2"), Value::Int(2));
    assert_eq!(ev!(g, g.a2i[1], "&A2"), Value::Int(2));
    // ⛔ 目标不是当前格的主格 ⇒ 显式报错（教程：「Cellx 必须是当前格的主格」）
    g.f.set_cur(g.d2i[0]);
    let err = evaluate(&parse("&A1").unwrap(), &mut g.f).unwrap_err();
    assert!(err.to_string().contains("unknown cell: A1"), "{err}");
}

/// 坐标里带主格序号（教程 `058_fzhz128` 的 `C2[A2:2]{}` / `D2[A2:1,B2:2]{}`）。
#[test]
fn master_ordinal_narrows_the_set() {
    let mut g = grid();
    // A2:2 ⇒ 第二个部门下的全部 D2
    assert_eq!(ev!(g, g.d5, "sum(D2[A2:2]{})"), Value::Int(7000));
    assert_eq!(ev!(g, g.d5, "sum(D2[A2:1]{})"), Value::Int(3000));
    // A2:1, B2:2 ⇒ 第一个部门下第二个学历 ⇒ 单格
    assert_eq!(ev!(g, g.d5, "D2[A2:1,B2:2]"), Value::Int(2000));
    assert_eq!(ev!(g, g.d5, "sum(D2[A2:1,B2:2]{})"), Value::Int(2000));
    // 缺省主格 `D2[A2]` ⇒ 当前格所属的那个 A2（`061_pm137` 的 `C2[A2]{C2>$C2}`）
    assert_eq!(ev!(g, g.d4i[1], "sum(D2[A2]{})"), Value::Int(7000));
    assert_eq!(ev!(g, g.d4i[0], "sum(D2[A2]{})"), Value::Int(3000));
}

/// `$Cellx` 在格集条件里指代**当前格**的该主格（教程 `060_fztqb134`）。
///
/// 判据 = 排名：`sum(B2[`0]{B2>$B2})` 在"B2 值 = 300"的那一格上，
/// 结果是**大于 300 的那些 B2 值之和** = 400。若 `$` 被误当成目标格（而不是当前格），
/// 条件会退化成 `B2>B2`（恒假）⇒ 结果 0。
#[test]
fn dollar_in_a_condition_refers_to_the_home_cell() {
    let mut g = grid();
    // b2_2 的值是 300；比它大的只有 b2_4（400）
    assert_eq!(ev!(g, g.b2i[1], "sum(B2[`0]{B2>$B2})"), Value::Int(400));
    // b2_1 的值是 100；比它大的有 300/200/400
    assert_eq!(ev!(g, g.b2i[0], "sum(B2[`0]{B2>$B2})"), Value::Int(900));
    // 最大值那格 ⇒ 空集 ⇒ `sum` 的空集语义 `Double(0.0)`（`Sum.java:111-113`）
    assert_eq!(ev!(g, g.b2i[3], "sum(B2[`0]{B2>$B2})"), Value::Double(0.0));
}

/// 带主格的**位移**（教程 `057_tqb124` 的 `C2[A2:-1]` 形态）：
/// 偏移量算在当前格所属的该主格上，不是算在目标格自己身上。
#[test]
fn master_offset_is_relative_to_the_home_cells_master() {
    let mut g = grid();
    // D4_2 所属的 A2 是第 2 个 ⇒ A2:-1 ⇒ 第一个部门 ⇒ d2_1 + d2_2
    assert_eq!(ev!(g, g.d4i[1], "sum(D2[A2:-1]{})"), Value::Int(3000));
    // D4_1 所属的 A2 是第 1 个 ⇒ 再往前越界
    assert_eq!(ev!(g, g.d4i[0], "sum(D2[A2:-1]{})"), Value::Double(0.0));
}

/// 格集按**坐标界定顺序**返回（教程 `058_fzhz128` 的 `C2[`0]{}` 明确列出 c2..c12）。
///
/// 顺序错了 `sum` 看不出来（加法可交换），但 `list`/`first` 与后续的
/// 「定位到多格取第一个」会错 —— 所以单列一条。
#[test]
fn a_cell_set_keeps_the_tree_order() {
    let mut g = grid();
    assert_eq!(ints(&ev!(g, g.d5, "D2[`0]{}")), WAGE.to_vec());
    assert_eq!(ints(&ev!(g, g.d5, "D2[A2:2]{}")), vec![3000, 4000]);
    assert_eq!(ints(&ev!(g, g.d3i[0], "D2{}")), vec![1000]);
}
