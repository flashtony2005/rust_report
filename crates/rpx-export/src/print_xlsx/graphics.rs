use base64::{engine::general_purpose::STANDARD, Engine};
use rpx_engine::{
    arena::{CalcCell, CalcGrid},
    graphics::{ChartKind, Graphic, GraphicContent},
    value::Value,
};
use rust_xlsxwriter::{
    Chart, ChartFormat, ChartLayout, ChartLegendPosition, ChartLine, ChartSolidFill, ChartType,
    Image, Worksheet,
};

pub(super) fn write(
    ws: &mut Worksheet,
    graphics: &[Graphic],
    data: &mut CalcGrid,
) -> Result<(), String> {
    for g in graphics {
        match &g.content {
            GraphicContent::Image(uri) => {
                let (_, encoded) = uri.split_once(',').ok_or("图片缺少 data URI 分隔符")?;
                let bytes = STANDARD
                    .decode(encoded)
                    .map_err(|e| format!("图片 base64 不合法：{e}"))?;
                let image = Image::new_from_buffer(&bytes)
                    .map_err(|e| e.to_string())?
                    .set_scale_to_size(g.width, g.height, true);
                ws.insert_image((g.row - 1) as u32, (g.col - 1) as u16, &image)
                    .map_err(|e| e.to_string())?;
            }
            GraphicContent::Chart(model) => {
                let kind = match model.kind {
                    ChartKind::Column | ChartKind::ColumnLine => ChartType::Column,
                    ChartKind::StackedColumn => ChartType::ColumnStacked,
                    ChartKind::Bar => ChartType::Bar,
                    ChartKind::StackedBar => ChartType::BarStacked,
                    ChartKind::Line | ChartKind::Curve | ChartKind::DualLine => ChartType::Line,
                    ChartKind::Area => ChartType::Area,
                    ChartKind::Pie => ChartType::Pie,
                    ChartKind::Scatter => ChartType::Scatter,
                    ChartKind::Radar => ChartType::Radar,
                    _ => {
                        return Err(format!(
                            "打印 XLSX 无原生 {} 图表；请在图形预览保存 PDF",
                            model.kind.name()
                        ))
                    }
                };
                if model.categories.is_empty() {
                    continue;
                }
                let first = data.rows;
                for (i, v) in model.categories.iter().enumerate() {
                    data.cells
                        .push(CalcCell::bare(first + i as i32 + 1, 1, v.clone()));
                }
                let mut chart = Chart::new(kind);
                let mut secondary = Chart::new(ChartType::Line);
                let combined = matches!(model.kind, ChartKind::ColumnLine | ChartKind::DualLine);
                for (i, series) in model.series.iter().enumerate() {
                    for (j, v) in series.values.iter().enumerate() {
                        data.cells.push(CalcCell::bare(
                            first + j as i32 + 1,
                            i as i32 + 2,
                            v.clone(),
                        ));
                    }
                    let target = if combined && i > 0 {
                        &mut secondary
                    } else {
                        &mut chart
                    };
                    let native = target.add_series();
                    native
                        .set_name(series.name.as_str())
                        .set_categories((
                            "_chart_data",
                            first as u32,
                            0,
                            first as u32 + model.categories.len() as u32 - 1,
                            0,
                        ))
                        .set_values((
                            "_chart_data",
                            first as u32,
                            i as u16 + 1,
                            first as u32 + model.categories.len() as u32 - 1,
                            i as u16 + 1,
                        ))
                        .set_secondary_axis(combined && i > 0)
                        .set_smooth(model.kind == ChartKind::Curve);
                    if !model.appearance.colors.is_empty() {
                        let color =
                            model.appearance.colors[i % model.appearance.colors.len()].as_str();
                        native.set_format(
                            ChartFormat::new()
                                .set_solid_fill(ChartSolidFill::new().set_color(color))
                                .set_line(ChartLine::new().set_color(color)),
                        );
                        if model.kind == ChartKind::Pie {
                            native.set_point_colors(
                                &model
                                    .appearance
                                    .colors
                                    .iter()
                                    .map(String::as_str)
                                    .collect::<Vec<_>>(),
                            );
                        }
                    }
                }
                if combined {
                    chart.combine(&secondary);
                }
                chart.title().set_name(model.title.as_str());
                chart.set_width(g.width).set_height(g.height);
                match model.appearance.legend.as_str() {
                    "none" => {
                        chart.legend().set_hidden();
                    }
                    name => {
                        chart.legend().set_position(match name {
                            "top" => ChartLegendPosition::Top,
                            "left" => ChartLegendPosition::Left,
                            "right" => ChartLegendPosition::Right,
                            _ => ChartLegendPosition::Bottom,
                        });
                    }
                }
                if model.appearance.background != "transparent" {
                    chart
                        .chart_area()
                        .set_format(ChartFormat::new().set_solid_fill(
                            ChartSolidFill::new().set_color(model.appearance.background.as_str()),
                        ));
                }
                if model.kind == ChartKind::Pie {
                    chart.set_rotation(
                        model.appearance.pie_rotation.rem_euclid(360.0).round() as u16
                    );
                }
                let [top, right, bottom, left] = model.appearance.margins.map(f64::from);
                let (w, h) = (f64::from(g.width), f64::from(g.height));
                if left + right < w && top + bottom < h {
                    chart.plot_area().set_layout(
                        &ChartLayout::new()
                            .set_offset(left / w, top / h)
                            .set_dimensions((w - left - right) / w, (h - top - bottom) / h),
                    );
                }
                chart.show_hidden_data();
                ws.insert_chart((g.row - 1) as u32, (g.col - 1) as u16, &chart)
                    .map_err(|e| e.to_string())?;
                data.rows += model.categories.len() as i32 + 1;
                data.cols = data.cols.max(model.series.len() as i32 + 1);
            }
            GraphicContent::CellBackground(_)
            | GraphicContent::Echarts(_)
            | GraphicContent::EchartsScript(_) => {
                return Err(
                    "打印 XLSX 不支持单元格背景图片或自定义 ECharts 模板；请使用图形预览打印"
                        .into(),
                )
            }
        }
    }
    Ok(())
}

/// 空数据页不得写入；否则仅图形工作簿会多出一个可见空白页。
pub(super) fn has_data(data: &CalcGrid) -> bool {
    data.cells.iter().any(|c| !matches!(c.value, Value::Null))
}
