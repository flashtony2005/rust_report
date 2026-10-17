//! `reportdef` 的钉子：合法定义逐字段落地 + 非法定义**显式报错**（不静默）。
//!
//! 判据 = 直接查 `ReportDefine` 的字段（不经过渲染链路）—— 本层是**构造**层，
//! 它的契约就是「产出的模型对不对」。端到端（→ 快照 / → xlsx）在
//! `tests/reportdef_http.rs`。

use rpx_core::reader::ObjValue;

use super::{build, open_report, DefError, SourceError, MERGE_SAME_VERTICAL, VERSION};

/// 取格：`(r,c)`（1 起）→ store 下标（无格 ⇒ None）。
fn at(rd: &rpx_core::ReportDefine, r: usize, c: usize) -> Option<usize> {
    *rd.grid.get(r)?.get(c)?
}

/// 合法定义的最小骨架（`cells` 由各用例替换）。
fn doc(cells: &str) -> String {
    format!(r#"{{"version":{VERSION},"grid":{{"rows":3,"cols":4}},"cells":[{cells}]}}"#)
}

#[test]
fn minimal_definition_lands_all_fields() {
    let rd = build(&doc(
        r#"{"row":1,"col":1,"text":"标题","merge":{"rows":1,"cols":4}},
           {"row":2,"col":2,"expr":"ds1.select(qty)"},
           {"row":3,"col":4,"text":"尾","h_align":"right","v_align":"bottom","merge_same":"vertical"}"#,
    ))
    .expect("合法定义应通过");

    // 矩阵尺寸 = 逻辑 +1（0 号脚手架）。
    assert_eq!((rd.rows, rd.cols), (4, 5));

    // 静态文本 + 合并锚点的 merged_area。
    let a = at(&rd, 1, 1).expect("(1,1) 应有格");
    assert_eq!(rd.store[a].value, ObjValue::Str("标题".into()));
    let area = rd.store[a]
        .merged_area
        .as_ref()
        .expect("锚点应有 merged_area");
    assert_eq!(
        (area.begin_row, area.begin_col, area.end_row, area.end_col),
        (1, 1, 1, 4)
    );
    // 矩形内非锚点槽必须留 None（引擎靠锚点铺满）。
    assert_eq!(at(&rd, 1, 2), None);
    assert_eq!(at(&rd, 1, 4), None);

    // 表达式格写在 exp_map 键 40。
    let e = at(&rd, 2, 2).expect("(2,2) 应有格");
    assert_eq!(
        rd.store[e].exp_map.as_ref().expect("应有 exp_map"),
        &vec![(40u8, ObjValue::Str("ds1.select(qty)".into()))]
    );

    // 对齐（契约附件 B.3 注 3：right=-46、bottom=-30）+ 同值合并。
    let t = at(&rd, 3, 4).expect("(3,4) 应有格");
    assert_eq!(rd.store[t].h_align, -46);
    assert_eq!(rd.store[t].v_align, -30);
    assert_eq!(rd.store[t].merge_same, MERGE_SAME_VERTICAL);
    assert_eq!(
        rd.store[t].merge_same_mode, 118,
        "同值合并判据应保持默认 118"
    );

    // `merges` 也登记（供 CLI 展示）。
    assert_eq!(rd.merges.len(), 1);
    assert_eq!((rd.merges[0].begin_row, rd.merges[0].end_row), (1, 1));
}

#[test]
fn row_and_col_props_become_expression_slots() {
    let rd = build(
        r#"{"grid":{"rows":3,"cols":4},
             "row_props":[{"row":1,"height":36},{"row":3,"break_page":true}],
             "col_props":[{"col":1,"width":14},{"col":4,"width":"8+8"}],
             "cells":[]}"#,
    )
    .expect("合法定义应通过");

    assert_eq!(rd.row_cells[0].row_height, 36.0);
    assert_eq!(rd.row_cells[0].exp_map, None);
    assert!(rd.row_cells[2].break_page);
    assert_eq!(rd.col_cells[0].col_width, 14.0);
    assert_eq!(
        rd.col_cells[3]
            .exp_map
            .as_ref()
            .expect("第 4 列应有 exp_map"),
        &vec![(11u8, ObjValue::Str("8+8".into()))]
    );
}

