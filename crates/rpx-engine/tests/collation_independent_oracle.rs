//! M2-12 独立对拍（验证方 collation-rust-verify 自造，**不复用**现成金标准）。
//!
//! 事实源是**本验证方新种子**生成的 `tests/fixtures/_collation_oracle.tsv`
//! （生成器 `ProbeCollationRustOracle.java`，seed=0x5EED2A17，**非** 20261014 /
//! 314159265）。每行 `<hexA>\t<hexB>\t<-1|0|1>`；hex 是**产品 String.codePoints()
//! 的单元序列**（代理对→增补码点；孤立代理→其自身），Rust 侧喂进同一序列。
//!
//! 另附三条定点钉子：孤立代理走 `[0x7FFF, cp]` 兜底、`U+FA6C → U+242EE` 的增补
//! 平面单分解、以及 `=`（精确相等）与 `<`（collation）分工不同。

use rpx_engine::collation::{compare_cps, compare_text, nfd, primary_seq_cps};
use rpx_engine::expr::{evaluate, DataSet, EvalCtx};
use rpx_engine::{parse, CellId, Value};
use std::cmp::Ordering;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn hex_cps(s: &str) -> Vec<u32> {
    s.split_whitespace()
        .map(|t| u32::from_str_radix(t, 16).unwrap_or_else(|_| panic!("坏 hex token [{t}]")))
        .collect()
}

fn representable(cps: &[u32]) -> bool {
    cps.iter().all(|&c| char::from_u32(c).is_some())
}

fn to_string(cps: &[u32]) -> String {
    cps.iter()
        .map(|&c| char::from_u32(c).unwrap_or_else(|| panic!("U+{c:X} 非标量值")))
        .collect()
}

/// 独立 oracle：逐行比符号，0 不符；并统计两条路径各自的条数。
#[test]
fn independent_oracle_matches_the_product_collator() {
    let path = fixtures_dir().join("_collation_oracle.tsv");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("读 {} 失败：{e}", path.display()));
    let (mut checked, mut mismatched) = (0usize, 0usize);
    let (mut via_text, mut via_cps) = (0usize, 0usize);
    let mut first = String::new();
    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(f.len(), 3, "坏行：{line}");
        let a = hex_cps(f[0]);
        let b = hex_cps(f[1]);
        let want: i32 = f[2].parse().unwrap_or_else(|_| panic!("坏符号 [{line}]"));
        let ord = if representable(&a) && representable(&b) {
            via_text += 1;
            compare_text(&to_string(&a), &to_string(&b))
        } else {
            via_cps += 1;
            compare_cps(&a, &b)
        };
        let got = match ord {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        };
        checked += 1;
        if got != want {
            mismatched += 1;
            if first.is_empty() {
                first = format!("[{a:X?}] vs [{b:X?}] got={got} want={want}");
            }
        }
    }
    assert!(
        checked >= 2000,
        "独立 oracle 行数不足（应 ≥2000）：{checked}"
    );
    assert_eq!(mismatched, 0, "独立 oracle 不符（首例）：{first}");
    println!(
        "SUMMARY checked={checked} mismatched={mismatched} via_text={via_text} via_cps={via_cps}"
    );
}

/// 定点：孤立代理按**未映射**处理 ⇒ `[0x7FFF, cp]`（产品 `ProbeCollationRustOracle` 实测）。
#[test]
fn isolated_surrogates_take_the_unmapped_fallback() {
    assert_eq!(primary_seq_cps(&[0xD800]), vec![0x7FFF, 0xD800]);
    assert_eq!(primary_seq_cps(&[0xDC00]), vec![0x7FFF, 0xDC00]);
}

/// 定点：`U+FA6C → U+242EE`（增补平面的单分解）。
///
/// 这条正是"nfd/primary_seq 必须按**码点**而非 UTF-16 `char` 遍历"的反例：
/// 按 `char` 走会得到 4 个元素（D850/DEEE 两个代理），按码点走才是 3 个。
#[test]
fn fa6c_decomposes_into_a_supplementary_code_point() {
    assert_eq!(nfd(&[0xFA6C]), vec![0x242EE]);
    assert_eq!(primary_seq_cps(&[0xFA6C]), vec![0x7FFF, 0xD850, 0xDEEE]);
}

