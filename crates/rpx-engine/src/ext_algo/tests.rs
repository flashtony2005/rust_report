//! 扩展算法的单测：手工搭建最小状态，对拍**探针实证**的结论
//! （设计文档 §6.3 的 `engine_ext` 受害格表、§6.4 的 `engine_merge` 观测、
//! 附件 I.3 的部分平移产物）。夹具级全网格回归待顶层 `calculate()` 接线。
//!
//! 坐标口径：`row_size`/`col_size` **含** 0 号脚手架行/列（`_$16`/`_$15`）。

use crate::arena::CellId;
use crate::error::EngineError;
use crate::heads::{HeadCell, HeadState};

use super::col::extend_col;
use super::row::extend_row;

/// 造空状态；**第一个推入的必须是 (0,0) 的 cell00**。
fn g() -> HeadState {
    HeadState::new()
}

/// 推入一格。
fn put(g: &mut HeadState, r: i32, c: i32, ext: i8, rt: i8) -> CellId {
    g.push(HeadCell::new(r, c, ext, rt))
}

/// cell00：Java 里是 `ExtExtendCell`（`ExtCellSet.java:148`）⇒ `is_extender` 必须为真，
/// 否则 head 登记守卫会把它当普通格（register 不生效、钩子不扇出）。
fn root(g: &mut HeadState) -> CellId {
    let id = put(g, 0, 0, 16, -91);
    g.cells[id.0 as usize].is_extender = true;
    id
}

/// `extend_top_head` 直击 `cell00._$5` 的受害格集合（`engine_ext` 探针实证：
/// (1,1)(2,1)(3,1) 各 `cm=1+4=5`；锚点与 cell00 排除，设计文档 §6.3 判定式）。
#[test]
fn expand_top_head_hits_top_subs_members() {
    let mut g = g();
    root(&mut g);
    let v1 = put(&mut g, 1, 1, 16, -90);
    g.set_cell_top_head(v1, CellId::ROOT).unwrap();
    let v2 = put(&mut g, 2, 1, 16, -90);
    g.set_cell_top_head(v2, CellId::ROOT).unwrap();
    let a = put(&mut g, 4, 1, 19, -90);
    g.set_cell_top_head(a, CellId::ROOT).unwrap();

    super::expand::expand_top_head(&mut g, a, 4);

    assert_eq!(g.cells[v1.0 as usize].col_merge, 5, "受害格 (1,1)");
    assert_eq!(g.cells[v2.0 as usize].col_merge, 5, "受害格 (2,1)");
    assert_eq!(
        g.cells[a.0 as usize].col_merge, 1,
        "⛔ 锚点被 sub1==sub 守卫挡掉"
    );
    assert_eq!(
        g.cells[CellId::ROOT.0 as usize].col_merge,
        1,
        "⛔ cell00 被列整块包含失败挡掉"
    );
}

/// `engine_merge` 观测（设计文档 §6.4）：模板 4 行、`A2:A3` 合并 + 18 锚点、
/// count=3 ⇒ 计算态 8 行；三个实例 (2,1)/(4,1)/(6,1) 各占 2 行（合并跨度不分摊）。
#[test]
fn extend_row_multiplies_block_height() {
    let mut g = g();
    root(&mut g);
    for r in 1..4 {
        put(&mut g, r, 0, 16, -91);
    }
    put(&mut g, 0, 1, 16, -91);
    let a = put(&mut g, 2, 1, 18, -90);
    g.cells[a.0 as usize].row_merge = 2;
    g.fill_rect(2, 1, 4, 2, a); // 模板合并块铺满（阶段 A 行为）
    g.set_cell_left_head(a, CellId::ROOT).unwrap();
    g.set_cell_top_head(a, CellId::ROOT).unwrap();

    extend_row(&mut g, a, 3).unwrap();

    assert_eq!(g.row_size, 8, "4 + (3-1)×2");
    assert_eq!(g.ext_cell_size(a), 3);
    let m1 = g.at_pos(4, 1);
    let m2 = g.at_pos(6, 1);
    assert_eq!(g.cells[m1.0 as usize].row_merge, 2, "实例继承块高 2");
    assert_eq!(g.cells[m2.0 as usize].row_merge, 2);
    assert_eq!(g.at_pos(5, 1), m1, "铺满 (4,1)-(5,1)");
    assert_eq!(g.at_pos(7, 1), m2, "铺满 (6,1)-(7,1)");
}