#[test]
fn datasets_sql_and_file() {
    let rd = build(
        r#"{"grid":{"rows":1,"cols":1},
            "datasets":[
              {"name":"ds1","sql":{"dsn":"sqlite","sql":"SELECT 1"}},
              {"name":"ds2","file":{"file_name":"a.csv","encoding":"UTF-8"}}],
            "cells":[]}"#,
    )
    .expect("合法定义应通过");

    assert_eq!(rd.datasets.len(), 2);
    let sql = rd.datasets[0].sql.as_ref().expect("第一条应是 SQL 数据集");
    assert_eq!(sql.name.as_deref(), Some("ds1"));
    assert_eq!(sql.data_source_name.as_deref(), Some("sqlite"));
    assert_eq!(sql.sql.as_deref(), Some("SELECT 1"));
    assert_eq!(
        rd.datasets[0].class_name,
        "com.raqsoft.reportl.usermodel.SQLDataSetConfig"
    );
    assert!(rd.datasets[0].factory_class.is_some(), "工厂类应可反查");

    let f = rd.datasets[1].file.as_ref().expect("第二条应是文件数据集");
    assert_eq!(f.file_name.as_deref(), Some("a.csv"));
    assert_eq!(f.encoding.as_deref(), Some("UTF-8"));
    assert_eq!(f.name.as_deref(), Some("ds2"));
}

// ---------------------------------------------------------------- 非法定义：必须显式报错

/// 断言报错且消息里带指定片段（错误消息本身就是契约的一部分）。
#[track_caller]
fn expect_err(json: &str, needle: &str) {
    match build(json) {
        Ok(_) => panic!("该定义应报错（含 {needle:?}）却通过了：{json}"),
        Err(e) => {
            let msg = e.to_string();
            assert!(msg.contains(needle), "错误消息应含 {needle:?}，实得：{msg}");
        }
    }
}

#[test]
fn rejects_unknown_version_loudly() {
    // 不猜：未来版本的语义未知，宁可拒绝也不按 v1 硬解。
    expect_err(
        r#"{"version":99,"grid":{"rows":1,"cols":1},"cells":[]}"#,
        "不支持的定义版本 99",
    );
}

