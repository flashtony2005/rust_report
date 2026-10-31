//! `format` 命令族（legacy `.rpx` ↔ JSON 双向转换，`legacy-rpx` feature 门控）。
//!
//! 从 `commands.rs` 拆出（G4 行数门禁）：两个函数原样搬来，
//! `read` 改用 `super::` 引用（在 `commands.rs` 里相应放开可见性）。
//! 本模块整体随 `legacy-rpx` feature 编译（`FormatCommand` 同门控），
//! 故内部不再逐项写 `cfg`。

use crate::args::FormatCommand;
use serde_json::{json, Value as J};

use super::read;
use rpx_runtime::{
    definition,
    error::{Error, Result},
    files, format as rpx_format,
};

pub(super) fn format_mode(value: &str) -> Result<rpx_format::Mode> {
    Ok(match value {
        "strict" => rpx_format::Mode::Strict,
        "preserve" => rpx_format::Mode::PreserveLegacy,
        "drop" => rpx_format::Mode::DropLegacy,
        _ => {
            return Err(Error::new(
                2,
                "USAGE",
                "arguments",
                "mode 可选 strict / preserve / drop",
            ))
        }
    })
}

pub(super) fn format_command(command: FormatCommand) -> Result<J> {
    match command {
        FormatCommand::ToJson { file, out, mode } => {
            let bytes = std::fs::read(&file)?;
            let converted = rpx_format::rpx_to_json(
                &bytes,
                rpx_format::Options {
                    mode: format_mode(&mode)?,
                    allow_structural: false,
                },
            )?;
            let out = files::absolute(&out)?;
            files::create(&out, &files::json_bytes(&converted.document)?)?;
            Ok(json!({
                "out": out,
                "legacy": converted.legacy,
                "warnings": converted.warnings,
            }))
        }
        FormatCommand::ToRpx {
            file,
            out,
            mode,
            allow_structural,
        } => {
            let document = read(&file)?;
            let bytes = rpx_format::json_to_rpx(
                &document,
                rpx_format::Options {
                    mode: format_mode(&mode)?,
                    allow_structural,
                },
            )?;
            let out = files::absolute(&out)?;
            files::create(&out, &bytes)?;
            Ok(json!({"out": out}))
        }
        FormatCommand::Verify { file, mode } => {
            let bytes = std::fs::read(&file)?;
            let report = rpx_format::verify(
                &bytes,
                rpx_format::Options {
                    mode: format_mode(&mode)?,
                    allow_structural: false,
                },
            )?;
            Ok(json!({
                "model_equal": report.model_equal,
                "bytes_equal": report.bytes_equal,
                "legacy": report.legacy,
                "source_sha256": report.source_sha256,
                "json_sha256": report.json_sha256,
            }))
        }
        FormatCommand::Check { file } => {
            let document = read(&file)?;
            definition::parse(&document)?;
            Ok(json!({"valid": true}))
        }
        FormatCommand::Normalize { file, out } => {
            let document = read(&file)?;
            let normalized = rpx_format::normalize(&document)?;
            let out = files::absolute(&out)?;
            files::create(&out, &files::json_bytes(&normalized)?)?;
            Ok(json!({"out": out}))
        }
    }
}
