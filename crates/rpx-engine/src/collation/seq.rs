//! 第二、三步：primary(u16) 序列与字典序，算法见 `docs/m2-collation.md` §4.2 / §4.3。
//!
//! 输出是**扁平 u16 主序序列**。比较就是两条序列的字典序（前缀短者小，即 `Vec::cmp`）。
//! 全部 CE 的 secondary/tertiary 恒为 0（md §5.2 实测 151,485 个 CE），故无需多级权重。

use super::nfd::nfd;
use super::table;
use super::tables::{Tables, CJK_HI, CJK_LO, UNMAPPED};

/// 一整串 → primary 序列。
pub fn primary_seq(s: &str) -> Vec<u16> {
    let cps: Vec<u32> = s.chars().map(|c| c as u32).collect();
    primary_seq_cps(&cps)
}

/// 码点序列版：测试覆盖 UTF-16 孤立代理（`&str` 承载不了 `D800..DFFF`）时用。
pub fn primary_seq_cps(cps: &[u32]) -> Vec<u16> {
    let t = table();
    let n = nfd(cps);
    let mut out: Vec<u16> = Vec::with_capacity(n.len());
    let mut i = 0;
    while i < n.len() {
        let cp = n[i];
        if let Some((kidx, kcnt, ignorable)) = t.contract(cp) {
            // 收缩贪心最长匹配：键长恒为 2（md §4.2），只看紧邻的下一码点。
            if i + 1 < n.len() {
                let next = n[i + 1];
                if let Some(ce) = match_key(t, kidx, kcnt, cp, next) {
                    out.push(ce);
                    i += 2;
                    continue;
                }
            }
            if ignorable {
                // ⛔ 可忽略字符是"截断"而不是"跳过"：命中可忽略起始字符且未命中键时，
                //    丢弃**整个后缀**（md §5.3；实测 CE("\u{301}A") == []）。
                return out;
            }
            push_single(&mut out, t, cp);
            i += 1;
            continue;
        }
        push_single(&mut out, t, cp);
        i += 1;
    }
    out
}

/// 在该起始字符的键组 `[kidx, kidx+kcnt)` 内找 `(cp0, cp1)`，命中返回键的主序。
///
/// 组内键为插入序、无序且至多 6 个 ⇒ 线性扫描；组已是**预过滤后**的活键（`fwd=true`），
/// Rust 侧不再过滤（md §4.2）。
fn match_key(t: &Tables, kidx: usize, kcnt: usize, cp0: u32, cp1: u32) -> Option<u16> {
    (kidx..kidx + kcnt)
        .find(|&k| t.contract_key(k) == (cp0, cp1))
        .map(|k| t.contract_key_ce(k))
}

/// 单码点映射：CJK 查段 0；其余查段 1；未映射发 `[0x7FFF, cp]`，
/// `cp > 0xFFFF` 时改发 `[0x7FFF, 高代理, 低代理]`（md §4.2）。
fn push_single(out: &mut Vec<u16>, t: &Tables, cp: u32) {
    if (CJK_LO..=CJK_HI).contains(&cp) {
        out.push(t.cjk_primary(cp));
        return;
    }
    if let Some(p) = t.single_primary(cp) {
        out.push(p);
        return;
    }
    out.push(UNMAPPED);
    if cp > 0xFFFF {
        let c = cp - 0x1_0000;
        out.push(0xD800 + (c >> 10) as u16);
        out.push(0xDC00 + (c & 0x3FF) as u16);
    } else {
        out.push(cp as u16);
    }
}
