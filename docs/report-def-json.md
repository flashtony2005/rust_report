# 契约 — 报表定义 JSON（M7-1）v1

> 状态：**M7-1 落地基线**（2026-10-16）。实现位置
> `crates/rpx-runtime/src/reportdef/`（`parse.rs` = 逐字段校验、`build.rs` = 装配）。
> v1 格式标记（`format` / `format_version` / `meta` / `extensions`）见
> `docs/format/rpx-report-json-v1.md`；当前 reader 已接受，writer 仍写 v0 形态。
> 上游：M1–M6 已闭合；本契约是「**前端定报表格式**」这条路的入口。
> 与 `.rpx` 的关系：**两条入口，一个出口**（§1）——JSON 定义与 `.rpx` 字节都产出
> 同一个 `ReportDefine`，下游主链路**完全不分叉**。
> ✅ **`.rpx` 字节已可产出**（2026-10-16 补，M7-2）：写侧序列化器
> `rpx_core::to_bytes` 对 37 个真实夹具**逐字节复现**（§7）。

## 0. 本文件的可信度声明

* 字段表（§3）与校验纪律（§6）**逐条对应实现**，不是设计意向：每条错误消息都是
  实现里真实存在的一行（`parse.rs` 的 `err(...)` 调用点）。
* 端到端判据（JSON 定义 → 快照 / xlsx）在 `crates/rpx-server/tests/reportdef_http.rs`，
  含**故障注入**证明（见 §6.3）。
* 与产品的对拍真值**不存在**：产品没有「JSON 定义」这个格式（它是本项目的扩展），
  故本契约是**自定契约**，与 `docs/spec-m6.md` §2.3 的 CSV 同性质。

## 1. 两条入口，一个出口

`open_report`（`reportdef.rs`）按**内容**分派，**不猜**：

| 输入 | 判定 | 路径 |
| --- | --- | --- |
| `.rpx` 字节 | 前 4 字节 = `RQQR` | `rpx_core::open_bytes`（既有读侧，零改动） |
| 定义 JSON | 首个非空白字节 = `{` | `parse::build`（本契约） |
| 两者都不是 | —— | `SourceError::UnknownFormat`（显式 400，**不静默当 .rpx 解**） |

两条入口汇合后，`/render` 与 `/export`（`docs/spec-m3.md` §3.2 主链路 ②–⑨）走的是
**同一份** `load_datasets_full → calculate_with → paginate_with_setup`，
出口才分家（快照 JSON vs 文件字节）。所以「JSON 定义能不能出报表」等价于
「`.rpx` 能不能出报表」——**能力面完全一致**。

## 2. 顶层结构

```jsonc
{
  "version": 1,                       // 可选；给了就必须是 1（未知版本不猜）
  "grid":    { "rows": 4, "cols": 2 },// 必填；逻辑行/列，均 ≥1
  "cells":   [ /* §3.2 */ ],          // 可选
  "datasets":[ /* §3.3 */ ],          // 可选
  "row_props":[ /* §3.4 */ ],         // 可选
  "col_props":[ /* §3.4 */ ]          // 可选
}
```

未知的**顶层键**被拒绝（`extensions` 是唯一扩展位）；**已知键的
非法取值一律显式报错**（§6）——「宽松接受 + 静默忽略」正是本仓反复踩过的失败形态。

## 3. 字段表

### 3.1 `grid`（必填）

| 字段 | 类型 | 必填 | 语义 |
| --- | --- | --- | --- |
| `rows` | 整数 ≥1 | ✔ | **逻辑**行数（前端口径） |
| `cols` | 整数 ≥1 | ✔ | **逻辑**列数 |

⛔ **坐标口径**：`grid` 与全部 `cells`/`row_props`/`col_props` 都用 **1 起的逻辑坐标**。
内部矩阵是**逻辑尺寸 + 1**（0 号是脚手架行/列，`Builder::new` 内部加一）——
快照输出（`docs/spec-m4.md` §4.2）与 xlsx 读回则是 **0 起**。三个坐标系的分工见 §4.1。

### 3.2 `cells[]`（可选）

| 字段 | 类型 | 必填 | 语义 |
| --- | --- | --- | --- |
| `row` | 整数 | ✔ | 行号（1 起，1..=`grid.rows`） |
| `col` | 整数 | ✔ | 列号（1 起，1..=`grid.cols`） |
| `text` | 字符串 | ✖ | **静态文本**；与 `expr` 互斥 |
| `expr` | 字符串 | ✖ | **值表达式**（引擎表达式语言，如 `ds1.select(region)`） |
| `h_align` | 枚举 | ✖ | 水平对齐（§4.3） |
| `v_align` | 枚举 | ✖ | 垂直对齐（§4.3） |
| `font_name` | 字符串 | ✖ | 字体名（缺省 `宋体`，即 `def::DEFAULT_FONT_NAME`；§4.5） |
| `font_size` | 整数 | ✖ | 字号（1..=32767，缺省 `12`；§4.5） |
| `bold` | 布尔 | ✖ | 粗体（缺省关；§4.5） |
| `italic` | 布尔 | ✖ | 斜体（缺省关；§4.5） |
| `underline` | 布尔 | ✖ | 下划线（缺省关；§4.5） |
| `fore_color` | 颜色 | ✖ | 前景色 `"#RRGGBB"`（缺省 `#000000`；§4.5） |
| `back_color` | 颜色 | ✖ | 背景色 `"#RRGGBB"`（缺省 `#FFFFFF`；§4.5） |
| `border` | 对象 | ✖ | 四边边框 `{left?,right?,top?,bottom?}`（§4.7） |
| `format` | 字符串 | ✖ | 数字格式串（如 `"#,##0.00"`；缺省无格式） |
| `indent` | 数字 | ✖ | 缩进（≥0，缺省 `0`；§4.7） |
| `rotation` | 数字 | ✖ | 旋转角，度（≥0，缺省 `0`；§4.7） |
| `extend_mode` | 枚举 | ✖ | 扩展模式（§4.7） |
| `left_head` | 字符串 | ✖ | **声明式左主格**：格号或根记法 `` `0 ``（§4.8） |
| `top_head` | 字符串 | ✖ | **声明式上主格**：格号或根记法 `` `0 ``（§4.8） |
| `merge` | 对象 | ✖ | 合并矩形 `{rows, cols}`（§4.2） |
| `merge_same` | 枚举 | ✖ | 同值合并（§4.2） |
| `hyperlink` | 字符串 | ✖ | 单元格超链接 URL；空字符串拒绝 |
| `hyperlink_window` | 字符串 | ✖ | 打开目标（如 `_blank`） |
| `notes` | 字符串 | ✖ | 单元格备注/批注文本 |
| `diagonal` | 对象 | ✖ | 对角线 `{style, width?, color?}`；style 同边框线型 |

* **样式字段逐字段独立**：给了才改，不给不动 —— 「只声明 `bold`」不会把字号 /
  颜色打回默认。落地实现 `crates/rpx-format/src/reportdef/style.rs`（7 槽）与
  `crates/rpx-format/src/reportdef/cellprops.rs`（18 槽）。
* 字体/字号/粗斜下划线/前景背景色 7 个字段与 `/render` 快照的 `cellData[r][c].s`
  （Univer `IStyleData`）**严格互逆**（映射表 `docs/spec-m4.md` §4.3）。
* ⚠ **边框 / `format` / `indent` / `rotation` 不在快照里**（`spec-m4.md` §4.3 列为
  后置）⇒ 它们的判据走**模型往返**而非快照读回（§4.7）。

