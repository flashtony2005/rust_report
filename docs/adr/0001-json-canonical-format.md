# ADR 0001 — JSON 成为规范报表格式，`.rpx` 降级为 legacy 适配器

- 状态：**Accepted — Phase 0/1/2 完成；Phase 3 进行中（runtime/CLI + rpx-core feature 边界已落地）**
- 日期：2026-10-17
- 决策范围：报表定义格式、`rpx-core`/`rpx-runtime` 边界、CLI/Web/Server 的输入输出
- 关联文档：
  - 格式：`docs/format/rpx-report-json-v1.md`
  - 转换器：`docs/format/rpx-core-json-converter.md`
  - 现状审计：`docs/upgrade-readiness-review-2026-10-17.md`

---

## 1. Context

当前项目已经形成事实上的 JSON-first：

- CLI 的 `run` / `template inspect` / `template validate` / `report` 都只接受 JSON；`.rpx` 只通过 `template compile` 与 `report import-rpx` 出现。
- 服务端已移除 `/render`、`/export`、BLOB 入库，只保留目录库 JSON API。
- Web 只接受 `.json`，RPX 明确提示用 CLI 导入。
- 库格式是 `manifest.json` + `template.json` + `data/`。

但仍有四个问题：

1. **JSON 没有正式版本标记**：只有可选的 `version: 1`，无法区分“当前 JSON”“未来 JSON”“产品二进制派生 JSON”。
2. **JSON 是 `ReportDefine` 的投影，不是独立规范**：大量产品字段无法表达，只能靠 import 的严格 compare 拒绝。
3. **`.rpx` 仍是测试与迁移的中心**：23 个引擎夹具输入是 `.rpx`；37 个字节往返、40 个 RPX 夹具是核心证据；`import-rpx` 对 40 个产品夹具 0/40。
4. **当前工作树未提交、无 CI**：任何格式升级都缺少可回滚基线和自动门禁。

问题不是“要不要 JSON”——JSON 已经是日常格式。问题是：

> **要不要把 JSON 正式定为唯一规范格式，并把 `.rpx` 从主路径降级为 legacy 适配器？**

---

## 2. Options

| 选项 | 内容 | 优点 | 缺点 | 结论 |
|---|---|---|---|---|
| A | 维持现状 | 零迁移成本 | JSON 无版本、无兼容策略；RPX 永久拖住主路径 | 否 |
| B | **JSON canonical + RPX legacy adapter** | 保留产品证据；风险可控；可分阶段 | 需要新转换器、schema、测试 | **采纳** |
| C | 立即删除 `.rpx`/`rpx-core` | 架构最干净 | 丢失 37 字节/40 夹具证据；import 0/40 阻断；外部资产不可读 | 否 |
| D | 重写内部模型/引擎为 JSON-native | 理论最彻底 | 21k 行引擎重写；语义/夹具全部重做 | 否 |

---

## 3. Decision

**采纳选项 B。**

具体含义：

1. **JSON v1 成为规范格式**，带 `format` / `format_version` / `schema` / `extensions`。
2. **`ReportDefine` 继续作为内部强类型模型**；不把引擎改成 JSON DOM。
3. **`.rpx` 保留为 legacy 适配器**：
   - `rpx-core` 继续读写 `.rpx`；
   - `template compile` / `report import-rpx` 保留，但标记 legacy；
   - 日常 CLI/Server/Web 路径以 JSON 为准。
4. **迁移默认采用 L1 + L3 无损**：
   - L1：`ReportDefine` 逐字段相等；
   - L3：v1 无法表达的字段进 `extensions["rpx.legacy"]`；
   - L2（字节相等）只对已知 37 个确定性夹具做硬门禁。
5. **不重写引擎**；先做边界、版本、转换、测试，再考虑内部模型演进。

---

## 4. Consequences

### 正面

- JSON 可 diff / merge / JSON Patch / schema 校验，人与 agent 都更好用。
- 可以独立演进自己的格式，不必等待产品 `.rpx` 字段反编译。
- CLI 日常路径不再依赖 `template compile` / `import-rpx`。
- 保留 `rpx-core` 作为产品对照证据，升级风险可控。
- 为后续条件样式、校验、规则层提供稳定的版本化入口。

### 负面

- 需要新增 `rpx-format`（或 runtime 内格式层）与转换器、schema、迁移测试。
- 需要处理 JSON 版本兼容、legacy 冲突、孤儿字段。
- 需要把 23 个引擎夹具的输入逐步 JSON 化，测试资产要迁移。
- 文档需要一次“当前态/历史态”清理。

### 中性

