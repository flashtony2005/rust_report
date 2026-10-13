//! [`crate::extend`] 的单元测试。
//!
//! 每条断言都挂着来源：`file:line`（产品源码）、夹具名（`tests/fixtures/engine_*.expected.json`）
//! 或实测命令 —— 没有来源的断言不许留在这里。
//!
//! 算法正文（本项目要求「算法写进 md，代码只留指针」）：
//! - `docs/m2-engine-design.md` §4.3 —— extensible 的两条写入路径
//! - `docs/format-contract.md` 附件 H.4 —— 拓扑改写；H.7.1 —— 块高与行数口径

use super::*;

mod wiring;

const ROOT: CellId = CellId::ROOT;
/// `INormalCell.TYPE_NORMAL = -64`（`INormalCell.java:21`）—— 不在 `checkExtend` 的排除集里。
const NORMAL_TYPE: i8 = -64;
/// `INormalCell.TYPE_BARCODE = -57`（`INormalCell.java:27`）—— 在排除集里。
const BARCODE_TYPE: i8 = -57;
const OTHER: CellId = CellId(7);

#[test]
fn codes_round_trip_and_the_vocabulary_is_closed() {
    for m in ExtendMode::ALL {
        assert_eq!(ExtendMode::from_code(m.code()), Some(m));
    }
    assert_eq!(
        ExtendMode::ALL.map(|m| m.code()),
        [16, 17, 18, 19],
        "码值必须与 props.rs::def 一致"
    );
    // 模板槽位是裸 i8，可能被填成任何值 —— 不许默默当成 16
    assert_eq!(ExtendMode::from_code(15), None);
    assert_eq!(ExtendMode::from_code(0), None);
    assert_eq!(ExtendMode::from_code(-1), None);
}

#[test]
fn only_18_and_19_drive_extension() {
    assert!(ExtendMode::Vertical18.drives_extension());
    assert!(ExtendMode::Horizontal19.drives_extension());
    assert!(!ExtendMode::None17.drives_extension());
    assert!(!ExtendMode::Default16.drives_extension());
    assert!(ExtendMode::Vertical18.extends_vertically());
    assert!(ExtendMode::Horizontal19.extends_horizontally());
}

/// 四个象限逐一钉死（`prepareCalculate:321-327`）。
#[test]
fn topology_covers_all_four_quadrants() {
    // (cell00, 非 cell00) -> 19
    assert_eq!(topology_default(ROOT, OTHER, ROOT), ExtendMode::Horizontal19);
    // (非 cell00, 非 cell00) -> 17
    assert_eq!(topology_default(OTHER, OTHER, ROOT), ExtendMode::None17);
    // (cell00, cell00) -> 18
    assert_eq!(topology_default(ROOT, ROOT, ROOT), ExtendMode::Vertical18);
    // (非 cell00, cell00) -> 18 —— 落在 else 分支，容易误判成 17/19
    assert_eq!(topology_default(OTHER, ROOT, ROOT), ExtendMode::Vertical18);
}

/// heads 为 `null` 时 Java 比较 `null == cell00` 得 false → 走"非 cell00"一侧。
#[test]
fn null_heads_count_as_not_cell00() {
    assert_eq!(topology_default(CellId::NONE, CellId::NONE, ROOT), ExtendMode::None17);
    assert_eq!(topology_default(CellId::NONE, ROOT, ROOT), ExtendMode::Vertical18);
}

/// ⛔ 无表达式的格不被改写 —— 这是 `engine_sum` 里 27 个 `extend=16` 的来源。
/// 两条写入路径的 `else` 分支都不改写（`ExtExtendCell.java:330` / `ExtNormalCell.java:542`）。
#[test]
fn the_rewrite_is_gated_on_having_an_expression() {
    let literal = cell_role(false, ExtendMode::Default16, false, NORMAL_TYPE);
    assert_eq!(literal, CellRole::Literal);
    // 无论拓扑如何，声明值原样保留
    for &(l, t) in &[(ROOT, ROOT), (ROOT, OTHER), (OTHER, ROOT), (OTHER, OTHER)] {
        assert_eq!(
            ExtendMode::Default16.resolve(literal, l, t, ROOT),
            ExtendMode::Default16
        );
    }
}

/// 扩展格（`Extender`）：声明 16 时按拓扑改写（`ExtExtendCell.java:315-329`）。
#[test]
fn an_extender_with_declared_16_follows_the_topology() {
    let ext = cell_role(true, ExtendMode::Default16, true, NORMAL_TYPE);
    assert_eq!(ext, CellRole::Extender);
    assert_eq!(
        ExtendMode::Default16.resolve(ext, OTHER, OTHER, ROOT),
        ExtendMode::None17
    );
    assert_eq!(
        ExtendMode::Default16.resolve(ext, ROOT, OTHER, ROOT),
        ExtendMode::Horizontal19
    );
    assert_eq!(
        ExtendMode::Default16.resolve(ext, ROOT, ROOT, ROOT),
        ExtendMode::Vertical18
    );
}

