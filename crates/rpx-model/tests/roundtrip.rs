//! `.rpx` **往返**（M7-2）：`open → to_bytes → open`，两层判据。
//!
//! # 判据一：模型往返（**必须恒成立**）
//!
//! `open(to_bytes(rd)) == rd` —— 写侧不得丢任何模型信息。这是写侧的**底线**：
//! 模型对不上就说明某个槽写错/漏写。
//!
//! # 判据二：字节往返（**对真实夹具必须成立**）
//!
//! `to_bytes(open(original)) == original` —— 逐字节复现。这比判据一强得多：
//! 它同时钉住了「槽序」「空槽形态（int32 -1）」「可用守卫尾巴一律写出」
//! 「字符串标签选择（≤65535 走标签 1）」等**读侧丢信息、写侧只能取规范形态**的地方。
//!
//! 两条判据的关系：判据一过而判据二不过 ⇒ 写侧语义正确但形态不唯一（可接受，
//! 但必须**逐条记录在案**）；判据一不过 ⇒ 写侧有真 bug。本仓口径是**两条都要过**，
//! 故此处不设「容忍清单」——真有例外必须写进本文件头部并给出理由。
//!
//! 输入 = `tests/fixtures/*.rpx`（真实夹具，Windows 设计器/探针产出）。

use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// 全部 `.rpx` 夹具（文件名排序，保证输出稳定）。
fn all_fixtures() -> Vec<(String, Vec<u8>)> {
    let dir = fixtures_dir();
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("夹具目录可读") {
        let path = entry.expect("目录项可读").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rpx") {
            continue;
        }
        let name = path
            .file_name()
            .expect("有文件名")
            .to_string_lossy()
            .into_owned();
        out.push((name, std::fs::read(&path).expect("夹具可读")));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!out.is_empty(), "至少应有一个 .rpx 夹具");
    out
}

/// 首个差异字节的位置与周边窗口（诊断用）。
fn first_diff(a: &[u8], b: &[u8]) -> String {
    let n = a.len().min(b.len());
    match (0..n).find(|&i| a[i] != b[i]) {
        None if a.len() == b.len() => "无差异（长度也相同）".to_string(),
        None => format!("前 {n} 字节相同，长度不同：{} vs {}", a.len(), b.len()),
        Some(i) => {
            let lo = i.saturating_sub(8);
            let hi_a = (i + 8).min(a.len());
            let hi_b = (i + 8).min(b.len());
            format!(
                "首个差异 @ byte {i}：原始 {:02X?} / 回写 {:02X?}",
                &a[lo..hi_a],
                &b[lo..hi_b]
            )
        }
    }
}

#[test]
fn model_round_trip_holds_for_every_fixture() {
    for (name, bytes) in all_fixtures() {
        let rd = rpx_core::open_bytes(&bytes).unwrap_or_else(|e| panic!("{name}: 原始解不开：{e}"));
        let out = rpx_core::to_bytes(&rd).unwrap_or_else(|e| panic!("{name}: 写失败：{e}"));
        let rd2 =
            rpx_core::open_bytes(&out).unwrap_or_else(|e| panic!("{name}: 回写物解不开：{e}"));
        assert_eq!(rd2, rd, "{name}: 模型往返不等（写侧丢了信息）");
    }
}

#[test]
fn byte_exact_round_trip_holds_for_every_fixture() {
    let mut failures = Vec::new();
    for (name, bytes) in all_fixtures() {
        let rd = rpx_core::open_bytes(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let out = rpx_core::to_bytes(&rd).unwrap_or_else(|e| panic!("{name}: {e}"));
        if out != bytes {
            failures.push(format!("  {name}: {}", first_diff(&bytes, &out)));
        }
    }
    assert!(
        failures.is_empty(),
        "以下夹具未能字节复现（{} 个）：\n{}",
        failures.len(),
        failures.join("\n")
    );
}
