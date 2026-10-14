//! fixtures 的**类型化**视图：一个 `.expected.json` 对应一个 [`Fixture`]。
//!
//! 字段语义全部来自产品侧 writer，出处写到每个字段上（`CalcJson.java` /
//! `CalcMergeJson.java` / `GenEngineFixtures.java`），因为这一层是后续所有验收
//! 断言的基座 —— 基座读错，上面全是幻觉。
//!
//! 关键纪律（来自 `docs/spec-m2.md` §7 / M2-09 与 `CalcJson.java`）：
//!
//! * `Cell::declared_type` 是类型的**唯一权威**，禁止从 `value_raw` 的字面形状推断
//!   （`{"value":150,"type":"double"}` 合法）。
//! * 可选键一律 `Option`：`source`（`engine_ext` 42 格里 30 格没有）、
//!   `disp_value`、`sub_values`、`first_value` / `last_value`、
//!   `extend_mode` / `merge_same`、`pagination` / `merged`（错误型夹具为 `null`）。

/// 一个夹具的全部地面真值。
#[derive(Debug, Clone, PartialEq)]
pub struct Fixture {
    pub name: String,
    /// 行高列宽的单位：1=mm 0=inch 2=point（`GenEngineFixtures.java:159` 的 `unit_note`）。
    pub unit: i8,
    pub template_rows: i32,
    pub template_cols: i32,
    pub dataset: Dataset,
    pub formulas: Vec<Formula>,
    pub row_cells: Vec<RowCellInfo>,
    pub calculated: Calculated,
    /// 错误型夹具（`engine_cycle`）为 `None`（产品写 `null`）。
    pub pagination: Option<Pagination>,
    /// 错误型夹具（`engine_cycle`）为 `None`（产品写 `null`）。
    pub merged: Option<Merged>,
    /// 报表参数（M3-01；`GenEngineFixtures.java` 的 `params` 段）。
    /// **只有 `engine_param` 夹具有这个键**（其余夹具的 JSON 里没有它）——
    /// 所以这里是空 vec 而不是 `Option`：无参夹具与"参数段为空"同形。
    pub params: Vec<ParamSpec>,
}

impl Fixture {
    /// 计算态网格；错误型夹具返回 `None`。
    pub fn grid(&self) -> Option<&Grid> {
        match &self.calculated {
            Calculated::Grid(g) => Some(g),
            Calculated::Error(_) => None,
        }
    }

    /// 计算态错误；正常夹具返回 `None`。
    pub fn error(&self) -> Option<&CalcError> {
        match &self.calculated {
            Calculated::Error(e) => Some(e),
            Calculated::Grid(_) => None,
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self.calculated, Calculated::Error(_))
    }

    /// 取计算态 (r,c) 的单元格；空格或错误型夹具返回 `None`。
    pub fn cell(&self, r: i32, c: i32) -> Option<&Cell> {
        self.grid().and_then(|g| g.cell(r, c))
    }
}

/// 数据集字面量（`"region","qty"` 两列；行是字符串）。
#[derive(Debug, Clone, PartialEq)]
pub struct Dataset {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: i32,
}

/// 一条报表参数（M3-01：`GenEngineFixtures.java` 的 `params` 段，
/// `EngineModelsM3.paramsJson` 序列化）。
///
/// `value` 直接是引擎的 [`rpx_engine::Value`]：类型推断规则见
/// [`crate::tests::common::loader::parse_param_value`]（数组 → List、整数 → Int、
/// 小数 → Double、字符串 → Text、`null` → Null）。产品侧这些值来自
/// `Context.setParamValue`（`CalcProbe.java` 的注入重载），进引擎前不做转换。
#[derive(Debug, Clone, PartialEq)]
pub struct ParamSpec {
    pub name: String,
    pub value: rpx_engine::Value,
}

/// 一条公式记录。`r`/`c` 是**模板态**坐标，1 基、不含 0 号脚手架。
#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    pub r: i32,
    pub c: i32,
    pub exp: String,
    /// 仅当 `!= 16` 时出现（`GenEngineFixtures.java:137-139`）。`Option` 表示"未写出"。
    pub extend_mode: Option<i8>,
    /// 仅当 `!= 97` 时出现（`GenEngineFixtures.java:140-142`）。
    pub merge_same: Option<i8>,
}

/// 一行行属性（`GenEngineFixtures.java:146-155`）。
#[derive(Debug, Clone, PartialEq)]
pub struct RowCellInfo {
    pub r: i32,
    pub row_type: i32,
    pub group_header_level: i32,
    pub row_height: f64,
    /// 任何本结构未建模的键，原样保留（键名, 值的原文切片）。
    /// 契约要求"任何出现的可选键都要留下"，这里不做静默丢弃。
    pub extra: Vec<(String, String)>,
}

/// 计算态：要么是网格，要么是产品抛出的错误（`GenEngineFixtures.java:167-175`）。
#[derive(Debug, Clone, PartialEq)]
pub enum Calculated {
    Grid(Grid),
    Error(CalcError),
}

/// 计算态网格。`rows`/`cols` 是产品 `getRowCount()`/`getColCount()` 的口径
/// （数据格数量，不含 0 号脚手架）。`cells` 只收**非空**格（`CalcJson.java:43-56`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    pub rows: i32,
    pub cols: i32,
    pub cells: Vec<Cell>,
}

impl Grid {
    /// 按计算态坐标取格；空格返回 `None`。
    pub fn cell(&self, r: i32, c: i32) -> Option<&Cell> {
        self.cells.iter().find(|x| x.r == r && x.c == c)
    }

