# Agent CLI 使用

2026-10-17。安装后的命令名是 `rpx`。JSON 是唯一编辑源码；引擎由 agent 调用。

## 安装与发现

```sh
make install-local
export PATH="$HOME/.local/bin:$PATH"
command -v rpx
rpx --help
rpx doctor
rpx schema template
rpx catalog functions --name sum
rpx catalog examples
```

无需服务或登录。Rust 1.89+；`clap` 负责命令解析，`json-patch` 负责 RFC 6902，文件锁使用标准库。
配套技能源码在 `skills/rpx/SKILL.md`，可安装到 `~/.codex/skills/rpx/SKILL.md`。

## 文件操作

```sh
rpx template create --example complex-master --out-dir /tmp/my-report
rpx template inspect /tmp/my-report/template.json --cell C6
rpx template validate /tmp/my-report/template.json
rpx dataset inspect /tmp/my-report/template.json --name ds1 --limit 5
rpx run /tmp/my-report/template.json --out-dir /tmp/my-run --export xlsx --export csv
rpx result inspect /tmp/my-run/artifacts/result.json --origin G9
rpx result inspect /tmp/my-run/artifacts/result.json --range A1:K20 --limit 50 --offset 0
rpx template patch /tmp/my-report/template.json --patch-file changes.json --out /tmp/changed.json --dry-run
rpx template compile /tmp/my-report/template.json --out /tmp/report.rpx
```

目标文件/运行目录必须尚不存在。路径相对当前目录，默认数据目录是模板所在目录；响应路径为绝对路径。
`--data-dir DIR` 覆盖文件目录，`--datasources connections.json` 显式提供 SQL 连接别名。
Excel 支持 `sheet_name`；CSV/TSV 使用定义中的 delimiter、encoding；多个文件与 SQL 数据集可同时使用。

`--params-file FILE` 传实际 JSON 对象，`--params-file -` 读 stdin；补丁同样支持 `--patch-file -`。
每次命令最多一个 stdin 输入。数值/布尔/字符串/null/列表保留类型，参数值对象和 i64 溢出显式失败。
模板网格（含内部首行首列）最多 1,000,000 槽，数值属性必须在有限精度范围内。
参数元数据中的 default 不自动绑定实际值。不传实际参数时，校验返回已检查与 deferred 项；不取文件或数据库。

`template inspect --cell` 使用模板地址；`result inspect --range` 使用计算结果地址；`--origin` 使用模板出处。
主格未声明时返回 inferred_at_run；显式 `left_head`/`top_head` 原样保留，`` `0 `` 表示显式根。

## 格式迁移（RPX legacy）

JSON v1 是日常格式；`.rpx` 只通过显式 legacy 命令进出：

```sh
# 先验收：读 → JSON → 模型/字节 → 再读
rpx format verify external.rpx --mode preserve

# 迁移：保留原始 RPX 字节快照（extensions.rpx.legacy）
rpx format to-json external.rpx --out external.json --mode preserve

# 未编辑快照时原样回写；编辑了可建模字段会报 LEGACY_READ_ONLY
rpx format to-rpx external.json --out external.rpx --mode preserve

# 只检查 JSON 能否被当前 reader 接受
rpx format check external.json

