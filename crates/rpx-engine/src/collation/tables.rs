//! `collation.bin` 的装载与只读访问。
//!
//! 冻结布局（全部 little-endian；正文与逐段字段表见 `docs/m2-collation.md` §3）：
//!
//! ```text
//! Header 16B : u32 magic=0x314C4352 | u32 version=1 | u32 sec_cnt=13 | u32 reserved=0
//! 段表       : 13 × ( u32 byte_offset , u32 count )   // offset 自文件起点；各段起始 4 字节对齐
//! 段数据     : 段宽（字节）依次 2,4,4,4,1,4,4,1,4,1,4,2,1
//! ```
//!
//! | # | 名称 | 含义（详见 md §3） |
//! |---|------|--------------------|
//! | 0 | cjk_primary | `index = cp − 0x4E00` |
//! | 1 | singles | `cp \| (primary<<16)`，按 cp 升序 |
//! | 2/3/4/5 | nfd_cp / nfd_off / nfd_len / nfd_pool | 规范分解（不含 Hangul 音节） |
//! | 6/7 | ccc_cp / ccc_val | 非 0 组合类，升序 |
//! | 8 | contract_starter | `cp \| (first_key_index<<16)` |
//! | 9/10/11/12 | contract_count / contract_key / contract_key_ce / ignorable | 收缩表 |
//!
//! ⛔ 本文件**不**做业务判断，只把字节翻成有类型的查询；`.bin` 是生成物，禁止手改。

use std::sync::OnceLock;

const MAGIC: u32 = 0x314C_4352;
const VERSION: u32 = 1;
/// 段数（与生成器 `GenCollationData.SEC_CNT` 一致）。
pub(super) const SEC_CNT: usize = 13;

/// CJK 统一表意区下界（段 0 的索引基）。
pub(super) const CJK_LO: u32 = 0x4E00;
/// CJK 统一表意区上界（含）。
pub(super) const CJK_HI: u32 = 0x9FA5;
/// Hangul 音节区下界（不进 nfd 表，由 L/V/T 算法分解，md §4.1）。
pub(super) const HANGUL_LO: u32 = 0xAC00;
/// Hangul 音节区上界（含）。
pub(super) const HANGUL_HI: u32 = 0xD7A3;
/// 未映射码点的占位主序（`md §4.2`）：先发它，再发 cp（或高/低代理）。
pub(super) const UNMAPPED: u16 = 0x7FFF;

/// 已装载的对照表：只持有对 `collation.bin` 字节的引用与各段 (offset, count)。
pub(super) struct Tables {
    bytes: &'static [u8],
    off: [usize; SEC_CNT],
    cnt: [usize; SEC_CNT],
}

impl Tables {
    /// 解析头部与段表，校验 magic / version / sec_cnt。
    ///
    /// 入参是 `include_bytes!` 的编译期内嵌字节 ⇒ 失败即代码/数据被改坏，
    /// 直接 panic 比静默算错更安全。真正的"防换表"由 `collation.rs` 的指纹钉子守。
    pub(super) fn parse(bytes: &'static [u8]) -> Tables {
        assert!(bytes.len() >= 16 + SEC_CNT * 8, "collation.bin 头部被截断");
        assert_eq!(u32_at_raw(bytes, 0), MAGIC, "collation.bin magic 不符");
        assert_eq!(u32_at_raw(bytes, 4), VERSION, "collation.bin version 不符");
        assert_eq!(
            u32_at_raw(bytes, 8) as usize,
            SEC_CNT,
            "collation.bin sec_cnt 不符"
        );
        let mut off = [0usize; SEC_CNT];
        let mut cnt = [0usize; SEC_CNT];
        for (i, (o, c)) in off.iter_mut().zip(cnt.iter_mut()).enumerate() {
            *o = u32_at_raw(bytes, 16 + i * 8) as usize;
            *c = u32_at_raw(bytes, 20 + i * 8) as usize;
        }
        Tables { bytes, off, cnt }
    }

