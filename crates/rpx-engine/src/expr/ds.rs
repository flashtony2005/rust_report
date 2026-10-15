//! 运行时数据集：求值期的 `DataSet` / `DsValue`（Row | Group）双态的最小模型。
//!
//! Java 侧结构（`reportlite/05-core-runtime/com/raqsoft/reportl/dataset/`）：
//! - `DsValue`（抽象基类，`DsValue.java:16-44`）：持 `ds` + 缓存 `value`
//!   （`setValue`/`getValue`）—— **Row 路径的返回值就藏在它的 `value` 缓存里**
//!   （`DSSelect.java:145-149`：`row.setValue(selectExp.calculate(ctx)); return cur;`
//!   ⇒ 引擎侧再用 `Variant2.getValue` 把 Row 拆回缓存值）；
//! - `Row.evaluate`（`Row.java:97-110`）：保存 ds 当前指针 → `setCurrent(this)` →
//!   求值 → finally 恢复；
//! - `Group.getRows()`：对每行 `evaluateRows` 后的**值列表**（`DSSelect` 聚合路径）。
//!
//! # 双态语义（附件 H.2 的机制根源）
//!
//! | `current` | `ds1.select(qty)` 的行为 | 结果 |
//! |-----------|--------------------------|------|
//! | `Row(i)`（数据集被扩展格引用，指针随实例推进） | 当前行的 `setValue(求 qty)` → 返回 Row ⇒ 拆包为该行 qty | **逐行** 100/200/150 |
//! | `Root`（未被引用） | 根 Group 全量 `evaluateRows` ⇒ List | **全量** ["R1".."R5"] |
//!
//! 指针的推进**不在调度器的 `prepare_eval` 钩子里**（那是 `_$19.setCell`，它只是
//! 「(leftHead, topHead) 对是否变了」的跟踪器，`lIlIlllIIllllIII.java:21-39`）。
//! 真正的推进在**每格的 `calculate` 内部**：`isDSFunction && isMainChanged()` 时调
//! `setDSCurrent(ctx)` → 全 ctx 的 DS 先清 null，再沿本格的 left/top head 链让**扩展格**
//! 把自己当前实例（Row）灌进自己绑定的 DataSet，最后「只填空」地把剩下的填成 Root。
//! 全套 `file:line` 与 4 组实测对照见 `docs/m2-engine-design.md` **§5.2**。
//! ⛔ 早先本文件写的"推进由 `_$19.setCell` 做"是**错的**，已按源码更正。

use crate::value::Value;

/// `DsValue` 的两种形态：根 Group（全量）或某一行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DsCur {
    /// 根 Group（`ds.setCurrent(ds.getRootGroup(), false)`，`ExtCellSet.java:572-574`）。
    Root,
    /// 当前指向第 `0` 起第 `i` 行（`Row`）。
    Row(usize),
}

/// 一个运行时数据集。
#[derive(Debug, Clone)]
pub struct DataSet {
    /// 数据集名（`ds1` 等；查表键，大小写敏感与 `FunctionLib` 同规）。
    pub name: String,
    /// 列名表（按声明序；`getData(colName)` 的查表依据）。
    pub cols: Vec<String>,
    /// 行数据（外层 = 行，内层与 `cols` 对齐）。CSV 夹具的值已按类型解析
    /// （"100" → `Int`，"North" → `Text`）。
    pub rows: Vec<Vec<Value>>,
    /// 当前 DsValue 指针。初始 `Root`。
    pub current: DsCur,
    /// Row 路径的 `DsValue.value` 缓存（`row.setValue(...)` 的落点；
    /// 下标与 `rows` 对齐）。`select`/`sum` 的 Row 路径先写这里再返回行。
    pub selected: Vec<Option<Value>>,
}

impl DataSet {
    /// 从列名与行数据构造；指针置 `Root`，缓存清空。
    pub fn new(name: &str, cols: &[&str], rows: Vec<Vec<Value>>) -> Self {
        let n = rows.len();
        DataSet {
            name: name.to_string(),
            cols: cols.iter().map(|s| s.to_string()).collect(),
            rows,
            current: DsCur::Root,
            selected: vec![None; n],
        }
    }

