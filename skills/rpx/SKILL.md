---
name: rpx
description: 用 rpx CLI 管理目录报表库、编辑 JSON 定义、运行 Rust 报表引擎，并检查已保存结果。适用于本项目的报表操作。
---

先执行 `command -v rpx` 和 `rpx doctor`。本地文件模式无需登录或 HTTP 服务。
使用 `rpx schema template` 的 `data.schema` 查询当前字段；函数与预制包分别使用
`rpx catalog functions`、`rpx catalog examples`。默认响应为 JSON，检查 `ok`、`diagnostics` 和进程退出码。

共享库使用显式 `--library DIR`。先 `report list`、`report show ID` 获取源码修订，理解人已保存的修改。
使用 `template inspect PATH --cell C6` 找真实 JSON Pointer，`dataset inspect PATH --name DS` 验证输入。
用 RFC 6902 补丁修改共享定义，携带读取到的 `--if-match`，可先 `--dry-run`。
冲突退出 7 时重新读取并处理人的修改，不能自动重试旧补丁。不要绕过共享写锁直接覆盖库内模板。

定义检查不取业务数据。没有实际参数时，参数绑定和部分推导检查会列在 `deferred`；
模板参数的字符串 default 是声明信息，不自动成为参数值。实际参数使用 JSON 对象文件，支持 `--params-file -`。
SQL 连接只通过显式 `--datasources FILE` 提供；不要写进模板。

运行前重新取得修订，用 `report run --if-match REV` 调用引擎。结果保存为固定运行目录，
`result inspect --origin C6` 查询模板出处，`--range A1:K20` 查询结果坐标，二者是不同坐标系。
大整数、decimal 的结果值为字符串，列表递归保留类型。Web 只保存定义和读取固定产物；保存不会执行引擎。
需要新导出时在 run 中指定 `--export csv|xlsx|print-xlsx`。失败记录保留，旧成功结果仍可查看。
RPX 通过显式 `report import-rpx` 分析和导入；不支持的属性必须修正或报告，不能丢字段后伪称无损。
迁移前先 `rpx format verify FILE.rpx --mode preserve`；`format to-json` 会把原始 RPX 字节放进
`extensions.rpx.legacy`。该快照是迁移用的，不是编辑面：编辑可建模字段后 `format to-rpx`
会报 `LEGACY_READ_ONLY`，必须先移除 legacy 扩展再导出。

三份可复制的例子（目录必须尚不存在）：

```sh
rpx template create --example grouped-sales --out-dir /tmp/rpx-grouped
rpx run /tmp/rpx-grouped/template.json --out-dir /tmp/rpx-grouped-run --export xlsx
```

```sh
rpx template create --example complex-master --out-dir /tmp/rpx-complex
rpx run /tmp/rpx-complex/template.json --out-dir /tmp/rpx-complex-run --export csv
rpx result inspect /tmp/rpx-complex-run/artifacts/result.json --origin G9
```

```sh
rpx library init /tmp/rpx-reports --presets
rpx report show complex-master --library /tmp/rpx-reports
rpx_revision=$(rpx report show complex-master --library /tmp/rpx-reports | jq -r '.data.revision')
rpx report run complex-master --library /tmp/rpx-reports --if-match "$rpx_revision" --export xlsx
```
