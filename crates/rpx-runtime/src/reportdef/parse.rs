//! 定义 JSON → [`Builder`] 调用（**逐字段校验**，错误消息带 JSON 路径）。
//!
//! Schema 正文（字段表 / 语义 / 示例）：`docs/report-def-json.md` —— 本模块只作
//! 实现，不复制那张表；改契约先改文档。
//!
//! # 两条校验纪律
//!
//! 1. **不猜**：未知的 `version`、未知的枚举取值、越界的行列号一律**显式报错**，
//!    不做「宽松接受 + 忽略」——静默忽略正是本仓反复踩过的失败形态。
//! 2. **不静默丢内容**：落在他人合并矩形内的格，若是**空壳**则跳过（Univer 导出
//!    的自然形态），若**带内容**则报错（否则内容会被悄悄丢掉）。

use std::collections::HashSet;

use serde_json::Value as J;

use rpx_core::cell::Cell;
use rpx_core::dataset::{factory_class_for, DataSetEntry, FileDataSetConfig, SqlDataSetConfig};
use rpx_core::ReportDefine;

use super::build::{
    self, Builder, KEY_COL_WIDTH, KEY_ROW_BREAK_PAGE, KEY_ROW_HEIGHT, MERGE_SAME_BOTH,
    MERGE_SAME_HORIZONTAL, MERGE_SAME_VERTICAL,
};
use super::cellprops;
use super::heads;
use super::json::{arr, as_str, as_u64, as_usize, err, field, kind_of, obj, opt_str};
use super::macros;
use super::params;
use super::printsetup;
use super::style;
use super::DefError;

/// 定义 JSON 的当前版本（`version` 字段；缺省视为 1）。
pub const VERSION: u64 = 1;

/// SQL 数据集配置类名（`factory_class_for` 的键，见 `rpx-core/src/dataset.rs`）。
const CLASS_SQL: &str = "com.raqsoft.reportl.usermodel.SQLDataSetConfig";
/// 文件数据集配置类名。
const CLASS_FILE: &str = "com.raqsoft.reportl.usermodel.FileDataSetConfig";

/// 定义 JSON → 模板，宏保存在模板自定义属性中。
pub fn build(text: &str) -> Result<ReportDefine, DefError> {
    build_full(text).map(|(rd, _)| rd)
}