    /// 带 `source` 的格数。
    pub fn with_source_count(&self) -> usize {
        self.cells.iter().filter(|x| x.source.is_some()).count()
    }

    /// 不带 `source` 的格数。
    pub fn without_source_count(&self) -> usize {
        self.cells.iter().filter(|x| x.source.is_none()).count()
    }
}

/// 一个非空计算格（`CalcJson.java:64-101` 的 `cellJson`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub r: i32,
    pub c: i32,
    /// `value` 的**原文 JSON token**：字符串含引号、数组含方括号。
    /// 不做数值归一化，理由见 [`super::json`] 模块文档第 1 条。
    pub value_raw: String,
    /// `type` 字段：`string`/`int`/`long`/`double`/`float`/`decimal`/`boolean`/`list`/`null`
    /// （`CalcJson.java:224-253`）。**类型的唯一权威**，不得从 `value_raw` 推断。
    pub declared_type: String,
    /// 仅当显示值 != `String.valueOf(value)` 时写出（`CalcJson.java:71-73`）。
    pub disp_value: Option<String>,
    /// 可选：模板尺寸外的扩展格没有它（`CalcJson.java:74-98`）。
    pub source: Option<Source>,
    /// 可选：该格的左 head，指向**模板坐标**下被引用的那一格
    /// （`{src_row, src_col}`；`(0,0)` = `cell00`）。
    ///
    /// ⚠ **head 目标可以没有 `source`** —— 计算态里存在 `source` 为 null 的格
    /// （如 `engine_head_left` 的 `(11,1)`，它在模板尺寸外）却仍被其它格当 head。
    /// ⇒ **不能用「有没有 `source`」判断「是不是合法 head 目标」**。
    pub left_head: Option<HeadRef>,
    /// 可选：该格的上 head，格式与 [`Cell::left_head`] 相同。
    pub top_head: Option<HeadRef>,
}

/// head 引用的目标格（`{src_row, src_col}`）。
///
/// 用**模板坐标**而非计算态坐标：`(0,0)` 唯一表示 `cell00`，裸数字没有这个约定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadRef {
    pub src_row: i32,
    pub src_col: i32,
}

/// 单元格的来源引用（`CalcJson.java:75-98`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub src_row: i32,
    pub src_col: i32,
    pub row_merge: i32,
    pub col_merge: i32,
    /// `getExtensible()`：16=不扩展，17/18/19 见 `docs/format-contract.md` 附件 H.7.1。
    pub extend: i8,
    pub sub_cells: i32,
    /// 仅当 `sub_cells > 1` 时写出（`CalcJson.java:84-97`）。
    pub sub_values: Option<Vec<SubValue>>,
}

/// 附属格（`CalcJson.java:86-95`）。
#[derive(Debug, Clone, PartialEq)]
pub struct SubValue {
    pub r: i32,
    pub c: i32,
    pub value_raw: String,
    pub declared_type: String,
}

/// 错误型夹具的 `calculated.error`（`GenEngineFixtures.java:185-204`）。
#[derive(Debug, Clone, PartialEq)]
pub struct CalcError {
    pub class: String,
    pub message: String,
    pub cause_class: Option<String>,
    pub cause_message: Option<String>,
    /// 循环引用链，如 `B2-C2-B2-C2`（`GenEngineFixtures.java:206-215`）。
    pub chain: Option<String>,
}

/// 分页结果（`CalcJson.java:108-160` 的 `renderPages`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Pagination {
    pub paper_w: f64,
    pub paper_h: f64,
    pub columns: i32,
    pub page_count: i32,
    pub x_pages: i32,
    pub y_pages: i32,
    pub row_num_per_page: i32,
    pub pages: Vec<Page>,
}

/// 单页（`CalcJson.java:113-143`）。
///
/// ⚠ **同一结构里两种字符串口径，且这是产品自己的口径，不要"顺手统一"**：
/// `first_value`/`last_value` 来自 `String.valueOf(v)` → `putString`
/// （`CalcJson.java:128`、`:130`），是**已解码的普通字符串**；
/// `col1_values` 走 `value(v)`（`:131`），是**带类型的 JSON 原文 token**。
/// 前者是显示口径、后者是值口径 —— 混用会得到 `region` vs `"region"` 这类假不等。
/// 加载器忠实复刻二者，正是为了让下游断言不至于因为"看起来一样"而写错。
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub page: i32,
    pub rows: i32,
    /// 第 1 列首个非空值，**已解码**（无引号）；整页全空则缺省。
    pub first_value: Option<String>,
    /// 第 1 列最后一个非空值，**已解码**（无引号）；整页全空则缺省。
    pub last_value: Option<String>,
    /// 第 1 列所有非空值的 **JSON 原文 token**（字符串含引号）；个数可小于
    /// `rows`，因为 `v == null` 的格被跳过（`CalcJson.java:124-126`）。
    pub col1_values: Vec<String>,
}

/// 合并格汇总（`CalcMergeJson.java:30-71`）。`row_merge<=1 && col_merge<=1` 的格不收录。
#[derive(Debug, Clone, PartialEq)]
pub struct Merged {
    pub page_count: i32,
    pub cells: Vec<MergedCell>,
}

/// 一个合并格（`CalcMergeJson.java:51-58`）。
#[derive(Debug, Clone, PartialEq)]
pub struct MergedCell {
    pub page: i32,
    pub r: i32,
    pub c: i32,
    pub value_raw: String,
    pub row_merge: i32,
    pub col_merge: i32,
}
