//! rpx-cli：`rpx print <file.rpx>` 矩阵打印（AC-07）。
//!
//! 输出内容：矩阵维度、行高列宽表、合并区清单、非空单元格摘要
//! （坐标 / value / extendMode / 合并标记）、params/datasets 计数、print_setup 摘要。

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

fn run(args: &[String]) -> rpx_core::Result<()> {
    let mut path: Option<&str> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "print" => {
                i += 1;
                path = args.get(i).map(|s| s.as_str());
            }
            other => {
                eprintln!("unknown argument: {}", other);
                std::process::exit(2);
            }
        }
        i += 1;
    }
    let Some(path) = path else {
        eprintln!("usage: rpx-cli print <file.rpx>");
        std::process::exit(2);
    };
    let file = std::fs::File::open(path)?;
    let report = rpx_core::open(file)?;
    print!("{}", render(&report));
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
