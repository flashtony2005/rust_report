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
