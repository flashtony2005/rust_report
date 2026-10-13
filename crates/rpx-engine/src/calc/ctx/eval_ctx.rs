//! `impl EvalCtx for CalcCtx` —— 求值器（[`crate::expr::evaluate`] 与
//! [`crate::expr::resolve`]）眼中的世界。
//!
//! 全部方法都是薄转发：head 图读 [`CalcCtx::grid`]，两指针读写 ctx 自身字段，
//! DS 旁路读写 `note_row`/`note_list`。没有一处有自己的逻辑 —— 这是有意的：
//! 语义全部在 [`EvalCtx`] 的契约文档与 [`crate::expr::resolve`] 里，这里只负责
//! "把 `ExtCellSet` 的那部分可观察面接到 head 工作台上"。
//!
//! # 与 Java 的对应关系
//!
//! | 本文件 | Java（`ExtCellSet.java`） |
//! |--------|--------------------------|
//! | [`EvalCtx::current`] | `getCurrent()`（`_$21`，`:578-580`） |
//! | [`EvalCtx::saved_current`] | `getSavedCurrent()`（`_$20`，`:590-592`） |
//! | [`EvalCtx::set_current`] | `setCurrent(cell)`（`:578-580`，只写 `_$21`） |
//! | [`EvalCtx::save_current`] | `saveCurrent()`（`:586-588`，`_$20 = _$21`） |
//! | [`EvalCtx::resume_current`] | `resumeCurrent()`（`:594-596`，`_$21 = _$20`） |
//! | [`EvalCtx::source_of`] | `getSource(cell)`（`:623-629`，查**引擎矩阵**） |
//! | [`EvalCtx::source_by_id`] | `getSource(String)`（`:610-621`，按地址查引擎矩阵） |
//!
//! # ⛔ `set_current` 不同步 `grid.current`
//!
//! [`crate::heads::HeadState`] 也有一个 `current` 字段，但它只服务**阶段 B 的
//! head 查找**（`HeadGrid::current()` 用于排除"head-of-head 就是当前格"）。
//! 求值期不再跑 find/normalize，`grid.current` 从此不再被读；写它只会制造
//! 第二份真相。两指针的唯一权威是 [`CalcCtx`] 自身的 `current`/`saved` 字段。
//!
//! # 参数（`EvalCtx::param`）
//!
//! 恒返回 `None`：12 个夹具没有一个带报表参数，`Expr::Param` 携带内联值的路径
//! 已在求值器里消化。将来支持报表参数时在这里接参数表，不要在求值器里加特判。

use crate::arena::CellId;
use crate::expr::ds::DataSet;
use crate::expr::eval::EvalCtx;
use crate::heads::HeadGrid;
use crate::value::Value;

use super::CalcCtx;

impl EvalCtx for CalcCtx {
    fn source_of(&self, c: CellId) -> CellId {
        // `HeadState::source_of`：按 `src_row/src_col` 查 `by_src`（引擎矩阵地址表）。
        self.grid.source_of(c)
    }

    fn left_head(&self, c: CellId) -> CellId {
        self.grid.get(c).map(|h| h.left_head).unwrap_or(CellId::NONE)
    }

    fn top_head(&self, c: CellId) -> CellId {
        self.grid.get(c).map(|h| h.top_head).unwrap_or(CellId::NONE)
    }

    fn left_subs(&self, c: CellId) -> Vec<CellId> {
        // `ExtExtendCell._$6`：仅扩展格与 root 维护（`HeadCell.left_subs` 的文档）。
        self.grid.get(c).map(|h| h.left_subs.clone()).unwrap_or_default()
    }

    fn top_subs(&self, c: CellId) -> Vec<CellId> {
        self.grid.get(c).map(|h| h.top_subs.clone()).unwrap_or_default()
    }

    fn ext_cells(&self, c: CellId) -> Vec<CellId> {
        // `ExtExtendCell._$7`（实例数组）。⛔ 基类契约（`ExtCell.java:172-178`）：
        // `extCellSize()` 恒 **1**、`getExtCell(0) == this` —— 只有 `ExtExtendCell:126`
        // 覆写成 `_$7` 长度。⇒ 非扩展格（或还没扩展的锚点）必须返回 **[自身]**，
        // 不能返回空：`CSVariable._$3/_$2`（`:144-170`）靠 `getExtCell(j)` 扫描，
        // 空 Vec 会让"解析到非扩展格自身"这条路静默失配
        // （实测：engine_sum 的 `sum(B2:B4)` 解析 B4 走 `cell00.top_subs` 命中
        // 空白模板格 (4,2) 自身 —— 返回空时整条解析断成 `UnknownCell{B4}`）。
        match self.grid.get(c).and_then(|h| h.ext_cells_ref()) {
            Some(v) if !v.is_empty() => v.clone(),
            _ => vec![c],
        }
    }

