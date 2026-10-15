//! 导出回归基线（M6-05）：引擎夹具 → `tests/fixtures/out/export_*.{csv,xlsx}`。
//!
//! # 口径（spec-m6 §3 M6-05）
//!
//! 20 名义夹具（`crates/rpx-engine/tests/common/loader.rs::all_names` 同序）
//! **减 1**：`engine_cycle` 是错误型夹具（`calculate` 必报循环引用、**无计算态
//! 网格** —— `engine_grid_compare.rs` 第 3 组单独断言）⇒ 物理上不可能导出，
//! 本测试的可导集合 = 其余 **19** 个（活证见
//! [`engine_cycle_is_error_fixture_thus_excluded`]）。
//!
//! # 生成 / 比对双模（体例同 Java 侧 `out/gen.ps1` 的 gen/verify 双模）
//!
//! * **生成**：`RPX_UPDATE_BASELINES=1 cargo test -p rpx-export --test export_baseline`
//!   —— 显式**维护动作**（写 `out/`），仅在导出契约**合法变更**后执行；
//! * **比对**（默认）：`cargo test -p rpx-export` —— CSV 逐字节；
//!   xlsx **读回级**（calamine → 逐格 + 合并区文本）。xlsx 的 zip 层噪声
//!   （压缩级别等）不在契约内（契约是「读回自洽」，spec-m6 §2.4）——
//!   但**读回**的任何一格/一个合并区变化都会红（「任一导出改动导致读回 diff
//!   即红」的字面实现）。
//!
//! # 输入口径
//!
//! 数据集 = `tests/fixtures/` 直读（模板 `file_name` 相对名）；参数 =
//! [`params_for`]（仅 `engine_param` 有；值出处 = 其 expected.json 的
//! `params` 段：`p=[1,2,3]`、`s=7`，Java 侧 GenEngineFixtures 同款定义）。

use std::path::{Path, PathBuf};

use calamine::{Data, Reader, Xlsx};
use rpx_engine::params::ParamTable;
use rpx_engine::value::Value;

/// 20 名义夹具（顺序 = engine 侧 `loader.rs::all_names`；末两位 M5-04 新增）。
fn names() -> [&'static str; 20] {
    [
        "engine_sum",
        "engine_ext",
        "engine_expr",
        "engine_pager",
        "engine_merge",
        "engine_cycle",
        "engine_samemerge",
        "engine_disp",
        "engine_types",
        "engine_rowheight",
        "engine_head_top",
        "engine_head_left",
        "engine_avgrange",
        "engine_collate",
        "engine_listrange",
        "engine_param",
        "engine_gbk",
        "engine_emp",
        "engine_cross",
        "engine_cross_root",
    ]
}

/// 错误型夹具（无计算态网格 ⇒ 不可导出；见模块文档）。
const ERROR_FIXTURE: &str = "engine_cycle";

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn out_dir() -> PathBuf {
    fixtures_dir().join("out")
}

/// 夹具标准输入参数（仅 `engine_param` 有；出处见模块文档）。
fn params_for(name: &str) -> ParamTable {
    let mut t = ParamTable::new();
    if name == "engine_param" {
        t.insert(
            "p",
            Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
        );
        t.insert("s", Value::Int(7));
    }
    t
}

