# 参数设定：润乾教程学习与项目对照

核对日期：2026-10-17。本文记录教程语义、当前代码行为与后续建议；建议部分尚未实现。

## 1. 阅读范围与结论

已阅读[参数章节](http://d.raqsoft.com.cn:6999/report/tutorial/cs24.html)及其六个小节：

| 官方教程 | 本项目需要掌握的规则 |
| --- | --- |
| [参数概念](http://d.raqsoft.com.cn:6999/report/tutorial/csdgn25.html) | 用参数改变筛选条件，使同一模板生成不同结果 |
| [参数用法](http://d.raqsoft.com.cn:6999/report/tutorial/csdsy26.html) | 单元格引用、SQL 按顺序绑定、宏替换各有不同职责 |
| [参数模板设计](http://d.raqsoft.com.cn:6999/report/tutorial/csmbsj30.html) | 控件绑定参数名；下拉框区分存储值与显示文字 |
| [发布参数模板](http://d.raqsoft.com.cn:6999/report/tutorial/jbbhcsmbjhfb34.html) | 参数表单与结果报表关联；`_arg.rpx` 是示例应用约定 |
| [动态参数](http://d.raqsoft.com.cn:6999/report/tutorial/dtcs35.html) | 动态参数通过表达式求值，先于取数与报表计算 |
| [临时变量](http://d.raqsoft.com.cn:6999/report/tutorial/lsbl39.html) | 计算过程中的可变中间值，与一次运行的输入参数分开 |

参数能力需要同时覆盖**模板声明、运行输入、参数求值、数据集绑定和表单展示**。
仅保存名称与默认值，或只增加几个输入框，都不能完成教程中的闭环。

## 2. 教程的关键语义

### 2.1 参数声明与参数值

声明说明名称、类型、初值、说明及参数类别；运行时再提供本次的实际值。
Agent 修改声明是在修改模板；人输入一个筛选值是在发起一次运行，两种操作需要分开保存。

普通参数由外部输入。动态参数由表达式计算，不能把“每次运行需要填写”解释为动态参数。
教程中的周报只让用户输入一个日期，再由两个动态参数计算该周的起止日期。
教程没有为本项目现有的 `attribute` 类别提供足够定义，不能把它并入动态参数处理。

### 2.2 单元格参数、SQL 参数和宏

| 方式 | 含义 | 例子 |
| --- | --- | --- |
| `@gender` | 表达式读取一个参数值 | `ds1.select(NAME,GENDER==@gender)` |
| SQL 的 `?` | 数据库驱动绑定一个值 | `WHERE gender = ?`，绑定列表为 `[@gender]` |
| `${condition}` | 将文本替换进定义或表达式 | 改变查询条件片段或计算表达式 |

教程也允许裸参数名，并说明引用不区分大小写。项目当前按大小写精确匹配，存在语义差异。
Agent 编写表达式时宜使用 `@`，避免与字段或格号重名。

SQL 参数按占位符出现顺序逐项绑定，绑定项也可以是表达式。同一参数出现两次，就需要两项绑定。
例如以下是根据教程规则编写的查询示意：

```sql
SELECT employee_name, bonus
FROM employees
WHERE (gender = ? OR ? IS NULL)
  AND (bonus > ? OR ? IS NULL)
```

对应绑定顺序为 `[@gender, @gender, @min_bonus, @min_bonus]`。
允许不填写的条件可以用 `null` 代表不限制；`null`、空字符串、未提供和数值 `0` 必须有明确区别。
必填、可空、清空控件后的取值，需要成为声明的一部分。

宏会改变表达式结构；SQL 值绑定只提供值。这两条通道应保留独立语义。
本项目目前的 `${}` 来自独立的 `macros` 表，不能假定运行 `params` 会自动进入宏表。

### 2.3 参数表单

教程以文本、下拉框和日期控件收集参数。控件通过 WEB 变量名绑定报表参数，布局坐标不是参数名。
下拉框例如显示“男/女”，实际提交对应代码；结果筛选用代码，不能把显示文字误当代码传给数据库。

表单与报表应显式关联。教程说明自建应用可以自行命名参数模板，因此无需在本项目引入
通过文件后缀猜测关联关系的机制。

### 2.4 动态参数的执行时机

教程示例用 `pdate@w(arg1)` 和 `pdate@we(arg1)` 从输入日期产生周起止日期。
这两个结果同时被 SQL 条件和报表标题使用，所以必须在查询之前完成计算。
动态结果应在结果页可查看，在运行输入表单中作为计算值展示。

实现日期例子前，还须核实 `pdate` 的周起始日、日期与时间边界以及数据库比较规则。
教程原查询使用严格大于/小于，不能未经验证就把它解释为另一种闭合区间。

### 2.5 临时变量

教程允许给临时变量赋值、在其他单元格引用并重复赋值，也可以在一个表达式中依次完成
中间计算与最终取值，用来减少只存放过渡值的隐藏列。
它涉及运行上下文、计算顺序和变量作用域，需要单独实现，不能用修改输入参数代替。

## 3. 当前代码实际支持到哪里

以下结论来自调用链及现有测试，未将“解析器认识语法”当成“引擎已能执行”。

| 能力 | 现状 | 代码依据 |
| --- | --- | --- |
| JSON 参数声明 | 已支持 `name/data_type/default/description/kind`，`default` 目前只能是字符串 | [params.rs](../crates/rpx-server/src/reportdef/params.rs) |
| 参数声明保存、读取 | 可进入 RPX 资产并由 `/meta` 读回 | [params_http.rs](../crates/rpx-server/tests/params_http.rs) |
| 运行参数传入公式 | 已支持 `@name`，注册后的裸名也可读取；在建表达式树时绑定值 | [atom.rs](../crates/rpx-engine/src/expr/atom.rs) |
| 参数名大小写 | 当前精确匹配，与教程有差异 | [params.rs](../crates/rpx-engine/src/params.rs) |
| 参数值类型 | HTTP 接收数值、布尔、字符串、null、数组；拒绝对象 | [form.rs](../crates/rpx-server/src/form.rs) |
| 按声明校验类型 | 当前 HTTP 主要按 JSON 值转换，没有统一按声明校验、必填校验 | [form.rs](../crates/rpx-server/src/form.rs) |
| 默认值 | Web 预填后再提交；引擎没有自动合并模板默认值，直接运行时不能依赖默认值生效 | [report.ts](../web/src/report.ts)、[run.rs](../crates/rpx-engine/src/calc/run.rs) |
| 动态参数 | 保存 `kind=dynamic` 的元数据，但没有在取数前计算参数表达式 | [http.rs](../crates/rpx-server/src/http.rs)、[run.rs](../crates/rpx-engine/src/calc/run.rs) |
| SQL 值绑定 | 尚未接通；预取没有接收运行参数，查询调用仅传 SQL 文本 | [http.rs](../crates/rpx-server/src/http.rs)、[sql_source.rs](../crates/rpx-ds/src/sql_source.rs) |
| 宏替换 | 已有独立宏表和表达式、SQL 文本替换；与运行参数不是同一命名空间 | [macros.rs](../crates/rpx-engine/src/macros.rs) |
| 日期参数 | JSON 参数类型枚举尚未开放日期；字符串形态的日期不等于完整日期语义 | [params.rs](../crates/rpx-server/src/reportdef/params.rs) |
| 参数控件 | 现有面板为名称/值文本行，没有教程中的类型化控件和代码/显示值映射 | [panes.tsx](../web/src/panes.tsx) |
| 参数声明可视化编辑 | 参数面板修改的是运行值，不会直接修改、保存模板 `params[]` | [App.tsx](../web/src/App.tsx) |
| 临时变量 | 能解析部分 `=`、逗号结构，但赋值与顺序求值运算当前显式报不支持 | [ops.rs](../crates/rpx-engine/src/expr/eval/ops.rs) |

两处需要在后续实施中一起纠正的现有表述：

- `panes.tsx` 将非静态参数提示为运行时必须提供，还将动态与属性类别合并标注。
- `reportdef/params.rs` 关于静态默认值自动参与计算的注释，超出了当前引擎入口的实际能力。

Web 还会根据文本外观推断数字：例如 `"001"` 会变成 `1`，`"false"` 仍是字符串。
后续必须按声明类型提交，保留业务编码的前导零。

当前可运行的参数声明片段：

```json
{
  "params": [
    { "name": "region", "data_type": "string", "default": "001", "kind": "static" },
    { "name": "min_bonus", "data_type": "int", "default": "200", "kind": "static" }
  ]
}
```

它是完整模板中的片段；当前直接调用运行接口时，仍应显式传入参数值：

```json
{ "region": "001", "min_bonus": 200 }
```

HTTP 可以保留这里的字符串编码；当前基础 Web 文本面板不能保证同样的类型区分。

## 4. 成熟实现参考

已查阅 JasperReports 的 GitHub 源码与官方查询示例：

- [Query sample](https://github.com/Jaspersoft/jasperreports/blob/master/demo/samples/query/README.md)：
  `$P{}` 绑定查询值，`$P!{}` 替换查询文本，`$X{}` 处理列表、null 等条件。
  可借鉴的是区分值与语法片段，不需要照搬整套占位符语言。
- [JRParameter](https://github.com/Jaspersoft/jasperreports/blob/master/core/src/main/java/net/sf/jasperreports/engine/JRParameter.java)：
  参数含类型、提示属性和默认值表达式；默认表达式只引用此前声明的参数。
  顺序声明可以作为本项目第一版动态参数依赖规则，暂不引入依赖图调度器。

项目已经依赖 `sqlx`、`serde_json`，应先使用已有的类型化绑定与 JSON 处理能力。
不同数据库的占位符与日期绑定方式需要按现有驱动接口核实，不能只用字符串替换 `?`。

## 5. 面向 Agent 与人的建议契约（尚未实现）

模板 JSON 保存参数声明、动态表达式、数据集绑定和表单元数据。
Agent 提交运行参数 JSON；人通过 Web 使用相同声明生成的表单。
参数校验与动态求值由共享 Rust 运行层负责，Web 只编辑、提交和展示。

下面仅示意目标字段，是**尚未实现的模板片段，不能作为当前可执行定义使用**：

```json
{
  "params": [
    { "name": "day", "data_type": "date", "kind": "static", "required": true },
    { "name": "gender", "data_type": "int", "kind": "static", "nullable": true, "default": null },
    { "name": "week_start", "data_type": "date", "kind": "dynamic", "expression": "pdate@w(@day)" },
    { "name": "week_end", "data_type": "date", "kind": "dynamic", "expression": "pdate@we(@day)" }
  ],
  "datasets": [
    {
      "name": "orders",
      "sql": {
        "dsn": "sales-db",
        "sql": "SELECT order_id, ordered_at FROM orders WHERE ordered_at > ? AND ordered_at < ?",
        "bindings": ["@week_start", "@week_end"]
      }
    }
  ],
  "parameter_form": {
    "fields": [
      { "param": "day", "label": "查询日期", "control": "date" },
      {
        "param": "gender", "label": "性别", "control": "select",
        "options": [
          { "value": null, "label": "全部" },
          { "value": 1, "label": "男" },
          { "value": 2, "label": "女" }
        ]
      }
    ]
  }
}
```

示意数据集只演示周起止参数绑定，`gender` 演示表单值映射；最终示例需补齐实际筛选条件。
`default` 若改为 JSON 原生值，应同时调整定义解析与 Web 编辑，采用一套明确规则。
动态表达式用独立 `expression` 字段，不再复用默认值文本表达两个不同含义。

建议执行顺序：

```mermaid
flowchart LR
  A[读取模板与运行输入] --> B[按声明合并默认值并校验类型]
  B --> C[按声明顺序计算动态参数]
  C --> D[展开显式宏并求 SQL 绑定值]
  D --> E[从数据库、Excel、CSV 取数]
  E --> F[计算主格扩展、公式与分页]
  F --> G[保存参数快照、结果与展示快照]
```

动态参数第一版只允许引用已完成的参数，禁止引用报表结果单元格或等待取数的数据集；
外部输入不能覆盖动态结果。错误应带参数名、表达式和声明位置。
缺失字段才考虑默认值，显式传入 null 则按可空规则处理。
未来支持表单选项数据集时，需要另行规定它与主报表取数的先后关系。

Excel、CSV 与数据库共享同一参数值表。SQL 可以在取数时筛选；文件来源可以先读取，
再在数据集表达式或单元格表达式中筛选，无需把文件伪装成数据库。
每次运行应记录输入值和最终计算值，供 Agent 排查、人检查及重复运行。

## 6. 建议实施顺序与验收

| 顺序 | 完成内容 | 端到端验收 |
| --- | --- | --- |
| 1 | 共享参数解析：类型、默认值、null、必填、重名检查 | 相同输入从 Agent 与 Web 运行，参数值和结果一致；`"001"` 保持字符串 |
| 2 | 数据集有序绑定，复用已有 `sqlx` | 两个数据库使用同一运行参数；重复 `?`、null、引号文本正确 |
| 3 | 动态表达式及日期能力 | 只输入一个日期，计算周边界，SQL 与标题使用相同结果；覆盖跨年和边界记录 |
| 4 | 参数声明编辑与表单生成 | 在 Web 修改类型、控件及选项后保存模板；Agent 再读、再运行得到相同定义 |
| 5 | 宏入口的明确关联、临时变量 | 宏与值绑定各有用例；变量赋值、跨格引用和扩展作用域单独验证 |

先做一个“周订单报表”贯通声明、输入、动态日期、SQL 绑定和可见结果，再增加其他控件。
实际 Agent CLI 仍以 [CLI 设计文档](agent-cli-design.md) 的实施状态为准，本文未新增 CLI 命令。

本次核验执行了已有测试：

- `cargo test -p rpx-engine param -- --nocapture`：13 项通过，其中引擎单测 12 项、夹具参数键检查 1 项。
- `cargo test -p rpx-server --test params_http`：3 项通过，覆盖参数声明入库与元数据读回。
- 在 `web/` 执行 `npm test -- src/report.test.ts`：25 项通过。

这些测试确认已有能力，没有验证本文提出的日期、动态求值或 SQL 参数绑定。
