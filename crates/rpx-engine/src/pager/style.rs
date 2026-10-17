//! 输出所需的轻量样式；不把模板的值对象带入导出器。
use rpx_core::{cell::Cell, props::def};
#[derive(Debug, Clone, PartialEq)]
pub struct PrintStyle {
    pub font: String,
    pub size: i16,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub foreground: u32,
    pub background: u32,
    pub horizontal: i8,
    pub vertical: i8,
    pub format: Option<String>,
    pub stretch_when_paged: bool,
    pub split: bool,
    /// 上、右、下、左：线型、线宽、RGB。
    pub borders: [(u8, f32, u32); 4],
}
impl From<&Cell> for PrintStyle {
    fn from(c: &Cell) -> Self {
        Self {
            font: c
                .font_name
                .clone()
                .unwrap_or_else(|| def::DEFAULT_FONT_NAME.into()),
            size: c.font_size,
            bold: c.bold(),
            italic: c.italic(),
            underline: c.underline(),
            foreground: c.fore_color as u32 & 0xffffff,
            background: c.back_color as u32 & 0xffffff,
            horizontal: c.h_align,
            vertical: c.v_align,
            format: c.format.clone(),
            stretch_when_paged: c.bit_flags & rpx_core::props::bit::STRETCH_WHEN_PAGED != 0,
            split: c.bit_flags & rpx_core::props::bit::SPLITTED != 0,
            borders: [
                (c.tb_style, c.tb_width, c.tb_color as u32 & 0xffffff),
                (c.rb_style, c.rb_width, c.rb_color as u32 & 0xffffff),
                (c.bb_style, c.bb_width, c.bb_color as u32 & 0xffffff),
                (c.lb_style, c.lb_width, c.lb_color as u32 & 0xffffff),
            ],
        }
    }
}
