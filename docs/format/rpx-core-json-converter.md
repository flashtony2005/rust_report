# `rpx-core → JSON` 无损转换器 — 接口与迁移测试清单

> 状态：**Phase 2 字段级 overlay 已实现并收口（cell/row/col/report/dataset/print）；字节相等只承诺未编辑快照**
> 上游格式：`docs/format/rpx-report-json-v1.md`
> 决策：`docs/adr/0001-json-canonical-format.md`
> 本文里的“无损”有严格分层，不是一句口号；见 §1。

---

## 1. 无损的四个层级

| 层级 | 名称 | 判据 | 本设计是否承诺 |
|---|---|---|---|
| L0 | 语法无损 | JSON 可解析、schema 通过 | 是 |
| L1 | **语义无损** | `JSON → ReportDefine` 与原始 `ReportDefine` 逐字段相等 | **是（默认）** |
| L2 | 字节无损 | `to_bytes(JSON→model) == 原始 .rpx` | **仅对已知确定性夹具**（当前 37 个） |
| L3 | 未建模字段保留 | 无法映射到 v1 的字段由 `extensions["rpx.legacy"]` 原始快照承载 | **是（Preserve 模式，快照级）** |

因此：

- **迁移默认 = L1 + L3**：所有 `ReportDefine` 字段不丢；Phase 1 用原始 `.rpx` 快照承载未建模字段（不是逐字段覆盖）。
- **L2 不是对所有产品文件的承诺**：writer 的规范化可能改变字节；当前 `to_bytes` 对 37 个夹具逐字节复现，`verify --bytes` 只应把这 37 个当硬基线。
- “Strict 模式”只承诺 L1；遇到未建模字段直接失败，不静默丢。
- “Drop 模式”只用于“渲染不需要 legacy”的场景，必须显式开启并输出 warning。

---

## 2. 代码位置与依赖

### 2.1 推荐最终结构

```text
rpx-core       二进制模型 / 读写 / 零依赖
   ▲
rpx-format     JSON schema / v1 读写 / legacy 转换
   ▲
rpx-runtime    运行编排 / 库 / 快照 / 导入
   ▲
rpx-cli        rpx format ...
rpx-server     只读库 API
```

- 新 crate：`crates/rpx-format`。
- 依赖：`rpx-core`、`serde_json`、`sha2`、`base64`。
- `rpx-engine` 不依赖它，保持零依赖。
- `rpx-runtime` 不再自己维护一套 JSON 解析；`reportdef` 的 parse/build 迁到 `rpx-format`（Phase 2）。
- Phase 1 可以先放在 `rpx_runtime::format`，API 不变；Phase 2 再抽 crate，避免一次性大搬迁。

### 2.2 分层门禁

G8 需要新增：

```text
rpx-format -> rpx-core
rpx-runtime -> rpx-format
rpx-cli -> rpx-format
rpx-server -> rpx-format
```

不允许：

```text
rpx-format -> rpx-engine
rpx-core -> rpx-format
rpx-engine -> rpx-format
```

同时要改 `scripts/dep_allowlist.txt`（`rpx-format serde_json` / `sha2` / `base64`）和 G8 矩阵；否则新 crate 会被门禁直接拦下。

---

## 3. 公共接口

