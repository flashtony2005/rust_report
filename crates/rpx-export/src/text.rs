//! `Value` → 导出文本口径（自定，spec-m6 §2.3 规则 2 的实现）。
//!
//! 独立实现而不复用 `rpx-server/src/snapshot/value.rs:49-68` 的 `scalar_text`：
//! 那是 server 私有且 G8 分层矩阵禁止 export 依赖 server（依赖方向是
//! `server -> export`，不是反向）。两处口径**对齐**（数值 shortest Display /
//! Decimal 原样 / Bool `true|false` / List 逗号连接、嵌套 `[..]`），改动任一
//! 处要同步另一处。
//!
//! ⚠ 非有限数（`NaN`/`inf`）不在「null ⇒ 空串」之列：显式落文本（Rust Display
//! 的 `NaN`/`inf`/`-inf`），不静默成空——比 server 的「落 null」更保守，
//! 属于自定契约的刻意选择（有测试钉）。

use rpx_engine::arena::CalcCell;
use rpx_engine::value::Value;

/// 一格 → 文本：`disp_value` 优先，否则 [`value_text`]（spec-m6 §2.3 规则 2）。
pub(crate) fn cell_text(cell: &CalcCell) -> String {
    if let Some(disp) = &cell.disp_value {
        return disp.clone();
    }
    value_text(&cell.value)
}

/// 一个值 → 文本（规则见模块文档）。
pub(crate) fn value_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Int(n) => n.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Short(n) => n.to_string(),
        Value::Byte(n) => n.to_string(),
        Value::Double(f) => f.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Decimal(s) => s.clone(),
        Value::Text(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::List(items) => join_list(items),
    }
}

/// `List` 的 v1 近似：逗号连接（嵌套 List 递归成 `[..]`，同 server 口径）。
fn join_list(items: &[Value]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|v| match v {
            Value::List(inner) => format!("[{}]", join_list(inner)),
            other => value_text(other),
        })
        .collect();
    parts.join(",")
}
