//! Shared data materialization, calculation and pagination. No HTTP dependencies.
use crate::{
    datasource::DataSourceRegistry,
    form,
    reportdef::{self, SourceError},
    snapshot,
};
use rpx_engine::EngineError;
use std::collections::HashMap;
pub fn render_request(req: &form::RenderRequest) -> Result<String, RenderError> {
    let (rd, macros) = reportdef::open_report_full(&req.report).map_err(RenderError::Source)?;
    render_core(req, &rd, &HashMap::new(), &macros)
}

/// 主链路异步版（HTTP handler 入口）：先**预取**全部 SQL 数据集（注册表
/// 逐条物化 [`RowSet`]），再进同步 [`render_core`]——引擎侧的 `sql_fetch`
/// 闭包是同步契约（G8），async 取数只在 server 编排层发生（产品同构：
/// 连接管理在编排层，引擎只见 `getConnectionFactory`）。
pub async fn render_request_ds(
    req: &form::RenderRequest,
    dsr: Option<&DataSourceRegistry>,
) -> Result<String, RenderError> {
    let (rd, macros) = reportdef::open_report_full(&req.report).map_err(RenderError::Source)?;
    let prefetch = prefetch_sql(&rd, dsr, &macros).await?;
    render_core(req, &rd, &prefetch, &macros)
}

/// SQL 数据集预取：`(dsn, sql_text) → RowSet`。注册表缺失 / dsn 未登记 /
/// 取数失败都**显式报错**（数据集缺失会被表达式求值期报错掩盖成更难定位
/// 的形态，见 `rpx_engine::dataset::load_datasets_full` 的同款口径）。
pub async fn prefetch_sql(
    rd: &rpx_core::ReportDefine,
    dsr: Option<&DataSourceRegistry>,
    macros: &rpx_engine::MacroTable,
) -> Result<HashMap<(String, String), rpx_core::rowset::RowSet>, RenderError> {
    let mut map = HashMap::new();
    if !rd.datasets.iter().any(|e| e.sql.is_some()) {
        return Ok(map);
    }
    let Some(dsr) = dsr else {
        let dsn = rd
            .datasets
            .iter()
            .find_map(|e| e.sql.as_ref().and_then(|s| s.data_source_name.clone()))
            .unwrap_or_default();
        return Err(RenderError::NoDataSource { dsn });
    };
    for e in &rd.datasets {
        let Some(sql) = e.sql.as_ref() else {
            continue;
        };
        let dsn = sql.data_source_name.as_deref().unwrap_or("").to_string();
        let text = sql.sql.as_deref().unwrap_or("").to_string();
        if map.contains_key(&(dsn.clone(), text.clone())) {
            continue; // 同 (dsn,sql) 只取一次（同连接同 SQL 的模板重复声明）
        }
        let src = dsr
            .get(&dsn)
            .ok_or_else(|| RenderError::UnknownDataSource {
                dsn: dsn.clone(),
                available: dsr.names().iter().map(|s| s.to_string()).collect(),
            })?;
        // ⛔ 宏替换只作用于**发给数据库的那一份**，`map` 的键仍是**原文**：
        // 引擎侧 `sql_fetch` 闭包是拿模板里的 SQL 原文来查这张表的
        // （见本函数下方 `.get(&(dsn.to_string(), sql.to_string()))`），
        // 把键换成展开后的文本会让查表**永远落空**。
        // 宏可放"任何位置"包含数据集定义（`报表引擎设计.md:244`），这是它的落点。
        let expanded = rpx_engine::substitute_macros(&text, macros).map_err(RenderError::Engine)?;
        let rs = src
            .fetch_sql(&expanded)
            .await
            .map_err(RenderError::SqlFetch)?;
        map.insert((dsn, text), rs);
    }
    Ok(map)
}

/// 同步主链路（输入 = 表单 + 已预取的 SQL 数据集）→ 快照 JSON。
fn render_core(
    req: &form::RenderRequest,
    rd: &rpx_core::ReportDefine,
    prefetch: &HashMap<(String, String), rpx_core::rowset::RowSet>,
    macros: &rpx_engine::MacroTable,
) -> Result<String, RenderError> {
    let grid = calculate_grid(req, rd, prefetch, macros)?;
    snapshot::build(rd, &grid, &req.report_name).map_err(RenderError::Snapshot)
}