* `text` 与 `expr` 都缺 ⇒ **空白格**（合法：它可以是纯合并锚点或占位）。
* `text` 与 `expr` **都给** ⇒ 报错（不猜哪个优先）。
* 一个 `cells[]` 元素若**落在另一个元素的合并矩形内**（非锚点槽）：
  * 它**是空壳**（只有 `row`/`col`）⇒ **容忍并跳过**（Univer 导出会把矩形内每个槽
    都列出来，这是自然形态）；
  * 它**带 `text`/`expr`** ⇒ **报错**（否则内容会被静默丢弃）。
* **声明顺序无关**：非锚点槽可以先于锚点出现（实现是两趟：先解析 + 收集全部矩形，
  再落盘）。

### 3.3 `datasets[]`（可选）

每个元素给 **`sql` 或 `file` 之一**（都给 / 都不给都报错）：

| 字段 | 类型 | 必填 | 语义 |
| --- | --- | --- | --- |
| `name` | 字符串 | ✖ | **数据集名**——表达式里的 `ds1.select(...)` 用的就是它 |
| `sql` | 对象 | ✖ | `{ "dsn": "…", "sql": "SELECT …" }` |
| `file` | 对象 | ✖ | `{ "file_name": "…", "encoding": "UTF-8", "delimiter": ",", "sheet_name": "预算" }`；后三项可选 |

* `name` 缺省时，文件数据集退化为**文件名**，SQL 数据集退化为 `<unnamed-sql>`
  （`rpx-engine/src/dataset.rs::load_datasets_full` 的口径）。
* 同一份报表可以声明任意多个文件数据集及 SQL 数据集，所有有效 `name` 必须唯一；重名会使运行失败。
* Excel：按文件扩展名识别 `.xlsx/.xls/.xlsm/.xlsb/.ods`，由 `calamine` 读取。
  `file.sheet_name` 选择工作表，缺省使用第一张。同一工作簿可声明为多个数据集，分别选择工作表。
  工作表已用区域的首行作为列名，列名不得为空或重复；保留数字、布尔、文本和空值，日期转换为 ISO 文本。
  公式只读取文件内缓存的值，不执行 Excel 公式；无缓存公式和 Excel 错误值会明确报错。
* CSV/TSV：`file.encoding` 缺省 `GBK`，可显式给 `UTF-8`/`UTF8`；
  `file.delimiter` 缺省 TAB（`"\t"`），标准逗号 CSV 显式给 `","`。
  分隔符必须是一个 ASCII 字符，且不能是引号或换行；支持引号内逗号、双引号转义和换行。
  首行列名必须非空且唯一，每行字段数必须一致；空字段为 null，数字及 true/false 按标量识别。
  文本编号如需保留前导零，使用 Excel 文本列；CSV 当前会将数字文本推断为数字。
* `file.file_name` 必须与 multipart 上传的 `dataset` 字段文件名**逐字一致**
  （`docs/spec-m3.md` §3.2 主链路 ①）。
* SQL 数据集的连接由 server 编排层注入（`--datasources`），模板只存 `dsn` 引用。
  不同 SQL 数据集可以引用不同 `dsn`，无需将多库数据预先搬进同一个数据库。

完整混合来源模板见 `examples/r-model/mixed-data-sources.json`，执行和展示方式见 `examples/r-model/README.md`。

### 3.4 `row_props[]` / `col_props[]`（可选）

| 字段 | 类型 | 必填 | 语义 |
| --- | --- | --- | --- |
| `row_props[].row` | 整数 | ✔ | 行号（1 起，1..=`grid.rows`） |
| `row_props[].height` | 数字 \| 字符串 | ✖ | 行高；**数字 → 字面量**，字符串 → 当**表达式**原样透传 |
| `row_props[].break_page` | 布尔 \| 字符串 | ✖ | 断页；`true`→`"1==1"`、`false`→`"1==0"`，字符串 → 原样透传 |
| `col_props[].col` | 整数 | ✔ | 列号（1 起，1..=`grid.cols`） |
| `col_props[].width` | 数字 \| 字符串 | ✖ | 列宽；口径同行高 |

### 3.5 `params[]`（可选，2026-10-16 补，M7-4）

参数声明。落点是 `ReportDefine.params`（`rpx_core::param::Param` 的 5 个语义槽）；
`GET /reports/{id}/meta` 会把清单读回给前端生成参数表单（§4.6）。

| 字段 | 类型 | 必填 | 语义 |
| --- | --- | --- | --- |
| `name` | 字符串 | ✔ | 参数名（表达式里引用的就是它） |
| `data_type` | 枚举 | ✔ | 参数型 → `Types2.DT_*` 码（§4.6 的表） |
| `default` | 字符串 | ✖ | 默认值（→ `value` 槽） |
| `description` | 字符串 | ✖ | 描述（前端表单的说明文字） |
| `kind` | 枚举 | ✖ | `static`（默认）/ `dynamic` / `attribute` → `PARAM_*` 码（§4.6） |

* `name` **必填**：`param_entry` 会丢弃无名参数，一条静默消失的声明比一个 400
  更难查 ⇒ 这里先拦。
* `data_type` **必填**，无默认型。静默当成 `string` 正是「不猜」要禁的形态。
* `default` **必须是字符串**：`.rpx` 里参数值是 String，`1.0` 的文本形态有歧义，
  而这条值会原样进 `/meta` 的 `default_val`。
* `params` 缺省或 `[]` ⇒ 空清单（合法，不是错误）。

### 3.6 `print_setup`（可选，2026-10-16 补，M7-6）

顶层对象，逐字段覆盖 `PrintSetup`（33 槽，`docs/format-contract.md` 附件 D）。
**只开放语义明确的子集**（§4.7 给了「为什么不开放其余」的理由）：

| 字段 | 类型 | 缺省 | 备注 |
| --- | --- | --- | --- |
| `paper` | `A4` / `A3` / `A5` / `Letter` | 不设 | 命名纸张；设置后 `paper_width/height` 自动填值 |
| `orientation` | `portrait` / `landscape` | `portrait` | 命名方向；只交换宽高，不写裸 `orient` 码 |
| `paper_width` | 数字 | 210 | 毫米；**横向版 = 与 `paper_height` 互换** |
| `paper_height` | 数字 | 297 | 毫米 |
| `left_margin` | 数字 | 19 | 毫米，≥0 |
| `right_margin` | 数字 | 19 | 毫米，≥0 |
| `top_margin` | 数字 | 25 | 毫米，≥0 |
| `bottom_margin` | 数字 | 25 | 毫米，≥0 |
| `pager_style` | `none` / `rows` / `paper` | `paper` | 不分页 / 按行 / 按纸 |
| `row_num_per_page` | 正整数或 `=表达式` 字符串 | 20 | `rows` 模式的每栏数据行数；表达式在根上下文求值 |
| `copy_count` | 整数 | 1 | 打印份数 |
| `zoom_scale` | 整数 | 100 | 缩放百分比 |
| `table_column_num` | 整数 | 1 | 分栏数 |
| `layout_rows` | 整数 | 1 | 版面行数 |
| `layout_cols` | 整数 | 1 | 版面列数 |
| `back_graph_printed` | 布尔 | false | 打印背景图 |
| `column_repeated_group_header` | 布尔 | false | 重复列分组头 |
| `full_screen_preview` | 布尔 | false | 全屏预览 |
| `blank_line_filled` | 布尔 | false | 空行填充 |
| `virtual_printer` | 字符串 | null | 虚拟打印机名 |