```rust
// crates/rpx-format/src/lib.rs

use rpx_core::ReportDefine;
use serde_json::Value as J;

pub const FORMAT: &str = "rpx-report";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertMode {
    /// 只接受可映射字段；遇到未建模字段立即失败。
    Strict,
    /// 可映射字段进 v1；未建模字段进 extensions["rpx.legacy"]。
    PreserveLegacy,
    /// 丢弃未建模字段，输出 warning；仅用于渲染。
    DropLegacy,
}

#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub mode: ConvertMode,
    pub source_sha256: bool,
    pub schema_id: Option<String>, // 默认 Some("rpx-report/v1")
}

impl Default for ConvertOptions { /* Strict + source hash */ }

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: &'static str,
    pub pointer: String,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Converted {
    pub document: J,
    pub warnings: Vec<Diagnostic>,
    pub legacy_entries: usize,
    pub source_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diff {
    pub pointer: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone)]
pub struct VerifyReport {
    pub model_equal: bool,
    pub bytes_equal: Option<bool>,
    pub diffs: Vec<Diff>,
    pub legacy_entries: usize,
    pub json_sha256: String,
    pub source_sha256: String,
}

#[derive(Debug)]
pub struct ConvertError {
    pub code: &'static str,
    pub pointer: String,
    pub message: String,
    pub hint: Option<String>,
}

pub type Result<T> = std::result::Result<T, ConvertError>;

/// .rpx bytes → v1 JSON（不经过用户 import 的严格重建）。
pub fn rpx_to_json(bytes: &[u8], opts: &ConvertOptions) -> Result<Converted>;

/// 已解析的 ReportDefine → v1 JSON。
pub fn model_to_json(rd: &ReportDefine, opts: &ConvertOptions) -> Result<Converted>;

/// v1 / v0 JSON → ReportDefine；忽略 extensions["rpx.legacy"]。
pub fn json_to_model(doc: &J) -> Result<ReportDefine>;

/// v1 / v0 JSON → .rpx bytes；Preserve 且未编辑时返回原始快照，编辑后走字段级 overlay。
pub fn json_to_rpx(doc: &J, opts: &ConvertOptions) -> Result<Vec<u8>>;

/// 迁移验收：读 → JSON → 模型/字节 → 再读，比较。
pub fn verify(bytes: &[u8], opts: &ConvertOptions) -> Result<VerifyReport>;
```

### 3.1 关键语义

| 函数 | legacy 处理 | 用途 |
|---|---|---|
| `rpx_to_json` | Preserve 时写入 | 迁移/导出 |
| `json_to_model` | 忽略 | runtime/渲染/校验 |
| `json_to_rpx` | Preserve 时未编辑原样返回；编辑后 overlay 合并未建模字段再写出 | 回写/验收 |
| `verify` | Preserve 时使用快照 | 测试/交付验证 |

- `json_to_model` 必须忽略未知 `extensions`，否则 Web 加一个 vendor 扩展就会让运行失败。
- `json_to_rpx` 在 `PreserveLegacy` 下读取 `extensions["rpx.legacy"]`：`modeled_sha256` 一致则原样返回原始字节；不一致（即 JSON 被编辑）则用字段级 overlay 合并未建模字段后写出。语义保留，不保证字节相等。
- `LEGACY_INVALID` 用于快照 base64/sha256/format 损坏；不静默忽略。
- `rpx_to_json` 在 `Strict` 下遇到任一未建模字段就失败；错误必须带 JSON Pointer。

### 3.2 错误码

| code | 含义 | 退出码（CLI） |
|---|---|---:|
| `FORMAT_VERSION_UNSUPPORTED` | reader 不支持该 `format_version` | 3 |
| `UNSUPPORTED_FIELD` | Strict 模式下有未建模字段 | 3 |
| `LEGACY_INVALID` | 迁移快照损坏（base64/sha256/format） | 3 |
| `LEGACY_STRUCTURAL_EDIT` | 含快照时增删行列（Phase 2 暂不支持） | 3 |
| `ROUNDTRIP_MISMATCH` | verify 的 L1/L2 不成立 | 3 |
| `INVALID_JSON` / `INVALID_MODEL` | 输入不合法 | 3 |
| `SIZE_LIMIT` | 超出网格/文件上限 | 3 |
| `IO_ERROR` | 读写失败 | 6 |

---

## 4. 转换算法

### 4.1 `.rpx → JSON`

