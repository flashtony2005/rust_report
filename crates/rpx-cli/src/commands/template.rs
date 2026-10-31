//! `spec` 与 `template` 两个命令族（从 `commands.rs` 拆出，G4 行数门禁）。
//!
//! 拆分只动**代码位置**，不动行为：两个函数原样搬来，`read`/`params`
//! 改用 `super::` 引用（在 `commands.rs` 里相应放开可见性）。

use crate::args::{SpecCommand, Template};
use serde_json::{json, Value as J};
use std::path::Path;

use super::{params, read};
use rpx_runtime::{
    catalog, definition,
    error::{Error, Result},
    files,
};

/// 规格 → 模板。
///
/// 默认同时跑一遍 `lint`：编译器只保证「按规则展开」，静态可判的坑
/// （列宽超纸张、扩展块里漏了 `left_head`）仍要靠 lint 兜住。
/// 调用方拿到的是一次性的「编译 + 自检」结果，不用再记两条命令。
pub(super) fn spec(command: SpecCommand) -> Result<J> {
    match command {
        SpecCommand::Compile {
            file,
            out,
            dry_run,
            data_dir,
        } => {
            let spec = read(&file)?;
            let template = rpx_runtime::compile::compile(&spec)?;
            let diag = rpx_runtime::lint::lint(&template, data_dir.as_deref());
            let mut value = json!({
                "geometry": rpx_runtime::compile::geometry(&template),
                "lint": diag,
            });
            match out {
                Some(path) if !dry_run => {
                    files::atomic_json(&path, &template)?;
                    value["out"] = json!(files::absolute(&path)?);
                }
                _ => value["template"] = template,
            }
            Ok(value)
        }
        SpecCommand::Analyze {
            file,
            data_dir,
            out,
        } => {
            let spec = read(&file)?;
            let rel = spec
                .pointer("/dataset/file")
                .and_then(J::as_str)
                .ok_or_else(|| Error::new(4, "SPEC_INVALID", "analyze", "规格缺少 dataset.file"))?
                .to_string();
            let path = data_dir.join(&rel);
            let data = std::fs::read(&path).map_err(|e| {
                Error::new(
                    2,
                    "FILE_NOT_FOUND",
                    "analyze",
                    format!("读不到数据 {rel}：{e}"),
                )
            })?;
            let analysis = rpx_runtime::analyze::analyze(&spec, &data)?;
            let mut value = json!({ "analysis": analysis });
            if let Some(path) = out {
                files::atomic_json(&path, &analysis)?;
                value["out"] = json!(files::absolute(&path)?);
            }
            Ok(value)
        }
    }
}

pub(super) fn template(command: Template) -> Result<J> {
    match command {
        Template::Validate {
            file,
            params_file,
            probe_data,
            data_dir,
        } => {
            let template = read(&file)?;
            let params = params_file.as_deref().map(params).transpose()?;
            let probe = if probe_data {
                Some(match &data_dir {
                    Some(dir) => files::absolute(dir)?,
                    None => files::absolute(&file)?
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| Path::new(".").to_path_buf()),
                })
            } else {
                None
            };
            definition::validate_with(&template, params.as_ref(), probe.as_deref())
        }
        Template::Create { example, out_dir } => catalog::write_package(&example, &out_dir, false),
        Template::Patch {
            file,
            patch_file,
            out,
            dry_run,
        } => {
            if file == Path::new("-") && patch_file == Path::new("-") {
                return Err(Error::new(2, "USAGE", "arguments", "只能有一个 stdin 输入"));
            }
            let changed = definition::patch(&read(&file)?, &read(&patch_file)?)?;
            if !dry_run {
                files::create(&out, &files::json_bytes(&changed)?)?;
            }
            Ok(json!({"dry_run":dry_run,"out":files::absolute(&out)?,"template":changed}))
        }
        #[cfg(feature = "legacy-rpx")]
        Template::Compile { file, out } => {
            let value = read(&file)?;
            definition::validate(&value, None)?;
            let (rd, _) = definition::parse(&value)?;
            let bytes =
                rpx_core::to_bytes(&rd).map_err(|e| Error::new(6, "RPX_COMPILE", "export", e))?;
            files::create(&out, &bytes)?;
            Ok(json!({"out":files::absolute(&out)?}))
        }
        Template::Inspect { file, cell, page } => {
            let value = read(&file)?;
            definition::parse(&value)?;
            let point = cell.as_deref().map(definition::coord).transpose()?;
            let mut cells:Vec<_>=value["cells"].as_array().into_iter().flatten().enumerate().filter(|(_,c)|point.is_none_or(|(r,col)|c["row"]==r&&c["col"]==col)).map(|(i,c)|json!({"json_pointer":format!("/cells/{i}"),"address":definition::address(c["row"].as_u64().unwrap(),c["col"].as_u64().unwrap()),"definition":c,"left_head":c.get("left_head").map_or(json!({"state":"inferred_at_run"}),|h|json!({"state":"declared","value":h})),"top_head":c.get("top_head").map_or(json!({"state":"inferred_at_run"}),|h|json!({"state":"declared","value":h}))})).collect();
            cells.sort_by_key(|c| {
                (
                    c["definition"]["row"].as_u64(),
                    c["definition"]["col"].as_u64(),
                )
            });
            let mut data = files::page(&cells, page.limit as usize, page.offset)?;
            data["grid"] = value["grid"].clone();
            data["datasets"] = value["datasets"].clone();
            data["params"] = value["params"].clone();
            Ok(data)
        }
    }
}
