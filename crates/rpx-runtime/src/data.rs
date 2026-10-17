//! 将不同文件与数据库的结果集统一物化，按数据集名称绑定到引擎。
use crate::{execution::RenderError, form::RenderRequest};
use rpx_core::{rowset::RowSet, ReportDefine};
use rpx_engine::{expr::ds::DataSet, EngineError};
use std::collections::{HashMap, HashSet};

pub(crate) fn load(
    req: &RenderRequest,
    rd: &ReportDefine,
    sql_rows: &HashMap<(String, String), RowSet>,
) -> Result<Vec<DataSet>, RenderError> {
    let mut datasets = Vec::new();
    let mut names = HashSet::new();
    for entry in &rd.datasets {
        let (name, mut rows) = if let Some(file) = &entry.file {
            let filename = file
                .file_name
                .as_deref()
                .ok_or_else(|| RenderError::Data("文件数据集没有文件名".into()))?;
            let name = file.name.as_deref().unwrap_or(filename);
            let bytes = req
                .datasets
                .get(filename)
                .ok_or_else(|| RenderError::MissingFile {
                    file: filename.into(),
                    dataset: name.into(),
                })?;
            let parsed = if rpx_ds::file_source::is_excel(filename) {
                rpx_ds::file_source::excel(bytes, file.sheet_name.as_deref())
            } else {
                let text = rpx_engine::encoding::decode(bytes, file.encoding.as_deref())
                    .map_err(RenderError::Engine)?;
                let delimiter = u8::try_from(file.seperator)
                    .map_err(|_| RenderError::Data(format!("数据集 {name} 的分隔符无效")))?;
                rpx_ds::file_source::delimited(&text, delimiter)
            }
            .map_err(|e| RenderError::Data(format!("数据集 {name}（{filename}）：{e}")))?;
            (name.to_string(), parsed)
        } else if let Some(sql) = &entry.sql {
            let name = sql.name.as_deref().unwrap_or("<unnamed-sql>");
            let key = (
                sql.data_source_name.clone().unwrap_or_default(),
                sql.sql.clone().unwrap_or_default(),
            );
            let rows = sql_rows.get(&key).cloned().ok_or_else(|| {
                RenderError::Engine(EngineError::SqlDatasetNoConnection {
                    name: name.into(),
                    dsn: key.0,
                })
            })?;
            (name.to_string(), rows)
        } else {
            return Err(RenderError::Data(format!(
                "不支持的数据集配置：{}",
                entry.class_name
            )));
        };
        if !names.insert(name.clone()) {
            return Err(RenderError::Data(format!("数据集名称重复：{name}")));
        }
        rows.name = name;
        datasets.push(DataSet::from_rowset(rows));
    }
    Ok(datasets)
}