* `print_setup` 缺省或 `{}` ⇒ **全部走契约默认值**（合法）。
* ⚠ **未知字段与「已知但不开放」的字段一律 400**：`paper` / `orient` /
  `h_align` / `v_align` / `zoom_mode` / `layout` / 各 `*_mode`
  给了会**显式报错**，不会静默接受后丢弃 —— 后者正是「前端以为设了、实际没生效」
  的失败形态（理由见 §4.7）。

### 3.7 `macros`（可选，2026-10-17 补，M7-8）

| JSON | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `macros` | 对象 | ✖ | 宏名 → **字符串**值。缺省 `{}` |

**形态是对象，不是数组** —— 与 `params[]` **刻意不同**：参数有 5 个槽
（类型/默认值/说明…）非数组装不下；宏只有"名字"和"值"两样，而值就是
「没有数据类型的字符串」（`报表引擎设计.md` §二 #7）⇒ 对象才是它的自然形状。
形状跟着语义走，不是不一致。

值的类型**必须是字符串**：`{"n": 3}` 显式 400，**不偷偷 `to_string`** ——
与 `params[].default` 同一条纪律（`3.0` / `true` 的文本形态有歧义，让写的人
自己决定）。语义见 §4.9。

## 4. 语义与构造纪律

### 4.1 三个坐标系

| 坐标系 | 起算 | 用在哪 |
| --- | --- | --- |
| **逻辑坐标**（1 起） | 1 | 本契约的 `grid`/`cells`/`row_props`/`col_props` |
| **矩阵坐标**（1 起，含 0 号脚手架） | 1（0 号不用） | `ReportDefine` 内部（`Builder` 的 `put`/`merge`） |
| **输出坐标**（0 起） | 0 | 快照 `cellData`/`mergeData`/`rowData`/`columnData`；xlsx |

`Builder::new(logical_rows, logical_cols)` 内部建 `(logical+1) × (logical+1)` 的矩阵。
**前端只面对逻辑坐标**，加一是实现细节。

### 4.2 合并（`merge` / `merge_same`）

两类合并**机制不同**，不可混为一谈：

* **`merge`（静态矩形合并）**——设计期就确定的版式（标题跨列、多级表头）。
  * 只能声明在**锚点**（矩形左上角）；矩形内其余槽必须留空（§3.2 的空壳规则）。
  * `1×1` 合并**无意义**（Excel 也不允许单格合并）⇒ 报错。
  * 超出网格的矩形 ⇒ 报错（不裁剪）。
  * 落法：锚点格的 `merged_area`；引擎（`stage_a::seed_body`）替其余槽铺满。
    ⚠ `rd.merges` 仅供 CLI 展示，**不是**引擎的合并来源。
* **`merge_same`（同值合并）**——计算期把**相邻等值**的格并成一格，中式报表的
  「合并相同项」标准形态。

  | 取值 | 码 | 语义 |
  | --- | --- | --- |
  | `vertical` | 98 | 纵向（相邻行等值 ⇒ 并） |
  | `horizontal` | 99 | 横向 |
  | `both` | 100 | 双向 |

  判据用 `merge_same_mode = 118`（`getSource()`，保持默认）：扩展出的同列格同源 ⇒
  相邻等值即可并。缺省（不声明）= `97`（关）。

  ⚠ **分组格的跨度不来自 `merge_same`**：`extend_mode: vertical` 的 `group()` 格
  天然占满它生成的那一块（**引擎跨度**，`CellSource::row_merge`），**不声明
  `merge_same` 也照样合并**。实测（2026-10-18）：交叉表的大区 / 门店列去掉
  `merge_same` 后跨度一字不变；真正吃 `merge_same` 的只有**明细列**（无分组、
  相邻等值）。把两者混为一谈会写出假绿判据 —— 判据见
  `crates/rpx-runtime/tests/crosstab_merge.rs`（第三条自带反证：摘掉 `merge_same`
  必须一格都不并）。

### 4.3 对齐码

枚举 → 契约整数码（出处 `docs/format-contract.md` 附件 B.3 注 3）：

| `h_align` | 码 | | `v_align` | 码 |
| --- | --- | --- | --- | --- |
| `left` | −48 | | `top` | −32 |
| `center` | −47 | | `middle` | −31 |
| `right` | −46 | | `bottom` | −30 |
| `justify` | −45 | | | |
| `distribute` | −44 | | | |

`v_align` 的枚举名是 `middle`（**不是** `center`——`h_align` 与 `v_align` 的同名取值
故意不同名，避免「写了个 `center` 不知道指的是哪个轴」）。

### 4.4 属性表达式落在哪

`ReportDefine` 里属性不是独立字段，而是 **RowCell/ColCell/Cell 的 `exp_map` 槽**：

| 属性 | 宿主 | `exp_map` 键 |
| --- | --- | --- |
| 行高 | RowCell | 1 |
| 断页 | RowCell | 13 |
| 列宽 | ColCell | 11 |
| 值表达式 | Cell | 40 |

本契约把「默认值一律取自 `rpx_core::props::def`」收敛到 `build.rs` 一处，
避免每个调用点各抄 40+ 个字段、各错一处。

### 4.5 样式字段（2026-10-16 补，M7-3）

7 个字段直接落到 `Cell` 的对应槽，其余 42 个槽仍走 `build::blank_cell` 的契约默认
（附件 B.3）：

| JSON | `Cell` 槽 | 缺省 | 越界/非法 |
| --- | --- | --- | --- |
| `font_name` | `font_name: Option<String>` | `宋体` | — |
| `font_size` | `font_size: i16` | `12` | 0 或 >32767 ⇒ 400（格式的 i16 上限） |
| `bold` | `bit_flags` bit8 | 关 | 非布尔 ⇒ 400 |
| `italic` | `bit_flags` bit9 | 关 | 同上 |
| `underline` | `bit_flags` bit10 | 关 | 同上 |
| `fore_color` | `fore_color: i32` | `#000000` | 非 `"#RRGGBB"` ⇒ 400 |
| `back_color` | `back_color: i32` | `#FFFFFF` | 同上 |

三条约定：

1. **颜色只认 6 位十六进制**（大小写均可，必须带 `#`）。这**正是** `/render` 快照
   回吐的形态（`snapshot::style::argb_hex` 取低 24 位）—— 两边严格互逆，写进去什么
   就读回什么。带 alpha 的 8 位形态是 v1 范围之外（§7.2），未知形态**显式报错不猜**。
2. **位标志可置可清**：`true` 置位、`false` 清位（默认即清）。`visible`（bit5，默认
   置位）不在本契约暴露，样式写路径不得误动它。
3. **字号的界就是格式的界**：`.rpx` 里字号是 `i16`，所以 `32767` 合法、`0` 与
   `65536` 一律拒绝 —— 不引入本契约自己的取值范围。

### 4.6 参数声明（2026-10-16 补，M7-4）

#### `data_type` → `Types2.DT_*` 码

出处 `Types2.java:22-44`（探针实测 `tests/fixtures/ProbeParamFull.java:177-186`）。
**码本身不连续**（`1/2/3/…` 后**没有 4**，直接跳到 `5`）：