```text
bytes
  → rpx_core::open_bytes
  → ReportDefine
  → modeled::document(rd)           # v1 可表达字段 + warnings
  → 按 ConvertMode 处理：
      Strict   : 任一 warning → FORMAT_UNSUPPORTED_FIELD
      Drop     : 丢弃未建模字段，输出 warnings
      Preserve : 把原始 bytes 放进 extensions.rpx.legacy（含 modeled_sha256）
  → 补 format / format_version / schema
  → definition::parse 回读验证
  → Converted
```

关键点：

- `rpx_core::open_bytes` 是唯一解析入口；不要复制字段读取逻辑。
- `modeled::document` 只写 v1 有字段的槽；无法表达的字段进入 warnings，Preserve 由原始快照兜底。
- 先回读再返回：`definition::parse` 必须接受产物，否则转换失败；不产出“看起来成功”的 JSON。
- Phase 1 的 L1 由 `json_to_rpx` 的原始快照保证；逐字段 L1 比较在 Phase 2 字段级 legacy 落地后补。
- `source_sha256` 是对原始 bytes 的哈希，便于追溯；不是完整性校验门。

### 4.2 `JSON → .rpx`

```text
doc
  → 若 Preserve 且存在快照：
        modeled_sha256(doc) == snapshot.modeled_sha256 ?
          → 返回 snapshot.bytes
          → 否则继续
  → json_to_model(doc)              # v0/v1 + 字段级 overlay
  → validate_model(rd)
  → rpx_core::to_bytes(rd)
  → （可选）open_bytes(bytes) 断言模型相等
```

- `json_to_model` 与 `rpx_to_json` 共用同一套 JSON 字段表。
- 编辑路径由 `format/overlay.rs` 把原 `ReportDefine` 的未建模字段合并回解析模型：整 struct clone + 覆盖已建模槽，避免逐字段枚举遗漏。
- 写回后再次 `validate_model`，防止 JSON 改动产生非法模型。

### 4.3 `verify`

```text
original = open_bytes(bytes)
json = rpx_to_json(bytes, Preserve)
rebuilt = json_to_rpx(json, Preserve)
actual = open_bytes(rebuilt)

model_equal = original == actual
bytes_equal = (original_bytes == rebuilt_bytes)   # 仅 --bytes
source_sha256 = sha256(bytes)
json_sha256 = sha256(canonical(json))
```

`diffs` 至少覆盖：

- `rows/cols`
- `row_cells` / `col_cells`
- `store`/`grid`/`merges`
- `datasets`
- `params`
- `print_setup`
- `custom_properties`
- `exp_map`
- `scalars`
- `macro_meta`/`sub_report_meta`/`back_graph_config`/`export_config`

---

## 5. 字段覆盖矩阵

### 5.1 报表级

| 字段 | v1 表达 | 处理 |
|---|---|---|
| `rows`/`cols` | `grid` | 建模 |
| `row_cells`/`col_cells` | `row_props`/`col_props` | 部分建模；未开放属性进 legacy |
| `store`/`grid`/`merges` | `cells`/`merge` | 建模 |
| `print_setup` | `print_setup` 子集 | 未开放槽进 legacy |
| `params` | `params` | 建模 |
| `datasets` | `datasets` 子集 | 高级槽进 legacy |
| `custom_properties` | `macros`/`printing`/已知键 | 未知键进 legacy |
| `exp_map` | 无 | legacy |
| `scalars` | 无 | legacy（base64/结构化） |
| `macro_meta` | 无 | legacy（base64） |
| `sub_report_meta` | 无 | legacy（base64） |
| `back_graph_config` | `background_image` 已知部分 | 未知部分 legacy |
| `export_config` | 无 | legacy（base64） |

### 5.2 单元格级

