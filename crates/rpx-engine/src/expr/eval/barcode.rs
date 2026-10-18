//! `barcode()` —— Code 128B 条码格用的普通函数。
//!
//! 单独成文件有两个理由：`expr/eval.rs` 已贴近 500 行上限（`docs/code-review.md` §3.1），
//! 且编码本体在 [`crate::barcode`]，这里只做「取值 → 调编码器 → 包成 Value」。
use super::{evaluate, EvalCtx};
use crate::error::{EngineError, Result};
use crate::expr::ast::Call;
use crate::value::Value;

/// `barcode(文本 [, 模块宽 [, 高度]])` → PNG data URI。
///
/// 只做 Code 128B（ASCII 32..126）。为什么返回 PNG 而不是 SVG：引擎侧
/// [`crate::graphics`] 与 `rpx-export::validate_image` 两处都只认
/// PNG/JPEG/GIF/BMP 前缀，SVG 会在两处被拒 —— 详见 [`crate::barcode`] 模块文档。
pub(super) fn call(c: &Call, ctx: &mut dyn EvalCtx) -> Result<Value> {
    let invalid = || EngineError::UnsupportedExpr {
        expr: format!("{}：参数应为 barcode(文本 [, 模块宽 [, 高度]])", c.name),
    };
    if c.args.is_empty() || c.args.len() > 3 {
        return Err(invalid());
    }
    let text = match evaluate(&c.args[0], ctx)? {
        Value::Text(s) => s,
        Value::Null | Value::List(_) => return Err(invalid()),
        Value::Bool(b) => u8::from(b).to_string(),
        Value::Byte(n) => n.to_string(),
        Value::Short(n) => n.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Float(n) => n.to_string(),
        Value::Double(n) => n.to_string(),
        Value::Decimal(s) => s,
    };
    let mut module = 1;
    let mut height = 40;
    for (index, slot) in [&mut module, &mut height].into_iter().enumerate() {
        if let Some(arg) = c.args.get(index + 1) {
            *slot = unsigned(&evaluate(arg, ctx)?).ok_or_else(invalid)?;
        }
    }
    crate::barcode::data_uri(&text, module, height)
        .map(Value::Text)
        .map_err(|message| EngineError::UnsupportedExpr {
            expr: format!("{}：{message}", c.name),
        })
}

/// 非负整数取值（尺寸参数用）。
fn unsigned(v: &Value) -> Option<u32> {
    let n = crate::expr::num::to_f64(v)?;
    if !n.is_finite() || n < 0.0 || n > f64::from(u32::MAX) {
        return None;
    }
    Some(n as u32)
}
