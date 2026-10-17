use rpx_core::value::Value;
use rpx_ds::file_source::{delimited, excel};

const BOOK: &[u8] = include_bytes!("fixtures/file-sources.xlsx");

#[test]
fn csv_quotes_newlines_bom_and_types() {
    let rows = delimited("\u{feff}name,amount,note,active,optional\r\n华东,50.5,\"退款,调整\n含\"\"引号\"\"\",true,\r\n", b',').unwrap();
    assert_eq!(rows.cols, ["name", "amount", "note", "active", "optional"]);
    assert_eq!(
        rows.rows,
        vec![vec![
            Value::Text("华东".into()),
            Value::Double(50.5),
            Value::Text("退款,调整\n含\"引号\"".into()),
            Value::Bool(true),
            Value::Null
        ]]
    );
    assert_eq!(
        delimited("id\tamount\n2147483648\t12\n", b'\t')
            .unwrap()
            .rows[0],
        vec![Value::Long(2_147_483_648), Value::Int(12)]
    );
}

#[test]
fn csv_rejects_ambiguous_headers_and_inconsistent_rows() {
    for text in ["", "a,a\n1,2", "a,\n1,2", "a,b\n1", "a,b\n1,2,3"] {
        assert!(delimited(text, b',').is_err(), "{text:?}");
    }
}

#[test]
fn excel_selected_sheets_preserve_types_and_used_range_origin() {
    let first = excel(BOOK, None).unwrap();
    assert_eq!(first.cols, ["name", "amount", "active", "date", "optional"]);
    assert_eq!(
        first.rows[0],
        vec![
            Value::Text("华东".into()),
            Value::Double(12.5),
            Value::Bool(true),
            Value::Text("2026-10-17T09:30:00.000".into()),
            Value::Null
        ]
    );
    assert_eq!(first.rows[1][4], Value::Text("001".into()));
    let shifted = excel(BOOK, Some("偏移")).unwrap();
    assert_eq!(shifted.cols, ["region", "budget"]);
    assert_eq!(
        shifted.rows,
        vec![vec![Value::Text("华南".into()), Value::Double(500.0)]]
    );
}

#[test]
fn excel_missing_sheet_uncached_formula_and_error_are_explicit() {
    assert!(excel(BOOK, Some("不存在")).unwrap_err().contains("不存在"));
    let error = excel(BOOK, Some("无缓存公式")).unwrap_err();
    assert!(error.contains("第4行第3列公式没有缓存值"), "{error}");
    assert!(excel(BOOK, Some("错误"))
        .unwrap_err()
        .contains("不支持的值"));
    assert!(excel(BOOK, Some("重名")).unwrap_err().contains("重复"));
}
