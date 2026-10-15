//! 计算态 → JSON（`tests/fixtures/CalcJson.java` 的逐行镜像）。
//!
//! # 契约
//!
//! [`render_document`] 产出的文本与 `engine_*.expected.json` **同 schema**
//! （顶层 `calculated` + `pagination` + `merged` 三段，供 `tests/common/compare` 逐格对拍）；
//! [`render_calculated`] / [`render_merged`] 是它的两个分段渲染器，
//! [`render_calculated_error`] 产出错误型夹具（`engine_cycle`）的 `calculated.error` 段。
//!
//! `pagination` 段由 [`pagination`] 渲染（M2-11 已落地）；网格未跑分页器时写 `null`
//! （错误型夹具的 `pagination` 也正是 `null`）。
//!
//! 事实源（唯一）：`tests/fixtures/CalcJson.java`（`calculated` + `pagination`）
//! 与 `tests/fixtures/CalcMergeJson.java`（`merged`）
//! - `renderCalc(IReport):33-62` → [`render_calculated`] 的外层与稀疏规则
//! - `cellJson(...):64-119` → [`render_cell`]
//! - `headJson(...):129-134` → [`head_json`]
//! - `renderPages(...):140-192` → [`pagination::section`]
//! - `value(Object):219-254` / `typeName(Object):256-285` → [`val`] 与 [`crate::value::Value::type_name`]
//! - `renderMerged(...):30-71` → [`render_merged`]
//!
//! # 五条必须照抄、且每条都踩过的细节
//!
//! 1. **键序**（`CalcJson.java:66-117`）：`r, c, value, type, [disp_value],
//!    [left_head], [top_head], [source]`。对象在 [`json`] 里按插入序写，所以顺序即契约。
//! 2. **`cells` 稀疏**（`CalcJson.java:51-53`）：`value == null && dispValue == null`
//!    的格**不落盘**。所以"坐标不在 `cells` 里" = "该格为空"，**不是**断言失败
//!    （对拍器按 `(r,c)` 建索引，不按下标对位）。
//! 3. **`disp_value` 的判据是"与 `String.valueOf(value)` 不等"**（`CalcJson.java:71`）：
//!    `dv != null && !dv.equals(String.valueOf(v))`。⛔ `String.valueOf(null)` 是
//!    字符串 **`"null"`**（不是空串！），所以 `value == null && disp_value == "null"`
//!    时**不写**该键。判等用 [`val::java_tostring`]（`Object.toString`），
//!    **不是** [`val::render_value`]（差在整数值 `Double` 的折叠上，见该模块文档）。
//! 4. **`left_head` / `top_head` 是可选键**（`CalcJson.java:77-90`）：`getCellLeftHead()`
//!    返回 `null` 时**省略键**、不写 `null`。值取 **head 目标的模板出处**
//!    （`head.getSourceRow()/getSourceCol()`），不是它在计算态里的坐标 ——
//!    167 个被引用目标里两者有 149 个不同（模块文档 `CalcJson.java:122-128` 明写），
//!    所以这个选择有判别力。本 crate 已在 [`crate::arena::CalcCell::left_head`] 里
//!    存模板坐标对，导出期不再做任何换算。
//! 5. **`source` 是可选键**（`CalcJson.java:92-117`）：`ecs.getSource(r,c)` 为 `null`
//!    时省略键；`sub_values` 只在 `sub_cells > 1` 时才写（`:102`）。
//!    内层键序 `src_row, src_col, row_merge, col_merge, extend, sub_cells, [sub_values]`，
//!    `sub_values[]` 每项 `r, c, value, type`。
//!
//! # 尺寸口径
//!
//! [`CalcGrid::rows`] / [`CalcGrid::cols`] 已经是**数据格数**（= 矩阵尺寸 − 1），
//! 与 `CalcJson.java:39-42` 的 `getRowCount()/getColCount()` 同口径，**不再减 1**。
//!
//! # 分片
//!
//! 受"单文件 ≤500 行"约束：JSON 文本构造在 [`json`]，Java 数字记法在 [`num`]，
//! 值渲染与 `Object.toString` 在 [`val`]，`pagination` 段在 [`pagination`]，单测在 `tests.rs`。

use crate::arena::{CalcCell, CalcGrid, MergedCell, SourceRef, SubValue};

mod json;
mod num;
mod pagination;
mod val;

