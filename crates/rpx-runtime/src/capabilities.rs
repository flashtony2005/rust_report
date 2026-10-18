//! 能力自描述：一个 Agent 在**不读文档**的前提下需要的全部事实。
//!
//! 动机（真实故障）：文档写的是 `0.13.0` 的用法，机器上装的是 `0.1.0`，
//! 于是 `rpx doctor --library` 直接 `USAGE`。Agent 最信任文档，
//! 所以版本、命令面、导出格式、错误码必须能**从二进制自己问出来**。
use serde_json::{json, Value as J};

/// 退出码语义：所有子命令共用一套，调用方可以只看数字决定是否重试。
fn exit_classes() -> J {
    json!({
        "0": "成功（可能有 warning 诊断）",
        "2": "用法/参数错误：命令或参数写错，重试同样参数无用",
        "3": "定义错误：模板 JSON 不合法、表达式语法错、结构冲突",
        "4": "输入/数据错误：文件缺失、列不存在、数据源连不上、参数没绑定",
        "5": "计算错误：表达式求值失败（多为作用域/列引用问题）",
        "6": "IO/导出错误：写盘失败、产物生成失败",
    })
}

/// 错误码目录：`code` 是稳定契约，`stage` 告诉调用方该去哪一层查。
fn error_catalog() -> J {
    json!([
        {"code":"USAGE","exit":2,"stage":"arguments","meaning":"命令或参数写错"},
        {"code":"TEMPLATE_INVALID","exit":3,"stage":"definition","meaning":"模板不符合 JSON v1 schema","hint":true},
        {"code":"JSON_INVALID","exit":3,"stage":"definition","meaning":"文件不是合法 JSON"},
        {"code":"CELL_DUPLICATE","exit":3,"stage":"definition","meaning":"同一 row/col 有多个 cell"},
        {"code":"FORMULA_INVALID","exit":3,"stage":"prepare","meaning":"表达式语法错或引用了未登记参数"},
        {"code":"PREPARE_FAILED","exit":3,"stage":"prepare","meaning":"主格/扩展关系推导失败"},
        {"code":"LITERAL_UNSUPPORTED","exit":3,"stage":"prepare","meaning":"字面量类型引擎不支持"},
        {"code":"PATCH_INVALID","exit":3,"stage":"patch","meaning":"RFC 6902 补丁无法应用"},
        {"code":"PARAM_INVALID","exit":4,"stage":"parameters","meaning":"参数文件形状或类型不对"},
        {"code":"PARAM_OVERFLOW","exit":4,"stage":"parameters","meaning":"参数数量/长度超限"},
        {"code":"DATA_MISSING","exit":4,"stage":"data","meaning":"文件数据集找不到","hint":true},
        {"code":"DATA_FETCH","exit":4,"stage":"data","meaning":"取数失败：解析、SQL 连接或结果集问题","hint":true},
        {"code":"DATASET_UNKNOWN","exit":4,"stage":"data","meaning":"模板里没有该数据集名"},
        {"code":"DATASOURCE_CONFIG","exit":4,"stage":"data","meaning":"数据源注册表文件无法加载","hint":true},
        {"code":"DATASOURCES_INVALID","exit":4,"stage":"doctor","meaning":"数据源配置形状不对"},
        {"code":"SQL_MACRO","exit":4,"stage":"data","meaning":"SQL 文本里的宏无法替换"},
        {"code":"CALCULATION_FAILED","exit":5,"stage":"calculate","meaning":"表达式求值失败，常见于列名/作用域写错","hint":true},
        {"code":"SNAPSHOT_FAILED","exit":6,"stage":"snapshot","meaning":"计算结果无法映射为快照"},
        {"code":"IO_ERROR","exit":6,"stage":"io","meaning":"读写文件失败"},
        {"code":"INTERNAL","exit":6,"stage":"internal","meaning":"内部不变式被违反，属 bug"},
        {"code":"EXPORT_FAILED","exit":0,"stage":"export","meaning":"单个导出失败；run 仍成功，诊断里带 format","hint":true},
        {"code":"RPX_COMPILE","exit":6,"stage":"export","meaning":"JSON 定义无法编译为 legacy .rpx"},
        {"code":"RPX_INVALID","exit":3,"stage":"definition","meaning":"legacy .rpx 字节不合法"},
        {"code":"RPX_IMPORT_UNSUPPORTED","exit":3,"stage":"definition","meaning":"legacy 资产含不支持的构造"},
        {"code":"EXAMPLE_UNKNOWN","exit":2,"stage":"arguments","meaning":"内置示例名不存在"},
        {"code":"REVISION_REQUIRED","exit":4,"stage":"library","meaning":"库内写操作必须带 --if-match"},
        {"code":"NOT_FOUND","exit":4,"stage":"library","meaning":"库内报表/运行记录不存在"},
        {"code":"ID_EXISTS","exit":4,"stage":"library","meaning":"报表 id 已存在"},
        {"code":"MANIFEST_INVALID","exit":4,"stage":"library","meaning":"库清单文件损坏或版本不支持"},
        {"code":"RESULT_INVALID","exit":3,"stage":"result","meaning":"result.json 形状不对"},
        {"code":"ARTIFACT_MISSING","exit":4,"stage":"library","meaning":"运行记录缺产物文件"},
    ])
}

