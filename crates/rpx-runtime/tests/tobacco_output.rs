//! 「卷烟产值计算表」的判据：多口径分组合计 + 逐行产值计算。
//!
//! 这个报表的失效形态**都不会报错**：
//! ① 某一组的口径漏了几个牌号 —— 那一组的合计就变小，其他组照旧，看起来只是"这组就是这样"；
//! ② 产值用合计产量 × 合计单价（而不是逐行算完再加）—— 数字照样出来，但**必然是错的**
//!    （各组价格不同，加权平均不等于任一口径）；
//! ③ 累计列拿本月值糊弄 —— 只有两列都对得上才能发现。
//!
//! 所以判据做了三件事：**四组口径给出同一个总数**（②的判据）、
//! **单个牌号的产值 = 产量 × 单价 / 1000**（①的判据）、
//! **分组小计 = 其下各明细之和**。
//!
//! 结果行号（0 基）：各厂合计 4、包装形式合计 9、卷烟类型合计 18、价类合计 26；
//! 条盒硬盒 10 及其区间 11..14；中华(软) 15。
use rpx_engine::params::ParamTable;
use rpx_runtime::{execution::render_request, form::RenderRequest};
use serde_json::Value as J;
use std::{collections::BTreeMap, path::Path};

const OUT_M: usize = 2; // 产量本月
const OUT_C: usize = 3; // 产量累计
const PROD_M: usize = 4; // 总产值本月
const PROD_C: usize = 5; // 总产值累计
const SALE_M: usize = 6; // 销量本月
const SALE_C: usize = 7; // 销量累计
const REV_M: usize = 8; // 销售产值本月
const REV_C: usize = 9; // 销售产值累计

fn render() -> J {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/r-model");
    let req = RenderRequest {
        report: std::fs::read(dir.join("tobacco-output.json")).unwrap(),
        report_name: "tobacco-output".into(),
        params: ParamTable::EMPTY,
        datasets: BTreeMap::from([(
            "tobacco-output.csv".into(),
            std::fs::read(dir.join("tobacco-output.csv")).unwrap(),
        )]),
    };
    serde_json::from_str(&render_request(&req).unwrap()).unwrap()
}

fn at(book: &J, row: usize, col: usize) -> f64 {
    book["sheets"]["rpx-sheet-1"]["cellData"][row.to_string()][col.to_string()]["v"]
        .as_f64()
        .unwrap_or_else(|| panic!("({row},{col}) 没有数值"))
}

fn close(got: f64, want: f64, what: &str) {
    assert!((got - want).abs() < 1e-6, "{what}：得到 {got}，应为 {want}");
}

#[test]
fn all_four_groupings_agree_on_the_same_total() {
    let book = render();
    let rows = [
        ("各厂合计", 4usize),
        ("包装形式合计", 9),
        ("卷烟类型合计", 18),
        ("价类合计", 26),
    ];
    for (what, row) in rows {
        close(at(&book, row, OUT_M), 17.0, &format!("{what} · 产量本月"));
        close(at(&book, row, OUT_C), 21.0, &format!("{what} · 产量累计"));
        close(
            at(&book, row, PROD_M),
            0.598,
            &format!("{what} · 总产值本月"),
        );
        close(
            at(&book, row, PROD_C),
            0.774,
            &format!("{what} · 总产值累计"),
        );
        close(at(&book, row, SALE_M), 12.0, &format!("{what} · 销量本月"));
        close(at(&book, row, SALE_C), 20.0, &format!("{what} · 销量累计"));
        close(
            at(&book, row, REV_M),
            0.468,
            &format!("{what} · 销售产值本月"),
        );
        close(
            at(&book, row, REV_C),
            0.676,
            &format!("{what} · 销售产值累计"),
        );
    }
}

#[test]
fn each_leaf_row_multiplies_output_by_its_own_price() {
    let book = render();
    // 中华(软)：本月 5 万支 × 65 元/万支 = 325 元 = 0.325 千元；累计同理。
    let row = 15;
    close(at(&book, row, OUT_M), 5.0, "中华(软) 产量本月");
    close(at(&book, row, OUT_C), 5.0, "中华(软) 产量累计");
    close(at(&book, row, PROD_M), 0.325, "中华(软) 总产值本月");
    close(at(&book, row, PROD_C), 0.325, "中华(软) 总产值累计");
    close(at(&book, row, SALE_M), 4.0, "中华(软) 销量本月");
    close(at(&book, row, REV_M), 0.260, "中华(软) 销售产值本月");
    // 中华(硬)：4 × 44 / 1000 = 0.176
    close(at(&book, 29, PROD_M), 0.176, "中华(硬) 总产值本月");
    // 红塔山(硬)：1 × 7 / 1000 = 0.007
    close(at(&book, 37, PROD_M), 0.007, "红塔山(硬) 总产值本月");
}

#[test]
fn group_subtotal_equals_the_sum_of_its_leaf_rows() {
    let book = render();
    // 条盒硬盒（行 10）= 中华(硬) 12 + 双喜(硬) 13 + 红塔山(硬) 14
    for col in [OUT_M, PROD_M, REV_C] {
        let want: f64 = (11..=13).map(|r| at(&book, r, col)).sum();
        close(at(&book, 10, col), want, &format!("条盒硬盒 第 {col} 列"));
    }
    // 一类烟（行 27）= 中华(软) 28 + 中华(硬) 29
    for col in [OUT_M, PROD_M, REV_M] {
        let want: f64 = (28..=29).map(|r| at(&book, r, col)).sum();
        close(at(&book, 27, col), want, &format!("一类烟 第 {col} 列"));
    }
}

#[test]
fn two_level_header_merges_name_and_group_columns() {
    let book = render();
    let spans: Vec<(i64, i64, i64, i64)> = book["sheets"]["rpx-sheet-1"]["mergeData"]
        .as_array()
        .expect("mergeData")
        .iter()
        .map(|m| {
            (
                m["startRow"].as_i64().unwrap(),
                m["startColumn"].as_i64().unwrap(),
                m["endRow"].as_i64().unwrap(),
                m["endColumn"].as_i64().unwrap(),
            )
        })
        .collect();
    for want in [
        (2i64, 0i64, 3i64, 0i64),
        (2, 1, 3, 1),
        (2, 2, 2, 3),
        (2, 4, 2, 5),
        (2, 6, 2, 7),
        (2, 8, 2, 9),
    ] {
        assert!(spans.contains(&want), "应存在合并 {want:?}，实际 {spans:?}");
    }
}
