//! DS current 生命周期 —— 「谁把 `ds.current` 变成 `Row`」的逐行移植。
//!
//! 算法正文：`docs/m2-engine-design.md` **§5.2**（触发器 / 动作 / 谁能灌 Row /
//! `_$4`·`_$3` 是谁 / 5.2.5 四组实测对照 / 5.2.6 产品探针私有状态实录）。
//!
//! Java 出处：
//! - `lIlIlllIIllllIII.java:21-53` —— `_$19` 跟踪器（[`HeadPairTracker`]）
//! - `ExtCellSet.java:550-576` —— `isMainChanged` / `resetDSCurrent` / `setDSCurrent`
//! - `ExtNormalCell.java:602-613`、`ExtExtendCell.java:250-312` —— 三处 `setDSCurrent*`
//! - `ExtNormalCell.java:647-651`、`ExtExtendCell.java:373-376` —— 触发点
//! - `DataSet.java:336-376` —— `isLeftTop()` / `getCurrent()` / `setCurrent(g, flag)`
//! - `Row.java:68-79` —— `cross`（只移植 `Row` 做接收者的两分支）
//!
//! # 一句话
//!
//! `ds.current` 是**全 ctx 共享的可变状态**。每遇到一个
//! `isDSFunction && isMainChanged()` 的格，就先把**所有** DS 的指针清空，再沿该格的
//! left/top head 链让链上的**扩展格**把自己当前代表的实例（`Row`）灌进自己绑定的 DS，
//! 最后把仍是 `null` 的 DS 填成根 Group（= 全量聚合）。
//! ⇒ 「逐行 / 全量」的差别**不是** DS 自己决定的，而是「它有没有被 head 链覆盖到」。
//!
//! # ⛔ 五条别写错（每条都有出处，且都被实测锚点钉住）
//!
//! 1. **`_$19` 不是推进器**：它只记 `(leftHead, topHead)` 对变没变（`:21-39`），
//!    不做任何 DS 操作。旧版 `expr/ds.rs` 的模块文档写错了，已更正。
//! 2. **`resetDSCurrent` 清的是 ctx 里的全部 DS**，不是"当前格的那个"（`:556-563`）。
//!    所以 `engine_sum` 的 `(2,7) ds2.sum(qty)` 被**顺带**清成 null ⇒ 全量 450。
//! 3. **`ExtExtendCell.setDSCurrent:295-304` 的 self 段没有 `current == null` 守卫**
//!    （与 `setDSCurrentLeftHead:251/:257` 不同）；而 `ExtNormalCell` **压根没有**
//!    `_$4`/`_$3` 字段 ⇒ 它没有 self 段是"不存在"而不是"漏抄"。
//! 4. **只有扩展格会灌 Row**：`setDSCurrentLeftHead`/`setDSCurrentTopHead` 的基类实现是
//!    空的（`ExtCell.java:215-219`），仅 `ExtExtendCell` 覆写。
//! 5. **`resetDSCurrent` 把 `is_left_top` 置真**（`ds.setCurrent(null, true)`，`:561`），
//!    所以 `setDSCurrentTopHead` 的 `ds.isLeftTop()` 守卫在"没被 top 头写过"时**通过** ——
//!    这不是"数据集天生是 leftTop"。`isLeftTop()` 读的就是 `_$4`（`DataSet.java:336-338`）。
//! 6. ⛔ **`_$4` 与 `_$7` 必须是两个独立字段**：产品探针实测 `engine_sum` 的 `(2,2)`
//!    是"**跟随型**"扩展格（`_$7 == null` 却有 per-instance 的 `_$4`）。
//!    用"只遍历 `_$7` 成员"的简化写法会漏掉这一类格 ⇒ `(2,5)/(2,6)` 退化成 450。

mod types;

#[cfg(test)]
mod tests;

pub use types::{DsInstance, DsSpot, HeadPairTracker, NULL_ROW};

use crate::arena::CellId;
use crate::error::Result;

