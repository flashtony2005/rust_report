//! 求值器单测：手工假网格 + 内存数据集。
//!
//! CSVariable 解析用例按 `resolve.rs` 模块文档的流水线手工构造 head 图，
//! 断言的是**算法自身的分支语义**（分支命中/实例定位/head 平移）；夹具级
//! 数值回归（`engine_expr` 的 100/200/150 等）等顶层 `calculate()` 接线后做。
//!
//! 假网格的契约（与产品对齐）：
//! - **引擎矩阵本体格的 `source` = 自身**（`getSource()` = `engine.srcCurrent`）；
//! - `ext_cells`：扩展格返回 `_$7` 成员；**非扩展格返回 `[自身]`**
//!   （`ExtCell.getExtCell(0) == this`、`extCellSize()==1`）；
//! - root 的子格列表需要手工登记（产品由 `setCellLeftHead` 时的
//!   `addCellLeftHead` 副作用完成）。

use std::collections::HashMap;

use crate::arena::CellId;
use crate::error::EngineError;
use crate::expr::ds::{DataSet, DsCur};
use crate::expr::eval::{evaluate, EvalCtx};
use crate::expr::parse;
use crate::value::Value;

pub(crate) struct FCell {
    src: CellId,
    lh: CellId,
    th: CellId,
    lsubs: Vec<CellId>,
    tsubs: Vec<CellId>,
    exts: Vec<CellId>,
    ext: i8,
    val: Value,
}

pub(crate) struct Fake {
    cells: Vec<FCell>,
    by_id: HashMap<String, CellId>,
    cur: CellId,
    saved: CellId,
    dss: Vec<DataSet>,
}

impl Fake {
    pub(crate) fn new() -> Self {
        let root = FCell {
            src: CellId::ROOT,
            lh: CellId::NONE,
            th: CellId::NONE,
            lsubs: Vec::new(),
            tsubs: Vec::new(),
            exts: Vec::new(),
            ext: 19,
            val: Value::Null,
        };
        Fake {
            cells: vec![root],
            by_id: HashMap::new(),
            cur: CellId::ROOT,
            saved: CellId::ROOT,
            dss: Vec::new(),
        }
    }
    /// 引擎矩阵本体格：`source` = 自身，head 兜底到 ROOT。
    pub(crate) fn put(&mut self, id: &str, ext: i8, val: Value) -> CellId {
        let cid = CellId(self.cells.len() as u32);
        self.cells.push(FCell {
            src: cid,
            lh: CellId::ROOT,
            th: CellId::ROOT,
            lsubs: Vec::new(),
            tsubs: Vec::new(),
            exts: Vec::new(),
            ext,
            val,
        });
        if !id.is_empty() {
            self.by_id.insert(id.to_string(), cid);
        }
        cid
    }
    /// 扩展实例：`source` = 引擎原点。
    pub(crate) fn put_inst(&mut self, src: CellId, ext: i8, val: Value) -> CellId {
        let cid = CellId(self.cells.len() as u32);
        self.cells.push(FCell {
            src,
            lh: CellId::ROOT,
            th: CellId::ROOT,
            lsubs: Vec::new(),
            tsubs: Vec::new(),
            exts: Vec::new(),
            ext,
            val,
        });
        cid
    }
    pub(crate) fn reg_left(&mut self, head: CellId, sub: CellId) {
        self.cells[head.0 as usize].lsubs.push(sub);
    }
    pub(crate) fn reg_top(&mut self, head: CellId, sub: CellId) {
        self.cells[head.0 as usize].tsubs.push(sub);
    }
    pub(crate) fn ds3(name: &str, col: &str, vals: &[i32]) -> DataSet {
        DataSet::new(
            name,
            &[col],
            vals.iter().map(|v| vec![Value::Int(*v)]).collect(),
        )
    }
    /// 拨当前格指针（`_$21`）。兄弟测试模块不用直接摸 `cur` 字段。
    pub(crate) fn set_cur(&mut self, c: CellId) {
        self.cur = c;
    }
    /// 注册数据集。`tests_aggregate` 等兄弟测试模块要造非 Int 列，走这个入口，
    /// 免得为了复用把 `dss` 字段公开出去。
    pub(crate) fn push_ds(&mut self, ds: DataSet) {
        self.dss.push(ds);
    }
    /// 把第 `i` 个数据集的指针拨到第 `row` 行（Row 态，`DSSum.java:46`）。
    pub(crate) fn ds_set_row(&mut self, i: usize, row: usize) {
        self.dss[i].set_current(DsCur::Row(row));
    }
}

