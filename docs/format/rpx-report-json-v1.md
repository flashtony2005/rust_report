# RPX Report JSON — Format v1（设计稿）

> 状态：**Phase 0 reader 已实现；Phase 1 迁移快照 + 合并延续 overlay 已实现；新 preset 已写 v1；v2 尚未实现**
> 目标：把当前“事实上的 JSON 定义”升级为**有明确 `format_version`、兼容策略和扩展位的规范格式**。
> 关联决策：`docs/adr/0001-json-canonical-format.md`
> 关联转换器：`docs/format/rpx-core-json-converter.md`
> 机器可读 schema：[rpx-report-v1.schema.json](rpx-report-v1.schema.json)（设计骨架）
> 本文不改变现有 `.rpx` 二进制格式；`.rpx` 在本设计中降级为 legacy 适配器。

---

## 1. 设计原则

1. **JSON 是规范格式，`ReportDefine` 是内部模型。**
   不把引擎改成 JSON DOM；JSON 只做边界。

2. **v1 尽量不破坏现有字段名。**
   当前 JSON 的 `grid`/`cells`/`datasets`/`params`/`row_props`/`col_props`/`print_setup`/`printing`/`group_breaks`/`macros` 保持原名，避免一次性重写 CLI、Web、测试和外部脚本。

3. **只增加版本标记、元数据和扩展位。**
   `format`、`format_version`、`schema`、`meta`、`extensions` 是 v1 的新增字段；其余字段沿用。

4. **未知字段默认拒绝，唯一例外是 `extensions`。**
   继续坚持“拼错字段不能静默生效”的纪律；可演进性通过显式扩展位解决。

5. **legacy 字段只做迁移，不做编辑面。**
   `.rpx` 中无法映射到 v1 的字段进入 `extensions["rpx.legacy"]`；编辑器应原样保留、不解释。

---

## 2. 版本语义

| 字段 | 类型 | 必填 | 语义 |
|---|---|:---:|---|
| `format` | string | 新文件必填 | 固定 `"rpx-report"` |
| `format_version` | integer | 新文件必填 | 格式版本；本设计为 `1` |
| `schema` | string | 可选 | 文档 schema 标识，如 `"rpx-report/v1"`；仅信息性，不联网解析 |
| `version` | integer | 可选（废弃） | 兼容旧文件的别名；若存在必须等于 `format_version` |

规则：

- **v0**：当前没有 `format` 的文件；reader 继续接受，内存中按 v1 语义处理。
- **v1**：带 `format: "rpx-report"` 与 `format_version: 1` 的文件。
- reader 接受 v0 和 v1；writer 默认写 v1，`--compat=v0` 可写无版本标记的旧形态。
- `format_version` 高于 reader 支持版本 → `FORMAT_VERSION_UNSUPPORTED`，不猜测、不降级。
- **发布顺序必须是 reader 先行**：旧 reader 的 `strict` 会拒绝新字段，所以不能先让 writer 写 v1。

> 不把 `format_version` 与 library 的 `revision` 混为一谈：前者是语法/语义版本，后者是内容哈希。

---

## 3. 顶层结构

```json
{
  "format": "rpx-report",
  "format_version": 1,
  "schema": "rpx-report/v1",
  "version": 1,
  "meta": {
    "title": "月度经营分析",
    "description": "区域 / 事业部 / 成员",
    "tags": ["经营分析", "月度"]
  },
  "grid": { "rows": 12, "cols": 11 },
  "cells": [],
  "row_props": [],
  "col_props": [],
  "datasets": [],
  "params": [],
  "print_setup": {},
  "printing": {},
  "group_breaks": [],
  "background_image": null,
  "macros": {},
  "extensions": {}
}
```