    fn u8_at(&self, sec: usize, i: usize) -> u8 {
        self.bytes[self.off[sec] + i]
    }

    fn u16_at(&self, sec: usize, i: usize) -> u16 {
        let p = self.off[sec] + i * 2;
        u16::from_le_bytes([self.bytes[p], self.bytes[p + 1]])
    }

    fn u32_at(&self, sec: usize, i: usize) -> u32 {
        u32_at_raw(self.bytes, self.off[sec] + i * 4)
    }

    /// 在升序 u32 段里二分找 `key & mask`，返回元素下标。
    fn search(&self, sec: usize, key: u32, mask: u32) -> Option<usize> {
        let (mut lo, mut hi) = (0usize, self.cnt[sec]);
        while lo < hi {
            let mid = (lo + hi) / 2;
            let v = self.u32_at(sec, mid) & mask;
            if v < key {
                lo = mid + 1;
            } else if v > key {
                hi = mid;
            } else {
                return Some(mid);
            }
        }
        None
    }

    /// 段 0：CJK 主序。调用方须保证 `CJK_LO <= cp <= CJK_HI`。
    pub(super) fn cjk_primary(&self, cp: u32) -> u16 {
        self.u16_at(0, (cp - CJK_LO) as usize)
    }

    /// 段 1：非 CJK 单 CE 码点 → 主序（不存在返回 `None`；含 CJK 以外的 BMP）。
    pub(super) fn single_primary(&self, cp: u32) -> Option<u16> {
        self.search(1, cp, 0xFFFF)
            .map(|i| (self.u32_at(1, i) >> 16) as u16)
    }

    /// 段 2/3/4：规范分解 → `(pool 起始下标, 长度)`。长度为 1 也合法。
    pub(super) fn nfd_of(&self, cp: u32) -> Option<(usize, usize)> {
        self.search(2, cp, 0xFFFF_FFFF)
            .map(|i| (self.u32_at(3, i) as usize, self.u8_at(4, i) as usize))
    }

    /// 段 5：分解结果码点序列（可能 > 0xFFFF）。
    pub(super) fn nfd_pool(&self, idx: usize) -> u32 {
        self.u32_at(5, idx)
    }

    /// 段 6/7：规范组合类；查不到视为 0（即"起始字符"，会切断重排段）。
    pub(super) fn ccc(&self, cp: u32) -> u8 {
        self.search(6, cp, 0xFFFF_FFFF)
            .map_or(0, |i| self.u8_at(7, i))
    }

    /// 段 8/9/12：若 `cp` 是收缩起始字符，返回 `(键起始下标, 键数, 是否完全可忽略)`。
    pub(super) fn contract(&self, cp: u32) -> Option<(usize, usize, bool)> {
        self.search(8, cp, 0xFFFF).map(|i| {
            (
                (self.u32_at(8, i) >> 16) as usize,
                self.u8_at(9, i) as usize,
                self.u8_at(12, i) != 0,
            )
        })
    }

    /// 段 10：第 `k` 个收缩键的 `(cp0, cp1)`（键长恒为 2）。
    pub(super) fn contract_key(&self, k: usize) -> (u32, u32) {
        let v = self.u32_at(10, k);
        (v & 0xFFFF, (v >> 16) & 0xFFFF)
    }

    /// 段 11：第 `k` 个收缩键对应的主序。
    pub(super) fn contract_key_ce(&self, k: usize) -> u16 {
        self.u16_at(11, k)
    }
}

/// 无对齐前提的 little-endian u32 读（`collation.bin` 的段起始已 4 字节对齐，
/// 但这里不依赖切片对齐，避免 `unsafe`）。
fn u32_at_raw(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}

/// 进程内唯一的表实例（`include_bytes!` 内容不变 ⇒ 装载一次即可）。
pub(super) fn global(bytes: &'static [u8]) -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| Tables::parse(bytes))
}
