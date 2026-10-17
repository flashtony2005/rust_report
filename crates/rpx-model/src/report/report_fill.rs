//! report.rs 的装载辅助（拆出以保持单文件 ≤300 行门禁）。

use super::CellId;
use crate::error::{Result, RpxError};
use crate::param::Param;
use crate::reader::RecordReader;

/// 锚点格读入时把合并区铺满网格（ReportDefine.java:1151-1159）。
/// 行优先循环后续槽位的完整记录会按自身内容覆盖（Java setCell 语义），
/// 因此这里只负责铺，不负责防覆盖。
///
/// `area` 是调用方**已归一化**的 `(begin_row, begin_col, end_row, end_col)`：
/// 保证 end ≥ begin 且四值均在矩阵内，故此处无需再做边界夹取或越界判断。
pub(super) fn fill_merge(
    grid: &mut [Vec<Option<CellId>>],
    area: (usize, usize, usize, usize),
    id: CellId,
) {
    let (r1, c1, r2, c2) = area;
    for row_slots in grid.iter_mut().take(r2 + 1).skip(r1) {
        for slot in row_slots.iter_mut().take(c2 + 1).skip(c1) {
            *slot = Some(id);
        }
    }
}

/// ParamMetaData 记录体：int16 个数 + 逐条 readRecord(Param)（ParamMetaData.java:156-169）。
pub(super) fn read_params(slice: &[u8]) -> Result<Vec<Param>> {
    let mut r = RecordReader::new(slice);
    let count = r.read_i16().map_err(|_| RpxError::prop(6, "param count"))?;
    let mut out = Vec::new();
    if count > 0 {
        for _ in 0..count {
            let rec = r
                .read_record_raw()
                .map_err(|_| RpxError::prop(6, "param record"))?
                .ok_or_else(|| RpxError::prop(6, "empty param slot"))?;
            out.push(Param::read(rec)?);
        }
    }
    Ok(out)
}
