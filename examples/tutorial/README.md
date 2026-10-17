# 教程可运行示例

这些示例对应 `润乾报表教程-参悟.md` 第 6 节的主格、坐标、宏、打印与图形增量。
契约与已知边界见 [实现与验收](../../docs/tutorial-implementation.md) 和
[定义 JSON](../../docs/report-def-json.md)。

| 文件 | 操作 | 预期 |
| --- | --- | --- |
| `grouped-sales.json` + `sales.csv` | CLI 执行 JSON（配套数据同目录） | 两个区域各 3 条明细、组内排名、每组一张柱图；2 页、总计 190 |
| `chart-gallery.json` | CLI 执行 JSON | 28 类图各一页，最后一页为带 formatter 回调的 JavaScript 模板，共 29 页 |
| `images.json` | CLI 执行 JSON | 页面背景、格背景与一张图片；套打背景进入 PDF |
| `multi-column.json` | CLI 执行 JSON | 20 条记录，每栏 5 条、每纸 2 栏，共 2 页；第二列为序号 ×10 |
| `complex-master-expansion.json` + `complex-master-sales.csv` | CLI 执行 JSON（配套数据同目录） | 三级纵向主格 × 年度/季度两级横向扩展；23 行 × 25 列，8 名成员、两级小计、排名与序号；[详细说明](complex-master-expansion.md) |

`sales.csv` 与 `complex-master-sales.csv` 都是 **UTF-8、TAB 分隔**；这是项目当前文件数据集契约。

## Agent 执行，人查看

先安装并初始化共享库：

```bash
make install-local
export PATH="$HOME/.local/bin:$PATH"
rpx library init /tmp/tutorial-reports --presets
revision=$(rpx report show grouped-sales --library /tmp/tutorial-reports | jq -r '.data.revision')
rpx report run grouped-sales --library /tmp/tutorial-reports --if-match "$revision" --export xlsx --export csv --export print-xlsx
cargo run --locked -p rpx-server -- serve --library /tmp/tutorial-reports --addr 127.0.0.1:8090
```

另一个终端运行 `npm run dev --prefix web`，打开 `http://127.0.0.1:5288/`。
选择报表，查看保存的结果或图形与打印预览。人保存共享定义后，由 agent 重新读取修订并执行。
页面保存、下载、刷新均不运行引擎。完整命令见 [CLI 使用](../../docs/agent-cli.md)。

也可直接运行文件：

```bash
rpx run examples/tutorial/grouped-sales.json --out-dir /tmp/tutorial-sales --export print-xlsx
```

打印 Excel 每页一张工作表，图表数据放在隐藏的 `_chart_data` 工作表。
三维、任务/区间图、自定义模板、打印背景使用浏览器 PDF；这些图形请求 `print-xlsx` 会明确失败。

## 重现验收

```bash
bash scripts/gate.sh
python3 scripts/fault_inject_tutorial.py
```

故障注入只改临时副本，每层分别验证基线通过、错误实现触发断言、恢复后通过。
日志在 `output/tutorial/faults/`。浏览器截图/PDF 在 `output/playwright/`；
这两类可重生成产物不进入 Git。
