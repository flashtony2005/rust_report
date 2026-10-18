# Rust Report

Agent 用 `rpx` 管理 JSON 报表定义、数据源、主格关系并执行引擎；人通过 Univer Web 修正定义、查看固定运行结果。

```sh
make install-local            # JSON-only（默认）
# make install-local-legacy  # 需要 format/compile/import-rpx 时
export PATH="$HOME/.local/bin:$PATH"
rpx doctor
rpx library init ./reports --presets
rpx report show complex-master --library ./reports
revision=$(rpx report show complex-master --library ./reports | jq -r '.data.revision')
rpx report run complex-master --library ./reports --if-match "$revision" --export xlsx --export csv --export pdf
# pdf/html 复用同一份打印快照；pdf 需要本机 Chrome/Chromium/Edge（或用 RPX_CHROME 指定），
# 找不到浏览器就明确失败，不会静默降级。图表不参与渲染，会在原位留占位框并记 EXPORT_PARTIAL。
rpx format verify external.rpx --mode preserve
cargo run --locked -p rpx-server -- serve --library ./reports
# 启用 Bearer 鉴权时：
# cargo run --locked -p rpx-server -- serve --library ./reports --auth-token "$TOKEN"
npm run dev --prefix web
```

Web 开发服务默认 http://localhost:5288，API 默认 127.0.0.1:8090。
保存 JSON 只更新源码修订；结果、打印预览和下载都读取 agent 已生成的产物。
源码变化后旧结果会保留，并标明所属修订。已有 RPX 使用显式 `format`/导入命令管理；v1 格式与迁移快照见
[docs/format/rpx-report-json-v1.md](docs/format/rpx-report-json-v1.md)；legacy 退役计划见 [docs/format/rpx-legacy-retirement.md](docs/format/rpx-legacy-retirement.md)，v2.0 工作安排见 [docs/v2.0-work-plan.md](docs/v2.0-work-plan.md)；JSON-only / RPX frozen 决策见 [docs/adr/0002-json-canonical-rpx-frozen.md](docs/adr/0002-json-canonical-rpx-frozen.md)，设定覆盖见 [docs/format/json-settings-coverage.md](docs/format/json-settings-coverage.md)。

[CLI 使用与错误协议](docs/agent-cli.md) · [Web 鉴权](docs/web-auth.md) · [设计](docs/agent-cli-design.md) · [Agent 配套技能](skills/rpx/SKILL.md)

要求 Rust 1.89+，安装的依赖版本以 Cargo.lock 为准。`make check` 运行全部项目检查。
