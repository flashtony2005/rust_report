//! M2-22 行高基准：把「计算行高 ⇒ 分页」这条因果的**观测值**钉死。
//!
//! # 存在意义
//!
//! 前 9 个夹具的**模板行高全是定值**，所以"行高算错 ⇒ 页数错"这条失效模式
//! 在基准里根本测不出来。`engine_rowheight`（M2-22）第一次让模板行高挂上表达式，
//! 于是有了可对拍的一对数。
//!
//! # ⛔ 纪律：本文件只钉**可从夹具直接读出的观测值**，不钉任何机制解释
//!
//! 具体地说，`engine_rowheight` 的 `pagination` 里 **`paper_h = 842.0`**
//! （`paper_w = 595.0`、`unit = 1`，即 A4 纵向毫米），而实际切页是 **8 行 / 9 行**。
//! 这两页的行高之和恰好是 **290 / 297**。
//!
//! ⇒ 「可用高 = 297mm，所以 290 ≤ 297 就切页」这个解释**与夹具声明矛盾**（声明的是 842）。
//! 真机制可能是边距扣减、可用区、页眉页脚扣减等，**尚未核实**。
//! 所以本文件**只断言**：
//! * `computed_row_heights` 的 17 个值、
//! * `page_count`、每页 `rows`、两页行数之和 == 计算态行数、
//! * 每页行高之和（由上面两个数组**算术推导**得到，不是复述）、
//! * 反事实「全用模板行高」的总高对比。
//!
//! **不断言**「上限是 297」「290 ≤ 297 所以切页」这类推理性因果 ——
//! 判据里的因果一旦写错，实现就会照着错的写。宁可只钉观测值。
//!
//! # 变异测试里"没失败"有两种含义
//!
//! 上轮做变异时，我把 `computed_row_heights` 的 `27.0 → 28.0`，**3 个测试变红、1 个保持绿**
//! （`pages_cover_every_calculated_row_exactly_once`）。那第 4 条**不是漏网** ——
//! 它只依赖 `pagination`，而我只变异了 `row_cells`，**它根本不在变异路径上**。
//! 两种"没失败"必须区分：
//! * ①**没被覆盖**（该断言与被变异的数据无关）⇒ 要补断言；
//! * ②**不在变异路径上**（该断言只看别的段）⇒ 精确对应，无需处理。
//!
//! 判断办法是先看那条断言**读的是哪一段**，再对照变异点。

mod common;

use common::json::{self, Json};
use common::Fixture;

/// 从 `RowCellInfo::extra` 里取出 `computed_row_heights` 并解析成 `f64` 序列。
///
/// 走的是"加载器保存原文 → 用同一个 JSON reader 读回"这条路，顺带证明
/// `extra` 里存的原文是**完整可解析**的，而不是被截断/改写过的片段。
fn computed_heights(fx: &Fixture) -> Vec<f64> {
    let raw = fx
        .row_cells
        .iter()
        .filter_map(|rc| {
            rc.extra
                .iter()
                .find(|(k, _)| k == "computed_row_heights")
                .map(|(_, v)| v.as_str())
        })
        .next()
        .expect("engine_rowheight 的 row_cells 里应带 computed_row_heights");
    json::parse(raw)
        .unwrap_or_else(|e| panic!("computed_row_heights 原文解析失败：{e}"))
        .arr()
        .iter()
        .map(Json::as_f64)
        .collect()
}

/// 模板行高（`row_cells` 按模板行序，索引 0 ↔ 模板第 1 行）。
fn template_heights(fx: &Fixture) -> Vec<f64> {
    fx.row_cells.iter().map(|rc| rc.row_height).collect()
}

