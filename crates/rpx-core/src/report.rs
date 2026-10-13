//! ReportDefine：模板态报表根，按契约附件 C.2 的 24 步字段序装载。
//!
//! 关键点：
//! - 矩阵维度 r/c 含 0 号头行/头列；RowCell/ColCell/NormalCell 循环均从 1 开始；
//! - NormalCell 空槽（int32 -1）= 合并非锚点；锚点格的 Area 铺满整个合并区网格
//!   （各处持同一 CellId，ReportDefine.java:1151-1159）；
//! - cpSize 的 null key 分支：读侧跳过 value 不进表；
//! - _$5 expMap 是 ByteMap 记录；尾部是 boolean highPrecision + int32 threshold，无 md5；
//! - ReportDefine.fillRecord 无 available() 守卫（附件 C.1 勘误⑤）。

/// ParamMetaData 记录体：int16 个数 + 逐条 readRecord(Param)（ParamMetaData.java:156-169）。
/// 合并区铺满与参数表展开在 `report_fill` 子模块。
mod report_fill;

use self::report_fill::{fill_merge, read_params};
use crate::cell::Cell;
use crate::colcell::ColCell;
use crate::dataset::{read_data_set_meta, DataSetEntry};
use crate::error::{Result, RpxError};
use crate::param::Param;
use crate::print_setup::PrintSetup;
use crate::reader::{ObjValue, RecordReader};
use crate::rowcell::RowCell;

/// 网格单元格引用：索引进 ReportDefine::store（合并区各处共享同一 id）。
pub type CellId = usize;

/// 步骤 6-16 的 10 个标量 + 步骤 18-24 的尾部字段（契约 C.2/C.3）。
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarTail {
    pub disp_ratio: i32,                    // _$18，默认 133
    pub unit: i8,                           // _$17，默认 1
    pub tip: Option<String>,                // _$15
    pub submit: i8,                         // _$14，默认 0
    pub notes: Option<String>,              // _$13
    pub future_cell_num_exp: Option<String>, // _$11
    pub image_max_width: i32,               // _$10，默认 0
    pub image_max_height: i32,              // _$9，默认 0
    pub page_border_mode: i8,               // _$8，默认 1
    pub report_style_name: Option<String>,  // _$7
    pub chart_style_name: Option<String>,   // _$12
    pub validity_script: Option<String>,    // _$4
    pub big_data_set_name: Option<String>,  // _$3
    pub password: Option<String>,           // _$2
    pub high_precision: bool,               // _$16，默认 true
    pub threshold: i32,                     // _$1，默认 -1
}

/// 合并区清单条目（锚点坐标 + 网格内共享的 CellId）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergedArea {
    pub begin_row: usize,
    pub begin_col: usize,
    pub end_row: usize,
    pub end_col: usize,
    pub cell: CellId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportDefine {
    /// 矩阵行数（含 0 号头行）
    pub rows: usize,
    /// 矩阵列数（含 0 号头列）
    pub cols: usize,
    /// 索引 = 行号-1（RowCell 循环 1..r-1）
    pub row_cells: Vec<RowCell>,
    /// 索引 = 列号-1（ColCell 循环 1..c-1）
    pub col_cells: Vec<ColCell>,
    /// 单元格存储（合并区各处网格共享同一 id）
    pub store: Vec<Cell>,
    /// rows×cols 模板态网格；None = 空槽（含合并非锚点）
    pub grid: Vec<Vec<Option<CellId>>>,
    pub merges: Vec<MergedArea>,
    pub print_setup: PrintSetup,
    pub params: Vec<Param>,
    pub datasets: Vec<DataSetEntry>,
    /// 步骤 17 的 cpSize 属性表（null key 条目被 Java 读侧丢弃，此处同样只收非 null key）
    pub custom_properties: Vec<(String, Option<String>)>,
    /// 步骤 18 的 _$5 expMap（ByteMap 记录）
    pub exp_map: Option<Vec<(u8, ObjValue)>>,
    pub scalars: ScalarTail,
    /// 四个跳过型 MetaData：保留原始字节（记录存在性，格式展开待 M2）
    pub macro_meta: Option<Vec<u8>>,
    pub sub_report_meta: Option<Vec<u8>>,
    pub back_graph_config: Option<Vec<u8>>,
    pub export_config: Option<Vec<u8>>,
}

