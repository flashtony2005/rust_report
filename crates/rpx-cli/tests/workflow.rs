use serde_json::{json, Value as J};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("rpx-cli-test-{}", rpx_runtime::files::unique()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn path(&self, n: &str) -> String {
        self.0.join(n).to_str().unwrap().into()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn process(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rpx"))
        .args(args)
        .current_dir("/tmp")
        .output()
        .unwrap()
}
fn call(args: &[&str]) -> J {
    let p = process(args);
    let v: J = serde_json::from_slice(&p.stdout)
        .unwrap_or_else(|_| panic!("stdout {:?},stderr {:?}", p.stdout, p.stderr));
    assert!(p.status.success(), "{args:?}: {v}");
    v["data"].clone()
}
fn write(path: &str, value: J) {
    fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}
fn simple() -> J {
    json!({"version":1,"grid":{"rows":1,"cols":2},"cells":[{"row":1,"col":1,"expr":"to(1,3)"},{"row":1,"col":2,"expr":"A1*10"}]})
}
#[test]
fn actual_process_file_execution_queries_and_failures() {
    let t = Temp::new();
    let file = t.path("template.json");
    write(&file, simple());
    let usage = process(&["run"]);
    assert_eq!(usage.status.code(), Some(2));
    let e: J = serde_json::from_slice(&usage.stdout).unwrap();
    assert_eq!(e["error"]["code"], "USAGE");
    assert!(usage.stderr.is_empty());
    assert_eq!(call(&["doctor"])["offline"], true);
    assert_eq!(
        call(&["schema", "template"])["schema"]["properties"]["grid"]["required"],
        json!(["rows", "cols"])
    );
    let run = call(&[
        "run",
        &file,
        "--out-dir",
        &t.path("run"),
        "--export",
        "csv",
        "--export",
        "xlsx",
    ]);
    assert_eq!(run["run"]["shape"], json!({"rows":3,"cols":2,"pages":1}));
    let result = run["result"].as_str().unwrap();
    let query = call(&[
        "result", "inspect", result, "--origin", "B1", "--limit", "2",
    ]);
    assert_eq!(query["total"], 3);
    assert_eq!(query["returned"], 2);
    assert_eq!(query["next_offset"], 2);
    assert_eq!(query["items"][1]["value"]["value"], 20);
    assert_eq!(
        call(&["result", "inspect", result, "--origin", "B1", "--range", "B3:B3"])["items"][0]
            ["value"]["value"],
        30
    );
    assert_eq!(
        call(&["result", "inspect", result, "--origin", "Z99"])["total"],
        0
    );
    assert_eq!(
        process(&["run", &file, "--out-dir", &t.path("run")])
            .status
            .code(),
        Some(6)
    );
    let mut bad = simple();
    bad["cells"][1]["typo"] = json!(1);
    write(&file, bad);
    let error = process(&["template", "validate", &file]);
    assert_eq!(error.status.code(), Some(3));
    let error: J = serde_json::from_slice(&error.stdout).unwrap();
    assert_eq!(error["error"]["json_pointer"], "/cells/1/typo");
}
#[test]
fn installed_examples_complex_master_and_library_concurrency() {
    let t = Temp::new();
    let lib = t.path("reports");
    call(&["library", "init", &lib, "--presets"]);
    assert_eq!(call(&["report", "list", "--library", &lib])["total"], 5);
    let detail = call(&["report", "show", "complex-master", "--library", &lib]);
    let rev = detail["revision"].as_str().unwrap();
    let run = call(&[
        "report",
        "run",
        "complex-master",
        "--library",
        &lib,
        "--if-match",
        rev,
        "--export",
        "xlsx",
    ]);
    assert_eq!(run["run"]["shape"]["rows"], 23);
    assert_eq!(run["run"]["shape"]["cols"], 25);
    for (origin, expected) in [("G9", 33975.0), ("H9", 8490.0)] {
        let r = call(&[
            "result",
            "inspect",
            run["result"].as_str().unwrap(),
            "--origin",
            origin,
        ]);
        let values = r["items"].as_array().unwrap();
        assert!(
            values
                .iter()
                .any(|c| c["value"]["value"].as_f64() == Some(expected)),
            "{r}"
        );
    }
    let cell = call(&[
        "template",
        "inspect",
        &t.path("reports/complex-master/template.json"),
        "--cell",
        "C6",
    ]);
    assert_eq!(cell["items"][0]["definition"]["left_head"], "D6");
    let patch = t.path("patch.json");
    write(
        &patch,
        json!([{"op":"replace","path":"/cells/0/text","value":"人和 agent 共同编辑"}]),
    );
    let a = Command::new(env!("CARGO_BIN_EXE_rpx"))
        .args([
            "report",
            "patch",
            "complex-master",
            "--library",
            &lib,
            "--if-match",
            rev,
            "--patch-file",
            &patch,
        ])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let b = process(&[
        "report",
        "patch",
        "complex-master",
        "--library",
        &lib,
        "--if-match",
        rev,
        "--patch-file",
        &patch,
    ]);
    let a = a.wait_with_output().unwrap();
    let mut codes = vec![a.status.code().unwrap(), b.status.code().unwrap()];
    codes.sort();
    assert_eq!(codes, vec![0, 7]);
    let new = call(&["report", "show", "complex-master", "--library", &lib]);
    assert_eq!(new["result_state"], "older_revision");
    assert_eq!(new["display_run_id"], run["run"]["run_id"]);
    assert_eq!(
        process(&[
            "report",
            "run",
            "complex-master",
            "--library",
            &lib,
            "--if-match",
            rev
        ])
        .status
        .code(),
        Some(7)
    );
    call(&[
        "report",
        "copy",
        "complex-master",
        "--as",
        "monthly",
        "--library",
        &lib,
    ]);
    assert_eq!(
        call(&["report", "show", "monthly", "--library", &lib])["result_state"],
        "none"
    );
}
#[test]
fn patch_is_atomic_and_parameter_values_are_not_fabricated() {
    let t = Temp::new();
    let file = t.path("params.json");
    write(
        &file,
        json!({"grid":{"rows":1,"cols":1},"params":[{"name":"p","data_type":"int[]","default":"[9]"}],"cells":[{"row":1,"col":1,"expr":"@p"}]}),
    );
    let v = call(&["template", "validate", &file]);
    assert!(v["deferred"]
        .as_array()
        .unwrap()
        .contains(&json!("parameter_binding")));
    let params = t.path("values.json");
    write(&params, json!({"p":[1,2,3]}));
    let run = call(&[
        "run",
        &file,
        "--out-dir",
        &t.path("good"),
        "--params-file",
        &params,
    ]);
    assert_eq!(run["run"]["shape"]["rows"], 3);
    write(&params, json!({"p":18446744073709551615u64}));
    assert_eq!(
        process(&["template", "validate", &file, "--params-file", &params])
            .status
            .code(),
        Some(4)
    );
    let patch = t.path("bad-patch.json");
    write(
        &patch,
        json!([{"op":"replace","path":"/grid/rows","value":0}]),
    );
    assert_eq!(
        process(&[
            "template",
            "patch",
            &file,
            "--patch-file",
            &patch,
            "--out",
            &t.path("new.json")
        ])
        .status
        .code(),
        Some(3)
    );
    assert!(!Path::new(&t.path("new.json")).exists());
}
