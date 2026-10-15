//! 用代码构造一份「非线性中式报表」模板（`ReportDefine`）。
//!
//! # 为什么能在本机手搓模板
//!
//! 产品模板（`.rpx`）平时由 Windows 上的设计器生成——本机没有那条产线。
//! 但 `ReportDefine` 的字段**全部 `pub`**，且所有默认值都能从
//! `rpx_core::props::def`（契约附件 B 的"默认值列"）取到，所以可以纯代码
//! 构造出**合法且能被引擎正确消费**的模板。这就是本实例的价值：
//! 脱开设计器也能造出复杂版面。
//!
//! # 两条必须知道的构造纪律
//!
//! 1. **合并靠锚点格的 `merged_area`，不是 `rd.merges`**。引擎
//!    （`stage_a::seed_body`）只扫每格的 `merged_area` 来铺合并矩形；
//!    `rd.merges` 仅供 `rpx-cli print` 展示。矩形内的非锚点槽在 `grid`
//!    里必须留 `None`（锚点会替它们铺满两张矩阵）。
//! 2. **行高/列宽/断页等"属性表达式"写在 RowCell/ColCell 的 `exp_map` 里**
//!    （行高键 1、断页键 13），值表达式写在 Cell 的 `exp_map` 键 40。

use rpx_core::cell::{Area, Cell};
use rpx_core::colcell::ColCell;
use rpx_core::dataset::{DataSetEntry, SqlDataSetConfig};
use rpx_core::print_setup::PrintSetup;
use rpx_core::props::{bit, def};
use rpx_core::reader::ObjValue;
use rpx_core::report::{MergedArea, ReportDefine, ScalarTail};
use rpx_core::rowcell::RowCell;

/// Cell 的 `exp_map` 里"值表达式"的键（`cell::EXP_MAP` 槽位的 ByteMap 键）。
pub const KEY_VALUE: u8 = 40;
/// RowCell 的 `exp_map` 里 `rowHeight` 的键（`ExtRowCell` 属性码 1）。
pub const KEY_ROW_HEIGHT: u8 = 1;
/// RowCell 的 `exp_map` 里 `breakPage` 的键（属性码 13）。
pub const KEY_ROW_BREAK_PAGE: u8 = 13;
/// ColCell 的 `exp_map` 里 `colWidth` 的键（`ExtColCell` 属性码 11）。
pub const KEY_COL_WIDTH: u8 = 11;

/// 「同值合并」= 纵向（`ExtCellSet.mergeSame` 的 `98`）。
///
/// ⚠ `rpx-core::props::def` 只导出**默认值** `MERGE_SAME = 97`（= 关），
/// `98/99/100`（纵/横/双向）未导出 —— 取值来源见 `rpx-engine/src/merge.rs` 的
/// `SAME_V`/`SAME_H`/`SAME_VH`（`pub(crate)`，故此处按契约字面量复写）。
pub const MERGE_SAME_VERTICAL: i8 = 98;

/// 空白 Cell：全字段 = 契约默认（附件 B.3）。
pub fn blank_cell() -> Cell {
    Cell {
        merged_area: None,
        h_align: def::HALIGN_LEFT,
        v_align: def::VALIGN_MIDDLE,
        diagonal_style: 0,
        diagonal_color: def::COLOR_BLACK,
        diagonal_width: def::DIAGONAL_WIDTH,
        extend_mode: def::EXTEND_DEFAULT,
        extend_area: None,
        left_head: None,
        top_head: None,
        to_excel_mode: def::TO_EXCEL_MODE,
        adjust_size_mode: def::ADJUST_SIZE_MODE,
        cell_obj: None,
        value: ObjValue::Null,
        disp_value: None,
        indent: def::INDENT,
        font_name: None,
        font_size: def::FONT_SIZE,
        format: None,
        fore_color: def::COLOR_BLACK,
        back_color: def::COLOR_WHITE,
        hyperlink: None,
        hyperlink_window: None,
        lb_style: 0,
        lb_width: def::BORDER_WIDTH,
        lb_color: def::COLOR_BLACK,
        rb_style: 0,
        rb_width: def::BORDER_WIDTH,
        rb_color: def::COLOR_BLACK,
        tb_style: 0,
        tb_width: def::BORDER_WIDTH,
        tb_color: def::COLOR_BLACK,
        bb_style: 0,
        bb_width: def::BORDER_WIDTH,
        bb_color: def::COLOR_BLACK,
        exp_map: None,
        bit_flags: bit::DEFAULT_FLAGS,
        cell_type: def::CELL_TYPE,
        notes: None,
        ois_blob: Some(def::OIS_DEFAULT_BYTES.to_vec()),
        cell_graph_config: None,
        under_line_style: 0,
        cell_style_name: None,
        merge_same: def::MERGE_SAME,
        merge_null: def::MERGE_NULL,
        merge_same_mode: def::MERGE_SAME_MODE,
        merge_null_mode: def::MERGE_NULL_MODE,
        rotation_angle: def::ROTATION_ANGLE,
    }
}

/// 静态文本格。
pub fn text_cell(s: &str) -> Cell {
    let mut c = blank_cell();
    c.value = ObjValue::Str(s.to_string());
    c
}

/// 表达式格（值表达式写在 `exp_map` 键 40）。
pub fn expr_cell(expr: &str) -> Cell {
    let mut c = blank_cell();
    c.exp_map = Some(vec![(KEY_VALUE, ObjValue::Str(expr.to_string()))]);
    c
}

/// 空白 RowCell（默认行高 8.0，可见，非断页）。
pub fn blank_row() -> RowCell {
    RowCell {
        row_type: def::ROW_TYPE,
        row_height: def::ROW_HEIGHT,
        row_visible: true,
        break_page: false,
        exp_map: None,
        notes: None,
        group_header_level: 0,
        ois_blob: Some(def::OIS_DEFAULT_BYTES.to_vec()),
        break_column: false,
        adjust_row_h: false,
    }
}