- `.rpx` 不会立刻消失；它会从“主格式”变成“兼容格式”。
- `source_revision` 仍按原始字节哈希；canonical hash 只作为可选的语义哈希，避免已有 If-Match 全部失效。
- `rpx-core` 从“核心”变成“legacy 核心”，代码不删，职责收窄。

---

## 5. 四阶段实施计划

总时长建议：**3–4 个月**（不含外部资产治理）。每一阶段都有硬门禁，未过不得进入下一阶段。

### Phase 0 — 定规范、立基线（1–2 周）

**目标**：让“升级”有可回滚的基线和明确的规范。

**前置**：`docs/upgrade-readiness-review-2026-10-17.md` 的 P0 CLI 边界（`print-xlsx` 不拖垮运行、断管、`result inspect` 校验、运行时间排序）先修或明确排除在格式迁移之外；格式迁移不应掩盖这些真实缺陷。

任务：

1. 把当前工作树整理成 checkpoint commit：`rpx-runtime`、`examples/`、`README.md`、`Makefile`、`skills/`、新 CLI、`Cargo.toml`/`Cargo.lock`；删除/忽略 `rpx.db`；打 `v0.2.0-baseline` tag。
2. 加 CI：G1–G9 + `scripts/e2e_smoke.sh`；固定 `CARGO_TARGET_DIR`、Vitest `cacheDir`，让 gate 在干净环境可复现。
3. 落地格式文档与 schema：`docs/format/rpx-report-json-v1.md` + `rpx-report-v1.schema.json`。
4. reader 先支持 v0/v1：接受无 marker 的 v0；遇到 v1 校验 `format`/`format_version`；writer 暂不改变。
5. 建立 JSON golden corpus（至少 23 个引擎夹具的语义输入）。
6. 文档加“当前架构 / 历史里程碑”索引；给过期规格加横幅。

交付物：

- baseline commit + tag；
- CI 绿灯；
- schema 文件；
- v0/v1 reader 测试；
- golden corpus 雏形。

**门禁 G0**：

- 干净 clone `cargo test --workspace --locked` 全绿；
- CI 可跑；
- v0 文件仍可读；
- v1 最小文档可校验；
- 未修改 writer 的默认输出（避免提前破坏旧 reader）。

**回滚**：本阶段只加 reader 和文档，不改 writer；回滚只需 revert commit。

---

### Phase 1 — 无损桥（3–4 周）

**目标**：让 `.rpx` 能可靠转换成 v1 JSON，且能回写验证。

任务：

1. 实现 `rpx-format`（或先 `rpx_runtime::format`）的：
   - `rpx_to_json` / `model_to_json`；
   - `json_to_model` / `json_to_rpx`；
   - `verify`。
2. 实现三种模式：`Strict` / `PreserveLegacy` / `DropLegacy`。
3. 落地 `extensions["rpx.legacy"]` 的字段覆盖矩阵：
   - cell 未建模字段（diagonal/extend_area/ois_blob/exp_map/hyperlink/...）；
   - row/col 未建模字段；
   - report blobs/scalars/custom properties；
   - dataset 高级槽；
   - print_setup 未开放槽；
   - `ObjValue` 未建模类型。
4. 实现 CLI：`rpx format to-json` / `to-rpx` / `verify` / `check`。
5. 把 40 个 `.rpx` 跑通 Preserve；40/40 快照字节往返；23 个引擎夹具 JSON 化并逐格/逐段对拍。
6. 修 `import-rpx` 的边界或明确标注“只用于 legacy”，不要让用户 import 承担迁移。
7. 建立迁移测试清单（`docs/format/rpx-core-json-converter.md` 的 T01–T20）。

交付物：

- 转换器 + CLI；
- 40/40 Preserve 报告；
- 37/37 字节报告；
- 23/23 引擎 JSON 投影逐格/逐段对拍；
- 迁移测试与差异清单。

**门禁 G1**：

- T05：40/40 `rpx_to_json` 成功（Preserve）；
- T06：已知 37 个 `verify --bytes` 全过；
- T07：40/40 `model_equal`；
- T04：23/23 引擎夹具用 JSON 投影完成逐格/逐段对拍；
- 任何 `UNSUPPORTED_FIELD` 都有 pointer + 处理决定（补映射或进 legacy），不允许静默丢弃；
- CLI `format verify` 在 CI 可跑。

**回滚**：转换器是新增路径；保留 `template compile`/`report import-rpx` 原行为，不删旧代码。

---

### Phase 2 — 切主路径（4–6 周）

**目标**：让 JSON 成为日常唯一路径，`.rpx` 只做 legacy/测试。

**已完成（2026-10-17）**：

