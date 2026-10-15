//! XLSX 导出（M6-02）—— 契约：`docs/spec-m6.md` §2.4（读回自洽对拍）。
//!
//! # 为什么验收是「读回自洽」而不是字节对拍
//!
//! xlsx 是 zip 容器（时间戳/条目序/压缩级别），**字节级对拍无意义**。
//! 验收闭环：`CalcGrid` → [`to_xlsx`] → 字节 → `calamine` 读回 →
//! 逐格比值/**类型** + 合并区与 `MergedCell` 全等（测试在 `xlsx/tests.rs`）。
//!
//! # 写值口径（自定；产品导出真值未探针，spec-m6 §0 / §5.1 带账）
//!
//! | 值 | xlsx 落法 | 读回形态 |
//! | --- | --- | --- |
//! | `Null`（无 disp） | 不写（空格） | 空 |
//! | `Int/Long/Short/Byte` | number（f64） | Float × |
//! | `Double/Float`（有限） | number | Float |
//! | `Double/Float`（NaN/inf） | **文本** `NaN`/`inf`/`-inf`（同 [`crate::text`] 口径）—— Excel 数字本体没有非有限数，写库会自作主张落 `NAN`/`INF` 全大写文本，故在导出面拦截 | String |
//! | `Decimal` | number（`parse::<f64>` 成功且有限）；失败/溢出 → 文本原样 | Float / String |
//! | `Text` | string | String |
//! | `Bool` | boolean（**原生布尔**，不是 "true"/"false" 文本） | Bool |
//! | `List` | 文本（`value_text` 的逗号连接口径；xlsx 无列表类型） | String |
//! | `disp_value` 非空 | **优先**：文本 | String |
//!
//! ⚠ 数值族（含 Decimal）经 **f64** 落盘 —— Excel 单元格的存储本体就是
//! IEEE 754 double（15 位有效数字），超过 2^53 的大整数与超精度 Decimal
//! 会带损，这是格式的物理边界，不是本 crate 的实现选择。
//!
//! # 合并区（锚点重建）
//!
//! [`CalcGrid::merged`] 是**逐槽**视图：合并区矩形内**每个槽各一条**记录、
//! 都报**锚点整段跨度**（`merged_view::global_slots`）。xlsx 的
//! `merge_range` 要的是**每区一条锚点矩形**，故先重建：
//!
//! * 延续槽判定 = 「本槽已被先前（按 `(r,c)` 升序）某个矩形消费」——
//!   同一矩形内最先遇到（最上、再最左）的槽即左上锚点；
//! * 锚点值取 `merged` 记录带的锚点值；若锚点格在 [`CalcGrid::cells`] 里
//!   且有 `disp_value`，disp 优先（同普通格口径）。
//! * 病态输入防御（引擎不产出，但本函数对外部构造的 `CalcGrid` 也不 panic）：
//!   重叠矩形只保首见者；裁剪后退化成 1×1 的区不写（Excel 不允许单格合并）。
//!
//! # 边界（有测试钉）
//!
//! 1. **Excel 硬上限**：行 > 1,048,576 / 列 > 16,384 的部分**裁剪**不导
//!    （Excel 本身打不开；裁剪是不 panic 的防御边界）；
//! 2. **超长文本**：> 32,767 **字符**（写库判据同粒度）截断到 32,767 ——
//!    不截断则写库报 `MaxStringLengthExceeded` 而整份文件写不出；
//! 3. **越界格**（row/col 超出 `rows`/`cols`）忽略（同 CSV 口径）；
//! 4. **空网格**（rows/cols <= 0）仍产出**合法**的空 xlsx（不是空字节）；
//! 5. **确定性**：同输入**同字节** —— 写库的 zip 条目时间戳固定
//!    （内部 `DateTime::default()`，DOS 纪元 1980-01-01），且本函数把
//!    `DocProperties.creation_time` 也钉死为 1980-01-01T00:00:00Z。
//!    ⛔ 不钉 `creation_time` 则 `docProps/core.xml` 写**当前 UTC 时刻**，
//!    跨秒两次调用字节即不同 —— M6-03 的 socket×库直调对拍实测抓到
//!    （跨秒即不等）。M6-04 的「CLI 产出与库函数直调逐字节一致」依赖本条。

