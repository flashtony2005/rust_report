//! 把 `tests/fixtures/engine_*.expected.json` 读成 [`Fixture`]。
//!
//! 入口：[`load`]（按夹具名读盘）与 [`parse_fixture`]（对已读入的文本解析，
//! 供合成夹具的单测使用）。所有形状错误都会 panic，消息带上下文，绝不静默兜底。
//!
//! 目录口径与 `tests/units_contract.rs:40-42` 一致：
//! `CARGO_MANIFEST_DIR/../../tests/fixtures`。

use std::path::PathBuf;

use super::json::{self, Json};
use super::model::*;

/// 15 个夹具名（顺序与 `GenEngineFixtures.java` 的 `NAMES` 一致）。
pub fn all_names() -> [&'static str; 17] {
    [
        "engine_sum",
        "engine_ext",
        "engine_expr",
        "engine_pager",
        "engine_merge",
        "engine_cycle",
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
    ]
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn read_fixture(name: &str) -> (PathBuf, String) {
    let path = fixtures_dir().join(format!("{name}.expected.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("读取夹具 {} 失败：{e}", path.display()));
    (path, text)
}

/// 按夹具名加载 `tests/fixtures/<name>.expected.json`。
pub fn load(name: &str) -> Fixture {
    let (path, text) = read_fixture(name);
    // 解析错误带上文件名再抛，避免"裸 panic"看不出是哪个夹具
    match parse_checked(name, &text) {
        Ok(fx) => fx,
        Err(e) => panic!("{}：{e}", path.display()),
    }
}

/// 读出夹具的**原始 JSON**（不做类型化），供 schema 哨兵比对键集合。
/// 类型化的 [`Fixture`] 只保留**已建模**的字段（未知顶层键直接丢弃），
/// 所以"产品到底写了哪些键"**只能**从原始 JSON 回答。
pub fn load_raw(name: &str) -> Json {
    let (path, text) = read_fixture(name);
    json::parse(&text).unwrap_or_else(|e| panic!("{}：{e}", path.display()))
}

/// 解析已读入的夹具文本。`expected_name` 必须与该 JSON 的 `name` 字段一致。
pub fn parse_fixture(expected_name: &str, text: &str) -> Fixture {
    parse_checked(expected_name, text)
        .unwrap_or_else(|e| panic!("夹具 `{expected_name}` 解析失败：{e}"))
}

fn parse_checked(expected_name: &str, text: &str) -> Result<Fixture, String> {
    let root = json::parse(text)?;
    if !matches!(root.kind, json::Kind::Obj(_)) {
        return Err("顶层必须是对象".to_string());
    }
    root.expect_keys(
        "fixture(顶层)",
        &[
            "name",
            "unit",
            "unit_note",
            "dataset_file",
            "dataset",
            "template_rows",
            "template_cols",
            "formulas",
            "row_cells",
            "calculated",
            "pagination",
            "merged",
            // M3-01：**只 engine_param 写**这个键（其余夹具没有，见
            // `GenEngineFixtures.java` 的写入条件）。`expect_keys` 是"不许多出键"
            // 的白名单，缺键不要求齐全 ⇒ 加进来对 15 个无参夹具零影响。
            "params",
        ],
    );
    let name = root.req("name").as_string();
    if name != expected_name {
        return Err(format!("自述名 `{name}` 与请求名 `{expected_name}` 不一致"));
    }
    Ok(Fixture {
        name,
        unit: root.req("unit").as_i8(),
        template_rows: root.req("template_rows").as_i32(),
        template_cols: root.req("template_cols").as_i32(),
        dataset: parse_dataset(root.req("dataset")),
        formulas: parse_formulas(root.req("formulas")),
        row_cells: parse_row_cells(root.req("row_cells")),
        calculated: parse_calculated(root.req("calculated")),
        // `null` 与缺键都归一成 None（产品错误型夹具写的是 null）
        pagination: root
            .get("pagination")
            .filter(|v| !v.is_null())
            .map(parse_pagination),
        merged: root
            .get("merged")
            .filter(|v| !v.is_null())
            .map(parse_merged),
        // `params` 缺键 = 无参夹具（15 个既有夹具的 JSON 里就没有这个键）。
        params: root.get("params").map(parse_params).unwrap_or_default(),
    })
}

/// `params` 段（M3-01）：`[{"name": "p", "value": [1,2,3]}, ...]`。
///
/// 出处：`GenEngineFixtures.java` 的 `params` 写入 + `EngineModelsM3.paramsJson`。
/// 产品侧这些值在 `CalcProbe.calc(rd, ds, params)` 里经 `Context.setParamValue`
/// 注入（`Context.java:163-168`），引擎侧由 `rpx_engine::calculate_with` 消费。
fn parse_params(j: &Json) -> Vec<super::ParamSpec> {
    j.arr()
        .iter()
        .map(|p| {
            p.expect_keys("param", &["name", "value"]);
            super::ParamSpec {
                name: p.req("name").as_string(),
                value: parse_param_value(p.req("value")),
            }
        })
        .collect()
}

/// 参数值的 JSON → [`rpx_engine::Value`]（按 JSON 形态推断，与 Java 侧
/// `paramValueJson` 的写入规则互逆）。
///
/// ⚠ **数字按 token 原文分家**：`1` → `Int`、`1.5` → `Double`。产品的
/// `Types.getProperData`（真机矩阵见 RUNLOG §24.5）有更细的类型面
/// （Long/Short/Byte/BigDecimal 各有其类），但那是 **Servlet 层的字符串 →
/// 类型**转换，与这里的 JSON → 引擎值不是同一层；当前参数注入面（Java 侧
/// `setParamValue` 传的是已定型对象）只需要 Int/Double 两档，来了新形态再补。
fn parse_param_value(j: &Json) -> rpx_engine::Value {
    use rpx_engine::Value;
    match &j.kind {
        json::Kind::Null => Value::Null,
        json::Kind::Bool(b) => Value::Bool(*b),
        // `Kind::Num` 的原文在 `raw`（`10` 与 `10.0` 必须可分，见 json 模块文档）。
        json::Kind::Num => {
            let raw = j.raw.as_str();
            if raw.contains('.') || raw.contains('e') || raw.contains('E') {
                Value::Double(j.as_f64())
            } else {
                Value::Int(j.as_i32())
            }
        }
        json::Kind::Str(s) => Value::Text(s.clone()),
        json::Kind::Arr(_) => Value::List(j.arr().iter().map(parse_param_value).collect()),
        json::Kind::Obj(_) => {
            panic!("参数值出现对象形态 —— 产品注入面（setParamValue）未见过，先补探针")
        }
    }
}

fn parse_dataset(j: &Json) -> Dataset {
    j.expect_keys("dataset", &["columns", "rows", "row_count"]);
    let str_list = |v: &Json| v.arr().iter().map(Json::as_string).collect::<Vec<String>>();
    Dataset {
        columns: str_list(j.req("columns")),
        rows: j.req("rows").arr().iter().map(str_list).collect(),
        row_count: j.req("row_count").as_i32(),
    }
}

fn parse_formulas(j: &Json) -> Vec<Formula> {
    j.arr()
        .iter()
        .map(|f| {
            f.expect_keys("formula", &["r", "c", "exp", "extend_mode", "merge_same"]);
            Formula {
                r: f.req("r").as_i32(),
                c: f.req("c").as_i32(),
                exp: f.req("exp").as_string(),
                extend_mode: f.get("extend_mode").map(Json::as_i8),
                merge_same: f.get("merge_same").map(Json::as_i8),
            }
        })
        .collect()
}

fn parse_row_cells(j: &Json) -> Vec<RowCellInfo> {
    const KNOWN: [&str; 4] = ["r", "row_type", "group_header_level", "row_height"];
    j.arr()
        .iter()
        .map(|rc| RowCellInfo {
            r: rc.req("r").as_i32(),
            row_type: rc.req("row_type").as_i32(),
            group_header_level: rc.req("group_header_level").as_i32(),
            row_height: rc.req("row_height").as_f64(),
            // 契约要求保留"任何出现的可选键"，故这里收集而非丢弃
            extra: rc
                .obj()
                .iter()
                .filter(|(k, _)| !KNOWN.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.raw.clone()))
                .collect(),
        })
        .collect()
}

fn parse_calculated(j: &Json) -> Calculated {
    if let Some(err) = j.get("error") {
        return Calculated::Error(parse_error(err));
    }
    j.expect_keys("calculated", &["rows", "cols", "cells"]);
    Calculated::Grid(Grid {
        rows: j.req("rows").as_i32(),
        cols: j.req("cols").as_i32(),
        cells: parse_cells(j.req("cells")),
    })
}

/// 各段的键白名单（产品写出什么，就锁什么）。
const CELL_KEYS: [&str; 8] = [
    "r",
    "c",
    "value",
    "type",
    "disp_value",
    "source",
    "left_head",
    "top_head",
];
const SOURCE_KEYS: [&str; 7] = [
    "src_row",
    "src_col",
    "row_merge",
    "col_merge",
    "extend",
    "sub_cells",
    "sub_values",
];
const PAGINATION_KEYS: [&str; 8] = [
    "paper_w",
    "paper_h",
    "columns",
    "page_count",
    "x_pages",
    "y_pages",
    "row_num_per_page",
    "pages",
];
const MERGED_CELL_KEYS: [&str; 6] = ["page", "r", "c", "value", "row_merge", "col_merge"];

fn parse_cells(j: &Json) -> Vec<Cell> {
    j.arr()
        .iter()
        .map(|cell| {
            cell.expect_keys("cell", &CELL_KEYS);
            Cell {
                r: cell.req("r").as_i32(),
                c: cell.req("c").as_i32(),
                // 原文切片，含引号/方括号；不做数值归一化
                value_raw: cell.req("value").raw.clone(),
                declared_type: cell.req("type").as_string(),
                disp_value: cell.get("disp_value").map(Json::as_string),
                source: cell.get("source").map(parse_source),
                left_head: parse_head(cell.get("left_head"), "cell.left_head"),
                top_head: parse_head(cell.get("top_head"), "cell.top_head"),
            }
        })
        .collect()
}

/// 解析 `left_head` / `top_head`（`{src_row, src_col}`，**模板坐标**，见 [`HeadRef`]）；缺键 ⇒ `None`。
fn parse_head(v: Option<&Json>, ctx: &str) -> Option<HeadRef> {
    let v = v?;
    v.expect_keys(ctx, &["src_row", "src_col"]);
    Some(HeadRef {
        src_row: v.req("src_row").as_i32(),
        src_col: v.req("src_col").as_i32(),
    })
}

fn parse_source(j: &Json) -> Source {
    j.expect_keys("source", &SOURCE_KEYS);
    Source {
        src_row: j.req("src_row").as_i32(),
        src_col: j.req("src_col").as_i32(),
        row_merge: j.req("row_merge").as_i32(),
        col_merge: j.req("col_merge").as_i32(),
        extend: j.req("extend").as_i8(),
        sub_cells: j.req("sub_cells").as_i32(),
        sub_values: j
            .get("sub_values")
            .map(|sv| sv.arr().iter().map(parse_sub_value).collect()),
    }
}

fn parse_sub_value(j: &Json) -> SubValue {
    j.expect_keys("sub_value", &["r", "c", "value", "type"]);
    SubValue {
        r: j.req("r").as_i32(),
        c: j.req("c").as_i32(),
        value_raw: j.req("value").raw.clone(),
        declared_type: j.req("type").as_string(),
    }
}

fn parse_error(j: &Json) -> CalcError {
    j.expect_keys(
        "error",
        &["class", "message", "cause_class", "cause_message", "chain"],
    );
    CalcError {
        class: j.req("class").as_string(),
        message: j.req("message").as_string(),
        cause_class: j.get("cause_class").map(Json::as_string),
        cause_message: j.get("cause_message").map(Json::as_string),
        chain: j.get("chain").map(Json::as_string),
    }
}

fn parse_pagination(j: &Json) -> Pagination {
    j.expect_keys("pagination", &PAGINATION_KEYS);
    Pagination {
        paper_w: j.req("paper_w").as_f64(),
        paper_h: j.req("paper_h").as_f64(),
        columns: j.req("columns").as_i32(),
        page_count: j.req("page_count").as_i32(),
        x_pages: j.req("x_pages").as_i32(),
        y_pages: j.req("y_pages").as_i32(),
        row_num_per_page: j.req("row_num_per_page").as_i32(),
        pages: j.req("pages").arr().iter().map(parse_page).collect(),
    }
}

fn parse_page(j: &Json) -> Page {
    j.expect_keys(
        "page",
        &["page", "rows", "first_value", "last_value", "col1_values"],
    );
    Page {
        page: j.req("page").as_i32(),
        rows: j.req("rows").as_i32(),
        first_value: j.get("first_value").map(Json::as_string),
        last_value: j.get("last_value").map(Json::as_string),
        col1_values: j
            .req("col1_values")
            .arr()
            .iter()
            .map(|v| v.raw.clone())
            .collect(),
    }
}

fn parse_merged(j: &Json) -> Merged {
    j.expect_keys("merged", &["page_count", "cells"]);
    Merged {
        page_count: j.req("page_count").as_i32(),
        cells: j.req("cells").arr().iter().map(parse_merged_cell).collect(),
    }
}

fn parse_merged_cell(j: &Json) -> MergedCell {
    j.expect_keys("merged_cell", &MERGED_CELL_KEYS);
    MergedCell {
        page: j.req("page").as_i32(),
        r: j.req("r").as_i32(),
        c: j.req("c").as_i32(),
        value_raw: j.req("value").raw.clone(),
        row_merge: j.req("row_merge").as_i32(),
        col_merge: j.req("col_merge").as_i32(),
    }
}
