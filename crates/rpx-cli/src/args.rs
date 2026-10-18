use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
fn source_path(value: &str) -> Result<PathBuf, String> {
    if value == "-" {
        Err("源码与结果必须使用文件路径；仅 --params-file 和 --patch-file 支持 stdin".into())
    } else {
        Ok(value.into())
    }
}
#[derive(Parser)]
#[command(
    name = "rpx",
    version,
    about = "Agent 管理报表定义与执行；默认输出 JSON"
)]
pub struct Cli {
    #[arg(long,global=true,default_value="json",value_parser=["json","text"])]
    pub output: String,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Args, Default)]
pub struct Page {
    /// 每页条数（1..=500）
    #[arg(long,default_value_t=50,value_parser=clap::value_parser!(u16).range(1..=500))]
    pub limit: u16,
    /// 起始偏移
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
}
#[derive(Args)]
pub struct Inputs {
    /// 文件数据集所在目录
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    /// 参数 JSON 文件；`-` 表示 stdin
    #[arg(long)]
    pub params_file: Option<PathBuf>,
    /// 数据源注册表（只读取配置）
    #[arg(long)]
    pub datasources: Option<PathBuf>,
    /// 导出格式，可重复
    #[arg(long="export",value_parser=["csv","xlsx","print-xlsx","html","pdf"])]
    pub exports: Vec<String>,
}
#[derive(Args)]
pub struct Library {
    /// 目录库根路径
    #[arg(long)]
    pub library: PathBuf,
}
#[derive(Subcommand)]
pub enum Command {
    /// 环境与能力自检
    Doctor {
        /// 检查目录库是否可读可写
        #[arg(long)]
        library: Option<PathBuf>,
        /// 只做 JSON 形状检查，不建立数据库连接
        #[arg(long)]
        datasources: Option<PathBuf>,
    },
    /// 能力自描述：命令面/导出/限制/错误码（Agent 无需读文档）
    Capabilities,
    /// 静态检查：拦截"不报错但产物错"的写法
    Lint {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        /// 数据集目录；给了才检查文件是否存在
        #[arg(long)]
        data_dir: Option<PathBuf>,
        /// 有 warning 也返回退出码 3（CI 闸门用）
        #[arg(long)]
        strict: bool,
    },
    /// 输出内置 JSON schema
    Schema {
        #[arg(value_parser=["template"])]
        kind: String,
    },
    /// 模板查看、校验、创建与补丁
    Template {
        #[command(subcommand)]
        command: Template,
    },
    #[cfg(feature = "legacy-rpx")]
    /// legacy .rpx ↔ JSON 迁移
    Format {
        #[command(subcommand)]
        command: FormatCommand,
    },
    /// 直接运行一个 JSON 模板
    Run {
        /// JSON 模板路径
        #[arg(value_parser=source_path)]
        file: PathBuf,
        /// 结果与产物目录
        #[arg(long)]
        out_dir: PathBuf,
        #[command(flatten)]
        inputs: Inputs,
    },
    /// 查询固定结果
    Result {
        #[command(subcommand)]
        command: ResultCommand,
    },
    /// 数据源/数据集检查
    Dataset {
        #[command(subcommand)]
        command: Dataset,
    },
    /// 内置函数与示例
    Catalog {
        #[command(subcommand)]
        command: Catalog,
    },
    /// 目录库初始化
    Library {
        #[command(subcommand)]
        command: LibraryCommand,
    },
    /// 目录库中的报表生命周期
    Report {
        #[command(subcommand)]
        command: Report,
    },
}
#[cfg(feature = "legacy-rpx")]
#[derive(Subcommand)]
pub enum FormatCommand {
    /// .rpx → JSON v1；preserve 会保留原始字节迁移快照
    ToJson {
        file: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long,default_value="preserve",value_parser=["strict","preserve","drop"])]
        mode: String,
    },
    /// JSON v1 → .rpx；preserve 且未编辑时原样还回迁移快照
    ToRpx {
        file: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long,default_value="preserve",value_parser=["strict","preserve","drop"])]
        mode: String,
        /// 允许含快照的结构性编辑；仅保证坐标不变的重叠区
        #[arg(long)]
        allow_structural: bool,
    },
    /// 读 → JSON → 模型/字节 → 再读
    Verify {
        file: PathBuf,
        #[arg(long,default_value="preserve",value_parser=["strict","preserve","drop"])]
        mode: String,
    },
    /// 检查 JSON 定义是否可被当前 reader 接受
    Check { file: PathBuf },
    /// v0/v1 → v1：补 format / format_version / schema
    Normalize {
        file: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}
