# 教程增量实现与验收

实现范围依据 `/Users/lushaohui/project/report/润乾报表教程-参悟.md` 第 6 节，
按显式主格 → 坐标/格集 → 宏 → 打印 → 图形的顺序接通。
本轮未把教程 91 页全部功能声明为已完成。

## 能力与验收入口

| 能力 | 最终行为 | 核心验证 |
| --- | --- | --- |
| 显式主格 | JSON 声明左右/上下主格，经 RPX 保存读取，校验格号、方向与循环；主格可在右侧/下方 | `tutorial_formulas::explicit_right_hand_head_drives_ordinal_and_copies`、定义装载与主格测试 |
| 坐标与格集 | 绝对/相对坐标、根、条件过滤、`&`、`seq`、计算列聚合；组成员按原始行号保存，交叉作用域求交 | `tutorial_formulas`、`expr::tests_coord` 与上下文恢复测试 |
| 宏 | `${}` / `%{}` 在建树前展开，覆盖表达式、属性与 SQL；引用栈检测循环；保存 RPX 不丢宏 | `reportdef_macros`、`macros_wiring_tests` |
| 打印 | 三种分页、动态行数、8 种行带/左右表头、强制分页、条件组后分页、打印矩阵、13 条规则、多栏与页投影 | `tutorial_printing`、`tutorial_print_layout`；打印 XLSX 实际读回 |
| 图形 | 28 类图按宿主实例绑定，页面/格背景、图片、JSON option 与 JS 模板；SVG/PDF 与支持的原生 Excel 图表 | `tutorial_graphics`、前端 28 类 SVG 实际渲染、浏览器 PDF |

引擎与 core 保持零第三方生产依赖。宏、命名打印选项与图形定义复用 RPX 的
`custom_properties`，没有猜测产品私有宏/图形序列化记录。
普通字体、边框、行列类型和分页标志使用已有槽位。

## 实现选择

图形复用 Apache ECharts 的 SVG 渲染和 custom series：任务图、里程碑、工字、
全距用同一套数据绑定与向量输出。三维图采用固定斜投影几何，使截图与打印输出一致。
ECharts 源码/约定参考 `apache/echarts` 的 GitHub 仓库及本项目安装包类型定义。

Excel 复用已有 `rust_xlsxwriter`，参考 `jmcnamara/rust_xlsxwriter` 的图表与打印接口。
新增 `base64` 只负责 data URI 解码；图片字节验证交给已有图片解码器。
原生图表写入隐藏数据页，测试读取实际 OOXML 图表与 drawing 条目。
依赖理由登记在 [代码审查规范](code-review.md)。

分页后的真实行列映射是预览和打印 Excel 的共同输入。
分栏用各栏几何边界并集拼接，允许不同栏有不同的行高。
整幅重复区域使用独立列映射，合并标题保留完整。
零值空白、空字段占位和页末空隙只修改页副本，原始公式数据保持原值。

JavaScript 模板在独立 sandbox iframe 执行，加载本地 ECharts 运行时；
`${参数名}` 以 JSON 字面量单次替换，支持 formatter 回调。
执行错误通过消息返回预览，报表宏替换不扫描图形模板正文。

## 可重复验证

仓库根目录运行：

```bash
bash scripts/gate.sh
python3 scripts/fault_inject_tutorial.py
```

本轮完整门禁结果：G1–G9 全绿，Rust 690 项测试通过，前端 253 项测试通过。
包含格式、Clippy 全目标零警告、工作区测试、文件长度、依赖策略、脚本、文档引用、
crate 分层与前端构建。完整输出保存在 `output/tutorial/gate.log`。

五类故障注入逐条在临时副本运行，编译失败不计为捕获，不能修改测试来制造失败：

| 层 | 注入错误 | 断言抓住的行为 |
| --- | --- | --- |
| 主格 | 装载后丢掉显式左主格 | 右侧主格驱动的附属格复制/序号错误 |
| 坐标 | 左右/上下约束的 AND 改为 OR | 学历、部门、全表的同文本格集得到错误集合 |
| 宏 | 每个宏展开结果额外加 1 | 保存/读取后 42 变为 43 |
| 分页 | 每栏行数比较方向改反 | 多栏页数和记录排列错误 |
| 图形 | 反转每个系列的数据值 | 组内数据顺序与图形绑定不一致 |

五类均完成基线绿 → 运行时断言红 → 恢复绿；脚本校验原工作区文件 SHA-256
未改变。逐步日志与机器可读结果在 `output/tutorial/faults/results.json`。

## 浏览器验收

从真实文件选择、上传、渲染进入预览，未用手造页面代替产品页面。

| 示例 | 验证结果 | 本地产物 |
| --- | --- | --- |
| 分组销售 | 2 页，各组图形只绑定本组数据，排名正确，末页合计 190 | `output/playwright/grouped-sales.png`、`grouped-sales.pdf` |
| 图形集 | 29 页，28 个实际 SVG 图；JS iframe 显示 ¥10/¥11/¥12，未产生图形错误提示 | `output/playwright/javascript-chart.png`、`chart-gallery.pdf` |
| 图片/套打 | 实际图片 decode 成功，页面和格背景均存在，进入打印 PDF | `output/playwright/images.png`、`images.pdf` |
| 两栏 | 2 页，20 条序号各出现一次，第二列值等于序号 ×10 | `output/playwright/multi-column.png`、`multi-column.pdf` |

同时修复了切换文件后仍持有旧 JSON 定义的页面状态问题。
运行说明与示例清单见 [examples/tutorial/README.md](../examples/tutorial/README.md)。

## 明确边界

- 本项目的图形、宏及命名打印扩展是独立契约；尚未与产品私有宏/图形记录互转。
- 原生 Excel 图表覆盖二维柱/堆积柱、条/堆积条、线/曲线、面积、饼、散点、雷达和双轴组合。
  三维、任务/区间、仪表、自定义 JS/option 图形与打印背景使用浏览器 PDF；打印 Excel 明确报错。
- 三维是固定投影，不提供可旋转 WebGL 场景。
- 文本自动适应采用浏览器字体测量与缩字；没有承诺与 Java 字体布局逐像素相同。
- `pno()` / `pcount()`、页统计区、子报表、报表组、钻取、折叠与流式大报表尚未实现。
  后几项不在第 6 节本轮增量的实现范围内。
- 时间字符串在浏览器渲染时验证；数值时间倒序、非法图片、非法 option、系列长度错误
  在引擎或服务端明确拒绝。
