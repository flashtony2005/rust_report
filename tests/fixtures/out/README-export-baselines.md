# 导出基线（M6-05）

`export_engine_*.csv` / `export_engine_*.xlsx` 是 **19 个可导出引擎夹具**的导出
回归基线。**20 名义夹具 − `engine_cycle`**：它是错误型夹具（`calculate` 必报
循环引用、无计算态网格），物理上不可能导出；依据与活证见
`crates/rpx-export/tests/export_baseline.rs` 的
`engine_cycle_is_error_fixture_thus_excluded`。

## 生成（显式维护动作）

```bash
RPX_UPDATE_BASELINES=1 cargo test -p rpx-export --test export_baseline
```

仅在**导出契约合法变更**后执行；执行后把这些文件的 diff 作为这次变更的
证据一起提交。

## 比对（默认，回归网）

```bash
cargo test -p rpx-export
```

- **CSV**：逐字节（RFC 4180 子集即契约）；
- **xlsx**：**读回级**（calamine → 逐格 + 合并区文本）——zip 层噪声
  （压缩级别等）不在契约内，但**读回**的任何一格/一个合并区变化都会红
  （spec-m6 §2.4 / §3 M6-05「任一导出改动导致读回 diff 即红」）。

## 归档性质

这些文件是**夹具资产**（回归基线），不是构建产物——**入仓库**。它们在
`tests/fixtures/out/` 是混合目录（还含历史讲座 HTML / 验证脚本），
`.gitignore` 只精准忽略含口令的 `ds_*.json` 与沙箱写入垫片的 `*.res` 残留。

## 变更记录

- **2026-10-16**：修 `rpx-export::xlsx::rebuild_areas` 的**合并区静默丢失** bug
  （旧判据「存在另一条**同跨度**记录覆盖本槽 ⇒ 本槽是延续槽」会让**相邻等跨度**区
  互相把**对方的锚点**判成延续槽 ⇒ 后一区整区被丢）。重生成后 3 个夹具的
  **读回**变化（**只增不减**，且新增区恰为相邻等跨度区）：

  | 夹具 | 旧区数 | 新区数 | 新增 |
  |------|-------|-------|------|
  | `export_engine_merge.xlsx` | 4 | 6 | `A4:A5`、`A6:A7` |
  | `export_engine_samemerge.xlsx` | 1 | 2 | `A4:A5` |
  | `export_engine_cross.xlsx` | 7 | 9 | `F6:F7`、`F15:F16` |

  其余 16 个 `export_*.xlsx` 重生成后**读回完全相同**（解包逐条目哈希一致），
  仅 zip 层字节有**非契约**噪声（本地工具链与入库时的差异）⇒ **未改动**，保持最小 diff。
  回归钉：`crates/rpx-export/src/xlsx/tests.rs::adjacent_same_extent_areas_all_survive`。