/// 一夹具 → 计算态网格（与 server `/export`、CLI `render` 同构的链路）。
fn calculate(name: &str) -> Result<rpx_engine::arena::CalcGrid, String> {
    let dir = fixtures_dir();
    let rd = rpx_core::open_bytes(
        &std::fs::read(dir.join(format!("{name}.rpx"))).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let ds = rpx_engine::dataset::load_datasets_full(
        &rd,
        &|file_name| std::fs::read(dir.join(file_name)).ok(),
        None,
    )
    .map_err(|e| e.to_string())?;
    let mut grid =
        rpx_engine::calculate_with(&rd, &ds, &params_for(name)).map_err(|e| e.to_string())?;
    let pg = rpx_engine::pager::paginate_with_setup(&grid, &rd.print_setup)
        .map_err(|e| e.to_string())?;
    grid.page_count = pg.page_count();
    grid.pagination = Some(pg);
    Ok(grid)
}

/// xlsx 读回规范化文本：逐格 `(r,c)=Data`（相对 range 起点、跳过 Empty）
/// + 合并区矩形（0 起，排序后）。
fn xlsx_readback_text(bytes: &[u8]) -> String {
    let mut wb: Xlsx<_> =
        calamine::open_workbook_from_rs(std::io::Cursor::new(bytes)).expect("xlsx 应可读回");
    let name = wb.sheet_names().first().expect("至少一个 sheet").clone();
    let range = wb.worksheet_range(&name).expect("sheet 应可读");
    let mut merged = wb.merge_cells_by_sheet_name(&name).expect("合并区应可读");
    merged.sort_by_key(|d| (d.start, d.end));
    let mut out = String::new();
    for (r, c, v) in range.cells() {
        if !matches!(v, Data::Empty) {
            out.push_str(&format!("({r},{c})={v:?}\n"));
        }
    }
    for d in &merged {
        out.push_str(&format!("merge {:?}-{:?}\n", d.start, d.end));
    }
    out
}

/// 首个差异行的定位描述（长文本 assert 的报错只管一行，这里给上下文）。
fn first_diff_line(new: &str, base: &str) -> String {
    // ⛔ 按**字符**截断（按字节切会切断中文的 UTF-8 边界 panic）。
    fn clip(s: &str) -> String {
        if s.chars().count() <= 120 {
            s.to_string()
        } else {
            s.chars().take(120).collect::<String>() + "…"
        }
    }
    let (ln, lb): (Vec<_>, Vec<_>) = (new.lines().collect(), base.lines().collect());
    for i in 0..ln.len().max(lb.len()) {
        if ln.get(i) != lb.get(i) {
            let a = ln
                .get(i)
                .map(|s| clip(s))
                .unwrap_or_else(|| "<行缺失>".into());
            let b = lb
                .get(i)
                .map(|s| clip(s))
                .unwrap_or_else(|| "<行缺失>".into());
            return format!("@行{i}：新 {a:?} vs 基线 {b:?}");
        }
    }
    "(逐行相同——长度差)".to_string()
}

/// 生成模式开关（显式维护动作；见模块文档）。
fn update_mode() -> bool {
    std::env::var("RPX_UPDATE_BASELINES").as_deref() == Ok("1")
}

/// 生成模式：写基线（失败**收集**不中断——一次运行看全貌）。
fn write_baseline(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().expect("out/ 应存在")).map_err(|e| e.to_string())?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

fn read_baseline(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| {
        panic!(
            "基线缺失 {}（{e}）——先跑 RPX_UPDATE_BASELINES=1 cargo test -p rpx-export --test export_baseline 生成",
            path.display()
        )
    })
}

#[test]
fn csv_baselines_match_or_update() {
    let update = update_mode();
    let mut report: Vec<String> = Vec::new();
    let mut n = 0;
    for name in names() {
        if name == ERROR_FIXTURE {
            continue; // 活证见 engine_cycle_is_error_fixture_thus_excluded
        }
        let grid = calculate(name).unwrap_or_else(|e| panic!("{name} 计算失败：{e}"));
        let bytes = rpx_export::to_csv(&grid).into_bytes();
        let path = out_dir().join(format!("export_{name}.csv"));
        if update {
            if let Err(e) = write_baseline(&path, &bytes) {
                report.push(format!("{} 写失败：{e}", path.display()));
            }
        } else {
            let base = read_baseline(&path);
            if base != bytes {
                report.push(format!(
                    "{}：{}",
                    path.display(),
                    first_diff_line(
                        &String::from_utf8_lossy(&bytes),
                        &String::from_utf8_lossy(&base)
                    )
                ));
            }
        }
        n += 1;
    }
    assert_eq!(n, 19, "可导集合 = 20 名义 − engine_cycle");
    assert!(
        report.is_empty(),
        "CSV 导出基线 diff（若变更是预期的：RPX_UPDATE_BASELINES=1 重生成）：\n{}",
        report.join("\n")
    );
}

#[test]
fn xlsx_baselines_match_or_update() {
    let update = update_mode();
    let mut report: Vec<String> = Vec::new();
    let mut n = 0;
    for name in names() {
        if name == ERROR_FIXTURE {
            continue;
        }
        let grid = calculate(name).unwrap_or_else(|e| panic!("{name} 计算失败：{e}"));
        let bytes = rpx_export::to_xlsx(&grid);
        let path = out_dir().join(format!("export_{name}.xlsx"));
        if update {
            if let Err(e) = write_baseline(&path, &bytes) {
                report.push(format!("{} 写失败：{e}", path.display()));
            }
        } else {
            let base = read_baseline(&path);
            let new_text = xlsx_readback_text(&bytes);
            let base_text = xlsx_readback_text(&base);
            if new_text != base_text {
                report.push(format!(
                    "{}：{}",
                    path.display(),
                    first_diff_line(&new_text, &base_text)
                ));
            }
        }
        n += 1;
    }
    assert_eq!(n, 19, "可导集合 = 20 名义 − engine_cycle");
    assert!(
        report.is_empty(),
        "xlsx 导出基线**读回** diff（若变更是预期的：RPX_UPDATE_BASELINES=1 重生成）：\n{}",
        report.join("\n")
    );
}

#[test]
fn engine_cycle_is_error_fixture_thus_excluded() {
    // 「19 而非 20」的口径活证：engine_cycle 无计算态网格（必报循环引用）。
    let err = calculate(ERROR_FIXTURE).expect_err("engine_cycle 必须报错，绝不产出网格");
    assert!(
        err.contains("circular reference"),
        "应为循环引用错误，实得：{err}"
    );
}