#[test]
fn rejects_missing_and_malformed_fields() {
    expect_err(r#"{"cells":[]}"#, "$.grid：缺少必填字段");
    expect_err(r#"{"grid":{"rows":0,"cols":1},"cells":[]}"#, "必须 ≥1");
    expect_err(
        r#"{"grid":{"rows":2,"cols":2},"cells":[{"row":3,"col":1,"text":"x"}]}"#,
        "行号 3 越界（1..=2）",
    );
    expect_err(
        r#"{"grid":{"rows":2,"cols":2},"cells":[{"row":1,"col":9,"text":"x"}]}"#,
        "列号 9 越界（1..=2）",
    );
    expect_err(
        r#"{"grid":{"rows":2,"cols":2},"cells":[{"row":1}]}"#,
        "$.cells[0].col：缺少必填字段",
    );
    // JSON 语法错（不是结构错）。
    assert!(matches!(build("{not json"), Err(DefError::Json(_))));
}

#[test]
fn rejects_ambiguous_or_unknown_enum_values() {
    expect_err(
        r#"{"grid":{"rows":2,"cols":2},"cells":[{"row":1,"col":1,"text":"a","expr":"b"}]}"#,
        "`text` 与 `expr` 互斥",
    );
    expect_err(
        r#"{"grid":{"rows":2,"cols":2},"cells":[{"row":1,"col":1,"text":"a","h_align":"middle"}]}"#,
        "未知取值 \"middle\"",
    );
    expect_err(
        r#"{"grid":{"rows":2,"cols":2},"cells":[{"row":1,"col":1,"text":"a","merge_same":"diagonal"}]}"#,
        "未知取值 \"diagonal\"",
    );
    expect_err(
        r#"{"grid":{"rows":2,"cols":2},"datasets":[{"name":"d","sql":{"dsn":"x","sql":"y"},"file":{"file_name":"f"}}],"cells":[]}"#,
        "`sql` 与 `file` 互斥",
    );
    expect_err(
        r#"{"grid":{"rows":2,"cols":2},"datasets":[{"name":"d"}],"cells":[]}"#,
        "必须给 `sql` 或 `file` 之一",
    );
}

#[test]
fn rejects_degenerate_and_oversized_merge() {
    expect_err(
        r#"{"grid":{"rows":3,"cols":3},"cells":[{"row":1,"col":1,"text":"x","merge":{"rows":1,"cols":1}}]}"#,
        "1×1 合并无意义",
    );
    expect_err(
        r#"{"grid":{"rows":3,"cols":3},"cells":[{"row":1,"col":1,"text":"x","merge":{"rows":1,"cols":9}}]}"#,
        "超出网格 3×3",
    );
}

#[test]
fn content_inside_another_merge_is_loud_but_empty_shell_is_tolerated() {
    // 带内容 ⇒ 报错（内容会被静默丢弃）。
    expect_err(
        r#"{"grid":{"rows":3,"cols":3},
            "cells":[{"row":1,"col":1,"text":"锚","merge":{"rows":1,"cols":3}},
                     {"row":1,"col":2,"text":"会被丢掉的"}]}"#,
        "落在另一个合并矩形内，其内容会被静默丢弃",
    );
    // 空壳 ⇒ 容忍（Univer 导出会把矩形内所有槽都列出来）。
    let rd = build(
        r#"{"grid":{"rows":3,"cols":3},
            "cells":[{"row":1,"col":2},
                     {"row":1,"col":1,"text":"锚","merge":{"rows":1,"cols":3}}]}"#,
    )
    .expect("空壳非锚点槽应被容忍");
    // 顺序无关：锚点在后声明也应正确铺满，且空壳不进 store。
    let a = at(&rd, 1, 1).expect("锚点应在 (1,1)");
    assert_eq!(rd.store[a].value, ObjValue::Str("锚".into()));
    assert_eq!(at(&rd, 1, 2), None, "矩形内非锚点槽应留 None");
    assert_eq!(rd.store.len(), 1, "空壳不应进 store（否则是幽灵格）");
}

// ---------------------------------------------------------------- 声明式主格

/// 声明式主格落到 `Cell.left_head` / `Cell.top_head`（引擎 `stage_b.rs:282-283`
/// 从这里读进 `HeadCell.declared_*`，`find_top_head` 分支 A 优先用它）。
#[test]
fn declared_heads_reach_the_cell_slots() {
    let rd = build(
        r#"{"grid":{"rows":3,"cols":3},
            "cells":[{"row":1,"col":2,"expr":"ds1.select(q)","extend_mode":"horizontal"},
                     {"row":2,"col":3,"text":"小计","top_head":"b1"}]}"#,
    )
    .expect("声明指向真实存在的格 ⇒ 应通过");

    let h = at(&rd, 1, 2).expect("(1,2) 应有格");
    assert!(rd.store[h].left_head.is_none(), "未声明 ⇒ None");

    let s = at(&rd, 2, 3).expect("(2,3) 应有格");
    assert_eq!(
        rd.store[s].top_head.as_deref(),
        Some("B1"),
        "声明应落到槽里，且归一成大写"
    );
    assert!(rd.store[s].left_head.is_none(), "两槽独立");
}

/// ⛔ 这条是**本功能的核心判据**：引擎 `find.rs` 分支 A 对「地址解析不到格」是
/// **静默落空**（`if !head.is_none()`）—— 打错格号与不声明完全同形。
/// 装载层的第三趟存在性校验必须把这种情况变成显式错误。
#[test]
fn declared_head_pointing_at_a_missing_cell_is_loud() {
    // 网格 3×3，Z9 越界。
    expect_err(
        r#"{"grid":{"rows":3,"cols":3},
            "cells":[{"row":1,"col":1,"text":"x","top_head":"Z9"}]}"#,
        "格 Z9 不存在（网格 3×3）",
    );
    // 网格内但**没有格**的槽（(3,3) 没声明）——同样必须报错，
    // 否则引擎会静默回退到自动推导。
    expect_err(
        r#"{"grid":{"rows":3,"cols":3},
            "cells":[{"row":1,"col":1,"text":"x","left_head":"C3"}]}"#,
        "格 C3 不存在",
    );
    // 路径要指到具体字段（前端据此定位）。
    expect_err(
        r#"{"grid":{"rows":3,"cols":3},
            "cells":[{"row":1,"col":1,"text":"a"},
                     {"row":1,"col":2,"text":"b","left_head":"Z9"}]}"#,
        "$.cells[1].left_head",
    );
}