/// 定义 JSON → 模板及可直接传给 SQL 供数层的宏表。
pub fn build_full(text: &str) -> Result<(ReportDefine, rpx_engine::MacroTable), DefError> {
    let root: J = serde_json::from_str(text).map_err(|e| DefError::Json(e.to_string()))?;
    crate::definition::strict(&root)?;
    let root = obj(&root, "$")?;

    // `version` 可选；给定了就必须是本实现支持的版本（未知版本不猜）。
    if let Some(v) = root.get("version") {
        let n = as_u64(v, "$.version")?;
        if n != VERSION {
            return Err(err(
                "$.version",
                format!("不支持的定义版本 {n}（本实现支持 {VERSION}）"),
            ));
        }
    }

    let grid = obj(field(root, "grid", "$")?, "$.grid")?;
    let rows = as_usize(field(grid, "rows", "$.grid")?, "$.grid.rows")?;
    let cols = as_usize(field(grid, "cols", "$.grid")?, "$.grid.cols")?;
    if rows == 0 || cols == 0 {
        return Err(err(
            "$.grid",
            format!("rows/cols 必须 ≥1（实得 {rows}×{cols}）"),
        ));
    }
    // The definition is materialized as a dense grid, including the scaffold row/column.
    if rows
        .checked_add(1)
        .and_then(|r| cols.checked_add(1).and_then(|c| r.checked_mul(c)))
        .is_none_or(|n| n > 1_000_000)
    {
        return Err(err("$.grid", "模板网格含内部首行首列最多 1,000,000 槽"));
    }

    let mut b = Builder::new(rows, cols);

    // ---- 单元格：两趟。先解析 + 收集全部合并矩形，再落盘 ----
    // （顺序无关：非锚点槽可能在锚点之前声明，一趟处理会把它当独立格 put 进 store。）
    let mut specs: Vec<Spec> = Vec::new();
    if let Some(cells) = root.get("cells") {
        for (i, c) in arr(cells, "$.cells")?.iter().enumerate() {
            specs.push(parse_cell(c, &format!("$.cells[{i}]"), rows, cols)?);
        }
    }
    let mut covered: HashSet<(usize, usize)> = HashSet::new();
    for s in &specs {
        if let Some((mr, mc)) = s.merge {
            for r in s.row..s.row + mr {
                for c in s.col..s.col + mc {
                    if (r, c) != (s.row, s.col) {
                        covered.insert((r, c));
                    }
                }
            }
        }
    }
    // 已落盘格 `(store 下标, JSON 路径, 行, 列)` —— 供第三趟的声明式主格校验回读。
    let mut placed: Vec<heads::Placed> = Vec::new();
    for s in specs {
        if covered.contains(&(s.row, s.col)) {
            if s.has_content {
                return Err(err(
                    &s.path,
                    format!(
                        "格 ({},{}) 落在另一个合并矩形内，其内容会被静默丢弃 —— 只声明合并锚点",
                        s.row, s.col
                    ),
                ));
            }
            continue; // 空壳：Univer 导出的自然形态，跳过
        }
        let path = s.path.clone();
        let id = b.put(s.row, s.col, s.cell);
        if let Some((mr, mc)) = s.merge {
            b.merge(s.row, s.col, s.row + mr - 1, s.col + mc - 1);
        }
        if let Some(m) = s.merge_same {
            b.same_merge(id, m);
        }
        placed.push((id, path, s.row, s.col));
    }

    // ---- 声明式主格校验（**第三趟**，必须在全部格落盘之后）----
    // 存在性 + 教程 `dpkz101` 的三条硬约束里的 ①/③（② 由引擎拦，见 [`heads`]）。
    heads::check_declared_heads(&b, &placed, rows, cols)?;

    // ---- 数据集 ----
    if let Some(ds) = root.get("datasets") {
        for (i, d) in arr(ds, "$.datasets")?.iter().enumerate() {
            b.push_dataset(parse_dataset(d, &format!("$.datasets[{i}]"))?);
        }
    }

    // ---- 行属性 ----
    if let Some(rp) = root.get("row_props") {
        for (i, p) in arr(rp, "$.row_props")?.iter().enumerate() {
            let path = format!("$.row_props[{i}]");
            let p = obj(p, &path)?;
            let r = as_usize(field(p, "row", &path)?, &format!("{path}.row"))?;
            if r == 0 || r > rows {
                return Err(err(
                    &format!("{path}.row"),
                    format!("行号 {r} 越界（1..={rows}）"),
                ));
            }
            super::bands::row(&mut b, r, p, &path)?;
            if let Some(h) = p.get("height") {
                if h.is_number() {
                    b.row_mut(r).row_height = super::json::as_f32(h, &format!("{path}.height"))?;
                } else {
                    b.row_prop(r, KEY_ROW_HEIGHT, &prop_expr(h, &format!("{path}.height"))?);
                }
            }
            if let Some(bp) = p.get("break_page") {
                if let Some(v) = bp.as_bool() {
                    b.row_mut(r).break_page = v;
                } else {
                    b.row_prop(
                        r,
                        KEY_ROW_BREAK_PAGE,
                        &bool_or_expr(bp, &format!("{path}.break_page"))?,
                    );
                }
            }
        }
    }

    // ---- 列属性 ----
    if let Some(cp) = root.get("col_props") {
        for (i, p) in arr(cp, "$.col_props")?.iter().enumerate() {
            let path = format!("$.col_props[{i}]");
            let p = obj(p, &path)?;
            let c = as_usize(field(p, "col", &path)?, &format!("{path}.col"))?;
            if c == 0 || c > cols {
                return Err(err(
                    &format!("{path}.col"),
                    format!("列号 {c} 越界（1..={cols}）"),
                ));
            }
            super::bands::col(&mut b, c, p, &path)?;
            if let Some(w) = p.get("width") {
                if w.is_number() {
                    b.col_mut(c).col_width = super::json::as_f32(w, &format!("{path}.width"))?;
                } else {
                    b.col_prop(c, KEY_COL_WIDTH, &prop_expr(w, &format!("{path}.width"))?);
                }
            }
        }
    }

    // ---- 参数声明（落在 Builder 之外：`finish()` 之后填，避免污染网格装配） ----
    let mut rd = b.finish();
    rd.params = params::parse_params(root)?;
    printsetup::apply_print_setup(&mut rd.print_setup, root)?;
    super::print_config::apply(&mut rd, root)?;
    super::graphics::apply(&mut rd, root)?;
    let macros = macros::parse_macros(root)?;
    if let Some(J::Object(values)) = root.get("macros") {
        for (name, value) in values {
            rd.custom_properties.push((
                format!("rust_report.macro.{name}"),
                value.as_str().map(str::to_string),
            ));
        }
    }
    Ok((rd, macros))
}

/// 一个待落盘的格（解析产物）。
struct Spec {
    /// JSON 路径（错误定位）。
    path: String,
    row: usize,
    col: usize,
    cell: Cell,
    /// 是否声明了 `text`/`expr`（用于「被合并覆盖」时的静默丢内容判定）。
    has_content: bool,
    /// 合并矩形尺寸 `(rows, cols)`（声明在锚点上）。
    merge: Option<(usize, usize)>,
    /// 同值合并模式（`98/99/100`）。
    merge_same: Option<i8>,
}

