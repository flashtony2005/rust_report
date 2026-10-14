//! M2-12 `Text-Text` 字典序的 Rust 金标准对拍（`docs/m2-collation.md` §4 算法的判据）。
//!
//! 唯一事实源是产品实测产物 `tests/fixtures/_collation_*.tsv`（生成器
//! `GenCollationData.java` / `GenCollationGolden.java`），**禁止手搓期望**。
//!
//! 五条覆盖面（对应任务书 §五）：
//! 1. `_collation_dump.tsv`：全 BMP(63,488) + 增补(384) 的 CE 流**逐点**比对（最强）；
//! 2. `_collation_golden.tsv`：3200 条 SORT 复现产品次序 + 58 条 PAIR 符号逐一相等；
//! 3. `_collation_nfd_golden.tsv`：NFD 逐行复现（验 Hangul 算法分解与 ccc 重排）；
//! 4. `_collation_contract_golden.tsv`：54 个收缩键 + 12 个可忽略码点的截断行为；
//! 5. 参照值 + "码点序与产品序相反"的钉子。
//!
//! 另加一条**口径钉子**（`surrogate_pair_and_lone_surrogates_are_distinct_paths`）：
//! 代理对与孤立代理是两条不同路径 —— 口径辨析见 [`decode_field`] 的文档。
//!
//! 夹具目录口径与 `tests/engine_grid_compare.rs:34-36` 一致。

use rpx_engine::collation::{compare_cps, compare_text, nfd, primary_seq, primary_seq_cps};
use std::cmp::Ordering;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn read(name: &str) -> String {
    let p = fixtures_dir().join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("读 {} 失败：{e}", p.display()))
}

/// 空格分隔的码点 hex → 码点序列（允许 `%04X` 以上宽度，如 `10546`/`1F600`）。
fn hex_cps(s: &str) -> Vec<u32> {
    s.split_whitespace()
        .map(|t| u32::from_str_radix(t, 16).unwrap_or_else(|_| panic!("坏 hex token [{t}]")))
        .collect()
}

/// 码点序列 → Rust `String`（仅当全部为标量值；孤立代理由 [`decode_field`] 分流到 `Codes`）。
fn cps_to_string(cps: &[u32]) -> String {
    cps.iter()
        .map(|&c| {
            char::from_u32(c)
                .unwrap_or_else(|| panic!("码点 U+{c:X} 不是 Unicode 标量值，不能进 &str"))
        })
        .collect()
}

/// 一个 PAIR 操作数的两种承载形态。
enum Operand {
    /// 全部是 Unicode 标量值 ⇒ 与 Java 的 `String` 完全同路，走 `compare_text`。
    Text(String),
    /// 含**真正的孤立代理**码点（`D800..=DFFF`）⇒ `&str` 装不下，走码点版 `compare_cps`。
    Codes(Vec<u32>),
}

/// hex 列 → 操作数。
///
/// # 口径（必读 —— 本文件最易踩错的一处，且"踩了也不一定红"）
///
/// `_collation_golden.tsv` 的 hex 列是**码点**（`String.codePointAt` 口径），**不是** UTF-16 单元：
/// 写出侧 `GenCollationGolden.java` 的 `hex()` 用 `s.codePointAt(i)` + `Character.charCount(cp)`
/// 逐**码点**输出，所以一个**合法代理对会被折叠成单个码点** —— PAIRS 里的
/// `{"D800 DC00", "E000"}`（`GenCollationGolden.java:142`）落盘就是 `PAIR\t10000\tE000\t-1`
/// （该文件第 3230 行，`10000` 是 **U+10000**）。因此 hex 字段里**不会**出现 `D800 DC00` 这种
/// "一个字段里写代理对"的写法，每个 token 直接按码点解析就是对的。
///
/// 反过来说，"按 UTF-16 单元读"对本文件**不可行**：增补 token（`10000`/`20000`/`1F600`/
/// `2F900`/`10546`…）都 **> 0xFFFF**，根本装不进 `u16`。
///
/// 真正需要 `compare_cps` 回落的只有**孤立代理**：`PAIR\tD800\tDC00\t-1`（第 3236 行）的
/// hexA=`D800` 与 hexB=`DC00` 是**两个字段、各含一个孤立代理码点**（**不是**"一个字段里的
/// `D800 DC00` 代理对"）；同族还有第 3237/3238 行。产品对孤立代理发 `[7FFF, cp]`
/// （实测 `CE(U+D800)=[7FFF,D800]`，`docs/m2-collation.md` §5.4），`compare_cps` 正好复现。
///
/// ⛔ **反例警告**：若把合法代理对误当两个孤立代理喂进 `compare_cps`，`U+10000` 会被算成
/// `[7FFF,D800,7FFF,DC00]`（**4** 元素）而不是 `[7FFF,D800,DC00]`（**3** 元素）—— 语义不同，
/// 但在本夹具上**比较符号恰好一致**，所以"测试全绿"证明不了没踩这个坑。
/// 下方 `surrogate_pair_and_lone_surrogates_are_distinct_paths` 用正面断言把这条钉死。
fn decode_field(hex: &str) -> Operand {
    let cps = hex_cps(hex);
    if cps.iter().all(|&c| char::from_u32(c).is_some()) {
        Operand::Text(cps_to_string(&cps))
    } else {
        Operand::Codes(cps)
    }
}

