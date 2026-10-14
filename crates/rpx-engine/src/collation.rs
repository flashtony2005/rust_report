//! M2-12 `Text-Text` 字典序：`ChineseComparator` 的移植（数据表 + NFD + 主序 + 比较）。
//!
//! **算法正文见 `docs/m2-collation.md`**（§1 产品机制 / §2 数据反推 / §3 `collation.bin`
//! 13 段布局 / §4 三步算法 / §5 三条坑与证据）。本模块只留"装载 + 入口 + 钉子"。
//!
//! # 契约
//!
//! * [`compare_text`] = 两串 primary(u16) 序列的**字典序**（一方为前缀 ⇒ 短者小）；
//! * [`primary_seq`] 走 `NFD → 收缩贪心最长匹配 / 可忽略截断 / 单映射（含 0x7FFF 兜底）`；
//! * [`nfd`] 按码点展开（Hangul 走 L/V/T，其余查表），再对每段连续 `ccc≠0` 稳定升序重排。
//!
//! # 事实源（出处逐条落在 md 里）
//!
//! 产品 `com.raqsoft.commonl.ChineseComparator`：`reportlite/lib/reportlite.jar!`
//! `/com/raqsoft/commonl/rule.txt`（85,231 字节）经 `ChineseComparator.java` 构造成 JDK
//! `java.text.RuleBasedCollator`。数据表 `collation.bin` 由产品**实测行为**导出
//! （生成器 `tests/fixtures/GenCollationData.java`），本 crate 只按冻结格式读它。
//!
//! 调用点在 `Variant2.java:426`（`compare2` 的 String 分支）；上游 `<`/`>` 见
//! `operator/Smaller.java:40` 等，`max`/`min` 见 `function/Max.java:57,106`。
//!
//! ⛔ `==`（`BinOp::Eq`）**不**经此模块：它走 `Variant2.equals:469-470` 的精确字符串比较，
//! 见 `expr/num.rs::val_eq` 的说明。

mod nfd;
mod seq;
mod tables;

use tables::Tables;

pub use nfd::nfd;
pub use seq::{primary_seq, primary_seq_cps};

use core::cmp::Ordering;

/// `collation.bin` 的编译期内嵌字节（路径自本文件起算 → `crates/rpx-engine/data/`）。
const BIN: &[u8] = include_bytes!("../data/collation.bin");

/// 进程内唯一的表实例（装载逻辑与 `OnceLock` 在 [`tables::global`]）。
fn table() -> &'static Tables {
    tables::global(BIN)
}

/// 两串按**产品字典序**比较（`ChineseComparator.compare`）。
pub fn compare_text(a: &str, b: &str) -> Ordering {
    compare_cps(&cps(a), &cps(b))
}

/// 码点序列版比较：`primary_seq(a).cmp(primary_seq(b))`。
///
/// 暴露它是为了让测试能覆盖 UTF-16 孤立代理（`&str` 无法承载 `D800..DFFF`，
/// 但金标准 `_collation_golden.tsv` 的 PAIR 段含此类码点）。
pub fn compare_cps(a: &[u32], b: &[u32]) -> Ordering {
    primary_seq_cps(a).cmp(&primary_seq_cps(b))
}

/// `&str` → 码点序列（`char` 即 Unicode 标量值，恒非代理）。
fn cps(s: &str) -> Vec<u32> {
    s.chars().map(|c| c as u32).collect()
}

#[cfg(test)]
mod tests {
    use super::BIN;

    /// **表指纹钉子**：`collation.bin` 的内容与长度被冻死。
    ///
    /// 这是"防止 `.bin` 被静默替换"的钉子，**不是正确性证明** —— 正确性由
    /// `tests/collation_golden.rs`（全量 dump / golden / NFD / 收缩 / 参照值）负责。
    /// 一旦产品侧重生成 `.bin`，本测试必须**响亮失败**，提醒重跑那批金标准。
    #[test]
    fn collation_bin_fingerprint_is_pinned() {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a 64 偏移基
        for &b in BIN {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3); // FNV 质数
        }
        assert_eq!(BIN.len(), 81_444, "collation.bin 长度被改动");
        assert_eq!(
            h, 0x3998_5A59_A7D2_6227,
            "collation.bin 内容被改动（FNV-1a 指纹不符）；\
             若确属产品侧重生成，请连同金标准一起复核并更新本常量"
        );
    }
}
