//! `rpx-export` —— 导出面 crate（M6-01 起）。
//!
//! 契约正文：`docs/spec-m6.md` §2（设计决策）/ §2.3（CSV 格式契约，**自定**，
//! 非产品格式——产品导出真值未探针，见 §5.1 带账）。
//!
//! 三条边界（spec-m6 §2.1 / §4）：
//!
//! 1. **输入是已经算完的网格** —— 吃 [`rpx_engine::arena::CalcGrid`]，不读
//!    `.rpx`、不跑 SQL、不重跑引擎（导出器与「重算」解耦）；
//! 2. **不碰零依赖层** —— `rpx-core`/`rpx-engine` 的 `[dependencies]` 仍只许
//!    path（G5 不变）；本 crate 自身的第三方依赖（M6-02 的 `rust_xlsxwriter`）
//!    必须先在 `scripts/dep_allowlist.txt` 登记并在 `code-review.md` §3.2 写明理由；
//! 3. 数据出口 `to_xlsx` 保留值与合并；打印出口 `to_print_xlsx` 按引擎页映射
//!    输出行列几何、字体、边框和支持的原生图表；浏览器预览提供 SVG/PDF。
//!
//! 出口顺序（spec-m6 §2.2）：CSV（本文件+M6-01）→ xlsx（M6-02）→
//! server `/export`（M6-03）→ CLI `render --out`（M6-04）→ 夹具基线（M6-05）。

pub mod csv;
pub mod print_xlsx;
mod text;
pub mod xlsx;

pub use csv::to_csv;
pub use print_xlsx::to_print_xlsx;
pub use xlsx::to_xlsx;

/// 以实际导出使用的图片解码器验证 data URI，避免接受只有前缀的坏图片。
pub fn validate_image(uri: &str) -> Result<(), String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let (mime, encoded) = uri.split_once(',').ok_or("图片缺少 data URI 分隔符")?;
    if ![
        "data:image/png;base64",
        "data:image/jpeg;base64",
        "data:image/gif;base64",
        "data:image/bmp;base64",
    ]
    .contains(&mime)
    {
        return Err("图片应为 PNG/JPEG/GIF/BMP base64 data URI".into());
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|e| format!("图片 base64 不合法：{e}"))?;
    rust_xlsxwriter::Image::new_from_buffer(&bytes).map_err(|e| format!("图片内容无效：{e}"))?;
    Ok(())
}