use std::collections::{HashMap, HashSet};

use rpx_engine::arena::{CalcCell, CalcGrid, MergedCell};
use rpx_engine::value::Value;
use rust_xlsxwriter::{DocProperties, ExcelDateTime, Format, Workbook, Worksheet};

use crate::text::value_text;

/// Excel 工作表行数硬上限（1 起计）。
const EXCEL_MAX_ROWS: i32 = 1_048_576;
/// Excel 工作表列数硬上限（1 起计）。
const EXCEL_MAX_COLS: i32 = 16_384;
/// Excel 单元格文本硬上限（**字符**数；与写库 `MAX_STRING_LEN` 同粒度）。
const EXCEL_MAX_TEXT_CHARS: usize = 32_767;

/// 计算态网格 → xlsx 字节（单 sheet、无样式；边界见模块文档）。
pub fn to_xlsx(grid: &CalcGrid) -> Vec<u8> {
    let max_row = grid.rows.clamp(0, EXCEL_MAX_ROWS);
    let max_col = grid.cols.clamp(0, EXCEL_MAX_COLS);

    // 稀疏格索引（越界/超上限的格不参与）。
    let mut index: HashMap<(i32, i32), &CalcCell> = HashMap::with_capacity(grid.cells.len());
    for cell in &grid.cells {
        if (1..=max_row).contains(&cell.row) && (1..=max_col).contains(&cell.col) {
            index.insert((cell.row, cell.col), cell);
        }
    }

    // 合并区锚点重建 + 槽覆盖集。
    let areas = rebuild_areas(&grid.merged, max_row, max_col);
    let mut covered: HashSet<(i32, i32)> = HashSet::new();
    for a in &areas {
        for r in a.r0..=a.r1 {
            for c in a.c0..=a.c1 {
                covered.insert((r, c));
            }
        }
    }

    let mut workbook = Workbook::new();
    // 钉死文档时间戳（模块文档 §确定性 5）：`DocProperties::new()` 默认取
    // 「当前 UTC 时刻」⇒ 跨秒两次调用 core.xml 字节不同。导出是可复现产物，
    // 固定为 1980-01-01T00:00:00Z（对齐 zip 条目时间戳的 DOS 纪元）。
    let epoch = ExcelDateTime::from_ymd(1980, 1, 1).expect("1980-01-01 恒为合法日期");
    workbook.set_properties(&DocProperties::new().set_creation_datetime(&epoch));
    {
        let ws = workbook.add_worksheet();
        // ① 普通格：跳过全部合并槽（槽值统一由 merge_range 落锚点值）。
        for r in 1..=max_row {
            for c in 1..=max_col {
                if covered.contains(&(r, c)) {
                    continue;
                }
                if let Some(cell) = index.get(&(r, c)) {
                    write_payload(ws, r, c, &cell_payload(cell));
                }
            }
        }
        // ② 合并区：锚点矩形 + 锚点值。
        let format = Format::new();
        for a in &areas {
            let payload = match index.get(&(a.r0, a.c0)) {
                // 锚点格在 cells 里 ⇒ 走统一取值口径（disp 优先）。
                Some(cell) => cell_payload(cell),
                // 锚点不在 cells 里（`merged` 不受稀疏规则约束）⇒ 用记录带的锚点值。
                None => payload_of(&a.value),
            };
            write_merged(ws, a, &payload, &format);
        }
    }
    workbook
        .save_to_buffer()
        .expect("内存 zip 写入不可失败：Vec<u8> 后端无 IO 错误源（论证见模块文档）")
}

/// 重建后的一个合并区（锚点矩形 + 锚点值）。
struct Area {
    r0: i32,
    c0: i32,
    r1: i32,
    c1: i32,
    value: Value,
}

