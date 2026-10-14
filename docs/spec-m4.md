# Spec — ReportLite Rust 重实现 · M4（Univer 前端显示）v0.1

> 状态：**立档**（2026-10-14）。上游决策：总监 2026-10-14「M3 M4 一起推进，这样可以看到报表显示」。
> 蓝图（L1 层）：`docs/reportlite-rust全链路设计.html` 的 **§九（Univer 前端设计）/ §八（服务端 API）/ §13（M4 行）**；
> 本文件是其「任务与验收」下位文档 —— **冲突时以证据（夹具 > 源码 > 契约文档）为准**。

## 0. 本文件的可信度声明

与 `spec-m2.md` §0 同规：凡未附证据的断言一律标注「待验证」，不得当结论引用。
本文件定义 **M4 做什么、怎么验收、边界在哪**；快照字段名以 Univer 官方类型定义为唯一基准
（2026-10-14 逐条核对 docs.univer.ai，见 §4）。

## 1. 产品定义

M4 = 报表的**浏览器显示终端**：Univer 装载服务端产出的快照 JSON，零业务逻辑。

> 验收（蓝图 §13 M4 行原文）：**浏览器打开看到带合并/样式的报表；翻页可用。**

M4 **不承担**计算、导出、打印（蓝图 §一结论：Univer 只当渲染终端）。

## 2. 与 M3 的并行关系（本表是本轮排期的契约）

M3 与 M4 的**唯一接口 = 快照 JSON**（`IWorkbookData`）。因此：

| 原编号 | 原级别 | 调整                       | 理由                                   |
| ---- | ---- | ------------------------ | ------------------------------------ |
| M3-08 | P1   | **提为关键路径，v1 先行**（见 `docs/spec-m3.md` §2.1） | 它是 M4 的唯一输入；后置则 M4 无法启动 |

两条数据管道（前端开发不被 axum/libSQL 阻塞）：

- **开发期（a）**：`rpx-server snapshot <file.rpx> -o out.json` —— 本地文件直出快照，
  前端先吃文件；同一条代码路径（`snapshot::build`）也是将来 `/render` 的内核。
- **生产（b）**：`POST /api/reports/{id}/render` → 同一 JSON（M3-05 接线，M3-06 真实口径）。

⇒ 并行节奏：**M4 前端从第一天就有真实数据可用**（a 管道），HTTP 对接（b 管道）后置，
只是把 `fetch(本地文件)` 换成 `fetch(/api/reports/1/render)`。

## 3. 技术栈与目录

- 目录：`web/`（rpx 仓库内，与 crates/ 并列；单仓演进，M5/M6 复用）。
- 栈：**Vite + React + TypeScript**；渲染终端 `@univerjs/presets` + `@univerjs/preset-sheets-core`
  （蓝图 §九已定，Apache-2.0）。
- 端口（规划）：web dev **5288**；`rpx-server`（M3-05 后）**8090**。避开 admin 5188/8088 与 coucouya 5199。
- ⛔ 本机约束（跨项目已证）：`npx` 不可用（触发 wsl 被拦截）⇒ 一律
  `node node_modules/vite/bin/vite.js` 直连；npm 走官方 registry。

## 4. 快照契约 v1（`IWorkbookData` 子集，字段名以官方类型定义为准）

核对来源（2026-10-14）：`IWorkbookData` / `IWorksheetData` / `ICellData` / `IStyleData` / `HorizontalAlign` / `VerticalAlign`
六页官方 reference（docs.univer.ai）。

### 4.1 工作簿级

| 字段           | v1 取值                                            | 出处 / 备注                             |
| ------------ | ------------------------------------------------ | ----------------------------------- |
| `id`         | `"rpx-<报告名>"`                                   | 任意唯一串                               |
| `name`       | 报表名（.rpx 文件名去扩展名）                                |                                     |
| `appVersion` | `"1.0.0"`                                        | ⚠ 待联调与前端 Univer 版本对齐后再定（§7）          |
| `locale`     | `"zhCN"`                                         | `LocaleType.ZH_CN` 的串值              |
| `sheetOrder` | `["rpx-sheet-1"]`（v1 单页；多页 v2 见 §5 M4-05）        |                                     |
| `sheets`     | `{ "rpx-sheet-1": <IWorksheetData> }`            |                                     |
| `styles`     | **v1 不建样式表**：`s` 一律**内联** `IStyleData`（少一层 id 引用歧义） | 去重优化后置（v2）                          |