/// 非 16 的声明值是"已解析"的，扩展格上无论拓扑都不动（`engine_ext` 的 17/18/19）。
#[test]
fn explicitly_declared_modes_are_never_rewritten() {
    for declared in [ExtendMode::None17, ExtendMode::Vertical18, ExtendMode::Horizontal19] {
        let role = cell_role(true, declared, true, NORMAL_TYPE);
        for &(l, t) in &[(ROOT, ROOT), (ROOT, OTHER), (OTHER, ROOT), (OTHER, OTHER)] {
            assert_eq!(declared.resolve(role, l, t, ROOT), declared);
        }
    }
}

/// ⛔ **第二条路径**（`ExtNormalCell.java:539-541`）：有表达式但**不是**扩展格的普通格
/// —— **无条件**置 17，与 heads 拓扑无关。
///
/// 这一条是"因为错的原因得出对的答案"的解药：`engine_sum` 的 `(2,3)..(2,9)`
/// 在夹具里恰好 heads 为 `null`（拓扑也给出 17），但若沿用只看拓扑的实现，
/// 一个 heads 落在 `(cell00, cell00)` 的普通公式格会被判成 18 —— 错。
#[test]
fn a_plain_formula_cell_is_pinned_to_17_regardless_of_topology() {
    let plain = cell_role(true, ExtendMode::Default16, false, NORMAL_TYPE);
    assert_eq!(plain, CellRole::PlainFormula, "声明16 + 表达式不驱动扩展 → 普通格");
    let declared17 = cell_role(true, ExtendMode::None17, false, NORMAL_TYPE);
    assert_eq!(declared17, CellRole::PlainFormula, "声明17 → 也是普通格");

    for &(l, t) in &[(ROOT, ROOT), (ROOT, OTHER), (OTHER, ROOT), (OTHER, OTHER)] {
        // 注意 (ROOT, ROOT) —— 只看拓扑会给出 18，这里必须是 17
        assert_eq!(
            ExtendMode::Default16.resolve(plain, l, t, ROOT),
            ExtendMode::None17,
            "heads=({l:?},{t:?}) 时普通公式格仍须为 17"
        );
    }
}

/// 声明 18/19 的格恒为扩展格，**即使没有表达式**（`checkExtend` 末行无条件为真），
/// 且此时不被改写（`prepareCalculate` 的 `expression != null` 门禁）。
#[test]
fn declared_18_and_19_are_extenders_even_without_an_expression() {
    for declared in [ExtendMode::Vertical18, ExtendMode::Horizontal19] {
        assert_eq!(cell_role(false, declared, false, NORMAL_TYPE), CellRole::Extender);
        assert_eq!(cell_role(true, declared, false, NORMAL_TYPE), CellRole::Extender);
        // 无表达式 → 保留；有表达式 → 声明值非 16，也保留
        assert_eq!(declared.resolve(CellRole::Extender, OTHER, OTHER, ROOT), declared);
    }
}

/// ⛔ **类型短路优先于一切**（`ExtNormalCell.checkExtend:494-496`）：图形 / 子报表 / 条形码
/// 三种格**永远不是扩展格**，声明 18/19 也不例外。
///
/// 这条是**可构造但无夹具覆盖**的分叉（当前 7 个夹具都没有这三种类型的格，所以漏掉它
/// 没有任何测试会失败）：
/// `setCellType((byte)-57)` + `setExtendMode(18)` + 任意表达式
/// → Java 落成 `ExtNormalCell` → `prepareCalculate:539-541` 置 **17**；
/// 只按声明实现的版本会给 **18**。
#[test]
fn an_excluded_cell_type_is_never_an_extender() {
    // 三种被排除的类型，声明 18/19 都不许成为扩展格
    for &ty in &EXTEND_EXCLUDED_CELL_TYPES {
        assert!(is_extend_excluded_type(ty));
        for declared in [ExtendMode::Vertical18, ExtendMode::Horizontal19] {
            // 有表达式 → 普通公式格 → 无条件 17（而不是 18/19）
            let role = cell_role(true, declared, true, ty);
            assert_eq!(role, CellRole::PlainFormula, "type={ty} 声明={declared:?}");
            assert_eq!(
                declared.resolve(role, OTHER, OTHER, ROOT),
                ExtendMode::None17,
                "type={ty} 声明={declared:?} 计算态必须落到 17"
            );
            // 无表达式 → 不改写，保留声明值
            let lit = cell_role(false, declared, false, ty);
            assert_eq!(lit, CellRole::Literal, "type={ty} 无表达式");
            assert_eq!(declared.resolve(lit, OTHER, OTHER, ROOT), declared);
        }
    }
    // 对照组：普通类型（-64）声明 18 就是扩展格 —— 排除集是**具体三个**，不是"非普通即排除"
    assert!(!is_extend_excluded_type(NORMAL_TYPE));
    assert_eq!(
        cell_role(true, ExtendMode::Vertical18, false, NORMAL_TYPE),
        CellRole::Extender
    );
    assert_eq!(
        cell_role(true, ExtendMode::Vertical18, false, BARCODE_TYPE),
        CellRole::PlainFormula
    );
}