/// 从**逐槽**视图重建锚点矩形（规则见模块文档 §合并区）。
///
/// `slots` 是**逐槽**视图：一个矩形内**每槽一条**记录，且每条都报**整段跨度**
/// （`merged_view::global_slots`），结构里**没有**「谁是锚点」的字段。故按 `(r,c)`
/// 升序**贪心消费**：同一矩形内最先遇到（最上、再最左）的槽即左上锚点。
///
/// ⛔ **不可**用「存在另一条**同跨度**记录覆盖本槽」判延续槽（旧实现，已修）：
/// 相邻的等跨度区会互相把**对方的锚点**判成延续槽 ⇒ 整区被**静默丢弃**。
/// 触发面很常见：并排的多级表头（`A2:B2` / `C2:D2` / `E2:F2`）或相邻两段同值合并
/// （同为 `6×1` 的 `A4:A9` 与 `A13:A18`）。实测 18 区的报表只导出 11 区。
///
/// 复杂度：排序 `O(N log N)` + 消费 `O(槽数)`（旧实现是 `O(N²)` 的两两比对）。
fn rebuild_areas(slots: &[MergedCell], max_row: i32, max_col: i32) -> Vec<Area> {
    let mut out: Vec<Area> = Vec::new();
    // 病态输入（重叠矩形）防御：已占用槽；重叠区只保首见者。
    let mut taken: HashSet<(i32, i32)> = HashSet::new();
    // 行主序 ⇒ 每个矩形的左上锚点必先于其延续槽出现（`sort_by_key` 稳定）。
    let mut order: Vec<&MergedCell> = slots.iter().collect();
    order.sort_by_key(|s| (s.r, s.c));
    for s in order {
        // 已被先前矩形消费 ⇒ 本槽是延续槽。
        if taken.contains(&(s.r, s.c)) {
            continue;
        }
        // 锚点进 Excel 域：越界（含超上限）的整区跳过；跨界的裁剪到界内。
        let (r0, c0) = (s.r, s.c);
        if !(1..=max_row).contains(&r0) || !(1..=max_col).contains(&c0) {
            continue;
        }
        let r1 = (s.r + s.row_merge - 1).min(max_row);
        let c1 = (s.c + s.col_merge - 1).min(max_col);
        if r1 <= r0 && c1 <= c0 {
            // 裁剪后退化成单格（或倒矩形）：Excel 不允许单格合并。
            continue;
        }
        let mut cells_here: Vec<(i32, i32)> = Vec::new();
        let mut overlap = false;
        for r in r0..=r1 {
            for c in c0..=c1 {
                overlap |= taken.contains(&(r, c));
                cells_here.push((r, c));
            }
        }
        // ⚠ 即便因重叠要丢弃本区，也**必须整区占位**：否则本区的下半截会在后续轮次
        // 作为「新锚点」被裁剪后单独落盘，产出半个区（`pathological_overlap…` 钉住）。
        for &p in &cells_here {
            taken.insert(p);
        }
        if overlap {
            continue; // 病态重叠：丢后者（与「无构造点的分支不保留」同精神，此处保留防御）
        }
        out.push(Area {
            r0,
            c0,
            r1,
            c1,
            value: s.value.clone(),
        });
    }
    out
}

/// 一格的落盘载荷（口径表见模块文档）。
enum CellPayload {
    /// 不写（空格）。
    Skip,
    /// 数字。
    Number(f64),
    /// 原生布尔。
    Bool(bool),
    /// 文本（写前按 [`EXCEL_MAX_TEXT_CHARS`] 截断）。
    Text(String),
}

/// 格 → 载荷：`disp_value` 优先，否则按值类型（同 CSV 的取值顺序）。
fn cell_payload(cell: &CalcCell) -> CellPayload {
    if let Some(disp) = &cell.disp_value {
        return CellPayload::Text(disp.clone());
    }
    payload_of(&cell.value)
}

/// 值 → 载荷（类型分派见模块文档口径表）。
fn payload_of(v: &Value) -> CellPayload {
    match v {
        Value::Null => CellPayload::Skip,
        Value::Int(n) => CellPayload::Number(f64::from(*n)),
        Value::Long(n) => CellPayload::Number(*n as f64),
        Value::Short(n) => CellPayload::Number(f64::from(*n)),
        Value::Byte(n) => CellPayload::Number(f64::from(*n)),
        Value::Double(f) => number_or_text(*f),
        Value::Float(f) => number_or_text(f64::from(*f)),
        // Decimal 走 f64（Excel 数字的存储本体）；解析失败/溢出回退文本原样。
        Value::Decimal(s) => match s.parse::<f64>() {
            Ok(f) if f.is_finite() => CellPayload::Number(f),
            _ => CellPayload::Text(s.clone()),
        },
        Value::Text(s) => CellPayload::Text(s.clone()),
        Value::Bool(b) => CellPayload::Bool(*b),
        Value::List(_) => CellPayload::Text(value_text(v)),
    }
}

