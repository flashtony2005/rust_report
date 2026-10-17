//! 记录层读出的原始值类型。
//!
//! 它既是 `.rpx` reader 的输出，也是 JSON 定义里 `literal` / `exp_map` 的宿主类型；
//! 因此与二进制 reader 解耦，放在独立模块，默认 JSON-only 构建也保留。

/// readObject 标签字典解出的值（M1 子集，附件 A.5）。
#[derive(Debug, Clone, PartialEq)]
pub enum ObjValue {
    Null,
    Str(String),
    Int(i32),
    Long(i64),
    Double(f64),
    Float(f32),
    Bool(bool),
    Byte(i8),
    Short(i16),
    /// 标签 2：int32 scale + 大端补码 unscaled 字节（M1 透传不转 decimal）
    BigDecimal {
        scale: i32,
        unscaled: Vec<u8>,
    },
    /// 标签 7
    BigInteger(Vec<u8>),
    /// 标签 8
    Bytes(Vec<u8>),
    /// 标签 3/31/32/33：java.util.Date / sql.Timestamp / sql.Time / sql.Date，毫秒
    Millis {
        tag: i8,
        millis: i64,
    },
    /// 标签 0：IRecord 多态，M1 只留类名与原始字节
    Obj {
        class_name: Option<String>,
        raw: Vec<u8>,
    },
}
