//! 阶段 B 的夹具对拍（`docs/m2-engine-design.md` §4）。
//!
//! 三条判据，都只依赖**模板坐标**，因此不需要先做扩展：
//!
//! 1. **改写**（§4.3）：夹具每个格都带 `source` 块，其中的 `extend` 读自引擎矩阵那一格
//!    —— 而两张矩阵**指向同一批格对象**，所以它就是改写后的值。用 `source.src_row/src_col`
//!    反查模板坐标，可对**全部** 11 个网格夹具逐格比对。
//!    （⚠ 只比 `extend`：`row_merge`/`col_merge` 会被**扩展**改写，属扩展步的判据。）
//! 2. **head 真值**（§4.2/§4.4）：只在**没有被插入行列移动过**的区域比对
//!    —— `r < 最早克隆行 && c < 最早克隆列`；克隆行列由夹具自己的 `sub_values` 导出，
//!    不靠猜。移动区（克隆格、被挤到后面的本体格）的真值要等扩展算法接线后才可比，
//!    本文件**明确不覆盖**。
//! 3. **`disp_value` 继承**（判据 2b，见下）：`disp_value` 是"模板声明 + 覆盖"而不是
//!    按值合成，唯一带非空显示值的夹具是 `engine_disp`。
//!
//! 另见 [`tripwires`]：那两条不是"验证实现"，而是钉子 —— 守卫**当前无夹具可区分**
//! 这个前提一破就红。
//!
//! ⛔ 夹具的 `cells` 只收**非空格**（`CalcJson.java:51-53`），所以三条判据都只覆盖夹具
//! 列出来的格 —— 这是夹具的边界，不是实现的边界。

mod json;
mod literal;
mod tripwires;

use std::collections::HashMap;

use crate::stage_b::{cell_expression, prepare_calculate, StageB};
use json::{clone_axes, expected_text, fixtures_dir, parse_expected, template_dims};

/// 有计算网格的夹具（`engine_cycle` 是错误型夹具，无网格，另测）。
const GRID_FIXTURES: [&str; 11] = [
    "engine_disp",
    "engine_expr",
    "engine_ext",
    "engine_head_left",
    "engine_head_top",
    "engine_merge",
    "engine_pager",
    "engine_rowheight",
    "engine_samemerge",
    "engine_sum",
    "engine_types",
];

/// 读 `.rpx` 并跑阶段 B（与 `rpx-cli print` 同一条解析路径）。
fn stage_b_for(name: &str) -> StageB {
    let path = fixtures_dir().join(format!("{name}.rpx"));
    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("打开 {name}.rpx 失败：{e}"));
    let rd = rpx_core::open(file).unwrap_or_else(|e| panic!("解析 {name}.rpx 失败：{e}"));
    prepare_calculate(&rd, &crate::params::ParamTable::EMPTY)
        .unwrap_or_else(|e| panic!("{name} 阶段 B 失败：{e:?}"))
}

/// 取某格的阶段 B 产物（坐标不存在直接 panic，避免静默跳过）。
pub(super) fn cell_at(sb: &StageB, r: i32, c: i32) -> &crate::heads::HeadCell {
    let id = *sb
        .grid
        .by_pos
        .get(&(r, c))
        .unwrap_or_else(|| panic!("阶段 B 缺格 ({r},{c})"));
    &sb.grid.cells[id.0 as usize]
}

// ==================== M2-22 属性表达式 ====================

#[test]
fn row_height_property_expression_is_parsed_from_row_exp_map() {
    let stage = stage_b_for("engine_rowheight");
    for (row, source) in [(2, "40"), (3, "25*2"), (4, "20+5+2")] {
        let cell = cell_at(&stage, row, 0);
        let expression = cell
            .rt
            .property_exprs
            .iter()
            .find(|(key, _)| *key == 1)
            .map(|(_, expression)| expression);
        assert_eq!(
            expression,
            Some(&crate::expr::parse(source).unwrap()),
            "row {row}"
        );
    }
}

// ==================== 判据 1：改写（全量） ====================

