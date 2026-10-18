//! 纯静态检查：只读模板 JSON，不取数、不计算、不导出。
//!
//! 为什么单独一条命令：`template validate` 只保证"结构对"，它明确把
//! `data_loading/calculation/pagination/export` 列为 deferred。于是有四类问题
//! **一路静默到成品**——不报错、不警告，产物就是错的：
//!
//! | 写法 | validate | 产物 |
//! | --- | --- | --- |
//! | `text` 里写 `${period}` | 通过 | 原样打印 `${period}` |
//! | 单元格声明 `border` | 通过 | 导出不落地（写了等于没写） |
//! | 扩展块与合计行之间留空行 | 通过 | 多一行空行，合计被推下去 |
//! | 同排多个 `vertical` 且不串父子链 | 通过 | 扩展相乘（6 行可炸成数百行） |
//!
//! 这里的检查全部是**静态可判**的，所以不需要数据也能跑，适合放进 CI 或
//! Agent 的"写完先自检"回路。动态问题（值算错、分页超宽）仍需真跑断言。
use crate::definition::address;
use serde_json::{json, Map, Value as J};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// 单次 lint 最多报这么多条，避免一份病态模板刷屏。
const MAX_ISSUES: usize = 50;
/// 默认纸张可用宽度（mm）：A4 纵向减默认左右边距的口径，与引擎默认一致。
const DEFAULT_USABLE_MM: f64 = 190.0;
/// 两位小数的数字在窄于该宽度的列里会显示 `####`。
const DECIMAL_COL_MM: f64 = 16.0;

struct Linter {
    issues: Vec<J>,
}

impl Linter {
    fn new() -> Self {
        Self { issues: Vec::new() }
    }

    fn push(
        &mut self,
        code: &str,
        level: &str,
        message: impl Into<String>,
        hint: impl Into<String>,
        pointer: Option<String>,
    ) {
        if self.issues.len() >= MAX_ISSUES {
            return;
        }
        let mut item = Map::new();
        item.insert("code".into(), json!(code));
        item.insert("level".into(), json!(level));
        item.insert("stage".into(), json!("lint"));
        item.insert("message".into(), json!(message.into()));
        let hint = hint.into();
        if !hint.is_empty() {
            item.insert("hint".into(), json!(hint));
        }
        if let Some(pointer) = pointer {
            item.insert("json_pointer".into(), json!(pointer));
        }
        self.issues.push(J::Object(item));
    }
}

fn u64_at(cell: &J, key: &str) -> Option<u64> {
    cell.get(key).and_then(J::as_u64)
}

/// 列宽（mm）。模板里缺省时按 0 计——`S007` 只关心合计，缺省列不额外放大。
fn col_widths(template: &J) -> BTreeMap<u64, f64> {
    template["col_props"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let col = u64_at(c, "col")?;
            let width = c.get("width").and_then(J::as_f64).unwrap_or(0.0);
            Some((col, width))
        })
        .collect()
}

fn usable_mm(template: &J) -> f64 {
    let setup = &template["print_setup"];
    let paper = setup.get("paper_width").and_then(J::as_f64);
    match paper {
        Some(paper) => {
            let left = setup.get("left_margin").and_then(J::as_f64).unwrap_or(0.0);
            let right = setup.get("right_margin").and_then(J::as_f64).unwrap_or(0.0);
            paper - left - right
        }
        None => DEFAULT_USABLE_MM,
    }
}

/// 文档渲染型模板（图表/图片画布）不是纸面报表，列宽口径不适用。
fn is_canvas(cells: &[&J]) -> bool {
    cells
        .iter()
        .any(|c| c.get("chart").is_some() || c.get("echarts").is_some() || c.get("image").is_some())
}

fn first_macro(text: &str) -> Option<&str> {
    let start = text.find("${")?;
    let rest = &text[start..];
    let end = rest.find('}').map(|i| i + 1).unwrap_or(rest.len());
    Some(&rest[..end])
}