/// 操作数 → 码点序列（仅在"两侧不全为标量"时被调用，用于 `compare_cps` 兜底）。
fn as_cps(o: &Operand) -> Vec<u32> {
    match o {
        Operand::Text(s) => s.chars().map(|c| c as u32).collect(),
        Operand::Codes(v) => v.clone(),
    }
}

/// 比较两个操作数：全标量走 `compare_text`（与 Java 同路），否则走码点版。
fn compare_operands(a: &Operand, b: &Operand) -> Ordering {
    if let (Operand::Text(x), Operand::Text(y)) = (a, b) {
        return compare_text(x, y);
    }
    compare_cps(&as_cps(a), &as_cps(b))
}

/// 便于断言失败时定位操作数。
fn show(o: &Operand) -> String {
    match o {
        Operand::Text(s) => format!("text {s:?}"),
        Operand::Codes(v) => format!("codes {v:X?}"),
    }
}

/// `Ordering` → `-1/0/1`。
fn sign(o: Ordering) -> i32 {
    match o {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

// ═════════════════════ 1. dump：全量 CE 流逐点对拍 ═════════════════════

/// 对全部 63,488 个 BMP 码点与 384 个增补抽样，逐点比对 `primary_seq_cps` 与产品 dump。
///
/// 先断言每个 CE 的 secondary/tertiary 恒为 0（`docs/m2-collation.md` §5.2），
/// 再逐元素比 primary。**不抽样** —— 这是本移植最强的一条判据。
///
/// 口径：本文件 key 是**码点**（增补为 6 位 hex，如 `010000`），值是 `primary/secondary/tertiary`
/// 三元组，**与 UTF-16 单元无关** ⇒ 按码点读是对的（不要改成 UTF-16 口径）。
#[test]
fn dump_ce_streams_match_for_every_dumped_code_point() {
    let text = read("_collation_dump.tsv");
    let (mut checked, mut mismatched, mut sup) = (0usize, 0usize, 0usize);
    let mut first = String::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let (cph, ce) = line.split_once('\t').unwrap_or((line, ""));
        let cp = u32::from_str_radix(cph, 16).unwrap_or_else(|_| panic!("坏 cp [{cph}]"));
        if cp > 0xFFFF {
            sup += 1;
        }
        checked += 1;

        let mut want: Vec<u32> = Vec::new();
        if !ce.is_empty() {
            for triple in ce.split(',') {
                let f: Vec<&str> = triple.split('/').collect();
                assert_eq!(f.len(), 3, "CE 三元组格式异常：{triple}");
                assert_eq!(f[1], "0000", "U+{cph} 的 secondary 非 0：{triple}");
                assert_eq!(f[2], "0000", "U+{cph} 的 tertiary 非 0：{triple}");
                want.push(u32::from_str_radix(f[0], 16).unwrap());
            }
        }
        let got: Vec<u32> = primary_seq_cps(&[cp]).into_iter().map(u32::from).collect();
        if got != want {
            mismatched += 1;
            if first.is_empty() {
                first = format!("U+{cph} got=[{got:?}] want=[{want:?}]");
            }
        }
    }
    assert_eq!(checked, 63_872, "dump 行数应为 63,488 BMP + 384 增补");
    assert_eq!(sup, 384, "增补平面抽样应恰 384");
    assert_eq!(mismatched, 0, "CE 流不符（首例）：{first}");
}

// ═════════════════════ 2. golden：SORT 次序 + PAIR 符号 ═════════════════════

/// SORT：按本实现稳定排序后必须**逐条复现**产品次序；任取 i<j 不得为 Greater，
/// 同序组内按码点升序（生成时用码点预排序 + 稳定排序保证）。
/// PAIR：`sign(compare)` 逐一等于给定符号（hex 是码点口径，真正孤立代理才回落 `compare_cps`，
/// 见 [`decode_field`]）。
#[test]
fn golden_sort_and_pair_reproduce_product_order() {
    let text = read("_collation_golden.tsv");
    let mut sorts: Vec<String> = Vec::new();
    let mut pairs: Vec<(Operand, Operand, i32)> = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.splitn(4, '\t').collect();
        match f[0] {
            "SORT" => sorts.push(cps_to_string(&hex_cps(f[2]))),
            "PAIR" => pairs.push((
                decode_field(f[1]),
                decode_field(f[2]),
                f[3].parse().expect("PAIR 符号应为 -1/0/1"),
            )),
            other => panic!("未知行类型 [{other}]：{line}"),
        }
    }
    assert_eq!(sorts.len(), 3200, "SORT 条数");
    assert_eq!(pairs.len(), 58, "PAIR 条数");

    // ① 稳定排序后逐条复现
    let mut sorted = sorts.clone();
    sorted.sort_by(|a, b| compare_text(a, b));
    assert_eq!(sorted, sorts, "SORT 段未复现产品次序（compare_text 有误）");

    // ② 任取 i<j：不得为 Greater；Equal 组内码点非降序
    let seqs: Vec<Vec<u16>> = sorts.iter().map(|s| primary_seq(s)).collect();
    let cpsq: Vec<Vec<u32>> = sorts
        .iter()
        .map(|s| s.chars().map(|c| c as u32).collect())
        .collect();
    for (i, si) in seqs.iter().enumerate() {
        for (j, sj) in seqs.iter().enumerate().skip(i + 1) {
            match si.cmp(sj) {
                Ordering::Greater => panic!("SORT[{i}] > SORT[{j}]：次序被违反"),
                Ordering::Equal => {
                    assert!(cpsq[i] <= cpsq[j], "同序组 [{i},{j}] 未按码点升序")
                }
                Ordering::Less => {}
            }
        }
    }

    // ③ PAIR：符号逐一相等
    for (a, b, want) in &pairs {
        assert_eq!(
            sign(compare_operands(a, b)),
            *want,
            "PAIR {} vs {} 符号不符",
            show(a),
            show(b)
        );
    }
}

