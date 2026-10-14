//! `RowSet`：数据集**物化契约**（M3-03 定案，`docs/spec-m3.md` §3.4）。
//!
//! 取数方（`rpx-ds` 的 SQL 源、夹具 loader 的 CSV、HTTP 上传的字节流）统一产出
//! 本类型；引擎的求值态 `rpx_engine::expr::ds::DataSet` 经 `From<RowSet>` 持有它
//! 再叠加运行时指针（`DsCur`/`selected`——`DsValue` 缓存语义）。
//!
//! # 为什么单独一个类型
//!
//! `DataSet` 带求值期状态（当前指针、Row 缓存），物化产物**没有**——CSV 字节、
//! SQL 行集都不该知道"数据集正被哪个扩展格逐行消费"。分开后：
//!
//! * `rpx-ds` 只依赖 core（供数方不依赖消费方，依赖方向正确）；
//! * 物化对拍（SQL 行集 ≡ CSV 行集）可以只比 `RowSet`，不必起引擎；
//! * 引擎内 `name`/`cols`/`rows` 三个槽直接转发自 `RowSet`（零拷贝字段级一致，
//!   经 `From` 保证——**单一事实源**，两处平行声明迟早漂移）。
//!
//! # 与产品的对应
//!
//! 产品侧没有独立等价物——Java 的 `FileDataSetFactory`/SQL 工厂直接 `new DataSet(...)`
//! （同名字段一次性灌满，物化与运行时态混在一个构造函数里）。Rust 拆两型是
//! **工程分层**，不是语义偏离：`From` 之后引擎看到的数据与产品完全一致。

/// 一个已物化的数据集：名字 + 列名表 + 类型化行。
#[derive(Debug, Clone, PartialEq)]
pub struct RowSet {
    /// 数据集名（`ds1` 等；取模板 `DataSetConfig.name`，查表键大小写敏感）。
    pub name: String,
    /// 列名表（按声明序；`getData(colName)` 的查表依据）。
    pub cols: Vec<String>,
    /// 行数据（外层 = 行，内层与 `cols` 对齐；CSV/SQL 值已按类型解析）。
    pub rows: Vec<Vec<crate::value::Value>>,
}

impl RowSet {
    /// 从列名与行数据构造。
    pub fn new(name: &str, cols: &[&str], rows: Vec<Vec<crate::value::Value>>) -> Self {
        RowSet {
            name: name.to_string(),
            cols: cols.iter().map(|s| (*s).to_string()).collect(),
            rows,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RowSet;
    use crate::value::Value;

    #[test]
    fn rowset_holds_name_cols_rows_as_declared() {
        let rs = RowSet::new(
            "ds1",
            &["region", "qty"],
            vec![
                vec![Value::Text("North".into()), Value::Int(100)],
                vec![Value::Text("East".into()), Value::Int(200)],
            ],
        );
        assert_eq!(rs.name, "ds1");
        assert_eq!(rs.cols, vec!["region".to_string(), "qty".to_string()]);
        assert_eq!(rs.rows.len(), 2);
        assert_eq!(rs.rows[0][1], Value::Int(100));
        // 类型不归一（M2-14 契约在物化层同样生效）。
        assert_ne!(Value::Int(100), Value::Double(100.0));
    }
}
