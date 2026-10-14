//! 数据集载入：把 `.rpx` 里 `FileDataSetConfig` 指向的 CSV 读成运行时 [`DataSet`]。
//!
//! # 为什么放在引擎侧
//!
//! 顶层 `calculate()`（`m2-engine-design.md` §5）需要"模板 + 数据集 → 计算态"的
//! 完整输入；模板由 `rpx-core` 解出，数据集则必须由**引擎**按产品口径载入
//! （`Engine.java:129-166` 的 `calcDataSet` 就是把 `FileDataSetFactory` 的产物
//! 塞进 `ctx.getDataSetMap()`）。本模块只做"字节 → [`DataSet`]"这一段，不含求值。
//!
//! # 格式口径（与 `GenEngineFixtures.writeCsv` 同源，逐条对齐）
//!
//! * 分隔符 **TAB**；首行是列名；行尾 **CRLF**（`CalcProbe.writeCsv` 用 `\r\n`）。
//! * 单元格**按字面量解析**：整数 → [`Value::Int`]，其余 → [`Value::Text`]。
//!   这正是 [`crate::expr::ds`] 模块文档写死的契约（`"100"` → `Int`，`"North"` → `Text`），
//!   不是这里临时发明的规则。
//! * 编码：产品按 `FileDataSetConfig.encoding`（**默认 `"GBK"`**，
//!   `FileDataSetConfig.java:25`）解码；`writeCsv`（`CalcProbe.java:277`）也一直
//!   按 GBK 写。M3-02 起本模块按该槽解码（[`crate::encoding`]）：15 个既有夹具
//!   的 CSV 内容全是 ASCII，ASCII 是 GBK 子集 ⇒ 逐字节等价，行为不变。
//!
//! # 有意不做的事（诚实声明）
//!
//! 引号包裹（`quote_removed`）、`col_type_list` 强制类型、`begin_row`/`end_row` 裁剪、
//! 列筛选等 `FileDataSetConfig` 高级槽位**未实现**：夹具的 CSV 是裸 TAB、无引号、
//! 无裁剪，照着实现会写出没有基准可对的代码。这些槽位已在 `rpx-core` 里解析保留，
//! 待有夹具时再补。

use std::path::Path;

use rpx_core::ReportDefine;

use crate::error::{EngineError, Result};
use crate::expr::ds::DataSet;
use crate::value::Value;

/// 载入 `rd` 声明的全部文件数据集；`dir` 是 `.rpx`/`.csv` 所在目录。
///
/// 只处理 `file` 非空且带 `file_name` 的条目（其余配置类无文件可读，跳过）。
/// 数据集名取配置基类的 `name` 槽（`attachFileDataSet` 写入的 `ds1`/`ds2`）。
pub fn load_datasets(dir: &Path, rd: &ReportDefine) -> Result<Vec<DataSet>> {
    let mut out = Vec::new();
    for entry in &rd.datasets {
        let Some(file) = entry.file.as_ref() else {
            continue;
        };
        let Some(file_name) = file.file_name.as_deref() else {
            continue;
        };
        let name = file.name.as_deref().unwrap_or(file_name);
        out.push(load_one(dir, file_name, name, file.encoding.as_deref())?);
    }
    Ok(out)
}

/// 读一个 CSV 文件并建成 [`DataSet`]。`encoding` 是模板声明的 charset
/// （缺省按产品默认 GBK，见 [`crate::encoding`]）。
fn load_one(dir: &Path, file_name: &str, name: &str, encoding: Option<&str>) -> Result<DataSet> {
    let path = dir.join(file_name);
    let bytes = std::fs::read(&path).map_err(|_| EngineError::MissingDataset {
        name: name.to_string(),
    })?;
    let (cols, rows) = parse_csv(&bytes, name, encoding)?;
    let col_refs: Vec<&str> = cols.iter().map(String::as_str).collect();
    Ok(DataSet::new(name, &col_refs, rows))
}