| 字段 | v1 表达 | 处理 |
|---|---|---|
| `merged_area` | `merge`（锚点） | 建模 |
| `h_align`/`v_align` | 同名 | 建模 |
| `extend_mode` | `extend_mode` | 建模 |
| `left_head`/`top_head` | 同名 | 建模 |
| `value`/`disp_value` | `literal`/`text`/`expr`/`disp_value` | 支持的类型建模；不支持的类型进 legacy |
| `indent`/`font_*`/颜色/边框 | 同名 | 建模 |
| `format` | `format` | 建模 |
| `rotation_angle` | `rotation` | 建模 |
| `bit_flags` 的已知位 | `bold`/`italic`/`underline`/`split`/`stretch_when_paged` | 建模 |
| `merge_same` | `merge_same` | 建模 |
| `diagonal_*` | 无 | legacy |
| `extend_area` | 无 | legacy |
| `to_excel_mode`/`adjust_size_mode` | 无 | legacy |
| `cell_obj` | 无 | legacy |
| `hyperlink`/`hyperlink_window` | 无 | legacy |
| `exp_map` | 无 | legacy |
| `cell_type` | 无 | legacy |
| `notes` | 无 | legacy |
| `ois_blob` | 无 | legacy（base64） |
| `cell_graph_config` | 无 | legacy（base64） |
| `under_line_style` | 无 | legacy |
| `cell_style_name` | 无 | legacy |
| `merge_null`/`merge_same_mode`/`merge_null_mode` | 无 | legacy |
| 其余 `bit_flags` | 无 | `extra_bit_flags` |

### 5.3 `ObjValue` 的 legacy 编码

`literal` 只覆盖 JSON 能安全表达的类型。其余 `ObjValue`（`Obj`/`BigInteger`/`Millis`/`Bytes` 等）在 legacy 中使用带标签编码，避免“看起来是字符串”：

```json
{ "k": "int", "v": 42 }
{ "k": "decimal", "scale": 2, "unscaled": "base64:..." }
{ "k": "obj", "class_name": "...", "raw": "base64:..." }
{ "k": "millis", "tag": 3, "millis": 1700000000000 }
```

规则：

- 标签与 `rpx_core::reader::ObjValue` 一一对应；
- 未知标签 → `UNSUPPORTED_VALUE`；
- 不允许用 `{"value": "..."}` 猜测类型。

### 5.4 数据集级

| 字段 | v1 表达 | 处理 |
|---|---|---|
| `name` | `name` | 建模 |
| `file_name`/`sheet_name`/`encoding`/`seperator` | `file.*` | 建模（`seperator` → `delimiter`） |
| `data_source_name`/`sql` | `sql.*` | 建模 |
| `release_mode`/`cache_data`/`col_titles`/`no_data_prompt` | 无 | legacy |
| `col_list`/`title`/`quote_removed`/`col_type_list`/`col_format_list`/`filter`/`gather*`/`group*`/`select*`/`change_file`/`begin_row`/`end_row` | 无 | legacy |

### 5.5 打印级

| 字段 | v1 表达 | 处理 |
|---|---|---|
| `paper_width`/`paper_height`/四边距 | `print_setup.*` | 建模 |
| `pager_style`/`row_num_per_page`/`zoom_scale`/`layout_*`/`copy_count`/四个 bool/`virtual_printer` | `print_setup.*` | 建模 |
| `paper`/`orient`/`zoom_page_*`/`h_align`/`v_align`/`zoom_mode`/`layout`/各 `*_mode`/`column_mode` | 无 | legacy |

---

## 6. legacy 舱的 key 设计

**实现使用单个原始快照**（`format` / `encoding` / `sha256` / `modeled_sha256` / `data`）作为未建模字段的来源；`format/overlay.rs` 在模型层按 cell/row/col/report/dataset/print 合并。下面这份地址/名字映射是描述性目标，不是额外的 JSON 键。

不要用数组下标，用稳定标识：

