//! JSON schema strictness used by the parser.

use crate::reportdef;
use serde_json::Value as J;

pub fn schema() -> J {
    serde_json::from_str(include_str!("template.schema.json")).expect("bundled schema")
}

pub fn strict(j: &J) -> std::result::Result<(), reportdef::DefError> {
    fn walk(v: &J, s: &J, path: &str) -> std::result::Result<(), reportdef::DefError> {
        if path == "$.print_setup" {
            return Ok(());
        }
        if let (Some(o), Some(props)) = (v.as_object(), s["properties"].as_object()) {
            for (k, v) in o {
                let p = format!("{path}.{k}");
                let rule = props.get(k).ok_or_else(|| reportdef::DefError::Schema {
                    path: p.clone(),
                    detail: "未知字段".into(),
                })?;
                walk(v, rule, &p)?;
            }
        }
        if let (Some(a), Some(rule)) = (v.as_array(), s.get("items")) {
            for (i, v) in a.iter().enumerate() {
                walk(v, rule, &format!("{path}[{i}]"))?;
            }
        }
        Ok(())
    }
    walk(j, &schema(), "$")
}
