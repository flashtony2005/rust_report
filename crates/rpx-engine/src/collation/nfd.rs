//! 第一步：NFD（规范分解 + 规范重排），算法见 `docs/m2-collation.md` §4.1。
//!
//! 与产品行为等价的两处（md §1）：
//! * 产品把 NFD 交给 JDK `RuleBasedCollator`（`decomposition=CANONICAL_DECOMPOSITION`）
//!   内部完成；我们把它显式做成模型第一步，且**只做一次**（再叠一次 NFC/NFD 会改结果）。
//! * Hangul 音节不进表、由标准 L/V/T 算法分解（md §4.1），否则会与产品不一致。

use super::table;
use super::tables::{HANGUL_HI, HANGUL_LO};

/// 对码点序列做 NFD：先逐码点展开，再对每段连续 `ccc != 0` 的码点稳定升序重排。
///
/// 入参用码点（`&[u32]`）而非 `&str`，因为 `docs/m2-collation.md` §5.1 的反例
/// `U+FA6C → U+242EE` 落在增补平面：按 UTF-16 `char` 遍历会错，码点遍历才对。
pub fn nfd(cps: &[u32]) -> Vec<u32> {
    let t = table();
    let mut out: Vec<u32> = Vec::with_capacity(cps.len() + 4);
    for &cp in cps {
        if (HANGUL_LO..=HANGUL_HI).contains(&cp) {
            // 标准 Hangul 算法分解：SIndex = cp − 0xAC00；L = 0x1100 + S/588；
            // V = 0x1161 + (S%588)/28；T = S%28，仅当 >0 时追加 0x11A7 + T。
            let s = cp - HANGUL_LO;
            out.push(0x1100 + s / 588);
            out.push(0x1161 + (s % 588) / 28);
            let t_index = s % 28;
            if t_index > 0 {
                out.push(0x11A7 + t_index);
            }
        } else if let Some((off, len)) = t.nfd_of(cp) {
            // ⛔ len 可以是 1（如 U+0340 → U+0300），不能因为"看起来没展开"就跳过。
            for k in off..off + len {
                out.push(t.nfd_pool(k));
            }
        } else {
            out.push(cp);
        }
    }
    // 规范重排：逐个"连续的 ccc≠0 段"稳定升序（`slice::sort_by_key` 稳定）。
    // ccc 查不到视为 0 ⇒ 作为起始字符切断重排段。
    let mut i = 0;
    while i < out.len() {
        if t.ccc(out[i]) == 0 {
            i += 1;
            continue;
        }
        let start = i;
        while i < out.len() && t.ccc(out[i]) != 0 {
            i += 1;
        }
        out[start..i].sort_by_key(|&c| t.ccc(c));
    }
    out
}
