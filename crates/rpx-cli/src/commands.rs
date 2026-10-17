use crate::args::*;
#[cfg(feature = "legacy-rpx")]
use rpx_runtime::format as rpx_format;
use rpx_runtime::{
    catalog, definition,
    error::{Error, Result},
    files, library, result, run,
};
use serde_json::{json, Value as J};
use std::{
    io::{Read, Write},
    path::Path,
};
fn read(path: &Path) -> Result<J> {
    if path == Path::new("-") {
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes)?;
        Ok(serde_json::from_slice(&bytes)?)
    } else {
        files::read_json(path)
    }
}
fn params(path: &Path) -> Result<J> {
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
fn doctor_library(path: Option<&Path>) -> J {
    let Some(path) = path else {
        return json!({"name":"library","ok":true,"detail":"not requested"});
    };
    let checked = (|| {
        let root = files::absolute(path)?;
        let meta = files::read_json(&root.join(".library.json"))?;
        if meta["version"] != 1 {
            return Err(Error::new(
                3,
                "LIBRARY_INVALID",
                "doctor",
                "不支持的报表库版本",
            ));
        }
        let probe = root.join(format!(".doctor-{}", files::unique()));
        files::create(&probe, b"ok")?;
        let _ = std::fs::remove_file(&probe);
        Ok::<_, Error>(root)
    })();
    match checked {
        Ok(root) => json!({"name":"library","ok":true,"detail":root,"writable":true}),
        Err(e) => json!({"name":"library","ok":false,"detail":e.diagnostic}),
    }
}

fn doctor_datasources(path: Option<&Path>) -> J {
    let Some(path) = path else {
        return json!({"name":"datasources","ok":true,"detail":"not requested","connected":false});
    };
    let checked = (|| {
        let value: J = serde_json::from_slice(&std::fs::read(path)?)?;
        let Some(map) = value.as_object() else {
            return Err(Error::new(
                3,
                "DATASOURCES_INVALID",
                "doctor",
                "数据源配置必须是 JSON 对象",
            ));
        };
        for (name, spec) in map {
            let url = match spec {
                J::String(url) => url.as_str(),
                J::Object(m) => m.get("url").and_then(J::as_str).ok_or_else(|| {
                    Error::new(3, "DATASOURCES_INVALID", "doctor", format!("{name} 缺 url"))
                })?,
                _ => {
                    return Err(Error::new(
                        3,
                        "DATASOURCES_INVALID",
                        "doctor",
                        format!("{name} 必须是 URL 或 {{url,user,pass}}"),
                    ))
                }
            };
            if !url.contains("://") {
                return Err(Error::new(
                    3,
                    "DATASOURCES_INVALID",
                    "doctor",
                    format!("{name} 的 URL 缺 scheme"),
                ));
            }
        }
        Ok::<_, Error>(map.len())
    })();
    match checked {
        Ok(count) => {
            json!({"name":"datasources","ok":true,"detail":format!("{count} datasource(s)"),"connected":false})
        }
        Err(e) => json!({"name":"datasources","ok":false,"detail":e.diagnostic,"connected":false}),
    }
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
            "checks":[
                doctor_library(lib.as_deref()),
                doctor_datasources(datasources.as_deref()),
            ],
        })),
        Command::Schema { .. } => Ok(json!({"schema":definition::schema()})),
        Command::Template { command } => template(command),
        #[cfg(feature = "legacy-rpx")]
        Command::Format { command } => format_command(command),
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
            let source = run::capture(&file, &opts)?;
            run::inspect_dataset(source, &opts, &name, limit as usize).await
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
    }
}
fn template(command: Template) -> Result<J> {
    match command {
        Template::Validate { file, params_file } => definition::validate(
            &read(&file)?,
            params_file.as_deref().map(params).transpose()?.as_ref(),
        ),
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

#[cfg(feature = "legacy-rpx")]
fn format_mode(value: &str) -> Result<rpx_format::Mode> {
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

#[cfg(feature = "legacy-rpx")]
fn format_command(command: FormatCommand) -> Result<J> {
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
