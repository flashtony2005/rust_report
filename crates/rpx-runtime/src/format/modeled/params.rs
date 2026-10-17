//! 参数声明投影。

use rpx_core::ReportDefine;
use serde_json::{json, Value as J};

use super::naming::{param_kind, param_type};

pub(super) fn params(rd: &ReportDefine, warnings: &mut Vec<String>) -> Option<Vec<J>> {
    if rd.params.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for (i, p) in rd.params.iter().enumerate() {
        let Some(name) = &p.name else {
            warnings.push(format!("params/{i}/name"));
            continue;
        };
        let Some(data_type) = param_type(p.data_type) else {
            warnings.push(format!("params/{i}/data_type"));
            continue;
        };
        let Some(kind) = param_kind(p.param_type) else {
            warnings.push(format!("params/{i}/kind"));
            continue;
        };
        let mut value = json!({"name": name, "data_type": data_type, "kind": kind});
        if let Some(v) = &p.value {
            value["default"] = json!(v);
        }
        if let Some(v) = &p.description {
            value["description"] = json!(v);
        }
        out.push(value);
    }
    Some(out)
}