### 4.2 工作表级（`IWorksheetData`）

| 字段                    | v1 取值                                     | 转换                                                        |
| --------------------- | ----------------------------------------- | --------------------------------------------------------- |
| `id` / `name`         | `"rpx-sheet-1"` / `"Sheet1"`              |                                                           |
| `rowCount`            | `max(grid.rows + 5, 100)`                 | 余量便于滚动                                                    |
| `columnCount`         | `max(grid.cols + 3, 26)`                  |                                                           |
| `defaultRowHeight`    | `30`（≈8mm，`def::ROW_HEIGHT`）              | mm→px：`mm/25.4*96`（96dpi 近似，§7 声明的近似）                     |
| `defaultColumnWidth`  | `94`（≈25mm，`def::COL_WIDTH`）             | 同上                                                        |
| `showGridlines`       | `1`                                       | v1 无边框样式，先开网格线保可读性（联调后可调）                                 |
| `cellData`            | `{ "r":{"c": <ICellData>} }`              | 由 `grid.cells` 过滤 `is_emitted()`，坐标 **1 起 → 0 起**（`r-1`,`c-1`） |
| `mergeData`           | `[{startRow,startColumn,endRow,endColumn}]` | 由 `grid.merged` **去重取锚点**（§4.4）；0 起坐标                        |
| `rowData`             | `{ "r":{"h": <px>} }`                     | `grid.row_heights[r]`（已按扩展铺开）＋ mm→px                         |
| `columnData`          | `{ "c":{"w": <px>} }`                     | `grid.col_widths[c]` ＋ mm→px                                   |

### 4.3 单元格级（`ICellData` / `IStyleData` v1 映射表）

`ICellData`：`v` = 值（§4.3.1）、`s` = 内联 `IStyleData`（下表）、`t` 省略（Univer 自行推断）。

样式来源：`CalcCell.src`（模板出处 `src_row/src_col`）→ 模板 `rpx_core::Cell`。
⚠ 无 `src` 的格（如引擎生成的占位）不出样式。

| Univer 字段 | rpx 来源                                     | 转换                                                                                     |
| --------- | ----------------------------------------- | -------------------------------------------------------------------------------------- |
| `bl`      | `Cell.bit_flags & bit::BOLD`              | 0/1                                                                                    |
| `it`      | `bit::ITALIC`                             | 0/1                                                                                    |
| `ul`      | `bit::UNDERLINE`                          | `{"s":1}`（`ITextDecoration`；线型细分后置）                                                    |
| `ff`      | `Cell.font_name`（None → `def::DEFAULT_FONT_NAME`） | 原样                                                                                     |
| `fs`      | `Cell.font_size`（i16，默认 12）               | 原样（pt）                                                                                 |
| `cl`      | `Cell.fore_color`（ARGB int）                | → `{"rgb":"#RRGGBB"}`（取低 24 位；`0xFF000000`→`#000000`）                                     |
| `bg`      | `Cell.back_color`（ARGB int）                | 同上；⚠ **默认白 `0xFFFFFFFF` 也写出**（财政报表底色显式更稳，待联调确认）                                           |
| `ht`      | `Cell.h_align`                            | 数字枚举：`-48→1(left)` `-47→2(center)` `-46→3(right)` `-45→4(justified)` `-44→6(distributed)`（`format-contract.md` 注 3） |
| `vt`      | `Cell.v_align`                            | `-32→1(top)` `-31→2(middle)` `-30→3(bottom)`                                              |
| — 其他      | 边框（`bd`）/ 数字格式（`n`）/ 旋转（`tr`）/ 缩进          | **v1 不做**（后置；边框编码已备查 `format-contract.md` 注 2）                                           |

#### 4.3.1 值映射（`Value` → `v`）

| `Value`                  | JSON `v`                          |
| ------------------------ | --------------------------------- |
| `Null`                   | 省略 `v`（但 `disp_value` 存在时用 disp）  |
| `Int/Long/Short/Byte`    | number                            |
| `Double/Float`           | number（NaN/Inf 写 null + 显式标注，§7） |
| `Decimal(String)`        | **字符串原样**（保 `BigDecimal` 保真，不转 float） |
| `Text(String)`           | string                            |
| `Bool`                   | boolean                           |
| `List`                   | string：`join(",")`（v1 近似；见 §7）     |
| **有 `disp_value` 时**     | **一律优先 `disp_value`（字符串）** —— 显示值优先于原值                    |