fn parse_cell(v: &J, path: &str, rows: usize, cols: usize) -> Result<Spec, DefError> {
    let o = obj(v, path)?;
    let row = as_usize(field(o, "row", path)?, &format!("{path}.row"))?;
    let col = as_usize(field(o, "col", path)?, &format!("{path}.col"))?;
    if row == 0 || row > rows {
        return Err(err(
            &format!("{path}.row"),
            format!("行号 {row} 越界（1..={rows}）"),
        ));
    }
    if col == 0 || col > cols {
        return Err(err(
            &format!("{path}.col"),
            format!("列号 {col} 越界（1..={cols}）"),
        ));
    }

    let text = opt_str(o, "text", path)?;
    let expr = opt_str(o, "expr", path)?;
    if [text.is_some(), expr.is_some(), o.contains_key("literal")]
        .iter()
        .filter(|b| **b)
        .count()
        > 1
    {
        return Err(err(path, "`text` 与 `expr` 互斥（同一格只能二选一）"));
    }
    let mut cell = match (text, expr) {
        (Some(t), None) => build::text_cell(t),
        (None, Some(e)) => build::expr_cell(e),
        _ => build::blank_cell(),
    };
    if let Some(v) = o.get("literal") {
        cell.value = super::literal::decode(v, &format!("{path}.literal"))?;
    }
    cell.disp_value = opt_str(o, "disp_value", path)?.map(str::to_string);
    let has_content = text.is_some()
        || expr.is_some()
        || o.contains_key("literal")
        || o.contains_key("disp_value")
        || o.contains_key("data_type");

    if let Some(h) = o.get("h_align") {
        cell.h_align = parse_h_align(as_str(h, &format!("{path}.h_align"))?, path)?;
    }
    if let Some(v) = o.get("v_align") {
        cell.v_align = parse_v_align(as_str(v, &format!("{path}.v_align"))?, path)?;
    }

    // 样式（字体 / 字号 / 粗斜下划线 / 前景背景色）放在「内容之后、几何之前」：
    // 与 merge 互不相干，逐字段独立生效（未声明即保持 build 的契约默认）。
    style::apply_style(&mut cell, o, path)?;
    // 属性扩展（边框 / 数字格式 / 缩进 / 旋转 / 扩展模式）：同一条纪律，另 16 个槽。
    cellprops::apply_cell_props(&mut cell, o, path)?;

    let merge = match o.get("merge") {
        None => None,
        Some(m) => {
            let mp = format!("{path}.merge");
            let m = obj(m, &mp)?;
            let mr = as_usize(field(m, "rows", &mp)?, &format!("{mp}.rows"))?;
            let mc = as_usize(field(m, "cols", &mp)?, &format!("{mp}.cols"))?;
            if mr == 0 || mc == 0 {
                return Err(err(&mp, format!("rows/cols 必须 ≥1（实得 {mr}×{mc}）")));
            }
            if mr == 1 && mc == 1 {
                return Err(err(&mp, "1×1 合并无意义（Excel 也不允许单格合并）"));
            }
            if row + mr - 1 > rows || col + mc - 1 > cols {
                return Err(err(
                    &mp,
                    format!("合并矩形 ({row},{col})+{mr}×{mc} 超出网格 {rows}×{cols}"),
                ));
            }
            Some((mr, mc))
        }
    };

    let merge_same = match o.get("merge_same") {
        None => None,
        Some(s) => {
            let sp = format!("{path}.merge_same");
            Some(match as_str(s, &sp)? {
                "vertical" => MERGE_SAME_VERTICAL,
                "horizontal" => MERGE_SAME_HORIZONTAL,
                "both" => MERGE_SAME_BOTH,
                other => {
                    return Err(err(
                        &sp,
                        format!("未知取值 {other:?}（可选 vertical / horizontal / both）"),
                    ))
                }
            })
        }
    };

    Ok(Spec {
        path: path.to_string(),
        row,
        col,
        cell,
        has_content,
        merge,
        merge_same,
    })
}

