# JSON 报表设定覆盖矩阵

> 基线：`v0.11.0-json-diagonal`（2026-10-17）
> 策略：[JSON canonical / RPX frozen](../adr/0002-json-canonical-rpx-frozen.md)
> 图例：✅ 已建模；⚠️ 部分；⏳ 待定；⛔ 暂不开放（缺语义/证据）。

## 1. 顶层

| 字段 | 状态 | 说明 |
|---|:---:|---|
| `format` / `format_version` / `schema` | ✅ | v1 标记 |
| `meta` | ✅ | title/description/tags |
| `extensions` | ✅ | `rpx.*` / `x-*` 命名空间 |
| `grid` | ✅ | 逻辑 rows/cols |
| `cells` | ✅ | 见 §2 |
| `row_props` / `col_props` | ✅ | 见 §3 |
| `datasets` | ✅ | 文件 / SQL 数据集的公开槽 |
| `params` | ✅ | 参数声明 |
| `macros` | ✅ | 宏声明 |
| `print_setup` | ✅ | 见 §4 |
| `printing` | ✅ | 打印矩阵/页眉页脚位置等 |
| `group_breaks` | ✅ | `{cell, when}` |
| `background_image` | ✅ | data URI |
| `exp_map` / `scalars` | ⛔ | 产品内部尾部；legacy 快照承载 |
| `macro_meta` / `sub_report_meta` / `back_graph_config` / `export_config` | ⛔ | 未逆向的二进制/私有记录 |

## 2. `cells[]`

### 内容与几何

| 字段 | 状态 |
|---|:---:|
| `row` / `col` | ✅ |
| `text` / `expr` / `literal` | ✅ |
| `disp_value` / `data_type` | ✅ |
| `merge` / `merge_same` | ✅ |
| `left_head` / `top_head` | ✅ |
| `extend_mode` | ✅ |

### 样式

| 字段 | 状态 |
|---|:---:|
| `font_name` / `font_size` / `bold` / `italic` / `underline` | ✅ |
| `fore_color` / `back_color` | ✅ |
| `border` / `format` / `indent` / `rotation` | ✅ |
| `h_align` / `v_align` | ✅ |
| `diagonal` | ✅ |
| `hyperlink` / `hyperlink_window` / `notes` | ✅ |
| `under_line_style` | ⛔ | 缺命名码表；`underline` 布尔已覆盖常规用法 |
| `merge_null` / `merge_same_mode` / `merge_null_mode` | ⛔ | 裸码，缺产品语义 |
| `extend_area` | ⏳ | 需要明确“扩展边界”的 JSON 形状 |
| `to_excel_mode` / `adjust_size_mode` | ⛔ | 裸码，缺码表 |
| `cell_obj` / `cell_type` / `cell_style_name` | ⛔ | 产品内部/命名样式 |
| `cell_graph_config` / `ois_blob` | ⛔ | 私有图形/OIS 记录 |
| `exp_map`（私有键） | ⛔ | legacy 快照承载 |

## 3. 行/列

| 字段 | 状态 |
|---|:---:|
| `type` / `height` / `width` | ✅ |
| `visible` / `break_page` / `break_column` | ✅ |
| `group_level` / `adjust_height` | ✅ |
| `notes` | ⏳ | 语义明确，但低频；可按需补 |
| `ois_blob` / 未知 `exp_map` 键 | ⛔ | 私有记录 |

## 4. 打印

| 字段 | 状态 |
|---|:---:|
| `paper` / `orientation` | ✅ | 作者糖，映射宽高 |
| `paper_width` / `paper_height` / 四边距 | ✅ |
| `pager_style` / `row_num_per_page` | ✅ |
| `copy_count` / `zoom_scale` | ✅ |
| `table_column_num` / `layout_rows` / `layout_cols` | ✅ |
| `back_graph_printed` / `column_repeated_group_header` | ✅ |
| `full_screen_preview` / `blank_line_filled` | ✅ |
| `virtual_printer` | ✅ |
| 裸码 `orient` / `h_align` / `v_align` / `zoom_mode` / `layout` / `*_mode` | ⛔ | 附件 D 无码表；不猜 |
| `printing` 的矩阵/页眉页脚位置 | ✅ |

## 5. 结论

### 已覆盖

- 内容、公式、字面量、合并、扩展、主格
- 常用样式、边框、数字格式、对角线、超链接、备注
- 行/列尺寸、可见性、断页、分组
- 打印纸张/方向/边距/分页/缩放/份数
- 数据集、参数、宏、打印矩阵、分组断页

### 暂不开放

这些字段不是“忘了”，而是**没有足够的语义/码表证据**；在 JSON-only 策略下更不能靠猜：

- 打印裸码（`orient`/`h_align`/`v_align`/`zoom_mode`/`layout`/各 mode）
- `merge_null` / merge modes
- `extend_area`
- `to_excel_mode` / `adjust_size_mode`
- `cell_obj` / `cell_graph` / 子报表 / OIS
- 未逆向的 report meta 记录

### 何时再补

只有满足以下之一才新增字段：

1. 真实报表/客户资产需要；
2. 有可验证的产品语义或码表；
3. 能写出 JSON→model→RPX→JSON 往返测试和失败用例。

否则优先做 Web 登录/E2E、PDF、SQL 上限与性能，而不是继续平铺字段。
