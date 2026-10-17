//! PrintSetup：报表首个 MetaData，33 槽（契约附件 D）。
//!
//! 关键事实：ReportDefine._$25 构造时即 new PrintSetup()，载荷里**永远存在且长度恒 >0**，
//! 不存在 null 分支（附件 D）。槽 31-33 是 available() 守卫尾槽（成对结构：31 单独守卫，
//! 32+33 共同守卫），老文件可能缺，按默认值缺省。
//! 证据：PrintSetup.java:1027-1062(写) / :1066-1105(读)，默认值 :134-166。

#[cfg(feature = "legacy-rpx")]
use crate::error::{Result, RpxError};
#[cfg(feature = "legacy-rpx")]
use crate::reader::RecordReader;

#[derive(Debug, Clone, PartialEq)]
pub struct PrintSetup {
    pub paper: i16,
    pub table_column_num: i16,
    pub layout_rows: i16,
    pub layout_cols: i16,
    pub row_num_per_page: i32,
    pub zoom_page_width: i16,
    pub zoom_page_height: i16,
    pub zoom_scale: i16,
    pub paper_width: f32,
    pub paper_height: f32,
    pub left_margin: f32,
    pub right_margin: f32,
    pub top_margin: f32,
    pub bottom_margin: f32,
    pub orient: i8,
    pub layout: i8,
    pub page_header_and_footer_y_mode: i8,
    pub page_header_and_footer_x_mode: i8,
    pub row_table_header_and_footer_mode: i8,
    pub title_y_mode: i8,
    pub title_x_mode: i8,
    pub col_table_header_and_footer_mode: i8,
    pub pager_style: i8,
    pub zoom_mode: i8,
    pub h_align: i8,
    pub v_align: i8,
    pub back_graph_printed: bool,
    pub virtual_printer: Option<String>,
    pub column_repeated_group_header: bool,
    pub copy_count: i32,
    pub full_screen_preview: bool,
    pub blank_line_filled: bool,
    pub column_mode: i8,
}

impl PrintSetup {
    /// Java 兜底 new PrintSetup() 的对应物（ReportDefine.java:1163-1165）。
    /// 取值 = 附件 D 默认值列。
    pub fn default_values() -> Self {
        Self {
            paper: 9,
            table_column_num: 1,
            layout_rows: 1,
            layout_cols: 1,
            row_num_per_page: 20,
            zoom_page_width: 1,
            zoom_page_height: 1,
            zoom_scale: 100,
            paper_width: 210.0,
            paper_height: 297.0,
            left_margin: 19.0,
            right_margin: 19.0,
            top_margin: 25.0,
            bottom_margin: 25.0,
            orient: 1,
            layout: 0,
            page_header_and_footer_y_mode: 0,
            page_header_and_footer_x_mode: 2,
            row_table_header_and_footer_mode: 0,
            title_y_mode: 0,
            title_x_mode: 2,
            col_table_header_and_footer_mode: 0,
            pager_style: 2,
            zoom_mode: 1,
            h_align: 1,
            v_align: 0,
            back_graph_printed: false,
            virtual_printer: None,
            column_repeated_group_header: false,
            copy_count: 1,
            full_screen_preview: false,
            blank_line_filled: false,
            column_mode: 0,
        }
    }

