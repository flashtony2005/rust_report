//! 加载器**契约**测试：失败必须响亮、未建模的键不得静默丢弃。
//!
//! 与 `fixture_loader.rs` 的分工：那边钉的是**产品地面真值**（尺寸/计数/不变量），
//! 这边钉的是**加载器自身的行为契约** —— 一个"静默返回默认值"的加载器会让
//! 上面所有断言变成永真式，所以这两条性质本身就是被测对象。
//!
//! 纪律：形状不符一律 panic，**绝不**静默给默认值（理由见 `common/json/mod.rs`）。

mod common;

use common::{load, parse_fixture};

#[test]
#[should_panic(expected = "JSON 解析失败")]
fn malformed_json_fails_loudly() {
    // 截断的输入绝不能返回一个"看起来能用"的默认夹具
    let _ = parse_fixture("bad", "{\"name\": \"bad\", ");
}

#[test]
#[should_panic(expected = "缺少必填键")]
fn missing_required_key_fails_loudly() {
    let text = r#"{"name":"x","unit":1,
      "dataset":{"columns":[],"rows":[],"row_count":0},
      "template_rows":1,"formulas":[],"row_cells":[],
      "calculated":{"rows":1,"cols":1,"cells":[]},
      "pagination":null,"merged":null}"#;
    // template_cols 缺失 → 必须 panic，而不是默认为 0
    let _ = parse_fixture("x", text);
}

#[test]
#[should_panic(expected = "未识别键")]
fn unexpected_shape_fails_loudly() {
    let text = r#"{"name":"x","unit":1,
      "dataset":{"columns":[],"rows":[],"row_count":0},
      "template_rows":1,"template_cols":1,"formulas":[],"row_cells":[],
      "calculated":{"rows":1,"cols":1,"cells":[
        {"r":1,"c":1,"value":1,"type":"int","bogus":true}]},
      "pagination":null,"merged":null}"#;
    // cell 里出现 schema 之外的键 → 形状漂移必须响亮失败
    let _ = parse_fixture("x", text);
}

#[test]
#[should_panic(expected = "自述名")]
fn wrong_fixture_name_fails_loudly() {
    // 请求名与 JSON 的 name 字段不符 → 必须 panic，
    // 否则"读到的是别的夹具"会静默通过。
    let text = r#"{"name":"synth","unit":1,
      "dataset":{"columns":[],"rows":[],"row_count":0},
      "template_rows":1,"template_cols":1,"formulas":[],"row_cells":[],
      "calculated":{"rows":1,"cols":1,"cells":[]},
      "pagination":null,"merged":null}"#;
    let _ = parse_fixture("engine_sum", text);
}

/// **未知键不得静默丢弃。**
///
/// `engine_rowheight`（M2-22）在 `row_cells[]` 里引入了 4 个新键：三个行挂
/// `height_exp` / `exp_channel`，其中一个另带 `computed_anchor_row` 与
/// `computed_row_heights`。加载器**没有**为它们加字段，而是按"契约要求保留任何
/// 出现的可选键"把它们收进 [`RowCellInfo::extra`] —— 于是"产品加新键"不需要改
/// 加载器，但也不会被悄悄扔掉。
///
/// 本测试钉的就是后者：键名是 schema（改名应当响亮），键值是原文（不归一化）。
///
/// ⚠️ **本测试转录了 schema 键名**（`height_exp` / `exp_channel` /
/// `computed_anchor_row` / `computed_row_heights`）。
/// **键名漂移时本测试会红，这是有意的** —— 我们要的失败模式是"宁可响，不要静默"：
/// 产品改了键名而断言悄悄放过，才是真正的损失。下一个人若看到这里变红，
/// **不要**当成脆弱断言去删，而应先确认新键名，再同步更新这里。
#[test]
fn unknown_row_cell_keys_are_kept_in_extra_not_dropped() {
    let fx = load("engine_rowheight");

    // 1) 已知键绝不能混进 extra（否则 `row_height` 之类会被重复读两次）
    for rc in &fx.row_cells {
        for (k, _) in &rc.extra {
            assert!(
                !matches!(
                    k.as_str(),
                    "r" | "row_type" | "group_header_level" | "row_height"
                ),
                "已知键 {k} 不该出现在 extra"
            );
        }
    }

    // 2) 4 个新键必须都在，且值非空（空值等于"没落盘"，那是另一回事）
    let extras: Vec<(&str, &str)> = fx
        .row_cells
        .iter()
        .flat_map(|rc| rc.extra.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .collect();
    for key in [
        "height_exp",
        "exp_channel",
        "computed_anchor_row",
        "computed_row_heights",
    ] {
        assert!(
            extras.iter().any(|(k, _)| *k == key),
            "engine_rowheight 的 `{key}` 不该被加载器丢弃；已收进 extra 的是 {extras:?}"
        );
    }
    for (k, v) in &extras {
        assert!(!v.is_empty(), "未知键 `{k}` 的原文不该为空串");
    }

    // 3) 模板行高是定值 10，而计算行高另有序列 —— 两者必须都在，
    //    这正是 M2-22"行高算错 ⇒ 页数错"的基准形态。
    assert!(
        fx.row_cells.iter().all(|rc| rc.row_height == 10.0),
        "engine_rowheight 的模板行高应一律为 10（计算值另存于 extra）"
    );
}