/// 有限数 → number；非有限数 → 文本（Rust Display 的 `NaN`/`inf`/`-inf`）。
///
/// ⛔ 不能直接交给写库的 `write_number`：它会把 NaN/inf 自作主张落成
/// `NAN`/`INF`/`-INF` 全大写文本（`worksheet.rs` 的 `store_number_type`），
/// 与本 crate 的文本口径分裂成两套写法。
fn number_or_text(f: f64) -> CellPayload {
    if f.is_finite() {
        CellPayload::Number(f)
    } else {
        CellPayload::Text(f.to_string())
    }
}

/// 按 **字符** 粒度截断到 Excel 上限（写库判据 `chars().count() > 32767` 同粒度）。
fn clip_text(s: &str) -> &str {
    if s.chars().count() <= EXCEL_MAX_TEXT_CHARS {
        return s;
    }
    let cut = s
        .char_indices()
        .nth(EXCEL_MAX_TEXT_CHARS)
        .map_or(s.len(), |(i, _)| i);
    &s[..cut]
}

/// 坐标换算：1 起 `(row, col)` → 写库 0 起 `(RowNum, ColNum)`。
///
/// ⛔ 列类型是 **u16**（`ColNum`）：值域由 [`EXCEL_MAX_COLS`] 裁剪保证，
/// 转换不会截断。
fn pos(r: i32, c: i32) -> (u32, u16) {
    ((r - 1) as u32, (c - 1) as u16)
}

/// 写一格载荷（坐标 1 起 ⇄ 写库 0 起）。
///
/// `expect` 论证（本函数内自足，非「刚才还在」式取值，code-review.md §5.3）：
/// 写失败的全部已知源均已在本文件前置排除 —— ① 坐标越界（`RowColumnLimitError`）：
/// r/c 已裁剪进 Excel 上限；② 文本超长（`MaxStringLengthExceeded`）：文本已
/// [`clip_text`]；③ 空串无格式：写库对空串返回 Ok（忽略）。故 `expect` 不可达。
fn write_payload(ws: &mut Worksheet, r: i32, c: i32, p: &CellPayload) {
    let (row, col) = pos(r, c);
    let done = match p {
        CellPayload::Skip => return,
        CellPayload::Number(n) => ws.write_number(row, col, *n).map(|_| ()),
        CellPayload::Bool(b) => ws.write_boolean(row, col, *b).map(|_| ()),
        CellPayload::Text(s) => ws.write_string(row, col, clip_text(s)).map(|_| ()),
    };
    done.expect("写格不可失败（论证见函数注释）");
}

/// 写一个合并区：`merge_range` 落空白 + 左上格按载荷类型覆写。
///
/// `expect` 论证：`MergeRangeSingleCell`/`RowColumnOrderError` 由 [`rebuild_areas`]
/// 保证（跨度 >1、r0<=r1）；`MergeRangeOverlaps` 由 `taken` 去重保证；
/// `RowColumnLimitError` 由裁剪保证。
fn write_merged(ws: &mut Worksheet, a: &Area, p: &CellPayload, format: &Format) {
    let (r0, c0) = pos(a.r0, a.c0);
    let (r1, c1) = pos(a.r1, a.c1);
    let text = match p {
        CellPayload::Text(s) => clip_text(s).to_string(),
        _ => String::new(),
    };
    let merged = ws.merge_range(r0, c0, r1, c1, &text, format).map(|_| ());
    merged.expect("merge_range 不可失败（论证见函数注释）");
    // 数值/布尔：覆写左上格（merge_range 只能落字符串）。
    match p {
        CellPayload::Number(n) => {
            ws.write_number_with_format(r0, c0, *n, format)
                .expect("覆写左上格不可失败（同上论证）");
        }
        CellPayload::Bool(b) => {
            ws.write_boolean_with_format(r0, c0, *b, format)
                .expect("覆写左上格不可失败（同上论证）");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
