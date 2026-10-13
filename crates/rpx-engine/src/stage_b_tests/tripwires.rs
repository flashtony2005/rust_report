//! 两条**前提钉子**（tripwires）：它们不验证"实现对不对"，而是验证
//! **"当前没有夹具能区分两种语义"这个前提还没被打破**。
//!
//! 为什么需要这种测试：本项目反复出现同一类风险 —— 一处忠实实现的分支
//! **在当前夹具上恒等于**某个简化写法（`file:line` 有出处、语义却抓不到）。
//! 这类代码一旦被后人"顺手简化"，不会有任何失败；而等真夹具到来时，
//! 又没人记得当初为什么那么写。钉子测试把"前提"变成断言：
//! **前提一破就红**，红的地方就是该去核实产品行为、补真夹具的地方。
//!
//! 单独立一个文件的原因只有一条：`stage_b_tests.rs` 已接近 300 行门禁。

use super::json::{expected_text, fixtures_dir, template_dims};
use super::GRID_FIXTURES;
use crate::stage_b::cell_expression;

/// ⚠ **钉子 A**：`cell_expression` 把**空白串**也当成"没有表达式"，而 Java 的
/// 判据是 `expression != null` —— 只看 `expMap` 有没有键 40，**不看内容**（`ExtNormalCell.java:500`）。
/// 两种语义只在"键 40 存在但内容是空白"时才分叉，而**现有夹具没有这种格**，
/// 所以这是一处**未经证实的偏差**（行为未改，只是声明）。
///
/// 一旦将来出现这种格，这条测试会失败，提醒去核实产品的真实行为：
/// `Expression.java:496-512` 对空串也会走 `create` 解析，大概率抛 `ReportError`，
/// 即产品很可能**也是硬失败**；但在拿到证据前不擅自改语义。
#[test]
fn no_fixture_has_a_blank_expression_so_the_trim_assumption_stays_unexercised() {
    for name in GRID_FIXTURES {
        let path = fixtures_dir().join(format!("{name}.rpx"));
        let rd = rpx_core::open(std::fs::File::open(&path).unwrap()).unwrap();
        let (tr, tc) = template_dims(&expected_text(name));
        for r in 1..=tr {
            for c in 1..=tc {
                let Some(cell) = rd.cell_at(r as usize, c as usize) else { continue };
                let Some(map) = cell.exp_map.as_ref() else { continue };
                for (code, v) in map {
                    if *code != crate::stage_b::CELL_EXPR_PROP {
                        continue;
                    }
                    if let rpx_core::reader::ObjValue::Str(s) = v {
                        assert!(
                            !s.trim().is_empty(),
                            "{name} 模板格 ({r},{c}) 的 expMap[40] 是空白串 —— \
                             该去核实 Java 的 `expression != null` 语义了"
                        );
                    }
                }
            }
        }
    }
}

/// ⚠ **钉子 B**：值写入的守卫在本实现里是 **`expression == null`**
/// （`stage_b.rs` 第 2 段的 `!has_expr[…]`），而角色判据 `CellRole::Literal` 与它
/// **不等价**：`extend/role.rs` 的 `drives_extension()`（声明 18/19）短路在
/// `!has_expression` **之前**，于是「声明 18/19 且**无**表达式」的格角色是
/// [`crate::extend::CellRole::Extender`]，**但 Java 照样写值**
/// （`ExtExtendCell.java:331-332` 的 else 分支调 `setValue(模板值)`）。
///
/// 前提本身已由 `extend/tests.rs::declared_18_and_19_are_extenders_even_without_an_expression` 钉住；
/// 这条钉子补的是**另一头**：现有夹具里**没有**这种格，所以
/// `role == Literal` 与 `!has_expr` 在夹具上**恒等** ⇒ 没有任何测试能区分两者。
///
/// 一旦某个夹具出现这种格，本测试失败 —— 那时必须**为新守卫建真实断言**
/// （值确实被写进去了），而不是继续沿用"两种写法等价"的假设：
/// 用 `role == Literal` 会静默漏掉这一格的值。
#[test]
fn no_fixture_declares_18_or_19_without_an_expression_so_the_guard_equivalence_stays_vacuous() {
    let mut declared_extenders = 0usize;
    for name in GRID_FIXTURES {
        let path = fixtures_dir().join(format!("{name}.rpx"));
        let rd = rpx_core::open(std::fs::File::open(&path).unwrap()).unwrap();
        let (tr, tc) = template_dims(&expected_text(name));
        for r in 1..=tr {
            for c in 1..=tc {
                let Some(cell) = rd.cell_at(r as usize, c as usize) else { continue };
                if !matches!(cell.extend_mode, 18 | 19) {
                    continue;
                }
                declared_extenders += 1;
                // Java 只在 `expression == null` 时才写值；有表达式就走改写路径。
                if cell_expression(cell).is_none() {
                    panic!(
                        "{name} 模板格 ({r},{c}) 声明 extend={} 且**无**表达式：\
                         出现「角色是 Extender 但仍要写值」的格 —— \
                         该为 `!has_expr` 守卫建真实断言了（`stage_b.rs` 第 2 段）",
                        cell.extend_mode
                    );
                }
            }
        }
    }
    // 反向自检：扫描若失效（比如模板尺寸读成 0），上面那条 panic 永远不会触发。
    assert!(
        declared_extenders > 0,
        "11 个网格夹具里应存在声明 18/19 的格（engine_ext 的锚点），扫描口径可能错了"
    );
}
