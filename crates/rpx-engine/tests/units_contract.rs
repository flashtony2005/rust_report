//! 口径契约：模板尺寸 ↔ 裸矩阵尺寸 ↔ 计算态行数 ↔ 扩展块高。
//!
//! 这一组断言**不写死任何数字**（夹具名除外）：每一列都从产品产物现推 ——
//! `.rpx` 经 `rpx-core` 解析、`.csv` 数非空行、`expected.json` 取 `template_rows` /
//! `template_cols` / `dataset.row_count` / `calculated.rows` / 扩展格的 `row_merge`。
//! 目的是让"夹具改了但测试没跟着改"这件事**不可能静默通过**。
//!
//! # 为什么需要它
//!
//! `ReportDefine2(nRow, nColumn)` 的内部是 `new Matrix(++nRow, ++nColumn)`
//! （`ReportDefine2.java:56`），而 `getRowCount()` 在**模板态**（`ReportDefine.java:359-361`）
//! 与**计算态**（`ExtCellSet.java:414-415`）都是 `cellMatrix.getRowSize() - 1`。
//!
//! | 量 | engine_sum 的值 | 是否含 0 号脚手架行 |
//! |----|------------------|---------------------|
//! | `ReportDefine2` 第一参数 / `template_rows` | 6 | 否 |
//! | `rpx_core::ReportDefine.rows`（裸矩阵） | 7 | **是** |
//! | `calculated.rows` | 8（= 6 + (3−1)×1） | 否 |
//!
//! ⇒ 从解析结果调用 [`calc_row_count`] 必须先减 1；少减一次就整体偏大 1 行，
//! 而公式本身在 6 个夹具上都"看起来对"—— 所以这条换算必须单独钉。
//!
//! 旁证实测：`rpx-cli print tests/fixtures/engine_sum.rpx` → `matrix: 7x10 (logical 6x9)`。

use std::path::PathBuf;

use rpx_engine::calc_row_count;