/// 命令面：名字 + 一句话 + 是否离线。与 `args.rs` 的枚举一一对应，
/// 有测试保证不漂移（`capabilities_matches_cli_surface`）。
fn commands() -> J {
    json!([
        {"name":"doctor","summary":"环境与能力自检","offline":true},
        {"name":"capabilities","summary":"自描述：命令/导出/限制/错误码","offline":true},
        {"name":"schema.template","summary":"输出模板 JSON schema","offline":true},
        {"name":"template.inspect","summary":"查看模板摘要或单个单元格","offline":true},
        {"name":"template.validate","summary":"离线校验；--probe-data 可提前暴露数据问题","offline":true},
        {"name":"template.create","summary":"从内置示例创建模板包","offline":true},
        {"name":"template.patch","summary":"应用 RFC 6902 补丁","offline":true},
        {"name":"lint","summary":"纯静态检查：四类静默失败","offline":true},
        {"name":"run","summary":"执行模板并导出；返回 layout 摘要","offline":true},
        {"name":"result.inspect","summary":"分页/按区域查看 result.json","offline":true},
        {"name":"dataset.inspect","summary":"预览文件/SQL 数据集（看真实列名）","offline":true},
        {"name":"catalog.functions","summary":"列出内置函数与限制","offline":true},
        {"name":"catalog.examples","summary":"列出内置示例","offline":true},
        {"name":"library.init","summary":"初始化目录库","offline":true},
        {"name":"report.list","summary":"列出库内报表","offline":true},
        {"name":"report.show","summary":"查看报表源码与状态","offline":true},
        {"name":"report.runs","summary":"查看运行历史","offline":true},
        {"name":"report.copy","summary":"复制报表","offline":true},
        {"name":"report.patch","summary":"库内应用补丁","offline":true},
        {"name":"report.validate","summary":"库内离线校验","offline":true},
        {"name":"report.run","summary":"运行库内报表","offline":true},
        {"name":"report.update","summary":"更新名称/描述","offline":true},
        {"name":"report.import-json","summary":"从 JSON 定义入库","offline":true},
        {"name":"report.delete","summary":"删除报表（必须带修订）","offline":true},
        {"name":"report.migrate","summary":"迁移库内 v0 → v1","offline":true},
        {"name":"report.prune","summary":"清理旧运行记录","offline":true},
    ])
}

/// `version` 由 CLI 传入（`env!("CARGO_PKG_VERSION")` 在 crate 内会被编成
/// **runtime** 的版本；调用方想知道的是"我手上这个二进制是什么版本"，两者会漂移）。
pub fn describe(version: &str) -> J {
    json!({
        "version": version,
        "protocol_version": 1,
        "definition": "JSON v1",
        "json_format": "rpx-report/v1",
        "offline": true,
        "authentication": "not_required",
        "server_side": false,
        "commands": commands(),
        "exports": ["csv", "xlsx", "print-xlsx"],
        "datasources": ["CSV", "TSV", "Excel", "SQLite", "PostgreSQL", "MySQL"],
        "features": {
            "legacy_rpx": cfg!(feature = "legacy-rpx"),
            "pdf": false,
            "lint": true,
            "probe_data": true,
            "run_layout": true,
            "error_hints": true,
            "web_executes_engine": false,
        },
        "limits": {
            "page_limit_max": 500,
            "sql_result_rows_default": 100_000,
            "sql_result_rows_env": "RPX_SQL_MAX_ROWS",
            "excel_max_rows": 1_048_576,
            "excel_max_cols": 16_384,
        },
        "conventions": {
            "coordinates": "1 起，A1 风格",
            "extension": "同排多 vertical 必须用 left_head 串父子链",
            "macros": "宏只在 expr 中替换，写进 text 会原样上屏",
            "border": "模板可声明，但当前导出不落地",
        },
        "exit_codes": exit_classes(),
        "errors": error_catalog(),
    })
}