| `data_type` | 码 | 常量 | | `data_type` | 码 | 常量 |
| --- | --- | --- | --- | --- | --- | --- |
| `"int"` | 1 | `DT_INT` | | `"string"` | 11 | `DT_STRING` |
| `"long"` | 2 | `DT_LONG` | | `"bool"` | 12 | `DT_BOOL` |
| `"short"` | 3 | `DT_SHORT` | | `"int[]"` | 51 | `DT_INT_ARR` |
| `"float"` | 5 | `DT_FLOAT` | | `"string[]"` | 61 | `DT_STRING_ARR` |
| `"double"` | 6 | `DT_DOUBLE` | | | | |
| `"bigdecimal"` | 7 | `DT_BIGDEC` | | | | |

枚举名取**库侧原名**而非自造缩写（`bigdecimal` 对应 `DT_BIGDEC`，不是 `decimal`）。
**未列入的类型不开放**：`DT_DATE` 等在 `.rpx` 里存在但探针未实测其转换矩阵
⇒ 宁可少给也不猜码。

#### `kind` → `Param.PARAM_*` 码

| `kind` | 码 | 常量 | `is_static`（`/meta` 派生） |
| --- | --- | --- | --- |
| `"static"`（默认） | 0 | `PARAM_NORMAL` | `true` |
| `"dynamic"` | 1 | `PARAM_DYNAMIC` | `false` |
| `"attribute"` | 2 | `PARAM_ATTRIBUTE` | `false` |

`is_static` 只是 `param_type == 0` 的派生（`rpx-ds/src/asset.rs`），
**不是独立字段**——三类参数都原样保存码位，语义不丢。

#### 验证闭环（一条链同时压解析侧与写侧）

```text
JSON params[] ──parse──▶ ReportDefine.params
     ──rpx_core::to_bytes──▶ .rpx 字节        （M7-2 写侧）
     ──AssetStore::put──▶ open_bytes ──▶ param_entries
     ──▶ report_params 表 ──▶ GET /reports/{id}/meta
```

参数清单要原样读回，就**必须**先正确写进 `.rpx` 字节、再正确读出来 ——
因此 `/meta` 的断言同时是「`params[]` 解析正确」与「`write_param_meta` 正确」的证据
（`crates/rpx-server/tests/params_http.rs`）。
与 `render_http.rs::asset_meta_lists_params`（`full.rpx`，Java 侧夹具，验**读侧**）互补。

### 4.7 单元格属性扩展与打印设置（2026-10-16 补，M7-5 / M7-6）

#### 边框（`border`）

四条边各自可选，边内三个槽：

| 字段 | 类型 | 必填 | 缺省 |
| --- | --- | --- | --- |
| `style` | 枚举 | ✔ | —— |
| `width` | 数字 | ✖ | `1.0`（≥0） |
| `color` | 颜色 | ✖ | `#000000` |

线型码表（`docs/format-contract.md:419`，`NormalCell.java:1316-1336`）：

| `style` | 码 | | `style` | 码 |
| --- | --- | --- | --- | --- |
| `"none"` | 0 | | `"solid"` | 83 |
| `"dotted"` | 81 | | `"double"` | 84 |
| `"dashed"` | 82 | | | |

`85`（'U'）**不开放**：契约自己写明它与 `81` 同为 dotted（Java 侧的重复码），
开放两个名字对应同一视觉只会让人猜「到底哪个对」。读侧 `props::normalize_style`
把 `80`（'P'）归一成 `0`，那是 Java 的写入怪癖，同样不开放。

四条边落到**三组不同名**的槽（`lb_*` / `rb_*` / `tb_*` / `bb_*`）⇒ 逐边显式赋值。
颜色复用 §4.5 的同一个解析器 ⇒ 同一套互逆保证。

#### 数字格式 / 缩进 / 旋转 / 扩展模式

| JSON | `Cell` 槽 | 类型 | 缺省 | 界 |
| --- | --- | --- | --- | --- |
| `format` | `format: Option<String>` | 字符串 | `null` | —— |
| `indent` | `indent: f32` | 数字 | `0` | ≥0 |
| `rotation` | `rotation_angle: f32` | 数字 | `0` | ≥0 |
| `extend_mode` | `extend_mode: i8` | 枚举 | `16` | 见下 |

`extend_mode` 四态（契约注 3，`NormalCell.java:656`；**18/19 的方向已由夹具实证**：
`engine_ext` 的 19 → 横向铺开、`engine_head_left` 的 18 → 纵向下推）：

| `extend_mode` | 码 | 语义 |
| --- | --- | --- |
| `"default"` | 16 | 默认/自动 |
| `"none"` | 17 | 不扩展 |
| `"vertical"` | 18 | 纵向扩展 |
| `"horizontal"` | 19 | 横向扩展 |

⚠ 横向扩展（19）会把**列结构本身放大 N 倍**（契约 §… 的扩展矩阵），
且 19 与 16/18 **同行**时不增列（竖向扩展已把该行登记为可扩展行）。

#### 声明式主格（`left_head` / `top_head`）

