//! 参数声明（定义 JSON → [`ReportDefine::params`]）。
//!
//! 五个字段：`name` / `data_type` / `default` / `description` / `kind`。
//! 落点是 [`rpx_core::param::Param`] 的 5 个语义槽（另 5 槽是读侧丢弃槽，
//! 附件 B.2）。
//!
//! # 验证闭环
//!
//! 声明一路走到 **`GET /reports/{id}/meta`**：`parse` → [`rpx_core::to_bytes`]
//! → `AssetStore::put` → [`rpx_core::open_bytes`] → `param_entries` → `report_params`
//! 表 ⇒ `/meta`。这条链**同时压解析侧与写侧**——参数清单要原样读回，
//! 就必须先正确写进 `.rpx` 字节、再正确读出来。
//!
//! # 与 `style.rs` 分开的理由
//!
//! 同 `style.rs`：`parse.rs` 管语义，本文件管**参数**这一族的形态与码表；
//! `parse.rs` 已 402 行，再加进来会越过 G4 的 450 行预警线。
//!
//! 契约正文见 `docs/report-def-json.md` §3.5 / §4.6。

use serde_json::{Map, Value as J};

use rpx_core::param::Param;

use super::json::{arr, as_str, err, field, obj, opt_str};
use super::DefError;

/// `data_type` 枚举名 → `Types2.DT_*` 码。
///
/// 出处 `Types2.java:22-44`（探针实测见 `tests/fixtures/ProbeParamFull.java:177-186`）。
/// 名字取**库侧原名**而非自造缩写——`bigdecimal` 对应 `DT_BIGDEC` 而不是 `decimal`；
/// 数组型用 `[]` 后缀（`DT_INT_ARR` / `DT_STRING_ARR`）。
///
/// 码本身**不连续**（1/2/3/**4 缺**/5/6/7 …）：`DT_DATE` 等未列入的类型在
/// `.rpx` 里存在但探针未实测转换矩阵 ⇒ 本契约不开放，**宁可少给也不猜码**。
const DATA_TYPES: &[(&str, i8)] = &[
    ("int", 1),
    ("long", 2),
    ("short", 3),
    ("float", 5),
    ("double", 6),
    ("bigdecimal", 7),
    ("string", 11),
    ("bool", 12),
    ("int[]", 51),
    ("string[]", 61),
];

/// `kind` 枚举名 → `Param.PARAM_*` 码（`Param.java:22-24`）。
///
/// `static` 是 `PARAM_NORMAL=0`：**静态参数**的默认值在渲染时就参与计算；
/// `dynamic`/`attribute` 原样保存码位不归并——`/meta` 的 `is_static` 只是
/// `param_type == 0` 的派生（`rpx-ds/src/asset.rs:70-72`）。
const KINDS: &[(&str, i8)] = &[("static", 0), ("dynamic", 1), ("attribute", 2)];

/// 顶层 `params[]` → 参数清单（缺省 ⇒ 空清单，不是错误）。
pub(super) fn parse_params(root: &Map<String, J>) -> Result<Vec<Param>, DefError> {
    let Some(list) = root.get("params") else {
        return Ok(Vec::new());
    };
    let list = arr(list, "$.params")?;
    let mut out = Vec::with_capacity(list.len());
    for (i, p) in list.iter().enumerate() {
        out.push(parse_param(p, &format!("$.params[{i}]"))?);
    }
    Ok(out)
}