```json
"extensions": {
  "rpx.legacy": {
    "format": "rpx-rpx-v1",
    "source_sha256": "...",
    "report": { "...": "..." },
    "cells": { "C6": { "...": "..." } },
    "row_props": { "3": { "...": "..." } },
    "col_props": { "2": { "...": "..." } },
    "datasets": { "ds1": { "...": "..." } },
    "print_setup": { "...": "..." }
  }
}
```

- 单元格 key 用 `A1` 地址，不用 `cells[17]`；
- 行/列 key 用十进制编号；
- 数据集 key 用 name；
- 如果编辑移动了单元格/改了数据集名，legacy 找不到宿主 → `LEGACY_ORPHAN`，必须显式处理（不要让 legacy 静默丢）。
- Preserve 模式适合迁移，不适合长期编辑；长期编辑应逐步把 legacy 字段升级为 v1 字段或删除。

---

## 7. CLI 设计

```sh
# .rpx → JSON
rpx format to-json input.rpx --out report.json --mode preserve --source-hash

# JSON → .rpx（legacy 回写）
rpx format to-rpx report.json --out legacy.rpx --mode preserve

# 验收
rpx format verify input.rpx --mode preserve --bytes
rpx format verify input.rpx --mode preserve --json /tmp/report.json

# 只检查 JSON
rpx format check report.json --schema v1

# 旧入口（保留一个版本，打印 deprecation 到 stderr）
rpx template compile report.json --out legacy.rpx
rpx report import-rpx legacy.rpx --id legacy --library ./reports
```

`format verify` 输出：

```json
{
  "ok": true,
  "command": "format.verify",
  "data": {
    "model_equal": true,
    "bytes_equal": true,
    "legacy_entries": 12,
    "source_sha256": "…",
    "json_sha256": "…",
    "diffs": []
  }
}
```

规则：

- 默认 `--mode strict` 用于作者；迁移默认建议 `--mode preserve`。
- `--bytes` 只对已知确定性夹具当硬门禁；其他文件报告 `bytes_equal:null` 或 `false`，不阻断模型无损。
- `template compile` / `report import-rpx` 标注 legacy，不再作为日常工作流入口。
- 所有错误走统一 envelope；`UNSUPPORTED_FIELD` 必须带 `json_pointer`。

---

## 8. 迁移测试清单

| ID | 测试 | 夹具 | 通过判据 |
|---|---|---|---|
| T01 | v1 schema 自检 | `tests/fixtures/format/*.json` | 所有 golden 通过 schema；未知字段被拒 |
| T02 | v0 兼容读 | 当前 `examples/tutorial/*.json` | 无 `format` 也能 `json_to_model` |
| T03 | v1 读写 | 新增最小 v1 文档 | `json_to_model` / `model_to_json` 往返 L1 |
| T04 | **23 个引擎夹具 JSON 化** | `tests/fixtures/format/engine/*.json`（由 `.rpx` 迁移生成） | 23/23 通过：JSON 输入 → 引擎 → `calculated`/`pagination`/`merged` 逐段相等（含 `engine_cycle` 错误型） |
| T05 | **40 个 RPX Preserve** | `tests/fixtures/*.rpx` | 40/40 `rpx_to_json` 成功，`legacy=true` 且有原始快照 |
| T06 | **字节往返** | 40 个 + 历史 37 个 | Preserve 快照：40/40 `bytes_equal=true`；native writer：37/37（Phase 2 门禁） |
| T07 | 40 个语义往返 | 40 个 | 40/40 `model_equal=true` |
| T08 | 单元格未建模字段 | 含 diagonal/extend_area/ois_blob 的夹具 | legacy 保留，回写后 `ReportDefine` 相等 |
| T09 | 行/列未建模字段 | 含 notes/exp_map/ois_blob 的夹具 | 同上 |
| T10 | 报表级 blobs/custom props | `macro_meta`/`scalars`/未知 custom key | base64/结构化保留，L1 相等 |
| T11 | 数据集高级槽 | `colList`/`filter`/`beginRow` 等 | legacy 保留，L1 相等 |
| T12 | print_setup 全槽 | `paper`/`orient` 等 | legacy 保留，L1 相等 |
| T13 | `ObjValue` 未建模类型 | BigInteger/Millis/Obj/Bytes | 标签编码，L1 相等；未知标签显式失败 |
| T14 | 快照编辑 overlay | 手工编辑 preserve JSON | 未编辑时 `bytes_equal=true`；字段编辑可写出且未建模字段保留；增删行列报 `LEGACY_STRUCTURAL_EDIT` |
| T15 | 规范化/确定性 | 同语义不同 key 顺序/空白 | canonical JSON 相同；`source_revision` 仍按原始字节 |
| T16 | fuzz/鲁棒性 | 随机 JSON + 随机 RPX bytes | 无 panic；错误有 pointer；大小上限 |
| T17 | CLI 契约 | 真实二进制 | `format to-json/to-rpx/verify/check` 的 0/3/6 退出码与 JSON envelope |
| T18 | 跨版本/库迁移 | v0/v1 + library | v0 读入、v1 写出、revision 变化符合预期；旧结果仍可读 |
| T19 | 大网格/性能 | 999×999、40 夹具 | 转换在预算内（建议 <1s/夹具）；无异常内存 |
| T20 | Web 往返 | Web 导出的定义 JSON | `format check` 通过；保存/读取不丢字段 |

