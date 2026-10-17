//! 打印工作簿：一张工作表对应一张引擎页面，保留重复区带与跨页合并。
use rpx_engine::{
    arena::CalcGrid,
    pager::{paginate_with_setup, project_page, unit_to_pt, PrintSetup},
};
use rust_xlsxwriter::XlsxError;
mod graphics;
pub(crate) mod style;

/// 按模板纸张与页映射导出。错误直接返回，不生成半份工作簿。
pub fn to_print_xlsx(grid: &CalcGrid, setup: &PrintSetup) -> Result<Vec<u8>, String> {
    if setup.back_graph_printed && grid.page_background.is_some() {
        return Err(
            "Excel 不支持打印工作表背景图片；请使用 CSV/XLSX，或从 Web 打印预览另存 PDF".into(),
        );
    }
    let paper = if setup.paper == 256 {
        standard_paper(setup.paper_width, setup.paper_height)?
    } else {
        u8::try_from(setup.paper).map_err(|_| {
            "Excel 不支持此自定义纸张代码；请使用 CSV/XLSX，或从 Web 打印预览另存 PDF".to_string()
        })?
    };
    let pages = paginate_with_setup(grid, setup).map_err(|e| e.to_string())?;
    let mut wb = crate::xlsx::new_workbook();
    let mut data = CalcGrid::new(0, 0);
    for page in &pages.pages {
        let local = project_page(grid, page);
        let ws = wb.add_worksheet();
        ws.set_name(format!("Page {}", page.page))
            .map_err(message)?;
        for (i, &height) in local.row_heights.iter().enumerate() {
            let pt = unit_to_pt(height, local.unit);
            if pt > 0.0 {
                ws.set_row_height(i as u32, pt).map_err(message)?;
            }
            if local.row_visible.get(i) == Some(&false) {
                ws.set_row_hidden(i as u32).map_err(message)?;
            }
        }
        for (i, &width) in local.col_widths.iter().enumerate() {
            let px = unit_to_pt(width, local.unit) * 96.0 / 72.0;
            if px > 0.0 {
                ws.set_column_width_pixels(i as u16, px.round() as u32)
                    .map_err(message)?;
            }
            if local.col_visible.get(i) == Some(&false) {
                ws.set_column_hidden(i as u16).map_err(message)?;
            }
        }
        crate::xlsx::write_print_sheet(&local, ws);
        graphics::write(ws, &local.graphics, &mut data)?;
        if local.rows > 0 && local.cols > 0 {
            ws.set_print_area(0, 0, (local.rows - 1) as u32, (local.cols - 1) as u16)
                .map_err(message)?;
        }
        ws.set_paper_size(paper);
        ws.set_margins(
            f64::from(setup.left_margin) / 25.4,
            f64::from(setup.right_margin) / 25.4,
            f64::from(setup.top_margin) / 25.4,
            f64::from(setup.bottom_margin) / 25.4,
            0.0,
            0.0,
        );
        if setup.paper_width > setup.paper_height {
            ws.set_landscape();
        } else {
            ws.set_portrait();
        }
        ws.set_print_fit_to_pages(1, 1);
    }
    if graphics::has_data(&data) {
        let ws = wb.add_worksheet();
        ws.set_name("_chart_data").map_err(message)?;
        ws.set_hidden(true);
        crate::xlsx::write_sheet(&data, ws);
    }
    wb.save_to_buffer().map_err(message)
}

fn message(error: XlsxError) -> String {
    error.to_string()
}

/// 自定义纸张（`paper=256`）→ Excel 标准纸码。只映射可判定的标准尺寸；
/// 其他尺寸显式失败；CLI 不直接生成 PDF，可从 Web 打印预览另存。
fn standard_paper(width: f32, height: f32) -> Result<u8, String> {
    let (short, long) = if width <= height {
        (width, height)
    } else {
        (height, width)
    };
    let close = |a: f32, b: f32| (a - b).abs() < 1.0;
    let code = if close(short, 210.0) && close(long, 297.0) {
        9 // A4
    } else if close(short, 297.0) && close(long, 420.0) {
        8 // A3
    } else if close(short, 148.0) && close(long, 210.0) {
        11 // A5
    } else if close(short, 215.9) && close(long, 279.4) {
        1 // Letter
    } else {
        return Err(
            "Excel 不支持该自定义纸张尺寸；请使用 A3/A4/A5/Letter，或从 Web 打印预览另存 PDF"
                .into(),
        );
    };
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::standard_paper;

    #[test]
    fn custom_paper_maps_only_known_standard_sizes() {
        assert_eq!(standard_paper(420.0, 297.0), Ok(8));
        assert_eq!(standard_paper(297.0, 420.0), Ok(8));
        assert_eq!(standard_paper(210.0, 297.0), Ok(9));
        assert_eq!(standard_paper(148.0, 210.0), Ok(11));
        assert!(standard_paper(25.0, 100.0).is_err());
    }
}
