//! 健壮性回补测试 —— 针对 2026-10-12 独立验证（QA）确认的两处缺口。
//!
//! 1. **矩阵维度 DoS（严重）**：`ReportDefine::fill` 原先直接用未校验的 i32 维度做
//!    `Vec::with_capacity(rows-1)` 与 `vec![vec![None; cols]; rows]`。一个 24 字节文件
//!    （rows=i32::MAX, cols=1）会让进程以 `memory allocation of 188978560848 bytes failed`
//!    **abort（exit 127）**，而不是返回 Err —— 违反 AC-02「绝不 panic」，且 M3 服务一旦
//!    接收上传文件即可被远程打崩。修复见 `report.rs` 的 `max_slots` 上界。
//! 2. **记录层截断零覆盖**：golden 的 `never_panics_on_any_truncation` 只喂 `open_bytes`，
//!    16313 个截断点全部在文件层的长度/页脚守卫处提前返回，从未进入 payload 记录层。
//!    本文件直接对 payload 的每个前缀调用 `ReportDefine::fill`。

use rpx_core::ReportDefine;

const MAGIC: [u8; 4] = [82, 81, 81, 82];
const FOOTER: [u8; 8] = [82, 81, 81, 82, 82, 81, 81, 82];

/// 最小合法文件壳：`RQQR | int32 payload 长度 | payload | 8 字节页脚`。
fn synth_file(payload: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(payload.len() + 16);
    f.extend_from_slice(&MAGIC);
    f.extend_from_slice(&(payload.len() as i32).to_be_bytes());
    f.extend_from_slice(payload);
    f.extend_from_slice(&FOOTER);
    f
}

/// payload 只放步骤 1-2 的两个维度。
fn dims_only(rows: i32, cols: i32) -> Vec<u8> {
    let mut p = Vec::with_capacity(8);
    p.extend_from_slice(&rows.to_be_bytes());
    p.extend_from_slice(&cols.to_be_bytes());
    p
}

/// 超大维度必须**优雅返回 Err**。若退回旧实现，此处是进程 abort —— 整个测试进程都不会
/// 走到断言，因此「测试通过」本身就等价于「没有 abort」。
#[test]
fn rejects_absurd_matrix_dims() {
    let cases: [(i32, i32, &str); 6] = [
        (i32::MAX, 1, "rows=INT_MAX"),
        (1, i32::MAX, "cols=INT_MAX"),
        (i32::MAX, i32::MAX, "rows=cols=INT_MAX"),
        (1 << 20, 1 << 20, "1M x 1M grid"),
        (1 << 16, 1, "64K rows"),
        (i32::MIN, 1, "rows=i32::MIN"),
    ];
    for (rows, cols, what) in cases {
        let file = synth_file(&dims_only(rows, cols));
        let err = rpx_core::open_bytes(&file)
            .err()
            .unwrap_or_else(|| panic!("{}: dims ({},{}) must be rejected", what, rows, cols));
        assert!(!err.to_string().is_empty(), "{}: error must carry a message", what);
    }
}

/// 合法维度不能被新上界误伤（回归对照）。
#[test]
fn accepts_plausible_dims_but_rejects_missing_body() {
    // 维度本身合理，但步骤 3 起缺记录 → 必须是 Err（不是 abort、不是 Ok）
    let file = synth_file(&dims_only(4, 4));
    let err = rpx_core::open_bytes(&file).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("truncated") || msg.contains("slot"), "unexpected error: {}", msg);
}

/// 记录层截断：payload 的每个前缀都必须 Err，且任何 panic 都会被捕获并报出具体位置。
#[test]
fn record_layer_truncation_never_panics() {
    let fixtures: [(&str, &[u8]); 4] = [
        (
            "simple",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/simple.rpx")),
        ),
        (
            "merged",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/merged.rpx")),
        ),
        (
            "full",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/full.rpx")),
        ),
        (
            "realmerge",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/realmerge.rpx")),
        ),
    ];
    let mut checked = 0usize;
    for (name, rpx) in fixtures {
        let payload = &rpx[8..rpx.len() - 8];
        assert!(ReportDefine::fill(payload).is_ok(), "{}: intact payload must parse", name);
        for cut in 0..payload.len() {
            let prefix = &payload[..cut];
            let res = std::panic::catch_unwind(|| ReportDefine::fill(prefix));
            let parsed = res.unwrap_or_else(|_| panic!("{}: payload prefix {} panicked", name, cut));
            assert!(parsed.is_err(), "{}: payload prefix {} must be rejected", name, cut);
            checked += 1;
        }
    }
    // 四个夹具 payload 前缀合计应远超 2 万（simple 3610 + merged 5180 + full 7475 + realmerge 4981）
    assert!(checked > 20_000, "record-layer sweep covered only {} prefixes", checked);
}
