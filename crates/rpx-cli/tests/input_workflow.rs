use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
};
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn invoke(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rpx"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
fn ok(output: Output) -> Value {
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(output.status.success(), "{value}");
    value["data"].clone()
}
#[test]
fn stdin_parameters_patches_and_explicit_rpx_import_work_as_public_commands() {
    let temp =
        Temp(std::env::temp_dir().join(format!("rpx-input-{}", rpx_runtime::files::unique())));
    fs::create_dir(&temp.0).unwrap();
    let file = temp.0.join("template.json");
    fs::write(
        &file,
        json!({"grid":{"rows":1,"cols":1},"cells":[{"row":1,"col":1,"expr":"@p*10"}]}).to_string(),
    )
    .unwrap();
    let template = file.to_str().unwrap();
    let result_dir = temp.0.join("run");
    let result = ok(invoke(
        &[
            "run",
            template,
            "--params-file",
            "-",
            "--out-dir",
            result_dir.to_str().unwrap(),
        ],
        "{\"p\":7}",
    ));
    let saved: Value =
        serde_json::from_slice(&fs::read(result["result"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(saved["cells"][0]["value"]["value"], 70);
    let error = invoke(
        &["template", "validate", template, "--params-file", "-"],
        "{",
    );
    assert_eq!(error.status.code(), Some(4));
    let error: Value = serde_json::from_slice(&error.stdout).unwrap();
    assert_eq!(error["error"]["code"], "PARAMETERS_INVALID");
    assert_eq!(
        invoke(&["template", "validate", "-", "--params-file", "-"], "")
            .status
            .code(),
        Some(2)
    );
    let changed = temp.0.join("changed.json");
    let patch = json!([{"op":"replace","path":"/cells/0/expr","value":"to(1,3)"}]).to_string();
    ok(invoke(
        &[
            "template",
            "patch",
            template,
            "--patch-file",
            "-",
            "--out",
            changed.to_str().unwrap(),
        ],
        &patch,
    ));
    let rpx = temp.0.join("template.rpx");
    ok(invoke(
        &[
            "template",
            "compile",
            changed.to_str().unwrap(),
            "--out",
            rpx.to_str().unwrap(),
        ],
        "",
    ));
    let library = temp.0.join("reports");
    let root = library.to_str().unwrap();
    ok(invoke(&["library", "init", root], ""));
    let args = [
        "report",
        "import-rpx",
        rpx.to_str().unwrap(),
        "--id",
        "imported",
        "--library",
        root,
        "--dry-run",
    ];
    assert_eq!(ok(invoke(&args, ""))["dry_run"], true);
    assert!(!library.join("imported").exists());
    let imported = ok(invoke(&args[..args.len() - 1], ""));
    let revision = imported["revision"].as_str().unwrap();
    let run = ok(invoke(
        &[
            "report",
            "run",
            "imported",
            "--library",
            root,
            "--if-match",
            revision,
        ],
        "",
    ));
    assert_eq!(run["run"]["shape"]["rows"], 3);
}
