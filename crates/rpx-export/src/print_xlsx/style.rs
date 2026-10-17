use rpx_engine::arena::{CalcCell, CalcGrid};
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, FormatUnderline};

fn border(code: u8, width: f32) -> FormatBorder {
    match code {
        0 => FormatBorder::None,
        81 | 85 => FormatBorder::Dotted,
        82 => FormatBorder::Dashed,
        84 => FormatBorder::Double,
        _ => {
            if width >= 2.0 {
                FormatBorder::Thick
            } else if width >= 1.0 {
                FormatBorder::Medium
            } else {
                FormatBorder::Thin
            }
        }
    }
}
pub(crate) fn format(g: &CalcGrid, cell: Option<&CalcCell>) -> Format {
    let mut f = Format::new();
    let o = &g.print_options;
    if o.auto_size {
        f = f.set_shrink()
    }
    if o.wrap_numbers {
        f = f.set_text_wrap()
    }
    if o.border_width > 0.0 {
        f = f
            .set_border(border(83, o.border_width))
            .set_border_color(o.border_color)
    }
    let Some(s) = cell.and_then(|c| g.print_styles.get(&c.origin)) else {
        return f;
    };
    f = f
        .set_font_name(&s.font)
        .set_font_size(s.size as f64)
        .set_font_color(s.foreground)
        .set_background_color(s.background);
    if s.bold {
        f = f.set_bold()
    }
    if s.italic {
        f = f.set_italic()
    }
    if s.underline {
        f = f.set_underline(FormatUnderline::Single)
    }
    if let Some(n) = &s.format {
        f = f.set_num_format(n)
    }
    f = f.set_align(match s.horizontal {
        -47 => FormatAlign::Center,
        -46 => FormatAlign::Right,
        -45 => FormatAlign::Justify,
        _ => FormatAlign::Left,
    });
    f = f.set_align(match s.vertical {
        -32 => FormatAlign::Top,
        -30 => FormatAlign::Bottom,
        _ => FormatAlign::VerticalCenter,
    });
    let [(t, tw, tc), (r, rw, rc), (b, bw, bc), (l, lw, lc)] = s.borders;
    if t != 0 {
        f = f.set_border_top(border(t, tw)).set_border_top_color(tc)
    }
    if r != 0 {
        f = f.set_border_right(border(r, rw)).set_border_right_color(rc)
    }
    if b != 0 {
        f = f
            .set_border_bottom(border(b, bw))
            .set_border_bottom_color(bc)
    }
    if l != 0 {
        f = f.set_border_left(border(l, lw)).set_border_left_color(lc)
    }
    f
}
