//! M5-09 坐标语义的**变异钉子**：三类最可能被后人"顺手统一"的语义，各钉一处。
//!
//! # 为什么需要钉子，而不是再抄一遍 `expected.json`
//!
//! 坐标语义里有三处"看起来等价、实际分叉"的写法，任何一处被出于整洁而改写，
//! 都不会有任何**普通**测试变红 —— 因为夹具比对只回答"值对不对"，不回答
//! "这条语义还有没有承重"。钉子测试把**承重关系**写成断言：
//!
//! | 变异 | 改写方式 | 承重夹具 |
//! |------|----------|----------|
//! | M1 序号基准 | `coord.rs:274` `n - 1` → `n`；`coord.rs:376` `i as i32 + 1` → `i as i32` | `engine_hc_abs` / `engine_hc_amp` |
//! | M2 链次序 | `coord.rs:323` `for seg in &coord.left` → `coord.left.iter().rev()` | `engine_hc_order` |
//! | M3 位移基准 | `coord.rs:279` `resolve_cell(ctx, &base_id, false)`（书写格） → `Ok(cands[0])`（首实例） | `engine_hc_offset` |
//! | M4 合并别名 | `stage_b/merge_alias.rs:59` 横向延续槽并入纵向影子格路径 | `engine_hc_merge` |
//!
//! **M1–M3 是 §6 验收标准第 3 条点名的三类**；M4 是 #7 夹具落地后补的同类钉子
//! （别名 vs 独立格，与 M1–M3 同性质：一处忠实实现的分支在当前夹具上恒等于某个
//! 简化写法）。
//!
//! 2026-10-20 四条变异**实跑过一次**，均使本文件变红（逐条结论见
//! `tests/fixtures/RUNLOG.md` §48）。注意 M1 的变异会让 `[A2:3]` 越界，
//! 先把 `calculate` 打死（报 `unknown cell`），红在 `calc()` 里而不是断言里 ——
//! 依然是红，但读日志时别误以为是夹具坏了。
//!
//! # 每条钉子必须自带"非空转"守卫
//!
//! 光断言"值 = 正确值"是弱钉子：若夹具里两个候选语义**恰好同值**，变异后测试
//! 照样绿。所以每条都先证明"变异预测值 ≠ 正确值"（候选格确实不同值），再钉值。
//! 守卫一破就红 —— 红的地方就是该去补夹具、而不是改断言的地方。
//!
//! # 怎么跑变异（本文件写完后实跑过，结论见 `tests/fixtures/RUNLOG.md`）
//!
//! 改 `coord.rs` 对应一行 → `cargo test -p rpx-engine --test hc_tripwire` 必须红
//! → `git checkout -- crates/rpx-engine/src/expr/coord.rs` 复原。

mod common;

use std::path::PathBuf;

use rpx_engine::{CalcGrid, Value};

/// 夹具目录口径与 `tests/engine_grid_compare.rs` / `tests/common/loader.rs` 一致。
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// 跑一个引擎夹具，返回计算态网格（参数表照抄夹具 `params` 段）。
fn calc(name: &str) -> CalcGrid {
    let rd = common::open_json_fixture(name);
    let ds = rpx_engine::dataset::load_datasets(&fixtures_dir(), &rd)
        .unwrap_or_else(|e| panic!("{name}: load_datasets 失败：{e}"));
    let fx = common::load(name);
    let params = rpx_engine::ParamTable::from_iter(
        fx.params.iter().map(|p| (p.name.clone(), p.value.clone())),
    );
    rpx_engine::calculate_with(&rd, &ds, &params)
        .unwrap_or_else(|e| panic!("{name}: calculate 失败：{e}"))
}

/// 计算态 `(r, c)` 的值；格里没有该坐标 ⇒ `Null`（与"未落值的格"同义）。
fn value(g: &CalcGrid, r: i32, c: i32) -> Value {
    g.get(r, c)
        .map(|cell| cell.value.clone())
        .unwrap_or(Value::Null)
}