#[test]
fn stage_b_rewrite_matches_the_source_block_of_every_fixture_cell() {
    let mut total = 0usize;
    for name in GRID_FIXTURES {
        let sb = stage_b_for(name);
        let cells = parse_expected(&expected_text(name));
        // 同一模板坐标会被多个夹具格引用（合并块铺满、克隆格）——要求值一致。
        let mut seen: HashMap<(i32, i32), i32> = HashMap::new();
        for e in &cells {
            let Some((sr, sc, _rm, _cm, ext)) = e.src else {
                continue;
            };
            match seen.get(&(sr, sc)) {
                Some(prev) => {
                    assert_eq!(
                        *prev, ext,
                        "{name} 模板格 ({sr},{sc}) 的 source.extend 自相矛盾"
                    )
                }
                None => {
                    seen.insert((sr, sc), ext);
                }
            }
        }
        assert!(!seen.is_empty(), "{name} 没有任何 source 可比对");
        for ((sr, sc), ext) in seen {
            let id =
                *sb.grid.by_src.get(&(sr, sc)).unwrap_or_else(|| {
                    panic!("{name} 模板格 ({sr},{sc}) 不在阶段 B 的引擎地址表里")
                });
            let cell = &sb.grid.cells[id.0 as usize];
            assert_eq!(
                cell.extend as i32, ext,
                "{name} 模板格 ({sr},{sc}) 的生效 extend 不符"
            );
            total += 1;
        }
    }
    assert!(
        total > 100,
        "比对的模板格总数过少（{total}）——扫描器可能没读到东西"
    );
}

// ==================== 判据 2：未移动区的 head 真值 ====================

#[test]
fn stage_b_heads_match_the_fixture_outside_the_moved_region() {
    let mut checked_total = 0usize;
    for name in GRID_FIXTURES {
        let sb = stage_b_for(name);
        let text = expected_text(name);
        let cells = parse_expected(&text);
        let (tr, tc) = template_dims(&text);
        let (clone_rows, clone_cols) = clone_axes(&cells);
        let r_stop = clone_rows.iter().next().copied().unwrap_or(i32::MAX);
        let c_stop = clone_cols.iter().next().copied().unwrap_or(i32::MAX);

        let mut checked = 0usize;
        for e in &cells {
            if e.r >= r_stop || e.c >= c_stop || e.r > tr || e.c > tc {
                continue; // 落在插入行列之后，坐标已不是模板坐标
            }
            let Some(expect_lh) = e.lh else { continue };
            let id = *sb
                .grid
                .by_pos
                .get(&(e.r, e.c))
                .unwrap_or_else(|| panic!("{name} 阶段 B 缺格 ({},{})", e.r, e.c));
            let cell = &sb.grid.cells[id.0 as usize];
            assert!(
                cell.left_head.is_some(),
                "{name} 格 ({},{}) 的 left_head 为空",
                e.r,
                e.c
            );
            // 模板形状下：head 的计算态坐标 = 它的模板出处坐标
            let head = &sb.grid.cells[cell.left_head.0 as usize];
            assert_eq!(
                (head.row, head.col),
                expect_lh,
                "{name} 格 ({},{}) 的 left_head 不符",
                e.r,
                e.c
            );
            checked += 1;
        }
        assert!(
            checked > 0,
            "{name} 未移动区没有可比的 head —— 夹具口径变了？"
        );
        checked_total += checked;
    }
    // 实测口径（独立 Python 复算，逐夹具 9/10/3/12/6/8/7/6/4/18/16）：11 个夹具的未移动区
    // 共 **99** 格，且这 99 格**全部**带 `left_head`（引擎矩阵之外还有 3 格因插入列越界被跳过）。
    // 钉到实测值而非"够大就行"：夹具一漂移就能看出来，而不是被宽松阈值吞掉。
    assert_eq!(
        checked_total, 99,
        "未移动区可比的 left_head 格数变了 —— 夹具或判据口径改了？"
    );
}

// ==================== 判据 2b：未移动区的 `disp_value` ====================

