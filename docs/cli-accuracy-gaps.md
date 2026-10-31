# CLI 生成报表的准确性缺口（实测）

> 日期：2026-10-18　基线：`rpx 0.13.0`（源码 `11f0238`）；**P0-1 ~ P1-2 已于同日落地进 0.14.0**
> 方法：用 8 组最小对照模板（同一个可用基线，每次只改一处）跑 `template validate` + `run`，
> 观察「工具说成功」与「实际产物」是否一致。全部命令可复现。

---

## 1. 结论

**CLI 能准确生成"数学上正确"的报表，但不能保证"打出来对"。**

- 它保证的：语法、主格、表达式准备、计算、分页、导出——**能算出来**。
- 它不保证的：宏是否替换、边框是否落地、空行是否混进明细、同排多扩展列是否相乘、
  列宽是否够放下数字、几个导出是否要分别验证。

`template validate` 的 `checked` 固定为 6 项，`deferred` 明确写着
`["data_loading","calculation","pagination","export"]`——**结构对 ≠ 结果对**，这是设计如此，不是 bug。

所以"准确"不能靠单条命令，只能靠**一条固定的验证回路**：
`validate → run(全导出) → 读产物断言 → 才交付`。这篇文档记录回路上每个会静默出错的地方。

---

## 2. 八组对照实测

| # | 变体 | `validate` | `run` | 实际产物 | 判定 |
| --- | --- | --- | --- | --- | --- |
| P0 | 基线（正确写法） | valid | ok | 8 行 CSV，4 产物齐全 | ✅ |
| P1 | CSV 未写 `delimiter` | **valid** | **failed** | `CALCULATION_FAILED: Engine(UnknownCell { id: "ds.customer" })` | ❌ 报错不可读 |
| P2 | 数据集文件不存在 | **valid** | failed | `DATA_MISSING` + `json_pointer: /datasets/0/file/file_name` | ⚠️ 报错质量好，但 validate 放行 |
| P3 | `text` 里写 `${period}` | valid | ok | 输出字面量 `${period} 测试报表` | ❌ **静默** |
| P4 | 单元格声明 `border: solid` | valid | ok | xlsx `styles.xml` 只有 **1** 个 `<border>`，与不写完全一致 | ❌ **静默** |
| P5 | 扩展块与合计行之间留空行 | valid | ok | 8 行变 9 行，中间多一行空行，合计被推下去 | ❌ **静默** |
| P6 | 同一行两个 `extend_mode=vertical` | valid | ok | 行数不变（8→8），行为未定义 | ❌ **静默**（历史真实项目曾 6 行炸成 798 行） |
| P7 | `print-xlsx` 自定义纸张（0.1.0） | valid | **failed** | 整个 run 无任何产物，连 `result.json` 都没有 | ✅ 0.13.0 已修 |

**P1 的真相**：`UnknownCell { id: "ds.customer" }` 的真实原因是 CSV **没有按逗号切分**，
数据集只有一列、列名恰好是 `customer,amount`。此时唯一能看出来的命令是
`rpx dataset inspect FILE --name ds`（`columns: ["customer,amount"]`）。
报错信息自己不会告诉你去查 delimiter。

**P4 的真相**：边框在 JSON 里合法（`style: solid`），引擎也接受，但导出侧不落地——
Univer 快照的 style 只有 `bg/bl/cl/ff/fs/ht/vt/n`，xlsx 只有默认边框定义。
**写了等于没写，且没有任何提示**。

---

## 3. 顺带发现的版本问题

| 项 | 事实 |
| --- | --- |
| 已安装二进制 | `~/.local/bin/rpx`，2026-10-17 18:11，`rpx --version` → **0.1.0** |
| 源码 | `0.13.0`，CLI 最后改动 `11f0238`（10-17 22:41），比安装晚 4.5 小时 |
| 后果 | 按源码文档写的 `rpx doctor --library` 在已装版本上直接 `USAGE` 报错；`print-xlsx` 仍是"一个导出失败拖垮整次 run"的旧行为 |
| 根因 | 没有任何"安装版本 vs 源码版本"的一致性检查；`doctor` 只报自己的版本，不报是否落后 |

> 对一个"Agent 驱动的 CLI"来说这是最危险的一类问题：文档、二进制、源码三者不一致，
> 而 Agent 恰好最信任文档。

---

## 4. CLI 该不该增强：该，按性价比排序

| 优先级 | 增强 | 解决什么 | 落点 | 验收（可证伪） |
| --- | --- | --- | --- | --- |
| **P0-1** | 错误消息加 `hint` | P1：`UnknownCell` 不知道该查什么 | `crates/rpx-runtime/src/error.rs`（`Error::new` 的 diagnostic 构造）+ 计算阶段 UnknownCell 映射 | 对 P1 复现用例，`error.hint` 明确写出「已加载列：customer,amount，疑似 delimiter 未设置」 |
| **P0-2** | `template validate --probe-data` | validate 放行"文件不存在/列不存在" | `rpx-runtime/src/definition.rs` 增加可选数据探针（只在传 flag 时执行） | P1/P2 在 validate 阶段即失败；不传 flag 行为不变（离线可用） |
| **P0-3** | 布局 lint（新命令或 validate 子集） | P3/P4/P5/P6 四类静默失败（本文的 `verify_report.py` 是原型） | 新增 `rpx-runtime/src/lint.rs`，纯静态（不取数） | 4 个变体各报 1 条 `level=warning` 且给出修法；基线 0 告警 |
| **P0-4** | `run` 返回 `layout` 摘要 | 不知道打出来几页、有没有超宽 | `run.rs` 的 `data` 增加 `layout: {page_count,x_pages,y_pages,rows}`（数据已存在 `workbook.json.pagination`） | `run` 的 stdout 即可读到页数，不必再开 workbook.json |
| **P1-1** | `capabilities --json` | Agent 无法自描述能力 | 新增 `rpx-runtime/src/capabilities.rs` | 输出含 version/commands/exports/limits/features，有快照测试 |
| **P1-2** | `doctor` 检测版本落后 | §3 | `doctor` 比对 `CARGO_PKG_VERSION` 与安装时间/源码修订 | 安装 0.1.0、源码 0.13.0 时 `checks` 出现 `binary_stale` |
| **P2** | `--output text` 覆盖所有子命令 | 人读时仍需 jq | 已有全局 flag，补齐渲染 | 主要命令都有可读输出 |