| 字段 | 类型 | 必填 | 说明 |
|---|---|:---:|---|
| `format` | string | 是（v1） | `"rpx-report"` |
| `format_version` | integer | 是（v1） | `1` |
| `schema` | string | 否 | `"rpx-report/v1"` |
| `version` | integer | 否 | 废弃别名，必须等于 `format_version` |
| `meta` | object | 否 | 独立文件的可读元信息；库内 manifest 优先 |
| `grid` | object | 是 | `rows`、`cols`，逻辑坐标 1 起 |
| `cells` | array | 否 | 单元格定义 |
| `row_props` | array | 否 | 行属性 |
| `col_props` | array | 否 | 列属性 |
| `datasets` | array | 否 | 文件 / SQL 数据集 |
| `params` | array | 否 | 参数声明 |
| `print_setup` | object | 否 | 打印设置（当前开放子集） |
| `printing` | object | 否 | 打印选项 |
| `group_breaks` | array | 否 | 分组断页 |
| `background_image` | string/null | 否 | 页面背景 data URI |
| `macros` | object | 否 | 字符串宏 |
| `extensions` | object | 否 | 扩展命名空间 |

### 3.1 `meta`

```json
{
  "title": "月度经营分析",
  "description": "区域 / 事业部 / 成员",
  "tags": ["经营分析", "月度"]
}
```

- 只允许 `title`/`description`/`tags`（v1）。
- 库内报告以 `manifest.json` 的 `name`/`description`/`tags` 为准；`meta` 用于独立文件与导出，不制造第二套权威。
- 不做时间戳：时间属于运行记录/library，不属于报表定义；避免每次保存 revision 变化。

### 3.2 `extensions`

```json
{
  "extensions": {
    "rpx.legacy": { "format": "rpx-rpx-v1" },
    "x-acme": { "whatever": 1 }
  }
}
```

规则：

- key 必须形如 `rpx.*` 或 `x-<vendor>`；其他 key 拒绝。
- reader 对未知的 `extensions` key **必须忽略**；writer **必须原样保留**。
- 扩展内容不得改变基础语义，除非该扩展被显式实现并写入文档。
- `rpx.legacy` 是保留键，只由转换器读写；普通编辑器/引擎忽略它。
- 每项必须是 JSON object，便于未来加版本号。

---

## 4. `cells[]`

字段沿用当前定义 JSON，但把“内容 / 样式 / 几何 / 图形 / 扩展”写清。

### 4.1 内容

| 字段 | 类型 | 必填 | 说明 |
|---|---|:---:|---|
| `row` | integer | 是 | 1 起，`1..=grid.rows` |
| `col` | integer | 是 | 1 起，`1..=grid.cols` |
| `text` | string | 否 | 静态文本；与 `expr`/`literal` 互斥 |
| `expr` | string | 否 | 引擎表达式；与 `text`/`literal` 互斥 |
| `literal` | object | 否 | `{type, value}`；与 `text`/`expr` 互斥 |
| `disp_value` | string | 否 | 显示覆盖；不改变原始值 |
| `data_type` | string | 否 | `image` / `chart` / `cell_background` / `echarts` / `echarts_js` |

`literal.type` 取值：

```text
null | text | bool | byte | short | int | long | float | double | decimal | date
```

- `long`/`decimal` 的 `value` 用字符串保存，避免 JSON number 精度损失。
- `date` 当前只保留定义；引擎不计算日期对象（执行时会明确拒绝或按文本处理）。

### 4.2 样式

| 字段 | 类型 | 说明 |
|---|---|---|
| `font_name` | string | 缺省 `宋体` |
| `font_size` | integer | `1..=32767` |
| `bold` / `italic` / `underline` | boolean | 独立开关 |
| `fore_color` / `back_color` | string | `#RRGGBB` |
| `border` | object | `{left?,right?,top?,bottom?}`，每边 `{style, width?, color?}` |
| `format` | string | 数字格式串 |
| `indent` | number | `>=0` |
| `rotation` | number | 角度 `>=0` |
| `h_align` | enum | `left` / `center` / `right` / `justify` / `distribute` |
| `v_align` | enum | `top` / `middle` / `bottom` |

`border.style` 取值：`none` / `dotted` / `dashed` / `solid` / `double`。

