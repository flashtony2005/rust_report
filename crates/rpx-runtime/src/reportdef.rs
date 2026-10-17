//! 报表定义（JSON）→ [`ReportDefine`]：**「前端定报表格式」的落地入口**（M7-1）。
//!
//! # 为什么要有这条通道
//!
//! 产品模板（`.rpx`）平时由 Windows 设计器生成，本机没有那条产线。但
//! `ReportDefine` 的字段**全部 `pub`**、默认值全在 `rpx_core::props::def`，
//! 因此「**用代码/JSON 造一份合法模板**」在技术上完全可行——本模块就是这条路的
//! 生产实现（此前只在 `examples/sqlite_chinese_report` 里作为示例存在）。
//!
//! # 与 `.rpx` 的关系（两条入口，一个出口）
//!
//! [`open_report`] 按**内容**分派，两条入口都产出同一个 `ReportDefine`，下游
//! （`load_datasets_full` → `calculate_with` → `paginate_with_setup` → 快照/导出）
//! **完全不分叉**：
//!
//! | 输入 | 判定 | 路径 |
//! |---|---|---|
//! | `.rpx` 字节 | 前 4 字节 = `RQQR` | [`rpx_core::open_bytes`]（既有读侧，零改动） |
//! | 定义 JSON | 首个非空白字节 = `{` | [`parse::build`]（本模块，纯内存构造） |
//!
//! # 边界（诚实声明）
//!
//! * **`.rpx` 字节已可产出**（2026-10-16 补，M7-2）：写侧序列化器
//!   [`rpx_core::to_bytes`] 对 37 个真实夹具**逐字节复现**（`docs/report-def-json.md` §7.1）。
//!   定义 JSON 因此既能直接渲染/导出，也能经 `POST /reports` **归一化**为 `.rpx`
//!   入库、与 Java 侧互通。
//! * 仍未支持（v1 范围之外）：超链接、批注、`cell_graph`、子报表、
//!   `to_excel_mode` / `adjust_size_mode` / 对角线 / `extend_area`。
//!   这些需要时按同一模式加（`parse.rs` 加字段 + `build.rs` 加装配）。
//! * **单元格样式已支持**（2026-10-16 补，M7-3）：字体 / 字号 / 粗斜下划线 /
//!   前景背景色 7 个字段，见 [`style`] 与 `docs/report-def-json.md` §4.5。它们与
//!   `/render` 快照的 `cellData[r][c].s` 严格互逆。
//! * **参数声明已支持**（2026-10-16 补，M7-4）：`params[]` 的 `name` / `data_type` /
//!   `default` / `description` / `kind` 五个字段，见 [`params`] 与
//!   `docs/report-def-json.md` §4.6。声明经 `to_bytes` 入库后由
//!   `GET /reports/{id}/meta` 读回 —— 该链路**同时压解析侧与写侧**。
//! * **单元格属性扩展已支持**（2026-10-16 补，M7-5）：边框（4 边 × 3 槽）/
//!   数字格式 / 缩进 / 旋转角 / `extend_mode`，见 [`cellprops`] 与
//!   `docs/report-def-json.md` §4.7。这些字段**不在快照里**（`spec-m4.md` §4.3
//!   列为后置）⇒ 判据走**模型往返**（`tests/definition_roundtrip.rs`）。
//! * **打印设置覆盖已支持**（2026-10-16 补，M7-6）：顶层 `print_setup` 的 17 个
//!   语义明确字段，见 [`printsetup`] 与 `docs/report-def-json.md` §3.6 / §4.7。
//!   ⚠ **码表字段（`paper`/`orient`/`pager_style`/…）不开放**：附件 D 没给码表、
//!   夹具只有默认值 ⇒ 按「不猜」纪律显式 400，不静默丢弃。
//!
//! Schema 正文见 `docs/report-def-json.md`。

use std::fmt;

use rpx_core::{ReportDefine, RpxError};
use rpx_engine::MacroTable;

mod bands;
mod build;
mod cellprops;
mod chart;
mod format;
mod graphics;
mod heads;
mod json;
pub(crate) mod literal;
mod macros;
mod params;
mod parse;
mod print_config;
mod print_options;
mod printsetup;
mod style;