# v0/v1 → v1：补 format / format_version / schema
rpx format normalize template.json --out template-v1.json
```

- `preserve` 是迁移默认；`strict` 遇到 v1 尚不能表达的字段会显式失败；`drop` 只用于渲染。
- `template create` / `library init --presets` 现在写出带 v1 标记的 JSON；旧 v0 文件仍可读。
- `extensions.rpx.legacy` 是迁移快照，不是长期编辑面。编辑后要导出，先移除该扩展（接受未建模字段不可表达）或等待 Phase 2 字段级覆盖。
- `template compile` 与 `report import-rpx` 保留，但属于 legacy 入口；日常用 `format` + 原生 JSON。

## 目录模板库

```sh
rpx library init ./reports --presets
rpx report list --library ./reports --tag 经营分析
rpx report copy complex-master --as monthly --library ./reports
rpx report show monthly --library ./reports
revision=$(rpx report show monthly --library ./reports | jq -r '.data.revision')
rpx report patch monthly --library ./reports --if-match "$revision" --patch-file changes.json --dry-run
rpx report patch monthly --library ./reports --if-match "$revision" --patch-file changes.json
revision=$(rpx report show monthly --library ./reports | jq -r '.data.revision')
rpx report validate monthly --library ./reports
rpx report run monthly --library ./reports --if-match "$revision" --export xlsx
rpx report runs monthly --library ./reports --limit 5
```

清单字段：version、id、name、description、tags、template、data_dir、可选 params_file。
包内路径相对该报表目录。源码修订覆盖清单、模板和默认参数的路径与字节，使用 SHA-256。
实际数据和命令行覆盖参数单独记录；同修订不表示数据仍是最新的。
用 `report update ID --library DIR --if-match REV --name NAME --description TEXT` 修改元信息。

CLI 与 Web 在同一个跨进程文件锁内核对修订。冲突返回 REVISION_CONFLICT，不自动重试。
初始化、复制、导入经暂存目录发布；补丁在副本上校验后原子替换。
运行在锁内捕获源码并登记，然后释放锁再取数；运行中修改定义不会改变该次运行的源码。
默认参数清单文件可由本次 `--params-file` 完整替换，不合并。

预制包：grouped-sales、complex-master、multi-column、chart-gallery、images。
所有模板与数据随二进制发布，从 /tmp 或复制后的库运行也不依赖仓库。

## RPX 交付与导入

```sh
rpx report import-rpx external.rpx --id external --library ./reports --dry-run
rpx report import-rpx external.rpx --id external --library ./reports --data-dir ./data
```

先解析 RPX，映射公开语义字段，再用 JSON 重新构建并逐字段比较。
已覆盖的字面量、公式、显式主格、合并、样式、行列属性、参数、文件/SQL 数据集和打印配置保留。
未知宏记录、子报表、私有图形记录、非默认未映射属性返回 RPX_IMPORT_UNSUPPORTED 和具体位置。
不会把二进制塞进 JSON，也不会丢字段后报告成功；源 RPX 保持原样。
日期字面量可保留定义，现有引擎尚不能计算日期对象，执行校验会明确拒绝；日期字符串可供 year/month 使用。
缺少数据文件时导入分析列出依赖，不能把包视作已具备所有运行输入。

新增的 `literal: {type, value}` 与 text/expr 互斥，支持 null、text、bool、byte、short、int、long、float、double、decimal、date。
long/decimal 用字符串保存精度。disp_value 是独立显示覆盖，不改变原始值。
行列尺寸或布尔属性的 JSON 数值/布尔写入静态属性，字符串保留为属性表达式。

## 运行记录与 Web

每次运行写入 run.json、source/、inputs/params.json。成功后发布 artifacts/result.json、workbook.json 和明确请求的导出。
run.json 记录 UTC 时间、引擎版本、源码修订、实际参数、输入文件 SHA-256、SQL 别名与查询摘要、诊断及产物索引。
SQL 不保存连接凭据，也不声称数据库状态可完全重现。
快照/导出失败时整体为 failed，暂存产物不发布；保留失败记录与上一次成功结果。

结果值为 `{type,value}`；Long/Decimal 的 value 为字符串，List 递归保留类型。
null、零、空文本、显示覆盖和合并槽单独表达。主格出处是模板地址。
列表默认 50、最大 500，响应含 returned、total、next_offset；空结果仍成功。

```sh
cargo run --locked -p rpx-server -- serve --library ./reports --addr 127.0.0.1:8090
npm run dev --prefix web
```

Web 读取/修正同一 JSON，PUT 必须带 If-Match；HTTP 412 时保留草稿。
源码框可以修正所有公开字段；Univer 与侧栏用于网格、公式、数据集和主格编辑。
运行历史显示修订、参数、状态和诊断，成功的固定工作簿用于结果与打印预览。
优先展示当前修订最近启动的成功运行，否则展示历史成功运行；latest_attempt 独立标明最新失败。
GET /reports、GET /reports/{id}、PUT /reports/{id}/template、GET /reports/{id}/runs、
GET /reports/{id}/runs/{run_id}、GET /reports/{id}/runs/{run_id}/source/{path}、
GET /reports/{id}/runs/{run_id}/artifacts/{name} 是服务端接口。运行详情可查看当时源码。
原 /render、/export 和 BLOB 入库路径已移除。Web 保存、查看、刷新和下载均不调用引擎。

## 输出与退出码

默认单行 JSON：protocol_version、ok、command、data、diagnostics、error、meta.duration_ms。
使用 `--output text` 查看文本，帮助与版本输出文本。参数解析错误也输出 JSON。

| 退出码 | 含义 |
| --- | --- |
| 0 | 成功，包括空查询 |
| 1 | 内部错误 |
| 2 | 命令参数错误 |
| 3 | 定义、主格或公式准备错误 |
| 4 | 数据、参数或 SQL 错误 |
| 5 | 计算或分页错误 |
| 6 | 文件或导出错误 |
| 7 | 源码修订冲突 |

`make check` 执行完整门禁；CLI 测试使用真实二进制进程，HTTP 测试使用真实 socket。
