# 报表模型实例与混合数据源

对应润乾《报表模型》文章：https://www.raqsoft.com.cn/r/r-model 。
按文中六张示例图实现计算结构，另补行列对称、混合来源、多源关联分片综合示例、统计图与条码，
以及十份**面向客户的典型业务报表**（利润表、资产负债表、工资表、进销存台账、资金日记账、
销售发货单、应收账款账龄、收入结构占比、预算执行预警、门店组内排名）；`catalog.json` 现共 29 份，数据为本项目自行构造。
界面复用现有 Univer，打印复用现有打印预览。计算由 agent 调用本地 Rust 进程完成。

## 执行与查看

在项目根目录执行：

```sh
bash scripts/build_r_model_gallery.sh
cd web
npm run dev -- --port 5291
```

打开 `http://127.0.0.1:5291/gallery.html`。页面提供结果画布、JSON 模板定义、数据来源、独立预期校验、打印与 Excel/CSV 下载。
“在设计器中打开”会装载当前运行版本的定义及其 Excel/CSV 附件；可调整数据集、公式、主格和样式，再下载 JSON。
将修正后的定义保存回本目录对应文件，再执行上面的构建命令生成下一轮结果。

修改定义后由 agent 使用 CLI 再执行，例如：

```sh
rpx run examples/r-model/multisource-slicing.json \
  --datasources output/r-model/datasources.json \
  --out-dir /tmp/multisource-slicing-run --export xlsx --export csv
```

图库只读取静态产物。共享设计器使用 `rpx-server serve --library DIR` 连接 CLI 管理的目录库，
保存定义后由 agent 执行引擎，再刷新结果，见 [CLI 使用](../../docs/agent-cli.md)。

## 覆盖范围

| 文中模型 | 模板 | 真实输入 | 主要校验 |
| --- | --- | --- | --- |
| 纵向分片 | `vertical-slices.json` | CSV 销售 + Excel 产品、人员 | 地区 1200/2400/3600/175；总额 7375；产品动态列合计 |
| 横向分片 | `horizontal-slices.json` | 同上 | 行列及主格方向交换，金额与纵向分片一致 |
| 按段不完全分组 | `incomplete-groups.json` | CSV 库存 + Excel 价格段 | 左闭右开区间，20/50/80/100/150 边界；1000001 元进入无上限最后段；负价格不分组 |
| 重叠分组 | `overlapping-groups.json` | CSV 贷款 + Excel 自定义分组 | 山东 2900，其中青岛 1400；重点北京+山东 3900；原始合计 8900，不良余额 4500 |
| 同比环比 | `period-comparison.json` | CSV 12 季度合同 | 上一期、去年同季、跨年和逐期累计；无基期及零分母提示 |
| 动态格间运算 | `dynamic-cell-calculation.json` | CSV 电信业务 + Excel 业务、全市口径 | 8 地区及各业务排名、动态列组、前三名金额和占比 |
| 行列对称 | `symmetric-crosstab.json` | CSV 销售 + Excel 区域、产品、预算 | 独立横纵主格，保留无业务区域，缺预算时不除零 |
| 混合数据源 | `mixed-data-sources.json` | 两个 SQLite 库 + Excel 预算 + CSV 退款 | 组织库负责人、经营库收入、预算 4500、退款 100.5、净收入 3399.5、毛利 1449.5 |

| 多源分片综合 | `multisource-slicing.json` | 两个 SQLite 库 + Excel 人员/产品 + CSV 月预算 | 日期两层交叉、横纵分片、`in`、条件汇总、显式根、九张订单整片复制、浏览冻结；销售 3650、运费 187、应收 3837 |

纵向与横向分片的“其中”行、贷款的重叠分组都重复引用事实，独立合计直接汇总原始明细。
同比与环比采用文中本期/基期的比值口径，例如 1.5 显示 150%，不是增长率 50%。
价格段 `unbounded=true` 表示无上限，其余段按 `[low, high)` 筛选。

## 典型业务报表（客户演示用）

