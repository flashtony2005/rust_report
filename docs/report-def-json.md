# 契约 — 报表定义 JSON（M7-1）v1

> 状态：**M7-1 落地基线**（2026-10-16）。实现位置
> `crates/rpx-server/src/reportdef/`（`parse.rs` = 逐字段校验、`build.rs` = 装配）。
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

未知的**顶层键**被忽略（JSON 是开放结构，前端框架可能塞注释字段）；但**已知键的
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
| `merge` | 对象 | ✖ | 合并矩形 `{rows, cols}`（§4.2） |
| `merge_same` | 枚举 | ✖ | 同值合并（§4.2） |

* **样式字段逐字段独立**：给了才改，不给不动 —— 「只声明 `bold`」不会把字号 /
  颜色打回默认。落地实现 `rpx-server/src/reportdef/style.rs`（7 槽）与
  `rpx-server/src/reportdef/cellprops.rs`（16 槽）。
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
| `file` | 对象 | ✖ | `{ "file_name": "…", "encoding": "UTF-8"? }` |

* `name` 缺省时，文件数据集退化为**文件名**，SQL 数据集退化为 `<unnamed-sql>`
  （`rpx-engine/src/dataset.rs::load_datasets_full` 的口径）。
* `file.encoding` 缺省 = `GBK`；显式 `UTF-8`/`UTF8` 走 `std::str::from_utf8`。
* ⚠ **CSV 分隔符硬编码 TAB**（引擎 `parse_csv` 用 `split('\t')`）——`FileDataSetConfig`
  的 `seperator` 槽**当前不被读取**，故 `build.rs` 按引擎**实际行为**声明 TAB，
  避免模板里写着一个假值。
* `file.file_name` 必须与 multipart 上传的 `dataset` 字段文件名**逐字一致**
  （`docs/spec-m3.md` §3.2 主链路 ①）。
* SQL 数据集的连接由 server 编排层注入（`--datasources`），模板只存 `dsn` 引用。

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
| `paper_width` | 数字 | 210 | 毫米；**横向版 = 与 `paper_height` 互换** |
| `paper_height` | 数字 | 297 | 毫米 |
| `left_margin` | 数字 | 19 | 毫米，≥0 |
| `right_margin` | 数字 | 19 | 毫米，≥0 |
| `top_margin` | 数字 | 25 | 毫米，≥0 |
| `bottom_margin` | 数字 | 25 | 毫米，≥0 |
| `row_num_per_page` | 整数 | 20 | 每页行数（分页器直接用它） |
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
  `pager_style` / `h_align` / `v_align` / `zoom_mode` / `layout` / 各 `*_mode`
  给了会**显式报错**，不会静默接受后丢弃 —— 后者正是「前端以为设了、实际没生效」
  的失败形态（理由见 §4.7）。

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

#### 打印设置：为什么不开放全部 33 槽

附件 D 给了全部 33 槽的**类型与默认值**，但**没给码表**：`paper`、`orient`、
`pagerStyle`、`hAlign`、`vAlign`、`zoomMode`、`layout` 及各 `*_mode` 都是裸字节码。
夹具实证同样只有默认值 —— `tests/fixtures/` 里出现过的只有 `"paper":9` 与
`"orientation":1`，**没有任何夹具带横向版或其他纸张码**。

⇒ 按「不猜」纪律（§6.1），这些字段**不开放命名枚举**：给 `orient` 猜一个
`"landscape" → 2` 就是在零证据下定契约，而一旦猜错，前端写出的模板会在 Java 侧
**静默按错的版式渲染**。需要横竖版时**互换 `paper_width` / `paper_height`** ——
这两个是裸数值槽，语义无歧义，且分页器按它们算页宽页高。

给了未开放字段 ⇒ **400 显式报错**，不静默丢弃。

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
| `dataset` | 数据集 CSV（`filename` 必须等于 `file.file_name`） |
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

* **打印设置的码表字段**（`paper` / `orient` / `pager_style` / `h_align` / `v_align` /
  `zoom_mode` / `layout` / 各 `*_mode`）—— 契约附件 D 未给码表、夹具只有默认值
  ⇒ 按「不猜」纪律不设命名枚举（横竖版用 `paper_width`/`paper_height` 互换，§4.7）。
* 超链接、批注、`cell_graph`（图表）、子报表、`to_excel_mode` / `adjust_size_mode` /
  对角线（`diagonal_*`）、`extend_area` 显式声明。
  （单元格样式 7 字段 = M7-3 §3.2/§4.5；**参数声明** = M7-4 §3.5/§4.6；
  **边框 / 数字格式 / 缩进 / 旋转 / `extend_mode`** = M7-5 §3.2/§4.7；
  **打印设置（语义明确子集）** = M7-6 §3.6/§4.7。）
* `cell_graph`（图表）、超链接、批注、子报表；
* 在线设计器（`web/` 侧）——Univer 目前仍是**纯渲染终端**（`docs/spec-m4.md` §1），
  在它上面做「定格式」是**新阶段**。**已立 spec**：`docs/spec-m7-web.md`
  （2026-10-16）。其**准入条件是 W0（前端可验证性地基：装依赖 + vitest + 新 gate G9）**
  —— 本机 `web/` 无 `node_modules`、`package.json` 无测试运行器 ⇒ 前端改动目前
  **无机器判据**，与「判据有效性一律用故障注入证明」冲突。W0 未绿之前不进 W1/W2。

这些按同一模式加（`parse.rs` 加字段 + `build.rs` 加装配）。

## 8. 变更记录

| 日期 | 变更 |
| --- | --- |
| 2026-10-16 | v1 立稿：字段表（§3）/ 语义（§4）/ 示例（§5）/ 错误面（§6）/ 边界（§7）。实现 `crates/rpx-server/src/reportdef/`。 |
| 2026-10-16 | §7 拆分：写侧序列化器落地（M7-2），`.rpx` 产出从「缺口」改为「已具备」；`POST /reports` 增加 JSON → `.rpx` 归一化。 |
| 2026-10-16 | M7-3 单元格样式：§3.2 加 7 个样式字段、新增 §4.5（语义与越界）、§7.2 把「字体/颜色」从未支持列表移除。实现 `reportdef/style.rs`。 |
| 2026-10-16 | M7-4 参数声明：新增 §3.5 `params[]` 字段表、§4.6（`DT_*` 码表 + `kind` 三态 + 验证闭环）、§7.2 把「参数声明」从未支持列表移除。实现 `reportdef/params.rs`。 |
| 2026-10-16 | M7-5 单元格属性扩展：§3.2 加 `border`/`format`/`indent`/`rotation`/`extend_mode`、§4.7 加边框线型码表与「模型往返」判据、§7.2 把「边框/数字格式/缩进/旋转/extend_mode」从未支持列表移除。实现 `reportdef/cellprops.rs`。 |
| 2026-10-16 | M7-6 打印设置覆盖：新增 §3.6 `print_setup`（17 个语义明确字段）、§4.7 加「为什么不开放全部 33 槽」的理由、§7.2 把未开放字段改为「码表未给 ⇒ 不猜」。实现 `reportdef/printsetup.rs`。 |