### 4.3 几何与扩展

| 字段 | 类型 | 说明 |
|---|---|---|
| `extend_mode` | enum | `default` / `none` / `vertical` / `horizontal` |
| `left_head` | string | 格号或显式根 `` `0 `` |
| `top_head` | string | 格号或显式根 `` `0 `` |
| `merge` | object | `{rows, cols}`；只写在锚点 |
| `merge_same` | enum | `vertical` / `horizontal` / `both` |
| `split` | boolean | 允许跨页拆分 |
| `stretch_when_paged` | boolean | 分页时拉伸 |

### 4.4 图形

| 字段 | 类型 | 说明 |
|---|---|---|
| `image` | object | `{source, width?, height?}`；`source` 为 data URI |
| `chart` | object | `{type, title?, categories?, series[]}` |

ECharts 的 `echarts`/`echarts_js` 使用 `data_type` + 顶层配置；v1 沿用现状，不在本次重命名。

### 4.5 单元格级扩展

允许 `cells[].extensions`，规则同顶层 `extensions`：

- 未知 vendor 扩展忽略并保留；
- `rpx.legacy` 不放在单元格内，统一放在顶层 `extensions["rpx.legacy"].cells[address]`，避免数组重排导致路径失效。

---

## 5. `row_props[]` / `col_props[]`

### 5.1 行

| 字段 | 类型 | 说明 |
|---|---|---|
| `row` | integer | 1 起 |
| `type` | enum | `page_header` / `title` / `table_header` / `data` / `group_header` / `table_footer` / `tail_title` / `page_footer` |
| `height` | number | 行高 |
| `visible` | boolean/string | 布尔或表达式 |
| `break_page` | boolean/string | 布尔或表达式 |
| `break_column` | boolean/string | 布尔或表达式 |
| `adjust_height` | boolean/string | 布尔或表达式 |
| `group_level` | integer | `type=group_header` 时有效 |

### 5.2 列

| 字段 | 类型 | 说明 |
|---|---|---|
| `col` | integer | 1 起 |
| `type` | enum | `left_header` / `data` / `right_header` |
| `width` | number | 列宽 |
| `visible` | boolean/string | 布尔或表达式 |
| `break_page` | boolean/string | 布尔或表达式 |
| `break_column` | boolean/string | 布尔或表达式 |

---

## 6. `datasets[]`

```json
{
  "name": "ds1",
  "file": {
    "file_name": "sales.csv",
    "encoding": "UTF-8",
    "delimiter": ",",
    "sheet_name": "预算"
  }
}
```

| 字段 | 类型 | 必填 | 说明 |
|---|---|:---:|---|
| `name` | string | 否 | 缺省：文件名 / `<unnamed-sql>` |
| `file` | object | 二选一 | `file_name` 必填；`encoding` 缺省 GBK；`delimiter` 缺省 TAB；`sheet_name` 可选 |
| `sql` | object | 二选一 | `{dsn, sql}`；凭据不进模板 |

约束：

- `file` 与 `sql` 恰好一个。
- 文件数据集只使用包内相对路径；拒绝绝对路径和 `..`。
- `delimiter` 必须是单个 ASCII 字符，不能是引号/换行。
- 数据集名唯一。

v1 不开放产品高级槽（`colList`/`title`/`quoteRemoved`/`colTypeList`/`filter`/`gather*`/`group*`/`select*`/`changeFile`/`beginRow`/`endRow`）；这些由 legacy 转换器保留。

---

## 7. `params[]`

| 字段 | 类型 | 必填 | 说明 |
|---|---|:---:|---|
| `name` | string | 是 | 唯一 |
| `data_type` | enum | 是 | `int` / `long` / `short` / `float` / `double` / `bigdecimal` / `string` / `bool` / `int[]` / `string[]` |
| `kind` | enum | 否 | `static` / `dynamic` / `attribute`，缺省 `static` |
| `default` | string | 否 | 声明元数据，不自动绑定实际值 |
| `description` | string | 否 | 说明 |

