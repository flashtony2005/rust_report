#!/usr/bin/env python3
"""从 PrintSetup.java 的两个 switch 生成 `pager/paper.rs`（禁手抄）。

事实源（reportlite）：
  * getPaperHeight(int paper, float defaultHeight) —— PrintSetup.java:437-676
  * getPaperWidth (int paper, float defaultWidth)  —— PrintSetup.java:677-916
  * 无匹配分支 ⇒ return defaultWidth/defaultHeight（自定义纸）。

用法（**文件路径一律走 argv** —— 沙箱对内联代码的文件访问有限制）：
  python3 scripts/gen_paper_table.py <PrintSetup.java> <输出 paper.rs>

输出 = 模块文档 + 纸码表（两个 match）+ usable_page_pt + 单测。
表字面量保留 Java 的 double 展开形态（float 常量的精确值）。
"""
import re
import sys


def parse_switch(src: str, fn_name: str) -> dict:
    """取 `fn_name` 方法体里的 `case N: ... return X;` → {code: literal}。"""
    m = re.search(rf"public static double {fn_name}\(.*?\)\s*\{{", src)
    if not m:
        sys.exit(f"找不到 {fn_name}")
    i, depth, body = m.end(), 1, []
    while depth > 0:
        ch = src[i]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                break
        body.append(ch)
        i += 1
    text = "".join(body)
    table = {}
    for cm in re.finditer(r"case\s+(\d+)\s*:", text):
        code = int(cm.group(1))
        rm = re.search(r"return\s+([0-9.eE+-]+)\s*;", text[cm.end():])
        if not rm:
            sys.exit(f"case {code} 后找不到 return")
        lit = rm.group(1)
        if code in table and table[code] != lit:
            sys.exit(f"纸码 {code} 有两个不同的值：{table[code]} / {lit}")
        table[code] = lit
    return table


def emit(name: str, table: dict) -> str:
    arms = "\n".join(f"        {c} => Some({table[c]})," for c in sorted(table))
    return (
        f"/// 纸码 → 高/宽（mm）查找表（`{name}` 的 Rust 形态）。\n"
        f"/// 无该码 ⇒ `None`（调用方兜底自定义字段）。\n"
        f"fn {name}(paper: i16) -> Option<f64> {{\n"
        f"    match paper {{\n{arms}\n        _ => None,\n    }}\n}}"
    )


HEADER = '''//! 纸张码表与**模板真值口径**的可用页尺寸（M3-06）。
//!
//! # 与探针口径的分工
//!
//! 分页算法（[`super::split_rows`] / [`super::split_cols`]）只有一份；差在**页尺寸**：
//!
//! | 口径 | 页尺寸来源 | 消费者 |
//! | --- | --- | --- |
//! | 探针（M2-11 对拍） | [`super::PAPER_W_PT`] 常量 595×842pt、零边距 | `calculate()` → 夹具 `pagination` 段 |
//! | 模板真值（M3-06） | 本模块：纸码表 + 模板边距 | `/render` 快照 `pagination` 块 |
//!
//! 探针路径之所以长那样，是产品探针自己覆盖的：
//! `ReportUtils2.getPagerInfo` 把纸张改成传入的 595×842（pt→mm，
//! `setPaperSize` 同时把纸码置 **256=自定义**，`:74`/`:118-121`）并**清零四边边距**
//! （`:75-82`）。模板真值路径 = `new PagerInfo(PrintSetup)` 不覆盖
//! （`PagerInfo.java:43-68`）⇒ 纸码表 + 模板边距原样进断页。
//!
//! # 算术（`PagerInfo.getPageFormat(true)`，`:269-278`）
//!
//! ```text
//! paper_mm = 纸码表(paper) 或自定义字段（表中无该码，switch 默认分支）
//! usable_pt = (paper_mm - 边距) * 72.0 / 25.4      // mmToPixel，`:1150-1152`
//! ```
//!
//! `isHighPrecision` 恒 true（`ReportDefine._$16 = true`，`:75`）⇒ double 运算、
//! **不取整**（`getPageWidth(true)` 分支，`:241-244`）。与探针路径的
//! [`super::arith::paper_size_pt`] 是两套算术：探针把 pt 转成 mm 存进 float32
//! 字段再转回（有 842→841.99997… 的舍入），本路径的值**本来就以 mm 为单位**
//! （模板槽 9/10 是 f32 mm）⇒ 不做那次往返。
//!
//! # orient 不换宽高（faithful port）
//!
//! `PagerInfo.getPageFormat` 只把 orient 写进 `PageFormat.setOrientation`，
//! **不**交换 imageable 宽高（对照 `PrintSetup.java:920-935` 那个会把边距
//! 轮换的方法——SimplePager 走的是前者）。故本模块同样不换。
//!
//! # 表的来历
//!
//! `scripts/gen_paper_table.py` 从 `PrintSetup.java` 的两个 switch 生成
//! （height `:437-676` / width `:677-916`），**禁手抄**；字面量保留 Java 的
//! double 展开形态（float 常量的精确值）。

use rpx_core::print_setup::PrintSetup;

/// `PrintSetup.getPaperWidth(int paper, float defaultWidth)`：纸码 → 宽（mm）。
/// 表中无该码 ⇒ 自定义字段（Java switch 的默认分支，`:914-916`）。
pub fn paper_width_mm(paper: i16, custom_width: f32) -> f64 {
    paper_width_mm_table(paper).unwrap_or(f64::from(custom_width))
}

/// `PrintSetup.getPaperHeight(int paper, float defaultHeight)`：纸码 → 高（mm）。
pub fn paper_height_mm(paper: i16, custom_height: f32) -> f64 {
    paper_height_mm_table(paper).unwrap_or(f64::from(custom_height))
}

/// 模板真值口径的**可用页尺寸**（pt）：`PagerInfo.getPageFormat(true)` 的
/// `getImageableWidth/Height`（高精度 double，不取整）。
///
/// 即 `(纸mm − 左右边距) × 72/25.4` 与 `(纸mm − 上下边距) × 72/25.4`。
pub fn usable_page_pt(ps: &PrintSetup) -> (f64, f64) {
    let w_mm = paper_width_mm(ps.paper, ps.paper_width);
    let h_mm = paper_height_mm(ps.paper, ps.paper_height);
    let usable_w = (w_mm - f64::from(ps.left_margin) - f64::from(ps.right_margin)) * 72.0 / 25.4;
    let usable_h = (h_mm - f64::from(ps.top_margin) - f64::from(ps.bottom_margin)) * 72.0 / 25.4;
    (usable_w, usable_h)
}
'''