| JSON | `Cell` 槽 | 类型 | 缺省 | 界 |
| --- | --- | --- | --- | --- |
| `left_head` | `left_head: Option<String>` | 格号或根字符串 | `null` | `` `0 `` 或 `^[A-Za-z]{1,3}[0-9]+$`，非根格必须存在 |
| `top_head` | `top_head: Option<String>` | 格号或根字符串 | `null` | 同上 |

**语义**：把该格的左主格 / 上主格**人为指定**为给定格，**跳过引擎的自动上溯**
（`ExtCellSet._$3` 分支 B / `_$2` 分支 B），直接走**分支 A**
（`ExtCellSet.java:633-647` / `:670-684`，本仓 `rpx-engine/src/heads/find.rs`）。

- **两槽独立**：一格可以**同时**有左主格与上主格 —— 交叉表正是靠这个（行维 + 列维各一条链）。
- **教程依据**（`008_jcbb14`「组表头重复」）给的就是一条**整链**人为接起来的主格：
  `A3.左主格=B3`、`B3.左主格=A2`、`C4.左主格=B3`、`A5.左主格=A2` …… 缺一格链就断。
- **产品侧的三条硬约束**（教程 `dpkz101`「人为改变主格规则」）—— 分工见下表，
  **不是**「一律交给引擎」：

  | 约束 | 拦在哪一层 | 理由 |
  | --- | --- | --- |
  | ① 左主格必须是纵向扩展格、上主格必须是横向扩展格 | **装载层**（`reportdef/heads.rs`） | 引擎分支 A **不查方向** |
  | ② 不允许循环（A→B→C→A ⇒ 报表无法计算） | **引擎**（`heads/find.rs` 分支 A） | 产品原码即报错：`isLeftHeadOf` **自含** ⇒ 命中即成环 |
  | ③ 横向扩展格不允许有左主格、纵向扩展格不允许有上主格 | **装载层**（同上） | 同 ① |

  **为什么 ①/③ 必须由装载层拦（实测，2026-10-17）**：违反 ①/③ 的六种形态在
  引擎侧**全部静默通过**，且**渲染成功**——只是主格链与预期不同，值悄悄算错：

  ```
  [接受] ③ 自横向扩展格 + left_head        ⇒ 渲染成功（cellData 行数 2）
  [接受] ③ 自纵向扩展格 + top_head         ⇒ 渲染成功（cellData 行数 6）
  [接受] ① left_head 指向横向扩展格        ⇒ 渲染成功（cellData 行数 2）
  [接受] ① top_head 指向纵向扩展格         ⇒ 渲染成功（cellData 行数 6）
  [拒绝] ② A2.left_head=B2, B2.left_head=A2 ⇒ left head 自声明成环
  ```

  产品里拦 ①/③ 的是**设计器**（「设置无效」是设计期判定，引擎原码照单全收），
  而本契约就是本仓的设计期格式 ⇒ 校验放装载层，引擎与产品逐行同形。
  错误消息带 JSON 路径与教程出处：

  ```
  $.cells[1].left_head：本格声明为横向扩展（extend_mode="horizontal"），
  横向扩展格不允许有左主格 —— 教程《多片扩展》人为改变主格规则 ③
  $.cells[1].left_head：左主格 C1 声明为横向扩展（extend_mode="horizontal"），
  但左主格必须是纵向扩展格 —— 教程《多片扩展》人为改变主格规则 ①
  ```

  ⛔ **已声明的边界：只查「显式声明」了方向的主格**。格的**生效**方向不是声明值
  —— 声明 `16`（缺省）的格由运行期拓扑改写成 17/18/19
  （`rpx-engine/src/extend.rs::topology_default`）。所以：

  - **③ 是完备的**：一格声明了非根 `left_head` 就说明它没被改写成 19
    （拓扑给 19 的唯一条件是 `left == cell00 && top != cell00`，而声明了左主格的格
    `left != cell00`）⇒「自横向 + 左主格」只能来自**显式**声明 19，装载层必能判出；
  - **① 不完备**：主格若没声明方向（16），它的生效方向取决于运行期改写 ⇒ 装载层
    不可判。这条边界**照产品**（引擎分支 A 同样不查方向），由测试
    `an_undeclared_master_direction_is_not_checked_at_load` 钉住，**不假装已覆盖**。

**扩展行为本身**（主格扩展 ⇒ 附属格被复制、同级格被拉大）由
`crates/rpx-server/tests/multipiece.rs` 五组判据钉死，含教程 `dpkz101` 例 10.6.1 的
「多片扩展」（人为把一整片格的左主格指向同一主格）与「主格可以在右（下）边」两条原话。

⛔ **打错格号在引擎侧是静默的**：`find.rs` 分支 A 写的是 `if !head.is_none()` ——
地址解析不到格**不报错**，直接落回分支 B 的自动推导，于是「打错一个格号」与
「压根没声明」在引擎侧**完全同形**。装载层因此在**全部格落盘之后**做一趟存在性校验
（`reportdef/heads.rs` 第三趟），把这种情况变成带 JSON 路径的显式错误：

```
$.cells[4].top_head：格 Z9 不存在（网格 3×3）—— 声明式主格必须指向真实存在的格
```

**存在性口径与引擎矩阵 `_$25` 对齐**：**锚点格**与**合并延续槽**都算存在
（`stage_b/merge_alias.rs` 对延续槽有登记）。

**显式根主格**使用唯一记法 `` `0 ``：`"left_head":"`0"` / `"top_head":"`0"`。
省略字段表示自动上溯；显式根表示在这一轴停止上溯，静态合计和独立分片因此不会被邻近主格带动。
根不代表一个真实扩展格，方向约束只对非根目标生效：横向格允许左主格设根，纵向格允许上主格设根。
该值通过 JSON 装载、RPX 保存读取、引擎找主格及 Web 编辑保存完整往返。
测试见 `crates/rpx-server/tests/root_heads.rs` 与 `crates/rpx-engine/tests/root_heads.rs`。

**实测效果**（`demo_data/report_month_quarter_2026.json`）：小计表头 `C2` 声明
`"top_head": "B1"` 后，季度表头的 xlsx 合并区从 `B1:E1`（4 列，吞掉小计列）
变成 **`B1:D1`（3 列）**，与参考图一致；`verify_month_quarter.py` 16 项数值全绿。

#### 打印设置：为什么不开放全部 33 槽

附件 D 给了全部 33 槽的**类型与默认值**，但**没给码表**：`orient`、
`hAlign`、`vAlign`、`zoomMode`、`layout` 及各 `*_mode` 都是裸字节码。
`pager_style` 已按教程开放 `none` / `rows` / `paper`；打印矩阵使用下文独立的 `printing` 命名契约。
夹具实证同样只有默认值 —— `tests/fixtures/` 里出现过的只有 `"paper":9` 与
`"orientation":1`，**没有任何夹具带横向版或其他纸张码**。

⇒ 按「不猜」纪律（§6.1），裸码字段**不开放**：给 `orient` 猜一个
`"landscape" → 2` 就是在零证据下定契约。

但 **JSON 是唯一演进面**，因此提供两个语义明确的作者糖：
`paper`（A4/A3/A5/Letter）填 `paper_width/height`，`orientation` 只交换宽高，
内部置 `paper=256`；不写 `orient` 裸码。需要自定义尺寸时继续用
`paper_width/paper_height`。给了未开放字段 ⇒ **400 显式报错**，不静默丢弃。

#### 判据：模型往返（为什么不是快照读回）

`snapshot::style::style_json` 只暴露 9 个字段（`bl/it/ul/ff/fs/cl/bg/ht/vt`），
**边框 / `format` / `indent` / `rotation` / 打印设置不在其中**（`spec-m4.md` §4.3
列为后置）⇒ 端到端 HTTP 读回走不通。判据改为经**真实 `.rpx` 字节**闭合整圈：

```text
JSON ──parse──▶ ReportDefine ──to_bytes──▶ .rpx 字节 ──open_bytes──▶ ReportDefine'
```

断言 `ReportDefine' == ReportDefine`（`crates/rpx-server/tests/definition_roundtrip.rs`），
同时证明解析侧、M7-2 写侧、读侧三者槽序一致。

⚠ **往返相等抓不到「两边一致地错」**：把 `dashed` 的码从 82 改成 83，往返照样绿
（解析写 83、读侧读回 83）。所以**另有写死期望值的判据**钉住每个槽的具体取值 ——
两类判据互补，缺一不可（已用故障注入实证：码表对调 ⇒ 只有写死期望值那条红）。

### 4.9 宏（2026-10-17 补，M7-8）

**记号两个都收**，各有出处：

| 记号 | 出处 |
| --- | --- |
| `${名}` | 产品：`SQLDataSetFactory.createDataSetM` 的「宏 `${...}` 替换 → `?` 占位参数求值」（`tests/fixtures/RUNLOG.md` §34.1） |
| `%{名}` | 2010 设计文档原文「引用方法为 `%{宏变量名}`」（`报表引擎设计.md` §二 #7 / §五） |

**语义**：宏是**没有数据类型的字符串**，替换的是**宏变量所占的位置**，可放在
**任何位置**（单元格值、表达式、数据集定义、单元格属性表达式）。

**时机是契约，不是实现细节**：替换发生在**建表达式之前**
（`MacroResolver2.replaceMacros(expr, ctx)` 在 `new Expression(...)` 之前一行，
`docs/m2-engine-design.md` §13.0）⇒ 词法层根本不认识 `${`，替换晚了只会得到
一条与真实原因无关的语法错。

**四个落点**（都在"建树之前"这一条上）：

| 落点 | 位置 |
| --- | --- |
| 单元格表达式 | `rpx-engine` 阶段 B 第 1 段，`cell_role_of` 与 `parse_with` **共用一次**替换 |
| 属性表达式（行/列/格 `expMap`） | `rpx-engine` 阶段 B 的 `property_expressions` |
| 数据集 SQL 文本 | `rpx-server` 的 `prefetch_sql`（发给库的那一份） |
| 图形 JS 模板 | **不在本契约内** —— 见下方"四个易混物" |

**三条规则**（每条都有单测 + 故障注入）：