/// ① 序列长度 == 计算态行数；首元素 == 模板首行高；末两元素 == 模板第 5/6 行高
/// （第 5/6 行是不参与扩展的尾部，在计算态里仍一一对应）。
#[test]
fn computed_row_heights_length_and_tail_correspond_to_the_template() {
    let fx = load_rowheight();
    let g = fx.grid().expect("engine_rowheight 有计算态网格");
    let h = computed_heights(&fx);
    let t = template_heights(&fx);

    // 非永真式守卫：序列必须存在、长度对、且**不等于**全模板行高
    // （若全等，说明属性表达式没被求值，M2-22 的前提就不成立）
    assert!(!h.is_empty(), "computed_row_heights 不该为空");
    assert_eq!(h.len() as i32, g.rows, "序列长度应等于计算态行数");
    assert!(
        h.iter().any(|v| t.iter().all(|tv| v != tv)),
        "computed_row_heights 全等于模板行高 ⇒ 属性表达式未被求值，本测试是永真式"
    );

    assert_eq!(h[0], t[0], "首元素应等于模板第 1 行行高（该行不扩展）");
    let n = h.len();
    // ⚠️ 下面两条在当前夹具里是 `10 == 10` 的**恒真**（模板 6 行行高全同 10）。
    // 它们钉的是**结构对应关系**（尾部两行不扩展、在计算态里位置对齐），**不是数值判别** ——
    // 数值区分力由上面那条"逐值钉 17 个值"和
    // `template_only_row_heights_would_change_the_page_budget`（`170 < 587`）承担。
    // 看到这里恒真**不要**误以为漏写。
    assert_eq!(h[n - 2], t[4], "末第二个应等于模板第 5 行行高");
    assert_eq!(h[n - 1], t[5], "末元素应等于模板第 6 行行高");

    // 17 个值本身即地面真值（M2-11 必须复现它）：12 个 40 是同一锚点的复制
    assert_eq!(
        h,
        vec![
            10.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 40.0, 50.0,
            27.0, 10.0, 10.0
        ],
        "computed_row_heights 逐值不符"
    );
}

/// ② 两页必须**不重不漏**地覆盖全部计算态行。
#[test]
fn pages_cover_every_calculated_row_exactly_once() {
    let fx = load_rowheight();
    let g = fx.grid().unwrap();
    let p = fx.pagination.as_ref().expect("engine_rowheight 必须有分页");

    assert_eq!(p.page_count, 2, "M2-22 基准的页数");
    let rows: Vec<i32> = p.pages.iter().map(|pg| pg.rows).collect();
    assert_eq!(rows, vec![8, 9], "逐页行数");

    let total: i32 = rows.iter().sum();
    assert_eq!(total, g.rows, "各页行数之和应等于计算态行数（不重不漏）");
    assert_eq!(p.pages.len() as i32, p.page_count, "pages 数组长度应等于 page_count");

    let nums: Vec<i32> = p.pages.iter().map(|pg| pg.page).collect();
    assert_eq!(nums, vec![1, 2], "页号应从 1 起连续");
}

/// 每页行高之和 —— 由 `computed_row_heights` 按页切分**算术推导**，
/// 不是复述夹具里的任何数字。
///
/// ⛔ 这里**不**推断切页上限：夹具声明 `paper_h = 842.0`，而两页之和是 290 / 297。
/// 「上限 297」只是与观测值自洽的一种**解释**，尚未核实，故不写进判据。
#[test]
fn per_page_height_sums_follow_from_the_computed_row_heights() {
    let fx = load_rowheight();
    let h = computed_heights(&fx);
    let p = fx.pagination.as_ref().unwrap();

    let mut sums: Vec<f64> = Vec::new();
    let mut at = 0usize;
    for pg in &p.pages {
        let n = pg.rows as usize;
        assert!(at + n <= h.len(), "第 {0} 页的行数超出了计算态行数", pg.page);
        sums.push(h[at..at + n].iter().sum());
        at += n;
    }
    assert_eq!(at, h.len(), "各页行数之和应恰好用完整个行高序列");

    assert_eq!(sums, vec![290.0, 297.0], "两页行高之和（观测值）");
    assert_eq!(sums.iter().sum::<f64>(), 587.0, "总高（观测值）");
}

/// ③ 反事实：若实现用**模板行高**（全 10）而不是计算行高，总高会从 587 掉到 170。
///
/// 本测试**只做算术自洽性断言**（"两者相差很大 ⇒ 这条失效模式有基准可测"），
/// **不**断言"产品会切成几页" —— 那是推测。
#[test]
fn template_only_row_heights_would_change_the_page_budget() {
    let fx = load_rowheight();
    let g = fx.grid().unwrap();
    let h = computed_heights(&fx);
    let t = template_heights(&fx);

    // 模板行高全同（都是 10），故反事实总高 = 行数 × 模板行高
    let template_only = t[0];
    assert!(
        t.iter().all(|v| *v == template_only),
        "engine_rowheight 的模板行高应全部相同"
    );
    let counterfactual = template_only * g.rows as f64;
    let actual: f64 = h.iter().sum();

    assert_eq!(counterfactual, 170.0, "反事实总高 = 17 × 10");
    assert_eq!(actual, 587.0, "实际总高（观测值）");
    assert!(
        counterfactual < actual,
        "全用模板行高会显著低估总高 ⇒ 『行高算错 ⇒ 页数错』这条失效模式有基准可测"
    );
}

/// 载入被测夹具的小助手（集中一处，避免每条测试重复 `load` + 报错文案）。
fn load_rowheight() -> Fixture {
    common::load("engine_rowheight")
}
