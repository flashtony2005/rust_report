//! M2-09 **对拍器**：把「引擎输出」与「产品夹具」逐格比对，产出可定位的差异报告。
//!
//! # 定位
//!
//! `disp_assert` / `subcell_assert` / `merged_assert` 那一轮完成的是**判据**——
//! 「夹具自身是否合规」。本模块是另一半：**拿我的引擎输出与夹具逐格对比**。
//! head 图（§12.1 第 5/6 步）一落地就能立刻对拍，不必等断言写完。
//!
//! # 三条设计纪律（都来自前几轮栽过的坑）
//!
//! ## ① 必须区分三类差异，不能只报 mismatch
//!
//! | 类别 | 含义 | 典型成因 |
//! |---|---|---|
//! | [`Kind::Missing`] | 夹具有、引擎没有 | **扩展行数少了** —— 最严重 |
//! | [`Kind::Extra`] | 引擎有、夹具没有 | **扩展行数多了** |
//! | [`Kind::Mismatch`] | 两边都有但内容不同 | 求值/类型/扩展结构错 |
//!
//! 若只报 mismatch，「少了几行整个扩展块」会被当成「没问题」——那是最贵的漏检。
//!
//! ## ② `cells` 是**稀疏**的：缺席 ≠ 不一致
//!
//! `value` 与 `disp_value` **双 null** 的格**本就不落盘**（`CalcJson.java:51-53`）。
//! 所以比对应遍历**两侧格集合的并集**，而**不是**遍历 `rows × cols` 的全部位置；
//! 否则一个 8×9 里只有 36 格的夹具会立刻报出 36 条假 missing。
//! 两边都缺席的坐标**直接跳过**，不产生任何差异。
//!
//! ## ③ `source` 是可选键，且 **calc 坐标 ≠ src 坐标**
//!
//! `CalcJson.java:74-98` 只在 `getSource(r,c) != null` 时才写 `source`。
//! 另外 `engine_merge` 的 `(2,4)` 是唯一一个 `src = (2,3) != (r,c)` 的格 ——
//! 所以本模块**绝不**把「calc 坐标 == src 坐标」当不变量，只按夹具写下来的值比。
//!
//! # 值按**原文 token** 比，不做数值归一化
//!
//! `value` 用原始 JSON 文本比（`100` 与 `100.0` 因此不等）。这是有意的：
//! `Integer(100)` 与 `Double(100.0)` 的区分正是 M2-14 的核心（`CalcJson.java:209-211`
//! 会把整数值的 Double 折叠成裸 `100`）。若在这里做 f64 归一化，M2-14 就白测了。
//!
//! # 本轮不比较的东西（有意为之，逐条说明）
//!
//! * **`message` / `cause_message`**：产品是本地化文本（`engine_cycle` 里是中文），
//!   夹具专门留了 `chain` 字段就是为了"不必解析本地化消息"（`GenEngineFixtures.java:179-184`）。
//!   故错误形态只比 `class` 与 `chain`。
//! * **数组顺序**：只按 `(r, c)` 建索引，不假设引擎与夹具的 `cells` 数组顺序相同。
//!
//! # 分片
//!
//! 受"单文件 ≤300 行"门禁约束，拆成四片：本文件放类型与入口，
//! [`util`] 放索引小工具，[`cells`] 放逐格比对，[`sections`] 放 `pagination` / `merged`。

mod cells;
mod sections;
mod types;
mod util;

pub use types::{Diff, Dim, Kind, Report};

use std::path::Path;

use super::json::{self, Json};
use super::load_raw;
use util::grid_dims;

// =====================================================================
// 入口
// =====================================================================

/// 引擎从文件导出其 `expected.json` 同 schema 的 JSON 后，用这个入口对比。
pub fn compare_actual_file(actual_path: &Path, fixture_name: &str) -> Report {
    let text = std::fs::read_to_string(actual_path)
        .unwrap_or_else(|e| panic!("读取引擎输出 {} 失败：{e}", actual_path.display()));
    compare_actual_text(&text, fixture_name)
}