1. **递归展开**：宏值里可以再引用宏（`a = "${b}-1"`），一直替换到没有引用为止。
2. **未登记 ⇒ 显式报错**（`unknown macro: ${名}`）—— 与 `@名` 未登记报
   `unknown parameter` 同一条纪律。静默留着 `${x}` 会让它在词法层被拆成
   `$` + `{x}`，报出一个与真实原因无关的语法错。
3. **成环 ⇒ 显式报错**：引用栈检测环，最多嵌套 64 层；独立宏引用的数量不受 64 限制。

空表仍扫描引用：没有登记的 `${名}` / `%{名}` 一律报 `unknown macro`。

#### 四个易混物（记号或名字相近，语义无关）

| 机制 | 记号 | 谁处理 |
| --- | --- | --- |
| **宏** | `${名}` / `%{名}` | 本契约（建树前**文本替换**） |
| 参数 | `@名` | 建树期**内联成常量**（§4.6） |
| 宏参数 | `@@名` | 同上，且**不查** `isParamName`（`Expression.java:183-186`） |
| 第三方图形 JS 模板参数 | `${参数名}` | **图形模板生成器**（三个固定参数 `id`/`width`/`height`），与报表表达式链无关 |

⛔ 最后一行与宏**记号完全相同**但是两套东西：那是 JS 侧的插值，发生在生成 JS
的阶段。本契约的替换**不碰图形模板**。

#### RPX 保存与读取

本项目通过现有 `custom_properties` 槽保存 `rust_report.macro.<名称>`。
`build()`、`build_full()` 和 `POST /reports` 产出的 RPX 都保留这些宏；
`open_report_full()` 与引擎计算入口均读取它们。显式传入的同名宏覆盖报表保存值。
产品私有 `MacroMetaData` 仍原样保留字节，未转换为本项目宏表；这不是 Java 产品宏格式互操作承诺。

### 4.10 教程坐标与格集

```text
C2[A2:2,B2:1]       从远到近定位主格链，序号从 1 起
C2[1]              第一个实例
C2[-1]             相对当前实例向上偏移
C2[`0]{}           根下的全部实例
C2[A2]{C2>$C2}     当前 A2 组内过滤，$C2 保留外层基准
&C2                所属 C2 实例在当前父组内的序号
seq(C2)            同一模板格的全局实例序号
sum(C2*D2,B2{})    在 B2 格集上下文中汇总计算列
```

`[]` 是坐标，`{}` 是条件，不能交换。空 `{}` 的管辖范围取决于公式所在格的主格链。
条件过滤、计算列聚合和嵌套数据集选择在退出时恢复外层当前格与数据集指针。
`count`、`to`、`int`、`value`、惰性 `if` / `ifn` 支持教程的排名、累积和补空行组合。
`to` 端点为 i32，最多生成 1,048,576 项；毫秒时间戳数组请用 `list(...)`。

#### 多源分片的数据集公式

```text
sales.group(year(ordered_at);year(ordered_at):1)
sales.group(month(ordered_at);month(ordered_at):1)
products.group(category,category!=null;category:1)
sales.sum(qty*price,product_id in products.select(product_id))
sales.sum(qty*price,employee_id==B8 and product_id in products.select(product_id))
sum(C5{C5>300})
```

`in` 按值判定成员；标量右值作为单成员参与比较，左侧为集合时要求每个成员都属于右侧（子集）。
空集合不包含标量，集合内重复值不会重复计算左侧记录。数组左值的 all 子集语义（空数组为真）是本项目明确采用的约定。
`ds.sum(表达式,条件)` 先在当前数据集分片中筛选，再累加表达式；省略条件保留单参数语义。
嵌套的另一数据集选择拥有自己的字段绑定，退出后恢复原数据集当前行。
`group` 支持分号后的排序表达式及 `:1` 升序 / `:-1` 降序，也支持先过滤再分组。
组内记录保留原始行号，来源记录不连续时，附属格仍能在正确子集内统计。
本轮支持主体和排序两段；第三段分号的组后聚合排序尚未实现，显式报错。

`year` / `month` 接受合法 `yyyy-MM-dd` 日期或带时间的日期字符串，空值返回空；
支持空格或 `T` 时间分隔、1–9 位小数秒（`.` 或 `:`）、`Z` 或 `±HH:mm` 时区后缀；
非法日期、类型或选项显式报错，不自动补造日期。日期函数提取输入中的年月，不转换时区。
没有事实的 `ds.sum` 保留空值语义；需要数值 0 的交叉格应明确写 `ifn(...,0)`。
综合可运行定义见 `examples/r-model/multisource-slicing.json`。

### 4.11 教程打印契约

`row_props[].type` 可选 `page_header`、`title`、`table_header`、`data`、
`group_header`、`table_footer`、`tail_title`、`page_footer`。
`group_header` 可设 `group_level`（1..20）；`col_props[].type` 可选
`left_header`、`data`、`right_header`。这些值写入已有 RPX 行列类型槽。
行列 `visible`、`break_page` 和行 `break_column` 支持布尔或表达式。
`break_page` / `break_column` 表示该行列之后，只有按纸分页时生效。

```json
{
  "print_setup": {
    "pager_style": "rows", "row_num_per_page": "=@rows", "table_column_num": 2
  },
  "group_breaks": [{"cell": "A3", "when": "=A3=='华东'"}],
  "printing": {"column_gap": 8, "header_x": "every", "title_y": "first"}
}
```

`group_breaks` 在纵向扩展格的每个实例上下文求值，断点是组的最后明细行。
多栏先向下排、再向右排；手动分页跳到下一张纸，分栏符移到下一栏。
多栏不允许隐藏行，分栏数 1..16。表头每栏重复；组头是否跨栏重复由
`column_repeated_group_header` 决定。页眉、标题、页脚和尾标题使用整页宽度。

13 条打印规则对应如下：

| 规则 | JSON 字段与行为 |
| --- | --- |
| 空行填充 | `print_setup.blank_line_filled`，按纸分页时补足剩余高度 |
| 自动适应 | `printing.auto_size`，默认 true；图形按宿主跨度扩展行列，预览文字按宽度缩字，Excel 使用 shrink |
| 横向拆页 | `printing.horizontal_split`，默认 true |
| 纵向拆页 | `printing.vertical_split`，默认 true |
| 数字折行 | `printing.wrap_numbers`，默认 false |
| 零值空白 | `printing.zero_as_blank`，只改变页副本的显示值 |
| 空字段输出 | `printing.output_empty`，默认 false；保留空字段；`empty_value` 可指定占位文本 |
| 套打 | `print_setup.back_graph_printed`，启用页面背景图打印；配合 `pager_style:"none"` |
| 边框线宽 | `printing.border_width`，pt，默认 0；显式单元格边框优先 |
| 边框颜色 | `printing.border_color`，`#RRGGBB`，默认黑色 |
| 多栏 | `print_setup.table_column_num` 大于 1 |
| 栏距 | `printing.column_gap`，mm，默认 5 |
| 页末空隙 | `printing.end_space`：`none` / `last` / `average` / `proportional`，默认 none |

默认值是本项目契约，未声称与所有产品版本一致。

打印矩阵：`header_y`（页眉页脚）、`title_y` 可选 `first` / `every`；
`header_x`、`title_x` 可选 `first` / `every` / `first_slice` / `every_slice`。
整幅模式在每个横向页呈现完整内容；slice 模式使用该页的数据列映射。
合并标题保留整幅。`row_header_every` 默认 true、`row_footer_every` 默认 false，
`column_header_every` / `column_footer_every` 默认 true。

