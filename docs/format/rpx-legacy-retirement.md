# RPX legacy 退役与 v2.0 默认切换

> 状态：Phase 3 feature 边界已完成（2026-10-17）；默认切换安排在 v2.0。
> 关联：[ADR 0001](../adr/0001-json-canonical-format.md)、[JSON v1 格式](rpx-report-json-v1.md)、[转换器](rpx-core-json-converter.md)。

## 1. 当前默认（v0.7+）

- `rpx-cli` / `rpx-runtime` / `rpx-server` 默认 **JSON-only**。
- `rpx-model` 默认不编译 `.rpx` reader/writer。
- 默认 CLI 不暴露 `format` / `template compile` / `report import-rpx`。
- 需要 legacy 时：

```sh
cargo build -p rpx-cli --features legacy-rpx
cargo test -p rpx-runtime --features legacy-rpx
make install-local-legacy
```

- 默认二进制仍可读 JSON、运行、导出、管理目录库。
- 默认 `cargo tree` 不含 `rpx-core`，也不启用 `legacy-rpx`。

## 2. JSON-only 构建

```sh
cargo build -p rpx-cli --no-default-features
cargo check -p rpx-core --no-default-features
```

no-default 构建中：

- 不暴露 `format` / `template compile` / `report import-rpx`
- runtime 不编译 `format` / `import` / `.rpx` reader 分支
- core 不编译 `reader`/`writer`/`open`/`fill` 等二进制路径
- `ObjValue` 与类型模型仍在，JSON 路径可用

CI 会跑 `scripts/json_only_smoke.sh`：

```text
template validate → run → result inspect → library init --presets → report run
```

## 3. 迁移步骤

```sh
# 1) 验收：40/40 产品夹具可原样迁移
rpx format verify external.rpx --mode preserve

# 2) 迁移：未建模字段进 extensions.rpx.legacy
rpx format to-json external.rpx --out external.json --mode preserve

# 3) 编辑 JSON v1 字段
# 4) 回写：未编辑原样字节；编辑后走字段级 overlay
rpx format to-rpx external.json --out external.rpx --mode preserve

# 可选：把 RPX 直接放入目录库
rpx report import-rpx external.rpx --id external --library ./reports
```

结构性编辑（增删行列）：

```sh
rpx format to-rpx edited.json --out edited.rpx --mode preserve --allow-structural
```

只保证坐标不变的重叠区；中间插入/行列移动需要先移除 `extensions.rpx.legacy`。

## 4. v2.0 切换清单

- `legacy-rpx` 默认关闭；需要时显式安装/启用。
- 默认构建不含 `.rpx` 读写。
- 抽离 `rpx-format`，移除 `rpx-runtime::reportdef` 的重复解析路径。
- 引擎/运行时测试默认读 JSON；`.rpx` 迁到独立 legacy 测试目标。
- 评估 `.rpxpkg` 与 schema v2；只在有迁移工具时做。

## 5. 回滚

不删除 `.rpx` 代码，只切换默认 feature：

```sh
cargo build -p rpx-cli --features legacy-rpx
```

`format` 转换器始终可独立启用，外部资产仍可迁移。