/// 七个**有扩展、且能算出网格**的夹具。
///
/// 两个刻意的排除：
/// * `engine_cycle` 是错误型夹具，没有 `calculated.rows`（见本文件末尾的单测）；
/// * `engine_types` 全是字面量、无扩展格 ⇒ `block_height` 为 0，
///   行数公式 `template + (rows-1)*block` 不适用（其计算态恒等于模板态 4×4）。
const FIXTURES: [&str; 7] = [
    "engine_sum",
    "engine_ext",
    "engine_expr",
    "engine_pager",
    "engine_merge",
    "engine_samemerge",
    "engine_disp",
];

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn read_fixture(name: &str, ext: &str) -> String {
    let path = fixtures_dir().join(format!("{name}.{ext}"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// 解析 `.rpx`（产品 writer 产出的真实字节）。
fn open_rpx(name: &str) -> rpx_core::ReportDefine {
    let path = fixtures_dir().join(format!("{name}.rpx"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    rpx_core::open_bytes(&bytes).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// 数据集行数 = CSV 的非空行数 − 1（首行是列名）。
fn csv_row_count(name: &str) -> i32 {
    let text = read_fixture(name, "csv");
    (text.lines().filter(|l| !l.trim().is_empty()).count() as i32) - 1
}

// =====================================================================
// 极简 JSON 取值器（测试专用）
//
// 本 crate 零依赖、没有 JSON 库，而夹具是产品 writer 产出的、形状固定且简单的 JSON，
// 所以这里手写最小实现。**形状不符一律 panic**（响亮失败），不静默返回默认值 ——
// 这条纪律比"少写几行"重要：静默放过的测试等于没有测试。
// =====================================================================

/// 取顶层 `"name": { … }` 段落的原文（字符串内的大括号不参与配平）。
fn section<'a>(text: &'a str, name: &str) -> &'a str {
    let pat = format!("\"{name}\":");
    let at = text
        .find(&pat)
        .unwrap_or_else(|| panic!("expected.json 里找不到段落 {name}"))
        + pat.len();
    let rest = &text[at..];
    let open = rest
        .find('{')
        .unwrap_or_else(|| panic!("段落 {name} 不是对象"));
    let bytes = rest.as_bytes();
    let (mut depth, mut in_str, mut esc) = (0i32, false, false);
    for (i, b) in bytes.iter().enumerate().skip(open) {
        // 只比较 ASCII 字节；UTF-8 续字节(>=0x80)不会命中任何一个分支
        match *b {
            b'"' if !esc => in_str = !in_str,
            b'\\' if in_str => esc = !esc,
            b'{' if !in_str => depth += 1,
            b'}' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    return &rest[open..=i];
                }
            }
            _ => {}
        }
        if *b != b'\\' {
            esc = false;
        }
    }
    panic!("段落 {name} 的大括号没有闭合");
}

/// 取 `"key": <整数>`。找不到 key、或值不是整数 → panic。
fn int_after(hay: &str, key: &str) -> i32 {
    let pat = format!("\"{key}\":");
    let at = hay.find(&pat).unwrap_or_else(|| panic!("找不到键 {key}")) + pat.len();
    let digits: String = hay[at..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    digits
        .parse()
        // 注意不要在这里切 `hay` 做片段展示：夹具里有中文，按字节切会踩到
        // UTF-8 边界并抛出"与取值失败无关"的 panic，掩盖真正的问题。
        .unwrap_or_else(|_| panic!("键 {key} 的值不是整数"))
}

/// 扩展块高 = 计算态里所有**扩展格**（`source.extend != 16`）的 `row_merge` 最大值。
///
/// 为什么这样推：块高就是锚点的 `row_merge`（附件 H.7.1）。
/// ⛔ **不能**从 `merged` 段推 —— `merged` 同时收录模板合并区与 `mergeSame` 的结果，
/// 二者数值可能相同而含义不同：`engine_merge` 与 `engine_samemerge` 的 `merged` 都写
/// `row_merge=2`，但前者的计算态锚点 `row_merge=2`（块高 2）、后者是 1
/// （`mergeSame` 只在分页后生效，计算态仍是 1）。用 `source` 段才分得开。
///
/// ⚠ **已知假设（未经一般性证明）**：本函数取的是"非 16 格的 `row_merge` **最大值**"，
/// 而"最大值 ≡ 纵向锚点的 `row_merge`"这一点**只在现有 6 个夹具上恰好成立** ——
/// 它们里唯一 `>1` 的扩展格恰好就是 `engine_merge` 的锚点本身。
/// 若将来出现「非锚点的扩展格，其 `row_merge` 大于真正的纵向锚点」（例如位于模板纵向
/// 合并区里的普通公式格），本函数会**高估**块高。
///
/// 好在这个假设出错的方向是**安全的**：高估会让下面的公式断言**失败**（响亮），
/// 而不是静默通过 —— 所以它不会掩盖实现缺陷，只可能在新夹具上误报。
/// 真正的一般化推导需要读扩展算法本身（`ExtCellSet.extendRow` 的包络定义），
/// 属扩展算法切片，届时替换本函数。
fn block_height(calc_section: &str) -> i32 {
    let mut best = 0;
    // ⛔ 只在 `"source":{ … }` 里扫，**不能**直接按 `"src_row":` 切全段：
    // 头图切片给每格加了 `left_head` / `top_head`，它们也是 `{"src_row":N,"src_col":N}`，
    // 按 `"src_row":` 切会把 head 引用误当 source，随后 `int_after(seg, "extend")`
    // 在 head 段里找不到 `extend` 而 panic（实测：加键后本函数整体不可用）。
    // 限定到 `"source":{` 之后，每段开头就是该 source 自己的 `src_row`，
    // 而段内其后只有 `src_col`/`row_merge`/`col_merge`/`extend`/`sub_cells`
    // （`sub_values` 子对象只有 r/c/value/type，不含 `extend`），故取到的仍是本源的值。
    for seg in calc_section.split("\"source\":{").skip(1) {
        if int_after(seg, "extend") != 16 {
            best = best.max(int_after(seg, "row_merge"));
        }
    }
    assert!(best > 0, "计算态里没有扩展格（source.extend != 16）");
    best
}

// =====================================================================
// 断言
// =====================================================================

/// 裸矩阵尺寸必须恰好比模板尺寸多 1（0 号脚手架行/列）。
#[test]
fn parsed_matrix_dims_are_one_more_than_the_template_dims() {
    for name in FIXTURES {
        let json = read_fixture(name, "expected.json");
        let template_rows = int_after(&json, "template_rows");
        let template_cols = int_after(&json, "template_cols");
        let define = open_rpx(name);
        assert_eq!(
            define.rows as i32,
            template_rows + 1,
            "{name}: ReportDefine.rows(裸矩阵) 应 = 模板行数 + 1"
        );
        assert_eq!(
            define.cols as i32,
            template_cols + 1,
            "{name}: ReportDefine.cols(裸矩阵) 应 = 模板列数 + 1"
        );
    }
}

/// 行数公式成立 —— 四列输入全部现推：`.rpx` 出模板尺寸、`.csv` 出数据集行数、
/// `dataset.row_count` 交叉校验 csv、计算态 `source.row_merge` 出块高。
#[test]
fn row_count_formula_is_derived_from_product_artifacts() {
    for name in FIXTURES {
        let json = read_fixture(name, "expected.json");
        let calc = section(&json, "calculated");
        let dataset = section(&json, "dataset");

        let template_rows = int_after(&json, "template_rows");
        let calc_rows = int_after(calc, "rows");
        let block = block_height(calc);

        // 数据集行数：csv 与 expected.json 必须一致（两者都是产品产出的）
        let dataset_rows = csv_row_count(name);
        assert_eq!(
            dataset_rows,
            int_after(dataset, "row_count"),
            "{name}: csv 行数 vs expected.json 的 dataset.row_count"
        );

        // 模板行数与 .rpx 对齐（再钉一次，防止 json 与 rpx 不同步）
        assert_eq!(
            open_rpx(name).rows as i32 - 1,
            template_rows,
            "{name}: 模板行数口径"
        );

        assert_eq!(
            calc_row_count(template_rows, dataset_rows, block),
            calc_rows,
            "{name}: calc = {template_rows} + ({dataset_rows}-1)*{block}"
        );
    }
}

/// 块高这个"间接推出来的输入"本身也要有直接证据：`engine_merge` 的锚点带模板合并区
/// （`A2` 合并 `A2:A3`，附件 H.7.1）⇒ 块高 2；其余 5 个夹具块高 1。
#[test]
fn block_height_reflects_template_merges_not_merge_same() {
    let merge = read_fixture("engine_merge", "expected.json");
    assert_eq!(
        block_height(section(&merge, "calculated")),
        2,
        "engine_merge"
    );

    // engine_samemerge 的 `merged` 段写 row_merge=2（mergeSame 的结果），
    // 但计算态锚点仍是 1 —— 两者必须分得开，这正是不能从 merged 推块高的原因
    let same = read_fixture("engine_samemerge", "expected.json");
    assert_eq!(
        block_height(section(&same, "calculated")),
        1,
        "engine_samemerge"
    );
    assert!(
        section(&same, "merged").contains("\"row_merge\":2"),
        "engine_samemerge 的 merged 段应含 row_merge=2（分页后才有）"
    );

    for name in [
        "engine_sum",
        "engine_ext",
        "engine_expr",
        "engine_pager",
        "engine_disp",
    ] {
        let json = read_fixture(name, "expected.json");
        assert_eq!(block_height(section(&json, "calculated")), 1, "{name}");
    }
}

/// 错误型夹具的形状（spec §7）：`calculated` 只有 `error`，`pagination` / `merged` 为 `null`。
/// 这也是为什么 `engine_cycle` 不在 [`FIXTURES`] 里。
#[test]
fn the_error_fixture_has_no_grid() {
    let json = read_fixture("engine_cycle", "expected.json");
    let calc = section(&json, "calculated");
    assert!(
        calc.contains("\"error\""),
        "engine_cycle 的 calculated 应是 error 对象"
    );
    assert!(!calc.contains("\"rows\""), "错误型夹具不该有网格尺寸");
    assert!(calc.contains("\"chain\""), "错误型夹具必须带引用链");
    assert!(
        json.contains("\"pagination\":null"),
        "错误型夹具的 pagination 应为 null"
    );
    assert!(
        json.contains("\"merged\":null"),
        "错误型夹具的 merged 应为 null"
    );
}