impl EvalCtx for Fake {
    fn source_of(&self, c: CellId) -> CellId {
        self.cells[c.0 as usize].src
    }
    fn left_head(&self, c: CellId) -> CellId {
        self.cells[c.0 as usize].lh
    }
    fn top_head(&self, c: CellId) -> CellId {
        self.cells[c.0 as usize].th
    }
    fn left_subs(&self, c: CellId) -> Vec<CellId> {
        self.cells[c.0 as usize].lsubs.clone()
    }
    fn top_subs(&self, c: CellId) -> Vec<CellId> {
        self.cells[c.0 as usize].tsubs.clone()
    }
    fn ext_cells(&self, c: CellId) -> Vec<CellId> {
        let e = &self.cells[c.0 as usize].exts;
        if e.is_empty() {
            vec![c] // getExtCell(0) == this（ExtCell.java:176-178）
        } else {
            e.clone()
        }
    }
    fn extensible(&self, c: CellId) -> i8 {
        self.cells[c.0 as usize].ext
    }
    fn root(&self) -> CellId {
        CellId::ROOT
    }
    fn current(&self) -> CellId {
        self.cur
    }
    fn saved_current(&self) -> CellId {
        self.saved
    }
    fn set_current(&mut self, c: CellId) {
        self.cur = c;
    }
    fn cell_value(&self, c: CellId) -> Value {
        self.cells[c.0 as usize].val.clone()
    }
    fn source_by_id(&self, id: &str) -> Option<CellId> {
        self.by_id.get(id).copied()
    }
    fn ds(&mut self, name: &str) -> Option<&mut DataSet> {
        self.dss.iter_mut().find(|d| d.name == name)
    }
    fn save_current(&mut self) {
        // `saveCurrent()`（`ExtCellSet.java:586-588`）：快照当前 `_$21`。
        self.saved = self.cur;
    }
    fn resume_current(&mut self) {
        // `resumeCurrent()`（`:594-596`）：`_$21 = _$20`。
        self.cur = self.saved;
    }
    fn ds_index(&self, name: &str) -> Option<usize> {
        self.dss.iter().position(|d| d.name == name)
    }
    fn param(&self, _name: &str, _m: bool) -> Option<Value> {
        None
    }
}

/// 场景 A：`B2` 在扩展实例行上解析到**本行的 B2 实例**。
///
/// `current = C_r`（引用格的实例，source=引擎 C）；引擎 B2 是 18 锚点、
/// 在 root 的左子格列表里 ⇒ 走 `left/top 出处都是 cell00 → extensible==18
/// → via_left_subs` 分支，命中 `A_r`（`isTopHeadOf(ROOT, A_r)` 成立）。
#[test]
fn cell_ref_resolves_to_same_row_instance() {
    let mut f = Fake::new();
    let a = f.put("B2", 18, Value::Int(0)); // 引擎 B2（锚点）
    let a_r = f.put_inst(a, 18, Value::Int(200)); // A 的第 r 个实例
    f.cells[a.0 as usize].exts.push(a_r); // setExtCells（extendRow 的 :994-996）
    let c = f.put("", 16, Value::Null); // 引擎 C（引用 B2 的宿主格）
    let c_r = f.put_inst(c, 16, Value::Null); // C 的第 r 个实例
    f.cells[c_r.0 as usize].lh = a_r; // 实例行上 C_r 的左 head 是 A_r
    f.reg_left(CellId::ROOT, a);
    f.reg_top(CellId::ROOT, a);
    f.cur = c_r;
    let e = parse("B2").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Int(200)));
}

/// 场景 B：`$B2` 用 `saved_current` 而不是 `current`（附件 G.1：`$` = isSaved）。
#[test]
fn dollar_uses_saved_current() {
    let mut f = Fake::new();
    let a = f.put("B2", 18, Value::Int(0));
    let a1 = f.put_inst(a, 18, Value::Int(111)); // current 指向的实例
    let a2 = f.put_inst(a, 18, Value::Int(222)); // saved 指向的实例
    f.reg_left(CellId::ROOT, a);
    f.reg_top(CellId::ROOT, a);
    f.cur = a1;
    f.saved = a2;
    let e = parse("$B2").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Int(222)));
    let e2 = parse("B2").unwrap();
    assert!(matches!(evaluate(&e2, &mut f).unwrap(), Value::Int(111)));
}