### 8.1 当前基线与阻断项

- 当前 `report import-rpx` 对 40 个产品 `.rpx` **0/40** 成功（详见 `docs/upgrade-readiness-review-2026-10-17.md`）。因此：
  - **不能**用用户 import 作为迁移工具；
  - 必须先实现本文的 `rpx_to_json`（直接走 `ReportDefine`，不经过 import 的严格 `compare`）；
  - T05/T07 是 Phase 1 的硬门禁，未过不得进入 Phase 2。
- 当前 37 个字节往返是 `rpx_core::to_bytes` 的既有证据（`docs/report-def-json.md:8-9`），T06 应复用同一夹具集。
- 23 个引擎夹具的 JSON 输入已落盘到 `tests/fixtures/format/engine/`；T04 直接读 JSON，不再在测试时从 `.rpx` 生成。`crates/rpx-engine/tests/engine_grid_compare.rs` 的 `.rpx` 版本保留为兼容对照。

---

## 9. 验收命令（建议写进 CI）

```sh
# 1) 格式/schema
cargo test -p rpx-format
# 2) 40 个 RPX 的 Preserve 转换
cargo test -p rpx-format --test legacy_corpus
# 3) 37 个字节往返
cargo test -p rpx-format --test byte_roundtrip -- --nocapture
# 4) 23 个引擎夹具 JSON 化
cargo test -p rpx-engine --test engine_grid_compare
# 5) CLI 真实进程
cargo test -p rpx-cli --test format_cli
# 6) 无 panic/大小上限
cargo test -p rpx-format --test fuzz_smoke
```

失败处理：

- `UNSUPPORTED_FIELD`：补字段映射或明确列入 legacy，不允许“加个 `#[serde(flatten)]` 就过”。
- `ROUNDTRIP_MISMATCH`：先定位字段，再改映射；不要放宽比较。
- `bytes_equal=false` 且夹具不在 37 个内：记录为已知差异，不做硬失败，但必须输出 diff。

---

## 10. 开放问题

1. `rpx.legacy` 是否允许长期存在？建议只用于迁移，两个 minor 版本后要求用户升级。
2. 单文件包用 zip 还是 base64 JSON？建议 zip（见格式文档 §13）。
3. `meta` 与 library manifest 的权威关系？建议库内 manifest 优先。
4. `source_revision` 是否改成 canonical hash？建议暂不改，避免所有已有 If-Match 失效。
5. 是否把 `reportdef` 整体抽到 `rpx-format`？建议 Phase 2 做，Phase 1 先复用 runtime 解析器。