- `format/overlay.rs`：编辑后的 preserve JSON 用原模型整 struct 合并未建模字段（cell/row/col/report/dataset/print），`format to-rpx` 可正常写出。
- `format normalize`：v0/v1 → v1；`template create` / `library init --presets` 写 v1 标记。
- 23/23 引擎夹具 JSON 投影逐格/逐段对拍；40/40 产品 RPX Preserve + 快照字节往返。
- 引擎 JSON 门禁读取 `tests/fixtures/format/engine/*.json`；`.rpx` 只保留迁移/兼容对照。
- 结构性编辑（增删行列）默认报 `LEGACY_STRUCTURAL_EDIT`，不静默丢字段；`--allow-structural` 可显式接受。

**Phase 3 已开始**：

- `rpx-runtime` / `rpx-cli` 增加 `legacy-rpx` feature（默认开启，保持兼容）。
- `format`、`report import-rpx`、`template compile`、`.rpx` reader 分支都受 feature 控制；`cargo check -p rpx-cli --no-default-features` 通过，JSON-only 构建不暴露 legacy 命令。
- `rpx-core` 拆分完成：`writer`、`open/open_bytes`、`fill`、`Cell/RowCell/ColCell/Param/PrintSetup/DataSet` 读方法、`RecordReader` 都受 `legacy-rpx` 控制；`ObjValue` 移到独立模块，JSON-only 仍可用。
- no-default 的 runtime/CLI 依赖图不再启用 `rpx-core/legacy-rpx`；core 的二进制测试用 `required-features` 隔离。
- `scripts/json_only_smoke.sh` 在 no-default 构建上跑 validate → run → result inspect → library/preset → report run；CI 同步执行。
- 剩余：默认关闭 `legacy-rpx`。

任务：

1. writer 默认写 v1（`format`/`format_version`/`schema`）；保留 `--compat=v0` 一个版本。
2. `rpx-runtime` 的 `reportdef` parse/build 切到 `rpx-format`（或共享同一套 JSON 表）。
3. 引擎测试默认输入改 JSON；`.rpx` 只保留兼容性测试和 37 字节基线。
4. CLI 日常命令不再要求 `template compile`；`compile`/`import-rpx` 打印 deprecation 到 stderr。
5. library：
   - 保存/读取 v1；
   - 旧库按需迁移；
   - `source_revision` 继续按原始字节，避免 If-Match 全部失效；
   - 增加 `rpx format migrate`（库级 dry-run）。
6. Web/Server：
   - 只接受 v1/v0 JSON；
   - 保存时保留未知 `extensions`；
   - Web 不再提示“上传数据文件用于运行”（Web 不执行引擎）。
7. 文档全部切换当前架构；历史规格归档。

**门禁 G2**：

- 正常 CLI 路径不出现 `.rpx`（除 legacy 命令）；
- 23 个引擎夹具全部由 JSON 输入驱动；
- 旧库可读，保存后 revision 行为符合预期；
- v0 reader 仍可读；
- Web 保存/读取往返不丢 `extensions`；
- 文档索引中不再把 `/render`、`/export`、`/meta` 当现状。

**回滚**：`--compat=v0` + 双读；writer 的 v1 输出可用 feature flag 关闭；不删任何 `.rpx` 代码。

---

### Phase 3 — legacy 退役（持续，至少两个 minor 版本）

**目标**：降低 `.rpx` 在默认依赖中的权重，但不破坏外部资产。

任务：

1. `rpx-core` 变成可选 feature（`legacy-rpx`），默认构建不包含 `.rpx` 读写；`rpx format`/import/export 才启用。
2. `rpx-format` 成为 JSON 解析/构建的唯一入口；`rpx-runtime` 不再含 `reportdef`。
3. 单文件包：`.rpxpkg`（zip：`manifest.json` + `template.json` + `data/` + `assets/`），可选。
4. schema v2（如需要）：
   - 把 `print_setup`/`printing`/`group_breaks`/`background_image` 归入 `print` 对象；
   - 把 cell 样式归入 `style`；
   - 只在有迁移工具时做。
5. 发布策略：
   - v1.x 继续读 `.rpx`；
   - v2.0 默认不含 `.rpx`，需安装 legacy feature 或使用 `rpx-legacy` 工具；
   - 至少两个 minor 版本的 deprecation 窗口。

**门禁 G3**：

- 默认构建无 `rpx-core` 依赖；
- 所有引擎/运行时测试用 JSON；
- 外部 `.rpx` 资产有迁移指南；
- `format` 转换器仍可独立安装；
- 至少两个 release 的兼容窗口。

**回滚**：feature flag 重新打开；`rpx-core` 代码不删除，只改变默认 feature。

---