/// TAB + CRLF 的 CSV → `(列名, 行)`。
///
/// 行宽按列数对齐（短行补 [`Value::Null`]、长行截断），避免下游 `row[col]` 越界 panic。
/// 空行（含文件末尾 `\r\n` 产生的尾行）跳过。`encoding` 见 [`crate::encoding`]。
fn parse_csv(
    bytes: &[u8],
    name: &str,
    encoding: Option<&str>,
) -> Result<(Vec<String>, Vec<Vec<Value>>)> {
    let text = crate::encoding::decode(bytes, encoding).map_err(|e| match e {
        // 字节非法：消息里带数据集名，便于定位是哪个文件（变体保留）。
        EngineError::MalformedEncoding { detail } => EngineError::MalformedEncoding {
            detail: format!("数据集 {name}：{detail}"),
        },
        // charset 名不支持：与文件无关，原样透传（产品对应
        // Charset.forName 抛 UnsupportedEncodingException）。
        other => other,
    })?;
    let mut lines = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l));
    let header = lines.next().unwrap_or("");
    let cols: Vec<String> = if header.is_empty() {
        Vec::new()
    } else {
        header.split('\t').map(str::to_string).collect()
    };
    let width = cols.len();
    let mut rows = Vec::new();
    for line in lines {
        if line.is_empty() || width == 0 {
            continue;
        }
        let mut row: Vec<Value> = line.split('\t').map(parse_cell).collect();
        row.resize(width, Value::Null);
        rows.push(row);
    }
    Ok((cols, rows))
}

/// 单元格字面量 → [`Value`]：整数走 `Int`，其余一律 `Text`（见模块文档§格式口径）。
fn parse_cell(text: &str) -> Value {
    match text.parse::<i32>() {
        Ok(i) => Value::Int(i),
        Err(_) => Value::Text(text.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
    }

    #[test]
    fn parse_csv_handles_tab_crlf_and_types() {
        let (cols, rows) =
            parse_csv(b"region\tqty\r\nNorth\t100\r\nEast\t200\r\n", "t", None).unwrap();
        assert_eq!(cols, vec!["region", "qty"]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], Value::Text("North".into()));
        assert_eq!(rows[0][1], Value::Int(100));
        assert_eq!(rows[1][1], Value::Int(200));
    }

    #[test]
    fn parse_csv_pads_and_truncates_to_the_column_count() {
        // 短行补 Null，长行截断：下游 row[col] 不再可能越界。
        let (_, rows) = parse_csv(b"a\tb\nonly-a\nx\ty\tz\n", "t", None).unwrap();
        assert_eq!(rows[0], vec![Value::Text("only-a".into()), Value::Null]);
        assert_eq!(
            rows[1],
            vec![Value::Text("x".into()), Value::Text("y".into())]
        );
    }

    #[test]
    fn reads_a_real_fixture_csv() {
        // 用真实夹具核对（不是手搓期望）：engine_sum.csv 三行，qty 是整数。
        let bytes = std::fs::read(fixtures_dir().join("engine_sum.csv")).unwrap();
        let (cols, rows) = parse_csv(&bytes, "ds1", None).unwrap();
        assert_eq!(cols, vec!["region", "qty"]);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[2][0], Value::Text("South".into()));
        assert_eq!(rows[2][1], Value::Int(150));
    }

    #[test]
    fn reads_a_gbk_csv_with_chinese_columns() {
        // M3-02：GBK 编码的中文列名/数据（0xD6D0=中 0xB1B1=北 0xB7BD=方）。
        // 这是产品默认口径（FileDataSetConfig.encoding 缺省 = "GBK"），
        // 内容形状与 engine_sum.csv 相同、只是非 ASCII。
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0xC7, 0xF8, 0xD3, 0xF2]); // 区域
        bytes.push(b'\t');
        bytes.extend_from_slice(&[0xCA, 0xFD, 0xC1, 0xBF]); // 数量
        bytes.extend_from_slice(b"\r\n");
        bytes.extend_from_slice(&[0xD6, 0xD0]); // 中
        bytes.extend_from_slice(&[0xB1, 0xB1]); // 北
        bytes.push(b'\t');
        bytes.extend_from_slice(b"100\r\n");
        bytes.extend_from_slice(&[0xB7, 0xBD]); // 方（0xB7A2 是"法"，反查表确认）
        bytes.extend_from_slice(&[0xB1, 0xB1]); // 北
        bytes.push(b'\t');
        bytes.extend_from_slice(b"200\r\n");
        let (cols, rows) = parse_csv(&bytes, "ds1", None).unwrap();
        assert_eq!(cols, vec!["区域", "数量"]);
        assert_eq!(rows[0][0], Value::Text("中北".into()));
        assert_eq!(rows[0][1], Value::Int(100));
        assert_eq!(rows[1][0], Value::Text("方北".into()));
        assert_eq!(rows[1][1], Value::Int(200));
        // 显式写 "GBK" 与缺省同路；写 UTF-8 则同一字节必须失败（诚实失败，
        // 不静默乱码——这正是 M3-02 之前 UTF-8 硬编码会走到的坑）。
        assert!(parse_csv(&bytes, "ds1", Some("UTF-8")).is_err());
        assert!(parse_csv(&bytes, "ds1", Some("Big5")).is_err());
    }
}