// ═════════════════════ 3. NFD 逐行对拍 ═════════════════════

/// `<输入码点 hex>\t<JRE NFD 码点 hex>` 逐行复现（Hangul 算法分解 + ccc 重排）。
///
/// 口径（已核生成器）：两侧 hex 都是**码点** —— 输入取自 `CollationData.hex(cp)`（增补用 `%06X`）
/// 与字面序列（`CollationGoldens.writeNfdGolden`），期望取自 `cpsOf(...)`（`codePointAt` 口径）
/// ⇒ 按码点读是对的。
#[test]
fn nfd_reproduces_the_jre_normalization() {
    let text = read("_collation_nfd_golden.tsv");
    let mut checked = 0usize;
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let (ins, outs) = line
            .split_once('\t')
            .unwrap_or_else(|| panic!("无制表符：{line}"));
        let want = hex_cps(outs);
        let got = nfd(&hex_cps(ins));
        assert_eq!(got, want, "NFD 不符：输入 [{ins}]");
        checked += 1;
    }
    assert!(checked >= 2000, "NFD golden 行数异常：{checked}");
}

// ═════════════════════ 4. 收缩键 + 可忽略截断 ═════════════════════

/// 54 个收缩键（含命中时的 primary）与 12 个可忽略码点的**整串截断**行为。
///
/// 口径（已核生成器）：hex 是**码点**（`CollationGoldens.writeContractGolden` 的 `%04X`），
/// 与 UTF-16 单元无关。
#[test]
fn contraction_keys_and_ignorable_truncation_match() {
    let text = read("_collation_contract_golden.tsv");
    let (mut keys, mut ig) = (0usize, 0usize);
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        if let Some(h) = line.strip_prefix("IGNORABLE ") {
            let cp = u32::from_str_radix(h.trim(), 16).unwrap();
            let s = char::from_u32(cp).unwrap().to_string() + "A";
            assert!(
                primary_seq(&s).is_empty(),
                "可忽略 U+{cp:04X} 未截断整串：{:?}",
                primary_seq(&s)
            );
            ig += 1;
            continue;
        }
        let (k, p) = line
            .split_once('\t')
            .unwrap_or_else(|| panic!("坏行 {line}"));
        let kc = hex_cps(k);
        assert_eq!(kc.len(), 2, "收缩键长应恒为 2：{k}");
        let want = u32::from_str_radix(p.trim(), 16).unwrap() as u16;
        assert_eq!(primary_seq_cps(&kc), vec![want], "收缩键 {k} 的主序不符");
        keys += 1;
    }
    assert_eq!(keys, 54, "收缩键数");
    assert_eq!(ig, 12, "可忽略码点数");
    // 任务书点名的一条：可忽略起始字符 + A ⇒ 空
    assert!(primary_seq("\u{301}A").is_empty());
}