#[cfg(test)]
mod tests;

pub use build::{
    blank_cell, expr_cell, text_cell, Builder, KEY_COL_WIDTH, KEY_ROW_BREAK_PAGE, KEY_ROW_HEIGHT,
    KEY_VALUE, MERGE_SAME_BOTH, MERGE_SAME_HORIZONTAL, MERGE_SAME_VERTICAL,
};
pub use parse::{build, FORMAT, FORMAT_VERSION, VERSION};

/// 定义 JSON 不合法。
#[derive(Debug)]
pub enum DefError {
    /// JSON 文本本身解析失败（语法错）。
    Json(String),
    /// 结构/取值不合法；`path` 是 JSON 路径（如 `$.cells[3].merge.rows`）。
    Schema { path: String, detail: String },
}

impl fmt::Display for DefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DefError::Json(e) => write!(f, "JSON 解析失败：{e}"),
            DefError::Schema { path, detail } => write!(f, "{path}：{detail}"),
        }
    }
}

impl std::error::Error for DefError {}

/// 报表来源错误：`.rpx` 坏 / JSON 坏 / 两者都不是。
#[derive(Debug)]
pub enum SourceError {
    /// `.rpx` 字节解不开（魔数/截断/页长/页脚）。
    Core(RpxError),
    /// 定义 JSON 不合法。
    Def(DefError),
    /// 既无 `RQQR` 魔数、也不是 JSON 对象 —— 无法判定格式（显式报错，不猜）。
    UnknownFormat,
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceError::Core(e) => write!(f, "{e}"),
            SourceError::Def(e) => write!(f, "{e}"),
            SourceError::UnknownFormat => write!(
                f,
                "无法识别的报表格式：既不是 .rpx（缺 RQQR 魔数），也不是 JSON 定义（首个非空白字节不是 `{{`）"
            ),
        }
    }
}

impl std::error::Error for SourceError {}

/// `.rpx` 文件层魔数（`docs/format-contract.md` §0）。
const RPX_MAGIC: &[u8; 4] = b"RQQR";

/// 按**内容**分派报表来源 → [`ReportDefine`]。
///
/// 判定顺序：`RQQR` 魔数 → `.rpx` 读侧；首个非空白字节 `{` → 定义 JSON。
/// 两者都不命中 → [`SourceError::UnknownFormat`]（**不猜**：既不静默当 .rpx
/// 解析，也不静默当 JSON 解析）。
pub fn open_report(bytes: &[u8]) -> Result<ReportDefine, SourceError> {
    open_report_full(bytes).map(|(rd, _)| rd)
}

/// 同 [`open_report`]，但**连宏表一起**返回（M7-8）。
///
/// ⛔ 两条入口的宏**能力不同**，这是**已声明边界**不是疏漏：
/// `.rpx` 的宏存在 `MacroMetaData`（`_$24`）里，该记录的 `fillRecord` 格式本仓
/// **从未逆向**（`spec-m1.md:113`："只保留原始字节"）⇒ `.rpx` 入口的宏表**恒为空**。
/// 定义 JSON 入口则从顶层 `macros` 块取（`docs/report-def-json.md` §3.7）。
/// 跑计算一律走本函数，免得"JSON 里写了宏、渲染时静默没生效"。
pub fn open_report_full(bytes: &[u8]) -> Result<(ReportDefine, MacroTable), SourceError> {
    if bytes.starts_with(RPX_MAGIC) {
        return rpx_core::open_bytes(bytes)
            .map(|rd| {
                let macros = MacroTable::from_report(&rd);
                (rd, macros)
            })
            .map_err(SourceError::Core);
    }
    let first = bytes.iter().find(|b| !b.is_ascii_whitespace());
    if first == Some(&b'{') {
        // 定义 JSON 必须是 UTF-8（前端 `JSON.stringify` 天然如此）。
        let text = std::str::from_utf8(bytes).map_err(|e| {
            SourceError::Def(DefError::Json(format!("定义 JSON 不是合法 UTF-8：{e}")))
        })?;
        return parse::build_full(text).map_err(SourceError::Def);
    }
    Err(SourceError::UnknownFormat)
}