### 4.4 `mergeData` 锚点恢复（v1 算法，2026-10-14 实证修订）

`grid.merged` 的每项是「合并趟后计算矩阵上的槽」（**含延续槽**）；延续槽上的格就是锚点
对象 ⇒ 每槽报出**整块**跨度，但**槽坐标是槽自己的**（`calc/run.rs` 的 `export_merged`）。
⇒ **单槽无法推断锚点位置**：初版"被覆盖"判定在 `engine_merge` 上误滤了 `(4,1)`/`(6,1)`
两块（槽 `(3,1)` 携带整块跨度 2，看起来"覆盖"了 `(4,1)`）——端到端测试抓到的，故改成
下述**消费式**恢复：

不变式：**合并区 = 完整矩形槽集** ⇒ 贪心消费 + 逐格校验：

1. 槽集合去重（`(r,c,rm,cm)`），按 `(r,c)` 排序；
2. 游标扫未消费槽：它必为某块**最小槽**（块内其余槽若先行出现会被更小的游标消费，
   矛盾）⇒ 以其跨度 `(rm,cm)` 展开网格 `[r,r+rm)×[c,c+cm)`；
3. 网格内**每格**必须有**同跨度**的槽；缺槽 / 已被其他块消费 ⇒ **Err**（不静默）；
4. 消费网格、产出 `{startRow:r-1, startColumn:c-1, endRow:r+rm-2, endColumn:c+cm-2}`（0 起）。

实证：`engine_merge` 12 槽 → 6 块（逐块坐标钉在 `crates/rpx-server/src/snapshot/tests.rs`）；
`engine_samemerge` 4 槽 → 2 块（该夹具的跨度**只**在 `merged` 段可见——实现误用 `source`
段必红）。

⚠ v1 未对拍（产品无 IWorkbookData 夹具）⇒ 前端目测 + 上述夹具级断言为准。

### 4.5 `pagination` 块（M3-06，**rpx 扩展键**，workbook 顶层）

**非 Univer `IWorkbookData` 字段**——前端/Univer 忽略未知顶层键；M4-05（翻页）的输入。
值来自 `/render` 接线的**模板真值口径**（纸码表 + 模板边距，`pager::paginate_with_setup`；
探针口径 595×842 是夹具对拍专用，不出现在快照里——双口径见 `pager.rs` 模块文档）。

```json
"pagination": {
  "page_w_pt": 487.5590515136719,   // 可用页宽（pt）= (纸mm − 左右边距) × 72/25.4，f32 存储
  "page_h_pt": 700.157470703125,    // 可用页高（pt）= (纸mm − 上下边距) × 72/25.4
  "columns": 1,
  "page_count": 2,
  "x_pages": 1,
  "y_pages": 2,
  "row_num_per_page": 20,
  "pages": [
    { "page": 1, "rows": 12, "row_range": [0, 11], "col_range": [0, 8] },
    { "page": 2, "rows": 12, "row_range": [1, 22], "col_range": [0, 8] }
  ]
}
```

* `row_range` / `col_range`：该页在网格里的足迹，**0 起含端**（与 §4.2 同规）。
  ⚠ 页首区带（页眉/标题/表头）与分组头副本**每页重复** ⇒ 相邻页的 `row_range`
  **可以重叠**（如实记录，`engine_pager` 第 2 页起首行 = 分组头副本行）。
* `row_num_per_page`：模板 `rowNumPerPage` 的**回显**，`pagerStyle==2` 下不参与断页。
* 守卫：`pagerStyle != 2` / 多栏（`columnMode`/`tableColumnNum`/重复组头）/ 列区带
  （`colType != -79`）/ 可用尺寸非正 ⇒ HTTP 400（`UnsupportedPaging`）。
* **换行回填无守卫**（静态不可行）：`-64` 是 `INormalCell.TYPE_NORMAL`（普通格默认
  类型），测量分支只在「值为 String 且页内放不下」时触发——两个条件都是断页期才知道。
  运行时风险带账（`pager/guard.rs` 尾注）；含大量长文本换行格的模板切点可能与产品有偏差。

### 4.6 多 sheet 翻页（M4-05，2026-10-15）