/// `engine_ext` 的 18 场景：模板 6 行、18 锚点 (3,1)、count=5 ⇒ 10 行、
/// 克隆 (4,1)..(7,1)，克隆的 left/top head **原样共享**（都 = root）。
#[test]
fn extend_row_clones_band_downward() {
    let mut g = g();
    root(&mut g);
    for r in 1..6 {
        put(&mut g, r, 0, 16, -91);
    }
    for c in 1..3 {
        put(&mut g, 0, c, 16, -91);
    }
    let a = put(&mut g, 3, 1, 18, -90);
    g.set_cell_left_head(a, CellId::ROOT).unwrap();
    g.set_cell_top_head(a, CellId::ROOT).unwrap();

    extend_row(&mut g, a, 5).unwrap();

    assert_eq!(g.row_size, 10, "6 + (5-1)×1");
    assert_eq!(g.ext_cell_size(a), 5);
    for r in 4..8 {
        let cl = g.at_pos(r, 1);
        assert!(cl.is_some(), "克隆 (r,1) 应存在");
        assert_eq!(g.cells[cl.0 as usize].row, r);
        assert_eq!(
            g.cells[cl.0 as usize].left_head,
            CellId::ROOT,
            "head 原样共享"
        );
        assert!(g.at_pos(r, 0).is_some(), "第 0 列脚手架同步克隆");
    }
}

/// 钉子：`extendRow` 的 n 个克隆必须与锚点**共享同一份** `ext_cells` 分配。
///
/// 变异即红：把 `HeadState::set_ext_cells` 改回 `members.to_vec()`（每克隆一份
/// 长度 n 的数组），`Arc::ptr_eq` 立刻为假。这正是 50k 明细 ≈ 9.9 GB
/// （n²×4 字节）的病根——`scripts/bench_detail_mem.py` 实测峰值
/// 10k/20k/50k = 474/1693/9900 MB，与 n²×4 预测比值收敛到 1.04。
#[test]
fn extend_row_shares_one_ext_cells_allocation() {
    let mut g = g();
    root(&mut g);
    for r in 1..6 {
        put(&mut g, r, 0, 16, -91);
    }
    for c in 1..3 {
        put(&mut g, 0, c, 16, -91);
    }
    let a = put(&mut g, 3, 1, 18, -90);
    g.set_cell_left_head(a, CellId::ROOT).unwrap();
    g.set_cell_top_head(a, CellId::ROOT).unwrap();

    extend_row(&mut g, a, 6).unwrap();

    let anchor_ext = g.cells[a.0 as usize]
        .ext_cells
        .clone()
        .expect("锚点必须有 ext_cells");
    assert_eq!(anchor_ext.len(), 6, "共享数组长度 = count");
    for r in 4..9 {
        let cl = g.at_pos(r, 1);
        assert!(cl.is_some(), "克隆 ({r},1) 应存在");
        let cl_ext = g.cells[cl.0 as usize]
            .ext_cells
            .clone()
            .expect("克隆必须有 ext_cells");
        assert!(
            std::sync::Arc::ptr_eq(&anchor_ext, &cl_ext),
            "克隆 ({r},1) 必须与锚点共享同一份 ext_cells 分配（O(n)）；\
             逐格拷贝会退化为 n²×4 字节（50k 行 ≈ 9.9 GB）"
        );
    }
}

/// 第 2 趟的 leftHead **平移**：锚点克隆 (3,1) 存在后，b 的克隆 (3,2) 的
/// left_head 必须指向锚点克隆而不是原锚点（`ExtCellSet.java:1044-1048`）。
#[test]
fn extend_row_shifts_left_head_of_clones() {
    let mut g = g();
    root(&mut g);
    for r in 1..5 {
        put(&mut g, r, 0, 16, -91);
    }
    put(&mut g, 0, 1, 16, -91);
    put(&mut g, 0, 2, 16, -91);
    let a = put(&mut g, 2, 1, 18, -90);
    g.set_cell_left_head(a, CellId::ROOT).unwrap();
    g.set_cell_top_head(a, CellId::ROOT).unwrap();
    let b = put(&mut g, 2, 2, 16, -90);
    g.set_cell_left_head(b, a).unwrap();

    extend_row(&mut g, a, 2).unwrap();

    let a1 = g.at_pos(3, 1);
    let b1 = g.at_pos(3, 2);
    assert!(a1.is_some() && b1.is_some());
    assert_eq!(
        g.cells[b1.0 as usize].left_head, a1,
        "leftHead 平移到锚点克隆"
    );
}

