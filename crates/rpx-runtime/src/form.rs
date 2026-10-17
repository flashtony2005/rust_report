//! In-memory execution inputs and JSON parameters.
use rpx_engine::ParamTable;
use std::collections::BTreeMap;
pub struct RenderRequest {
    pub report: Vec<u8>,
    pub report_name: String,
    pub datasets: BTreeMap<String, Vec<u8>>,
    pub params: ParamTable,
}