`pagination.page_count > 1` ⇒ 快照 `sheets` 为**每页一个 sheet**：

| 项 | 契约 |
| --- | --- |
| sheet id / 名 | `rpx-sheet-{页码}` / `第 {页码} 页`（页码 1 起） |
| `sheetOrder` | **顺序即页序**（Univer 底部 tab 即翻页；M4-05 的交付形态） |
| `cellData`/`rowData`/`columnData` | 页内坐标 = 全局 − 页偏移（0 起，与 §4.2 同规）；页首区带/分组头副本因页间 `row_range` 重叠**自然每页重现**（与产品每页独立矩阵同观感） |
| `rowCount`/`columnCount` | 页尺寸 + 余量（同 §4.2 规则） |
| `mergeData` | 锚点在页内才保留；**跨页合并区裁剪**到页界（⚠ 声明近似：产品做拆分、延续段独立成块，两者块边缘有显示差异；产品拆分语义无夹具基准——`merged.cells` 多页全空是 M2 已声明缺口，等价性待探针补证） |
| 顶层 `pagination` | 保留（§4.5 观测块，不因多 sheet 退役） |

单页 / 手搭网格（无 pagination）⇒ 单 sheet（§4.1 现状）；夹具对拍走 `rpx-engine`
emit（不经 snapshot.rs）⇒ 结构变更不影响 17 夹具对拍。

### 4.7 参数表单（M4-04，2026-10-15）

前端（`web/src/App.tsx`）：「渲染」显式按钮 + 参数动态行（参数名/值 + SVG 图标
添加/删除）；值推断与 `form.rs::json_to_value` 同规（整数字面量 → Int、小数 →
Double、其余字符串），随 multipart `params` 段提交。v1 手工核对口径（spec §5）——
自动从参数清单生成表单待资产库 `/meta` 接前端（后置）。

## 5. 验收标准（EARS）

| 编号    | 功能      | 验收标准                                                                                                                | 级别  |
| ----- | ------- | ------------------------------------------------------------------------------------------------------------------- | --- |
| M4-01 | 前端脚手架   | While 在 `web/`，**必须**可 `node node_modules/vite/bin/vite.js build` 成功（exit 0）；产物含 Univer 运行时                                    | P0  |
| M4-02 | Univer 壳 | While dev server 打开，**必须**渲染出空工作簿（canvas 挂载、无控制台错误）                                                                   | P0  |
| M4-03 | 快照装载    | ✅ **已达成**（2026-10-14）：值/合并区/行高列宽/样式 9 字段直通（`style.rs`：bl/it/ul/ff/fs/cl/bg/ht/vt；对齐总输出）；`mergeData` 锚点断言（§4.4） | P0  |
| M4-04 | 参数表单    | ✅ **已达成（v1 手工核对）**（2026-10-15）：动态参数行表单（§4.7）→ multipart `params`；值推断与 `form.rs` 同规；浏览器实测表单交互全链路（截图 `tests/fixtures/out/m4-paged-tabs.png`）；自动生成表单待 `/meta` 接前端（后置） | P1  |
| M4-05 | 翻页      | ✅ **已达成**（2026-10-15）：多 sheet 切片（§4.6）——`engine_pager` 真值 4 页 = 4 个「第 N 页」tab，`sheetOrder` 即页序；坐标平移/区带副本每页重现/跨页合并裁剪（声明近似）；单测（真实夹具）+ 浏览器实测切页截图（`m4-page4.png`） | P1  |
| M4-06 | HTTP 联调  | ✅ **已达成**（2026-10-14）：`fetchSnapshot(files)` 走 `POST /api/render`（multipart：`.rpx` → `report`，其余 → `dataset`）；vite dev 代理 `/api` → `127.0.0.1:8090`；错误体 `{"error":…}` 原样上屏；浏览器实测中文报表全链路（截图 `tests/fixtures/out/m4-render-http.png`） | P1  |

## 6. 本地开发工作流

```bash
# 1) 起报表服务端（M3-05）：POST /render 主链路（默认 127.0.0.1:8090）
cargo run -p rpx-server -- serve

# 2) 起前端（本机 npx 不可用，直连 node；--host 127.0.0.1 必须，否则绑 IPv6 ::1）
cd web && node node_modules/vite/bin/vite.js --host 127.0.0.1 --port 5288

# 3) 浏览器打开 http://127.0.0.1:5288 —— 选中 .rpx 模板**及其同目录 CSV**
#    （多选一起上传；少给 CSV 会显式报「缺少数据集文件：<文件名>」）
```