上面九份偏向引擎特性说明；这一组是客户日常真正会要的表，形态覆盖财务报表、人事、
仓储、资金、销售与应收，每份都在 A4 一页内、可直接打印交付。

| 报表 | 模板 | 数据 | 演示的能力 |
| --- | --- | --- | --- |
| 利润表 · 本期与上期 | `income-statement.json` | `income-statement.csv`（11 个科目两期金额） | 多级科目缩进、两期对比、变动率；营业利润/利润总额/净利润由格引用逐级汇总，净利润本期 499,200 |
| 资产负债表 · 左右双栏 | `balance-sheet.json` | `balance-sheet.csv`（资产 9 项、负债权益 9 项） | 一页两栏并排、每栏期末/年初两列、左右独立汇总与平衡校验差额 0 |
| 员工工资表 · 部门分组 | `payroll.json` | `payroll.csv`（3 部门 6 人） | 部门纵向分组自动合并、明细逐行、应发/实发为计算列、合计 68,650 |
| 库存进销存台账 | `stock-ledger.json` | `stock-ledger.csv`（6 个物料） | 期末 = 期初 + 入库 − 出库 的行内表达式、数量与金额混排、合计行 |
| 资金日记账 · 余额结转 | `cash-journal.json` | `cash-journal.csv`（8 笔流水） | 余额逐行结转 `E4[-1]`、期初值走宏 `openingBalance`、本期合计 |
| 销售发货单 · 一单多行 | `sales-order.json` | `sales-order.csv`（6 行明细） | 单据抬头、明细扩展、金额×单价、13% 增值税与价税合计 78,688.68、签收栏 |
| 应收账款账龄分析 | `ar-aging.json` | `ar-aging.csv`（6 个客户） | 账龄区间条件汇总（当前客户 + 天数区间两个条件）、占比列、横向合计自校验 |
| 收入结构 · 占比与帕累托 | `pareto-contribution.json` | `revenue-channel.csv`（6 渠道） | 根坐标取总额做分母、累计列走位移坐标、降序排名与 Top3 标注；累计占比到末行正好 100% |
| 部门预算执行 · 差异与预警 | `budget-variance.json` | `budget-plan.csv`（5 部门） | 差异带符号、执行率分档（达标/关注/预警）、累计实际逐行累加、合计行整体判定 |
| 门店销售 · 组内排名与占比 | `store-ranking.json` | `store-sales.csv`（3 大区 8 门店） | 大区 `mergeSame` 纵向合并、根坐标把集合收窄到同一大区做组内排名与组内占比、合计行回到全体 |

三份新例子把「自由格间运算」一组从 3 份扩到 6 份，覆盖管理分析最常见的三个口径问题：
占比怎么算、差异怎么判、组内怎么比。

三点实现约定，抄模板时直接照用：

* **明细列跟随扩展源**：同一行里只有一列声明 `extend_mode=vertical`（明细驱动格），
  其余列写 `extend_mode=none` + `left_head` 指向该驱动格；把多列都设成 `vertical`
  会让扩展相乘（实测 6 行明细炸成 798 行）。
* **计算列优先写数据集表达式**（`pay.select(base+bonus)`），不要在扩展块里写
  `C4+D4` —— 后者在扩展上下文里是「整列集合相加」，引擎显式报 `UnsupportedExpr`。
* **扩展块后面的行要紧贴**：明细行与合计行之间不能留空模板行，否则空行会被算进块内、
  合计被推到更下方。
* `border` 目前在「JSON 定义 → 快照 / xlsx」链路上没有落地（实测 xlsx `borders` 只有
  默认定义），模板里因此不写边框，视觉层次靠表头与合计行的底色区分。

## 来源与定义

`sources.xlsx` 含多张独立工作表：产品、人员、价格段、贷款分组、区域、预算、业务、全市口径。
`slicing.xlsx` 提供人员与产品两个工作表，`slicing-budget.csv` 提供乱序年月预算。
`sales.csv`、`inventory.csv`、`loans.csv`、`contracts.csv`、`telecom.csv`、`refunds.csv` 均为逗号 CSV。
退款样例包含小数、引号内逗号、引号内换行；Excel 工作表可与另一数据集共用一个上传文件。

