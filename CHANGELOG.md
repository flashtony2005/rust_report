# Changelog

本项目遵循“迁移检查点”式版本记录；不保证 SemVer 兼容性承诺之外的二进制格式稳定。

## v0.10.0-json-cell-settings — 2026-10-17

### Added

- `cells[].hyperlink` / `hyperlink_window` / `notes`（超链接、打开目标、备注）。
- JSON → model → `.rpx` → JSON 往返覆盖新增字段。
- 空超链接显式拒绝；schema/文档同步。

## v0.9.0-json-settings — 2026-10-17

### Added

- JSON-only 决策 ADR：`docs/adr/0002-json-canonical-rpx-frozen.md`。
- `print_setup.paper`（A4/A3/A5/Letter）与 `print_setup.orientation`（portrait/landscape）作者糖。
- paper/orientation 映射到宽高，不写裸 `orient` 码；与显式宽高冲突时显式报错。
- schema 与文档同步；新增 parser 回归测试。

## v0.8.0-wp1-wp2 — 2026-10-17

### Added

- `rpx report migrate --library DIR [--dry-run] [--backup]`：库内 v0 JSON → v1，备份可回滚。
- `scripts/legacy_tests.sh` 与独立 CI `legacy` job；默认 gate 只跑 JSON-only 路径。

### Changed

- `report migrate` 不依赖 `legacy-rpx`，默认构建即可使用。
- 默认测试与 legacy 测试完全分离，避免 feature 漂移。

## v0.7.0-v2-defaults — 2026-10-17

### Changed

- `rpx-runtime` 与 `rpx-server` 默认改为 JSON-only；默认构建不再启用 `legacy-rpx`。
- 默认 `cargo tree` 不含 `rpx-core`，也不含 `rpx-model/legacy-rpx`。
- `.rpx` 运行时/集成测试改为显式 feature；gate 额外跑 legacy runtime + CLI 测试。
- 默认 `rpx` 仍是 JSON-only；legacy 安装继续使用 `make install-local-legacy`。

## v0.6.0-json-cli-default — 2026-10-17

### Changed

- `rpx-cli` 默认构建切为 JSON-only：不再暴露 `format` / `template compile` / `report import-rpx`。
- `rpx-model` 默认不编译 `.rpx` reader/writer；`rpx-runtime` 库默认仍开启 legacy 兼容。
- `make install-local` 安装 JSON-only；新增 `make install-local-legacy`。
- CI/gate 额外运行 legacy CLI 测试，保证两条路径都有覆盖。

## v0.5.0-rpx-model — 2026-10-17

### Changed

- 原 `rpx-core` 包重命名为 `rpx-model`（lib 名仍为 `rpx_core`，兼容既有 import）。
- `.rpx` reader/writer 继续由 `legacy-rpx` feature 控制。
- `cargo tree -p rpx-cli --no-default-features` 不再包含 `rpx-core`。
- 引擎 grid/pager/cross 测试改读 `tests/fixtures/format/engine/*.json`；`.rpx` 只留兼容。
- HTTP JSON rejection 测试合并进 server 测试，消除并行启动抖动。

## v0.4.0-format-crate — 2026-10-17

### Changed

- 抽离 `rpx-format`：JSON 定义 parse/build、`definition::strict` 与 template schema 的唯一入口。
- `rpx-runtime` 通过 re-export 保持 `rpx_runtime::reportdef` / `definition` 兼容。
- `rpx-format` 默认不依赖 `.rpx` 二进制；`legacy-rpx` feature 仅用于 `.rpx` 入口。
- G5/G8 登记新 crate；JSON-only 构建、734 测试与 e2e 保持通过。

## v0.3.0-json-migration — 2026-10-17

### Added

- JSON 报表定义 v1：`format` / `format_version` / `schema` / `meta` / 命名空间 `extensions`。
- `rpx format`：`to-json` / `to-rpx` / `verify` / `check` / `normalize`。
- RPX Preserve 迁移快照：40/40 产品 `.rpx` 可转换，未编辑时字节一致回写。
- 字段级 legacy overlay：编辑后的 preserve JSON 可写出，未建模字段从快照合并。
- `report import-rpx` 支持 40/40 产品 `.rpx`。
- `report import-json` / `report delete` / `report prune`。
- `rpx doctor --library/--datasources` 基础环境检查。
- HTTP JSON rejection 统一返回 `error` envelope。
- `--allow-structural`：含快照的结构性编辑 opt-in（只保证重叠区）。

### Changed

- CLI 命令数增至 30；帮助文本补齐子命令说明。
- `report list` 跳过坏包并返回 `diagnostics`。
- 单个导出失败只记 `EXPORT_FAILED` warning，不再拖垮 result/workbook。
- `print-xlsx` 支持 A3/A4/A5/Letter 自定义纸张映射。

### Fixed

- stdout 断管不再 panic。
- `result inspect` 拒绝模板文件和非结果 JSON。
- 运行记录按补齐小数秒排序。
- SQL 空结果集保留列名。
- 默认参数/断管/命令错误路径不再产生 101 退出码。

### Engineering

- `legacy-rpx` Cargo feature：`rpx-core` / `rpx-runtime` / `rpx-cli` 可关闭 `.rpx` 二进制路径。
- JSON-only 构建：`cargo check -p rpx-cli --no-default-features`。
- `scripts/json_only_smoke.sh` 与 CI JSON-only job。
- 23/23 引擎夹具 JSON 门禁；40/40 RPX Preserve 门禁；734 个测试。
- CI：Rust gate G1–G9 + Web build/Vitest + e2e + JSON-only smoke。

### Compatibility

- v1.x 默认仍开启 `legacy-rpx`；默认关闭安排在 v2.0。
- v2.0 计划：`rpx-format` 抽离、测试全面切 JSON、默认无 `.rpx`。
- 迁移指南：`docs/format/rpx-legacy-retirement.md`。

## v0.2.0-baseline — 2026-10-17

- JSON-first 基线检查点。
- 作为后续格式迁移的回滚点。