/// 与 `engine_*.expected.json` 同 schema 的**顶层文档**：`calculated` + `pagination` + `merged`。
///
/// 键序即契约：夹具顶层是 `…, calculated, pagination, merged`（`CalcJson.java:177-185`
/// 与 `CalcMergeJson` 的拼接序），本函数按同序产出。
pub fn render_document(grid: &CalcGrid) -> String {
    json::obj(vec![
        ("calculated", calculated_section(grid)),
        ("pagination", pagination::section(grid)),
        ("merged", merged_section(grid)),
    ])
}

/// 把计算态网格渲染成 `calculated` 段的顶层文档。
///
/// 返回 `{"calculated":{"rows":R,"cols":C,"cells":[...]}}`（**不含** `merged`）。
/// 逐格规则见模块文档；`grid.cells` 只装已落盘的格（稀疏），这里再按 `(row, col)`
/// **行优先**排序 —— `CalcJson.java:43-44` 的两层 `for` 就是这个顺序，排序让
/// 引擎输出与夹具的字符级 diff 可读（对拍器本身按 `(r,c)` 建索引，不依赖顺序）。
pub fn render_calculated(grid: &CalcGrid) -> String {
    json::obj(vec![("calculated", calculated_section(grid))])
}

/// 把分页结果渲染成 `pagination` 段的顶层文档：`{"pagination":{…}}`；未跑分页器时
/// `{"pagination":null}`（与错误型夹具同形）。
///
/// 事实源 `CalcJson.renderPages`（`tests/fixtures/CalcJson.java:140-192`），
/// 键序与两个可选键的判据见 [`pagination`] 模块文档。
pub fn render_pagination(grid: &CalcGrid) -> String {
    json::obj(vec![("pagination", pagination::section(grid))])
}

/// 把合并格渲染成 `merged` 段的顶层文档：`{"merged":{"page_count":N,"cells":[...]}}`。
///
/// 事实源 `CalcMergeJson.renderMerged`：**只跳** `rm<=1 && cm<=1`，不跳「值为空」
/// （`:48-50`）⇒ 锚点值可能是 `null`，本段因此比 `calculated.cells` 稠密。
/// 跑了分页器时按**页格**口径投影（页内坐标 + 页内跨度，M5-02，见
/// [`merged_section`]）。
pub fn render_merged(grid: &CalcGrid) -> String {
    json::obj(vec![("merged", merged_section(grid))])
}

/// `calculated` 段的内层对象（不带外层键）。
fn calculated_section(grid: &CalcGrid) -> String {
    let mut shown: Vec<&CalcCell> = grid.cells.iter().filter(|c| c.is_emitted()).collect();
    shown.sort_by_key(|c| (c.row, c.col));
    let cells = json::arr(shown.iter().map(|c| render_cell(c)).collect());
    json::obj(vec![
        ("rows", json::int(i64::from(grid.rows))),
        ("cols", json::int(i64::from(grid.cols))),
        ("cells", cells),
    ])
}

/// `merged` 段的内层对象（`CalcMergeJson.java:62-64`：`page_count` 在前、`cells` 在后）。
///
/// # M5-02：页内视图来自 [`CalcGrid::merged_pages`]
///
/// 产品 `CalcMergeJson.renderMerged` 读的是**页**格 ⇒ 对拍形状是**页内逐槽 +
/// 页内跨度**。引擎在分页后经 [`crate::calc::merged_view`] 把合并区投影成
/// 页内视图（`merged_pages`；产品「跨页拆分」=`SimplePager._$1:1624-1638`
/// 行向 deepClone 锚点 + `setEndRow` 收窄，`:2316-2335` 列向同构），本函数
/// 只做序列化 —— 与产品「拆分在分页器、渲染器只读页格」的分工同构。
///
/// 未跑分页器（`grid.pagination == None`：错误型夹具 / 手搭网格）⇒ 全局视图
/// （`grid.merged`）原样透出。
fn merged_section(grid: &CalcGrid) -> String {
    let mut shown: Vec<MergedCell> = match &grid.pagination {
        Some(_) => grid.merged_pages.clone(),
        None => grid.merged.clone(),
    };
    // `CalcMergeJson.java:37-60`：页序 → 行 → 列。
    shown.sort_by_key(|m| (m.page, m.r, m.c));
    let cells = json::arr(shown.iter().map(merged_cell_json).collect());
    json::obj(vec![
        ("page_count", json::int(i64::from(grid.page_count))),
        ("cells", cells),
    ])
}