/// 数据集：`{"name": …, "sql": {"dsn","sql"}}` 或 `{"name": …, "file": {"file_name","encoding"?}}`。
fn parse_dataset(v: &J, path: &str) -> Result<DataSetEntry, DefError> {
    let o = obj(v, path)?;
    let name = opt_str(o, "name", path)?.map(str::to_string);
    match (o.get("sql"), o.get("file")) {
        (Some(_), Some(_)) => Err(err(path, "`sql` 与 `file` 互斥")),
        (None, None) => Err(err(path, "必须给 `sql` 或 `file` 之一")),
        (Some(s), None) => {
            let sp = format!("{path}.sql");
            let s = obj(s, &sp)?;
            let dsn = as_str(field(s, "dsn", &sp)?, &format!("{sp}.dsn"))?.to_string();
            let text = as_str(field(s, "sql", &sp)?, &format!("{sp}.sql"))?.to_string();
            Ok(DataSetEntry {
                factory_class: factory_class_for(CLASS_SQL),
                class_name: CLASS_SQL.to_string(),
                file: None,
                sql: Some(SqlDataSetConfig {
                    name,
                    data_source_name: Some(dsn),
                    release_mode: 0,
                    cache_data: false,
                    col_titles: None,
                    no_data_prompt: false,
                    sql: Some(text),
                    begin_row: None,
                    end_row: None,
                    schema: None,
                    params: None,
                    is_guarded: false,
                    threshold: -1,
                }),
                raw: Vec::new(),
            })
        }
        (None, Some(f)) => {
            let fp = format!("{path}.file");
            let f = obj(f, &fp)?;
            let file_name =
                as_str(field(f, "file_name", &fp)?, &format!("{fp}.file_name"))?.to_string();
            let encoding = opt_str(f, "encoding", &fp)?.map(str::to_string);
            let sheet_name = opt_str(f, "sheet_name", &fp)?.map(str::to_string);
            let delimiter = opt_str(f, "delimiter", &fp)?.unwrap_or("\t");
            if delimiter.len() != 1
                || !delimiter.is_ascii()
                || matches!(delimiter, "\n" | "\r" | "\"")
            {
                return Err(err(
                    &format!("{fp}.delimiter"),
                    "分隔符必须是一个 ASCII 字符，且不能为引号或换行",
                ));
            }
            Ok(DataSetEntry {
                factory_class: factory_class_for(CLASS_FILE),
                class_name: CLASS_FILE.to_string(),
                sql: None,
                file: Some(FileDataSetConfig {
                    name,
                    data_source_name: None,
                    release_mode: 0,
                    cache_data: false,
                    col_titles: None,
                    no_data_prompt: false,
                    file_name: Some(file_name),
                    sheet_name,
                    encoding,
                    col_list: None,
                    title: true,
                    quote_removed: false,
                    seperator: delimiter.as_bytes()[0] as i16,
                    col_type_list: None,
                    col_format_list: None,
                    filter: None,
                    gather_col_list: None,
                    gather_fun_list: None,
                    group_col_list: None,
                    select_col_list: None,
                    change_file: false,
                    begin_row: None,
                    end_row: None,
                }),
                raw: Vec::new(),
            })
        }
    }
}

/// 水平对齐：取值语义见 `docs/format-contract.md` 附件 B.3 注 3
/// （`NormalCell.java:1225-1235`）。
fn parse_h_align(s: &str, path: &str) -> Result<i8, DefError> {
    Ok(match s {
        "left" => -48,
        "center" => -47,
        "right" => -46,
        "justify" => -45,
        "distribute" => -44,
        other => {
            return Err(err(
                &format!("{path}.h_align"),
                format!("未知取值 {other:?}（可选 left / center / right / justify / distribute）"),
            ))
        }
    })
}

/// 垂直对齐：同上（`NormalCell.java:1237-1244`）。
fn parse_v_align(s: &str, path: &str) -> Result<i8, DefError> {
    Ok(match s {
        "top" => -32,
        "middle" => -31,
        "bottom" => -30,
        other => {
            return Err(err(
                &format!("{path}.v_align"),
                format!("未知取值 {other:?}（可选 top / middle / bottom）"),
            ))
        }
    })
}

/// 尺寸类属性（行高/列宽）：数字 → 字面量；字符串 → 当作**表达式**原样透传
/// （引擎侧这些槽是表达式，见 `build.rs` 的键说明）。
fn prop_expr(v: &J, path: &str) -> Result<String, DefError> {
    match v {
        J::Number(n) => Ok(n.to_string()),
        J::String(s) => Ok(s.clone()),
        other => Err(err(
            path,
            format!("应为数字或表达式字符串，实得 {}", kind_of(other)),
        )),
    }
}

/// 布尔或表达式（`break_page`）：`true`→`"1==1"`、`false`→`"1==0"`；字符串原样透传。
pub(super) fn bool_or_expr(v: &J, path: &str) -> Result<String, DefError> {
    match v {
        J::Bool(true) => Ok("1==1".to_string()),
        J::Bool(false) => Ok("1==0".to_string()),
        J::String(s) => Ok(s.clone()),
        other => Err(err(
            path,
            format!("应为布尔或表达式字符串，实得 {}", kind_of(other)),
        )),
    }
}
