//! 几何与合并：行高列宽换算 + `mergeData` 锚点去重（算法：`docs/spec-m4.md` §4.4）。

use serde_json::{json, Map, Value as J};

use rpx_core::props::def;
use rpx_engine::arena::CalcGrid;

/// 声明近似：96dpi 线性换算（`spec-m4.md` §7 —— 无产品基准可对拍，只求视觉正确）。
const MM_TO_PX: f64 = 96.0 / 25.4;

/// 默认行高（`def::ROW_HEIGHT` = 8mm ≈ 30px）。
pub(crate) fn default_row_height_px() -> i64 {
    px(def::ROW_HEIGHT as f64).round() as i64
}

/// 默认列宽（`def::COL_WIDTH` = 25mm ≈ 94px）。
pub(crate) fn default_col_width_px() -> i64 {
    px(def::COL_WIDTH as f64).round() as i64
}

/// 数值 → px（保留 2 位小数）。
fn px(mm: f64) -> f64 {
    (mm * MM_TO_PX * 100.0).round() / 100.0
}

/// 计算态尺寸 → px：`unit == 1` 是 mm（`CalcGrid.unit` 文档）；
/// 其余单位语义未建模 ⇒ **原值直出**（声明：`spec-m4.md` §7）。
/// M4-05 起 `paged` 切片模块也用（页内 rowData/columnData）。
pub(crate) fn size_px(v: f32, unit: i8) -> f64 {
    if unit == 1 {
        px(v as f64)
    } else {
        ((v as f64) * 100.0).round() / 100.0
    }
}

/// `rowData`：`{"r": {"h": px}}`，键 **0 起**（`spec-m4.md` §4.2）。
///
/// `CalcGrid.row_heights` 已按扩展铺开并含 `calcProperties` 的行高覆盖
/// （`arena/calc_grid.rs` 字段文档）⇒ 直接映射。
pub(crate) fn row_data(grid: &CalcGrid) -> J {
    let mut m = Map::new();
    for (i, h) in grid.row_heights.iter().enumerate() {
        m.insert(i.to_string(), json!({ "h": size_px(*h, grid.unit) }));
    }
    J::Object(m)
}

/// `columnData`：`{"c": {"w": px}}`，键 **0 起**。
pub(crate) fn col_data(grid: &CalcGrid) -> J {
    let mut m = Map::new();
    for (i, w) in grid.col_widths.iter().enumerate() {
        m.insert(i.to_string(), json!({ "w": size_px(*w, grid.unit) }));
    }
    J::Object(m)
}

/// `mergeData` 锚点提取（`spec-m4.md` §4.4）。
///
/// 实证依据：`engine_merge.expected.json` 的 `merged.cells` **12 槽 = 6 块 × 2 槽**；
/// 每槽携带**整块**跨度（`calc/run.rs` 的 `export_merged`：延续槽上的格就是
/// 锚点对象，故报锚点跨度；M5-02 起 `CalcGrid.merged` 定型为**全局逐槽**视图，
/// 页内投影只在 `emit` 侧，本函数因此始终面对全局坐标）——**但槽坐标是槽自己的**，
/// 单槽无法推断锚点位置（相邻块接续时 `(3,1)` 会"看起来覆盖" `(4,1)` 的块，
/// 这就是初版被覆盖判定误滤两条锚点的原因）。
///
/// 正确算法：**合并区 = 完整矩形槽集** ⇒ 贪心消费 + 逐格校验——
/// 游标指向的未消费槽必为某块的最小槽（块内其余槽若先行出现会被更小的
/// 游标消费掉，矛盾），以其跨度 `(rm,cm)` 展开网格，要求网格内每格都有
/// **同跨度**的槽；不满足即报错（不静默——数据违反"矩形槽集"不变式时
/// 宁可失败，不给错误分页）。
///
/// ⚠ 复杂度 O(n² × rm×cm)：合并槽数在报表规模下很小（百量级），v1 不做索引。
pub(crate) fn merge_data(grid: &CalcGrid) -> Result<Vec<J>, String> {
    // 槽集合（去重），按 (r, c) 排序保证输出稳定。
    let mut slots: Vec<(i32, i32, i32, i32)> = Vec::new();
    for m in &grid.merged {
        let key = (m.r, m.c, m.row_merge, m.col_merge);
        if !slots.contains(&key) {
            slots.push(key);
        }
    }
    slots.sort_by_key(|s| (s.0, s.1));

    let mut consumed = vec![false; slots.len()];
    let mut out = Vec::new();
    for i in 0..slots.len() {
        if consumed[i] {
            continue;
        }
        let (r0, c0, rm, cm) = slots[i];
        // 展开网格并逐格校验（同跨度）。
        for di in 0..rm {
            for dj in 0..cm {
                let (pr, pc) = (r0 + di, c0 + dj);
                let found = slots
                    .iter()
                    .position(|s| s.0 == pr && s.1 == pc && s.2 == rm && s.3 == cm);
                match found {
                    None => {
                        return Err(format!(
                            "合并区锚点恢复失败：块起点 ({r0},{c0}) 跨度 ({rm},{cm}) \
                             的网格点 ({pr},{pc}) 缺槽（数据违反矩形槽集不变式）"
                        ));
                    }
                    Some(j) if consumed[j] => {
                        return Err(format!(
                            "合并区锚点恢复失败：网格点 ({pr},{pc}) 已被其他块消费（块重叠）"
                        ));
                    }
                    Some(j) => consumed[j] = true,
                }
            }
        }
        // 1 起坐标 + 跨度 → 闭区间端（0 起）。
        out.push(json!({
            "startRow": r0 - 1,
            "startColumn": c0 - 1,
            "endRow": r0 + rm - 2,
            "endColumn": c0 + cm - 2,
        }));
    }
    Ok(out)
}
