//! CLI 命令分发（`execute`）+ 各命令族实现。
//!
//! 命令族按主题拆到子模块（G4 行数门禁，2026-10-22）：
//! * [`template`] —— `spec`（规格 → 模板）与 `template`（模板校验/创建/修补/查看）；
//! * [`format`] —— legacy `.rpx` ↔ JSON 转换（`legacy-rpx` feature 门控）。
//!
//! 本文件保留：共享输入帮手（`read`/`params`/`inputs`）、总分发 `execute`、
//! `report` 命令族与 `print_text` 人读输出。

#[cfg(feature = "legacy-rpx")]
mod format;
mod template;

use crate::args::*;
use rpx_runtime::{
    capabilities, catalog, definition, error::Result, files, library, lint, result, run,
};
use serde_json::{json, Value as J};
use std::{
    io::{Read, Write},
    path::Path,
};
/// 读 JSON（`-` = stdin）。
pub(crate) fn read(path: &Path) -> Result<J> {
    if path == Path::new("-") {
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes)?;
        Ok(serde_json::from_slice(&bytes)?)
    } else {
        files::read_json(path)
    }
}
pub(crate) fn params(path: &Path) -> Result<J> {
    read(path).map_err(|mut e| {
        if e.diagnostic["code"] == "JSON_INVALID" {
            e.exit = 4;
            e.diagnostic["code"] = json!("PARAMETERS_INVALID");
            e.diagnostic["stage"] = json!("parameters");
        }
        e
    })
}
fn inputs(inputs: Inputs) -> Result<run::Options> {
    Ok(run::Options {
        data_dir: inputs.data_dir,
        params: inputs.params_file.as_deref().map(params).transpose()?,
        datasources: inputs.datasources,
        exports: inputs.exports,
    })
}
pub async fn execute(command: Command) -> Result<J> {
    match command {
        Command::Doctor {
            library: lib,
            datasources,
        } => Ok(json!({
            "version":env!("CARGO_PKG_VERSION"),
            "protocol_version":1,
            "offline":true,
            "authentication":"not_required",
            "definition":"JSON v1",
            "json_format":"rpx-report/v1",
            "legacy":"rpx-rpx-v1",
            "legacy_rpx":cfg!(feature = "legacy-rpx"),
            "datasources":["CSV","TSV","Excel","SQLite","PostgreSQL","MySQL"],
            "exports":["csv","xlsx","print-xlsx"],
            "web_executes_engine":false,
            "pdf":false,
            "checks": crate::doctor::checks(lib.as_deref(), datasources.as_deref()),
        })),
        Command::Capabilities => Ok(capabilities::describe(env!("CARGO_PKG_VERSION"))),
        Command::Lint {
            file,
            data_dir,
            strict,
        } => {
            let template = read(&file)?;
            let base = match data_dir {
                Some(dir) => Some(files::absolute(&dir)?),
                None => files::absolute(&file)?.parent().map(Path::to_path_buf),
            };
            let mut out = lint::lint(&template, base.as_deref());
            let warnings = out["summary"]["warning"].as_u64().unwrap_or(0);
            let errors = out["summary"]["error"].as_u64().unwrap_or(0);
            let failed = errors > 0 || (strict && warnings > 0);
            out["gate"] = json!({
                "strict": strict,
                "passed": !failed,
                "exit_code": if failed { 3 } else { 0 },
            });
            Ok(out)
        }
        Command::Schema { .. } => Ok(json!({"schema":definition::schema()})),
        Command::Template { command } => template::template(command),
        #[cfg(feature = "legacy-rpx")]
        Command::Format { command } => format::format_command(command),
        Command::Run {
            file,
            out_dir,
            inputs: args,
        } => {
            let opts = inputs(args)?;
            let source = run::capture(&file, &opts)?;
            let out = files::absolute(&out_dir)?;
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let record = run::start(&source, &out)?;
            run::execute(source, &opts, &out, record).await
        }
        Command::Result {
            command:
                ResultCommand::Inspect {
                    file,
                    range,
                    origin,
                    page,
                },
        } => result::inspect(
            &read(&file)?,
            range.as_deref(),
            origin.as_deref(),
            page.limit as usize,
            page.offset,
        ),
        Command::Dataset {
            command:
                Dataset::Inspect {
                    file,
                    name,
                    data_dir,
                    datasources,
                    limit,
                },
        } => {
            let opts = run::Options {
                data_dir,
                datasources,
                ..Default::default()
            };
            match name {
                // 有名字：按模板内声明口径探查（SQL 数据集也走这里）
                Some(name) => {
                    let source = run::capture(&file, &opts)?;
                    run::inspect_dataset(source, &opts, &name, limit as usize).await
                }
                // 没名字：裸数据文件直查 —— 上传阶段还没有模板，先看列与样例行
                None => run::inspect_data_file(&file, limit as usize).await,
            }
        }
        Command::Catalog { command } => match command {
            Catalog::Examples => Ok(catalog::list()),
            Catalog::Functions { name, page } => {
                catalog::functions(name.as_deref(), page.limit as usize, page.offset)
            }
        },
        Command::Library {
            command: LibraryCommand::Init { dir, presets },
        } => library::init(&dir, presets),
        Command::Report { command } => report(command).await,
        Command::Spec { command } => template::spec(command),
    }
}