fn parse_param(v: &J, path: &str) -> Result<Param, DefError> {
    let o = obj(v, path)?;

    // `name` **必填**：`param_entry` 会丢弃无名参数（`asset.rs:231` 的 `?`），
    // 一条静默消失的声明比一个 400 更难查 ⇒ 这里先拦。
    let name = as_str(field(o, "name", path)?, &format!("{path}.name"))?.to_string();

    // `data_type` 必填：不设默认型。静默当成 `string` 正是「不猜」要禁的形态。
    let dt_path = format!("{path}.data_type");
    let data_type = lookup(
        DATA_TYPES,
        as_str(field(o, "data_type", path)?, &dt_path)?,
        &dt_path,
    )?;

    let kind_path = format!("{path}.kind");
    let param_type = match o.get("kind") {
        None => 0, // 静态——绝大多数参数都是
        Some(k) => lookup(KINDS, as_str(k, &kind_path)?, &kind_path)?,
    };

    Ok(Param {
        param_type,
        data_type,
        name: Some(name),
        description: opt_str(o, "description", path)?.map(str::to_string),
        // `default` 在 `.rpx` 里是 String（`Param` 第 7 槽）⇒ JSON 侧必须是字符串。
        // 数字/布尔**显式拒绝**而非偷偷 `to_string`：`1.0` 的文本形态有歧义，
        // 而这条值会原样进 `/meta` 的 `default_val` 给前端填表单。
        value: match o.get("default") {
            None => None,
            Some(d) => Some(as_str(d, &format!("{path}.default"))?.to_string()),
        },
    })
}

/// 枚举查码（未知取值报错并列出全部合法值——不猜、不静默丢弃）。
fn lookup(table: &[(&str, i8)], s: &str, path: &str) -> Result<i8, DefError> {
    table
        .iter()
        .find(|(name, _)| *name == s)
        .map(|(_, code)| *code)
        .ok_or_else(|| {
            let all = table
                .iter()
                .map(|(n, _)| *n)
                .collect::<Vec<_>>()
                .join(" / ");
            err(path, format!("未知取值 {s:?}（合法取值：{all}）"))
        })
}

