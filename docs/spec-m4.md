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

## 5. 验收标准（EARS）

| 编号    | 功能      | 验收标准                                                                                                                | 级别  |
| ----- | ------- | ------------------------------------------------------------------------------------------------------------------- | --- |
| M4-01 | 前端脚手架   | While 在 `web/`，**必须**可 `node node_modules/vite/bin/vite.js build` 成功（exit 0）；产物含 Univer 运行时                                    | P0  |
| M4-02 | Univer 壳 | While dev server 打开，**必须**渲染出空工作簿（canvas 挂载、无控制台错误）                                                                   | P0  |
| M4-03 | 快照装载    | While 加载 `rpx-server snapshot` 产出的 JSON，**必须**显示：网格值 / 合并区 / 行高列宽 / 样式（§4.3 表内字段）；断言：`mergeData.length == 去重后锚点数`（§4.4） | P0  |
| M4-04 | 参数表单    | While 报表带参数清单，**必须**生成可提交的参数表单（静态从模板 `Param` 读，v1 手工核对）                                                               | P1  |
| M4-05 | 翻页      | While 报表多页，**必须**可翻页（多 sheet 或 `pno` 拉取）；**依赖 M3-06 真实口径**，可后置                                                             | P1  |
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