async fn report(command: Report) -> Result<J> {
    match command {
        Report::List {
            library: lib,
            query,
            tag,
            page,
        } => library::list(
            &lib.library,
            query.as_deref(),
            tag.as_deref(),
            page.limit as usize,
            page.offset,
        ),
        Report::Show { id, library: lib } => library::show(&lib.library, &id),
        Report::Runs {
            id,
            library: lib,
            page,
        } => files::page(
            &library::runs(&lib.library, &id)?,
            page.limit as usize,
            page.offset,
        ),
        Report::Copy {
            id,
            new_id,
            library: lib,
        } => library::copy(&lib.library, &id, &new_id),
        Report::Patch {
            id,
            patch_file,
            if_match,
            dry_run,
            library: lib,
        } => library::patch(&lib.library, &id, &if_match, &read(&patch_file)?, dry_run),
        Report::Validate { id, library: lib } => library::validate(&lib.library, &id),
        Report::Run {
            id,
            if_match,
            library: lib,
            inputs: args,
        } => library::execute(&lib.library, &id, &if_match, &inputs(args)?).await,
        Report::Update {
            id,
            if_match,
            name,
            description,
            library: lib,
        } => library::update(
            &lib.library,
            &id,
            &if_match,
            name.as_deref(),
            description.as_deref(),
        ),
        Report::ImportJson {
            file,
            id,
            data_dir,
            dry_run,
            library: lib,
        } => library::import_json(&lib.library, &file, &id, data_dir.as_deref(), dry_run),
        Report::Delete {
            id,
            if_match,
            library: lib,
        } => library::delete(&lib.library, &id, &if_match),
        Report::Migrate {
            dry_run,
            backup,
            library: lib,
        } => library::migrate(&lib.library, dry_run, backup),
        Report::Prune {
            id,
            keep,
            library: lib,
        } => library::prune(&lib.library, &id, usize::from(keep)),
        #[cfg(feature = "legacy-rpx")]
        Report::ImportRpx {
            file,
            id,
            data_dir,
            dry_run,
            library: lib,
        } => library::import(&lib.library, &file, &id, data_dir.as_deref(), dry_run),
    }
}

pub fn print_text(value: &J) {
    let command = value["command"].as_str().unwrap_or("rpx");
    if value["ok"] != true {
        eprintln!(
            "{command}: {}",
            value["error"]["message"].as_str().unwrap_or("error")
        );
        return;
    }
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{command}: OK");
    let data = &value["data"];
    if let Some(issues) = data["issues"].as_array() {
        // lint 的人读视图：一条一行 + hint 起头箭头，与 python 闸门同款。
        let summary = &data["summary"];
        let _ = writeln!(
            out,
            "{} 条：error {} / warning {} / info {}",
            summary["total"].as_u64().unwrap_or(0),
            summary["error"].as_u64().unwrap_or(0),
            summary["warning"].as_u64().unwrap_or(0),
            summary["info"].as_u64().unwrap_or(0),
        );
        for issue in issues {
            let _ = writeln!(
                out,
                "{:7} {:5} {}",
                issue["level"].as_str().unwrap_or("?"),
                issue["code"].as_str().unwrap_or(""),
                issue["message"].as_str().unwrap_or(""),
            );
            if let Some(hint) = issue["hint"].as_str() {
                let _ = writeln!(out, "        → {hint}");
            }
        }
        if let Some(passed) = data["gate"]["passed"].as_bool() {
            let _ = writeln!(
                out,
                "gate: {}（strict={}）",
                if passed { "PASS" } else { "FAIL" },
                data["gate"]["strict"]
            );
        }
        return;
    }
    if let Some(items) = data["items"].as_array() {
        let _ = writeln!(out, "返回 {} / {}", items.len(), data["total"]);
        for item in items {
            let _ = writeln!(out, "{}", serde_json::to_string(item).unwrap_or_default());
        }
    } else {
        for (key, v) in data.as_object().into_iter().flatten() {
            let _ = writeln!(out, "{key}: {v}");
        }
    }
}