    fn extensible(&self, c: CellId) -> i8 {
        // `getExtensible()`：阶段 B 改写后的生效值（`HeadCell.extend`）。
        self.grid.get(c).map(|h| h.extend).unwrap_or(16)
    }

    fn root(&self) -> CellId {
        CellId::ROOT
    }

    fn current(&self) -> CellId {
        self.current
    }

    fn saved_current(&self) -> CellId {
        self.saved
    }

    fn set_current(&mut self, c: CellId) {
        // 只写 `_$21`（`ExtCellSet.java:578-580`）。不同步 `grid.current`，见模块文档。
        self.current = c;
    }

    fn save_current(&mut self) {
        // `saveCurrent()`（`:586-588`）。⛔ 只有 `ExtNormalCell.calculate:647`
        // 与 `Sum` 的 cellSet 分支调它 —— 不要在别处"顺手"补。
        self.saved = self.current;
    }

    fn resume_current(&mut self) {
        // `resumeCurrent()`（`:594-596`）。
        self.current = self.saved;
    }

    fn cell_value(&self, c: CellId) -> Value {
        // `cell.getValue(false)`：读 `cellValue`；未求值的格是 Null（阶段 B 写入
        // 的字面量格除外 —— 它们从 build 起就带值，与产品"字面量在
        // prepareCalculate 落值"同构）。
        self.value_of(c)
    }

    fn cell_value_checked(&mut self, c: CellId) -> crate::error::Result<Value> {
        // `getValue(false)` 的忠实形态（`ExtNormalCell.java:326-336`）：**无条件
        // testValue()**（`:334`）—— 未算的格被引用 ⇒ 挂起 chase / 二次引用 ⇒
        // CIRCLE。字面量格因阶段 B 已置 bit30 而直接放行（`stage_b` 值写入注释）。
        // 挂起搭载在 [`EngineError::Suspend`] 上穿过求值器（`error.rs` 文档），
        // 由 `cell_calculate` 的出口还原成 `Flow::Suspend`。
        use crate::error::EngineError;
        crate::calc::cell_eval::test_value(self, c, 0).map_err(|f| match f {
            crate::eval::Flow::Suspend(s) => EngineError::Suspend(s),
            crate::eval::Flow::Fatal(e) => e,
        })?;
        Ok(self.value_of(c))
    }

    fn source_by_id(&self, id: &str) -> Option<CellId> {
        // `getSource(String)`（`:610-621`）：地址 → 引擎矩阵格。
        // `HeadGrid::get_source_by_id` 查不到返回 `NONE`，这里翻成 `None`
        // （`resolve_cell` 对 `None` 直接返回"解析不到"）。
        let c = self.grid.get_source_by_id(id);
        if c.is_none() { None } else { Some(c) }
    }

    fn ds(&mut self, name: &str) -> Option<&mut DataSet> {
        self.ds.iter_mut().find(|d| d.name == name)
    }

    fn ds_index(&self, name: &str) -> Option<usize> {
        // 稳定下标：`DsInstance.ds`、DS 旁路、`DsCurrentHost` 全用它寻址。
        // 大小写敏感与 `DataSet.name` 的查表同规（`crate::expr::ds` 模块文档）。
        self.ds.iter().position(|d| d.name == name)
    }

    fn param(&self, _name: &str, _is_macro: bool) -> Option<Value> {
        // 12 夹具无报表参数；见模块文档。
        None
    }

    fn note_ds_row(&mut self, ds: usize, row: usize) {
        self.note_row = Some((ds, row));
    }

    fn note_ds_list(&mut self, ds: usize, rows: &[usize]) {
        self.note_list = Some((ds, rows.to_vec()));
    }

    fn clear_ds_notes(&mut self) {
        // 每次逐格求值开始前调（`cell_eval` 的入口），保证只留"这一次"的记录。
        self.clear_notes();
    }

    fn ds_row_note(&self) -> Option<(usize, usize)> {
        self.note_row
    }

    fn ds_list_note(&self) -> Option<(usize, Vec<usize>)> {
        self.note_list.clone()
    }
}
