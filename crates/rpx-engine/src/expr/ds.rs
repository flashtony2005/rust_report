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