/// 计算主链路（`/render` 与 `/export` 的**共享段**，主链路 ②-⑨ 步）：
/// 请求 → 计算态网格（含分页结果与 `page_count` 覆盖）。
///
/// M3-06 口径（原 `render_core` 内注释原样保留）：`/render` 的 pagination =
/// **模板真值口径**（纸码表 + 模板边距，`PagerInfo(PrintSetup)` 路径），覆盖
/// `calculate_with` 内的探针口径（595×842 是产品探针覆盖出来的夹具对拍基准
/// ——双口径见 `pager.rs` 模块文档）。守卫（pagerStyle/多栏/列区带 + 可用
/// 尺寸非正）在此复核，报 400。
///
/// `/export`（M6-03）复用同一趟：导出打的是**整个计算矩阵**（不分页、不按纸切，
/// spec-m6 §4），但走同一守卫与同一页数覆盖 ⇒ 出口分家不产生口径分叉。
pub fn calculate_grid(
    req: &form::RenderRequest,
    rd: &rpx_core::ReportDefine,
    prefetch: &HashMap<(String, String), rpx_core::rowset::RowSet>,
    macros: &rpx_engine::MacroTable,
) -> Result<rpx_engine::arena::CalcGrid, RenderError> {
    for entry in &rd.datasets {
        let Some(file) = entry.file.as_ref() else {
            continue;
        };
        let Some(file_name) = file.file_name.as_deref() else {
            continue;
        };
        if !req.datasets.contains_key(file_name) {
            return Err(RenderError::MissingFile {
                file: file_name.to_string(),
                dataset: file.name.clone().unwrap_or_else(|| file_name.to_string()),
            });
        }
    }
    let ds = crate::data::load(req, rd, prefetch)?;
    calculate_datasets(rd, &ds, &req.params, macros)
}

pub fn calculate_datasets(
    rd: &rpx_core::ReportDefine,
    datasets: &[rpx_engine::expr::ds::DataSet],
    params: &rpx_engine::ParamTable,
    macros: &rpx_engine::MacroTable,
) -> Result<rpx_engine::arena::CalcGrid, RenderError> {
    let mut grid =
        rpx_engine::calculate_full(rd, datasets, params, macros).map_err(RenderError::Engine)?;
    crate::snapshot::graphics::validate(&grid).map_err(RenderError::Graphics)?;
    let pg = rpx_engine::pager::paginate_with_setup(&grid, &grid.print_setup)
        .map_err(RenderError::Engine)?;
    grid.page_count = pg.page_count();
    grid.pagination = Some(pg);
    Ok(grid)
}

/// 渲染错误 → HTTP 映射（400 = 客户端输入；500 = 服务端缺陷）。
#[derive(Debug)]
pub enum RenderError {
    /// 文件解析或数据集绑定失败，包含数据集名称与原因。
    Data(String),
    /// 报表来源解不开：`.rpx` 字节坏 / 定义 JSON 不合法 / 格式无法判定。
    Source(SourceError),
    /// 引擎拒绝：缺数据集 / 未登记参数 / 编码坏 / 循环引用 / 不支持的分页…。
    Engine(EngineError),
    /// 上传不完整：模板引用的 CSV 不在表单里（按**文件名**报，见上文）。
    MissingFile {
        file: String,
        dataset: String,
    },
    /// 模板含 SQL 数据集但服务端没有数据源注册表（`serve` 未给
    /// `--datasources`）——显式 400，消息里带 dsn 与补救路径。
    NoDataSource {
        dsn: String,
    },
    /// 注册表存在但模板引用的 dsn 未登记（消息列出已登记数据源名）。
    UnknownDataSource {
        dsn: String,
        available: Vec<String>,
    },
    /// SQL 取数失败（连接/查询/列型）——sqlx 错误文本原样透传给前端上屏。
    SqlFetch(rpx_ds::DsError),
    /// 定义 JSON → `.rpx` 序列化失败（`POST /reports` 的归一化步骤）。
    Serialize(String),
    /// 快照映射不变式被违反（`geom::merge_data` 的网格校验）——不该发生。
    Snapshot(String),
    /// 图形绑定产生的无效图片/模板或目标格式不支持的导出内容。
    Graphics(String),
    Export(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RenderError {}