TESTS = '''

#[cfg(test)]
mod tests {
    use super::*;

    /// 附件 D 默认 setup（paper=9 A4、边距 19/19/25/25）。
    fn default_setup() -> PrintSetup {
        PrintSetup::default_values()
    }

    #[test]
    fn paper_codes_hit_the_table() {
        // A4（9）= 210×297；Letter（1）= 215.9×279.4；A3（8）= 297×420；
        // B3（9999）= 353×500。值与 PrintSetup.java 两个 switch 逐字一致。
        assert_eq!(paper_width_mm(9, 0.0), 210.0);
        assert_eq!(paper_height_mm(9, 0.0), 297.0);
        assert_eq!(paper_width_mm(1, 0.0), 215.89999999999998);
        assert_eq!(paper_height_mm(1, 0.0), 279.4);
        assert_eq!(paper_width_mm(8, 0.0), 297.0);
        assert_eq!(paper_height_mm(8, 0.0), 420.0);
        assert_eq!(paper_width_mm(9999, 0.0), 353.0);
        assert_eq!(paper_height_mm(9999, 0.0), 500.0);
    }

    #[test]
    fn unknown_code_falls_back_to_custom_fields() {
        // 256（探针 setPaperSize 置的自定义码）与任何未列出的码都走自定义字段。
        assert_eq!(paper_width_mm(256, 209.9), 209.9);
        assert_eq!(paper_height_mm(256, 297.0), 297.0);
        assert_eq!(paper_width_mm(-1, 100.0), 100.0);
        // 自定义字段为 0 时不兜底成 0 以外的值——Java 原样返回 defaultWidth。
        assert_eq!(paper_width_mm(256, 0.0), 0.0);
    }

    #[test]
    fn usable_page_subtracts_margins_in_pt() {
        let ps = default_setup();
        let (w, h) = usable_page_pt(&ps);
        // A4 210×297 − 边距 19/19/25/25 ⇒ 172×247mm ⇒ ×72/25.4。
        let expect_w = 172.0 * 72.0 / 25.4;
        let expect_h = 247.0 * 72.0 / 25.4;
        assert!((w - expect_w).abs() < 1e-12, "w={w}");
        assert!((h - expect_h).abs() < 1e-12, "h={h}");
        // 与探针口径（595×842 零边距）显然不同——正是 M3-06 要的差别。
        assert!(w < 595.0 && h < 842.0);
    }

    #[test]
    fn custom_paper_uses_custom_fields_with_margins() {
        let mut ps = default_setup();
        ps.paper = 256; // 自定义
        ps.paper_width = 100.0;
        ps.paper_height = 150.0;
        ps.left_margin = 10.0;
        ps.right_margin = 10.0;
        ps.top_margin = 5.0;
        ps.bottom_margin = 5.0;
        let (w, h) = usable_page_pt(&ps);
        assert!((w - 80.0 * 72.0 / 25.4).abs() < 1e-12, "w={w}");
        assert!((h - 140.0 * 72.0 / 25.4).abs() < 1e-12, "h={h}");
    }
}
'''


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    src = open(sys.argv[1], encoding="utf-8").read()
    heights = parse_switch(src, "getPaperHeight")
    widths = parse_switch(src, "getPaperWidth")
    # 两表同码不同值是正常的（宽≠高）；只有同码同表多值才算解析错（上面已拦）。
    only_h = sorted(set(heights) - set(widths))
    only_w = sorted(set(widths) - set(heights))
    print(f"height {len(heights)} 项 / width {len(widths)} 项", file=sys.stderr)
    print(f"仅 height 有的码：{only_h}", file=sys.stderr)
    print(f"仅 width 有的码：{only_w}", file=sys.stderr)
    body = emit("paper_height_mm_table", heights) + "\n\n" + emit(
        "paper_width_mm_table", widths
    )
    out = HEADER + "\n" + body + TESTS
    open(sys.argv[2], "w", encoding="utf-8", newline="\n").write(out)
    print(f"written {sys.argv[2]}: {len(out.splitlines())} lines", file=sys.stderr)


if __name__ == "__main__":
    main()