单元格 `stretch_when_paged` 和 `split` 是布尔值，写入已有 RPX 位标志。
整幅区带优先伸缩指定列，没有指定时调整最右列；固定部分比页宽还大时按比例缩放。
`split:false` 的连续合并块整体移页，超大块独占一页；`split:true` 允许跨页拆分。
预览提供镜像打印和浏览器打印/PDF。`POST /export` 的 `fmt=print-xlsx`
按同一引擎页映射导出，每页一张工作表。

### 4.12 图形与 ECharts 模板

```json
{
  "row": 3, "col": 5, "left_head": "A3", "data_type": "chart",
  "chart": {
    "type": "column", "title": "=A3", "categories": "=B3{}",
    "series": [{"name": "金额", "values": "=C3{}"}],
    "width": 320, "height": 180,
    "appearance": {"colors": ["#4D6B88"], "legend": "bottom"}
  }
}
```

每个实际宿主实例分别绑定主格/数据集作用域，隐藏格仍可作为绑定数据。
分类和数值绑定以 `=` 开头；标题、系列名是普通文本，也可用 `=` 表达式。
系列长度必须与分类数一致；饼图与仪表盘仅接受一个系列，饼图不接受负数。

28 种 `chart.type`：

```text
column column_3d clustered_column_3d stacked_column stacked_column_3d
pie pie_3d line curve line_3d area area_3d bar clustered_bar_3d
stacked_bar stacked_bar_3d scatter column_line dual_line time_series
state_timeline gantt gauge gauge_3d radar milestone error_bar range
```

`gantt` / `state_timeline` / `milestone` 改用 `tasks:{categories,starts,ends,states}`
表达式绑定，不能同时声明 `categories` / `series`；里程碑可省 ends。
时间使用有限毫秒时间戳或浏览器能解析的日期字符串，开始不得晚于结束。
时间字符串有效性在浏览器呈现时校验；数值时间倒序在引擎报错。
`error_bar` / `range` 使用两个系列分别作为下界/上界，可带第三个中心值系列。

`appearance` 可选 `colors`、`legend`（top/bottom/left/right/none）、`background`、
`margins`（上右下左 px）、`pie_rotation`、`pie_depth`。
三维图使用固定斜投影 SVG 几何，饼图有挤出侧面；支持打印，不提供交互旋转。

页面背景用顶层 `background_image`；格背景用 `data_type:"cell_background"`；
图片内容用 `data_type:"image"`。后二者使用 `image:{source,width,height}`，source
可为图片 data URI 或 `=表达式`。支持 PNG/JPEG/GIF/BMP，验证实际图片字节。

`data_type:"echarts"` 使用 `echarts:{template,params,width,height}`：template
为 option JSON 文本，`${参数名}` 用表达式结果的 JSON 字面量单次替换。
`data_type:"echarts_js"` 使用同一配置，但 template 是 JS 函数体，可 `return option`
或调用传入的 `chart`、`container`、`width`、`height`。固定参数 `${id}`（chart）、
`${width}`、`${height}` 不能被用户覆盖。JS 在 sandbox iframe 中运行，本地 ECharts
运行时支持 formatter 函数，执行错误在预览显示。

宏、打印扩展和图形定义使用各自命名空间的 RPX 自定义属性，不改产品私有图形记录。
图形集示例在 `examples/tutorial/chart-gallery.json`。原生打印 Excel 支持常见二维图；
自定义模板、三维图、任务/区间图和打印背景使用浏览器 PDF，Excel 出口明确返回错误。

## 5. 完整示例

一个「区域销售明细」表：标题跨列合并 + 明细行同值纵向合并 + 合计聚合。

```json
{
  "version": 1,
  "grid": { "rows": 4, "cols": 2 },
  "cells": [
    { "row": 1, "col": 1, "text": "区域销售明细", "merge": { "rows": 1, "cols": 2 } },
    { "row": 2, "col": 1, "text": "区域", "h_align": "center" },
    { "row": 2, "col": 2, "text": "数量", "h_align": "center" },
    { "row": 3, "col": 1, "expr": "ds1.select(region)", "merge_same": "vertical" },
    { "row": 3, "col": 2, "expr": "ds1.select(qty)" },
    { "row": 4, "col": 1, "text": "合计", "h_align": "right" },
    { "row": 4, "col": 2, "expr": "ds1.sum(qty)" }
  ],
  "row_props": [ { "row": 1, "height": 30 } ],
  "col_props": [ { "col": 1, "width": 20 }, { "col": 2, "width": 20 } ],
  "datasets": [
    { "name": "ds1", "file": { "file_name": "engine_samemerge.csv", "encoding": "UTF-8" } }
  ]
}
```

配数据集 `engine_samemerge.csv`（TAB 分隔，5 行：`A/A/B/B/C`），渲染结果：

* **明细扩展**：`ds1.select` 把第 3 行扩成 5 行；
* **同值合并**：`A` 两行并成 `A3:A4`、`B` 两行并成 `A5:A6`、`C` 单行不并；
* **合计**：`ds1.sum(qty)` = 10+20+30+40+50 = **150**；
* **快照 `mergeData`**（0 起）：`[A1:B1, A3:A4, A5:A6]`；
* **xlsx 读回**：同三个合并区，延续槽为空、锚点带值。

端到端断言见 `crates/rpx-server/tests/reportdef_http.rs`。

### 5.1 怎么送到服务端

`/render` 与 `/export` 都吃同一个 multipart 表单（`docs/spec-m3.md` §3.2）：

| 字段 | 内容 |
| --- | --- |
| `report` | **JSON 定义字节**（文件名任意，如 `def.json`；`open_report` 按内容识别） |
| `dataset` | 数据集 Excel / CSV / TSV（`filename` 必须等于 `file.file_name`） |
| `params` | 参数 JSON（本契约 v1 不支持参数**声明**，见 §7；但引擎既有参数机制不受影响） |

资产库入口 `POST /reports`（见 `docs/spec-m3.md` 的 M3-04 行）另有一处差别：
上传物**先归一化**为规范 `.rpx` 再入库 —— 已是 `.rpx`（`RQQR` 魔数）**原样存**，
JSON 定义经 `parse::build → rpx_core::to_bytes` 转成 `.rpx`，两者都不是则 400
（`无法识别的报表格式`，同 `/render`）。故 `reports.rpx` 里**永远**是 `.rpx` 字节，
`GET /reports/{id}/meta` 与「按 id 渲染」都只有一条读侧路径（§7.1）。

## 6. 校验纪律（错误面）

### 6.1 两条纪律

1. **不猜**：未知的 `version`、未知的枚举取值、越界的行列号、退化的合并矩形
   ——一律**显式报错**，不做「宽松接受 + 忽略」。
2. **不静默丢内容**：落在他人合并矩形内的格，空壳跳过、带内容报错（§3.2）。

### 6.2 错误消息与 HTTP 映射

`DefError` 两种形态：`Json`（JSON 语法错）/ `Schema { path, detail }`（结构/取值错，
`path` 是 JSON 路径如 `$.cells[3].merge.rows`）。经 `SourceError::Def` 映射为：

| 情况 | HTTP | 消息前缀 |
| --- | --- | --- |
| JSON 语法错 / 结构错 | 400 | `报表定义不合法：<path>：<detail>` |
| `.rpx` 字节坏 | 400 | `模板解析失败：…` |
| 两者都不是 | 400 | `无法识别的报表格式：…` |

错误消息**本身就是契约的一部分**（前端据此定位到具体字段），故实现里每条消息都
带 JSON 路径与可选值清单。