## 6. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| 转换器“看起来成功”但丢字段 | 中 | 高 | Preserve 模式 + L1/L3 断言 + `verify` 门禁；禁止静默 drop |
| 40 个产品夹具转换失败 | 高 | 高 | Phase 1 先修未建模字段；T05/T07 硬门禁；不进入 Phase 2 |
| 旧 reader 读不了新 JSON | 中 | 高 | reader 先行；`--compat=v0`；发布顺序写入 ADR |
| legacy 与编辑冲突 | 中 | 中 | key 用地址/名字；`LEGACY_CONFLICT`/`LEGACY_ORPHAN` 显式报错 |
| 字节往返不再成立 | 高 | 中 | L2 只对 37 个已知夹具做硬门禁；其他报告 diff，不阻断 L1 |
| 文档再次漂移 | 高 | 中 | Phase 0 文档索引；过期规格横幅；CI 检查引用 |
| 迁移拖慢当前 CLI 修复 | 中 | 中 | Phase 0 先修 P0；格式迁移不与 CLI 修复混在一个 PR |
| 外部用户依赖 `.rpx` | 中 | 高 | 两个 minor 兼容窗口；legacy feature；迁移工具 |

---

## 7. 回滚策略

- **Phase 0**：只加 reader/文档，revert 即可。
- **Phase 1**：转换器是新增路径，旧 `compile`/`import` 不动；关闭新 CLI 即可。
- **Phase 2**：writer 有 `--compat=v0`；reader 双读；feature flag 关闭 v1 输出。
- **Phase 3**：`legacy-rpx` feature 重新打开；不删除代码，只改默认。

**硬规则**：在 Phase 1 门禁通过前，不删除任何 `.rpx` 代码；在 Phase 3 之前，不改变默认构建的 `.rpx` 可读性。

---

## 8. 度量

| 指标 | 目标 |
|---|---|
| Preserve 转换成功率（40 夹具） | 40/40 |
| 语义往返（40 夹具） | 40/40 `model_equal` |
| 字节往返（37 已知） | 37/37 |
| 引擎夹具 JSON 化 | 23/23 JSON 投影逐格全绿 |
| Strict 模式的未支持字段 | 全部有明确错误与处理决定 |
| CLI `format verify` CI 时长 | 在可接受预算内（建议 <2 min） |
| 旧库迁移成功率 | 100%（dry-run 可回滚） |
| 文档引用 | G7 全绿；当前态/历史态可区分 |

---

## 9. 决策记录

- 2026-10-17：提出 ADR；建议采纳选项 B。
- 2026-10-17：采纳选项 B，开始实施。已完成：
  - Phase 0：baseline commit `33c6b11` + tag `v0.2.0-baseline`；`.github/workflows/ci.yml`；`format` / `format_version` / `meta` / `extensions` reader；`tests/fixtures/format/` golden corpus；schema 文件。
  - Phase 1（迁移快照模式）：`rpx_runtime::format` 的 `rpx_to_json` / `json_to_rpx` / `verify`；`rpx format to-json|to-rpx|verify|check`；40/40 产品 `.rpx` Preserve + 快照字节往返；23/23 引擎夹具 JSON 投影逐格/逐段对拍。
  - Phase 1 收口：23/23 引擎夹具用 JSON 投影完成 `calculated`/`pagination`/`merged` 对拍；`engine_merge` 的合并延续独立格已用快照 overlay 还原。
- Phase 2：字段级 `format/overlay.rs` 已实现（编辑后的 preserve JSON 可写出）；`format normalize`；`template create` / `library init --presets` 写 v1；23/23 引擎夹具 JSON 对拍。
- 结构性编辑：默认拒绝；`format to-rpx --allow-structural` 可显式接受，只保证坐标不变的重叠区。
- 待办（Phase 3）：默认关闭 `legacy-rpx`。
- 评审通过条件：
  1. 确认“JSON canonical + RPX legacy”是目标；
  2. 确认 Phase 0 的 baseline/CI 是前置；
  3. 确认 Phase 1 的 40/40 Preserve、40/40 快照字节、23/23 JSON 对拍是硬门禁；
  4. 确认不重写引擎、不改 `ReportDefine` 内部模型。

---

## 10. 参考

- 格式设计：`docs/format/rpx-report-json-v1.md`
- 转换器与测试：`docs/format/rpx-core-json-converter.md`
- 现状审计：`docs/upgrade-readiness-review-2026-10-17.md`
- 当前 CLI 协议：`docs/agent-cli.md`
- 当前定义契约：`docs/report-def-json.md`
- 二进制模型：`crates/rpx-core/src/report.rs`、`crates/rpx-core/src/cell.rs`、`crates/rpx-core/src/print_setup.rs`
- 引擎夹具：`crates/rpx-engine/tests/engine_grid_compare.rs`