产物即所见：GBK CSV → 引擎计算 → 快照 JSON → Univer 装载（2026-10-14 实测截图
`tests/fixtures/out/m4-render-http.png`）。v1 的静态文件管道（`web/public/snapshot.json`）
已退役（文件已删），快照只从服务端来。

## 7. 已知坑与待验证项

| 项             | 状态   | 说明 / 消歧方法                                                                        |
| ------------- | ---- | --------------------------------------------------------------------------------- |
| `appVersion`  | 待联调  | 官方语义 = "Univer model definition version"；以实测能装载为准，前端可覆盖                            |
| 单位换算 mm→px    | 声明近似 | 96dpi 线性换算；**未对拍**（无产品基准）⇒ 只求视觉正确，精确口径后置                                             |
| `List` 值      | v1 近似 | `join(",")`；未见需要 List 展示的夹具，待有基准再定                                                   |
| NaN/Inf       | 待验证  | 须避开 JSON 非法数字（serde_json 序列化 NaN 会报错/写 null）；写出时显式标注                                 |
| Univer 体积     | 已知   | `@univerjs/presets` 依赖树大，npm 安装耗时；⛔ 不用 npx                                              |
| canvas 渲染     | 已知   | DOM 无表格内容，e2e 断言只能走 Univer 数据模型（`univerAPI.getActiveWorkbook().save()`）或截图            |
| `styles` 去重   | 后置   | v1 内联样式，JSON 体积偏大（可接受）                                                             |
| 多页 / 真实口径      | 依赖   | M4-05 依赖 M3-06；v1 单 sheet 全量行                                                      |
| 前端构建不入门禁      | 声明   | G1–G7 只管 Rust 侧；前端自审（build exit 0 + 控制台无错）；门禁扩展在 M3-07                             |
| **vite 代理不剥前缀** | ✅ 已定位（2026-10-14 实测） | vite 的 `server.proxy` 默认**原样转发全路径**：`'/api': 'http://127.0.0.1:8090'` 会把 `/api/health` 转成目标上的 `/api/health` → 服务端 404（现象是"代理没生效"，极易误判为代理坏）。修法 = `rewrite: (path) => path.replace(/^\/api/, '')`（见 `web/vite.config.ts` 注释）；定位手段：中间人转发器逐跳打印路径 |
| vite 绑 `localhost`  | ✅ 已知（IPv6） | 不带 `--host 127.0.0.1` 时绑 `::1`，curl/浏览器走 `127.0.0.1` 连不上（连接被拒，非代理问题） |

## 8. 变更记录

| 日期         | 变更                      | 原因                                    |
| ---------- | ----------------------- | ------------------------------------- |
| 2026-10-14 | v0.1 立档（team-lead）      | 总监令「M3 M4 一起推进，看到报表显示」；M3-08 提为关键路径 |
| 2026-10-14 | v0.2：**M4-06 闭合**（HTTP 联调） | `fetchSnapshot(files)` 走 `POST /api/render`；静态快照管道退役；工作流 §6 更新；记录 vite 代理剥前缀 + IPv6 绑定两坑（§7）；浏览器实测中文报表全链路 |
| 2026-10-15 | v0.5：**M4-04/M4-05 闭合（M4 全验收达成）** | §4.6 多 sheet 翻页契约（每页一 sheet、sheetOrder 即页序、坐标平移、区带副本重现、跨页合并裁剪=声明近似）+ §4.7 参数表单（动态行、值推断同 `form.rs`）；`snapshot/paged.rs` 切片实现 + 真实夹具单测；App.tsx 渲染按钮 + Token 化配色 + SVG 图标；浏览器实测 4 页 tab 切换（截图 2 张）；418 测试全绿 + G1–G8 全绿；RUNLOG §31 |
| 2026-10-14 | v0.3：**M4-05 输入就绪**——快照加 `pagination` 块（§4.5，M3-06） | workbook 顶层 rpx 扩展键：真值口径页尺寸 + 逐页 `row_range`/`col_range`（0 起）；换行回填静态守卫不可行的核对结论（`pager/guard.rs` 尾注）；M4-05 从「依赖 M3-06」变为「可直接接线」 |
