# 多源关联分片：教程语义与当前支持范围

核对日期：2026-10-17。

结论：本轮已补齐此前列出的六项缺口：集合成员运算、带条件数据集汇总、日期分组与排序、
显式根主格、浏览冻结和同一模板的综合验收。沿用当前 JSON 定义与 Rust 执行链。

## 1. 阅读范围

已阅读[多源分片章节](http://d.raqsoft.com.cn:6999/report/tutorial/dycp92.html)及全部七个小节：

- [多层交叉报表](http://d.raqsoft.com.cn:6999/report/tutorial/dcjcbb93.html)
- [纵向分片](http://d.raqsoft.com.cn:6999/report/tutorial/zxfp97.html)
- [横向分片](http://d.raqsoft.com.cn:6999/report/tutorial/hxfp98.html)
- [扩展区域与静态区域混合](http://d.raqsoft.com.cn:6999/report/tutorial/kzqyyjtqyhh99.html)
- [行列对称](http://d.raqsoft.com.cn:6999/report/tutorial/hldc100.html)
- [多片扩展](http://d.raqsoft.com.cn:6999/report/tutorial/dpkz101.html)
- [多源关联分片的概念与特征总结](http://d.raqsoft.com.cn:6999/report/tutorial/dyglfpdgnytzzj105.html)

同时参考了 JasperReports 的 GitHub [crosstabs 示例](https://github.com/Jaspersoft/jasperreports/blob/master/demo/samples/crosstabs/README.md)：
独立数据集、嵌套行列分组、指标和汇总是成熟约定；本项目的单元格主格关系进一步承担了分片布局与复制。

## 2. 概念与特征

| 概念 | 含义 | 对应需求 |
| --- | --- | --- |
| 多源 | 同一报表使用多个物理表、文件或数据库的数据 | 订单来自数据库、人员来自 Excel、退款来自 CSV |
| 分片 | 同一网格有多个独立或嵌套扩展区域，也可插入固定区域 | 上半区按月份，下半区按销售员；右侧再按产品分类 |
| 关联 | 区域之间按业务键和主格上下文取数，并可交叉运算 | 销售员区域读取当前城市内属于该人的订单金额 |
| 多层交叉 | 行头、列头各有多层嵌套，指标同时受两条主格链约束 | 年→月 × 地区→城市 |
| 多片扩展 | 一个主格带动一整片附属格复制，片内还能继续扩展 | 每张订单复制抬头、明细表头、明细行和表尾 |
| 行列对称 | 横纵方向都具有扩展、分片及对应的表头能力 | 交换行列后计算结构成立，宽表可横向分页重复左表头 |

这些能力需要在同一张结果网格中保持格线、公式和数据联系。
不能要求所有来源先拼成一张扁平大表：不同片的粒度和统计口径可能不一致，强行连接还可能放大金额。
例如订单有三条明细时，订单级运费不能随连接后的三行重复求和。

主格决定复制与上下文，关联表达式决定按什么业务键读取另一来源。字段同名不代表自动关联。
单格自身选择一个扩展方向；交叉指标格可以同时跟随左主格与上主格，最终铺成矩形。

## 3. 当前支持矩阵

| 教程能力 | 当前结论 | 证据与边界 |
| --- | --- | --- |
| 多层交叉 | 已验证两层行头 × 两层列头 | 本次新增 `multilevel_crosstab`；同名城市、跨年重复月份、不等长子组、重复事实均不串组 |
| 纵向分片 | 已有上下独立区域与跨源计算示例 | `vertical-slices.json`：地区片、人员片共享产品列头；金额来自销售 CSV，人员来自 Excel |
| 横向分片 | 已有左右独立区域示例 | `horizontal-slices.json`；与纵向示例对应，但尚未对照教程 10.3 整表逐格验收 |
| 扩展区与静态区混合 | 已有固定标题、汇总、说明及动态片混排 | 图库模板和格集作用域测试；静态汇总要明确自己的主格与统计范围 |
| 行列对称的计算 | 新增两轴交换验证 | 交换分组轴后，交叉值矩阵严格转置；不等于所有布局、合并和样式已经自动转置 |
| 横向分页重复左表头 | 已有运行与导出验证 | `tutorial_printing` 检查左右列区带、页面清单与打印 XLSX |
| 浏览时固定表头 | 已由行列类型生成 Univer 原生冻结 | 连续前置标题/表头与左表头按扩展后的索引冻结；分页时按每页投影，隐藏行列保留索引 |
| 多片扩展 | 已验证整片跟随主格复制 | `multipiece` 区分整片复制与同级格拉大；也检查主格在右下、方向和循环约束 |
| 主子表关联 | 已有跨数据集与嵌套主子表对拍 | `cross_dataset_compare` 对十个夹具逐格、逐页核对，含多条明细和无匹配 |
| Excel、CSV、不同数据库混用 | 已有真实输入验证 | `mixed_sources` 使用 Excel、带引号 CSV、两个独立 SQLite 库；其他数据库环境未在本次验证 |

已有可编辑定义与可见结果入口见 [报表模型示例说明](../examples/r-model/README.md)。
这些示例证明各个组成能力，不代表教程 10.1–10.6 的原始 RPX 和全部公式已经完整复现。

## 4. 本次新增的两层交叉实算

[multilevel_crosstab.rs](../crates/rpx-server/tests/multilevel_crosstab.rs) 单独验证两轴结构；
本轮生产能力与综合模板在后文说明。

测试使用九条自行构造的事实：两年、三个月份、两个地区、五个“地区内城市”分组。
两个地区都包含名为 `Shared` 的城市；同一个年份月份城市还包含两条应合并统计的事实。
第二年的月份比第一年多一个，West 的城市数也比 East 多一个，避免仅验证等长分组。

独立手工预期如下；表中的空白表示没有事实，不是丢失的分组：

| 年/月 | East/Shared | East/E2 | West/Shared | West/W2 | West/W3 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 2024/1 | 15 | | 20 | | |
| 2024/2 | | 30 | | 40 | |
| 2025/1 | 50 | | | 70 | |
| 2025/2 | | | 60 | | |
| 2025/3 | | | | | 80 |

主格链在当前 JSON 中即可声明：

```json
{
  "version": 1,
  "grid": { "rows": 3, "cols": 3 },
  "cells": [
    { "row": 1, "col": 3, "expr": "facts.group(region)", "extend_mode": "horizontal" },
    { "row": 2, "col": 3, "expr": "facts.group(city)", "extend_mode": "horizontal", "top_head": "C1" },
    { "row": 3, "col": 1, "expr": "facts.group(year)", "extend_mode": "vertical" },
    { "row": 3, "col": 2, "expr": "facts.group(month)", "extend_mode": "vertical", "left_head": "A3" },
    { "row": 3, "col": 3, "expr": "facts.sum(amount)", "extend_mode": "none", "left_head": "B3", "top_head": "C2" }
  ]
}
```

上例是测试使用的模板；`facts` 由测试通过引擎接口注入。
文件/HTTP 运行还需要补充实际 `datasets` 声明和输入，不能仅上传这份定义就完成取数。
该结构测试使用现成的年份月份字段；综合模板进一步用 `year(ordered_at)` / `month(ordered_at)` 提取日期分组。

两项测试各检查两种空值策略：

- 默认空交叉显示为空；结果使用稀疏存储，不能要求空格也存在于输出 `cells` 数组。
- 模板明确写 `ifn(facts.sum(amount),0)` 后，25 个交叉位置均产生数值，空交叉为 0。

正常与转置两轴分别检查全部 25 个位置，结果网格均为 7×7。
这同时验证了双向主格链、各层分组上下文和交叉聚合。

## 5. 本轮补齐的能力

| 项目 | 实现与约定 | 验证 |
| --- | --- | --- |
| `in` | 标量成员判定、集合子集判定，标量右值视为单成员；重复成员不增加记录数 | 空集合、重复值、数字类型及组合布尔条件 |
| 条件 `ds.sum` | `ds.sum(表达式,过滤条件)` 在当前数据分片内先筛选后求和 | 城市上下文 + 人员 + 当前产品类别，嵌套查询后恢复指针 |
| 日期与分组排序 | `year/month`，`group(表达式,过滤;排序表达式:方向)` | 乱序日期、跨年重复月份、闰日、乱序且不连续的类别 |
| 显式根主格 | 唯一记法 `` `0 ``，省略字段仍表示自动推导 | JSON、RPX、Web 编辑保存往返，静态片与动态片独立 |
| 浏览冻结 | 将实际网格连续的标题/表头行及左表头列映射到 `IFreeze` | 单轴、双轴、扩展、隐藏、合并、每页局部索引 |
| 综合运行 | `multisource-slicing.json`，七个逻辑数据集、四类物理来源 | 一张表内同时验证多层交叉、横纵分片、静态统计和九张订单整片复制 |

综合模板直接运行教程形态的公式：

```text
sales.group(year(ordered_at);year(ordered_at):1)
sales.group(month(ordered_at);month(ordered_at):1)
products.group(category,category!=null;category:1)
sales.sum(qty*price,product_id in products.select(product_id))
sales.sum(qty*price,employee_id==B8 and product_id in products.select(product_id))
sum(C5{C5>300})
```

日期来自 SQL 文本，`year/month` 对合法日期取分量、对空值返回空，对非法输入显式报错。
没有增加新的日期 Value 类型或参数系统。默认空交叉保留空值；示例需要 0 的格显式使用 `ifn`。
数组左值 `in` 的 all 子集包含（空数组为真）是本项目约定；教程本例使用标量左值。
`group` 第三个分号的组后聚合排序不在本轮支持范围，显式报错；已覆盖教程所用两段排序。

教程静态区文字与公式阈值有 1000/10000 的不一致；本例明确使用严格 `>300`，输入包含金额 300，
独立预期要求排除相等边界。条件汇总在每行的城市格上求值，总计行另按总计城市格求值，不能混淆口径。

### 综合示例的独立预期

来源为经营 SQLite 库的订单与明细、组织 SQLite 库的运输商、Excel 人员/产品两工作表及 CSV 月预算。
模板 15×9 扩展为 55×14，目录中声明 32 组检查。数据库连接保留在执行端，浏览页只读保存产物。

| 年/月 | East/E2 | East/Shared | West/Shared | West/W2 | West/W3 | 销售 | CSV 预算 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2024/1 | 0 | 150 | 200 | 0 | 0 | 350 | 400 |
| 2024/2 | 300 | 0 | 0 | 400 | 0 | 700 | 600 |
| 2025/1 | 0 | 500 | 0 | 700 | 0 | 1200 | 1300 |
| 2025/2 | 0 | 0 | 600 | 0 | 0 | 600 | 500 |
| 2025/3 | 0 | 0 | 0 | 0 | 800 | 800 | 900 |
| 合计 | 300 | 650 | 800 | 1100 | 800 | 3650 | 3700 |

Hardware / Software 分片金额为 1650 / 2000。人员 1–4 金额为 1150 / 1500 / 1000 / 0，
Excel 人员预算为 1200 / 1400 / 1100 / 100，保留无业务人员。
订单 101 有两条明细、109 没有明细；各订单小计为 150 / 200 / 300 / 400 / 500 / 600 / 700 / 800 / 0。
运费只按九张订单计一次，共 187；最终应收 3837。

原始 `result.json` 按结果槽保存值，合并表头的延续槽也会出现相同原格值；
例如两年对应五个月槽，运输商的两列合并对应每单两个槽。预期按槽检查，金额按其独立统计口径核对。

集成测试还将 CSV 的 2025/3 预算改为 990，并向数据库加入 2026 年、城市 W4、员工 4 的新订单。
结果随数据扩展为 60×15；销售 4650、运费 228、应收 4878，冻结仍是前四行两列。
JSON 保存为 RPX 后通过生产渲染入口计算，快照逐格一致；另切换纸张及双轴分页验证重复表头和页内冻结。

综合验收同时修复两处问题：空子片的格集汇总不能读取前一主片；
行列几何从扩展后的轴头读取，避免合并正文的延续槽污染类型、宽高与冻结边界。

## 6. 对 Agent 全程管理的意义

沿用当前 JSON 的 `datasets`、`extend_mode`、`left_head`、`top_head` 和表达式即可表达主体结构。
模板中引用数据源别名；数据库连接留在执行端。Web 编辑同一份定义，人可以修正主格、公式与来源。

执行链为：读取多源数据 → 按主格扩展与关联计算 → 分页 → 保存结果、Univer 快照和导出。
Agent 可以从结果的模板出处追查是哪一个格生成了某片结果。
现有 `result.json` 中的主格信息是模板出处，不是唯一运行实例 ID，不能当作完整实例依赖图使用。

综合例子已加入图库；人可在结果、定义、来源和校验视图查看，也可在设计器修正后保存并重新执行。
相关命令和输入说明见 [示例 README](../examples/r-model/README.md)。
参数能力仍按 [参数设定学习文档](report-parameters-study.md) 单独规划，本轮未改参数执行语义。

## 7. 验证入口与边界

```sh
bash scripts/build_r_model_gallery.sh
cargo test --locked -p rpx-server --test multilevel_crosstab --test multisource_slicing --test mixed_sources
cargo test --locked -p rpx-server --test slicing_formulas --test root_heads
cargo test --locked -p rpx-server --lib snapshot
bash scripts/gate.sh
```

图库同时重算原有八份示例及新综合示例；所有预期来自独立构造的输入。
完整仓库检查覆盖 Rust 格式、Clippy、测试、依赖/分层/文档规则与前端构建、测试。
浏览验收使用独立 Chrome 上下文，不占用用户正在操作的浏览器。

本轮证据覆盖上述七小节的计算模型和列出的原式，数据与布局由本项目自行构造；
没有宣称逐字节复现教程原始 RPX 或自动转置所有样式、合并。实际数据库连接验证 SQLite，
PostgreSQL/MySQL 的环境和大规模数据性能不属于本轮验收。

验收产物：`output/playwright/multisource-slicing/` 包含滚动前后截图、原生冻结与滚动状态、
32 项校验、七数据集来源、打印、Excel 下载、设计器保存及生产引擎重新运行记录。
设计器往返后的销售、运费与应收保持一致，浏览器无页面异常。

最终验证：G1–G9 全绿，Rust 740 项、前端 292 项测试通过。记录见
`output/slicing-validation/gate.log`。运行中的 8090 服务已更新到本轮引擎，图库 5291 的
综合模板保存及运行通过实际 `/api/render` 代理验证。