    #[cfg(feature = "legacy-rpx")]
    pub fn read(slice: &[u8]) -> Result<Self> {
        let mut r = RecordReader::new(slice);
        let p = |slot: u16, what: &'static str| RpxError::prop(slot, what);
        Ok(Self {
            paper: r.read_i16().map_err(|_| p(1, "paper"))?,
            table_column_num: r.read_i16().map_err(|_| p(2, "tableColumnNum"))?,
            layout_rows: r.read_i16().map_err(|_| p(3, "layoutRows"))?,
            layout_cols: r.read_i16().map_err(|_| p(4, "layoutCols"))?,
            row_num_per_page: r.read_i32().map_err(|_| p(5, "rowNumPerPage"))?,
            zoom_page_width: r.read_i16().map_err(|_| p(6, "zoomPageWidth"))?,
            zoom_page_height: r.read_i16().map_err(|_| p(7, "zoomPageHeight"))?,
            zoom_scale: r.read_i16().map_err(|_| p(8, "zoomScale"))?,
            paper_width: r.read_f32().map_err(|_| p(9, "paperWidth"))?,
            paper_height: r.read_f32().map_err(|_| p(10, "paperHeight"))?,
            left_margin: r.read_f32().map_err(|_| p(11, "leftMargin"))?,
            right_margin: r.read_f32().map_err(|_| p(12, "rightMargin"))?,
            top_margin: r.read_f32().map_err(|_| p(13, "topMargin"))?,
            bottom_margin: r.read_f32().map_err(|_| p(14, "bottomMargin"))?,
            orient: r.read_i8().map_err(|_| p(15, "orient"))?,
            layout: r.read_i8().map_err(|_| p(16, "layout"))?,
            page_header_and_footer_y_mode: r
                .read_i8()
                .map_err(|_| p(17, "pageHeaderAndFooterYMode"))?,
            page_header_and_footer_x_mode: r
                .read_i8()
                .map_err(|_| p(18, "pageHeaderAndFooterXMode"))?,
            row_table_header_and_footer_mode: r
                .read_i8()
                .map_err(|_| p(19, "rowTableHeaderAndFooterMode"))?,
            title_y_mode: r.read_i8().map_err(|_| p(20, "titleYMode"))?,
            title_x_mode: r.read_i8().map_err(|_| p(21, "titleXMode"))?,
            col_table_header_and_footer_mode: r
                .read_i8()
                .map_err(|_| p(22, "colTableHeaderAndFooterMode"))?,
            pager_style: r.read_i8().map_err(|_| p(23, "pagerStyle"))?,
            zoom_mode: r.read_i8().map_err(|_| p(24, "zoomMode"))?,
            h_align: r.read_i8().map_err(|_| p(25, "hAlign"))?,
            v_align: r.read_i8().map_err(|_| p(26, "vAlign"))?,
            back_graph_printed: r.read_bool().map_err(|_| p(27, "backGraphPrinted"))?,
            virtual_printer: r.read_string().map_err(|_| p(28, "virtualPrinter"))?,
            column_repeated_group_header: r
                .read_bool()
                .map_err(|_| p(29, "columnRepeatedGroupHeader"))?,
            copy_count: r.read_i32().map_err(|_| p(30, "copyCount"))?,
            full_screen_preview: if r.remaining() > 0 {
                r.read_bool().map_err(|_| p(31, "fullScreenPreview"))?
            } else {
                false
            },
            blank_line_filled: if r.remaining() > 0 {
                r.read_bool().map_err(|_| p(32, "blankLineFilled"))?
            } else {
                false
            },
            column_mode: if r.remaining() > 0 {
                r.read_i8().map_err(|_| p(33, "columnMode"))?
            } else {
                0
            },
        })
    }

    /// 附件 D 默认值（PrintSetup.java:134-166）。
    pub fn is_default(&self) -> bool {
        self.paper == 9
            && self.table_column_num == 1
            && self.layout_rows == 1
            && self.layout_cols == 1
            && self.row_num_per_page == 20
            && self.zoom_page_width == 1
            && self.zoom_page_height == 1
            && self.zoom_scale == 100
            && (self.paper_width - 210.0).abs() < 1e-5
            && (self.paper_height - 297.0).abs() < 1e-5
            && (self.left_margin - 19.0).abs() < 1e-5
            && (self.right_margin - 19.0).abs() < 1e-5
            && (self.top_margin - 25.0).abs() < 1e-5
            && (self.bottom_margin - 25.0).abs() < 1e-5
            && self.orient == 1
            && self.layout == 0
            && self.page_header_and_footer_y_mode == 0
            && self.page_header_and_footer_x_mode == 2
            && self.row_table_header_and_footer_mode == 0
            && self.title_y_mode == 0
            && self.title_x_mode == 2
            && self.col_table_header_and_footer_mode == 0
            && self.pager_style == 2
            && self.zoom_mode == 1
            && self.h_align == 1
            && self.v_align == 0
            && !self.back_graph_printed
            && self.virtual_printer.is_none()
            && !self.column_repeated_group_header
            && self.copy_count == 1
            && !self.full_screen_preview
            && !self.blank_line_filled
            && self.column_mode == 0
    }
}
