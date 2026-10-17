---
name: rpx
description: Operate the rpx CLI for JSON report definitions, local report libraries, runs, results, datasets, migrations and legacy RPX imports. Use whenever the user asks to create, inspect, validate, patch, run, query, copy, import, delete, prune or migrate an rpx report; mentions template.json, result.json, report library, report run, revision/If-Match, or asks to move an existing RPX asset into the JSON library.
---

# rpx CLI 操作

`rpx` 是本地 CLI，管理 JSON 报表定义、目录库、固定运行结果和导出。默认构建是 **JSON-only**，不需要服务或登录。

## 先确认环境

```sh
command -v rpx
rpx --help
rpx doctor
```

- 默认响应是单行 JSON；脚本读取 `data`，用 `--output text` 才看人类可读文本。
- 先看 `ok`、`error.code`、`diagnostics` 和进程退出码，不要只解析 stdout 的文本。
- `rpx doctor` 的 `data.checks` 能检查 `--library` 可写、`--datasources` 配置形状。
- 默认 CLI **没有** `format`、`template compile`、`report import-rpx`；这些只在 legacy 构建出现。不要假设它们存在。

## 命令地图

| 目的 | 命令 |
|---|---|
| 查 schema | `rpx schema template` |
| 查函数/示例 | `rpx catalog functions --name sum`、`rpx catalog examples` |
| 创建模板 | `rpx template create --example ID --out-dir DIR` |
| 查看/定位单元格 | `rpx template inspect FILE --cell C6` |
| 校验模板 | `rpx template validate FILE [--params-file P]` |
| 补丁 | `rpx template patch FILE --patch-file P --out FILE [--dry-run]` |
| 直接运行 | `rpx run FILE --out-dir DIR [--export csv|xlsx|print-xlsx]` |
| 查结果 | `rpx result inspect result.json [--origin C6] [--range A1:K20]` |
| 查数据集 | `rpx dataset inspect FILE --name DS [--data-dir D] [--datasources F]` |
| 初始化库 | `rpx library init DIR [--presets]` |
| 列/看报表 | `rpx report list --library DIR`、`rpx report show ID --library DIR` |
| 运行历史 | `rpx report runs ID --library DIR --limit 5` |
| 复制/改名 | `rpx report copy ID --as NEW --library DIR`、`rpx report update ID --library DIR --if-match REV --name NAME --description TEXT` |
| 库内补丁 | `rpx report patch ID --if-match REV --patch-file P --library DIR [--dry-run]` |
| 库内校验/运行 | `rpx report validate ID --library DIR`、`rpx report run ID --if-match REV --library DIR` |
| JSON 入库 | `rpx report import-json FILE --id ID --library DIR` |
| 迁移库 | `rpx report migrate --library DIR [--dry-run] [--backup]` |
| 删除/清理 | `rpx report delete ID --if-match REV --library DIR`、`rpx report prune ID --keep N --library DIR` |

目录库命令都显式带 `--library DIR`；文件命令默认相对当前目录，数据目录默认模板所在目录。

## 输出与退出码

JSON 响应：

```json
{
  "protocol_version": 1,
  "ok": true,
  "command": "template.validate",
  "data": {},
  "diagnostics": [],
  "error": null,
  "meta": { "duration_ms": 3 }
}
```

| 退出码 | 含义 |
|---|---|
| 0 | 成功，包括空查询 |
| 1 | 内部错误 |
| 2 | 参数/用法错误 |
| 3 | 定义、模板、主格或公式准备错误 |
| 4 | 数据、参数或 SQL 错误 |
| 5 | 计算或分页错误 |
| 6 | 文件/导出错误 |
| 7 | 源码修订冲突 |

失败时优先展示 `error.code`、`error.message` 和 `error.path`（若有），不要只说“命令失败”。

## 工作流 1：独立文件

```sh
rpx template create --example complex-master --out-dir /tmp/rpx-complex
rpx template inspect /tmp/rpx-complex/template.json --cell C6
rpx template validate /tmp/rpx-complex/template.json
rpx run /tmp/rpx-complex/template.json \
  --out-dir /tmp/rpx-complex-run --export xlsx
rpx result inspect /tmp/rpx-complex-run/artifacts/result.json --origin C6
```

注意：

- `--out-dir` 必须尚不存在；不要复用旧输出目录。
- `template inspect --cell` 用模板地址；`result inspect --origin` 也按模板出处；`--range` 才是结果坐标。
- 结果值是 `{type,value}`；long/decimal 的 `value` 是字符串，不要强制转 number。
- 空结果仍是成功；分页看 `returned/total/next_offset`。

## 工作流 2：共享目录库

先读，再改；修改必须带当前 revision：

```sh
rpx library init ./reports --presets
rpx report list --library ./reports --limit 20
rpx report show complex-master --library ./reports
rev=$(rpx report show complex-master --library ./reports | jq -r '.data.revision')

# 先 dry-run
rpx report patch complex-master --library ./reports \
  --if-match "$rev" --patch-file changes.json --dry-run

# 再写
rpx report patch complex-master --library ./reports \
  --if-match "$rev" --patch-file changes.json

# 重新取 revision 后运行
rev=$(rpx report show complex-master --library ./reports | jq -r '.data.revision')
rpx report validate complex-master --library ./reports
rpx report run complex-master --library ./reports \
  --if-match "$rev" --export xlsx
```

