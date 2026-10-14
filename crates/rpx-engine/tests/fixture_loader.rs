//! `tests/common` 加载器的自验证：断言 **产品产物里真实存在的数字**。
//!
//! 这些数字来自 `GenEngineFixtures.java` 跑出来的 `engine_*.expected.json`，
//! 是 M2-09 的比对基准。把它们写进测试是**故意**的：夹具是产品产物，
//! 一旦产品侧重生成导致数字变化，本文件必须**响亮失败**，不允许静默漂移。
//! （`tests/fixtures/_shape_audit.py` 可独立复核同一批数字。）
//!
//! 覆盖：模板/计算态尺寸与 `source` 计数 / `source` 双向不变量 / 错误型夹具 /
//! 分页逐页 / 合并逐格 / 扩展锚点 / 数值与类型保真。
//! （加载器自身的"失败必须响亮 / 未知键不丢弃"契约见 `tests/loader_contract.rs`。）
//! `engine_types` 的类型面断言单独放在 `tests/fixture_types.rs`。

mod common;

use common::{all_names, load, parse_fixture};

/// 十六个"能算出网格"的夹具（`engine_cycle` 是错误型，见下）。
const GRID_FIXTURES: [&str; 16] = [
    "engine_sum",
    "engine_ext",
    "engine_expr",
    "engine_pager",
    "engine_merge",
    "engine_samemerge",
    "engine_disp",
    "engine_types",
    "engine_rowheight",
    "engine_head_top",
    "engine_head_left",
    "engine_avgrange",
    "engine_collate",
    "engine_listrange",
    "engine_param",
    "engine_gbk",
];