/// 引擎输出以字符串给出时用这个入口。
pub fn compare_actual_text(actual_text: &str, fixture_name: &str) -> Report {
    let actual = json::parse(actual_text).unwrap_or_else(|e| panic!("引擎输出不是合法 JSON：{e}"));
    compare(&actual, &load_raw(fixture_name), fixture_name)
}

/// **空跑自测**：拿夹具与它自己比，必须零差异。
///
/// 这是对拍器自身的自检：证明"零差异"不是恒真式 —— 若比对逻辑写错（例如把稀疏格
/// 全报成 missing、或漏比某个维度），这里立刻红。
pub fn compare_fixture_to_itself(fixture_name: &str) -> Report {
    let raw = load_raw(fixture_name);
    compare(&raw, &raw, fixture_name)
}

/// 与类型化 [`super::Fixture`] 一起用的入口（省得调用方少写一次夹具名）。
pub fn compare_fixture_object(fx: &super::model::Fixture, actual_text: &str) -> Report {
    compare_actual_text(actual_text, &fx.name)
}

// =====================================================================
// 比对编排（差异按"第一处错误"的顺序累积）
// =====================================================================

/// `expected` = 夹具（基准）；`actual` = 引擎输出。
pub(super) fn compare(actual: &Json, expected: &Json, fixture: &str) -> Report {
    let mut d: Vec<Diff> = Vec::new();
    let exp_calc = expected.get("calculated").expect("夹具必有 calculated");
    let act_calc = actual
        .get("calculated")
        .unwrap_or_else(|| panic!("引擎输出缺 calculated 段（须与 expected.json 同 schema）"));
    let (exp_err, act_err) = (exp_calc.get("error"), act_calc.get("error"));
    let (exp_dims, act_dims) = (grid_dims(exp_calc), grid_dims(act_calc));

    match (exp_err, act_err) {
        (Some(_), None) => d.push(Diff {
            kind: Kind::Missing,
            r: -1,
            c: -1,
            dim: Dim::Error,
            detail: "引擎未产出 calculated.error".to_string(),
            expected: "error".to_string(),
            actual: String::new(),
        }),
        (None, Some(_)) => d.push(Diff {
            kind: Kind::Extra,
            r: -1,
            c: -1,
            dim: Dim::Error,
            detail: "夹具是网格形态，引擎却产出了 error".to_string(),
            expected: String::new(),
            actual: "error".to_string(),
        }),
        (Some(e), Some(a)) => compare_error(&mut d, e, a),
        (None, None) => {
            compare_grid_dims(&mut d, exp_dims, act_dims);
            cells::compare_cells(&mut d, act_calc, exp_calc);
        }
    }
    sections::compare_pagination(&mut d, actual, expected);
    sections::compare_merged(&mut d, actual, expected);

    Report {
        fixture: fixture.to_string(),
        diffs: d,
        expected_dims: exp_dims,
        actual_dims: act_dims,
    }
}

/// 错误形态：只比 `class` 与 `chain`（`message` 是本地化文本，见模块文档）。
fn compare_error(d: &mut Vec<Diff>, e: &Json, a: &Json) {
    for key in ["class", "chain"] {
        let ev = util::raw_of(e.get(key));
        let av = util::raw_of(a.get(key));
        if ev != av {
            d.push(Diff {
                kind: Kind::Mismatch,
                r: -1,
                c: -1,
                dim: Dim::Error,
                detail: key.to_string(),
                expected: ev,
                actual: av,
            });
        }
    }
}

fn compare_grid_dims(d: &mut Vec<Diff>, exp: Option<(i32, i32)>, act: Option<(i32, i32)>) {
    if let (Some((er, ec)), Some((ar, ac))) = (exp, act) {
        for (dim, e, a) in [(Dim::GridRows, er, ar), (Dim::GridCols, ec, ac)] {
            if e != a {
                d.push(Diff {
                    kind: Kind::Mismatch,
                    r: -1,
                    c: -1,
                    dim,
                    detail: String::new(),
                    expected: e.to_string(),
                    actual: a.to_string(),
                });
            }
        }
    }
}