/// 三处 `setDSCurrent*` / `resetDSCurrent` 需要的可观察面（`ExtCellSet` + `Context`）。
///
/// 拆成 trait 的意义与 [`crate::eval::SchedGrid`] 相同：单测可注入脚本化假实现，
/// 生产版由 `CalcCtx` 实现。
pub trait DsCurrentHost {
    /// `cell00`（`_$10`）。
    fn cell00(&self) -> CellId;
    /// `cell.getCellLeftHead()`（模板声明字符串与运行时字段都已归一到 `HeadCell.left_head`）。
    fn left_head(&self, c: CellId) -> CellId;
    /// `cell.getCellTopHead()`。
    fn top_head(&self, c: CellId) -> CellId;
    /// 是否 `ExtExtendCell`（**只有它**覆写了两个 `setDSCurrent*Head`）。
    fn is_extender(&self, c: CellId) -> bool;
    /// 本格的 `_$4`（"跟随型"扩展格也可能有，见模块文档第 6 条）。
    fn instance(&self, c: CellId) -> Option<DsInstance>;
    /// 本格的 `_$7`（Java 里的局部名是 `_$3`）；非扩展格为空。
    /// `Arc` 是共享分配 —— 实现方应 `Arc::clone`（O(1)），不得展开成新 `Vec`。
    fn ext_cells(&self, c: CellId) -> std::sync::Arc<[CellId]>;
    /// ctx 里 DataSet 的个数（`ctx.getDataSetMap()` 的遍历面）。
    fn ds_count(&self) -> usize;
    /// `ds.getCurrent()` / `isLeftTop()` / `getCurrentRow()` 三件套。
    fn ds_spot(&self, ds: usize) -> DsSpot;
    /// `ds.setCurrent(v, isLeftTop)`（`:358-366`）：写 `_$6`/`_$4`，并把 `_$5` 复位成 1。
    fn ds_set_current(&mut self, ds: usize, v: Option<DsInstance>, is_left_top: bool);
    /// `ds.getRootGroup()`（`:344-352`）。
    fn ds_root(&self, ds: usize) -> DsInstance;
    /// `ds.getNullRow()`（`:82`）。
    fn ds_null_row(&self, ds: usize) -> DsInstance;
    /// `Row.cross(other)`（`Row.java:68-79`）。`other` 恒是一个 `Row` 实例
    /// （`_$4` 只会被写成 `Row`）；接收者也可以是 `Group`，那条分支本 crate 不移植。
    fn ds_row_cross(
        &mut self,
        ds: usize,
        recv: DsInstance,
        other: DsInstance,
    ) -> Result<DsInstance>;
    /// `_$19.setChanged(false)` —— `resetDSCurrent` 的第一句（`:555`）。
    fn tracker_set_changed(&mut self, changed: bool);
}

/// `ExtCellSet.resetDSCurrent(ctx)`（`:554-564`）。
///
/// ⛔ 清的是 **ctx 里的全部 DS**；第二参数 `true` 会把 `is_left_top` 一起置真。
pub fn reset_ds_current<H: DsCurrentHost + ?Sized>(h: &mut H) {
    h.tracker_set_changed(false);
    for ds in 0..h.ds_count() {
        h.ds_set_current(ds, None, true);
    }
}

/// `ExtCellSet.setDSCurrent(ctx)`（`:566-576`）：**只填空**。
///
/// 已经是 `Row` 的 DS 不动 —— 这是「head 链命中的 DS 保持逐行、没命中的退化成全量」
/// 的机制本体。
pub fn set_ds_current<H: DsCurrentHost + ?Sized>(h: &mut H) {
    for ds in 0..h.ds_count() {
        if h.ds_spot(ds).value.is_none() {
            let root = h.ds_root(ds);
            h.ds_set_current(ds, Some(root), false);
        }
    }
}

/// `ExtExtendCell.setDSCurrentLeftHead()`（`:250-261`）；基类是空实现（`ExtCell.java:215-217`）。
///
/// ⛔ 两条写入都有 `current == null` 守卫；`_$4` 与 `_$7` 两段**都要走**
/// （跟随型扩展格只靠前一段）。
///
/// ⚡ **惰性短路**（2026-10-22 实测后加）：`_$3` 成员扫描里每条写入都有
/// `value.is_none()` 守卫，且扫描过程中槽位**只增不减**（写入必是 `Some`）⇒
/// 「全部槽位已填」时剩下的扫描**可证明**是空转 —— 每次迭代的 `instance(e)`
/// 是一次对 `cells` 数组的随机读（`HeadCell` 约 432 B，50k 行时必穿 cache），
/// 白扫的代价是 n² 次随机读。2000 行真实明细模板实测：成员扫描
/// **4,000,000 次 = 2000²**、全部命中实例、**0 次写入**（self 段的 `_$4` 早已
/// 把唯一的 DS 槽填满），`set_ds_current_left_head` 一个人占 `channel5` 的 95%。
/// 判据用**派生**的 [`ds_filled`] 而不是宿主计数器：平行状态漏一处同步就是静默
/// 错值（`ctx.rs` 模块文档同款立场），而 ds 个数极小（实测模板 1 个），逐槽数
/// 可忽略。正确性证据：关闭短路重跑 A/B，`report.csv` 与 `workbook.json` 逐字节
/// 一致；钉子 `ds_current::tests::left_head_scan_*` 三条（该早退要早退 / 不能
/// 过早退 / 有空槽必须扫到尾），变异测试（改回 `false &&`）按预期红两条。
pub fn set_ds_current_left_head<H: DsCurrentHost + ?Sized>(h: &mut H, cell: CellId) {
    if !h.is_extender(cell) {
        return;
    }
    if let Some(inst) = h.instance(cell) {
        if h.ds_spot(inst.ds).value.is_none() {
            h.ds_set_current(inst.ds, Some(inst), true);
        }
    }
    // 短路一：self 段已填满全部槽位 ⇒ 成员扫描不可能再触发任何写入。
    if ds_filled(h) == h.ds_count() {
        return;
    }
    let ext = h.ext_cells(cell);
    for &e in ext.iter() {
        if let Some(inst) = h.instance(e) {
            if h.ds_spot(inst.ds).value.is_none() {
                h.ds_set_current(inst.ds, Some(inst), true);
                // 短路二：本次写入后全部槽位已填 ⇒ 同理收工。
                if ds_filled(h) == h.ds_count() {
                    break;
                }
            }
        }
    }
}