/// 文本格的可见串（非文本值直接 panic，避免用 `{:?}` 蒙混）。
fn text(g: &CalcGrid, r: i32, c: i32) -> String {
    match value(g, r, c) {
        Value::Text(s) => s,
        v => panic!("({r},{c}) 期望文本，实际是 {v:?}"),
    }
}

/// 数值格（整数型）。
fn int(g: &CalcGrid, r: i32, c: i32) -> i32 {
    match value(g, r, c) {
        Value::Int(n) => n,
        v => panic!("({r},{c}) 期望整数，实际是 {v:?}"),
    }
}

/// **M1：序号是 1 起，不是 0 起。**
///
/// 承重格（`engine_hc_abs`，计算态 14×9）：
/// - `(10,7)` 模板 F8 = `B2[A2:2;B1:2]` ⇒ `"N/B"`
/// - `(10,8)` 模板 G8 = `B2[A2:3]`      ⇒ `"E/C"`
/// - `(10,9)` 模板 H8 = `B2[;B1:2]`     ⇒ `"N/A"`
///
/// 变异 M1 会把 `A2:2` 读成第 3 个实例 ⇒ `(10,7)` 变 `"N/C"`、`(10,8)` 越界变空。
/// `engine_hc_amp` 的 `&A2` 同样承重：正确值是 1/2/3，0 起会得到 0/1/2。
#[test]
fn m1_coordinate_index_starts_at_one() {
    let g = calc("engine_hc_abs");

    // 守卫：同一列 B2 的第 1/2 个实例必须**不同值**，否则"错位一格"观察不到。
    assert_ne!(
        text(&g, 2, 2),
        text(&g, 3, 2),
        "B2 的两个实例同值 ⇒ 序号错位不可观测（夹具退化，或实现把序号钉死了）"
    );

    assert_eq!(text(&g, 10, 7), "N/B", "[A2:2;B1:2] 必须是第 2 行第 2 列");
    assert_eq!(text(&g, 10, 8), "E/C", "[A2:3] 必须是第 3 行（缺列取行首）");
    assert_eq!(
        text(&g, 10, 9),
        "N/A",
        "[;B1:2] 必须是第 2 列（缺行取列首）"
    );

    // `&` 的基准同理：1 起（教程 063_xh141「组内序号」）。
    let amp = calc("engine_hc_amp");
    let seq = [int(&amp, 2, 4), int(&amp, 3, 4), int(&amp, 4, 4)];
    assert_eq!(seq, [1, 2, 3], "&A2 在三个实例上必须是 1/2/3");
    assert!(
        !seq.contains(&0),
        "&A2 出现 0 ⇒ 已退化成 0 起计数（M1 的 amp 分支没被咬住）"
    );
}

/// **M2：主格链是「从远到近」，反序解析会落到另一格。**
///
/// 承重格（`engine_hc_order`，计算态 17×8，部门→组→人 三级链）：
/// - `(15,6)` 模板 F8 = `D2[A2:2,B2:1,C2:2]` ⇒ **212**
/// - `(15,7)` 模板 G8 = `D2[A2:1,B2:2,C2:1]` ⇒ **121**
///
/// 值本身用「部门*100+组*10+人」编码（模板 D2 的 `&A2*100+&B2*10+&C2`），
/// 所以一条链的每一位都能反读出来：212 ⇔ 第 2 部门 / 第 1 组 / 第 2 人。
/// 反序（`[C2:2,B2:1,A2:2]`）从根先选人，落到的是另一条路径 ⇒ 不再是 212。
#[test]
fn m2_master_chain_runs_far_to_near() {
    let g = calc("engine_hc_order");

    // 守卫：两条链必须落在不同实例，否则"次序"不可观测。
    assert_ne!(
        value(&g, 15, 6),
        value(&g, 15, 7),
        "两条坐标链同值 ⇒ 次序不可观测（夹具退化，或实现忽略链次序）"
    );

    assert_eq!(int(&g, 15, 6), 212, "[A2:2,B2:1,C2:2] 必须是 D2/T1/P2");
    assert_eq!(int(&g, 15, 7), 121, "[A2:1,B2:2,C2:1] 必须是 D1/T2/P1");

    // 三级都必须有 >1 的实例，否则"链上某一级"是恒等映射，反序看不出差别。
    let grid = &g;
    let persons = grid
        .cells
        .iter()
        .filter(|c| c.col == 3 && c.row >= 2 && c.row <= 5)
        .count();
    assert!(
        persons >= 4,
        "第三人级实例不足 ⇒ 链次序退化（需 >=4 个实例才能分辨反序）"
    );
}

