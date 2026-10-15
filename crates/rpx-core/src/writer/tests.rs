//! 写侧原语的钉子：**写 → 读回 → 逐值相等**（与 `reader.rs` 的读侧单测互逆）。
//!
//! 这里是**原语层**；`ReportDefine` 级的逐夹具字节对拍在
//! `crates/rpx-core/tests/roundtrip.rs`。

use crate::reader::{ObjValue, RecordReader};
use crate::ReportDefine;

use super::{to_bytes, RecordWriter, UTF_MAX};

/// 写一个 `writeRecord` 体并读回切片。
fn record_round_trip(f: impl FnOnce(&mut RecordWriter)) -> Option<Vec<u8>> {
    let mut w = RecordWriter::new();
    f(&mut w);
    let bytes = w.into_bytes();
    let mut r = RecordReader::new(&bytes);
    r.read_record_raw().unwrap().map(<[u8]>::to_vec)
}

#[test]
fn primitives_round_trip_big_endian() {
    let mut w = RecordWriter::new();
    w.write_i8(-91);
    w.write_u8(200);
    w.write_bool(true);
    w.write_bool(false);
    w.write_i16(-12345);
    w.write_i32(-2_000_000_000);
    w.write_i64(i64::MIN);
    w.write_f32(8.0);
    w.write_f64(-1.5);
    let bytes = w.into_bytes();

    let mut r = RecordReader::new(&bytes);
    assert_eq!(r.read_i8().unwrap(), -91);
    assert_eq!(r.read_u8().unwrap(), 200);
    assert!(r.read_bool().unwrap());
    assert!(!r.read_bool().unwrap());
    assert_eq!(r.read_i16().unwrap(), -12345);
    assert_eq!(r.read_i32().unwrap(), -2_000_000_000);
    assert_eq!(r.read_i64().unwrap(), i64::MIN);
    assert_eq!(r.read_f32().unwrap(), 8.0);
    assert_eq!(r.read_f64().unwrap(), -1.5);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn null_record_is_int32_minus_one() {
    let mut w = RecordWriter::new();
    w.write_record(None);
    assert_eq!(w.as_bytes(), &[0xFF, 0xFF, 0xFF, 0xFF]);
    // 读侧：size <= 0 ⇒ None（附件 A.4）
    assert_eq!(record_round_trip(|s| s.write_record(None)), None);
}

#[test]
fn null_strings_array_is_two_bytes() {
    // 附件 A.3：writeStrings(null) 只占 2 字节 00 00（NormalCell 槽 40 的那个空洞）。
    let mut w = RecordWriter::new();
    w.write_strings(None);
    assert_eq!(w.as_bytes(), &[0x00, 0x00]);
    let mut r = RecordReader::new(w.as_bytes());
    assert_eq!(r.read_strings().unwrap(), None);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn string_null_and_empty_are_distinct() {
    let mut w = RecordWriter::new();
    w.write_string(None);
    w.write_string(Some(""));
    w.write_string(Some("区域"));
    let mut r = RecordReader::new(w.as_bytes());
    assert_eq!(r.read_string().unwrap(), None, "null 与空串必须可区分");
    assert_eq!(r.read_string().unwrap(), Some(String::new()));
    assert_eq!(r.read_string().unwrap(), Some("区域".to_string()));
}

#[test]
fn strings_array_round_trips_with_null_elements() {
    let items = vec![
        Some("a".to_string()),
        None,
        Some("区域".to_string()),
        Some(String::new()),
    ];
    let mut w = RecordWriter::new();
    w.write_strings(Some(&items));
    let mut r = RecordReader::new(w.as_bytes());
    assert_eq!(r.read_strings().unwrap(), Some(items));
    assert_eq!(r.remaining(), 0);
}

#[test]
fn write_utf_rejects_over_65535() {
    let mut w = RecordWriter::new();
    let long = "a".repeat(UTF_MAX);
    assert!(w.write_utf(&long).is_ok(), "恰好 65535 字节应可写");
    let too_long = "a".repeat(UTF_MAX + 1);
    assert!(w.write_utf(&too_long).is_err(), "超 65535 应报错");
}

#[test]
fn objects_round_trip_every_variant() {
    let cases = vec![
        ObjValue::Null,
        ObjValue::Str("区域".to_string()),
        ObjValue::Str(String::new()),
        ObjValue::Int(-7),
        ObjValue::Long(9_000_000_000),
        ObjValue::Double(1.5),
        ObjValue::Float(2.5),
        ObjValue::Bool(true),
        ObjValue::Byte(-3),
        ObjValue::Short(-300),
        ObjValue::BigDecimal {
            scale: 2,
            unscaled: vec![0x01, 0x2C],
        },
        ObjValue::BigInteger(vec![0xFF, 0x00]),
        ObjValue::Bytes(vec![0xAA, 0xBB]),
        ObjValue::Millis {
            tag: 31,
            millis: 1_700_000_000_000,
        },
        ObjValue::Obj {
            class_name: Some("com.raqsoft.X".to_string()),
            raw: vec![1, 2, 3],
        },
    ];
    for v in cases {
        let mut w = RecordWriter::new();
        w.write_object(&v).unwrap();
        let bytes = w.into_bytes();
        let mut r = RecordReader::new(&bytes);
        assert_eq!(r.read_object("test").unwrap(), v, "round-trip failed");
        assert_eq!(r.remaining(), 0, "必须恰好消费完毕：{v:?}");
    }
}

/// 长字符串（> 65535 字节）必须走标签 13（int32 前缀）——与 `NormalCell.java:1493` 同规则。
#[test]
fn long_string_uses_tag_13() {
    let s = "a".repeat(UTF_MAX + 1);
    let mut w = RecordWriter::new();
    w.write_object(&ObjValue::Str(s.clone())).unwrap();
    assert_eq!(w.as_bytes()[0], 13, "超长字符串应走标签 13");
    let mut r = RecordReader::new(w.as_bytes());
    assert_eq!(r.read_object("test").unwrap(), ObjValue::Str(s));
    assert_eq!(r.remaining(), 0);
}

/// 短字符串走标签 1（uint16 前缀）——普通单元格 VALUE 的常态形态。
#[test]
fn short_string_uses_tag_1() {
    let mut w = RecordWriter::new();
    w.write_object(&ObjValue::Str("abc".to_string())).unwrap();
    assert_eq!(w.as_bytes()[0], 1, "短字符串应走标签 1");
    // 标签 1 + uint16 长度 3 + "abc"
    assert_eq!(w.as_bytes(), &[1, 0x00, 0x03, b'a', b'b', b'c']);
}

#[test]
fn byte_map_round_trips() {
    let entries = vec![
        (1u8, ObjValue::Str("36".to_string())),
        (13u8, ObjValue::Bool(true)),
        (40u8, ObjValue::Str("ds1.select(qty)".to_string())),
    ];
    let mut w = RecordWriter::new();
    w.write_byte_map(&entries).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(RecordReader::parse_byte_map(&bytes).unwrap(), entries);
}

#[test]
fn file_layer_has_magic_length_and_footer() {
    let bytes = to_bytes(&minimal()).unwrap();
    assert_eq!(&bytes[..4], crate::MAGIC, "文件头魔数");
    assert_eq!(&bytes[bytes.len() - 8..], crate::FOOTER, "8 字节页脚");
    let declared = i32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert_eq!(
        declared as usize,
        bytes.len() - 16,
        "声明长度 = 总长 - 头 8 - 尾 8"
    );
}

/// 最小合法定义（3×3 矩阵：2 逻辑行 × 2 逻辑列）。
fn minimal() -> ReportDefine {
    ReportDefine::fill(&minimal_payload()).unwrap()
}

/// 手工拼一份 3×3 矩阵的最小载荷（无格、无数据集、默认标量）。
fn minimal_payload() -> Vec<u8> {
    let mut w = RecordWriter::new();
    w.write_i32(3); // rows
    w.write_i32(3); // cols
    for _ in 1..3 {
        // RowCell：10 槽的默认形态
        let body = {
            let mut s = RecordWriter::new();
            s.write_i8(-91);
            s.write_f32(8.0);
            s.write_bool(true);
            s.write_bool(false);
            s.write_record(None); // expMap
            s.write_string(None); // notes
            s.write_i8(1); // groupHeaderLevel
            s.write_blob(None); // ois blob
            s.write_bool(false);
            s.write_bool(false);
            s.into_bytes()
        };
        w.write_record(Some(&body));
    }
    for _ in 1..3 {
        let body = {
            let mut s = RecordWriter::new();
            s.write_i8(-79);
            s.write_f32(25.0);
            s.write_bool(true);
            s.write_bool(false);
            s.write_record(None);
            s.write_string(None);
            s.write_blob(None);
            s.write_bool(false);
            s.into_bytes()
        };
        w.write_record(Some(&body));
    }
    for _ in 1..3 {
        for _ in 1..3 {
            w.write_record(None); // 空格
        }
    }
    // PrintSetup：33 槽默认
    let ps = crate::print_setup::PrintSetup::default_values();
    let body = {
        let mut s = RecordWriter::new();
        super::records::write_print_setup(&mut s, &ps);
        s.into_bytes()
    };
    w.write_record(Some(&body));
    for _ in 0..6 {
        w.write_blob(None); // 其余 6 个 MetaData
    }
    // 标量 7-16
    w.write_i32(133);
    w.write_i8(1);
    w.write_string(None);
    w.write_i8(0);
    w.write_string(None);
    w.write_string(None);
    w.write_i32(0);
    w.write_i32(0);
    w.write_i8(1);
    w.write_string(None);
    w.write_i32(0); // cpSize
    w.write_record(None); // expMap
    w.write_string(None); // chartStyleName
    w.write_string(None); // validityScript
    w.write_string(None); // bigDataSetName
    w.write_string(None); // password
    w.write_bool(true); // highPrecision
    w.write_i32(-1); // threshold
    w.into_bytes()
}