v1 不引入 `required`/`enum`/`min`/`max`；这些属于校验层，不要混进格式版本。

---

## 8. 打印

v1 保持当前顶层字段名，避免一次性重排：

- `print_setup`：当前开放字段（`zoom_scale`、`table_column_num`、`layout_rows`、`layout_cols`、`row_num_per_page`、`copy_count`、`paper_width`、`paper_height`、四边距、`back_graph_printed`、`column_repeated_group_header`、`full_screen_preview`、`blank_line_filled`、`pager_style`、`virtual_printer`）。
- `printing`：`auto_size`、`hidden` 等打印选项。
- `group_breaks`：`{cell, when}`。
- `background_image`：data URI 或 null。

`paper`/`orient`/`h_align`/`v_align`/`zoom_mode` 等产品码表字段仍**不开放**；由 `rpx.legacy.print_setup` 保留。v2 如果要开放，先补码表证据。

---

## 9. `extensions["rpx.legacy"]`

这是转换器的“无损舱”，不是用户编辑面。**Phase 1 的实现形态是原始 RPX 快照**：

```json
{
  "extensions": {
    "rpx.legacy": {
      "format": "rpx-rpx-v1",
      "encoding": "base64",
      "sha256": "…",
      "modeled_sha256": "…",
      "data": "…"
    }
  }
}
```

- `data` 是原始 `.rpx` 字节的 base64；`sha256` 是原始字节哈希。
- `modeled_sha256` 是“会进入 `ReportDefine` 的可建模字段”的规范化哈希；不包含 `meta`/`extensions`。
- `json_to_rpx` 在快照存在且 `modeled_sha256` 未变化时原样返回原字节；一旦可建模字段被编辑，返回 `LEGACY_READ_ONLY`，要求先移除快照再导出。**不会静默丢弃未建模字段。**

Phase 2 可以把原始快照升级为以下**逐字段 legacy 舱**（迁移期间也可保留给需要细粒度编辑的场景）：

```json
"rpx.legacy": {
  "format": "rpx-rpx-v1",
  "cells": { "C6": { "diagonal_style": 0, "ois_blob": "base64:…" } },
  "row_props": { "3": { "notes": null } },
  "datasets": { "ds1": { "filter": null } },
  "print_setup": { "paper": 9 }
}
```

原则：

- 用**地址/名字**（`C6`、`ds1`、行号/列号）做 key，不用数组下标，避免重排后错位。
- 可用结构化 JSON 表达的用结构化；纯字节（OIS/未知 blob）用 `"base64:…"`。
- 读侧如果发现 legacy 与显式字段冲突，报 `LEGACY_CONFLICT`，不静默选择（字段级 legacy 落地时）。

---

## 10. 校验规则

1. 顶层未知字段拒绝；`extensions` 例外。
2. `format`/`format_version` 缺失时按 v0 处理；存在时必须合法。
3. `version` 存在时必须等于 `format_version`。
4. `grid.rows/cols` 为正整数；内部矩阵上限沿用 `1_000_000` 槽。
5. `cells` 的 `row`/`col` 必须在网格内；重复坐标拒绝。
6. `text`/`expr`/`literal` 至多一个。
7. 合并矩形只允许锚点带内容；非锚点空壳跳过，带内容拒绝。
8. 颜色 `#RRGGBB`；字号/缩进/旋转有界；枚举值必须精确匹配。
9. `file`/`sql` 恰好一个；数据集名唯一；路径必须包内相对。
10. `params` 名唯一；`data_type` 必填。
11. `extensions` key 必须命名空间化；值必须是 object。
12. 任何“不支持”必须返回 `UNSUPPORTED_FIELD` + JSON Pointer，不静默丢弃。

---

## 11. 规范化与哈希

