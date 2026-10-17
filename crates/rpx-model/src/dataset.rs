//! DataSetMetaData 与 FileDataSetConfig（本轮新逆向，归档为契约附件 E）。
//!
//! DataSetMetaData（DataSetMetaData.java:152-174）：
//!   int16 count + 逐条 { boolean flag; flag 时 String 类名 + readRecord(配置) }
//!   —— flag=false 的条目读侧直接跳过且不进列表。
//! FileDataSetConfig（FileDataSetConfig.java:244-337 写 / :340-418 读）：
//!   基类 DataSetConfig 6 槽（name, dataSourceName, releaseMode, cacheData,
//!   colTitles=readStrings, noDataPrompt）+ 文件槽（fileName, sheetName, encoding,
//!   colList, title, quoteRemoved, seperator, colTypeList, colFormatList, filter,
//!   gatherCol/gatherFun/groupCol/selectCol 四组, changeFile, beginRow, endRow）。
//!   注意：**factory_class 不在流里**——getFactoryClass() 是每配置类的常量返回值
//!   （FileDataSetConfig.java:42-44），需按类名映射。
//!   尾段多组为 available() 守卫（老文件兼容），本批夹具由新 writer 产出必含。

#[cfg(feature = "legacy-rpx")]
use crate::error::{Result, RpxError};
#[cfg(feature = "legacy-rpx")]
use crate::reader::RecordReader;

/// `*DataSetConfig.getFactoryClass()` 的常量返回值（类名 → 工厂类名映射）。
pub fn factory_class_for(class_name: &str) -> Option<&'static str> {
    match class_name {
        "com.raqsoft.reportl.usermodel.FileDataSetConfig" => {
            Some("com.raqsoft.reportl.dataset.FileDataSetFactory")
        }
        "com.raqsoft.reportl.usermodel.SQLDataSetConfig" => {
            Some("com.raqsoft.reportl.dataset.SQLDataSetFactory")
        }
        _ => None,
    }
}

#[cfg(feature = "legacy-rpx")]
fn read_string_list(r: &mut RecordReader) -> Result<Option<Vec<Option<String>>>> {
    let n = r.read_i16()?;
    if n <= 0 {
        return Ok(None);
    }
    let mut v = Vec::with_capacity(n as usize);
    for _ in 0..n {
        v.push(r.read_string()?);
    }
    Ok(Some(v))
}

#[cfg(feature = "legacy-rpx")]
fn read_i32_list(r: &mut RecordReader) -> Result<Option<Vec<i32>>> {
    let n = r.read_i16()?;
    if n <= 0 {
        return Ok(None);
    }
    let mut v = Vec::with_capacity(n as usize);
    for _ in 0..n {
        v.push(r.read_i32()?);
    }
    Ok(Some(v))
}

/// DataSetConfig 基类 6 槽（DataSetConfig.java:108-117 写 / :120-128 读）。
#[cfg(feature = "legacy-rpx")]
pub(crate) struct BaseConfig {
    name: Option<String>,
    data_source_name: Option<String>,
    release_mode: i8,
    cache_data: bool,
    col_titles: Option<Vec<Option<String>>>,
    no_data_prompt: bool,
}