/// 合法取值清单（供错误消息）。
#[cfg(test)]
fn names(table: &[(&str, i8)]) -> String {
    table
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(root: &str) -> Vec<Param> {
        let r: Map<String, J> = serde_json::from_str(root).expect("测试 JSON 合法");
        parse_params(&r).expect("参数声明应被接受")
    }

    /// DT_* 码表**全量**（`Types2.java:22-44`）：码**写死**在期望值里，
    /// 不从 `DATA_TYPES` 自己取——否则这条判据是同义反复，把码改了它也照绿
    /// （已用 DT_INT/DT_LONG 对调的故障注入实证过）。
    #[test]
    fn data_type_enum_maps_to_the_contract_codes() {
        let expected: &[(&str, i8)] = &[
            ("int", 1),
            ("long", 2),
            ("short", 3),
            ("float", 5),
            ("double", 6),
            ("bigdecimal", 7),
            ("string", 11),
            ("bool", 12),
            ("int[]", 51),
            ("string[]", 61),
        ];
        assert_eq!(DATA_TYPES.len(), expected.len(), "码表条目数变了");
        for (name, code) in expected {
            let got = lookup(DATA_TYPES, name, "$.p").expect("合法枚举名");
            assert_eq!(got, *code, "{name} → DT_ 码 {code}");
        }
    }

    /// `kind` 三态的码同样写死（`Param.java:22-24`）。
    #[test]
    fn kind_codes_are_hardcoded_against_the_contract() {
        let expected: &[(&str, i8)] = &[("static", 0), ("dynamic", 1), ("attribute", 2)];
        for (name, code) in expected {
            assert_eq!(
                lookup(KINDS, name, "$.p").unwrap(),
                *code,
                "{name} → {code}"
            );
        }
    }

    /// 码表必须与契约文档 §4.6 逐字一致（文档是唯一事实源，这里是回归网）。
    #[test]
    fn code_tables_match_the_documented_names() {
        assert_eq!(
            names(DATA_TYPES),
            "int / long / short / float / double / bigdecimal / string / bool / int[] / string[]"
        );
        assert_eq!(names(KINDS), "static / dynamic / attribute");
    }

    /// `kind` 三态：静态是默认，动态/属性原样落 `param_type`（`is_static` 只认 0）。
    #[test]
    fn kind_enum_maps_to_param_type_codes() {
        let p = parse(r#"{"params":[{"name":"a","data_type":"string"}]}"#);
        assert_eq!(p[0].param_type, 0, "kind 缺省 = static（PARAM_NORMAL）");

        let p = parse(r#"{"params":[{"name":"a","data_type":"string","kind":"dynamic"}]}"#);
        assert_eq!(p[0].param_type, 1);

        let p = parse(r#"{"params":[{"name":"a","data_type":"string","kind":"attribute"}]}"#);
        assert_eq!(p[0].param_type, 2);
    }

    /// 五个字段全部落地（`default` → `value`、`description` 原样）。
    #[test]
    fn all_five_slots_land() {
        let p = parse(
            r#"{"params":[{"name":"startDate","data_type":"string","default":"2024-01-01",
                             "description":"开始日期","kind":"static"}]}"#,
        );
        assert_eq!(
            p[0],
            Param {
                param_type: 0,
                data_type: 11,
                name: Some("startDate".to_string()),
                description: Some("开始日期".to_string()),
                value: Some("2024-01-01".to_string()),
            }
        );
    }

    /// `name` 必填：`param_entry` 会丢无名参数，这里必须先拦（否则声明静默消失）。
    #[test]
    fn name_is_required() {
        let r: Map<String, J> =
            serde_json::from_str(r#"{"params":[{"data_type":"string"}]}"#).unwrap();
        assert_eq!(
            format!("{}", parse_params(&r).unwrap_err()),
            "$.params[0].name：缺少必填字段"
        );
    }

    /// 未知枚举取值一律报错并列出合法值（不猜、不静默忽略）。
    #[test]
    fn unknown_enum_values_are_rejected_loudly() {
        for (root, want) in [
            (
                r#"{"params":[{"name":"p","data_type":"decimal"}]}"#,
                "$.params[0].data_type",
            ),
            (
                r#"{"params":[{"name":"p","data_type":"string","kind":"weird"}]}"#,
                "$.params[0].kind",
            ),
        ] {
            let r: Map<String, J> = serde_json::from_str(root).unwrap();
            let e = parse_params(&r).unwrap_err();
            let msg = format!("{e}");
            assert!(msg.starts_with(want), "路径应为 {want}，实得 {msg}");
            assert!(msg.contains("合法取值"), "错误消息应列出合法值：{msg}");
        }
    }

    /// `default` 必须是字符串：`.rpx` 里参数值是 String，`1.0` 的文本形态有歧义。
    #[test]
    fn default_must_be_a_string() {
        let r: Map<String, J> =
            serde_json::from_str(r#"{"params":[{"name":"p","data_type":"int","default":10}]}"#)
                .unwrap();
        assert_eq!(
            format!("{}", parse_params(&r).unwrap_err()),
            "$.params[0].default：应为字符串，实得 数字"
        );
    }

    /// 声明顺序无关，多条互不干扰。
    #[test]
    fn multiple_params_keep_declaration_order() {
        let p = parse(
            r#"{"params":[
                {"name":"a","data_type":"int"},
                {"name":"b","data_type":"string","default":"x"},
                {"name":"c","data_type":"double","kind":"dynamic"}
            ]}"#,
        );
        let got: Vec<(&str, i8, i8)> = p
            .iter()
            .map(|q| (q.name.as_deref().unwrap(), q.data_type, q.param_type))
            .collect();
        assert_eq!(got, vec![("a", 1, 0), ("b", 11, 0), ("c", 6, 1)]);
    }

    /// 缺省 `params` ⇒ 空清单（不是错误；v1 定义可以不声明参数）。
    #[test]
    fn absent_params_is_an_empty_list() {
        assert!(parse(r#"{"grid":{"rows":1,"cols":1}}"#).is_empty());
        assert!(parse(r#"{"params":[]}"#).is_empty());
    }

    /// 形态错误也要带路径：`params` 本身不是数组、元素不是对象。
    #[test]
    fn malformed_shape_carries_the_path() {
        let r: Map<String, J> = serde_json::from_str(r#"{"params": {}}"#).unwrap();
        assert_eq!(
            format!("{}", parse_params(&r).unwrap_err()),
            "$.params：应为数组，实得 对象"
        );
        let r: Map<String, J> = serde_json::from_str(r#"{"params": [1]}"#).unwrap();
        let e = parse_params(&r).unwrap_err();
        assert!(format!("{e}").starts_with("$.params[0]"), "实得 {e}");
    }
}
