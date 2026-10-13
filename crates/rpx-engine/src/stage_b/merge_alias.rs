//! 合并区的两张矩阵分歧建模（by_pos 别名 + by_src 影子格）。
//!
//! # 实证定案（2026-10-13，产品自带 JRE，探针 `ProbeMergeAlias`/`ProbeTplSlot`）
//!
//! engine_merge 夹具的两个延续槽给出了**逐对象**证据：
//!
//! | 槽 | `_$26` 计算矩阵（`getCell`） | `_$25` 引擎矩阵（`getSource`） |
//! |----|------------------------------|--------------------------------|
//! | 横向延续 `(2,4)` | 锚点别名（`==(2,3)` 同对象） | 锚点别名（src=(2,3) cm=2） |
//! | 纵向延续 `(3,1)` | 锚点别名（`==(2,1)` 同对象） | **独立槽**（src=(3,1) rm=1 cm=1 ext=16） |
//!
//! 机制骨架：构造循环（`ExtCellSet.java:166-192`）先对合并区铺满两张矩阵，
//! 随后模板迭代**跳过横向延续槽**（`:185` 的 `j += ar.getEndCol() - startCol`）
//! 而正常访问纵向延续槽 —— 后者把 `_$25` 覆盖成独立槽引擎；`_$26` 的别名由
//! state=2 的非主槽重铺（`:756`）等路径恢复（重铺的精确触发点未逐行核实，
//! 但**别名这一最终状态有同对象实证**，以此为准）。
//!
//! CalcJson 的值/head 读 `_$26`、`source` 读 `_$25` ⇒ 两张矩阵的分歧必须同时建模：
//!
//! ① `by_pos`：合并区**所有**槽位铺锚点 id（别名）。延续槽的值/disp/head 随锚点
//!    导出。别名槽被主格判定天然跳过（通道 `slot_matches`、stage_b 两段的
//!    `primary`、`normalize_heads` 的 `row!=i||col!=j`），扩展算法的 `shift_rows`
//!    按槽位平移也不丢别名。
//! ② `by_src`：横向延续槽 → 锚点；纵向延续槽 → **影子格**（[`HeadState::push_shadow`]）。
//!
//! ⚠ 二维合并（rm>1 且 cm>1）对角槽的行为无夹具实证 —— 按纵向规则处理，
//! 已声明缺口（`docs/m2-engine-design.md` §5.4）。

use rpx_core::ReportDefine;

use crate::arena::CellId;
use crate::extend::ExtendMode;
use crate::heads::{HeadCell, HeadState};
use crate::stage_a::StageA;
use crate::stage_b::row_type_at;

/// 按上述分歧规则铺 `by_pos` 别名与 `by_src` 地址表。
///
/// 前置条件：`g` 的 cells 已按 arena 序建满（[`crate::stage_b::build_grid`] 的
/// while 循环完成），合并锚点即 `row_merge>1 || col_merge>1` 的主体格
/// （此时克隆尚不存在 —— 本函数跑在扩展之前）。
pub(crate) fn seed_merge_aliases(g: &mut HeadState, a: &StageA, rd: &ReportDefine) {
    let anchors: Vec<CellId> = g
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.row >= 1 && c.col >= 1 && (c.row_merge > 1 || c.col_merge > 1))
        .map(|(i, _)| CellId(i as u32))
        .collect();
    for aid in anchors {
        let (ar, ac, rm, cm) = {
            let c = &g.cells[aid.0 as usize];
            (c.row, c.col, c.row_merge, c.col_merge)
        };
        for r in ar..ar + rm {
            for c in ac..ac + cm {
                // ① 计算矩阵：别名。
                g.by_pos.insert((r, c), aid);
                if r == ar {
                    // ② 横向延续槽：引擎矩阵也是锚点别名（实证 src(2,4)=(2,3) cm=2）。
                    g.index_src((r, c), aid);
                } else {
                    // ② 纵向延续槽：影子格（不进 by_pos，只服务 source 导出与
                    // 地址查询）。属性读 `a.engine` 的独立槽 `SourceRef`；模板格
                    // 是解析器为延续槽合成的空白格（值 Null、无声明 head）⇒
                    // 影子不带值、不带表达式、不参与计算。
                    let extend = a
                        .engine
                        .get(r, c)
                        .map(|s| s.extend)
                        .unwrap_or(ExtendMode::Default16);
                    let mut hc = HeadCell::new(r, c, extend.code(), row_type_at(rd, r));
                    hc.rt.cell_type = rd
                        .cell_at(r as usize, c as usize)
                        .map(|t| t.cell_type)
                        .unwrap_or(crate::literal::NORMAL_CELL_TYPE);
                    let sid = g.push_shadow(hc);
                    g.index_src((r, c), sid);
                }
            }
        }
    }
    // 非合并区槽位照常登记（`getSource("B2")` 的地址表语义）；合并区槽位
    // 上面已按分歧规则登记过，这里跳过。
    let (rows, cols) = (a.calc.row_size(), a.calc.col_size());
    for r in 1..rows {
        for c in 1..cols {
            let id = a.calc.get(r, c);
            if !id.is_none() && !g.by_src.contains_key(&(r, c)) {
                g.index_src((r, c), id);
            }
        }
    }
}