#[cfg(feature = "legacy-rpx")]
pub(crate) fn parse_base(blob: Option<&[u8]>) -> Result<BaseConfig> {
    let Some(slice) = blob else {
        return Ok(BaseConfig {
            name: None,
            data_source_name: None,
            release_mode: 0,
            cache_data: false,
            col_titles: None,
            no_data_prompt: false,
        });
    };
    let mut r = RecordReader::new(slice);
    Ok(BaseConfig {
        name: r.read_string()?,
        data_source_name: r.read_string()?,
        release_mode: r.read_i8()?,
        cache_data: r.read_bool()?,
        col_titles: r.read_strings()?,
        no_data_prompt: r.read_bool()?,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileDataSetConfig {
    // DataSetConfig 基类 6 槽
    pub name: Option<String>,
    pub data_source_name: Option<String>,
    pub release_mode: i8,
    pub cache_data: bool,
    pub col_titles: Option<Vec<Option<String>>>,
    pub no_data_prompt: bool,
    // FileDataSetConfig 文件槽
    pub file_name: Option<String>,
    pub sheet_name: Option<String>,
    pub encoding: Option<String>,
    pub col_list: Option<Vec<Option<String>>>,
    pub title: bool,
    pub quote_removed: bool,
    pub seperator: i16,
    pub col_type_list: Option<Vec<i32>>,
    pub col_format_list: Option<Vec<Option<String>>>,
    pub filter: Option<String>,
    pub gather_col_list: Option<Vec<Option<String>>>,
    pub gather_fun_list: Option<Vec<Option<String>>>,
    pub group_col_list: Option<Vec<Option<String>>>,
    pub select_col_list: Option<Vec<Option<String>>>,
    pub change_file: bool,
    pub begin_row: Option<String>,
    pub end_row: Option<String>,
}

impl FileDataSetConfig {
    #[cfg(feature = "legacy-rpx")]
    fn read(slice: &[u8]) -> Result<Self> {
        let mut r = RecordReader::new(slice);
        let p = |slot: u16, what: &'static str| RpxError::prop(slot, what);
        // 基类 6 槽整体包在 int32 长度 blob 里（FileDataSetConfig.java:248 writeBytes(super.serialize())，
        // 读侧 :343 super.fillRecord(in.readBytes())）
        let base_blob = r.read_blob().map_err(|_| p(0, "base blob"))?;
        let base = parse_base(base_blob.as_deref())?;
        Ok(Self {
            name: base.name,
            data_source_name: base.data_source_name,
            release_mode: base.release_mode,
            cache_data: base.cache_data,
            col_titles: base.col_titles,
            no_data_prompt: base.no_data_prompt,
            file_name: r.read_string().map_err(|_| p(7, "fileName"))?,
            sheet_name: r.read_string().map_err(|_| p(8, "sheetName"))?,
            encoding: r.read_string().map_err(|_| p(9, "encoding"))?,
            col_list: read_string_list(&mut r).map_err(|_| p(10, "colList"))?,
            title: r.read_bool().map_err(|_| p(11, "title"))?,
            quote_removed: r.read_bool().map_err(|_| p(12, "quoteRemoved"))?,
            seperator: r.read_i16().map_err(|_| p(13, "seperator"))?,
            col_type_list: if r.remaining() > 0 {
                read_i32_list(&mut r).map_err(|_| p(14, "colTypeList"))?
            } else {
                None
            },
            col_format_list: if r.remaining() > 0 {
                read_string_list(&mut r).map_err(|_| p(15, "colFormatList"))?
            } else {
                None
            },
            filter: if r.remaining() > 0 {
                r.read_string().map_err(|_| p(16, "filter"))?
            } else {
                None
            },
            gather_col_list: if r.remaining() > 0 {
                read_string_list(&mut r).map_err(|_| p(17, "gatherColList"))?
            } else {
                None
            },
            gather_fun_list: if r.remaining() > 0 {
                read_string_list(&mut r).map_err(|_| p(18, "gatherFunList"))?
            } else {
                None
            },
            group_col_list: if r.remaining() > 0 {
                read_string_list(&mut r).map_err(|_| p(19, "groupColList"))?
            } else {
                None
            },
            select_col_list: if r.remaining() > 0 {
                read_string_list(&mut r).map_err(|_| p(20, "selectColList"))?
            } else {
                None
            },
            change_file: if r.remaining() > 0 {
                r.read_bool().map_err(|_| p(21, "changeFile"))?
            } else {
                false
            },
            begin_row: if r.remaining() > 0 {
                r.read_string().map_err(|_| p(22, "beginRow"))?
            } else {
                None
            },
            end_row: if r.remaining() > 0 {
                r.read_string().map_err(|_| p(23, "endRow"))?
            } else {
                None
            },
        })
    }
}

/// `SQLDataSetConfig`（version=5；`SQLDataSetConfig.java:155-179` 写 / `:181-203` 读）。
///
/// 记录体 = `writeBytes(super.serialize())`（基类 6 槽 blob）+ 平铺 8 槽：
/// `sql` / `beginRow` / `endRow`（**表达式**行窗口，探针版原样保留不求值）/
/// `schema` / `params`（i16 条数 × [exp, type]） / `isGuarded` / `threshold`。
/// ⛔ **连接三要素（driver/url/user/pwd）不在模板里**——模板只存
/// `data_source_name` 引用，连接由运行时按名注入（产品：
/// `ctx.setConnectionFactory(dsn, NormalConnectionFactory)`；配置文件
/// `raqsoftConfig.xml` 的 `<DBList>`）。凭据与模板分离，rpx-server 同构。
#[derive(Debug, Clone, PartialEq)]
pub struct SqlDataSetConfig {
    // DataSetConfig 基类 6 槽
    pub name: Option<String>,
    pub data_source_name: Option<String>,
    pub release_mode: i8,
    pub cache_data: bool,
    pub col_titles: Option<Vec<Option<String>>>,
    pub no_data_prompt: bool,
    // SQLDataSetConfig 专属槽
    pub sql: Option<String>,
    pub begin_row: Option<String>,
    pub end_row: Option<String>,
    pub schema: Option<String>,
    /// `?` 占位参数（表达式 + 类型字节；求值/展开待夹具覆盖）。
    pub params: Option<Vec<(Option<String>, i8)>>,
    pub is_guarded: bool,
    pub threshold: i32,
}

impl SqlDataSetConfig {
    #[cfg(feature = "legacy-rpx")]
    fn read(slice: &[u8]) -> Result<Self> {
        let mut r = RecordReader::new(slice);
        let p = |slot: u16, what: &'static str| RpxError::prop(slot, what);
        let base_blob = r.read_blob().map_err(|_| p(0, "base blob"))?;
        let base = parse_base(base_blob.as_deref())?;
        Ok(Self {
            name: base.name,
            data_source_name: base.data_source_name,
            release_mode: base.release_mode,
            cache_data: base.cache_data,
            col_titles: base.col_titles,
            no_data_prompt: base.no_data_prompt,
            sql: r.read_string().map_err(|_| p(7, "sql"))?,
            begin_row: r.read_string().map_err(|_| p(8, "beginRow"))?,
            end_row: r.read_string().map_err(|_| p(9, "endRow"))?,
            schema: r.read_string().map_err(|_| p(10, "schema"))?,
            params: if r.remaining() > 0 {
                let n = r.read_i16().map_err(|_| p(11, "params size"))?;
                // 产品口径：size==0 时 readExternal 不建列表 ⇒ 字段保持 null
                // （SQLDataSetConfig.java:189-200），Some([]) 是错的形态。
                if n > 0 {
                    let mut v = Vec::new();
                    for i in 0..n {
                        let exp = r.read_string().map_err(|_| p(11 + i as u16, "param exp"))?;
                        let ty = r.read_i8().map_err(|_| p(11 + i as u16, "param type"))?;
                        v.push((exp, ty));
                    }
                    Some(v)
                } else {
                    None
                }
            } else {
                None
            },
            is_guarded: if r.remaining() > 0 {
                r.read_bool().map_err(|_| p(12, "isGuarded"))?
            } else {
                false
            },
            threshold: if r.remaining() > 0 {
                r.read_i32().map_err(|_| p(13, "threshold"))?
            } else {
                0
            },
        })
    }
}

/// DataSetMetaData 条目：M1 解析 FileDataSetConfig 全字段，
/// 其他配置类保留类名 + 原始字节（记录存在性，格式待 M2）。
#[derive(Debug, Clone, PartialEq)]
pub struct DataSetEntry {
    pub class_name: String,
    /// getFactoryClass() 常量映射（不在二进制流里）
    pub factory_class: Option<&'static str>,
    pub file: Option<FileDataSetConfig>,
    pub sql: Option<SqlDataSetConfig>,
    pub raw: Vec<u8>,
}

/// 从 DataSetMetaData 切出的记录体解析整表。
#[cfg(feature = "legacy-rpx")]
pub fn read_data_set_meta(slice: &[u8]) -> Result<Vec<DataSetEntry>> {
    let mut r = RecordReader::new(slice);
    let count = r
        .read_i16()
        .map_err(|_| RpxError::prop(0, "dataSet count"))?;
    let mut out = Vec::new();
    if count > 0 {
        for _ in 0..count {
            let flag = r
                .read_bool()
                .map_err(|_| RpxError::prop(0, "dataSet entry flag"))?;
            if !flag {
                continue; // 读侧跳过且不进列表（DataSetMetaData.java:160-169）
            }
            let class_name = r
                .read_string()
                .map_err(|_| RpxError::prop(0, "dataSet class name"))?
                .unwrap_or_default();
            let raw = r
                .read_record_raw()
                .map_err(|_| RpxError::prop(0, "dataSet record"))?
                .unwrap_or(&[])
                .to_vec();
            let file = if class_name == "com.raqsoft.reportl.usermodel.FileDataSetConfig" {
                Some(FileDataSetConfig::read(&raw)?)
            } else {
                None
            };
            let sql = if class_name == "com.raqsoft.reportl.usermodel.SQLDataSetConfig" {
                Some(SqlDataSetConfig::read(&raw)?)
            } else {
                None
            };
            out.push(DataSetEntry {
                factory_class: factory_class_for(&class_name),
                class_name,
                file,
                sql,
                raw,
            });
        }
    }
    Ok(out)
}

#[cfg(all(test, feature = "legacy-rpx"))]
mod tests {
    use std::fs::File;
    use std::path::Path;

    /// 第 19 号夹具 `engine_sql`（探针 `ProbeSql.java` 连金仓真机产出）：
    /// `.rpx` 里 SQL 数据集记录体的解析（`SqlDataSetConfig` version=5 全槽）。
    /// ⛔ 探针连接三要素不在模板里（`data_source_name` 只是指针）——本断言
    /// 同时钉住「凭据不进模板」这一架构契约。
    #[test]
    fn parses_a_real_sql_dataset_config_from_the_probe_fixture() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let rd = crate::open(File::open(dir.join("engine_sql.rpx")).unwrap()).unwrap();
        assert_eq!(rd.datasets.len(), 1, "探针模板只声明一个 SQL 数据集");
        let e = &rd.datasets[0];
        assert_eq!(
            e.class_name,
            "com.raqsoft.reportl.usermodel.SQLDataSetConfig"
        );
        assert_eq!(
            e.factory_class,
            Some("com.raqsoft.reportl.dataset.SQLDataSetFactory")
        );
        let sql = e.sql.as_ref().expect("SQL 配置应被解析");
        assert_eq!(sql.name.as_deref(), Some("ds1"));
        assert_eq!(sql.data_source_name.as_deref(), Some("kingbase"));
        assert_eq!(
            sql.sql.as_deref(),
            Some("SELECT id, dept, salary FROM rpx_sql_smoke ORDER BY id")
        );
        assert_eq!(sql.begin_row, None);
        assert_eq!(sql.end_row, None);
        assert_eq!(sql.params, None, "探针 SQL 无 ? 占位参数");
        assert!(!sql.is_guarded);
        assert_eq!(sql.threshold, 0);
        // 连接要素不在任何槽里（模板里连 url 字段都没有——把 e.sql 调试
        // 打印也扫一遍等于扫整个记录体：raw 里不该出现 jdbc 字样）。
        let raw_text = String::from_utf8_lossy(&e.raw);
        assert!(
            !raw_text.contains("jdbc"),
            "记录体不得包含连接串（凭据与模板分离）：{raw_text}"
        );
    }
}