/// 全部静态检查。`data_dir` 仅用于判断文件数据集是否存在（可选）。
pub fn lint(template: &J, data_dir: Option<&Path>) -> J {
    let mut l = Linter::new();
    let cells: Vec<&J> = template["cells"].as_array().into_iter().flatten().collect();
    let mut by_pos: BTreeMap<(u64, u64), (&J, usize)> = BTreeMap::new();
    for (index, cell) in cells.iter().enumerate() {
        if let (Some(row), Some(col)) = (u64_at(cell, "row"), u64_at(cell, "col")) {
            by_pos.entry((row, col)).or_insert((cell, index));
        }
    }

    // ---- L001 宏写进了 text -------------------------------------------------
    for ((row, col), (cell, index)) in &by_pos {
        let Some(text) = cell.get("text").and_then(J::as_str) else {
            continue;
        };
        if let Some(macro_text) = first_macro(text) {
            l.push(
                "L001",
                "warning",
                format!(
                    "{} 的 text 里写了宏 {macro_text}，但宏只在 expr 里替换",
                    address(*row, *col)
                ),
                "text 会原样上屏。把宏移到 expr（如 expr: \"\\\"${period} 销售表\\\"\"），或直接写字面量",
                Some(format!("/cells/{index}/text")),
            );
        }
    }

    // ---- L002 同排并列 vertical（扩展相乘） ---------------------------------
    let mut verticals: BTreeMap<u64, Vec<(u64, String)>> = BTreeMap::new();
    for ((row, col), (cell, _)) in &by_pos {
        if cell.get("extend_mode").and_then(J::as_str) == Some("vertical") {
            verticals
                .entry(*row)
                .or_default()
                .push((*col, address(*row, *col)));
        }
    }
    for (row, refs) in &verticals {
        if refs.len() < 2 {
            continue;
        }
        // 合法形态是"一棵树"：同排可以有多个 vertical，只要它们通过 left_head
        // 收敛到**唯一根**（兄弟节点共享父格时同步扩展，这是 detail-merge-same 的写法）。
        // 真正的病态是出现**多个根**——各自独立扩展，行数相乘。
        let addresses: BTreeSet<&str> = refs.iter().map(|(_, a)| a.as_str()).collect();
        let roots: Vec<&str> = refs
            .iter()
            .filter(|(col, _)| {
                by_pos
                    .get(&(*row, *col))
                    .and_then(|(cell, _)| cell.get("left_head").and_then(J::as_str))
                    .is_none_or(|head| !addresses.contains(head))
            })
            .map(|(_, a)| a.as_str())
            .collect();
        if roots.len() > 1 {
            l.push(
                "L002",
                "warning",
                format!(
                    "第 {row} 行有 {} 个互不指向的纵向扩展根：{}",
                    roots.len(),
                    roots.join(", ")
                ),
                "同排多个 vertical 必须用 left_head 收敛到唯一根（子级指向父格，兄弟可共享父格）；多根会各自扩展并相乘，6 行明细可能炸成数百行",
                None,
            );
        }
    }

    // ---- L003 扩展块内空行（合计被推下去） ---------------------------------
    if let Some(first) = verticals.keys().next().copied() {
        let last_used = by_pos.keys().map(|(r, _)| *r).max().unwrap_or(first);
        let used: BTreeSet<u64> = by_pos.keys().map(|(r, _)| *r).collect();
        for row in (first + 1)..last_used {
            if !used.contains(&row) {
                l.push(
                    "L003",
                    "warning",
                    format!(
                        "第 {row} 行在扩展块内是空行（扩展起始 {first}，最后使用行 {last_used}）"
                    ),
                    "扩展块与合计行之间不要留空行：空行会被算进扩展块，合计整体下移一行。删掉这一行并把后续行上移",
                    None,
                );
            }
        }
    }

    // ---- L004 border 不落地 -------------------------------------------------
    if let Some(((row, col), (_, index))) = by_pos
        .iter()
        .find(|(_, (cell, _))| cell.get("border").is_some())
    {
        let declared = cells.iter().filter(|c| c.get("border").is_some()).count();
        l.push(
            "L004",
            "warning",
            format!(
                "模板有 {declared} 个单元格声明了 border（首个是 {}），但当前导出不落地",
                address(*row, *col)
            ),
            "快照与 xlsx 都只带默认边框，写了等于没写。改用表头底色 + 合计行底色分层；不要靠边框表达结构",
            Some(format!("/cells/{index}/border")),
        );
    }

    // ---- L005 / L006 文件数据集 --------------------------------------------
    let base: Option<std::path::PathBuf> = data_dir.map(Path::to_path_buf);
    for (index, entry) in template["datasets"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let Some(file) = entry.get("file") else {
            continue;
        };
        let Some(name) = file.get("file_name").and_then(J::as_str) else {
            continue;
        };
        let dataset = entry.get("name").and_then(J::as_str).unwrap_or("<unnamed>");
        if let Some(base) = &base {
            let path = base.join(name);
            if !path.exists() {
                l.push(
                    "L005",
                    "warning",
                    format!("数据集 {dataset} 的文件不存在：{}", path.display()),
                    "file.file_name 是相对 --data-dir 的路径。核对文件名，或用 --data-dir 指向它所在目录",
                    Some(format!("/datasets/{index}/file/file_name")),
                );
            }
        }
        let lower = name.to_ascii_lowercase();
        let delimited = lower.ends_with(".csv") || lower.ends_with(".tsv");
        let has_delimiter = file
            .get("delimiter")
            .and_then(J::as_str)
            .is_some_and(|d| !d.is_empty());
        if delimited && !has_delimiter {
            l.push(
                "L006",
                "warning",
                format!("数据集 {dataset}（{name}）没有显式声明 delimiter"),
                "不写 delimiter 时不会按逗号切分：列名会变成整行文本（如「customer,amount」），随后所有 ds.field 都报 UnknownCell。加 \"delimiter\": \",\"",
                Some(format!("/datasets/{index}/file/delimiter")),
            );
        }
    }

    // ---- L007 列宽超出纸张 -------------------------------------------------
    let widths = col_widths(template);
    let total: f64 = widths.values().sum();
    let usable = usable_mm(template);
    if total > usable + 1.0 && !is_canvas(&cells) {
        l.push(
            "L007",
            "warning",
            format!("列宽合计 {total:.0}mm，超过可用宽度 {usable:.0}mm"),
            "超宽会横向分页（纸面上通常不可接受）。压列宽/减列；确需宽表就显式设 print_setup 用横向纸张",
            Some("/col_props".into()),
        );
    }

    // ---- L008 窄列放两位小数（显示 ####） ----------------------------------
    let mut flagged: BTreeMap<u64, (String, f64, usize)> = BTreeMap::new();
    for ((_, col), (cell, _)) in &by_pos {
        let Some(format) = cell.get("format").and_then(J::as_str) else {
            continue;
        };
        let decimals = format
            .split_once('.')
            .map(|(_, tail)| tail.chars().filter(|c| *c == '0').count())
            .unwrap_or(0);
        if decimals < 2 {
            continue;
        }
        let width = widths.get(col).copied().unwrap_or(99.0);
        if width < DECIMAL_COL_MM {
            let slot = flagged
                .entry(*col)
                .or_insert((format.to_string(), width, 0));
            slot.2 += 1;
        }
    }
    for (col, (format, width, count)) in flagged.into_iter().take(3) {
        l.push(
            "L008",
            "warning",
            format!("第 {col} 列（宽 {width:.0}mm）有 {count} 个格用 {format}"),
            "两位以上小数在窄列会显示 ####。改 #,##0，或把列宽提到 18mm 以上",
            Some("/col_props".into()),
        );
    }

    // ---- 汇总 --------------------------------------------------------------
    let count = |level: &str| -> usize {
        l.issues
            .iter()
            .filter(|i| i["level"].as_str() == Some(level))
            .count()
    };
    let (errors, warnings, infos) = (count("error"), count("warning"), count("info"));
    let codes: BTreeSet<&str> = l.issues.iter().filter_map(|i| i["code"].as_str()).collect();
    json!({
        "issues": l.issues,
        "summary": {
            "total": errors + warnings + infos,
            "error": errors,
            "warning": warnings,
            "info": infos,
            "codes": codes,
        },
        "checked": ["text_macros", "extension_shape", "extension_gaps", "border_support",
                    "dataset_files", "dataset_delimiters", "page_width", "number_formats"],
        "deferred": ["data_loading", "calculation", "pagination", "export"],
        "note": "lint 是静态检查：它证明不了值算对，只拦四类静默失败。值对不对必须真跑并断言产物",
    })
}