// ═════════════════════ 5. 参照值 + 码点序相反钉子 ═════════════════════

/// 产品公开参照主序 + "码点序与产品序相反"的钉子。
#[test]
fn published_reference_primaries_and_code_point_disagreement() {
    let refs: [(&str, u16); 7] = [
        ("\u{963F}", 294),   // 阿
        ("\u{767D}", 555),   // 白
        ("\u{9648}", 1854),  // 陈
        ("\u{4E01}", 3322),  // 丁
        ("\u{4E00}", 18183), // 一
        ("\u{5F20}", 19926), // 张
        ("\u{8D75}", 20001), // 赵
    ];
    for (ch, p) in refs {
        assert_eq!(primary_seq(ch), vec![p], "{ch} 的主序");
    }
    // 码点 一(U+4E00) < 丁(U+4E01)，但产品序 丁(3322) < 一(18183) ⇒ 比较为 Greater。
    assert_eq!(compare_text("一", "丁"), Ordering::Greater);
    assert_eq!(compare_text("丁", "一"), Ordering::Less);
}

// ═════════════════════ 6. 代理对 vs 孤立代理：口径钉子（正面证据） ═════════════════════

/// 正面钉死"合法代理对"与"两个孤立代理"是**两条不同路径**，且金标准的 hex 是**码点**口径。
///
/// 为什么需要它：把代理对误当两个孤立代理喂进 `compare_cps` 会算错（4 元素 vs 3 元素），
/// 但两种读法在现有夹具上**比较符号恰好一致** ⇒ "全绿"不能当通过证据。故这里直接断言序列本身。
#[test]
fn surrogate_pair_and_lone_surrogates_are_distinct_paths() {
    // ① 合法代理对 ≡ 单个标量 U+10000：同一个东西，走同一条 `compare_text` 路径。
    let pair = String::from_utf16(&[0xD800, 0xDC00]).expect("D800 DC00 是合法代理对");
    assert_eq!(pair, "\u{10000}");
    assert_eq!(compare_text(&pair, "\u{10000}"), Ordering::Equal);
    assert_eq!(primary_seq(&pair), primary_seq("\u{10000}"));
    assert!(matches!(decode_field("10000"), Operand::Text(_)));

    // ② U+10000 → [7FFF, 高代理, 低代理] —— **3 个元素**。
    //    cp=0x10000 ⇒ c=0 ⇒ hi=0xD800+0=0xD800、lo=0xDC00+0=0xDC00
    //    （与 dump 行 `010000  7FFF/0000/0000,D800/0000/0000,DC00/0000/0000` 一致）。
    assert_eq!(primary_seq_cps(&[0x10000]), vec![0x7FFF, 0xD800, 0xDC00]);
    assert_eq!(primary_seq_cps(&[0x10000]).len(), 3);

    // ③ 反例：把同一对当**两个孤立代理** ⇒ 4 个元素，与 ② 语义不同。
    let as_lone = primary_seq_cps(&[0xD800, 0xDC00]);
    assert_eq!(as_lone, vec![0x7FFF, 0xD800, 0x7FFF, 0xDC00]);
    assert_eq!(as_lone.len(), 4);
    assert_ne!(
        primary_seq_cps(&[0x10000]),
        as_lone,
        "代理对与孤立代理序列必须不同 —— 这正是'符号恰好一致但语义不同'要防的坑"
    );

    // ④ 金标准第 3236 行 `PAIR D800 DC00 -1` 的 hexA/hexB 各是**一个孤立代理码点**。
    assert_eq!(primary_seq_cps(&[0xD800]), vec![0x7FFF, 0xD800]);
    assert_eq!(primary_seq_cps(&[0xDC00]), vec![0x7FFF, 0xDC00]);
    assert_eq!(compare_cps(&[0xD800], &[0xDC00]), Ordering::Less);

    // ⑤ 解析函数的分流：>0xFFFF 的 token 必须走码点（`Text`），孤立代理才走 `Codes`。
    assert!(matches!(decode_field("D800"), Operand::Codes(_)));
    assert!(matches!(decode_field("D800 D800"), Operand::Codes(_)));
    assert!(matches!(decode_field("0061 0062"), Operand::Text(_)));
    assert!(matches!(decode_field("1F600"), Operand::Text(_)));
    // 若把 hex 当 UTF-16 单元，"10000" 会溢出 u16 —— 这里顺带钉住"只能按码点读"。
    assert!(u16::try_from(0x10000u32).is_err());
}