/// 空白 ColCell（默认列宽 25.0，可见）。
pub fn blank_col() -> ColCell {
    ColCell {
        col_type: def::COL_TYPE,
        col_width: def::COL_WIDTH,
        col_visible: true,
        break_page: false,
        exp_map: None,
        notes: None,
        ois_blob: Some(def::OIS_DEFAULT_BYTES.to_vec()),
        break_column: false,
    }
}

/// `ScalarTail` 的契约默认（附件 C.2/C.3 的"默认值"列）。
fn default_scalars() -> ScalarTail {
    ScalarTail {
        disp_ratio: 133,
        unit: 1,
        tip: None,
        submit: 0,
        notes: None,
        future_cell_num_exp: None,
        image_max_width: 0,
        image_max_height: 0,
        page_border_mode: 1,
        report_style_name: None,
        chart_style_name: None,
        validity_script: None,
        big_data_set_name: None,
        password: None,
        high_precision: true,
        threshold: -1,
    }
}

/// 模板构造器：累积 store / grid / merges / row_cells / col_cells。
pub struct Builder {
    rows: usize,
    cols: usize,
    store: Vec<Cell>,
    grid: Vec<Vec<Option<usize>>>,
    merges: Vec<MergedArea>,
    row_cells: Vec<RowCell>,
    col_cells: Vec<ColCell>,
}

impl Builder {
    pub fn new(rows: usize, cols: usize) -> Self {
        Builder {
            rows,
            cols,
            store: Vec::new(),
            grid: vec![vec![None; cols]; rows],
            merges: Vec::new(),
            row_cells: (1..rows).map(|_| blank_row()).collect(),
            col_cells: (1..cols).map(|_| blank_col()).collect(),
        }
    }

    /// 放一格到 `(r,c)`，返回它的 store 下标（`CellId`）。
    pub fn put(&mut self, r: usize, c: usize, cell: Cell) -> usize {
        let id = self.store.len();
        self.store.push(cell);
        self.grid[r][c] = Some(id);
        id
    }

    /// 给已 `put` 的格 `id` 挂「同值合并」属性（`98` 纵向 / `99` 横向 / `100` 双向）。
    ///
    /// 只改 `merge_same`；`merge_same_mode` 保持默认 `118`（判据用 `getSource()`），
    /// 这正合本实例：扩展出的同列格同源 ⇒ 相邻等值即可纵向并成一格。
    pub fn same_merge(&mut self, id: usize, mode: i8) {
        self.store[id].merge_same = mode;
    }

    /// 把 `(r1,c1)..=(r2,c2)` 合并——锚点格必须先 `put`。
    ///
    /// 锚点写 `merged_area`；矩形内其余槽清 `None`（引擎替它们铺满）。
    pub fn merge(&mut self, r1: usize, c1: usize, r2: usize, c2: usize) {
        let id = self.grid[r1][c1].expect("合并锚点必须先 put");
        self.store[id].merged_area = Some(Area {
            begin_row: r1 as i32,
            begin_col: c1 as i32,
            end_row: r2 as i32,
            end_col: c2 as i32,
        });
        for (r, row) in self.grid.iter_mut().enumerate().take(r2 + 1).skip(r1) {
            for (c, slot) in row.iter_mut().enumerate().take(c2 + 1).skip(c1) {
                if (r, c) != (r1, c1) {
                    *slot = None;
                }
            }
        }
        self.merges.push(MergedArea {
            begin_row: r1,
            begin_col: c1,
            end_row: r2,
            end_col: c2,
            cell: id,
        });
    }

    /// 给第 `r` 行（1 起）挂一条属性表达式。
    pub fn row_prop(&mut self, r: usize, key: u8, expr: &str) {
        self.row_cells[r - 1]
            .exp_map
            .get_or_insert_with(Vec::new)
            .push((key, ObjValue::Str(expr.to_string())));
    }

    /// 给第 `c` 列（1 起）挂一条属性表达式。
    pub fn col_prop(&mut self, c: usize, key: u8, expr: &str) {
        self.col_cells[c - 1]
            .exp_map
            .get_or_insert_with(Vec::new)
            .push((key, ObjValue::Str(expr.to_string())));
    }

    /// 收尾：装配 `ReportDefine`（附 SQL 数据集 + 默认 print_setup）。
    pub fn finish(self, dsn: &str, dataset_name: &str, sql: &str) -> ReportDefine {
        let entry = DataSetEntry {
            class_name: "com.raqsoft.reportl.usermodel.SQLDataSetConfig".to_string(),
            factory_class: None,
            file: None,
            sql: Some(SqlDataSetConfig {
                name: Some(dataset_name.to_string()),
                data_source_name: Some(dsn.to_string()),
                release_mode: 0,
                cache_data: false,
                col_titles: None,
                no_data_prompt: false,
                sql: Some(sql.to_string()),
                begin_row: None,
                end_row: None,
                schema: None,
                params: None,
                is_guarded: false,
                threshold: -1,
            }),
            raw: Vec::new(),
        };
        ReportDefine {
            rows: self.rows,
            cols: self.cols,
            row_cells: self.row_cells,
            col_cells: self.col_cells,
            store: self.store,
            grid: self.grid,
            merges: self.merges,
            print_setup: PrintSetup::default_values(),
            params: Vec::new(),
            datasets: vec![entry],
            custom_properties: Vec::new(),
            exp_map: None,
            scalars: default_scalars(),
            macro_meta: None,
            sub_report_meta: None,
            back_graph_config: None,
            export_config: None,
        }
    }
}