/// 一行尺寸真值：(夹具名, 模板行, 模板列, 计算行, 计算列, 非空格数, 带 source, 不带 source)。
/// 用具名别名代替裸元组，否则触发 `clippy::type_complexity`。
type ShapeRow = (&'static str, i32, i32, i32, i32, usize, usize, usize);

/// 全部尺寸真值。合并成一张表是为了守住"单文件 ≤300 行"门禁，
/// 同时让尺寸与计数不会各说各话。
const SHAPE: [ShapeRow; 16] = [
    ("engine_sum", 6, 9, 8, 9, 36, 36, 0),
    ("engine_ext", 4, 3, 8, 7, 42, 12, 30),
    ("engine_expr", 6, 5, 8, 5, 20, 20, 0),
    ("engine_pager", 4, 3, 43, 3, 124, 10, 114),
    ("engine_merge", 4, 4, 8, 4, 19, 12, 7),
    ("engine_samemerge", 3, 3, 7, 3, 12, 6, 6),
    ("engine_disp", 4, 4, 6, 4, 11, 10, 1),
    ("engine_types", 4, 4, 4, 4, 16, 16, 0),
    ("engine_rowheight", 6, 3, 17, 3, 42, 18, 24),
    ("engine_head_top", 6, 4, 6, 6, 22, 18, 4),
    ("engine_head_left", 5, 6, 11, 6, 66, 30, 36),
    ("engine_avgrange", 5, 6, 5, 6, 12, 12, 0),
    ("engine_collate", 8, 6, 8, 6, 48, 48, 0),
    ("engine_listrange", 5, 6, 5, 6, 19, 19, 0),
    // M3-01 参数夹具：模板 3x4；`@p`（List[1,2,3]）把 A2 铺成 18 扩展 ⇒ 计算态 5x4。
    // 非空格 17 = 4 表头 + 12 数据行 + (5,2)（模板 B3 的 `@s` 被扩展挤到第 5 行）；
    // 第 5 行只有 B 列有值（A/C/D 列没有格）。12 格带 source（行 1 全部 + 行 2/3
    // 全部），5 格是扩展铺开的新行（无 source）。
    ("engine_param", 3, 4, 5, 4, 17, 12, 5),
    // M3-02 GBK 夹具：模板 3x3；`ds1.select(区域)`/`ds1.select(数量)` 两个 18
    // 扩展（中文子格值）⇒ 计算态 5x3。非空格 13 = 3 表头 + 6 数据格 +
    // (5,1)（模板 A3 的中文字面量被扩展挤到第 5 行）；9 格带 source
    // （行 1 全部 + 行 2 全部），4 格是扩展铺开的新行（无 source）。
    // CSV 是 GBK 编码（中文列名/数据）——引擎按 encoding 缺省 GBK 解码。
    ("engine_gbk", 3, 3, 5, 3, 13, 9, 4),
];

#[test]
fn all_fixtures_load_and_self_identify() {
    assert_eq!(all_names().len(), 17);
    for name in all_names() {
        // load 内部会校验 JSON 的 name 字段与请求名一致
        let fx = load(name);
        assert_eq!(fx.name, name, "夹具自述名必须与请求名一致");
        assert_eq!(fx.unit, 1, "{name}: unit 应为 1(mm)");
    }
}

#[test]
fn template_dims_match_product_output() {
    for (name, tmpl_r, tmpl_c, ..) in SHAPE {
        let fx = load(name);
        assert_eq!(
            (fx.template_rows, fx.template_cols),
            (tmpl_r, tmpl_c),
            "{name} 模板尺寸"
        );
    }
}

#[test]
fn calculated_dims_and_source_counts_match_product_output() {
    for (name, _, _, calc_r, calc_c, cells, with_src, without_src) in SHAPE {
        let fx = load(name);
        let g = fx
            .grid()
            .unwrap_or_else(|| panic!("{name} 不应是错误型夹具"));
        assert_eq!((g.rows, g.cols), (calc_r, calc_c), "{name} 计算态尺寸");
        assert_eq!(g.cells.len(), cells, "{name} 非空格数量");
        assert_eq!(g.with_source_count(), with_src, "{name} 带 source 的格数");
        assert_eq!(
            g.without_source_count(),
            without_src,
            "{name} 不带 source 的格数"
        );
        assert_eq!(
            g.with_source_count() + g.without_source_count(),
            g.cells.len(),
            "{name}: source 计数必须覆盖全部非空格"
        );
    }
}

/// `source` 双向不变量：`有 source` ⟺ `落在模板尺寸内`。
/// 双向都要断言（借 `docs/spec-m2.md` 对"只比维度不算通过"的要求），
/// 且**跳过空值** —— 没出现在 `cells` 里的格不参与统计（`CalcJson.java:43-56`）。
#[test]
fn source_is_present_iff_within_template_dims() {
    for name in GRID_FIXTURES {
        let fx = load(name);
        let g = fx.grid().unwrap();
        for cell in &g.cells {
            let inside = cell.r <= fx.template_rows && cell.c <= fx.template_cols;
            match (&cell.source, inside) {
                (Some(_), false) => panic!(
                    "{name}: ({},{}) 在模板尺寸({}x{})外却带 source",
                    cell.r, cell.c, fx.template_rows, fx.template_cols
                ),
                (None, true) => panic!(
                    "{name}: ({},{}) 在模板尺寸({}x{})内却缺 source",
                    cell.r, cell.c, fx.template_rows, fx.template_cols
                ),
                _ => {}
            }
        }
    }
}

#[test]
fn cycle_fixture_is_the_error_form() {
    let fx = load("engine_cycle");
    assert_eq!(
        (fx.template_rows, fx.template_cols),
        (3, 3),
        "engine_cycle 模板尺寸"
    );
    assert!(fx.grid().is_none(), "engine_cycle 不该有计算态网格");
    assert!(fx.is_error());

    let e = fx.error().expect("engine_cycle 必须是错误型");
    assert_eq!(e.class, "com.raqsoft.commonl.ReportError");
    assert_eq!(e.chain.as_deref(), Some("B2-C2-B2-C2"), "循环引用链");

    let cause = e.cause_class.as_deref().expect("应有 cause_class");
    assert!(
        cause.ends_with("IIllIIIIIIIIIlll"),
        "cause_class 应以 IIllIIIIIIIIIlll 结尾，实际 {cause}"
    );

    assert!(fx.pagination.is_none(), "错误型夹具 pagination 应为 None");
    assert!(fx.merged.is_none(), "错误型夹具 merged 应为 None");

    // 转义解码证据：message / cause_message 里是 \uXXXX，必须已解成中文
    assert!(
        e.message.contains('单'),
        "message 的 \\uXXXX 未解码：{}",
        e.message
    );
    let cm = e.cause_message.as_deref().expect("应有 cause_message");
    assert!(cm.ends_with("存在循环引用"), "cause_message = {cm}");
}

#[test]
fn pager_pagination_matches_product_output() {
    let fx = load("engine_pager");
    let p = fx.pagination.as_ref().expect("engine_pager 必须有分页");
    assert_eq!(p.page_count, 4);
    assert_eq!(p.x_pages, 1);
    assert_eq!(p.y_pages, 4);
    assert_eq!(p.row_num_per_page, 20);

    let rows: Vec<i32> = p.pages.iter().map(|pg| pg.rows).collect();
    assert_eq!(rows, vec![14, 14, 14, 4], "逐页行数");

    let nums: Vec<i32> = p.pages.iter().map(|pg| pg.page).collect();
    assert_eq!(nums, vec![1, 2, 3, 4], "页号应从 1 起递增");

    // 末页 rows=4 但第 1 列非空值只有 3 个：`CalcJson.java:121-131` 跳过空值。
    // 这条钉住"rows 与 col1_values.len() 不必相等"，防止后来者把两者混为一谈。
    assert_eq!(p.pages[3].rows, 4);
    assert_eq!(p.pages[3].col1_values.len(), 3, "空值被跳过");
    assert_eq!(p.pages[0].first_value.as_deref(), Some("region"));
    assert_eq!(p.pages[3].last_value.as_deref(), Some("R40"));
}

#[test]
fn samemerge_merged_is_exactly_four_row_spans() {
    let fx = load("engine_samemerge");
    let m = fx.merged.as_ref().expect("engine_samemerge 必须有 merged");
    assert_eq!(m.page_count, 1);
    assert_eq!(m.cells.len(), 4, "恰好 4 个合并格");
    for c in &m.cells {
        assert_eq!(c.page, 1);
        assert_eq!(c.c, 1, "mergeSame 只作用在第 1 列");
        assert_eq!(c.row_merge, 2, "两两相等的相邻值得以合并");
        assert_eq!(c.col_merge, 1, "mergeSame 不产生列合并");
    }
    let vals: Vec<&str> = m.cells.iter().map(|c| c.value_raw.as_str()).collect();
    assert_eq!(
        vals,
        vec!["\"A\"", "\"A\"", "\"B\"", "\"B\""],
        "value_raw 是含引号的 JSON 原文 token"
    );
}

#[test]
fn ext_anchor_spot_checks() {
    let fx = load("engine_ext");

    // (2,1)：extendMode 17（ArrayList）→ 单格承载整列列表
    let s = fx.cell(2, 1).unwrap().source.as_ref().unwrap();
    assert_eq!(fx.cell(2, 1).unwrap().declared_type, "list");
    assert_eq!(s.extend, 17);
    assert_eq!(s.sub_cells, 1);
    assert!(s.sub_values.is_none(), "sub_cells==1 时不写 sub_values");

    // (3,1)：extendMode 18（纵向）→ 5 个附属格
    let s = fx.cell(3, 1).unwrap().source.as_ref().unwrap();
    assert_eq!(s.extend, 18);
    assert_eq!(s.sub_cells, 5);
    assert_eq!(s.sub_values.as_ref().unwrap().len(), 5);

    // (4,1)：extendMode 19（横向）→ 附属格落在第 8 行 1..5 列
    let s = fx.cell(4, 1).unwrap().source.as_ref().unwrap();
    assert_eq!(s.extend, 19);
    assert_eq!(s.sub_cells, 5);
    let subs = s.sub_values.as_ref().unwrap();
    let pos: Vec<(i32, i32)> = subs.iter().map(|v| (v.r, v.c)).collect();
    assert_eq!(
        pos,
        vec![(8, 1), (8, 2), (8, 3), (8, 4), (8, 5)],
        "横向扩展把附属格排到同一行的连续列"
    );
}

#[test]
fn merge_anchor_spot_check() {
    let fx = load("engine_merge");
    // 锚点在跨模板纵向合并区 A2:A3 上 → row_merge=2（附件 H.7.1）
    let s = fx.cell(2, 1).unwrap().source.as_ref().unwrap();
    assert_eq!(s.row_merge, 2);
    assert_eq!(s.extend, 18);
}

/// **类型由 `type` 字段决定，不由 JSON 字面形状决定。**
///
/// `CalcJson.java:204-212` 会把整数值的 `Double` 打成裸整数（`150`），
/// `:201-203` 把 `BigDecimal` 打成 `toString()`（`100.00`）。若解析期经 `f64`
/// 回写，两者都会失真（`150`→`150.0`、`100.00`→`100.0`），M2-14 就断言不了。
/// 本测试用合成夹具直接钉死这两条；真实夹具上的同类断言见 `fixture_types.rs`。
#[test]
fn reader_preserves_exact_number_tokens_and_declared_type() {
    let text = r#"{"name":"synth","unit":1,
      "dataset":{"columns":[],"rows":[],"row_count":0},
      "template_rows":1,"template_cols":2,"formulas":[],"row_cells":[],
      "calculated":{"rows":1,"cols":2,"cells":[
        {"r":1,"c":1,"value":150,"type":"double"},
        {"r":1,"c":2,"value":100.00,"type":"decimal"}]},
      "pagination":null,"merged":null}"#;
    let fx = parse_fixture("synth", text);

    let a = fx.cell(1, 1).unwrap();
    assert_eq!(a.value_raw, "150", "整数值的 double 必须是裸整数原文");
    assert_eq!(
        a.declared_type, "double",
        "类型取自 type 字段，不是字面形状"
    );
    assert!(a.source.is_none());

    let b = fx.cell(1, 2).unwrap();
    assert_eq!(
        b.value_raw, "100.00",
        "BigDecimal.toString() 的两位小数原样保留"
    );
    assert_eq!(b.declared_type, "decimal");
}
