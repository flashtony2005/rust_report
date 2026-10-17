//! 聚合与过滤可嵌套；退出时同时恢复当前格与外层 `$` 锚点。
use super::EvalCtx;
use crate::error::Result;

pub(crate) fn with_current<T>(
    ctx: &mut dyn EvalCtx,
    f: impl FnOnce(&mut dyn EvalCtx) -> Result<T>,
) -> Result<T> {
    let current = ctx.current();
    let saved = ctx.saved_current();
    ctx.save_current();
    let result = f(ctx);
    ctx.set_current(saved);
    ctx.save_current();
    ctx.set_current(current);
    result
}