    /// 从物化产物构造（M3-03 定案 `docs/spec-m3.md` §3.4：`rpx-ds` 产出
    /// [`rpx_core::rowset::RowSet`]，引擎经此唯一入口进入求值态）。
    ///
    /// `name`/`cols`/`rows` 三槽**字段级转发**（单一事实源）；运行时态
    /// `current`/`selected` 初始化为 Root/清空——与 [`DataSet::new`] 完全一致。
    pub fn from_rowset(rs: rpx_core::rowset::RowSet) -> Self {
        let n = rs.rows.len();
        DataSet {
            name: rs.name,
            cols: rs.cols,
            rows: rs.rows,
            current: DsCur::Root,
            selected: vec![None; n],
        }
    }

    /// 列名 → 下标（大小写敏感，产品 `getData(String)` 同规）。
    pub fn col_index(&self, col: &str) -> Option<usize> {
        self.cols.iter().position(|c| c == col)
    }

    /// 整列的值（Root 路径的 `Group.getRows()` 产物）。
    pub fn column(&self, col: usize) -> Vec<Value> {
        self.rows.iter().map(|r| r[col].clone()).collect()
    }

    /// `ds.setCurrent(...)`。
    pub fn set_current(&mut self, cur: DsCur) {
        self.current = cur;
    }

    /// 当前行下标（`Row` 形态才有效）。
    pub fn current_row(&self) -> Option<usize> {
        match self.current {
            DsCur::Row(i) => Some(i),
            DsCur::Root => None,
        }
    }

    /// 当前行下标，**仅当在行数范围内**。
    ///
    /// M5-01：`ds.select` 过滤未命中时产品返回 `getNullRow()`，格把 `_$4` 钉成
    /// 空行 ⇒ 本仓 `note_ds_row(ds, NULL_ROW_NOTE)` → calc 镜像成
    /// `DsCur::Row(usize::MAX)`。此时**任何** `selected[i]` 直写都会越界 panic，
    /// 故 Row 路径统一过这道闸（产品对 NullRow 的 `setValue` 走 rowNo=-1 的
    /// 边缘，无夹具 ⇒ 本仓显式按空值返回，见 `dsselect.rs` 的 Row 态分支）。
    pub fn current_row_checked(&self) -> Option<usize> {
        match self.current {
            DsCur::Row(i) if i < self.rows.len() => Some(i),
            _ => None,
        }
    }

    /// 当前行的某列值（`DsVariable` 在 Row 形态下的取值）。
    pub fn current_col_value(&self, col: usize) -> Option<Value> {
        let i = self.current_row()?;
        self.rows.get(i).map(|r| r[col].clone())
    }

    /// Row 路径返回值的拆包：`DsValue.value` 缓存优先，退回该行原始列值。
    /// （产品在求值链上用 `Variant2.getValue` 把 Row 拆成它的缓存值。）
    pub fn unwrap_row(&self, i: usize) -> Value {
        self.selected
            .get(i)
            .cloned()
            .flatten()
            .unwrap_or(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 物化契约 → 求值态的唯一入口（`spec-m3.md` §3.4）：name/cols/rows 三槽
    /// 字段级一致；运行时态初始化与 `new` 完全相同（Root/清空）。
    #[test]
    fn from_rowset_carries_materialized_slots_and_resets_runtime_state() {
        let rs = rpx_core::rowset::RowSet::new(
            "ds1",
            &["region", "qty"],
            vec![
                vec![Value::Text("North".into()), Value::Int(100)],
                vec![Value::Text("East".into()), Value::Int(200)],
            ],
        );
        let ds = DataSet::from_rowset(rs);
        assert_eq!(ds.name, "ds1");
        assert_eq!(ds.cols, vec!["region".to_string(), "qty".to_string()]);
        assert_eq!(ds.rows[1][1], Value::Int(200));
        assert_eq!(ds.current, DsCur::Root);
        assert_eq!(ds.selected, vec![None, None]);
        // 与 new 直构等价（两条入口不许分叉）。
        let twin = DataSet::new("ds1", &["region", "qty"], ds.rows.clone());
        assert_eq!(twin.name, ds.name);
        assert_eq!(twin.cols, ds.cols);
        assert_eq!(twin.current, ds.current);
        assert_eq!(twin.selected, ds.selected);
    }
}