/// `spots` 里 `value.is_some()` 的槽位数（**派生**，不同步任何计数器）。
///
/// 唯一消费者是 [`set_ds_current_left_head`] 的惰性短路；语义 correctness 不依赖它
/// （去掉短路只是变慢，见该函数的实测注记）。
fn ds_filled<H: DsCurrentHost + ?Sized>(h: &H) -> usize {
    (0..h.ds_count())
        .filter(|&i| h.ds_spot(i).value.is_some())
        .count()
}

/// `ExtExtendCell.setDSCurrentTopHead()`（`:264-289`）；基类是空实现。
///
/// ⚠ `ds.isLeftTop()` 守卫读的是 `_$4`（模块文档第 5 条），两条分支都**不**做
/// `current == null` 判断 —— 有 current 时走 `cross`，没有才直接写。
pub fn set_ds_current_top_head<H: DsCurrentHost + ?Sized>(h: &mut H, cell: CellId) -> Result<()> {
    for inst in members_of(h, cell) {
        if !h.ds_spot(inst.ds).is_left_top {
            continue;
        }
        match h.ds_spot(inst.ds).value {
            Some(cur) => {
                let crossed = h.ds_row_cross(inst.ds, cur, inst)?;
                h.ds_set_current(inst.ds, Some(crossed), false);
            }
            None => h.ds_set_current(inst.ds, Some(inst), false),
        }
    }
    Ok(())
}

/// `cell.setDSCurrent(ctx)`（`ExtNormalCell.java:602-613` / `ExtExtendCell.java:291-312`）。
///
/// ⛔ 三段顺序即契约：**self 段 → left 链 → top 链 → 只填空**。
/// self 段只有 `ExtExtendCell` 有，且**无 `current == null` 守卫**（模块文档第 3 条）。
/// ⛔ head 链**一路走到 `null`**（`cell00` 也在链上，只是它是空实现效果）——
/// 别自行加"遇到 `cell00` 就停"的优化。
pub fn set_ds_current_for_cell<H: DsCurrentHost + ?Sized>(h: &mut H, cell: CellId) -> Result<()> {
    reset_ds_current(h);
    if h.is_extender(cell) {
        for inst in members_of(h, cell) {
            h.ds_set_current(inst.ds, Some(inst), true);
        }
    }
    let mut head = h.left_head(cell);
    for _ in 0..4096 {
        if head.is_none() {
            break;
        }
        set_ds_current_left_head(h, head);
        head = h.left_head(head);
    }
    let mut head = h.top_head(cell);
    for _ in 0..4096 {
        if head.is_none() {
            break;
        }
        set_ds_current_top_head(h, head)?;
        head = h.top_head(head);
    }
    set_ds_current(h);
    Ok(())
}

/// `[this._$4] + this._$3`（`ExtExtendCell.java:295-304`）：本格与它登记的实例，
/// 按"自身在前"的顺序展开（Java 的 self 段先写 `_$4` 再遍历 `_$3`）。
///
/// ⛔ 不要用 `Vec::with_capacity(1 + ext.len())` 预分配：`ext` 是**共享**的扩展实例
/// 数组（长度 = 扩展行数 n），而明细场景里克隆格此刻往往还没写 `_$4` ——
/// 于是 n 次调用各预留 n×16 字节却一个都不装（实测该循环装 0 个）。
/// 该过量预留对 RSS 峰值影响不显著（2026-10-21 实测），但仍是纯浪费；按需增长即可。
fn members_of<H: DsCurrentHost + ?Sized>(h: &H, cell: CellId) -> Vec<DsInstance> {
    if !h.is_extender(cell) {
        return Vec::new();
    }
    let ext = h.ext_cells(cell);
    let mut out = Vec::new();
    if let Some(inst) = h.instance(cell) {
        out.push(inst);
    }
    for &e in ext.iter() {
        if let Some(inst) = h.instance(e) {
            out.push(inst);
        }
    }
    out
}

/// `Row.cross(other)` 的通用实现（`Row.java:68-79`）。
///
/// 两分支：`other` 是同一行则返回自身；否则返回 `getNullRow()`。
/// ⛔ `Group.cross`（`:112` 起那段阈值化集合运算）**未移植** —— 无夹具覆盖；
/// 生产侧 [`DsCurrentHost::ds_row_cross`] 遇到无法处理的形态时报
/// [`crate::error::EngineError::UnsupportedExpr`]，不要静默返回自身。
pub fn row_cross_default<H: DsCurrentHost + ?Sized>(
    h: &H,
    ds: usize,
    recv: DsInstance,
    other: DsInstance,
) -> Result<DsInstance> {
    if recv == other {
        Ok(recv)
    } else {
        Ok(h.ds_null_row(ds))
    }
}