**判断标准（写给未来的自己）**：一条增强值不值得做，就看它是否让
**"Agent 单独跑完一次交付"的成功率上升**。P0-1/P0-3 之所以排最前，是因为它们把
"人看一眼就发现"的问题，变成"命令自己说得清楚"。

### 4.1 实施状态（0.14.0，2026-10-18）

六项全部落地，且每项都有**可证伪**的回归用例（`crates/rpx-runtime/tests/lint.rs`、
`crates/rpx-cli/tests/agent_surface.rs`，共 14 个用例）。

| 项 | 落地形态 | 实测证据 |
| --- | --- | --- |
| P0-1 hint | `Error::hint()`；`DATA_MISSING` / `DATA_FETCH` / `CALCULATION_FAILED` / `DATASOURCE_CONFIG` 四类带 hint | 缺 delimiter 时 `CALCULATION_FAILED` 的 hint 直接写出"只解析出 1 列「customer,amount」，加 \"delimiter\": \",\"" |
| P0-2 探针 | `template validate --probe-data [--data-dir D]` | 同一份模板：不传 flag `valid:true`；传 flag 退出 4、`code=DATA_SHAPE`、`json_pointer=/datasets/0/file/delimiter` |
| P0-3 lint | `rpx lint FILE [--data-dir D] [--strict]`，规则 L001–L008 | 四个病态变体各报且只报一条；`examples/r-model` 26 份模板只报 1 条真告警（24 个 `#,##0.00` 在 12mm 列） |
| P0-4 layout | `run.data.layout{rows,cols,paginated,page_count,x_pages,y_pages,page_columns,row_num_per_page}` | 一次调用即可判页数与是否横向分页 |
| P1-1 capabilities | `rpx capabilities`：26 条命令、5 个导出/数据源、limits、features、31 条错误码目录、退出码语义 | 有测试断言命令面包含 `lint`/`capabilities`/`run`/`template.validate`，且 `CALCULATION_FAILED` 必须带 hint |
| P1-2 doctor | `checks[0] = binary`：在源码仓库内比对二进制与 `crates/rpx-cli/Cargo.toml`；落后则 `ok:false, stale:true` | 仓库内跑 `rpx doctor` 显示"与源码一致"；仓库外显示"无法比对"而不是假装通过 |

**仍未做**：`--output text` 只覆盖了 `lint` 与 list 型命令（P2 未全面铺开）。
**刻意不做**：把 PDF 塞进 CLI，理由见下。

**L 码与旧 S 码的映射**（旧 Python 闸门的静态检查已删除，改为调用 `rpx lint`，
避免两套规则各自漂移）：L001 宏写 text、L002 多扩展根、L003 扩展块空行、
L004 border 不落地、L005 文件缺失、L006 缺 delimiter、L007 列宽超纸张、
L008 窄列两位小数。

**一处口径修正**：L002 的判据从"同排多个 vertical"改成"同排多个**根**"。
`detail-merge-same.json` 里 D4/E4 共享父格 C4 是合法写法（兄弟同步扩展），
按旧判据会误报；按"根的数量 > 1"判才既拦住相乘又放过兄弟。

**不建议做**：把 PDF 生成塞进 CLI（`--export pdf`）。现有架构里打印页映射在
`snapshot/paged.rs`、渲染在 Web，硬塞 CLI 会产生第二套分页逻辑。PDF 应作为
`rpx-server` 的出口，复用同一份打印 HTML。

---

## 5. 那么"准确"怎么落地：一条回路 + 一个技能

单靠命令无法保证准确，靠的是固定回路。已经把它固化成一个可执行闸门：

```sh
python3 skills/rpx-report-authoring/scripts/verify_report.py ./template.json \
  --data-dir ./data --expect-rows 10 --print --expect-cell C4=12000
```

它做四件事：

1. **静态 lint**（不取数）：宏写在 `text`、同排多扩展列、扩展块内空行、声明了边框、CSV 缺 delimiter、列宽超纸张、`#,##0.00` 配窄列。
2. **跑真流程**：`validate` → `run --export csv --export xlsx --export print-xlsx`。
3. **读产物断言**：产物是否齐全、CSV 行数是否等于预期、有没有 `${` 漏出来、分页几页几列。
4. **输出一份 JSON 结论**：`issues[]`（code/level/message/hint）+ `summary`，退出码 0/1 可供 Agent 直接判。

技能 SOP 见 [`skills/rpx-report-authoring/SKILL.md`](../skills/rpx-report-authoring/SKILL.md)。