/// `disp_value` 的**继承**语义：`ExtNormalCell.getDispValue():1645-1651` 是
/// "`propSetMap[41]` 优先，否则回退 `srcPrepare.getDispValue()`"，所以计算态格的显示值
/// 默认就是模板声明的那个 —— 阶段 B 把它铺上去（`build_grid`），后续 `setDispValue`
/// 才算覆盖。
///
/// ⛔ 这条判据不能只看 `engine_types`：那个夹具 16 格**全是 `None`**
/// （`CalcJson.java:71-73` 只在显示值与 `String.valueOf(value)` 不同时才落盘），
/// 把继承整段删掉也不会失败。真正带 `disp_value` 的只有 `engine_disp`（7 处），
/// 而它的第 3 行会纵向扩展（4 行模板 → 6 行计算），所以仍按"未移动区"口径比。
#[test]
fn stage_b_disp_value_matches_the_fixture_in_the_unmoved_region() {
    let mut checked_total = 0usize;
    let mut non_null_total = 0usize;
    for name in GRID_FIXTURES {
        let sb = stage_b_for(name);
        let text = expected_text(name);
        let cells = parse_expected(&text);
        let (tr, tc) = template_dims(&text);
        let (clone_rows, clone_cols) = clone_axes(&cells);
        let r_stop = clone_rows.iter().next().copied().unwrap_or(i32::MAX);
        let c_stop = clone_cols.iter().next().copied().unwrap_or(i32::MAX);

        for e in &cells {
            if e.r >= r_stop || e.c >= c_stop || e.r > tr || e.c > tc {
                continue;
            }
            let cell = cell_at(&sb, e.r, e.c);
            assert_eq!(
                cell.disp_value().as_deref(),
                e.disp.as_deref(),
                "{name} 格 ({},{}) 的 disp_value 不符",
                e.r,
                e.c
            );
            checked_total += 1;
            if e.disp.is_some() {
                non_null_total += 1;
            }
        }
    }
    assert!(checked_total > 50, "比对格总数过少（{checked_total}）");
    // 实测（独立 Python 复算）：未移动区共 **99** 格，其中带非空显示值的 **5** 处，
    // 全在 `engine_disp`：`(2,1) 北区` / `(2,2) 100.00` / `(2,3) 仅显示值` / `(2,4) 6 件` / `(3,1) Q`。
    // ⚠ 别把 5 看成"只有 5 个夹具格有显示值"：夹具**只在显示值 ≠ `String.valueOf(value)` 时落盘**
    // （`CalcJson.java:71-73`），12 个引擎夹具里只有 `engine_disp` 落过这个键。
    assert_eq!(
        non_null_total, 5,
        "带非空显示值的格数变了（{non_null_total}）—— 夹具口径改了？"
    );
}

// ==================== 阶段 B 自身的不变量 ====================

#[test]
fn stage_b_wraps_only_expression_driven_cells_and_registers_subs_on_cell00() {
    // `engine_sum` 的 9 条公式：`ds1.select(region)`（18 锚点）、`ds1.select(qty)`（18）、
    // `B2*2` 等（17），外加字面量（16，保留声明值）。
    let sb = stage_b_for("engine_sum");
    let text = expected_text("engine_sum");
    let formulas = text.matches("\"exp\":").count();
    assert_eq!(
        sb.expressed, formulas,
        "带表达式的格数应等于夹具 formulas 条数"
    );

    // cell00 是 ExtExtendCell，且收下所有指向它的子格（改写/归一化都会登记）
    let root = &sb.grid.cells[0];
    assert!(root.is_extender, "cell00 必须是扩展格");
    let pointing: Vec<_> = sb
        .grid
        .cells
        .iter()
        .enumerate()
        .filter(|(i, c)| c.left_head == crate::arena::CellId::ROOT && *i != 0)
        .map(|(i, _)| i)
        .collect();
    let registered: Vec<_> = root.left_subs.iter().map(|c| c.0 as usize).collect();
    assert_eq!(
        registered, pointing,
        "cell00 的左子格列表必须恰好等于「left_head 指向 cell00」的格集合（登记漏项会静默少格）"
    );
    assert!(
        !pointing.is_empty(),
        "engine_sum 应有多格以 cell00 为左 head"
    );

    // 上 head 的登记必须**对称**成立 —— 单独关掉 `set_top_head` 的登记时，
    // 上面那条只查 `left_subs` 的断言是**抓不到**的（本轮变异测试实测）。
    let pointing_top: Vec<_> = sb
        .grid
        .cells
        .iter()
        .enumerate()
        .filter(|(i, c)| c.top_head == crate::arena::CellId::ROOT && *i != 0)
        .map(|(i, _)| i)
        .collect();
    let registered_top: Vec<_> = root.top_subs.iter().map(|c| c.0 as usize).collect();
    assert_eq!(
        registered_top, pointing_top,
        "cell00 的上子格列表（_$5）必须恰好等于以它为上 head 的格集合"
    );
    assert!(
        !pointing_top.is_empty(),
        "engine_sum 应有多格以 cell00 为上 head"
    );

    // 声明 16 的字面量格保留 16（§4.3 易错点 1）
    let literals = sb.grid.cells.iter().filter(|c| c.extend == 16).count();
    assert!(literals > 0, "字面量格必须保留声明值 16，不能一律改写");
}

#[test]
fn cell_expression_reads_property_code_40_of_the_template_cell() {
    // 出处：`ExtCellSet.java:1381` 的 `getExpMap().get((byte)40)`。
    let path = fixtures_dir().join("engine_sum.rpx");
    let rd = rpx_core::open(std::fs::File::open(&path).unwrap()).unwrap();
    let cell = rd.cell_at(2, 1).expect("engine_sum 模板 (2,1) 必须存在");
    assert_eq!(cell_expression(cell), Some("ds1.select(region)"));
    // (1,1) 是文字表头，没有表达式
    let head = rd.cell_at(1, 1).expect("(1,1) 存在");
    assert_eq!(cell_expression(head), None, "字面量格不得被误认为带表达式");
}