/// 存在性口径与引擎矩阵 `_$25` 对齐：**合并延续槽**也算存在
/// （`stage_b/merge_alias.rs` 对延续槽有登记）⇒ 声明指向它必须放行。
#[test]
fn declared_head_may_point_at_a_merge_continuation_slot() {
    let rd = build(
        r#"{"grid":{"rows":2,"cols":4},
            "cells":[{"row":1,"col":1,"text":"锚","merge":{"rows":1,"cols":3}},
                     {"row":2,"col":2,"text":"小计","top_head":"C1"}]}"#,
    )
    .expect("指向合并延续槽 C1 应放行（引擎矩阵对它有登记）");

    assert_eq!(at(&rd, 1, 3), None, "延续槽在 grid 里仍是 None");
    let s = at(&rd, 2, 2).expect("(2,2) 应有格");
    assert_eq!(rd.store[s].top_head.as_deref(), Some("C1"));
}

/// 合并矩形的**锚点**自然可被声明（最常见形态）。
#[test]
fn declared_head_may_point_at_a_merge_anchor() {
    let rd = build(
        r#"{"grid":{"rows":2,"cols":3},
            "cells":[{"row":1,"col":1,"text":"季度","merge":{"rows":1,"cols":2}},
                     {"row":2,"col":1,"text":"小计","top_head":"A1"}]}"#,
    )
    .expect("指向合并锚点应放行");
    let s = at(&rd, 2, 1).expect("(2,1) 应有格");
    assert_eq!(rd.store[s].top_head.as_deref(), Some("A1"));
}

// ---------------------------------------------------------------- 来源分派

#[test]
fn open_report_dispatches_by_content() {
    // JSON 定义（首个非空白字节 `{`，前导空白容忍）。
    let rd = open_report(b"  \n {\"grid\":{\"rows\":1,\"cols\":1},\"cells\":[]} ")
        .expect("JSON 定义应被识别");
    assert_eq!((rd.rows, rd.cols), (2, 2));

    // 无 RQQR 魔数、也不是 JSON ⇒ 显式报「无法识别」（不猜）。
    match open_report(b"MZ\x90\x00garbage") {
        Err(SourceError::UnknownFormat) => {}
        other => panic!("应报 UnknownFormat，实得 {other:?}"),
    }

    // 有魔数但字节不完整 ⇒ 走 .rpx 读侧（Core 分支），不是 UnknownFormat。
    #[cfg(feature = "legacy-rpx")]
    {
        match open_report(b"RQQR\x00\x00") {
            Err(SourceError::Core(_)) => {}
            other => panic!("应走 .rpx 读侧报 Core，实得 {other:?}"),
        }
    }

    // 定义 JSON 报错时是 Def 分支，且错误消息带 JSON 路径（前端据此定位）。
    match open_report(br#"{"grid":{"rows":0,"cols":1},"cells":[]}"#) {
        Err(SourceError::Def(e)) => {
            let msg = e.to_string();
            assert!(msg.contains("$.grid"), "错误应带 JSON 路径，实得：{msg}");
        }
        other => panic!("应报 Def，实得 {other:?}"),
    }
}

#[test]
fn json_definition_bytes_are_not_mistaken_for_rpx() {
    // 反向保护：JSON 定义不得被当作 .rpx（魔数判定必须严格 4 字节前缀）。
    assert!(!b"{\"grid\"".starts_with(b"RQQR"));
    let rd = open_report(br#"{"grid":{"rows":1,"cols":1},"cells":[]}"#).expect("应识别为 JSON");
    assert_eq!((rd.rows, rd.cols), (2, 2));
}
