//! 日期字符串分量；无日期 Value 类型，因此明确校验输入而不补造默认日期。
use super::{evaluate, EvalCtx};
use crate::{
    error::{EngineError, Result},
    expr::ast::{Call, ParamKind},
    value::Value,
};

pub(super) fn call(c: &Call, ctx: &mut dyn EvalCtx) -> Result<Value> {
    if c.args.len() != 1 || c.param_kind != ParamKind::Leaf {
        return Err(invalid(&c.name, "需要一个日期字符串"));
    }
    let value = evaluate(&c.args[0], ctx)?;
    if value.is_null() {
        return Ok(Value::Null);
    }
    let Value::Text(text) = value else {
        return Err(invalid(&c.name, &format!("期望日期字符串，得到 {value:?}")));
    };
    let (year, month) = parts(&text).ok_or_else(|| {
        invalid(
            &c.name,
            &format!("非法日期 {text:?}，需要 yyyy-MM-dd 或 yyyy-MM-dd HH:mm:ss"),
        )
    })?;
    match (c.name.as_str(), c.option.as_deref().unwrap_or("")) {
        ("year", "") => Ok(Value::Int(year)),
        ("month", "") => Ok(Value::Int(month)),
        ("month", "y") => Ok(Value::Int(year * 100 + month)),
        _ => Err(invalid(&c.name, "不支持的日期函数选项")),
    }
}

fn invalid(name: &str, detail: &str) -> EngineError {
    EngineError::UnsupportedExpr {
        expr: format!("{name}: {detail}"),
    }
}

/// Gregorian 年 1..9999；日期后的时间若存在也必须合法，禁止静默截掉坏后缀。
fn parts(text: &str) -> Option<(i32, i32)> {
    let b = text.as_bytes();
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let year = digits(&b[..4])?;
    let month = digits(&b[5..7])?;
    let day = digits(&b[8..10])?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let last = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return None,
    };
    if year == 0 || day < 1 || day > last {
        return None;
    }
    if b.len() > 10 {
        if b.len() < 19 || !matches!(b[10], b' ' | b'T') || b[13] != b':' || b[16] != b':' {
            return None;
        }
        if digits(&b[11..13])? > 23 || digits(&b[14..16])? > 59 || digits(&b[17..19])? > 59 {
            return None;
        }
        let mut tail = &b[19..];
        if tail.first() == Some(&b'.') || tail.first() == Some(&b':') {
            let n = tail[1..].iter().take_while(|d| d.is_ascii_digit()).count();
            if !(1..=9).contains(&n) {
                return None;
            }
            tail = &tail[n + 1..];
        }
        if !tail.is_empty() && tail != b"Z" {
            if tail.len() != 6 || !matches!(tail[0], b'+' | b'-') || tail[3] != b':' {
                return None;
            }
            let hour = digits(&tail[1..3])?;
            let minute = digits(&tail[4..6])?;
            if hour > 14 || minute > 59 || (hour == 14 && minute != 0) {
                return None;
            }
        }
    }
    Some((year, month))
}

fn digits(bytes: &[u8]) -> Option<i32> {
    bytes.iter().try_fold(0, |value, &digit| {
        digit
            .is_ascii_digit()
            .then(|| value * 10 + i32::from(digit - b'0'))
    })
}