/// **M3：位移坐标的原点是「写这条表达式的格」，不是目标格的首实例。**
///
/// 承重格（`engine_hc_offset`，计算态 14×9）：同一份模板列在 A2 的三个实例下
/// 各复制一份，`A2:-1` / `A2:+1` 因此得到三组不同结果：
///
/// | 计算态 | 模板 | 表达式 | 值 |
/// |--------|------|--------|-----|
/// | `(2,4)` | D2 | `B2[A2:-1;B1:1]` | `null`（第 1 行再往上越界） |
/// | `(3,4)` | D2 | 同上（第 2 个副本） | `"E/A"`（退到第 1 行） |
/// | `(4,4)` | D2 | 同上（第 3 个副本） | `"E/B"`（退到第 2 行） |
/// | `(3,5)` | E2 | `B2[A2:+1;B1:1]` | `"E/C"`（第 2 行前进到第 3 行） |
///
/// 变异 M3（原点钉在第 1 个实例）会让三个副本**同值**，且 `(3,5)` 退化成 `"E/B"`。
#[test]
fn m3_offset_is_anchored_at_the_writing_cell() {
    let g = calc("engine_hc_offset");

    // 守卫：三个副本至少出现两种结果，否则"基准随书写格移动"不可观测。
    let copies = [value(&g, 2, 4), value(&g, 3, 4), value(&g, 4, 4)];
    assert!(
        copies.windows(2).any(|w| w[0] != w[1]),
        "三个副本同值 ⇒ 位移基准不可观测（夹具退化，或实现把原点钉死了）"
    );

    assert_eq!(copies[0], Value::Null, "(2,4) 第 1 行再 -1 必须越界成 null");
    assert_eq!(text(&g, 3, 4), "E/A", "(3,4) 第 2 行 -1 必须退到第 1 行");
    assert_eq!(text(&g, 4, 4), "E/B", "(4,4) 第 3 行 -1 必须退到第 2 行");
    // `+1` 方向单独钉一次：M3 会把它变成"首实例 +1" ⇒ E/B 而不是 E/C。
    assert_eq!(text(&g, 3, 5), "E/C", "(3,5) 第 2 行 +1 必须前进到第 3 行");
}

/// **M4：合并延续槽是锚点的别名，不是独立格。**
///
/// 承重格（`engine_hc_merge`，计算态 14×8，B2:C2 横向合并）：
/// - `(10,7)` 模板 G8 = `C2[A2:2]` ⇒ `"D2#"`（延续槽读锚点值）
/// - `(11,6)` / `(11,7)` = `count(C2[`0]{})` / `count(B2[`0]{})` ⇒ 都是 **3**
///
/// 若延续槽被当成独立格：`C2[A2:2]` 会读到 C2 自己（空），格集计数变成 6。
#[test]
fn m4_merged_continuation_slot_aliases_the_anchor() {
    let g = calc("engine_hc_merge");

    // 守卫：锚点与延续槽指向同一模板区，若两者都空则本钉子无意义。
    assert_eq!(
        value(&g, 2, 3),
        value(&g, 2, 2),
        "合并区首实例的延续槽必须与锚点同值（别名语义）"
    );

    assert_eq!(text(&g, 10, 7), "D2#", "C2[A2:2] 必须读到锚点第 2 实例");
    assert_eq!(int(&g, 11, 6), 3, "count(C2[`0]{{}}) 不得把延续槽重复计数");
    assert_eq!(int(&g, 11, 7), 3, "count(B2[`0]{{}}) 锚点侧同为 3");
}