/// `sum(B2:B4)` ⛔ **不是区域求和**（附件 H.1）：把 B4 设为 current 后求值 B2 一次。
#[test]
fn sum_link_is_not_excel_range() {
    let mut f = Fake::new();
    let b2 = f.put("B2", 16, Value::Int(100));
    let b4 = f.put("B4", 16, Value::Int(999));
    f.reg_top(CellId::ROOT, b2);
    f.reg_top(CellId::ROOT, b4);
    f.cur = b2;
    f.saved = b2; // 产品：saveCurrent() 在 sum 入口把 calculating cell 存进 _$20
    let e = parse("sum(B2:B4)").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Int(100)));
    // ⛔ current 已被 resumeCurrent 恢复到 saved 快照（`Sum.java:204-206`）
    assert_eq!(f.cur, b2);
}

/// `sum(List)` 聚合 + 空表 → `Double(0.0)` + 标量透传（`Sum.java:106-133`）。
#[test]
fn sum_list_aggregate_and_passthrough() {
    let mut f = Fake::new();
    let e = parse("sum(ds1.select(qty))").unwrap();
    f.dss.push(Fake::ds3("ds1", "qty", &[100, 200, 150]));
    // Root 态：select → List[100,200,150] → 450
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Int(450)));
    // 空表 → Double(0.0)（产品回退路径，附件 H.1）
    f.dss[0].rows.clear();
    f.dss[0].selected.clear();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Double(_)));
}

/// DS 双态（附件 H.2）：Row 态逐行、Root 态全量。
#[test]
fn ds_select_row_vs_root_modes() {
    let mut f = Fake::new();
    f.dss.push(Fake::ds3("ds1", "qty", &[100, 200, 150]));
    let e = parse("ds1.select(qty)").unwrap();
    // Root 态 → 全量 List
    assert!(matches!(
        evaluate(&e, &mut f).unwrap(),
        Value::List(ref l) if l.len() == 3
    ));
    // Row 态 → 当前行值（指针由调度器推进）
    f.dss[0].set_current(DsCur::Row(1));
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Int(200)));
    // `ds1.sum(qty)` Row 态 = 当前行值（DSSum.java:41-47 的退化路径）
    let s = parse("ds1.sum(qty)").unwrap();
    assert!(matches!(evaluate(&s, &mut f).unwrap(), Value::Int(200)));
    // `ds2.count()` Root 态 = 行数
    f.dss.push(Fake::ds3("ds2", "qty", &[1, 2, 3]));
    let c = parse("ds2.count()").unwrap();
    assert!(matches!(evaluate(&c, &mut f).unwrap(), Value::Int(3)));
}

/// 类型保真（附件 H.3）：Int 不提升为 Double。
#[test]
fn arithmetic_keeps_int_type() {
    let mut f = Fake::new();
    let b2 = f.put("B2", 16, Value::Int(100));
    f.cur = b2;
    let e = parse("B2*2").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Int(200)));
    let e2 = parse("6*1").unwrap();
    assert!(matches!(evaluate(&e2, &mut f).unwrap(), Value::Int(6)));
    // `$B2*3`
    f.saved = b2;
    let e3 = parse("$B2*3").unwrap();
    assert!(matches!(evaluate(&e3, &mut f).unwrap(), Value::Int(300)));
    // 除法走 Double（Variant2 数值域）
    let e4 = parse("7/2").unwrap();
    assert!(matches!(evaluate(&e4, &mut f).unwrap(), Value::Double(d) if (d - 3.5).abs() < 1e-9));
}

/// 引用了不存在的格号 → Null（Java getCell()==null ⇒ getValue null）；
/// 不存在的数据集 → MissingDataset。
#[test]
fn missing_refs_degrade_gracefully() {
    let mut f = Fake::new();
    let e = parse("Z9").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
    let e2 = parse("ds9.select(qty)").unwrap();
    assert!(matches!(
        evaluate(&e2, &mut f),
        Err(EngineError::MissingDataset { .. })
    ));
}
