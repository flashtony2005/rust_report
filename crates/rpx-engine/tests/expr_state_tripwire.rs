//! ⛔ **前提钉子（夹具级）**：15 个引擎夹具里的**全部 90 条公式**，`get_state() == 4`。
//!
//! # 为什么这条前提必须被钉住
//!
//! 顶层 `calculate()` 的调度状态机靠 `engine.state` 与当前轮次比较来换格
//! （`ExtNormalCell.java:636-639`：`engine.state > cs.getState() ⇒ 抛挂起`）。
//! `engine.state` 的来源是**表达式的 `getState()`**（`checkExtend:508`）。
//!
//! 而 `getState()` 在本 crate 支持的表达式上**结构性地恒等于 4**：
//! `Node` 默认 4（`Node.java:107-109`），`Function`/`Operator`/`SpecialFunction`
//! 三层都只是「取大 + 下限 4」，叶子（`Constant`/`CSVariable`/`DSVariable`/
//! `UnknownSymbol`）全都 `extends Node` 且**未覆写**。
//! 只有显式覆写 `getState` 的函数（`page()` / `cellRowNo()` / `cellColNo()` /
//! `cellCount()` / `cellSum()` / `sequence()` / `mcOrder()` / `getCell()` /
//! `CalcGraph` / `CalcSubGantt` / `CalcCustomGraph` / `CalcSubRpt`）才会产出 > 4。
//!
//! ⇒ **state 5 / 6 / 8 的挂起路径在现有夹具上恒不可达**：
//! 通道 5（`_$2(ctx,special,st):1366`）的两条特殊支（special 入队、级联失效）
//! 与通道 6（`_$1(ctx,special,st):1428`）整条，都不会被夹具触发。
//!
//! # 这条钉子守的是什么
//!
//! 不是"测试通过"，而是**一个实现的合法性边界**：
//! 只要这条前提成立，「通道 5/6 暂时只实现 chase 分支」是**等价**的；
//! 一旦有人往夹具里加了 `page()` 之类的公式，这里**先红**，
//! 于是"state 8 的级联失效还没实现"就不可能被静默放过。
//!
//! ⚠ 与 `tests/fixture_loader.rs` 的 SHAPE 表同一立场：数字是**产品产物里真实存在的**，
//! 夹具重生成导致它变化时，本文件必须响亮失败，不允许静默漂移。

mod common;

use common::{all_names, load};

/// 15 个引擎夹具的公式总数（= `EngineModels*.java` 里写进 `.rpx` 的表达式条数）。
/// 更新它必须同时说明"新加了什么形态、`get_state()` 为什么仍是 4"。
///
/// 2026-10-14：`engine_avgrange` 加入 4 条 `sum/avg/max/min(B2:B4)` 区域形态
/// （M2-12），均为「格引用 + 聚合调用」，无 DS 函数/无挂起 ⇒ `get_state()==4`，
/// 故 40 → 44。
///
/// 2026-10-14（M2-12 Text-Text）：`engine_collate` 加入 34 条 —— 文本 `>`/`<`/`>=`
/// `/`<=` 比较（16 条）、全可忽略字符 U+0301 的截断（6 条）、收缩键 + 规范
/// 重排（6 条）、由 `["…"]` 值列表驱动的文本 `max`/`min`（4 条）+ 两条 `==`
/// 精确相等（2 条）。全部是 Binary/RegularCall ⇒ 全部 `get_state()==4`，
/// 故 44 → 78。
///
/// 2026-10-14（M2-12 List 逐格）：`engine_listrange` 加入 12 条 —— `sum/avg/max/min`
/// 的三种 cellSet 形态各 4 条（`[B2:B4]` ValueList 逐格、`(B2:B4)` Link 逐格、
/// `B2:B4` 单格 else 对照）。`@` 是 `CSCurrent extends Node`（未覆写 `getState`）、
/// `:` 是 `operator.Link extends Function` ⇒ 仍全 4，故 78 → 90。
const TOTAL_FORMULAS: usize = 90;

#[test]
fn every_fixture_formula_is_state_four() {
    let mut total = 0usize;
    for name in all_names() {
        let fx = load(name);
        for f in &fx.formulas {
            let e = rpx_engine::parse(&f.exp).unwrap_or_else(|err| {
                panic!("{name} ({},{}) 的 `{}` 解析失败：{err}", f.r, f.c, f.exp)
            });
            assert_eq!(
                e.get_state(),
                4,
                "{name} 模板 ({},{}) 的 `{}` 不是 state 4 —— \
                 state 5/6/8 的挂起路径将变为可达，通道 5/6 必须先补实现（见本文件模块文档）",
                f.r,
                f.c,
                f.exp
            );
            total += 1;
        }
    }
    assert_eq!(
        total, TOTAL_FORMULAS,
        "公式条数变了 —— 夹具被重生成了？先核对新增形态的 get_state() 再改这个常量"
    );
}

/// 顺带把 `is_ds_function()` 也钉到真实夹具上：**至少要有一条公式是
/// 「DS 函数真、驱动扩展假」**（就是 `sum(ds1.select(qty))` 那一类）。
///
/// 若这条前提消失（例如夹具被简化到只剩 `ds1.select(x)`），
/// 那么"两条链必须分开实现"在夹具层面就失去了证据 —— 钉子先红。
#[test]
fn fixtures_contain_at_least_one_ds_function_that_does_not_extend() {
    let mut hits: Vec<(String, String)> = Vec::new();
    for name in all_names() {
        let fx = load(name);
        for f in &fx.formulas {
            let e = rpx_engine::parse(&f.exp).expect("上一条用例已保证可解析");
            if e.is_ds_function() && !e.is_extend() {
                hits.push((name.to_string(), f.exp.clone()));
            }
        }
    }
    assert!(
        !hits.is_empty(),
        "没有任何「DS 函数真 / 驱动扩展假」的公式 ⇒ `is_ds_function` 与 `is_extend` \
         必须分开实现这条结论在夹具上失去了证据"
    );
    // 已知样本（`engine_sum` 的 (2,6)）：逐行 100/200/150 的值级证据在 §5.2.5。
    assert!(
        hits.iter()
            .any(|(n, exp)| n == "engine_sum" && exp == "sum(ds1.select(qty))"),
        "engine_sum 的 `sum(ds1.select(qty))` 是本结论的实测锚点，实际命中：{hits:?}"
    );
}
