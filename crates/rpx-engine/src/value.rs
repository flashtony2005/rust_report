//! 值模型的 re-export 层（M3-03 起）。
//!
//! `Value` 本体住在 [`rpx_core::value`]（定案 `docs/spec-m3.md` §3.4：值模型是
//! **数据契约**，供数方 `rpx-ds` 只依赖 core）。本模块保持
//! `crate::value::Value` / `rpx_engine::value::Value` 两条既有路径不变——
//! 引擎内部与 server/cli 的引用零改动。
//!
//! 仍留在本模块的只有一件东西：[`single_value`]——它是 `Variant2.getSingleValue`
//! 的**求值期**语义（聚合函数的入口规约），不属于物化契约。
//!
//! 原值模型全文档与契约测试已随类型迁往 `rpx_core::value`（含 `engine_types`
//! 夹具背景与 `"Short"`/`"Byte"` 兜底分支的探针证据，一处不少）。

pub use rpx_core::value::Value;

/// `Variant2.getSingleValue`（`Variant2.java:85-94`）：**空 `List` → `null`、
/// 非空 `List` → 首元素、其余原样**。
///
/// 产品里所有"聚合一个 List"的函数（`Sum` / `Average` / `Max` / `Min` /
/// `Variant2.equals`）都先过这一层，所以它必须是**全仓唯一一份** ——
/// 两处并行声明迟早漂移（本仓 `silent-failure-hunt` 记过的老毛病）。
/// 2026-10-13 收敛：此前 `merge.rs` 有一份私有副本，`expr/dsfn.rs` 的
/// `sum_list` 则**完全没过这一层**（见那边的 §M2-12 注记）。
///
/// ⚠ 与 Java 的一处**已声明近似**：Java 的 `getSingleValue` 会先过
/// `Variant2.getValue` 拆 `Variable` / `DispValue` 包装（`:86`），而本 crate
/// 的值已是裸值，无需拆。
pub(crate) fn single_value(v: &Value) -> Option<&Value> {
    match v {
        Value::Null => None,
        Value::List(items) => items.first(),
        other => Some(other),
    }
}