规则：

- **不要直接覆盖**库内 `template.json`/`manifest.json`；CLI 有跨进程文件锁和暂存发布。
- 修改/运行/删除前重新取 revision；旧 revision 会返回 7（`REVISION_CONFLICT`）。
- 冲突时重新读取并重做修改，不要自动重试旧补丁。
- `report delete` 必须带 `--if-match`；`report prune` 只保留最近 N 次运行。
- `report list` 遇到坏包会跳过并返回 `diagnostics`，不要因此认为整个库损坏。
- `report import-json FILE --id ID --library DIR` 可直接创建报表；会补 v1 标记。
  有文件数据集时可加 `--data-dir DIR` 复制依赖。

## 工作流 3：参数、数据集与 SQL

```sh
rpx template validate FILE --params-file params.json
rpx run FILE --out-dir DIR --params-file - < params.json
rpx dataset inspect FILE --name ds1 --data-dir ./data --limit 5
rpx run FILE --out-dir DIR --datasources connections.json
```

- `--params-file -` / `--patch-file -` 支持 stdin；每次命令最多一个 stdin 输入。
- 参数文件是 JSON 对象；模板里的 `default` 只是声明，不会自动成为实际参数值。
- 没有真实参数时，校验的 `deferred` 会列出未执行项，这是正常的。
- SQL 连接只通过 `--datasources FILE` 提供，**不要写进报表 JSON**。
- SQL 默认最多物化 100,000 行，超限报 `TooManyRows`；`RPX_SQL_MAX_ROWS` 可调。
- CSV/TSV 的 delimiter/encoding、Excel 的 `sheet_name` 由数据集定义控制。

## 工作流 4：迁移 JSON 库

```sh
rpx report migrate --library ./reports --dry-run
rpx report migrate --library ./reports --backup
```

- 把库内 v0 JSON 模板规范化为 v1。
- `--dry-run` 只报告 `changed/unchanged/errors`，不写文件。
- `--backup` 在首次写入前复制 `template.json` 和 `manifest.json`，路径在 `data.backup`。
- 迁移失败不修改该报表；处理 `errors` 后重跑。
- 迁移会改变源码 revision；旧运行记录变为 `older_revision`，这是预期行为。

## 工作流 5：legacy RPX（仅 legacy 构建）

只有在 `rpx --help` 能看到 `format`/`compile`/`import-rpx` 时使用：

```sh
rpx format verify external.rpx --mode preserve
rpx format to-json external.rpx --out external.json --mode preserve
rpx report import-rpx external.rpx --id external --library ./reports --dry-run
rpx report import-rpx external.rpx --id external --library ./reports --data-dir ./data
```

- `preserve` 把原始 RPX 字节放进 `extensions.rpx.legacy`；未编辑时 `to-rpx` 原样回写。
- 编辑可建模字段后 `to-rpx` 用 overlay 合并未建模字段；语义保留，不保证字节相等。
- 增删行列默认报 `LEGACY_STRUCTURAL_EDIT`；`--allow-structural` 只保证重叠区。
- 不要声称 RPX 无损，除非 `format verify` 通过。
- RPX 已冻结：只做迁移/兼容，不做新功能。

## 导出与已知边界

- `--export csv|xlsx|print-xlsx` 可重复。
- `print-xlsx` 支持 A3/A4/A5/Letter 自定义纸张；3D 图表、工作表背景会返回 `EXPORT_FAILED`。
- 单个导出失败只记 warning，`result.json`/`workbook.json` 和成功导出仍发布。
- CLI **不生成 PDF**；需要 PDF 从 Web 打印预览另存。
- 快照失败时整次 run failed；保留上一次成功结果。

## 安全操作清单

- 先 `doctor`，再 `library init`；不要对已有目录初始化。
- 改共享库前 `report show` 取 revision；补丁先 `--dry-run`。
- 运行前重新取 revision；不重用旧 `--if-match`。
- 不手改 `.library.json`、`.runs` 或包内模板。
- 不把 SQL 凭据写进模板；用 `--datasources`。
- 迁移/导入先 dry-run，涉及写库时加 `--backup`。
- 解析 JSON 输出用 `jq`，不要依赖人类文本。

## 失败时怎么查

```sh
rpx doctor --library ./reports
rpx report show ID --library ./reports
rpx report runs ID --library ./reports --limit 5
rpx template inspect FILE --cell C6
rpx dataset inspect FILE --name DS --data-dir ./data
```

- `3`：定义/公式/主格问题；看 `error.path`，用 `template validate`/`inspect`。
- `4`：参数/数据/SQL 问题；查 `--params-file`、数据目录、`--datasources`、`TooManyRows`。
- `5`：计算/分页问题；看 run 诊断和打印设置。
- `6`：文件/导出问题；确认输出目录不存在、源文件存在、导出限制。
- `7`：revision 冲突；重新 `report show` 取 revision，再重做修改/运行。