- 写入默认：UTF-8、2 空格缩进、对象 key 排序、末尾换行、`ensure_ascii=false`。
- 数字：整数不带 `.0`；浮点用最短可逆表示；`long`/`decimal` 用字符串。
- 读取不依赖 key 顺序。
- **revision**：
  - `source_revision` = 文件原始字节的 SHA-256（用于 If-Match，检测任何字节改动）。
  - `semantic_revision` = 规范化 JSON 的 SHA-256（可选；用于跨格式化比较、缓存）。
  - v1 不改变现有 library 的 `source_revision` 语义，避免升级时全部冲突。

---

## 12. 兼容矩阵

| reader \ writer | v0（无 marker） | v1（marker） |
|---|---|---|
| 旧 reader（当前） | 可读 | **拒绝**（`strict` 未知字段） |
| 新 reader（v1） | 可读，内存升级 | 可读 |
| 新 writer 默认 | — | v1 |
| 新 writer `--compat=v0` | v0 | — |

因此 Phase 1 必须先发 reader，Phase 2 才切 writer；见 ADR。

---

## 13. JSON Schema 骨架（节选）

以下是 `rpx-report-v1.schema.json` 的结构骨架，用于固定顶层字段、互斥关系和扩展位；`literal`/`chart`/`image`/`print_setup` 的深层校验由实现阶段的 schema 与语义校验共同完成。

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "rpx-report/v1",
  "type": "object",
  "required": ["format", "format_version", "grid"],
  "additionalProperties": false,
  "properties": {
    "format": { "const": "rpx-report" },
    "format_version": { "const": 1 },
    "schema": { "type": "string" },
    "version": { "const": 1 },
    "meta": { "$ref": "#/$defs/meta" },
    "grid": {
      "type": "object",
      "required": ["rows", "cols"],
      "additionalProperties": false,
      "properties": {
        "rows": { "type": "integer", "minimum": 1 },
        "cols": { "type": "integer", "minimum": 1 }
      }
    },
    "cells": { "type": "array", "items": { "$ref": "#/$defs/cell" } },
    "row_props": { "type": "array", "items": { "$ref": "#/$defs/row_prop" } },
    "col_props": { "type": "array", "items": { "$ref": "#/$defs/col_prop" } },
    "datasets": { "type": "array", "items": { "$ref": "#/$defs/dataset" } },
    "params": { "type": "array", "items": { "$ref": "#/$defs/param" } },
    "print_setup": { "type": "object" },
    "printing": { "type": "object" },
    "group_breaks": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["cell", "when"],
        "additionalProperties": false,
        "properties": {
          "cell": { "type": "string" },
          "when": { "type": "string" }
        }
      }
    },
    "background_image": { "type": ["string", "null"] },
    "macros": { "type": "object", "additionalProperties": { "type": "string" } },
    "extensions": { "$ref": "#/$defs/extensions" }
  },
  "$defs": {
    "extensions": {
      "type": "object",
      "propertyNames": { "pattern": "^(rpx\\.|x-)" },
      "additionalProperties": { "type": "object" }
    },
    "meta": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "title": { "type": "string" },
        "description": { "type": "string" },
        "tags": { "type": "array", "items": { "type": "string" } }
      }
    },
    "cell": {
      "type": "object",
      "required": ["row", "col"],
      "additionalProperties": false,
      "properties": {
        "row": { "type": "integer", "minimum": 1 },
        "col": { "type": "integer", "minimum": 1 },
        "text": { "type": "string" },
        "expr": { "type": "string" },
        "literal": {
          "type": "object",
          "required": ["type", "value"],
          "additionalProperties": false,
          "properties": { "type": { "type": "string" }, "value": {} }
        },
        "disp_value": { "type": "string" },
        "data_type": { "type": "string" },
        "h_align": { "enum": ["left", "center", "right", "justify", "distribute"] },
        "v_align": { "enum": ["top", "middle", "bottom"] },
        "font_name": { "type": "string" },
        "font_size": { "type": "integer", "minimum": 1 },
        "bold": { "type": "boolean" },
        "italic": { "type": "boolean" },
        "underline": { "type": "boolean" },
        "fore_color": { "type": "string" },
        "back_color": { "type": "string" },
        "border": { "type": "object" },
        "format": { "type": "string" },
        "indent": { "type": "number", "minimum": 0 },
        "rotation": { "type": "number", "minimum": 0 },
        "extend_mode": { "enum": ["default", "none", "vertical", "horizontal"] },
        "left_head": { "type": "string" },
        "top_head": { "type": "string" },
        "merge": {
          "type": "object",
          "required": ["rows", "cols"],
          "additionalProperties": false,
          "properties": {
            "rows": { "type": "integer", "minimum": 1 },
            "cols": { "type": "integer", "minimum": 1 }
          }
        },
        "merge_same": { "enum": ["vertical", "horizontal", "both"] },
        "split": { "type": "boolean" },
        "stretch_when_paged": { "type": "boolean" },
        "image": { "type": "object" },
        "chart": { "type": "object" },
        "extensions": { "$ref": "#/$defs/extensions" }
      }
    },
    "row_prop": {
      "type": "object",
      "required": ["row"],
      "additionalProperties": false,
      "properties": {
        "row": { "type": "integer", "minimum": 1 },
        "type": { "enum": ["page_header", "title", "table_header", "data", "group_header", "table_footer", "tail_title", "page_footer"] },
        "height": {},
        "visible": { "type": ["boolean", "string"] },
        "break_page": { "type": ["boolean", "string"] },
        "break_column": { "type": ["boolean", "string"] },
        "adjust_height": { "type": ["boolean", "string"] },
        "group_level": { "type": "integer" }
      }
    },
    "col_prop": {
      "type": "object",
      "required": ["col"],
      "additionalProperties": false,
      "properties": {
        "col": { "type": "integer", "minimum": 1 },
        "type": { "enum": ["left_header", "data", "right_header"] },
        "width": {},
        "visible": { "type": ["boolean", "string"] },
        "break_page": { "type": ["boolean", "string"] },
        "break_column": { "type": ["boolean", "string"] }
      }
    },
    "dataset": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "name": { "type": "string" },
        "file": {
          "type": "object",
          "required": ["file_name"],
          "additionalProperties": false,
          "properties": {
            "file_name": { "type": "string" },
            "encoding": { "type": "string" },
            "delimiter": { "type": "string", "minLength": 1, "maxLength": 1 },
            "sheet_name": { "type": "string" }
          }
        },
        "sql": {
          "type": "object",
          "required": ["dsn", "sql"],
          "additionalProperties": false,
          "properties": {
            "dsn": { "type": "string" },
            "sql": { "type": "string" }
          }
        }
      },
      "oneOf": [
        { "required": ["file"], "not": { "required": ["sql"] } },
        { "required": ["sql"], "not": { "required": ["file"] } }
      ]
    },
    "param": {
      "type": "object",
      "required": ["name", "data_type"],
      "additionalProperties": false,
      "properties": {
        "name": { "type": "string" },
        "data_type": { "enum": ["int", "long", "short", "float", "double", "bigdecimal", "string", "bool", "int[]", "string[]"] },
        "kind": { "enum": ["static", "dynamic", "attribute"] },
        "default": { "type": "string" },
        "description": { "type": "string" }
      }
    }
  }
}
```

> 注：`cells` 的 `text`/`expr`/`literal` 互斥、非锚点合并、坐标越界等**不能用纯 JSON Schema 表达**，仍由语义校验负责（与当前 `reportdef` 的分工一致）。

---

## 14. 明确不做的事（v1）

- 不把 `print_setup` 重命名成 `print` 对象；
- 不把 `cells` 的样式字段重组成 `style` 对象；
- 不引入规则/条件样式/校验字段（那是后续 `format_version` 或扩展）；
- 不把 `.rpx` 扩展名复用给 JSON；
- 不在 v1 承诺“任意产品 `.rpx` 逐字节往返”；只承诺 L1+L3（见转换器文档）。