/// `merged.cells[]` 一项（`CalcMergeJson.java:51-58`）：`page, r, c, value, row_merge, col_merge`。
fn merged_cell_json(m: &MergedCell) -> String {
    json::obj(vec![
        ("page", json::int(i64::from(m.page))),
        ("r", json::int(i64::from(m.r))),
        ("c", json::int(i64::from(m.c))),
        ("value", val::render_value(&m.value)),
        ("row_merge", json::int(i64::from(m.row_merge))),
        ("col_merge", json::int(i64::from(m.col_merge))),
    ])
}

/// 错误形态（`expected.json` 的 `calculated.error` 块）：**只写 `class` 与 `chain`**。
///
/// ⛔ 不写 `message` / `cause_class` / `cause_message`：产品那三个都是本地化文本
/// （`engine_cycle` 里是中文），对拍器**只比** `class` 与 `chain`
/// （`tests/common/compare/mod.rs:142-159`，理由见 `GenEngineFixtures.java:179-184`）。
/// `chain` 为 `None` 时**省略键**（夹具生成器同样只在非空时写，`GenEngineFixtures.java:254-256`）。
pub fn render_calculated_error(class: &str, chain: Option<&str>) -> String {
    let mut err = vec![("class", json::quote(class))];
    if let Some(c) = chain {
        err.push(("chain", json::quote(c)));
    }
    json::obj(vec![(
        "calculated",
        json::obj(vec![("error", json::obj(err))]),
    )])
}

/// `CalcJson.cellJson`（`:64-119`）：一格的全部键。
fn render_cell(c: &CalcCell) -> String {
    let v = &c.value;
    let mut f: Vec<(&'static str, String)> = vec![
        ("r", json::int(i64::from(c.row))),
        ("c", json::int(i64::from(c.col))),
        ("value", val::render_value(v)),
        ("type", json::quote(v.type_name())),
    ];
    // 细节 3：判等用 Object.toString，不是 value() 的折叠写法
    if let Some(dv) = &c.disp_value {
        if *dv != val::java_tostring(v) {
            f.push(("disp_value", json::quote(dv)));
        }
    }
    // 细节 4：`{src_row, src_col}` 是 head 目标的**模板出处**
    if let Some((r, col)) = c.left_head {
        f.push(("left_head", head_json(r, col)));
    }
    if let Some((r, col)) = c.top_head {
        f.push(("top_head", head_json(r, col)));
    }
    // 细节 5：source 可选；sub_values 只在 sub_cells > 1 时写
    if let Some(s) = &c.src {
        f.push(("source", source_json(s, &c.sub_values)));
    }
    json::obj(f)
}

/// `CalcJson.headJson`（`:129-134`）：`{src_row, src_col}`，取**模板出处**。
fn head_json(src_row: i32, src_col: i32) -> String {
    json::obj(vec![
        ("src_row", json::int(i64::from(src_row))),
        ("src_col", json::int(i64::from(src_col))),
    ])
}

/// `CalcJson.cellJson`（`:92-117`）的 `source` 块。
fn source_json(s: &SourceRef, subs: &[SubValue]) -> String {
    let mut f: Vec<(&'static str, String)> = vec![
        ("src_row", json::int(i64::from(s.src_row))),
        ("src_col", json::int(i64::from(s.src_col))),
        ("row_merge", json::int(i64::from(s.row_merge))),
        ("col_merge", json::int(i64::from(s.col_merge))),
        // 运行时**生效**的扩展方式（16/17/18/19），不是模板声明值
        ("extend", json::int8(s.extend.code())),
        ("sub_cells", json::int(i64::from(s.sub_cells))),
    ];
    if s.sub_cells > 1 {
        f.push((
            "sub_values",
            json::arr(subs.iter().map(sub_value_json).collect()),
        ));
    }
    json::obj(f)
}

/// `source.sub_values[]` 一项（`CalcJson.java:105-112`）：**计算态**坐标 + 值。
fn sub_value_json(sv: &SubValue) -> String {
    json::obj(vec![
        ("r", json::int(i64::from(sv.r))),
        ("c", json::int(i64::from(sv.c))),
        ("value", val::render_value(&sv.value)),
        ("type", json::quote(sv.value.type_name())),
    ])
}

#[cfg(test)]
mod tests;
