//! 本项目的打印契约，通过 RPX 自定义属性保存，不猜测产品私有位编码。
use crate::error::{EngineError, Result};
use rpx_core::ReportDefine;

pub const PREFIX: &str = "rust_report.print.option.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    First,
    Every,
    EverySlice,
    FirstSlice,
}
impl Repeat {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "first" => Self::First,
            "every" => Self::Every,
            "every_slice" => Self::EverySlice,
            "first_slice" => Self::FirstSlice,
            _ => return None,
        })
    }
    pub fn included(self, index: usize) -> bool {
        matches!(self, Self::Every | Self::EverySlice) || index == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EndSpace {
    #[default]
    None,
    Last,
    Average,
    Proportional,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrintOptions {
    pub auto_size: bool,
    pub horizontal_split: bool,
    pub vertical_split: bool,
    pub wrap_numbers: bool,
    pub zero_as_blank: bool,
    pub output_empty: bool,
    pub empty_value: String,
    pub border_width: f32,
    pub border_color: u32,
    pub column_gap: f32,
    pub end_space: EndSpace,
    pub header_y: Repeat,
    pub header_x: Repeat,
    pub title_y: Repeat,
    pub title_x: Repeat,
    pub row_header_every: bool,
    pub row_footer_every: bool,
    pub column_header_every: bool,
    pub column_footer_every: bool,
}
impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            auto_size: true,
            horizontal_split: true,
            vertical_split: true,
            wrap_numbers: false,
            zero_as_blank: false,
            output_empty: false,
            empty_value: String::new(),
            border_width: 0.0,
            border_color: 0,
            column_gap: 5.0,
            end_space: EndSpace::None,
            header_y: Repeat::Every,
            header_x: Repeat::EverySlice,
            title_y: Repeat::Every,
            title_x: Repeat::EverySlice,
            row_header_every: true,
            row_footer_every: false,
            column_header_every: true,
            column_footer_every: true,
        }
    }
}

impl PrintOptions {
    pub fn from_report(rd: &ReportDefine) -> Result<Self> {
        let mut out = Self::default();
        for (key, value) in &rd.custom_properties {
            let Some(key) = key.strip_prefix(PREFIX) else {
                continue;
            };
            let value = value.as_deref().unwrap_or_default();
            let bad = || EngineError::UnsupportedPaging {
                detail: format!("无效打印选项 {key}={value}"),
            };
            match key {
                "auto_size" => out.auto_size = value.parse().map_err(|_| bad())?,
                "horizontal_split" => out.horizontal_split = value.parse().map_err(|_| bad())?,
                "vertical_split" => out.vertical_split = value.parse().map_err(|_| bad())?,
                "wrap_numbers" => out.wrap_numbers = value.parse().map_err(|_| bad())?,
                "zero_as_blank" => out.zero_as_blank = value.parse().map_err(|_| bad())?,
                "output_empty" => out.output_empty = value.parse().map_err(|_| bad())?,
                "empty_value" => out.empty_value = value.into(),
                "border_width" => out.border_width = value.parse().map_err(|_| bad())?,
                "border_color" => {
                    out.border_color =
                        u32::from_str_radix(value.trim_start_matches('#'), 16).map_err(|_| bad())?
                }
                "column_gap" => out.column_gap = value.parse().map_err(|_| bad())?,
                "end_space" => {
                    out.end_space = match value {
                        "none" => EndSpace::None,
                        "last" => EndSpace::Last,
                        "average" => EndSpace::Average,
                        "proportional" => EndSpace::Proportional,
                        _ => return Err(bad()),
                    }
                }
                "header_y" => out.header_y = Repeat::parse(value).ok_or_else(bad)?,
                "header_x" => out.header_x = Repeat::parse(value).ok_or_else(bad)?,
                "title_y" => out.title_y = Repeat::parse(value).ok_or_else(bad)?,
                "title_x" => out.title_x = Repeat::parse(value).ok_or_else(bad)?,
                "row_header_every" => out.row_header_every = value.parse().map_err(|_| bad())?,
                "row_footer_every" => out.row_footer_every = value.parse().map_err(|_| bad())?,
                "column_header_every" => {
                    out.column_header_every = value.parse().map_err(|_| bad())?
                }
                "column_footer_every" => {
                    out.column_footer_every = value.parse().map_err(|_| bad())?
                }
                _ => return Err(bad()),
            }
        }
        Ok(out)
    }
}
