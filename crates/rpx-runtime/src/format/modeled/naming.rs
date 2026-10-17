//! 码位 → v1 枚举名。未知码位返回 `None`，由调用方决定省略 + warning。

pub(super) fn row_type(code: i8) -> Option<&'static str> {
    Some(match code {
        -93 => "page_header",
        -96 => "title",
        -95 => "table_header",
        -91 => "data",
        -90 => "group_header",
        -94 => "table_footer",
        -89 => "tail_title",
        -92 => "page_footer",
        _ => return None,
    })
}

pub(super) fn col_type(code: i8) -> Option<&'static str> {
    Some(match code {
        -80 => "left_header",
        -79 => "data",
        -78 => "right_header",
        _ => return None,
    })
}

pub(super) fn extend_name(code: i8) -> Option<&'static str> {
    Some(match code {
        16 => "default",
        17 => "none",
        18 => "vertical",
        19 => "horizontal",
        _ => return None,
    })
}

pub(super) fn h_align(code: i8) -> Option<&'static str> {
    Some(match code {
        -48 => "left",
        -47 => "center",
        -46 => "right",
        -45 => "justify",
        -44 => "distribute",
        _ => return None,
    })
}

pub(super) fn v_align(code: i8) -> Option<&'static str> {
    Some(match code {
        -32 => "top",
        -31 => "middle",
        -30 => "bottom",
        _ => return None,
    })
}

pub(super) fn merge_same(code: i8) -> Option<&'static str> {
    Some(match code {
        98 => "vertical",
        99 => "horizontal",
        100 => "both",
        _ => return None,
    })
}

pub(super) fn border_style(code: u8) -> Option<&'static str> {
    Some(match code {
        0 => "none",
        81 => "dotted",
        82 => "dashed",
        83 => "solid",
        84 => "double",
        _ => return None,
    })
}

pub(super) fn param_type(code: i8) -> Option<&'static str> {
    Some(match code {
        1 => "int",
        2 => "long",
        3 => "short",
        5 => "float",
        6 => "double",
        7 => "bigdecimal",
        11 => "string",
        12 => "bool",
        51 => "int[]",
        61 => "string[]",
        _ => return None,
    })
}

pub(super) fn param_kind(code: i8) -> Option<&'static str> {
    Some(match code {
        0 => "static",
        1 => "dynamic",
        2 => "attribute",
        _ => return None,
    })
}