/// `engine_ext` 的 19 场景端到端：带内部分平移（B1 类格被挤到 +4 列）、
/// 第 0 行列头克隆、受害格 cm=5、锚点克隆 (4,2)..(4,5)。
#[test]
fn extend_col_shifts_band_and_clones_anchor() {
    let mut g = g();
    root(&mut g);
    for r in 1..6 {
        put(&mut g, r, 0, 16, -91);
    }
    for c in 1..4 {
        put(&mut g, 0, c, 16, -91);
    }
    let v1 = put(&mut g, 1, 1, 16, -90);
    g.set_cell_top_head(v1, CellId::ROOT).unwrap();
    let v2 = put(&mut g, 2, 1, 16, -90);
    g.set_cell_top_head(v2, CellId::ROOT).unwrap();
    let a = put(&mut g, 4, 1, 19, -90);
    g.set_cell_left_head(a, CellId::ROOT).unwrap();
    g.set_cell_top_head(a, CellId::ROOT).unwrap();
    let b1 = put(&mut g, 4, 2, 16, 0); // 锚点右邻普通格：带内平移对象
    let _b2 = put(&mut g, 4, 3, 16, 0);

    extend_col(&mut g, a, 5).unwrap();

    assert_eq!(g.col_size, 8, "4 + (5-1)×1");
    assert_eq!(g.at_pos(4, 6), b1, "带内行 +4 列平移");
    assert_eq!(g.cells[b1.0 as usize].col, 6, "字段同步 addCol");
    assert!(
        g.at_pos(0, 2).is_some() && g.at_pos(0, 5).is_some(),
        "第 0 行列头克隆"
    );
    assert_eq!(g.cells[v1.0 as usize].col_merge, 5, "受害格撑大");
    assert_eq!(g.ext_cell_size(a), 5);
    // 钉子（列向同规）：锚点克隆必须共享同一份 `ext_cells` 分配，不得逐格拷贝。
    let anchor_ext = g.cells[a.0 as usize]
        .ext_cells
        .clone()
        .expect("锚点必须有 ext_cells");
    for c in 2..6 {
        let cl = g.at_pos(4, c);
        assert!(cl.is_some(), "锚点克隆 (4,{c}) 应存在");
        assert_eq!(g.cells[cl.0 as usize].col, c);
        assert_eq!(g.cells[cl.0 as usize].left_head, CellId::ROOT);
        let cl_ext = g.cells[cl.0 as usize]
            .ext_cells
            .clone()
            .expect("克隆必须有 ext_cells");
        assert!(
            std::sync::Arc::ptr_eq(&anchor_ext, &cl_ext),
            "列克隆 (4,{c}) 必须与锚点共享同一份 ext_cells 分配"
        );
    }
}

/// 前置校验（附件 I.6.1）：锚点行不是行头行 ⇒ `EntendCol`。
#[test]
fn extend_col_precheck_rejects_non_header_rows() {
    let mut g = g();
    root(&mut g);
    put(&mut g, 1, 0, 16, -96); // 表头行类型：不在行头族
    let a = put(&mut g, 1, 1, 19, -96);
    let err = extend_col(&mut g, a, 2).unwrap_err();
    assert!(matches!(err, EngineError::EntendCol { row: 1 }));
}

/// head 写入的登记语义（模块文档硬约束 1）：自指报 CircularRef；
/// 扩展格 head 接收登记；普通 head 不登记。
#[test]
fn set_cell_left_head_registers_into_extender_only() {
    let mut g = g();
    root(&mut g);
    let a = put(&mut g, 1, 1, 16, -90);
    assert!(matches!(
        g.set_cell_left_head(a, a),
        Err(EngineError::CircularRef { .. })
    ));
    g.set_cell_left_head(a, CellId::ROOT).unwrap();
    assert!(g.cells[CellId::ROOT.0 as usize].left_subs.contains(&a));
    let b = put(&mut g, 2, 1, 16, -90); // 普通 head
    g.set_cell_left_head(a, b).unwrap();
    assert!(
        !g.cells[b.0 as usize].left_subs.contains(&a),
        "⛔ 普通 head 不接收登记（基类空实现）"
    );
}