```json
{
  "datasets": [
    { "name": "organization", "sql": { "dsn": "organization-db", "sql": "SELECT region, owner FROM organization ORDER BY display_order" } },
    { "name": "actuals", "sql": { "dsn": "operations-db", "sql": "SELECT region, revenue, cost FROM ledger" } },
    { "name": "budget", "file": { "file_name": "sources.xlsx", "sheet_name": "预算" } },
    { "name": "refunds", "file": { "file_name": "refunds.csv", "encoding": "UTF-8", "delimiter": "," } }
  ]
}
```

此片段只展示数据集声明；完整公式、扩展和主格关系以对应模板文件为准。
例如混合表按组织库的区域驱动纵向主格，再用 `sum(budget.select(budget,region==A4))` 等公式关联不同来源。
数据库 URL 位于执行端 `output/r-model/datasources.json`，定义仅存连接别名。准备脚本从两个 `.sql` 文件重建专用示例数据库。
驱动支持 SQLite、PostgreSQL、MySQL；本例和新增测试实际验证 SQLite，外部数据库需要各自连接环境。
Excel 数据公式使用文件缓存结果，报表表达式由 Rust 引擎计算，两者分别管理。

## Agent 产物与验证

`scripts/build_r_model_gallery.sh` 调用 `rpx-runtime` 的 `report_gallery` 示例命令：

```sh
cargo run --locked -p rpx-runtime --example report_gallery -- \
  examples/r-model web/public/report-gallery output/r-model/datasources.json
```

每次创建独立 `run-<时间戳>/<报表ID>/` 目录，一次计算保存：

- `template.json` 与 Excel/CSV 输入附件；
- `result.json`：原始值、类型、结果坐标和模板出处；
- `workbook.json`：可直接装载的 Univer 快照；
- `report.xlsx`、`report.csv`：同一次计算的导出；
- `run.json`：来源、尺寸、独立预期校验与成功或失败状态。

最后发布 `catalog.json`。生成失败时命令返回非零，索引和界面展示具体错误，保留既有运行目录。
这里提供批量执行与展示入口；完整模板仓库 CRUD CLI 仍按 `docs/agent-cli-design.md` 设计，尚未全部实现。

`catalog.json` 的预期独立来自输入数据，执行器逐个模板原格比较扩展结果。
回归测试还会改退款金额，确认旧预期失败；增加第四种电信业务，确认自动新增两列、重算排名及前三名。综合模板还检查新增年份、城市和订单后自动扩展行列，
以及空明细订单不串用前单小计、订单级运费不被明细重复累计。

```sh
cargo test --locked -p rpx-ds --test file_sources
cargo test --locked -p rpx-server --test mixed_sources --test multisource_slicing
```

实现参考成熟交叉表的独立数据集、行列分组及指标约定：
https://github.com/Jaspersoft/jasperreports/tree/master/demo/samples/crosstabs ，
https://github.com/Jaspersoft/jasperreports/tree/master/demo/samples/table 。
文件解析复用 `calamine` 和 `csv`，计算引擎不引入文件或数据库驱动依赖。

## 多源关联分片综合例子

`multisource-slicing.json` 使用七个逻辑数据集：经营库的销售事实、订单、明细，组织库的运输商，
Excel 的人员和产品，CSV 的月预算。模板 15×9 扩展为 55×14，校验目录包含 32 组独立预期。

上部以年→月和地区→城市组成两层交叉，右侧产品类别是独立横片；中部职务→人员是第二个纵片，
仍与城市和类别关联。条件统计使用严格 `>300`，专门检查金额等于 300 时不计入。
底部按订单复制抬头、表头、明细和小计；101 有两条明细，109 没有明细，运费分别只计一次。

`left_head` / `top_head` 的 `` `0 `` 明确终止自动上溯，使静态总计不落入相邻分片。
前四行标题/表头与前两列左表头通过原生 Univer `freeze` 固定，跟随实际展开和分页投影。
输入数据与期望自行构造，教程学习及实现边界见 `docs/report-multisource-slicing-study.md`。