// ───────────────────────── `=` vs `<` 分工 ─────────────────────────

/// 极简上下文：本测试的常量表达式不触发布局（各方法返回哑值）。
struct NoCtx;

impl EvalCtx for NoCtx {
    fn source_of(&self, c: CellId) -> CellId {
        c
    }
    fn left_head(&self, c: CellId) -> CellId {
        c
    }
    fn top_head(&self, c: CellId) -> CellId {
        c
    }
    fn left_subs(&self, _c: CellId) -> Vec<CellId> {
        Vec::new()
    }
    fn top_subs(&self, _c: CellId) -> Vec<CellId> {
        Vec::new()
    }
    fn ext_cells(&self, _c: CellId) -> Vec<CellId> {
        Vec::new()
    }
    fn extensible(&self, _c: CellId) -> i8 {
        0
    }
    fn root(&self) -> CellId {
        CellId(0)
    }
    fn current(&self) -> CellId {
        CellId(0)
    }
    fn saved_current(&self) -> CellId {
        CellId(0)
    }
    fn set_current(&mut self, _c: CellId) {}
    fn save_current(&mut self) {}
    fn resume_current(&mut self) {}
    fn cell_value(&self, _c: CellId) -> Value {
        Value::Null
    }
    fn source_by_id(&self, _id: &str) -> Option<CellId> {
        None
    }
    fn ds(&mut self, _name: &str) -> Option<&mut DataSet> {
        None
    }
    fn ds_index(&self, _name: &str) -> Option<usize> {
        None
    }
    fn param(&self, _name: &str, _is_macro: bool) -> Option<Value> {
        None
    }
}

/// `=`（精确字符串相等）与 `<`（collation）**语义不同**，且 `=` 不得走 collator。
///
/// 反例：`"\u{301}"`（纯组合符，可忽略 ⇒ primary 截断为空）与 `""`（空串）在
/// collation 下 `Equal`，但作为字符串**不同**。产品 `=` 走 `Variant2.equals`
/// 的精确 `String.equals`，故应为 `false`；`<` 走 `ChineseComparator`，故为 `Less`。
#[test]
fn equality_is_exact_while_less_than_uses_collation() {
    // `<`：collation ⇒ 二者相等。
    assert_eq!(compare_text("\u{301}", ""), Ordering::Equal);

    // `=`：精确相等 ⇒ false（若有人把 `=` 改成走 collation，本条会红）。
    let mut ctx = NoCtx;
    let eq = evaluate(&parse("\"\u{301}\"==\"\"").unwrap(), &mut ctx).unwrap();
    assert!(
        matches!(eq, Value::Bool(false)),
        "`\"\\u301\"==\"\"` 应为精确相等 false，得到 {eq:?}"
    );

    // 同一非空文本的 `=` 仍为 true（不是把 `=` 一律打成 false）。
    let mut ctx2 = NoCtx;
    let eq2 = evaluate(&parse("\"\u{301}\"==\"\u{301}\"").unwrap(), &mut ctx2).unwrap();
    assert!(
        matches!(eq2, Value::Bool(true)),
        "`\"\\u301\"==\"\\u301\"` 应为 true，得到 {eq2:?}"
    );

    // `<` 端到端：组合符串 < 空串 在 collation 下**不成立**（Equal，非 Less）。
    let mut ctx3 = NoCtx;
    let lt = evaluate(&parse("\"\u{301}\"<\"\"").unwrap(), &mut ctx3).unwrap();
    assert!(
        matches!(lt, Value::Bool(false)),
        "`\"\\u301\"<\"\"` 在 collation 下相等 ⇒ 应 false，得到 {lt:?}"
    );
}
