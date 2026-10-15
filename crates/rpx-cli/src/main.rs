//! rpx-cli：`rpx print <file.rpx>` 矩阵打印（AC-07）+ `rpx render <file.rpx>
//! --out <out.csv|out.xlsx>` 离线导出（M6-04）。
//!
//! print 输出内容：矩阵维度、行高列宽表、合并区清单、非空单元格摘要
//! （坐标 / value / extendMode / 合并标记）、params/datasets 计数、print_setup 摘要。
//!
//! render（M6-04）与 server `POST /export` **同构的链路**：模板 → 数据集
//! （`--data-dir`，缺省 = `.rpx` 同目录）→ `calculate_with` + 分页守卫 →
//! `rpx-export` 导出 → 写 `--out`。格式由扩展名决定（`.csv` / `.xlsx`，
//! 不区分大小写）。⛔ 参数（`params`）支持整体后置（spec-m6 §5.2 带账）：
//! 无参模板可直接导，`@p` 模板报「未登记参数」——CLI 的值推断口径
//! （List 需 JSON）无基准前不拍板。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rpx_core::reader::ObjValue;
use rpx_core::ReportDefine;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    match args.get(1).map(String::as_str) {
        Some("print") => {
            let Some(path) = args.get(2) else {
                eprintln!("usage: rpx-cli print <file.rpx>");
                std::process::exit(2);
            };
            let file = std::fs::File::open(path).map_err(|e| format!("打开 {path} 失败：{e}"))?;
            let report = rpx_core::open(file).map_err(|e| e.to_string())?;
            print!("{}", render(&report));
            Ok(())
        }
        Some("render") => render_cmd(&args[2..]),
        other => {
            if let Some(c) = other {
                eprintln!("unknown command: {c}");
            }
            eprintln!("usage: rpx-cli <print|render> ...");
            std::process::exit(2);
        }
    }
}

/// `rpx-cli render <file.rpx> --out <out.csv|out.xlsx> [--data-dir <dir>]`（M6-04）。
///
/// 离线导出（审计遗留③的闭合形态：凡验收写「CLI X」必有真跑 CLI 的断言——
/// 断言在 `tests/cli_render.rs`）。
fn render_cmd(rest: &[String]) -> Result<(), String> {
    let mut path: Option<&str> = None;
    let mut out: Option<&str> = None;
    let mut data_dir: Option<&str> = None;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--out" => {
                i += 1;
                out = rest.get(i).map(String::as_str);
                if out.is_none() {
                    eprintln!("--out 缺参数值");
                    std::process::exit(2);
                }
            }
            "--data-dir" => {
                i += 1;
                data_dir = rest.get(i).map(String::as_str);
                if data_dir.is_none() {
                    eprintln!("--data-dir 缺参数值");
                    std::process::exit(2);
                }
            }
            s if s.starts_with("--") => {
                eprintln!("unknown option: {s}");
                std::process::exit(2);
            }
            s if path.is_none() => path = Some(s),
            s => {
                eprintln!("多余的位置参数：{s}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    let (Some(path), Some(out)) = (path, out) else {
        eprintln!("usage: rpx-cli render <file.rpx> --out <out.csv|out.xlsx> [--data-dir <dir>]");
        std::process::exit(2);
    };
    // 格式 = `--out` 扩展名（不区分大小写——文件系统惯例；未知扩展名按
    // 用法错误 exit 2，与其它参数错一致）。
    let is_csv = match Path::new(out)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("csv") => true,
        Some("xlsx") => false,
        _ => {
            eprintln!("--out 需要 .csv 或 .xlsx 扩展名：{out}");
            std::process::exit(2);
        }
    };
    // 数据集目录：缺省 = `.rpx` 所在目录（模板的 `file_name` 是相对文件名）。
    let dir: PathBuf = match data_dir {
        Some(d) => PathBuf::from(d),
        None => Path::new(path)
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf(),
    };
    let report_bytes = std::fs::read(path).map_err(|e| format!("读取模板失败 {path}：{e}"))?;
    let rd = rpx_core::open_bytes(&report_bytes).map_err(|e| format!("解析模板失败：{e}"))?;
    // 数据集加载：`file_name` 相对 `dir` 直读；缺失 → `MissingDataset`（显式）。
    // SQL 数据集：CLI 无连接通道（`None`）→ `SqlDatasetNoConnection`（显式，
    // 不静默跳过——同 `load_datasets_full` 的契约）。
    let ds = rpx_engine::dataset::load_datasets_full(
        &rd,
        &|file_name| std::fs::read(dir.join(file_name)).ok(),
        None,
    )
    .map_err(|e| format!("数据集加载失败：{e}"))?;
    let mut grid = rpx_engine::calculate_with(&rd, &ds, &rpx_engine::params::ParamTable::EMPTY)
        .map_err(|e| format!("计算失败：{e}"))?;
    // 与 server `/export` 同构：跑模板真值口径的分页（守卫在此复核）。
    // 导出字节本身不分页（`pagination`/`page_count` 不参与 CSV/xlsx 输出）。
    let pg = rpx_engine::pager::paginate_with_setup(&grid, &rd.print_setup)
        .map_err(|e| format!("分页失败：{e}"))?;
    grid.page_count = pg.page_count();
    grid.pagination = Some(pg);
    let bytes = if is_csv {
        rpx_export::to_csv(&grid).into_bytes()
    } else {
        rpx_export::to_xlsx(&grid)
    };
    std::fs::write(out, &bytes).map_err(|e| format!("写入失败 {out}：{e}"))?;
    println!("exported: {out} ({} bytes)", bytes.len());
    Ok(())
}