impl ReportDefine {
    /// 从 payload（ReportDefine.serialize() 的字节）装载。
    pub fn fill(payload: &[u8]) -> Result<Self> {
        let mut r = RecordReader::new(payload);
        let p = |step: u16, what: &'static str| RpxError::prop(step, what);

        // 步骤 1-2：矩阵维度
        let ri = r.read_i32().map_err(|_| p(1, "rows"))?;
        let ci = r.read_i32().map_err(|_| p(2, "cols"))?;
        if ri < 0 || ci < 0 {
            return Err(RpxError::prop(1, "negative matrix dims"));
        }
        let (rows, cols) = (ri as usize, ci as usize);

        // 抗 DoS（独立验证 2026-10-12 发现）：维度是外部可控的 i32，若不设界，
        // 下面的 `Vec::with_capacity(...)` 与 `vec![vec![None; cols]; rows]` 会被诱导做
        // 10^11 字节级分配并直接 **abort 进程**（不是 Err）——一个 24 字节的
        // `RQQR | len=8 | INT_MAX,1 | footer` 即可复现，违反 AC-02「绝不 panic」。
        // 上界取自格式本身：矩阵每个槽（含空槽）在流里至少占 4 字节 int32 记录长度前缀，
        // RowCell/ColCell 亦然，故合法文件恒满足 rows*cols ≤ payload.len()/4，
        // 该界不会拒绝任何真实模板。
        let max_slots = payload.len() / 4 + 1;
        if rows > max_slots || cols > max_slots || rows.saturating_mul(cols) > max_slots {
            return Err(p(1, "matrix dims exceed payload capacity"));
        }

        // 步骤 3：RowCell（1..r-1）。有效 .rpx 必为完整记录（writer 无条件写出），
        // 空槽视为损坏（Java 侧会得到 null 行属性）。
        // 注意不预分配容量：原先的 `with_capacity(rows-1)` 会让「顶格 rows + 无记录」的
        // 输入触发 22×payload 的尖峰分配（独立验证实测，RowCell 88B / 每 4B payload）。
        // 改为按真实记录数增长，首条记录读失败即返回 Err，攻击者诱导不了任何预分配。
        let mut row_cells = Vec::new();
        for _ in 1..rows {
            let slice = r
                .read_record_raw()
                .map_err(|_| p(3, "rowCell record"))?
                .ok_or_else(|| RpxError::prop(3, "empty rowCell slot"))?;
            row_cells.push(RowCell::read(slice)?.ok_or_else(|| RpxError::prop(3, "empty rowCell record"))?);
        }

        // 步骤 4：ColCell（1..c-1）——同样不预分配（理由见步骤 3）
        let mut col_cells = Vec::new();
        for _ in 1..cols {
            let slice = r
                .read_record_raw()
                .map_err(|_| p(4, "colCell record"))?
                .ok_or_else(|| RpxError::prop(4, "empty colCell slot"))?;
            col_cells.push(ColCell::read(slice)?.ok_or_else(|| RpxError::prop(4, "empty colCell record"))?);
        }

        // 步骤 5：NormalCell 行优先（1..r-1 × 1..c-1）；空槽 = 合并非锚点
        let mut store: Vec<Cell> = Vec::new();
        let mut grid: Vec<Vec<Option<CellId>>> = vec![vec![None; cols]; rows];
        let mut merges: Vec<MergedArea> = Vec::new();
        for row in 1..rows {
            for col in 1..cols {
                let slot = r.read_record_raw().map_err(|_| p(5, "normalCell record"))?;
                let Some(slice) = slot else { continue };
                let Some(cell) = Cell::read(slice)? else { continue };
                let id = store.len();
                store.push(cell);
                let cell = &store[id];
                match cell.merged_area {
                    // 锚点：铺满合并区（ReportDefine.java:1151-1159）
                    Some(area) if area.begin_row == row as i32 && area.begin_col == col as i32 => {
                        // 端点归一化：夹进矩阵内（含 head 行/列之外的负值）且保证 end ≥ begin。
                        // 原实现直接 `area.end_row as usize`，畸形区（如 end_row=-1）会给
                        // MergedArea 留下 1.8e19 级索引，任何按网格消费它的下游都会越界 panic。
                        let last_row = rows.saturating_sub(1);
                        let last_col = cols.saturating_sub(1);
                        let end_row = (area.end_row.max(0) as usize).clamp(row, last_row);
                        let end_col = (area.end_col.max(0) as usize).clamp(col, last_col);
                        fill_merge(&mut grid, (row, col, end_row, end_col), id);
                        merges.push(MergedArea { begin_row: row, begin_col: col, end_row, end_col, cell: id });
                    }
                    // 带 mergedArea 但不在锚点位：Java 读侧 continue，网格留空
                    Some(_) => continue,
                    None => grid[row][col] = Some(id),
                }
            }
        }

        // 步骤 6：7 个 MetaData（PrintSetup 永远非空；null 时 Java 兜底 new PrintSetup()）
        let print_setup = match r.read_record_raw().map_err(|_| p(6, "printSetup record"))? {
            Some(s) => PrintSetup::read(s)?,
            None => PrintSetup::default_values(),
        };
        let macro_meta = r.read_blob().map_err(|_| p(6, "macroMetaData"))?;
        let param_meta = r.read_blob().map_err(|_| p(6, "paramMetaData"))?;
        let data_set_meta = r.read_blob().map_err(|_| p(6, "dataSetMetaData"))?;
        let sub_report_meta = r.read_blob().map_err(|_| p(6, "subReportMetaData"))?;
        let back_graph_config = r.read_blob().map_err(|_| p(6, "backGraphConfig"))?;
        let export_config = r.read_blob().map_err(|_| p(6, "exportConfig"))?;

        // 步骤 7-16：10 个标量
        let disp_ratio = r.read_i32().map_err(|_| p(7, "dispRatio"))?;
        let unit = r.read_i8().map_err(|_| p(8, "unit"))?;
        let tip = r.read_string().map_err(|_| p(9, "tip"))?;
        let submit = r.read_i8().map_err(|_| p(10, "submit"))?;
        let notes = r.read_string().map_err(|_| p(11, "notes"))?;
        let future_cell_num_exp = r.read_string().map_err(|_| p(12, "futureCellNumExp"))?;
        let image_max_width = r.read_i32().map_err(|_| p(13, "imageMaxWidth"))?;
        let image_max_height = r.read_i32().map_err(|_| p(14, "imageMaxHeight"))?;
        let page_border_mode = r.read_i8().map_err(|_| p(15, "pageBorderMode"))?;
        let report_style_name = r.read_string().map_err(|_| p(16, "reportStyleName"))?;

        // 步骤 17：cpSize 属性表；null key 跳过 value 不进表（C.2）
        let cp_size = r.read_i32().map_err(|_| p(17, "cpSize"))?;
        let mut custom_properties = Vec::new();
        if cp_size > 0 {
            for _ in 0..cp_size {
                let key = r.read_string().map_err(|_| p(17, "cp key"))?;
                let Some(key) = key else { continue };
                let value = r.read_string().map_err(|_| p(17, "cp value"))?;
                custom_properties.push((key, value));
            }
        }

        // 步骤 18：_$5 expMap（ByteMap 记录，不是 String）
        let exp_map = match r.read_record_raw().map_err(|_| p(18, "expMap record"))? {
            Some(s) => Some(RecordReader::parse_byte_map(s)?),
            None => None,
        };
        // 步骤 19-22
        let chart_style_name = r.read_string().map_err(|_| p(19, "chartStyleName"))?;
        let validity_script = r.read_string().map_err(|_| p(20, "validityScript"))?;
        let big_data_set_name = r.read_string().map_err(|_| p(21, "bigDataSetName"))?;
        let password = r.read_string().map_err(|_| p(22, "password"))?;
        // 步骤 23-24
        let high_precision = r.read_bool().map_err(|_| p(23, "highPrecision"))?;
        let threshold = r.read_i32().map_err(|_| p(24, "threshold"))?;

        // 参数表与数据集表（从对应 MetaData 记录体展开）
        let params = match &param_meta {
            Some(slice) => read_params(slice)?,
            None => Vec::new(),
        };
        let datasets = match &data_set_meta {
            Some(slice) => read_data_set_meta(slice)?,
            None => Vec::new(),
        };

        Ok(Self {
            rows,
            cols,
            row_cells,
            col_cells,
            store,
            grid,
            merges,
            print_setup,
            params,
            datasets,
            custom_properties,
            exp_map,
            scalars: ScalarTail {
                disp_ratio,
                unit,
                tip,
                submit,
                notes,
                future_cell_num_exp,
                image_max_width,
                image_max_height,
                page_border_mode,
                report_style_name,
                chart_style_name,
                validity_script,
                big_data_set_name,
                password,
                high_precision,
                threshold,
            },
            macro_meta,
            sub_report_meta,
            back_graph_config,
            export_config,
        })
    }

    pub fn row_height(&self, row: usize) -> Option<f32> {
        self.row_cells.get(row.checked_sub(1)?).map(|c| c.row_height)
    }

    pub fn col_width(&self, col: usize) -> Option<f32> {
        self.col_cells.get(col.checked_sub(1)?).map(|c| c.col_width)
    }

    pub fn cell_at(&self, row: usize, col: usize) -> Option<&Cell> {
        self.grid.get(row)?.get(col)?.map(|id| &self.store[id])
    }

    /// 与 expected.json 收录规则对齐的非空格判定（RUNLOG §4.2）。
    pub fn is_blank(&self, row: usize, col: usize) -> bool {
        match self.cell_at(row, col) {
            None => true,
            Some(c) => c.is_default(),
        }
    }
}
