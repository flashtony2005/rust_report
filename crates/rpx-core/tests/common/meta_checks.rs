//! 黄金测试的 MetaData 段检查（从 golden.rs 拆出，保持单文件 ≤300 行门禁）：
//! params / datasets / print_setup / scalars 四组对拍。

use super::J;
use rpx_core::ReportDefine;

const TOL: f64 = 1e-5;

pub fn check_meta(name: &str, r: &ReportDefine, j: &J) {
    // ---- params ----
    let jp = j.get("params").unwrap().as_arr().unwrap();
    assert_eq!(r.params.len(), jp.len(), "{}: params count", name);
    for (got, want) in r.params.iter().zip(jp.iter()) {
        assert_eq!(
            got.name.as_deref(),
            want.get("name").unwrap().as_str(),
            "{}: param name",
            name
        );
        assert_eq!(
            got.description.as_deref(),
            want.get("description").unwrap().as_str(),
            "{}: param desc",
            name
        );
        assert_eq!(
            got.param_type as i64,
            want.get("param_type").unwrap().as_i64().unwrap(),
            "{}: param_type",
            name
        );
        assert_eq!(
            got.data_type as i64,
            want.get("data_type").unwrap().as_i64().unwrap(),
            "{}: data_type",
            name
        );
        assert_eq!(
            got.value.as_deref(),
            want.get("value").unwrap().as_str(),
            "{}: param value",
            name
        );
    }

    // ---- datasets ----
    let jd = j.get("datasets").unwrap().as_arr().unwrap();
    assert_eq!(r.datasets.len(), jd.len(), "{}: datasets count", name);
    for (got, want) in r.datasets.iter().zip(jd.iter()) {
        // class_name 来自流内记录头；factory_class 不在流里，由类名常量映射得出（附件 E.4）。
        // 旧实现 `assert_eq!(Some(X), want.get("name").map(|_| X))` 是 X == X 的同义反复
        // （只验 JSON 有没有 name 键），从未校验 class_name 取值 —— 独立验证发现，已修正。
        assert!(
            !got.class_name.is_empty(),
            "{}: ds class_name must be non-empty",
            name
        );
        assert_eq!(
            rpx_core::dataset::factory_class_for(&got.class_name),
            got.factory_class,
            "{}: ds factory_class must be the class_name mapping result",
            name
        );
        assert_eq!(
            want.get("data_source_name").and_then(|v| v.as_str()),
            got.file
                .as_ref()
                .and_then(|f| f.data_source_name.as_deref()),
            "{}: ds data_source_name",
            name
        );
        assert_eq!(
            want.get("factory_class").and_then(|v| v.as_str()),
            got.factory_class,
            "{}: ds factory_class",
            name
        );
        assert_eq!(
            want.get("file_name").and_then(|v| v.as_str()),
            got.file.as_ref().and_then(|f| f.file_name.as_deref()),
            "{}: ds file_name",
            name
        );
        assert_eq!(
            want.get("name").and_then(|v| v.as_str()),
            got.file.as_ref().and_then(|f| f.name.as_deref()),
            "{}: ds name",
            name
        );
    }

    // ---- print_setup ----
    let ps = j.get("print_setup").unwrap();
    let rps = &r.print_setup;
    assert_eq!(
        rps.paper as i64,
        ps.get("paper").unwrap().as_i64().unwrap(),
        "{}: paper",
        name
    );
    assert!(
        (rps.paper_width as f64 - ps.get("paper_width").unwrap().as_f64().unwrap()).abs() < TOL,
        "{}: paper_width",
        name
    );
    assert!(
        (rps.paper_height as f64 - ps.get("paper_height").unwrap().as_f64().unwrap()).abs() < TOL,
        "{}: paper_height",
        name
    );
    assert_eq!(
        rps.orient as i64,
        ps.get("orientation").unwrap().as_i64().unwrap(),
        "{}: orientation",
        name
    );
    assert!(
        (rps.left_margin as f64 - ps.get("left_margin").unwrap().as_f64().unwrap()).abs() < TOL,
        "{}: left_margin",
        name
    );
    assert!(
        (rps.right_margin as f64 - ps.get("right_margin").unwrap().as_f64().unwrap()).abs() < TOL,
        "{}: right_margin",
        name
    );
    assert!(
        (rps.top_margin as f64 - ps.get("top_margin").unwrap().as_f64().unwrap()).abs() < TOL,
        "{}: top_margin",
        name
    );
    assert!(
        (rps.bottom_margin as f64 - ps.get("bottom_margin").unwrap().as_f64().unwrap()).abs() < TOL,
        "{}: bottom_margin",
        name
    );
    assert_eq!(
        rps.copy_count as i64,
        ps.get("copy_count").unwrap().as_i64().unwrap(),
        "{}: copy_count",
        name
    );

    // ---- scalars ----
    let sc = j.get("scalars").unwrap();
    let rs = &r.scalars;
    assert_eq!(
        rs.disp_ratio as i64,
        sc.get("disp_ratio").unwrap().as_i64().unwrap(),
        "{}: disp_ratio",
        name
    );
    assert_eq!(
        rs.unit as i64,
        sc.get("unit").unwrap().as_i64().unwrap(),
        "{}: unit",
        name
    );
    assert_eq!(
        rs.tip.as_deref(),
        sc.get("tip").unwrap().as_str(),
        "{}: tip",
        name
    );
    assert_eq!(
        rs.notes.as_deref(),
        sc.get("notes").unwrap().as_str(),
        "{}: notes",
        name
    );
    assert_eq!(
        rs.chart_style_name.as_deref(),
        sc.get("chart_style_name").unwrap().as_str(),
        "{}: chart_style_name",
        name
    );
    assert_eq!(
        rs.future_cell_num_exp.as_deref(),
        sc.get("future_cell_num_exp").unwrap().as_str(),
        "{}: future_cell_num_exp",
        name
    );
    assert_eq!(
        rs.image_max_width as i64,
        sc.get("image_max_width").unwrap().as_i64().unwrap(),
        "{}: image_max_width",
        name
    );
    assert_eq!(
        rs.image_max_height as i64,
        sc.get("image_max_height").unwrap().as_i64().unwrap(),
        "{}: image_max_height",
        name
    );
    assert_eq!(
        rs.page_border_mode as i64,
        sc.get("page_border_mode").unwrap().as_i64().unwrap(),
        "{}: page_border_mode",
        name
    );
    assert_eq!(
        rs.report_style_name.as_deref(),
        sc.get("report_style_name").unwrap().as_str(),
        "{}: report_style_name",
        name
    );
    assert_eq!(
        rs.validity_script.as_deref(),
        sc.get("validity_script").unwrap().as_str(),
        "{}: validity_script",
        name
    );
    assert_eq!(
        rs.big_data_set_name.as_deref(),
        sc.get("big_data_set_name").unwrap().as_str(),
        "{}: big_data_set_name",
        name
    );
    assert_eq!(
        rs.password.as_deref(),
        sc.get("password").unwrap().as_str(),
        "{}: password",
        name
    );
    assert_eq!(
        rs.submit as i64,
        sc.get("submit").unwrap().as_i64().unwrap(),
        "{}: submit",
        name
    );
    assert_eq!(
        rs.high_precision,
        sc.get("high_precision").unwrap().as_bool().unwrap(),
        "{}: high_precision",
        name
    );
    assert_eq!(
        rs.threshold as i64,
        sc.get("threshold").unwrap().as_i64().unwrap(),
        "{}: threshold",
        name
    );
}
