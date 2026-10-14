use std::fmt;

/// rpx-core 统一错误类型。
///
/// 分类依据 docs/spec-m1.md §4.2：
/// 文件层（BadLength/Truncated/BadFooter/UnsupportedFormat）、
/// 编解码层（Utf8）、槽位层（PropertyCode，带槽位定位上下文）、IO 层（Io）。
///
/// 纪律：**不保留死变体**。`BadMagic`（魔数不命中）与 `UnknownTag`（未知 readObject 标签）
/// 已分别因「AC-01 统一返回 UnsupportedFormat」「契约 A.5 末行要求兜底 readBytes()」
/// 而失去全部构造点，在 v1.3 清理中移除；若将来确需区分，请连同构造点一起加回。
#[derive(Debug)]
pub enum RpxError {
    /// 声明的 payload 长度为负或不可能值
    BadLength { declared: i64, file_len: usize },
    /// payload 实际字节不足（含记录内字节不足）
    Truncated { ctx: &'static str },
    /// 页脚缺失或不是 82,81,81,82,82,81,81,82
    BadFooter,
    /// 魔数不命中且不属于本工程支持的紧凑格式（含 Java OIS 老格式）
    UnsupportedFormat,
    /// 改良 UTF-8 解码失败
    Utf8 { detail: &'static str },
    /// 槽位解析越界/类型不符（定位上下文）
    PropertyCode { slot: u16, detail: &'static str },
    /// 底层 IO 错误
    Io(std::io::Error),
}

impl RpxError {
    pub(crate) fn truncated(ctx: &'static str) -> Self {
        RpxError::Truncated { ctx }
    }
    pub(crate) fn prop(slot: u16, detail: &'static str) -> Self {
        RpxError::PropertyCode { slot, detail }
    }
}

impl fmt::Display for RpxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RpxError::BadLength { declared, file_len } => {
                write!(
                    f,
                    "bad payload length {} in file of {} bytes",
                    declared, file_len
                )
            }
            RpxError::Truncated { ctx } => write!(f, "truncated stream at {}", ctx),
            RpxError::BadFooter => write!(f, "bad or missing 8-byte footer"),
            RpxError::UnsupportedFormat => {
                write!(
                    f,
                    "unsupported format (legacy Java OIS files are not accepted)"
                )
            }
            RpxError::Utf8 { detail } => write!(f, "modified UTF-8 decode error: {}", detail),
            RpxError::PropertyCode { slot, detail } => {
                write!(f, "property slot {} error: {}", slot, detail)
            }
            RpxError::Io(e) => write!(f, "io error: {}", e),
        }
    }
}

impl std::error::Error for RpxError {}

impl From<std::io::Error> for RpxError {
    fn from(e: std::io::Error) -> Self {
        RpxError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, RpxError>;