fn fmt_f(v: f32) -> String {
    if (v - v.round()).abs() < 1e-5 {
        format!("{}", v.round() as i64)
    } else {
        format!("{}", v)
    }
}

fn value_str(v: &ObjValue) -> String {
    match v {
        ObjValue::Null => "null".to_string(),
        ObjValue::Str(s) => format!("\"{}\"", s),
        ObjValue::Int(n) => n.to_string(),
        ObjValue::Long(n) => n.to_string(),
        ObjValue::Double(d) => format!("{}", d),
        ObjValue::Float(f) => format!("{}", f),
        ObjValue::Bool(b) => b.to_string(),
        ObjValue::Byte(b) => b.to_string(),
        ObjValue::Short(s) => s.to_string(),
        ObjValue::BigDecimal { scale, unscaled } => {
            format!("BigDecimal(scale={}, {} bytes)", scale, unscaled.len())
        }
        ObjValue::BigInteger(b) => format!("BigInteger({} bytes)", b.len()),
        ObjValue::Bytes(b) => format!("bytes({})", b.len()),
        ObjValue::Millis { tag, millis } => format!("Millis(tag={}, {})", tag, millis),
        ObjValue::Obj { class_name, raw } => {
            format!(
                "Obj({}, {} bytes)",
                class_name.as_deref().unwrap_or("?"),
                raw.len()
            )
        }
    }
}

/// 报表摘要渲染（纯函数，便于单测）。
pub fn render(r: &ReportDefine) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "matrix: {}x{} (logical {}x{})\n",
        r.rows,
        r.cols,
        r.rows.saturating_sub(1),
        r.cols.saturating_sub(1)
    ));

    s.push_str("row_heights: [");
    for row in 1..r.rows {
        if row > 1 {
            s.push_str(", ");
        }
        s.push_str(&fmt_f(r.row_height(row).unwrap_or(0.0)));
    }
    s.push_str("]\n");

    s.push_str("col_widths:  [");
    for col in 1..r.cols {
        if col > 1 {
            s.push_str(", ");
        }
        s.push_str(&fmt_f(r.col_width(col).unwrap_or(0.0)));
    }
    s.push_str("]\n");

    if r.merges.is_empty() {
        s.push_str("merges: none\n");
    } else {
        s.push_str("merges:\n");
        for m in &r.merges {
            s.push_str(&format!(
                "  ({},{})..({},{})\n",
                m.begin_row, m.begin_col, m.end_row, m.end_col
            ));
        }
    }

    let mut count = 0usize;
    for row in 1..r.rows {
        for col in 1..r.cols {
            if r.is_blank(row, col) {
                continue;
            }
            count += 1;
            let cell = r.cell_at(row, col).expect("non-blank");
            let mut line = format!("  ({},{}): value={}", row, col, value_str(&cell.value));
            if let Some(dv) = &cell.disp_value {
                line.push_str(&format!(" dispValue=\"{}\"", dv));
            }
            if cell.extend_mode != 16 {
                line.push_str(&format!(" extendMode={}", cell.extend_mode));
            }
            if let Some(a) = cell.merged_area {
                line.push_str(&format!(
                    " merged=({},{})..({},{})",
                    a.begin_row, a.begin_col, a.end_row, a.end_col
                ));
            }
            s.push_str(&line);
            s.push('\n');
        }
    }
    s.push_str(&format!("cells: {} non-default\n", count));
    s.push_str(&format!(
        "params: {}  datasets: {}\n",
        r.params.len(),
        r.datasets.len()
    ));
    let ps = &r.print_setup;
    s.push_str(&format!(
        "print_setup: paper={} {}x{} orient={} margins(l={},r={},t={},b={}) copies={}\n",
        ps.paper,
        fmt_f(ps.paper_width),
        fmt_f(ps.paper_height),
        ps.orient,
        fmt_f(ps.left_margin),
        fmt_f(ps.right_margin),
        fmt_f(ps.top_margin),
        fmt_f(ps.bottom_margin),
        ps.copy_count
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_dimensions_line() {
        // 构造最小报表不走完整解析，直接验证格式串形态
        let out = fmt_f(24.0);
        assert_eq!(out, "24");
        assert_eq!(fmt_f(92.5), "92.5");
    }
}
