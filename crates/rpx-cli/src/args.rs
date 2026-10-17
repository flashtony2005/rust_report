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
    #[arg(long,default_value_t=50,value_parser=clap::value_parser!(u16).range(1..=500))]
    pub limit: u16,
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
}
#[derive(Args)]
pub struct Inputs {
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    #[arg(long)]
    pub params_file: Option<PathBuf>,
    #[arg(long)]
    pub datasources: Option<PathBuf>,
    #[arg(long="export",value_parser=["csv","xlsx","print-xlsx"])]
    pub exports: Vec<String>,
}
#[derive(Args)]
pub struct Library {
    #[arg(long)]
    pub library: PathBuf,
}
#[derive(Subcommand)]
pub enum Command {
    Doctor {
        /// 检查目录库是否可读可写
        #[arg(long)]
        library: Option<PathBuf>,
        /// 只做 JSON 形状检查，不建立数据库连接
        #[arg(long)]
        datasources: Option<PathBuf>,
    },
    Schema {
        #[arg(value_parser=["template"])]
        kind: String,
    },
    Template {
        #[command(subcommand)]
        command: Template,
    },
    #[cfg(feature = "legacy-rpx")]
    Format {
        #[command(subcommand)]
        command: FormatCommand,
    },
    Run {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
        #[command(flatten)]
        inputs: Inputs,
    },
    Result {
        #[command(subcommand)]
        command: ResultCommand,
    },
    Dataset {
        #[command(subcommand)]
        command: Dataset,
    },
    Catalog {
        #[command(subcommand)]
        command: Catalog,
    },
    Library {
        #[command(subcommand)]
        command: LibraryCommand,
    },
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
    Inspect {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        #[arg(long)]
        cell: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    Validate {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        #[arg(long)]
        params_file: Option<PathBuf>,
    },
    Create {
        #[arg(long)]
        example: String,
        #[arg(long)]
        out_dir: PathBuf,
    },
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
    Compile {
        #[arg(value_parser=source_path)]
        file: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}
#[derive(Subcommand)]
pub enum ResultCommand {
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
    Functions {
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    Examples,
}
#[derive(Subcommand)]
pub enum LibraryCommand {
    Init {
        dir: PathBuf,
        #[arg(long)]
        presets: bool,
    },
}
#[derive(Subcommand)]
pub enum Report {
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
    Show {
        id: String,
        #[command(flatten)]
        library: Library,
    },
    Runs {
        id: String,
        #[command(flatten)]
        library: Library,
        #[command(flatten)]
        page: Page,
    },
    Copy {
        id: String,
        #[arg(long = "as")]
        new_id: String,
        #[command(flatten)]
        library: Library,
    },
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
    Validate {
        id: String,
        #[command(flatten)]
        library: Library,
    },
    Run {
        id: String,
        #[arg(long)]
        if_match: String,
        #[command(flatten)]
        library: Library,
        #[command(flatten)]
        inputs: Inputs,
    },
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
    Delete {
        id: String,
        #[arg(long)]
        if_match: String,
        #[command(flatten)]
        library: Library,
    },
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
                Report::Prune { .. } => "report.prune",
            },
        }
    }
}