### 6.3 判据的有效性（故障注入）

「一个固定报 N 条噪声的检查等于没有检查」（用户原话）。本契约的判据已用多组注入证明：

1. 把 `parse.rs` 的 `"vertical" → 98` 改成 `97`（同值合并失效）⇒
   `reportdef_http.rs` 的两个合并断言**变红**（只剩标题一个矩形），单元层
   `minimal_definition_lands_all_fields` 也红；改回 ⇒ 绿。
2. 把 `open_report` 的 JSON 分支短路（`&& false`）⇒ `reportdef_http.rs` **4/5 变红**
   （只剩「两者都不是」那条，符合预期）；改回 ⇒ 绿。
3. `params.rs`：把 `DT_INT`/`DT_LONG` 码对调 ⇒ 端到端 `/meta` 红（`2` ≠ `1`）。
   **同时暴露了一个同义反复的判据**：原本「遍历 `DATA_TYPES` 再断言表等于自己」
   的码表单测**照样绿** ⇒ 已改为写死期望码表（§4.7 的教训：期望值必须独立写死）。
4. `cellprops.rs`：① 把 `dashed`/`solid` 码对调 ⇒ **只有写死期望值那条红**，
   往返相等那条**照样绿**（两边一致地错）⇒ 证明两类判据互补、缺一不可；
   ② 把「右边」写进「左边」的槽 ⇒ 往返与写死期望值**都红**（槽位错位）。
5. `printsetup.rs`：把左右边距槽对调 ⇒ 单元与往返**都红**。

## 7. 边界

### 7.1 已具备：`.rpx` 字节的**产出**（2026-10-16 补，M7-2）

写侧序列化器 **`rpx_core::to_bytes`** 已落地（`crates/rpx-core/src/writer/`），
契约依据 `docs/format-contract.md` 附件 A（记录层）/ C.2（载荷 24 步）/ C.5（文件层）。
判据两条，都对 **37 个真实 `.rpx` 夹具**成立（`crates/rpx-core/tests/roundtrip.rs`）：

* **模型往返**：`open(to_bytes(rd)) == rd`（写侧不得丢模型信息）；
* **字节往返**：`to_bytes(open(original)) == original`（**逐字节复现**）。

⇒ 定义 JSON 现在既能直接渲染/导出，也能**落成 `.rpx` 文件与 Java 侧互通**。
`POST /reports` 已按此把 JSON 定义**归一化**为 `.rpx` 入库（§5.1），
资产库的唯一事实源因此仍是 `.rpx`，不引入双写。

### 7.2 未支持（v1 范围之外）

* **打印设置的裸码字段**（`orient` / `h_align` / `v_align` /
  `zoom_mode` / `layout` / 各 `*_mode`）—— 契约附件 D 未给码表
  ⇒ 不设命名枚举；`paper`/`orientation` 是语义作者糖，映射到宽高，不写裸码。
* 超链接、批注、`cell_graph`（图表）、子报表、`to_excel_mode` / `adjust_size_mode` /
  对角线（`diagonal_*`）、`extend_area` 显式声明。
  （单元格样式 7 字段 = M7-3 §3.2/§4.5；**参数声明** = M7-4 §3.5/§4.6；
  **边框 / 数字格式 / 缩进 / 旋转 / `extend_mode`** = M7-5 §3.2/§4.7；
  **打印设置（语义明确子集）** = M7-6 §3.6/§4.7；
  **声明式主格 `left_head`/`top_head`** = M7-7 §3.2/§4.8；
  **主格/附属格 的扩展与三条硬约束** = M7-9 §4.8。）
* 产品私有 `cell_graph` 记录的语义互操作；本项目图形走 §4.12 的独立契约。
* `pno()` / `pcount()`、页统计区与 Java 字体测量布局的逐像素对拍。
* 子报表、报表组、钻取、折叠、流式大报表属于教程的其他章节，本轮不扩展这些子系统。

## 8. 变更记录

| 日期 | 变更 |
| --- | --- |
| 2026-10-16 | v1 立稿：字段表（§3）/ 语义（§4）/ 示例（§5）/ 错误面（§6）/ 边界（§7）。实现 `crates/rpx-server/src/reportdef/`。 |
| 2026-10-16 | §7 拆分：写侧序列化器落地（M7-2），`.rpx` 产出从「缺口」改为「已具备」；`POST /reports` 增加 JSON → `.rpx` 归一化。 |
| 2026-10-16 | M7-3 单元格样式：§3.2 加 7 个样式字段、新增 §4.5（语义与越界）、§7.2 把「字体/颜色」从未支持列表移除。实现 `reportdef/style.rs`。 |
| 2026-10-16 | M7-4 参数声明：新增 §3.5 `params[]` 字段表、§4.6（`DT_*` 码表 + `kind` 三态 + 验证闭环）、§7.2 把「参数声明」从未支持列表移除。实现 `reportdef/params.rs`。 |
| 2026-10-16 | M7-5 单元格属性扩展：§3.2 加 `border`/`format`/`indent`/`rotation`/`extend_mode`、§4.7 加边框线型码表与「模型往返」判据、§7.2 把「边框/数字格式/缩进/旋转/extend_mode」从未支持列表移除。实现 `reportdef/cellprops.rs`。 |
| 2026-10-16 | M7-6 打印设置覆盖：新增 §3.6 `print_setup`（17 个语义明确字段）、§4.7 加「为什么不开放全部 33 槽」的理由、§7.2 把未开放字段改为「码表未给 ⇒ 不猜」。实现 `reportdef/printsetup.rs`。 |
| 2026-10-17 | JSON-only 策略：新增 `paper` / `orientation` 作者糖；裸码字段仍不开放。 |
| 2026-10-17 | M7-7 声明式主格：§3.2 加 `left_head`/`top_head`、新增 §4.8（语义 + 三条产品约束 + 存在性校验口径 + `` `0 `` 为何不开）、§7.2 登记。实现 `reportdef/cellprops.rs` 的 `parse_cell_address` 与 `reportdef/parse.rs` 的**第三趟存在性校验**（把引擎分支 A 的静默落空变成显式错误）。实测：季度表头合并区 `B1:E1` → `B1:D1`。 |
| 2026-10-17 | M7-9 主格/附属格 的扩展闭合：§4.8 把「三条硬约束」从「不代引擎校验」改写为**分工表**（② 引擎拦、①③ 装载层拦），并新增**实测**证据（六种违反形态原本全部静默通过）与**已声明边界**（只查显式声明的方向）。实现 `reportdef/heads.rs`（第三趟校验迁入 + ①③ 判定）。新增端到端判据 `tests/multipiece.rs` 五组（多片扩展 / 对照组拉大 / 缺省右附属格 / 主格在右下 / 三条约束），含教程 `dpkz101` 例 10.6.1 与「主格可以在右（下）边」两条原话。判据：10 条单测 + 5 条集成 + 5 条故障注入。 |
| 2026-10-17 | M7-8 宏：新增 §3.7 `macros`（对象形态 + 值必须字符串）、§4.9（两个记号 `${}`/`%{}` 的出处 + 替换时机是契约 + 四个落点 + 三条规则 + 四个易混物 + **不落 .rpx 的已声明边界**）。实现 `rpx-engine/src/macros.rs`（纯函数 `substitute`）、`stage_b` 两个建树点的替换、`reportdef/macros.rs` 与 `open_report_full`；服务面 `calculate_full` + `prefetch_sql` 的 SQL 文本替换。判据：16 条单测 + 6 条故障注入全过。 |
