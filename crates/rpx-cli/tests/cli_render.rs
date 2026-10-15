//! `rpx-cli render` 端到端（M6-04）：**真跑 CLI 二进制** + 与库函数直调逐字节一致。
//!
//! 审计遗留③的闭合形态：「凡验收标准里写了 `CLI X`，就必须有一条真的跑 CLI
//! 的自动化断言」（`docs/completeness-audit.md` §3 问题 3）。本文件的每条断言
//! 都经由 `CARGO_BIN_EXE_rpx-cli` 起的**真实进程**，不直调任何 CLI 内部代码
//! （CLI 无 lib target，物理上也不可能）。
//!
//! 「库函数直调」= 测试侧自行组合 `rpx_core::open_bytes + rpx_engine::dataset/
//! calculate/pager + rpx_export::to_csv|to_xlsx`（与 CLI 同构的下游链）——
//! CLI 面不得与库面有差（同 `render_http.rs` 的 socket×库对拍体例）。

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// 跑真 CLI（`CARGO_BIN_EXE_` 由 cargo 注入）→ (exit code, stdout+stderr)。
fn run_cli(args: &[&str]) -> (Option<i32>, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rpx-cli"))
        .args(args)
        .output()
        .expect("rpx-cli 二进制应可执行");
    let mut log = String::from_utf8_lossy(&out.stdout).into_owned();
    log.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.code(), log)
}

/// 库函数直调（对拍基准）：与 CLI 同构的链路，但不经 CLI 的任何代码。
fn library_bytes(name: &str, csv: bool) -> Vec<u8> {
    let dir = fixtures_dir();
    let rd =
        rpx_core::open_bytes(&std::fs::read(dir.join(format!("{name}.rpx"))).unwrap()).unwrap();
    let ds = rpx_engine::dataset::load_datasets_full(
        &rd,
        &|file_name| std::fs::read(dir.join(file_name)).ok(),
        None,
    )
    .unwrap();
    let mut grid =
        rpx_engine::calculate_with(&rd, &ds, &rpx_engine::params::ParamTable::EMPTY).unwrap();
    let pg = rpx_engine::pager::paginate_with_setup(&grid, &rd.print_setup).unwrap();
    grid.page_count = pg.page_count();
    grid.pagination = Some(pg);
    if csv {
        rpx_export::to_csv(&grid).into_bytes()
    } else {
        rpx_export::to_xlsx(&grid)
    }
}

/// 每次测试独占的产物路径（测试二进制内 pid 相同 ⇒ 用测试名区分）。
fn out_path(tag: &str, ext: &str) -> PathBuf {
    std::env::temp_dir().join(format!("rpx_cli_{tag}.{ext}"))
}

#[test]
fn csv_export_matches_library_bytes() {
    let out = out_path("csv", "csv");
    let (code, log) = run_cli(&[
        "render",
        fixtures_dir().join("engine_sum.rpx").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "CLI 应成功：{log}");
    assert!(log.contains("exported:"), "应有导出回执：{log}");
    let got = std::fs::read(&out).unwrap();
    assert_eq!(
        got,
        library_bytes("engine_sum", true),
        "CLI 产物应逐字节等于库直调"
    );
    // 语义抽查：表头行在（对拍之外的独立字面量）。
    let text = String::from_utf8(got).unwrap();
    assert!(text.starts_with("region,qty,"), "{text:.60}");
    let _ = std::fs::remove_file(&out);
}

#[test]
fn xlsx_export_matches_library_bytes() {
    let out = out_path("xlsx", "xlsx");
    let (code, log) = run_cli(&[
        "render",
        fixtures_dir().join("engine_sum.rpx").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "CLI 应成功：{log}");
    let got = std::fs::read(&out).unwrap();
    assert_eq!(&got[..4], b"PK\x03\x04", "xlsx 应是 zip 容器");
    assert_eq!(
        got,
        library_bytes("engine_sum", false),
        "CLI 产物应逐字节等于库直调"
    );
    let _ = std::fs::remove_file(&out);
}

#[test]
fn default_data_dir_is_rpx_dir_and_flag_relocates() {
    // 三段：① 只有 .rpx（无 CSV）⇒ 默认目录（= .rpx 同目录）找不到 ⇒ 显式失败；
    // ② 把 CSV 拷到同目录 ⇒ 默认找到（**正面**钉「缺省 = .rpx 同目录」）；
    // ③ 删掉同目录 CSV、给 `--data-dir` ⇒ 仍成功（钉 flag 生效）。
    let tmp = std::env::temp_dir().join("rpx_cli_datadir");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let rpx = tmp.join("engine_sum.rpx");
    std::fs::copy(fixtures_dir().join("engine_sum.rpx"), &rpx).unwrap();
    let out = out_path("datadir", "csv");

    let (code, log) = run_cli(&[
        "render",
        rpx.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(1), "默认目录缺 CSV 应显式失败：{log}");
    assert!(
        log.contains("数据集加载失败"),
        "错误应点名数据集加载：{log}"
    );

    for f in ["engine_sum.csv", "engine_sum.csv2"] {
        std::fs::copy(fixtures_dir().join(f), tmp.join(f)).unwrap();
    }
    let (code, log) = run_cli(&[
        "render",
        rpx.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "同目录数据集应被默认找到：{log}");

    for f in ["engine_sum.csv", "engine_sum.csv2"] {
        std::fs::remove_file(tmp.join(f)).unwrap();
    }
    let (code, log) = run_cli(&[
        "render",
        rpx.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--data-dir",
        fixtures_dir().to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "--data-dir 指向夹具目录应成功：{log}");
    assert!(Path::new(&out).exists(), "产物文件应存在");

    let _ = std::fs::remove_file(&out);
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn unknown_extension_exits_2() {
    let out = out_path("badext", "txt");
    let (code, log) = run_cli(&[
        "render",
        fixtures_dir().join("engine_sum.rpx").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(2), "未知扩展名是用法错误（exit 2）：{log}");
    assert!(log.contains(".csv 或 .xlsx"), "应提示可选格式：{log}");
    assert!(!Path::new(&out).exists(), "不得产出文件");
}

#[test]
fn missing_out_is_usage_error() {
    let (code, log) = run_cli(&[
        "render",
        fixtures_dir().join("engine_sum.rpx").to_str().unwrap(),
    ]);
    assert_eq!(code, Some(2), "缺 --out 是用法错误（exit 2）：{log}");
    assert!(log.contains("usage:"), "{log}");
}

#[test]
fn uppercase_extension_works() {
    // 扩展名不区分大小写（文件系统惯例）——`.XLSX` 也是 xlsx。
    let out = out_path("upper", "XLSX");
    let (code, log) = run_cli(&[
        "render",
        fixtures_dir().join("engine_sum.rpx").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "大写扩展名应被接受：{log}");
    assert_eq!(&std::fs::read(&out).unwrap()[..4], b"PK\x03\x04");
    let _ = std::fs::remove_file(&out);
}

#[test]
fn print_still_works() {
    // 回归：print 子命令（AC-07）未被 render 的引入破坏。
    let (code, log) = run_cli(&[
        "print",
        fixtures_dir().join("engine_sum.rpx").to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "print 应成功：{log}");
    assert!(log.contains("matrix:"), "print 应输出矩阵摘要：{log:.100}");
}