/// `engine_ext` 的三个锚点声明 17/18/19，计算态**原样**是 17/18/19
/// （附件 H.4 补遗：17→`sub_cells=1`、18→5 个同列子格、19→5 个同行子格）。
#[test]
fn engine_ext_anchors_keep_their_declared_mode() {
    let anchors = [
        (ExtendMode::None17, 17, 1),
        (ExtendMode::Vertical18, 18, 5),
        (ExtendMode::Horizontal19, 19, 5),
    ];
    for (declared, expected_code, expected_subs) in anchors {
        let role = cell_role(true, declared, true, NORMAL_TYPE);
        let resolved = declared.resolve(role, OTHER, OTHER, ROOT);
        assert_eq!(resolved.code(), expected_code);
        assert_eq!(expected_subs, if resolved == ExtendMode::None17 { 1 } else { 5 });
    }
}

/// 六个引擎夹具逐条验证行数公式。数据来自各自的 `expected.json`（实测值）。
///
/// 注意第 2 列是**模板行数**（`ReportDefine2` 第一参数，不含 0 号脚手架行），
/// 第 5 列是 `calculated.rows` —— 两者同为 `getRowSize() - 1` 口径，**直接相等，不加 1**。
#[test]
fn row_count_matches_every_fixture() {
    // (夹具, 模板行数 = ReportDefine2 第一参数, 数据集行数, 块高, 实测 calculated.rows)
    let fixtures = [
        ("engine_sum", 6, 3, 1, 8),
        ("engine_ext", 4, 5, 1, 8),
        ("engine_expr", 6, 3, 1, 8),
        ("engine_pager", 4, 40, 1, 43),
        ("engine_merge", 4, 3, 2, 8),      // A2 合并 A2:A3，块高 2
        ("engine_samemerge", 3, 5, 1, 7),
    ];
    for (name, tmpl, data, block, expected) in fixtures {
        assert_eq!(
            calc_row_count(tmpl, data, block),
            expected,
            "fixture {name}: {tmpl} + ({data}-1)*{block}"
        );
    }
}

/// 口径钉子：模板行数**不含** 0 号脚手架行，裸矩阵行数比它多 1。
///
/// `rpx-cli print tests/fixtures/engine_sum.rpx` → `matrix: 7x10 (logical 6x9)`。
/// 若有人把 `rpx_core::ReportDefine.rows`（裸矩阵 7）直接喂给 [`calc_row_count`]，
/// 结果会整体偏大 1 —— 这条测试把"必须先减 1"写死。
#[test]
fn template_row_count_excludes_the_scaffold_row() {
    let raw_matrix_rows = 7; // engine_sum 的 define.rows
    let template_row_count = 6; // ReportDefine2(6, 9) 的第一参数
    assert_eq!(raw_matrix_rows, template_row_count + 1);
    assert_eq!(calc_row_count(raw_matrix_rows - 1, 3, 1), 8); // 正确
    assert_eq!(calc_row_count(raw_matrix_rows, 3, 1), 9); // 少减 1 就多出一行
}

/// 声明 16 的普通报表（`engine_sum` / `engine_expr` / `engine_pager`）走的就是块高 1 的退化形式。
#[test]
fn block_height_one_degenerates_to_the_h4_formula() {
    assert_eq!(calc_row_count(6, 3, 1), 6 + (3 - 1));
    // 块高 <= 1 一律按 1 处理，不会因为传 0 而"倒扣"行
    assert_eq!(calc_row_count(6, 3, 0), calc_row_count(6, 3, 1));
    assert_eq!(calc_row_count(6, 3, -1), calc_row_count(6, 3, 1));
    // 数据集 0/负行 → 不插入任何行（约定，非实测：P0 无空数据集基准）
    assert_eq!(calc_row_count(5, 0, 1), 5);
    assert_eq!(calc_row_count(5, -3, 1), 5);
    // 数据集恰好 1 行 → 只占模板那一行
    assert_eq!(calc_row_count(5, 1, 1), 5);
}
