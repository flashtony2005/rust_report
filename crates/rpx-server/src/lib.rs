//! rpx-server 库面：快照映射（[`snapshot`]）+ HTTP 面（[`http`]）+ 导出面（[`export`]）
//! + 报表定义（[`reportdef`]）。
//!
//! 二进制（`rpx-server`）只是本库的 dev 外壳：`snapshot` 子命令直出快照 JSON
//! （M4 开发期数据管道），`serve` 子命令起 HTTP 服务（M3-05 `POST /render` +
//! M6-03 `POST /export`）。库/bin 分层的目的：集成测试能 import 主链路纯函数
//! （[`http::render_request`] / [`export::export_request`]），与「真 socket
//! 响应体」双轨对拍（`tests/render_http.rs` / `tests/export_http.rs`）。
//!
//! 依赖登记（G5）：axum / tokio / serde_json —— 逐条理由见
//! `docs/code-review.md` §3.2；`rpx-export` 是 path 依赖（受审层分层矩阵，G8）。

pub mod datasource;
pub mod export;
pub mod form;
pub mod http;
pub mod reportdef;
pub mod snapshot;
