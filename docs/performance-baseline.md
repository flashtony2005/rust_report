# 大模板性能基线

> 日期：2026-10-17；二进制：`target/debug/rpx`（debug 构建）；机器：本机开发环境。
> 目的：给 JSON-only 路径一个可重复的基线，不作为发布 SLA。

## 复现

```sh
python3 scripts/bench_large.py --binary target/debug/rpx --rows 100 --cols 50
```

脚本生成一个 100×50、5000 个静态文本 cell 的 JSON 模板，执行：

```text
rpx run ... --export csv
```

## 基线（debug）

| 指标 | 值 |
|---|---|
| 行 × 列 | 100 × 50 |
| cells | 5,000 |
| 模板大小 | 172,507 bytes |
| wall time | **1.07 s** |
| 退出码 | 0 |

## 使用建议

- 发布前用 `--release` 重跑；debug 数字只用于回归趋势。
- 关注模板大小、cell 数、计算表达式数量和导出格式四个维度。
- 如果耗时明显上升，先用 `cargo flamegraph`/`perf` 定位，再决定是否做流式/并行。
- 这只是静态文本模板；含大量表达式/数据集的模板需要单独基准。

## 后续

- SQL 取数已加 100k 行默认上限（`RPX_SQL_MAX_ROWS` 可调）。
- 大结果集仍是物化模型；真正流式需要引擎/快照层改造，单独立项。