#[derive(Subcommand)]
pub enum Template {
    /// 查看模板摘要/单个单元格
    Inspect {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        #[arg(long)]
        cell: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// 离线校验模板
    Validate {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        #[arg(long)]
        params_file: Option<PathBuf>,
        /// 真读文件数据集，核对文件存在与列名形状
        #[arg(long)]
        probe_data: bool,
        /// 探针的数据集目录（默认模板所在目录）
        #[arg(long)]
        data_dir: Option<PathBuf>,
    },
    /// 从内置示例创建模板包
    Create {
        #[arg(long)]
        example: String,
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// 应用 RFC 6902 补丁
    Patch {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        #[arg(long)]
        patch_file: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    #[cfg(feature = "legacy-rpx")]
    /// 编译为 legacy .rpx
    Compile {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}
#[derive(Subcommand)]
pub enum ResultCommand {
    /// 分页/按区域查看 result.json
    Inspect {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        #[arg(long)]
        range: Option<String>,
        #[arg(long)]
        origin: Option<String>,
        #[command(flatten)]
        page: Page,
    },
}
#[derive(Subcommand)]
pub enum Dataset {
    /// 预览文件/SQL 数据集
    Inspect {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long)]
        data_dir: Option<PathBuf>,
        #[arg(long)]
        datasources: Option<PathBuf>,
        #[arg(long,default_value_t=5,value_parser=clap::value_parser!(u16).range(1..=500))]
        limit: u16,
    },
}
#[derive(Subcommand)]
pub enum Catalog {
    /// 列出内置函数
    Functions {
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// 列出内置示例
    Examples,
}
#[derive(Subcommand)]
pub enum LibraryCommand {
    /// 初始化目录库
    Init {
        dir: PathBuf,
        #[arg(long)]
        presets: bool,
    },
}
#[derive(Subcommand)]
pub enum Report {
    /// 列出报表
    List {
        #[command(flatten)]
        library: Library,
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    /// 查看报表源码与状态
    Show {
        id: String,
        #[command(flatten)]
        library: Library,
    },
    /// 查看运行历史
    Runs {
        id: String,
        #[command(flatten)]
        library: Library,
        #[command(flatten)]
        page: Page,
    },
    /// 复制报表
    Copy {
        id: String,
        #[arg(long = "as")]
        new_id: String,
        #[command(flatten)]
        library: Library,
    },
    /// 应用 RFC 6902 补丁
    Patch {
        id: String,
        #[arg(long)]
        patch_file: PathBuf,
        #[arg(long)]
        if_match: String,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        library: Library,
    },
    /// 离线校验模板
    Validate {
        id: String,
        #[command(flatten)]
        library: Library,
    },
    /// 运行库内报表
    Run {
        id: String,
        #[arg(long)]
        if_match: String,
        #[command(flatten)]
        library: Library,
        #[command(flatten)]
        inputs: Inputs,
    },
    /// 更新名称/描述
    Update {
        id: String,
        #[arg(long)]
        if_match: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[command(flatten)]
        library: Library,
    },
    /// 从 JSON 定义创建报表
    ImportJson {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        #[arg(long)]
        id: String,
        #[arg(long)]
        data_dir: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        library: Library,
    },
    #[cfg(feature = "legacy-rpx")]
    /// 从 legacy .rpx 创建报表
    ImportRpx {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        #[arg(long)]
        id: String,
        #[arg(long)]
        data_dir: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        library: Library,
    },
    /// 删除报表（必须带修订）
    Delete {
        id: String,
        #[arg(long)]
        if_match: String,
        #[command(flatten)]
        library: Library,
    },
    /// 把库内 v0 JSON 模板迁移到 v1（可备份）
    Migrate {
        #[arg(long)]
        dry_run: bool,
        /// 首次写入前把 template/manifest 复制到库旁备份目录
        #[arg(long)]
        backup: bool,
        #[command(flatten)]
        library: Library,
    },
    /// 清理旧运行记录
    Prune {
        id: String,
        #[arg(long,default_value_t=10,value_parser=clap::value_parser!(u16).range(1..=500))]
        keep: u16,
        #[command(flatten)]
        library: Library,
    },
}
impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Doctor { .. } => "doctor",
            Self::Capabilities => "capabilities",
            Self::Lint { .. } => "lint",
            Self::Schema { .. } => "schema.template",
            Self::Run { .. } => "run",
            Self::Template { command } => match command {
                Template::Inspect { .. } => "template.inspect",
                Template::Validate { .. } => "template.validate",
                Template::Create { .. } => "template.create",
                Template::Patch { .. } => "template.patch",
                #[cfg(feature = "legacy-rpx")]
                Template::Compile { .. } => "template.compile",
            },
            #[cfg(feature = "legacy-rpx")]
            Self::Format { command } => match command {
                FormatCommand::ToJson { .. } => "format.to-json",
                FormatCommand::ToRpx { .. } => "format.to-rpx",
                FormatCommand::Verify { .. } => "format.verify",
                FormatCommand::Check { .. } => "format.check",
                FormatCommand::Normalize { .. } => "format.normalize",
            },
            Self::Result { .. } => "result.inspect",
            Self::Dataset { .. } => "dataset.inspect",
            Self::Catalog { command } => match command {
                Catalog::Functions { .. } => "catalog.functions",
                Catalog::Examples => "catalog.examples",
            },
            Self::Library { .. } => "library.init",
            Self::Report { command } => match command {
                Report::List { .. } => "report.list",
                Report::Show { .. } => "report.show",
                Report::Runs { .. } => "report.runs",
                Report::Copy { .. } => "report.copy",
                Report::Patch { .. } => "report.patch",
                Report::Validate { .. } => "report.validate",
                Report::Run { .. } => "report.run",
                Report::Update { .. } => "report.update",
                Report::ImportJson { .. } => "report.import-json",
                #[cfg(feature = "legacy-rpx")]
                Report::ImportRpx { .. } => "report.import-rpx",
                Report::Delete { .. } => "report.delete",
                Report::Migrate { .. } => "report.migrate",
                Report::Prune { .. } => "report.prune",
            },
        }
    }
}
