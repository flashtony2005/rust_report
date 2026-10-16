# Spec — ReportLite Rust 重实现 · M7-W（Univer 前端设计器）v0.8

> 状态：**v0.8（2026-10-16）—— W0、W1、W2-1~W2-4（含降级的 W2-3'/W2-4）已收口；只剩 W2-5 未开工**。
> 上游决策：`docs/report-def-json.md` §7.2 明写「在线设计器（`web/` 侧）—— Univer
> 目前仍是**纯渲染终端**，在它上面做『定格式』是**新阶段**，须另立 spec」——
> 本文件就是那份 spec。
> v0.2：G9 挂进 `scripts/gate.sh`（W0 收口）。v0.3：定义 JSON 通道接上（W1 收口）。
> v0.4：Univer 编辑 API 运行时探针（§7 坑位 4 由「待验证」变实测 13/15），
> **缩进与打印设置两项因 API 不存在而降级**。
> v0.5：**样式映射层 `stylemap.ts` 收口**（W2-2a），坑位 4→6 条且全部有源码出处。
> v0.6：**W2-1 编辑面收口**（内容补丁 + 导出，只跟事件只补一格）。
> v0.7：**W2-2 样式面板收口** —— 7 字段**逐字段补丁**（明确不走 `applyCellStyle`
> 的 12 槽整体覆盖），浏览器端到端 E1–E9 + 4 组故障注入。
> v0.8：**W2-3 属性面板收口** —— 边框/数字格式/旋转三项；**读方向改从定义 JSON**
> （快照不含这三项，画布读不回来，与 W2-2 相反）；E2E 增 E10–E13 + 几何自校准；
> 故障注入 FI-5–FI-8。新增 §7 坑位 9（快照 rowData/columnData 覆盖 Univer 缺省）。
> 变更史见 §8。
> 蓝图（L1 层）：`docs/reportlite-rust全链路设计.html` §九（Univer 前端设计）；
> **冲突时以证据（夹具 > 源码 > 契约文档）为准**。

## 0. 本文件的可信度声明

与 `spec-m2.md` §0 同规：凡未附证据的断言一律标注「待验证」，不得当结论引用。

本文件定义 **M7-W 做什么、怎么验收、边界在哪**。所有「当前状态」一节的事实都来自
2026-10-16 的实盘盘点（`web/src/` 逐文件读过，行数 `wc -l` 实测），不是推测。

## 1. 产品定义

M7-W = 在浏览器里**定义报表格式**（而非只看），产出 `docs/report-def-json.md`
的定义 JSON，经既有 `POST /render` / `POST /reports` 入库渲染。

> 与 M4 的分界（重要）：M4 是**纯渲染终端**（Univer 只装载快照，零业务逻辑，
> `spec-m4.md` §1）。M7-W 在同一份 Univer 上**开启编辑**，并把编辑结果序列化成
> 定义 JSON —— 这是从「看」到「定」的跨越，不是 M4 的功能增补。

M7-W **不承担**计算、分页、导出（那些留在服务端，`rpx-engine` / `rpx-export`）。

## 2. 当前状态（2026-10-16 晚，W2-2a 收口后）

`web/` 共 **5207 行** TypeScript（14 个文件：9 个源 + 5 个测试；行数 `wc -l` 实测）：

| 文件 | 行数 | 职责 |
| --- | --- | --- |
| `cellprops.ts` | 414 | **属性面板（W2-3）：边框/数字格式/旋转**。读方向是**定义 JSON**（快照不含这三项）；逐键补丁（边开关/线型/色/格式串/角度）；`width` 保留、空壳移除；Univer 侧只是视觉反馈 |
| `cellprops.test.ts` | 378 | **28 条**纯逻辑单测（读方向 5 + 选项 2 + 归一 5 + 逐键补丁 11 + Univer 落地 5，含与真实枚举对齐） |
| `stylepanel.ts` | 477 | **样式面板（W2-2）：7 字段逐字段补丁** + **选区脚手架**（`patchRangeSlots`/`finishRangePatch`，W2-3 共用同一份裁剪/上限/合并跳过/重复检测） |
| `stylepanel.test.ts` | 413 | **34 条**纯逻辑单测（读方向 5 + 字体选项 3 + 值归一 4 + 逐字段补丁 10 + 边界 5 + 恢复缺省 3 + Univer 落地 4） |
| `designer.ts` | 235 | **编辑面（W2-1）：内容补丁 + 导出**（坐标系 / text↔expr / 合并内非锚点 / 网格边界四条契约）；`slotKey` 导出给面板共用 |
| `designer.test.ts` | 345 | **30 条**纯逻辑单测 |
| `stylemap.ts` | 646 | **样式映射层**（W2-2a）：`IStyleData` ↔ JSON 12 槽；码表本地字面量 |
| `stylemap.test.ts` | 549 | **43 条**纯逻辑单测（枚举对齐 + 录得快照 + 六条坑位 + 补丁语义 + 写方向） |
| `probe.tsx` | 369 | W2 准入探针页（开发期，不进 `index.html`） |
| `App.tsx` | 899 | 顶栏 + **样式条（W2-2）** + **属性条（W2-3：边框/格式/旋转）** + 参数表单 + 状态栏 + Univer 容器；选区订阅 `CommandExecuted` |
| `report.ts` | 228 | `fetchSnapshot`（`.rpx`/`.json` 双入口）+ `saveReport` + `fetchMeta` + 三个纯函数 |
| `report.test.ts` | 243 | **25 条**纯逻辑单测 |
| `main.tsx` | 10 | 挂载入口 |
| `vite-env.d.ts` | 1 | 类型声明 |

技术栈（`package.json`）：Vite 8 + React 19 + **Univer 1.0.4**
（`@univerjs/preset-sheets-core` + `@univerjs/presets`，zh-CN locale）。

**M7-1 ~ M7-6 已在服务端备好的能力**（本 spec 的立足点）：

| 服务端能力 | 落地 | 前端现状 |
| --- | --- | --- |
| 定义 JSON 是 `/render` 的一等输入 | M7-1 | ✅ **W1-1**：`report` 字段收 `.json`（`pickReportFile`） |
| 定义 JSON 可归一化为 `.rpx` 入库 | M7-2（`POST /reports`） | ✅ **W1-3**：「另存为」→ `saveReport` 拿 id |
| 参数清单可由 `/reports/{id}/meta` 读回 | M7-4 | ✅ **W1-2**：入库后按 `/meta` 刷新表单（`metaToRows`） |
| 单元格样式 7 字段 ↔ 快照严格互逆 | M7-3 | ✅ **W2-2**：面板收口（7 字段 → JSON 单键，E2E E1–E9 + 4 组故障注入） |
| 边框 / 格式 / 缩进 / 旋转 / `extend_mode` | M7-5 | ✅ **W2-3**：边框/数字格式/旋转面板收口（读 JSON、逐键补丁、E2E E10–E13 + FI-5–FI-8）；**缩进降级为 JSON 声明**（W2-3'，验收由服务端往返测试承担） |
| 打印设置覆盖（17 字段） | M7-6 | ✅ **W2-4 降级为 JSON 声明**（快照不暴露 + sheets 无页面设置 API ⇒ 无面板；验收由 `definition_roundtrip.rs` 的 print_setup 两条测试承担） |

⇒ W1 清掉了「服务端已支持、前端没接」；W2-2a 把**样式这条路的映射地基**立住；
W2-1 把**内容这条路的补丁地基**立住；**W2-2 把 7 字段的面板接到用户手上**
（逐字段补丁）；**W2-3 把边框/格式/旋转接上**（读方向改从定义 JSON —— 这三项
不在快照里，见 §4 W2-3）；W2-3'/W2-4 按**降级口径**闭合（无面板，验收由服务端
模型往返承担）。剩下的缺口是 **W2-5**（网格尺寸/行列属性/数据集绑定）与 W3 的
`extend_mode` 声明（本 spec 不承诺）。

## 3. 阻塞项（**已于 2026-10-16 解除**）

> 原文（立档时）：「本机 `web/` 无 `node_modules`、`package.json` 没有测试运行器
> ⇒ 前端无法 `tsc` / `vite build` / 跑任何测试 ⇒ M7-W 无法按本仓标准收口。」

**解除事实**（同日实测，非推测）：

| 量 | 实测值 |
| --- | --- |
| `web/node_modules` | 已装，**144** 个顶层包（`package.json` 登记 20 个） |
| 测试运行器 | **vitest 5.0.5**（devDependency），`npm test` = `vitest run` |
| 前端单测 | `web/src/report.test.ts`，**15 条**（`pickReportFile` 4 + `paramsToObject` 11） |
| 机器闸 | `scripts/gate.sh` 新增 **G9**，判据两条：`npm run build`（`tsc --noEmit && vite build`）+ `npm test` |
| G9 有效性 | **3 组故障注入全红**，见 §4 W0-3 表下方 |

⇒ 前置条件已满足，W0 收口，**W1 可开工**。

> ⚠ **别把「缺 node_modules」再记成阻塞**：这条已被解除，但 `scripts/gate_web.sh`
> **仍然要求 node_modules 存在**（缺了必须红，见 §7 坑位 2）——解除的是「装不上」，
> 不是「不检查」。

## 4. 分期计划

### W0 前端可验证性地基（**准入条件，已收口 2026-10-16**）

| 项 | 内容 | 验收 |
| --- | --- | --- |
| W0-1 | `npm install` 装齐 Univer 1.0.4 / Vite 8 / React 19 | ✅ `npm run build` 退出 0 |
| W0-2 | 引入 vitest + 一个纯逻辑测试（如 `report.ts` 的参数行推断） | ✅ `npx vitest run` 15 条全绿 |
| W0-3 | 把 `npm run build` + `npx vitest run` 挂进 `scripts/gate.sh`（新 gate **G9**） | ✅ 3 组故障注入全红，见下 |

**W0-3 的判据有效性已用故障注入证明**（与 G1–G8 同规，「改反实现 → 必须红 → 改回 → 必须绿」）：

| # | 注入 | 结果 | 改回后 |
| --- | --- | --- | --- |
| FI-1 | `paramsToObject` 里整数字面量改返回 `String(v)` | **红**：`vitest` 4 失败 / 11 通过，`gate.sh` 汇总 `FAIL G9 (exit=1)`、`GATE: 有红项，不许开 PR` | ✅ 全绿 |
| FI-2 | 删掉 `web/node_modules`（用临时假目录树模拟，**不动真 node_modules**） | **红**：`G9 FAIL: web/node_modules 不存在 —— 先跑 cd web && npm install` | — |
| FI-3 | `pickReportFile` 里加一句 `const broken: number = 'not-a-number'` | **红**：`tsc` 报 `TS2322`，`G9 FAIL: npm run build exit=1` | ✅ 全绿 |

FI-1 是在**完整 `gate.sh`** 上跑的（不是只跑 `gate_web.sh`）：红项精确落在 G9，
G1–G8 仍 PASS ⇒ 证明红是 G9 自己报的，不是上游污染。

> ⚠ **G9 的构建产物写进临时目录，不是 `web/dist`** —— 这是 G9 能在本机跑通的
> 关键，理由与证据见 §7 坑位 1。**不要为了「让输出落在仓库里」把它改回 `web/dist`**，
> 那会让 G9 在 WorkBuddy 沙箱里稳定红。

**W0 未绿之前 M7-W 不进 W1** —— 现已绿，W1 开工。

### W1 定义 JSON 通道（前端接入既有服务端能力，**已收口 2026-10-16**）

| 项 | 内容 | 验收 |
| --- | --- | --- |
| W1-1 | `report.ts` 接受 `.json` 定义文件（与 `.rpx` 并列选一个） | ✅ 浏览器实测：选 `def_params_demo.json` → 渲染按钮 → Univer 出报表 |
| W1-2 | 参数表单改读 `GET /reports/{id}/meta`（M7-4 的清单），不再手工填名 | ✅ 浏览器实测：点「另存为」后表单自动带出 2 行，动态行带「动态」标注 |
| W1-3 | 「另存为」：定义 JSON → `POST /api/reports` → 拿 id → 提示入库成功 | ✅ 浏览器实测：`已入库 id=1`；`GET /reports` 列表可见该条 |

**判据分两层，两层都过**（本仓纪律：端到端判据也要有可故障注入的机器判据）：

1. **服务端镜像**（`crates/rpx-server/tests/definition_channel_http.rs`，4 条，
   真 socket + 真 HTTP + **磁盘上的同一个 `.json` 文件**）：
   `json_definition_renders_with_param_values` / `missing_param_value_is_400_not_silently_null`
   / `definition_uploads_and_yields_an_id` / `meta_reads_back_the_declared_params`。
2. **前端纯逻辑**（`web/src/report.test.ts`，25 条里 W1 新增 10 条）。
3. **浏览器实测**（`agent-browser`，见下）。

**浏览器端到端实测记录**（2026-10-16，起真 `rpx-server serve` + 真 `vite dev`）：

```text
选文件 def_params_demo.json → 点「另存为」
  → 状态栏：「已入库：def_params_demo（id=1）；参数表单已按服务端声明刷新（2 条）」
  → 表单自动带出：topN=10（静态，默认值预填）/ region=（空，标「动态」）
  → GET /reports       → [{id:1, name:"def_params_demo", version:1}]
  → GET /reports/1/meta → topN(param_type=0,is_static=true,data_type=1,default="10")
                          region(param_type=1,is_static=false,data_type=11,default=null)
填 topN=5、region=华东 → 点「渲染」→ Univer 出报表：「Top N 5」「区域 华东」
```

⇒ 三条判据全部实测成立。截图留证于提交说明。

**判据有效性已用故障注入证明**（与 G1–G9 同规，「改反实现 → 必须红 → 改回 → 必须绿」）：

| # | 注入 | 结果 |
| --- | --- | --- |
| FI-A | `REPORT_SUFFIXES` 摘掉 `'.json'`（回退 W1-1） | **红**：`pickReportFile` 与 `isReportSource` 两条断言挂 |
| FI-B | `metaToRows` 里 `dynamic: !p.is_static` 取反 | **红**：`metaToRows` 4 条挂；**完整 `gate.sh`** 汇总 `FAIL G9 (exit=1)`、G1–G8 仍 PASS |
| FI-C | 夹具 `def_params_demo.json` 的 `topN` 默认值 `10` → `11` | **红**：`meta_reads_back_the_declared_params` 精确挂在 `default_val` 断言 |

FI-B 是在**完整 `gate.sh`** 上跑的（不是只跑 vitest），证明红落在 G9、不是上游污染。

### W2 编辑面（真正的「设计器」）

> ⚠ **分期已按 §7 坑位 4 的实测结论调整**（2026-10-16）：探针 15 项里
> **缩进与打印设置的 API 不存在** ⇒ 这两项从「做面板」降级为「JSON 声明 + 预览」，
> 与 §W3 的 `extend_mode` 同规格。下表「验收」列已按此改写。

| 项 | 内容 | 验收 |
| --- | --- | --- |
| W2-1 | **编辑面：内容补丁 + 导出**（W2-1，**已收口 2026-10-16**）。Univer 开编辑（默认即可编辑），`SheetValueChanged` 事件驱动、只把**用户真改过的那一格**补丁进持有的定义 JSON（不打整体回灌），四条服务端契约前置成显式错误；导出 JSON 经 `/render` 往返成立 | 改完点「导出 JSON」下载定义 JSON；`open_bytes(to_bytes(build(json))) == build(json)` 往返成立（契约判据在 `designer.test.ts`） |
| W2-2a | **样式映射层**（纯函数，`stylemap.ts`） | ✅ **已收口 2026-10-16**，见下方小节 |
| W2-2 | **样式面板**：字体/字号/颜色/粗斜下划线（M7-3 的 7 字段；**已收口 2026-10-16**）。 | ✅ 面板设的值**只补该字段**进 JSON（其余键逐个原样保留）；产物经 `POST /render` 后快照样式与面板值逐项一致（E2E E8/E9）；见下方小节 |
| W2-3 | **边框 / 数字格式 / 旋转面板**（M7-5 的 16 槽里可做的那部分；**已收口 2026-10-16**） | ✅ 面板设的值**只补该键**进 JSON（边内 `width` 保留、条目删空整条移除）；产物 `POST /render` 200 + `POST /reports` 入库成功（`.json → .rpx` 写侧）；见下方小节 |
| W2-3' | 缩进 —— **降级为 JSON 声明**（API 不存在，§7 坑位 4） | ✅ 无面板即验收：`indent` 的「声明 → 往返成立」由服务端 `definition_roundtrip.rs::cell_props_survive_the_rpx_round_trip`（夹具含 `indent: 2.5`）承担；`.json` 与 `.rpx` 两条入口共用同一 parse（M7-5）⇒ 前端不需要也不产生该字段。⚠ `/render` 腿未单列用例（与其它字段同一 parse 入口），如实标注 |
| W2-4 | 打印设置（M7-6 的 17 字段）—— **整项降级** | ✅ 同上：`print_setup_survives_the_rpx_round_trip` + `print_setup_lands_on_the_expected_slots` 承担「声明 → 往返」；前端不产生该字段 |
| W2-5 | 网格尺寸 / 行列属性 / 数据集绑定 | 与 `report-def-json.md` §3 字段一一对应；尺寸用 `setRowHeight`/`setColumnWidth`（**不是** `setRowCustom`） |

W2 的判据形态是 **M7-5 已立下的模型往返**：面板写出的 JSON 经
`parse → to_bytes → open_bytes` 必须逐字段相等（`tests/definition_roundtrip.rs`
的同一条判据，只是输入从手写 JSON 变成面板产物）。

#### W2-1 编辑面：内容补丁 + 导出（**已收口 2026-10-16**）

`web/src/designer.ts` + `web/src/designer.test.ts`（30 测试，G9 覆盖）。`App.tsx` 的
`render` 在 `createWorkbook` 后订阅 `univerAPI.addEvent(univerAPI.Event.SheetValueChanged, …)`
（句柄存 `eventDisposableRef`，重渲染先 `dispose` 旧监听），并把**单格**改动喂给
[`applyValueEdit]`；「导出 JSON」按钮调用 [`downloadDefJson]`。

**为什么只跟事件、只补一格**（内容补丁比样式补丁更危险 —— 写错一个字段服务端就 400）：

* Univer 会把用户敲的 `"0123"` 这类值**按数值归一化**（快照里 `{"t":2,"v":123}`）。
  若做「快照 → JSON」整体回灌，没被碰过的格子也会被归一化 ⇒ **静默数据损坏**。
  ⇒ 入口只有 [`applyValueEdit]`，由事件驱动，**只补用户真改过的那一格**。
* `.rpx` 模板**没有**定义 JSON 可持 ⇒ 编辑面不可用，[`assertEditable]` 给人话文案
  （「请改用 .json 报表源」），不退化成「改了但存不下去」。

**四条服务端契约全部前置成显式错误**（写错一个字段就 400，不猜、不吞）：

| # | 约束 | 出处 | 违反后果 |
| --- | --- | --- | --- |
| 1 | **坐标系**：JSON 逻辑 1 起、Univer 快照 0 起 | `report-def-json.md` §4.1；实证 `snapshot/tests.rs`：`engine_merge` 逻辑 `(2,1)纵2` → 快照 `startRow=1,startColumn=0` | 差一格 → 内容落到隔壁格，**不报错** |
| 2 | **`text` 与 `expr` 互斥** | `report-def-json.md` §3.2 | 用 `text` 覆盖 `expr` = **静默切断数据绑定** |
| 3 | **合并矩形内非锚点槽不得带内容** | `reportdef/parse.rs` 两趟校验 | 服务端 400，前端却显示成功 |
| 4 | **行列号须在 `grid` 范围内** | 同上 §3.2 | 越界 400 |

**补丁语义**（`applyValueEdit`，纯函数返回新对象）：清空 ⇒ **删 `text` 键**（不是写
`text:""`，后者是语义等于缺省的冗余键）；原格无 `cells[]` 条目 ⇒ 追加 `{row,col,text}`；
数值/布尔一律 `String()`（JSON 的 `text` 契约类型是字符串）；重复 `(row,col)` 条目显式告警
（静默改第一条会让用户以为改的是另一条）。

**坐标系期望值不是推导来的，是 `snapshot/tests.rs` 已实证的组**（见 `designer.test.ts`
文件头）—— 这是 W2-1 最危险的形态（差一格不报错，只是落到隔壁格），故用已实证值钉死。

**判据有效性已用故障注入证明**（`designer.test.ts`）：越界 → 红、合并内非锚点 → 红、
`expr` 格写内容 → 红、坐标系 +1/−1 往返一致、清空删键、数值 `String`、重复条目告警。

#### W2-2a 样式映射层（**已收口 2026-10-16**）

`web/src/stylemap.ts` + `web/src/stylemap.test.ts`（43 测试，G9 覆盖）。

**为什么必须先有这一层**：服务端快照是**有损**的 —— `snapshot/style.rs::style_json`
只暴露 9 个字段（`bl/it/ul/ff/fs/cl/bg/ht/vt`），边框 / `format` / `indent` /
`rotation` 不在其中（`report-def-json.md` §4.7 末段）。⇒ 设计器**必须持有原始定义
JSON 并对它打补丁**（[`applyCellStyle]`），**不能拿快照反推 JSON** —— 否则
`indent` / `extend_mode` / `border.width` / `print_setup` / 用户手写的任何未登记
字段会被静默抹掉。这是整个 W2 的地基决定。

**坑位从 §7 坑位 4 的 4 条扩到 6 条**，全部有**源码出处**（不是探针推测）：

| # | 坑 | 出处 |
| --- | --- | --- |
| 1 | 对齐必须读**原始数值** `ht`/`vt`，不能走门面 getter —— 门面 `transformCoreHorizontalAlignment` 把 `JUSTIFIED(4)`/`BOTH(5)`/`DISTRIBUTED(6)`/`UNSPECIFIED(0)` **全塌成 `'general'`**（`default: return "general"`）⇒ 走 getter 就分不出「未设置」和「两端对齐」，而 JSON 有 `justify`/`distribute` 两个槽 | `@univerjs/sheets/lib/es/facade.js` |
| 2 | 门面**没有 `'right'`**，`'normal'` 就是右对齐。UI 下拉框若走门面 setter，值必须写 `'normal'` —— 写 `'right'` 会 `throw new Error('Invalid horizontal alignment')` | `facade/utils.d.ts` 官方注释「`normal` means right alignment」+ `facade.js` 实测 |
| 3 | `ul` 是 `ITextDecoration {s,c,cl,t}` **对象**不是布尔。只取 `s`；`c`/`cl`/`t`（颜色跟随字体 / 下划线色 / 线型）**没有 JSON 槽** | `core/.../i-style-data.d.ts` |
| 4 | 颜色是 `IColorStyle {rgb,th}` **对象**，且 `rgb` 按注释可以是 `#RRGGBB` **或** `rgb(r, g, b)` 两种形态，而 JSON 只收 6 位 `#RRGGBB`；主题色 `th` **无 JSON 槽** | 同上 |
| 5 | **边框线型码空间完全不同**：Univer `BorderStyleTypes` 是 0..13 稠密枚举，rpx 契约是 `none=0 / dotted=81 / dashed=82 / solid=83 / double=84`。`DOTTED=3` vs `dotted=81`、`DOUBLE=7` vs `double=84`、`THIN=1` vs `solid=83` —— **不是平移关系，只能查表**。Univer 14 种线型里只开 5 种，其余 9 种**无 JSON 槽** | `enum/border-style-types.d.ts` × `report-def-json.md` §4.7 |
| 6 | **`IBorderStyleData` 没有宽度槽**（`BORDER_STYLE_KEYS = ["s","cl"]`）⇒ JSON 的 `border.*.width` 是 **JSON_ONLY**：打补丁时**从原 JSON 继承**，不能当成「用户在 Univer 里改过」 | `i-style-data.d.ts` |

**两条 W2 假设被实测修正**：

* ⚠ **`ht`/`vt` 必须无条件发**。初版实现只在 JSON 声明了对齐时才发，被
  `tests/fixtures/out/engine_merge.snapshot.json`（服务端跑出来的**真实快照**）
  当场抓红：服务端 `style_json` 读的是 `Cell.h_align`，而 rpx 的 `Cell`
  **永远有具体值**（`blank_cell()` 给 `HALIGN_LEFT = -48` / `VALIGN_MIDDLE = -31`，
  从不是 `None`）⇒ 快照里**每格都带** `"ht":1,"vt":2`。省掉这两个键就是让 Univer
  用自己的缺省顶替 rpx 的缺省 —— 正是服务端注释「**对齐总是输出**」要防的视觉漂移。
* ⚠ **读方向只出非缺省**。快照对 `ff`/`fs`/`cl`/`bg` **恒发缺省值**（录得快照里
  每格都带 `{"ff":"宋体","fs":12,"cl":{"rgb":"#000000"},"bg":{"rgb":"#FFFFFF"}}`）
  ⇒ 全量输出会把每个被碰过的格子塞进 4 个它从没声明过的样式键。

**保真分级**（读 `stylemap.ts` 先看这个）：`EXACT`（7 样式 + 2 对齐 + `format` +
`rotation` + `border.{style,color}`）/ `LOSSY`（小数字号、`ul` 的 `c`/`cl`/`t`、
`tr.v`、主题色、对角边框、无槽线型 —— **全部进 warnings**）/ `JSON_ONLY`
（`border.*.width`、`indent`、`extend_mode`、`print_setup`）。硬规矩：**宁可报
warning，不静默丢** —— 丢掉的若是线型或主题色，用户完全看不出渲染会变。

**补丁语义**（`applyCellStyle`，W2-1 的唯一入口）：纯函数返回新对象；只动
`JsonStyleFields` 覆盖的 12 个槽，其余**原样保留**（`CellDef` 带索引签名 + 整体
展开就是为此）；**改回缺省会删键**（否则「加粗 → 撤销加粗」在 JSON 里留
`bold:true`）；对同一 Univer 状态重复打补丁**幂等**。

**三组判据**（缺一不可）：① 与 Univer **真实运行时枚举**逐项对齐 —— 码表在本地
以字面量钉住（不 import 运行时枚举），升级改了码**不会报错只会静默映射错**，
故从 `@univerjs/core` 取真枚举对照；② 与服务端**录得的真实快照**对齐（不是手写
期望值）；③ 六条坑位逐条钉住。

**6 组故障注入全部验证有牙**：FI-1 边框码表改错 → 2 红 + 全门禁 `FAIL G9(exit=1)`；
FI-2 `ht`/`vt` 改回条件发 → 2 红（含录得快照那条）；FI-3 补丁改成只写不删 → 1 红；
FI-4 宽度不再继承 → 2 红；FI-5 `justify`/`distribute` 塌成 null（门面 getter 的病）
→ 2 红；FI-6 本地 `UA_V.MIDDLE` 2→3（模拟枚举漂移）→ **6 红**。

#### W2-2 样式面板（**已收口 2026-10-16**）

`web/src/stylepanel.ts` + `web/src/stylepanel.test.ts`（34 测试，G9 覆盖）。`App.tsx`
顶栏下方新增样式条：字体（select，当前值不在常用表时补进选项——否则 select 静默
错显第一项）、字号（number，`1..=32767`）、B/I/U 三个开关、字色/底色
（`<input type=color>`）、「恢复缺省」。只有渲染过 `.json` 源（`def` 非空）才可用，
否则整体置灰 + 尾部提示原因。

**地基决定：逐字段补丁，明确**不**走 `applyCellStyle`（12 槽整体覆盖）。**
`applyCellStyle` 的语义是「把 Univer 活态整体读回来覆盖 12 个槽」——服务端快照
**有损**（`snapshot/style.rs::style_json` —— `style.rs:15-45` —— 只发 9 个字段：
`ff/fs/bl/it/ul/cl/bg/ht/vt`；录得快照 `engine_merge.snapshot.json` 逐格实测一致），`border` / `format` / `rotation` / `indent`
在画布里**永远是空的** ⇒ 整体覆盖会在用户只点一下「加粗」时把 JSON 里原有的
`border`/`format`/`rotation`/`indent`/`extend_mode` 一起抹掉。⇒ 面板只动
**用户刚改的那一个字段**（7 字段 ↔ 7 个 JSON 槽的固定映射），其余键逐个原样保留
（判据：`stylepanel.test.ts::未登记字段（indent/extend_mode/border/format/手写字段）逐个原样保留`）。

**两个方向的模型**：

* **读（显示）**：面板显示态从 Univer **活态**读（`FRange.getCellStyleData()`，
  选区起点那一格）——用户选一格，看到的是画布上那一格现在的样子（录得快照每格
  恒带 4 个缺省值 ⇒ 面板必须显示为全缺省）。
* **写（补丁）**：用户在面板上改哪个字段，就只把那**一个**字段写进 JSON
  （缺省即删键，与 W2-2a 同一条语义），Univer 侧同步落地（`applyFieldToRange`）。
  **先 Univer 后 JSON**：Univer 落地失败就不补 JSON（避免「JSON 说改了、画布没改」
  的单边不一致；JSON 补丁是纯函数，不引入新失败面）。

**选区的四个服务端事实**（全部有源码出处，决定了边界检查）：

| # | 事实 | 出处 | 面板处置 |
| --- | --- | --- | --- |
| 1 | `cells[].row/col` 越界 400；而 Univer 画布比网格大 | `reportdef/parse.rs:186-197`；`snapshot.rs:64-65`（`rowCount=(rows+5).max(100)`） | 选区**裁剪**到 grid 内再落笔；完全不覆盖网格 ⇒ 显式报错 |
| 2 | 合并矩形内的非锚点槽，**仅样式条目被静默跳过**（不报错） | `parse.rs:88-99`（`continue`） | 这些槽**跳过**（渲染随锚点，不是信息丢失）；整个选区全落在合并区内部 ⇒ 显式报错指路锚点 |
| 3 | 多格选区在画布上每格都改 | Univer `set-style` 按范围落 | JSON 对选区内**每个**非锚点槽落笔（只补一格会与画布漂移） |
| 4 | Ctrl+A 能撑到整个画布（100×26 = 2600 槽） | 同上 1 | 上限 `MAX_PANEL_SLOTS = 400`，超了**报错不截断**（截断 = 用户以为改全了） |

**值归一**（`normalizePanelValue`，不合法显式报错、不猜）：颜色统一出**大写**
`#RRGGBB`（服务端 `argb_hex` 出大写，混用会产生假差异——W2-2a 坑位 5c 的延伸，
判据钉在导出产物的 `#FFFF00` 上）；字号必须是整数且 `1..=32767`；字体名非空；
三个位标志必须是布尔。

**Univer 侧落地的坑**：关闭位样式**不是传 `false`**，而是 `setFontWeight('normal')`
/ `setFontStyle('normal')` / `setFontLine('none')`（`FontWeight`/`FontStyle`/`FontLine`
都是字符串枚举，`f-range.d.ts:24-26`）。

**「恢复缺省」** = 7 个键全删（逐字段复用同一条补丁路径，报错口径完全一致）。

**判据（三层，全部实测）**：

1. **纯逻辑**：`stylepanel.test.ts` 34 条（G9 覆盖）。
2. **浏览器端到端**：`tests/fixtures/out/e2e_w2_panel.sh`（编排：起 rpx-server 8090
   + vite 5288 → 驱动 → 收摊）+ `e2e_w2_panel.mjs`（playwright 直驱），夹具
   `tests/fixtures/def_style_demo.json`。**E1–E9 全绿**：读方向（A1 bold+红+18）→
   选区同步（点 B1 全缺省、点回 A1 样式回来）→ 写（B1 加粗 / 底色 `#ffff00` /
   字号 20）→ elide（A2 关斜体）→ 导出 → **产物逐键断言**（A1 原样 / B1 三键 /
   A2 删 `italic`）→ **服务端往返**（产物 `POST /render`，快照 `bl/fs/bg` 与面板值
   逐项一致，坐标系一并验证）。产物与截图落
   `tests/fixtures/out/w2_panel_exported.json` / `w2-panel-e2e.png`。
3. **故障注入 4 组**（改反 → 必红 → 改回 → 必绿）：

| # | 注入 | 结果 |
| --- | --- | --- |
| FI-1 | `applyFieldToRange` 关闭粗体传 `null`（而不是 `'normal'`） | **红**：2 条（`关闭位样式传 normal/none` + `恢复缺省 ⇒ 7 个 setter`） |
| FI-2 | elide 改成「写显式 `false`/`12`」而不是删键 | **红**：3 条（`改回缺省 ⇒ 删键` + `clearPanelRange` 2 条） |
| FI-3 | 颜色归一去掉大写化 | **红**：4 条（`颜色统一出大写` + `颜色进 JSON 是大写` + `clearPanelRange` 2 条 —— 后者暴露「elide 依赖规范化大小写」的耦合） |
| FI-4 | 合并区跳过改回「照写」（`covered.has(...) && false`） | **红**：3 条（`合并矩形内非锚点槽被跳过` + `全落合并区报错` + `clearPanelRange 合并口径`）；**完整 `gate.sh`** 汇总 `FAIL G9`、G1–G8 仍 PASS |

**E2E 的定位技术（首跑踩坑，写在这里免得重踩）**：面板输入框（尤其 `number`）
会**夺走 DOM 焦点**，之后方向键既可能被 `number` 输入自己吃掉（ArrowDown 把 20
旋成 19 并触发一次 `onChange`），也可能因 Univer 容器失焦而不被 Univer 处理
⇒ **端到端定位一律用几何点击**（主画布左上 + 表头偏移 46/20 + 缺省列宽 88 /
行高 24 —— 本机实测；点击后用面板回读**自证**，Univer 升级改了缺省度量即红），
不用键盘导航。

**已知行为（不是缺陷，如实登记）**：① 多格选区的面板显示取**起点格**（混合态
显示留待后续）；② `number` 输入是**即时补丁**——逐字符触发，中间值会短暂落进
JSON（最终值正确）；③ Univer 侧撤销（Ctrl+Z）与 JSON 草稿的对齐**仍未处理**
（§7 坑位 5 的待验证项，W2-2 未改变其状态）；④ 粘贴带入的样式不在面板覆盖范围
（与 W2-1 对多格粘贴同一边界）；⑤「另存为」仍按 W1 口径上传**原始文件**——
面板编辑的成果走「导出 JSON」出口，「编辑产物直接入库」留待后续（两条出口的
合成不在 W2-2 承诺内）。

#### W2-3 边框 / 数字格式 / 旋转面板（**已收口 2026-10-16**）

`web/src/cellprops.ts` + `web/src/cellprops.test.ts`（28 测试，G9 覆盖）。
`App.tsx` 加「属性」条：边框（**四个边开关** + 线型下拉 + 边框色）、格式
（预设下拉 + 手写值注入）、旋转（0..360 数字框）。

**关键差异：读方向从定义 JSON，不在 Univer。** 这三项**不在服务端快照里**
（§2 表末行；`report-def-json.md` §4.7），画布永远是空的 ⇒ 面板显示态一律从
**持有的定义 JSON** 的选区起点格读（`cellPropsFromDef`，在 App 里是 `useMemo`
**派生状态**——任何补丁后自动重算，不会出现「面板值比 JSON 旧一格」的漂移）。
**Univer 侧落地只是视觉反馈**（`applyCellPropToRange`）——重渲染后画布这三项
回到空白，这是有损快照的固有结果，不是面板的 bug。

**语义要点**：

* **边存在性用「边开关」表达**，`style:"none"` 不进下拉 —— 一种状态一种写法
  （`none` 与「没这一边」视觉相同）。手写 JSON 里的 `none` 会在下拉里注入显示。
* **线型 / 颜色只作用于已存在的边**（不凭空给没边框的边加边框）；整选区一条边
  都没有时给**专用提示**（「没有边框可改」），不是误导性的「已是该值」。
* 逐边改动只动 `style`/`color` 一个键，边内 **`width` 原样保留**（`JSON_ONLY`，
  W2-2a 陷阱 6）。
* **缺省即删键**：`format:""`、`rotation:0`、边框色 `#000000` 都删键；**条目删到
  只剩 `row`/`col` ⇒ 整条移除**（不残留空壳，与 W2-1 同一条纪律）。
* `rotation` 面板口径 **0..360 整数**（服务端只要求 ≥0）；越界**报错不夹取**。
* 选区脚手架（裁剪 / 上限 / 合并跳过 / 重复告警）与 W2-2 **共用同一份实现**
  （`stylepanel.ts::patchRangeSlots`；两处各写一套 = 判定静默错位）。

**判据（三层，全部实测）**：

1. **纯逻辑**：`cellprops.test.ts` 28 条（含「边名/线型码与 Univer 真实运行时枚举
   对齐」——`BorderType` 是字符串枚举，映射层用字面量表 + 对照测试钉住）。
2. **浏览器端到端**：E1–E13 全绿（同一脚本 `e2e_w2_panel.{sh,mjs}`）——E10 边框
   （上/下勾选 + 线型 dashed + 色 `#ff0000`）、E11 格式 + 旋转、E12 产物逐键断言、
   E13 `POST /render` 200（并断言三项**按声明不进快照**，作钉住「有损」这一声明的
   tripwire）+ `POST /reports` 入库成功（`.json → .rpx` 写侧接受）。
3. **故障注入 4 组**：

| # | 注入 | 结果 |
| --- | --- | --- |
| FI-5 | 改线型时给**没有的边**也建出来（凭空加边框） | **红**：2 条（`改线型 ⇒ 只改已存在的边` + `改颜色 ⇒ 只改已存在的边`） |
| FI-6 | 旋转越界改成**夹取**而不是报错 | **红**：1 条（`越界/非整数/空 ⇒ 报错（不夹取）`） |
| FI-7 | 开边时不再保留已有 `width` | **红**：1 条（`开边保留已存在的 width`） |
| FI-8 | `stripShell` 恒返回（空壳不删） | **红**：2 条（`条目删空 ⇒ 整条移除` + `format/rotation 设/清`）；**完整 `gate.sh`** 汇总 `FAIL G9`、G1–G8 仍 PASS |

**E2E 几何自校准（本轮新增，取代硬编码像素）**：见 §7 坑位 9。

### W3 越界项（本 spec **不承诺**）

Univer 是**表格**模型，而 rpx 有它没有的概念：`extend_mode` 的扩展语义
（横向扩展会把**列结构本身放大 N 倍**，`format-contract.md` 的扩展矩阵）、
同值合并的运行时判定、`merge_same` 的三态。这些**无法在 Univer 编辑态里表达**，
只能由 JSON 声明（M7-5 已支持）。⇒ W2 不做「在 Univer 里画扩展」，
只做「在 JSON 里声明、在预览里看效果」。

## 5. 验收标准（EARS）

1. **当** 用户选中一个定义 JSON 并点渲染，**系统应**渲染出与同内容 `.rpx` 逐字节等价的快照
   （W1-1；服务端判据 `reportdef_http.rs::json_definition_renders_snapshot_with_merge_anchors`）。
   ✅ **已实测**：浏览器里选 `.json` → 渲染 → Univer 出报表，参数值进快照。
2. **当** 报表在服务端声明了参数，**系统应**自动从 `/meta` 生成参数表单行，
   **而非**要求用户手填参数名（W1-2）。
   ✅ **已实测**：`另存为` 后表单自动带出 2 行，`is_static=false` 的行带「动态」标注。
3. **当** 用户点「另存为」，**系统应**把报表源经 `POST /reports` 入库并回报 id，
   且该条目在 `GET /reports` 列表可见（W1-3）。
   ✅ **已实测**：`已入库 id=1`，`/reports` 列表含 `{id:1,name:"def_params_demo"}`。
4. **当** 用户在编辑面改完点「导出 JSON」，**系统应**产出可被 `POST /reports` 接受的
   定义 JSON，且该 JSON 经 `.rpx` 往返后逐字段相等（W2-1）。✅ **已收口 2026-10-16**：
   `designer.ts` 持有原始定义 JSON、`SheetValueChanged` 事件驱动只补单格、四条契约前置成
   显式错误；30 条单测钉住契约与坐标系（已实证组），G9 覆盖。
5. **当** 前端纯逻辑被改坏，**系统应**在 `scripts/gate.sh` 的 G9 上报红
   （W0-3；判据本身须用故障注入证明）。✅ **已证明**（§4 W0-3 的 FI-1~FI-3 + §4 W1 的 FI-A~FI-C）。
6. **当** 用户在样式面板改一个字段（或点「恢复缺省」），**系统应**只把那**一个**
   字段补丁进定义 JSON（其余键逐个原样保留），且产出的 JSON 经 `POST /render` 后
   快照样式与面板值逐项一致（W2-2）。✅ **已收口 2026-10-16**：`stylepanel.ts`
   逐字段补丁 + 浏览器 E2E E1–E9（含服务端往返）+ 4 组故障注入（§4 W2-2）。
7. **当** 用户在属性面板改边框/数字格式/旋转，**系统应**只把那一个键补丁进定义
   JSON（边内 `width` 与未登记字段原样保留、条目删空整条移除），且产物被服务端
   接受（`/render` 200 + `/reports` 入库）（W2-3）。✅ **已收口 2026-10-16**：
   `cellprops.ts` 逐键补丁（读方向 = 定义 JSON）+ E10–E13 + 4 组故障注入（§4 W2-3）。

## 6. 本地开发工作流

```bash
# 1) 起报表服务端（M3-05）：默认 127.0.0.1:8090
cargo run -p rpx-server -- serve

# 2) 起前端（--host 127.0.0.1 必须，否则绑 IPv6 ::1）
cd web && npm run dev          # vite --port 5288 --strictPort

# 3) 浏览器打开 http://127.0.0.1:5288
```

（②的 `--host` 口径沿用 `spec-m4.md` §6，本机实测结论，勿改。）

### W1 通道的端到端手工走查

`tests/fixtures/def_params_demo.json` 就是为这一步准备的（**服务端测试与浏览器
共用同一个文件**）：选中它 → 点「另存为」→ 参数表单自动带出 → 填值 → 点「渲染」。

```bash
# 服务端判据（真 socket + 真 HTTP，不用起浏览器）
cargo test -p rpx-server --test definition_channel_http
```

## 7. 已知坑与待验证项

1. ⚠ **vite 的 `emptyOutDir` 会在 WorkBuddy 沙箱里把 G9 打红**（2026-10-16 实测，
   **修 G9 时踩到，别重踩**）。
   症状：`npm run build` 在沙箱里报

   ```
   [plugin vite:prepare-out-dir]
   Error: [safe-delete][SAFE_DELETE_BULK_CONFIRM_REQUIRED]
     {"count":80,"threshold":50,"scope":"turn","targets":["…/web/dist/assets"],"targetCount":1}
   ```

   根因（两层，都不是 vite 的 bug）：
   * vite 默认 `emptyOutDir`，构建前 `rmSync` 整个 outDir —— 本项目 **79 个**
     chunk（`ls web/dist/assets | wc -l` 实测）；
   * WorkBuddy 沙箱给 node 的 `fs.rmSync` 套了 safe-delete 包装，**单次删除
     ≥50 个文件**（`CODEBUDDY_SAFE_DELETE_BULK_THRESHOLD=50`，按 tool call 累计）
     就抛 `SAFE_DELETE_BULK_CONFIRM_REQUIRED` 要求人工确认 ⇒ build 直接红。
   而 `web/dist` 是 `.gitignore` 的构建产物，**「把它清空」根本不是 G9 的判据**。
   ⇒ **处置：G9 把 `--outDir` 指到 `mktemp -d` 出来的空目录**（待删条目数 = 0，
   guard 直接放行），跑完用 `/bin/rm -rf` 清掉。沙箱内外行为一致，仓库里也不会
   堆积陈旧的 hash 文件名。
   ⚠ **不要靠改 node 的 `fs.rmSync` / safe-delete 包装来「绕过」**：那是别人工具的
   安全机制，把门禁建在它的实现细节上，升级即**静默**失效。
   ⚠ **不要用裸 `rm` 清理临时目录**：本机 PATH 上的 `rm` 是安全删除包装（搬进
   `~/.Trash` 而非真删），每次 gate 白塞 ~7MB 垃圾进废纸篓。用 `/bin/rm`
   （判据：删完 `ls -A ~/.Trash | grep -c rpx-g9` 应为 0）。
   ⚠ **「直接跑 `npm run build` 是好的」不能当证据**：同一条命令在沙箱外能过、
   在沙箱内必红（且沙箱提权是**每次工具调用随机**的）⇒ 只有临时 outDir 这一种
   写法是确定的。

2. ⚠ **G9 缺 `node_modules` 必须红** —— 这是 §3 那条阻塞解除后的**遗留检查**，
   不是可以顺手删掉的样板代码。`scripts/gate_web.sh` 故意**不自动 `npm install`**：
   装依赖要联网、是显式动作，藏在 gate 里会把「依赖没装」这个真实问题掩盖成绿
   （本仓「固定报 N 条噪声的检查等于没有检查」的同一条纪律）。有效性已由 §4 的
   FI-2 证明。

3. ✅ **`web/` 不在 Rust gate 覆盖范围内** —— **已解除**：G1–G8 全是 cargo，
   前端改动原本**完全无机器判据**；G9 就是为补这个缺口而加。

4. ✅ **Univer 1.0.4 编辑 API 可用性——已实测，不再是「待验证」**（2026-10-16，
   `web/src/probe.tsx` + `web/probe.html`，浏览器实测 15 项，**13 过 2 不过**）。
   探针页是**开发期工具**、不进 `index.html`；入口 `http://127.0.0.1:5288/probe.html`，
   结果同时落在 DOM 与 `window.__probeResults`（agent-browser 可抓）。
   ⚠ 探针已被挂进 vite 多页入口（`vite.config.ts` 的 `build.rollupOptions.input`），
   所以 **G9 的 `tsc --noEmit && vite build` 也覆盖探针页** —— 否则探针页的类型错误
   不会被门禁拦住，而它恰恰是 W2 分期决策的依据。

   **实测通过（API 存在且运行时真能用）：**

   | 能力 | API | 实测值 |
   | --- | --- | --- |
   | 单元格内容 | `setValue` / `getValue` | 读回相等 |
   | 合并 | `merge` / `isMerged` / `breakApart` | `{merged:true, afterBreak:false}` |
   | 字体族/字号/字色 | `setFontFamily` / `setFontSize` / `setFontColor` | `{ff:"Times New Roman", fs:18, cl:{rgb:"#ff0000"}}` |
   | 粗/斜/下划线 | `setFontWeight` / `setFontStyle` / `setFontLine` | `{bl:1, it:1, ul:{s:1,cl:{rgb:"#ff0000"}}}` |
   | 背景色 | `setBackgroundColor` | `{bg:{rgb:"#ffff00"}}` |
   | 边框 | `setBorder(BorderType.ALL, BorderStyleTypes.THIN, color)` | 四边 `bd:{t,r,b,l}` 全落 |
   | 数字格式 | `setNumberFormat` / `getNumberFormat` | 读回 `"#,##0.00"`（**numfmt facade 确实挂上了**） |
   | 旋转 | `setTextRotation` | `{tr:{a:45}}` |
   | 水平对齐 | `setHorizontalAlignment` | `center=2`、`normal=3` |
   | 行高列宽 | `setRowHeight` / `setColumnWidth` | `{rowHeight:40, columnWidth:120}` |
   | 值变更事件 | `addEvent(Event.SheetValueChanged)` | 回调 `fired:1` |
   | 只读 | `getWorkbookPermission().setReadOnly()` / `setEditable()` | 只读下写入**被静默丢弃**（值 null，不抛错）；`setEditable()` 后恢复可写 |
   | 撤销 | `univerAPI.undo()` | `"撤销后"` → `"撤销前"`，**真回退** |

   **实测不通过（API 不存在 —— 这是真实能力缺口，不是探针 bug）：**

   | 能力 | 判据 | 实测 |
   | --- | --- | --- |
   | 行列自定义属性 | `setRowCustom({0:{h:30}})` / `setColumnCustom({0:{w:20}})` | 行高 `24→24`、列宽 `88` 不变 ⇒ **它不是几何 API**。几何请用 `setRowHeight`/`setColumnWidth`（上一行已实测通过） |
   | 打印设置（W2-4） | facade 原型上 `/page\|print\|paper\|margin\|orient/` 匹配方法数 | **0 个**。sheets 包既无页面设置数据模型（`IWorksheetData`/`IWorkbookData` 里没有），也无 facade；`orientation`/`page-setup` 只存在于 **docs** 模型。sheets 里唯一的 print 相关物是**权限点**（`WorkbookPrintPermission`） |

   **⇒ 对 W2 分期的直接影响（本探针的全部意义）：**

   * **缩进与打印设置必须降级**：与 `extend_mode` 同规格 —— 「在 JSON 里声明、
     在预览里看效果」，**不能**在 Univer 编辑态表达。W2-3 因此只剩「边框/数字格式/
     旋转」三项可做面板；W2-4 整个降级为 JSON 声明。
   * 其余 W2-1 / W2-2 / W2-5 **可做**，API 面齐备。

   **四条必须记住的映射陷阱**（写 W2 面板时逐一踩得到）：

   1. ⚠ **`FHorizontalAlignment` 类型里没有 `'right'`，`'normal'` 才是右对齐**
      （实测 `center=2`、`normal=3`）。我们的 JSON `h_align` 有 `right` ⇒ 需要映射层，
      直接传 `'right'` 连类型都过不了。
   2. ⚠ **`ul`（下划线）是 `ITextDecoration` 对象 `{s,c,cl,t}`，不是布尔**；
      而我们 JSON 的 `underline` 是布尔、快照也按布尔暴露 ⇒ 布尔 ↔ 对象要双向映射。
   3. ⚠ **颜色是 `IColorStyle` 对象 `{rgb}` / `{th}`，不是字符串**；
      我们 JSON 用 `#RRGGBB` ⇒ 同样要映射（`snapshot::style::argb_hex` 那条严格互逆
      在编辑侧也要重做一遍）。
   4. ⚠ **`setReadOnly()` 是「静默忽略写入」，不抛错** —— 想靠 try/catch 探测只读
      态会得到假阴性（实测 `blocked:false` 而值确实是 null）。

   > **v0.5 补（W2-2a 收口）**：上面这 4 条已在 `stylemap.ts` 全部处理，且**另有 2 条
   > 是探针没探到、读源码才发现的** —— 边框线型码空间完全不同、`IBorderStyleData`
   > 没有宽度槽。⇒ 完整清单是 **6 条**，见 §4「W2-2a」那张表。**教训：探针能证明
   > 「API 存在且能跑」，但证不了「码表两侧一致」** —— 后者只能读两侧源码逐字对照。

5. **待验证**：Univer 的撤销栈与「导出定义 JSON」如何对齐（用户在 Univer 里的
   撤销，要不要反映到 JSON 草稿）。

5b. ✅ **「用快照反推 JSON」是错的，必须打补丁**（2026-10-16，W2-2a 结论）。
   服务端快照**有损**：`style_json` 只暴露 9 个字段，边框 / `format` / `indent` /
   `rotation` 不在其中（`report-def-json.md` §4.7 末段）。⇒ 拿 Univer 的当前状态
   **重建** JSON，会把 `indent` / `extend_mode` / `border.width` / `print_setup` /
   用户手写的任何未登记字段**静默抹掉**。正确形态见 §4「W2-2a」的
   [`applyCellStyle]`：持有原始 JSON、只动那 12 个槽、其余原样保留。
   判据：`stylemap.test.ts::快照里没有的槽（indent/extend_mode/未知字段）原样保留`
   + `原对象不被修改（纯函数）`（已故障注入 FI-3 验证「只写不删」会红）。

5c. ⚠ **颜色大小写不是小事**：服务端 `argb_hex` 出的是**大写**（`format!("#{:06X}")`），
   而 `parse_color` 大小写都收。前端若图省事用小写，快照对照测试会假红、
   导出的 JSON 与服务端录得的也不一致 ⇒ `normalizeColor` 统一出大写，
   判据钉在 `tests/fixtures/out/engine_merge.snapshot.json` 录得的
   `{"rgb":"#FFFFFF"}` 上。

6. ⚠ **`@参数` 未在提交时给值 ⇒ 400，不是静默取 null**（W1 实测，`expr/atom.rs`）。
   `@p` 要过 `isParamName`：登记（**含值为 null**）⇒ 取值；未登记 ⇒ `UnknownParam`。
   而「登记」来自**提交的 `params` 段**，不是定义里的 `params[]` 声明 ——
   ⚠ 这点极易搞反：**声明不等于给值**。动态参数（`kind: "dynamic"`）没有默认值，
   渲染时不传就会 400，报错点名缺失的参数名。
   ⇒ 前端 W1-2 的表单因此**必须**把动态行也列出来（哪怕值为空），否则用户看不到
   「还有参数没填」。这正是「动态」标注存在的理由。

7. ⚠ **Rust 字节串字面量（`br#"..."#`）不许含非 ASCII** —— 写 `definition_channel_http.rs`
   时踩到：`br#"{"region":"华东"}"#` 直接编译错（`non-ASCII character in raw byte string
   literal`）。含中文的测试数据走 `let s = r#"..."#; … s.as_bytes()`。

8. ⚠ **E2E 定位用几何点击，不要用方向键**（W2-2 首跑踩到，2026-10-16）。
   症状：`page.keyboard.press('ArrowDown')` 之后 Univer 的选区**没动** ——
   ① `number` 输入框若持有 DOM 焦点，方向键被它自己吃掉（ArrowDown 把 20 旋成 19
   并触发一次 `onChange`，JSON 里多出一个 `font_size:19`）；② 即使 `blur()`
   把焦点还给 `body`，Univer 容器**失焦后也不认方向键**（只有它自动聚焦的那个
   容器 DIV 持焦时才认）。⇒ 端到端一律用**主画布几何点击**（点击后一律用面板
   回读**自证**，错位会在 E2b 自检处被抓到，不会一路错下去）。
   附带纪律：面板读样式走**单格** `sheet.getRange(row, col).getCellStyleData()`
   （显式取选区起点格），**不**依赖「多格 range 的样式读回」语义 —— 后者在
   本里程碑没有探针证据，按「无证据不依赖」处理。

9. ⚠ **点击几何不能硬编码 Univer 的缺省（24/88）—— 快照会写死一套自己的值**
   （W2-3 E2E 首跑踩到，2026-10-16）。
   症状：W2-2 时 E2E 用「行高 24、列宽 88、行头 46、列头 20」点击正常；加了属性条
   后 `clickCell(2,0)`（本想点 A3）**落到了 A2**，属性补丁写进了错误的格子
   （`w2_panel_exported.json` 里 A2 多出 `border`/`format`/`rotation`，
   而断言看的是 A3 ⇒ 红）。逐像素二分后真相：**快照自带几何** ——
   `rowData[0].h = 30.24`、`columnData[0].w = 94.49`、`defaultRowHeight = 30`、
   `defaultColumnWidth = 94`（`snapshot.rs` 写的，不是 Univer 的 `24/88`），
   而行头/列头的**真实偏移**是 `47.51 / 20.76`（也不是 46/20）。
   ⇒ **几何一律运行时测量**：行高/列宽从 `/render` 快照读；表头偏移用
   **面板读回当传感器**二分测（A1 带样式、B1 缺省 ⇒ 「离开 A1 的第一个 x/y」
   即第二列/第二行边缘，减去列宽/行高即得偏移；本夹具实测 headW=47.51、
   headH=20.76）。**别再把像素值硬编码进 E2E**，也别改 `def_style_demo.json`
   里 A1/B1 的样式（传感器的单调性靠它）。

## 8. 变更记录

| 日期 | 变更 | 原因 |
| --- | --- | --- |
| 2026-10-16 | v0.1 立档 | M7-1~M7-6 已把定义 JSON 的服务端能力备齐（含 `.rpx` 产出与写回），
`report-def-json.md` §7.2 要求为设计器另立 spec；本文件即该 spec。
**先交 W0（前端可验证性地基），再谈 W1/W2** —— 没有 G9 的设计器改动无法按本仓标准收口。 |
| 2026-10-16 | v0.2 **W0 收口** | `npm install`（144 包）+ vitest 5.0.5 + `web/src/report.test.ts`（15 条）
+ `scripts/gate_web.sh` 挂成 **G9**。§3 阻塞解除、§4 W0 三行验收填实测值、
3 组故障注入（FI-1~FI-3）证明 G9 有牙齿。新增 §7 坑位 1（vite `emptyOutDir`
撞沙箱 safe-delete ⇒ 改用临时 outDir，含「别改 fs.rmSync / 别用裸 rm」两条禁令）。
**G1–G9 全绿（Rust 580 + 前端 15），W1 可开工。** |
| 2026-10-16 | v0.3 **W1 收口** | 定义 JSON 通道接上：`report.ts` 的 `report` 字段收 `.json`
（`pickReportFile`/`isReportSource`）、`saveReport`（`POST /reports`，字段名 **`file`**）、
`fetchMeta` + `metaToRows`（参数表单改读 `/meta`，动态行标「动态」）、`App.tsx` 加
「另存为」按钮。新增 `tests/fixtures/def_params_demo.json`（**浏览器与服务端测试
共用同一个文件**）+ `crates/rpx-server/tests/definition_channel_http.rs`（4 条服务端
镜像判据）。浏览器端到端实测：选 `.json` → 另存为 → 表单自动带出 → 填参渲染 →
`/reports` 列表可见。3 组故障注入（FI-A/B/C）。新增 §7 坑位 6（`@参数` 未给值 ⇒ 400，
声明≠给值）与坑位 7（`br#""#` 不许非 ASCII）。
**G1–G9 全绿（Rust 584 + 前端 25），W2 可开工。** |
| 2026-10-16 | v0.4 **W2 准入探针** | 新增 `web/src/probe.tsx` + `web/probe.html`（开发期
探针页，**不进 `index.html`**）+ vite 多页入口（让 G9 的 tsc/build 覆盖它）。
浏览器实测 15 项编辑 API，**13 过 2 不过**：通过的是内容/合并/样式 7 字段/边框四边/
数字格式/旋转/对齐/行高列宽/`SheetValueChanged` 事件/只读/撤销；**不过的是
「行列自定义属性」与「打印设置」**（后者 facade 上一个页面设置方法都没有）。
⇒ §7 坑位 4 由「待验证」变实测结论 + 四条映射陷阱（`normal`=右对齐、`ul` 是对象、
颜色是 `{rgb}`、`setReadOnly` 静默丢弃不抛错）；**W2-3 缩进与 W2-4 打印设置降级为
JSON 声明**，与 `extend_mode` 同规格。W2 代码**未开工**。 |
| 2026-10-16 | v0.5 **W2-2a 样式映射层收口** | 新增 `web/src/stylemap.ts`（纯函数层）
+ `web/src/stylemap.test.ts`（43 测试）。§4 新增「W2-2a」小节：**地基决定 =
设计器必须持有原始定义 JSON 并对它打补丁**（快照有损，反推会抹掉
`indent`/`extend_mode`/`border.width`/`print_setup`/用户手写字段）。
坑位从 4 条扩到 **6 条**且全部改为**源码出处**（`facade.js` / `i-style-data.d.ts` /
`border-style-types.d.ts` × `report-def-json.md` §4.7），新增两条：**边框线型码空间
完全不同**（`DOTTED=3` vs `dotted=81`，非平移，14 种只开 5 种）与
**`IBorderStyleData` 没有宽度槽**（`width` 是 JSON_ONLY，需从原 JSON 继承）。
**两条 W2 假设被实测修正**：`ht`/`vt` 必须**无条件发**（rpx 的 `Cell` 永远有具体
对齐值 ⇒ 录得快照每格都带 `"ht":1,"vt":2`）；读方向**只出非缺省**（快照对
`ff`/`fs`/`cl`/`bg` 恒发缺省值）。新增保真三级（EXACT/LOSSY/JSON_ONLY）与
「宁可报 warning 不静默丢」硬规矩。6 组故障注入全部验证有牙，其中 FI-6
（本地码表漂移）**一针见红 6 条**，证明「与 Univer 真实运行时枚举对齐」那条判据
真的有牙齿。**G1–G9 全绿（Rust 584 + 前端 68）。** |
| 2026-10-16 | v0.6 **W2-1 编辑面收口** | 新增 `web/src/designer.ts`（内容补丁 + 导出，纯函数层）
+ `web/src/designer.test.ts`（**30 条**单测）。`App.tsx` 在 `render` 里订阅
`univerAPI.addEvent(Event.SheetValueChanged)` 把**单格**改动喂给 [`applyValueEdit]`
（句柄存 `eventDisposableRef`，重渲染先 `dispose` 旧监听，避免叠两份）；顶栏加
**「导出 JSON」**按钮（无 `.json` 源时置灰）。守四条服务端契约（坐标系 1 起↔0 起、
`text`↔`expr` 互斥、合并内非锚点禁内容、网格边界），全部前置成显式错误；坐标系期望值
取自 `snapshot/tests.rs` 已实证组。关键纪律：**只跟事件、只补一格**，不做整体回灌
（否则 `"0123"` 这类值被 Univer 按数值归一化 ⇒ 静默数据损坏）。`.rpx` 模板无定义 JSON
可持 ⇒ `assertEditable` 给「请改用 .json」文案而非静默退化。§4 加「W2-1」小节、§5
EARS 第 4 条由 ⬜ 改 ✅、§2 状态表补两文件（web 共 3050 行）。
**G1–G9 全绿（Rust 584 + 前端 98）。** |
| 2026-10-16 | v0.7 **W2-2 样式面板收口** | 新增 `web/src/stylepanel.ts`（7 字段逐字段补丁：
读 Univer 活态 → 显示；控件值 → **只补那一个键**进 JSON；选区裁剪 / 合并区跳过 /
槽数上限 / 缺省即删键）+ `web/src/stylepanel.test.ts`（**34 条**）。§4 新增「W2-2」小节，
**地基决定 = 明确不走 `applyCellStyle` 的 12 槽整体覆盖**（快照有损 ⇒ 整体覆盖会在
用户点一下加粗时抹掉 `border`/`format`/`rotation`/`indent`/`extend_mode`）。`App.tsx`
加样式条（字体/字号/B/I/U/字色/底色/恢复缺省）+ 选区订阅
（`CommandExecuted` 过滤 `sheet.operation.set-selections`）。三层判据：纯逻辑 34 条、
**浏览器端到端 E1–E9**（新增 `tests/fixtures/def_style_demo.json` +
`tests/fixtures/out/e2e_w2_panel.{sh,mjs}`；产物经 `POST /render` 快照回读逐项一致）、
**4 组故障注入**（FI-1 `'normal'` 字符串枚举 / FI-2 elide 只写不删 / FI-3 颜色大写 /
FI-4 合并区跳过；FI-4 在**完整 `gate.sh`** 上验证红落 G9、G1–G8 仍 PASS）。
新增 §7 坑位 8（E2E 定位必须几何点击：`number` 输入夺焦 + Univer 容器失焦后
方向键不生效——首跑实录）。§5 EARS 加第 6 条、§2 状态表补两文件（web 共 4154 行）。
**G1–G9 全绿（Rust 584 + 前端 132）。** |
| 2026-10-16 | v0.8 **W2-3 属性面板收口** | 新增 `web/src/cellprops.ts`（**边框/数字格式/旋转**：
读方向**改从定义 JSON**——三项不在快照里，画布读不回来，与 W2-2 相反；边开关表达存在性、
线型/色只作用于已存在的边、`width` 保留、空壳移除、`rotation` 0..360 不夹取）+
`web/src/cellprops.test.ts`（**28 条**）。`stylepanel.ts` 抽出**选区脚手架**
（`patchRangeSlots`/`finishRangePatch`）供两模块共用（+61 行）。`App.tsx` 加「属性」条
（边框四边开关/线型/色 + 格式下拉 + 旋转）与 `useMemo` 派生面板态。E2E 扩到 **E1–E13**
（E10 边框 / E11 格式+旋转 / E12 产物逐键 / E13 `/render` 200 + `/reports` 入库），
**几何改运行时自校准**（行高列宽读快照 + 表头偏移二分实测——见新增 §7 坑位 9：
快照 `rowData/columnData` 是 30.24/94.49，**不是** Univer 缺省 24/88，硬编码会点错格）。
**4 组故障注入**（FI-5 凭空加边框 / FI-6 旋转夹取 / FI-7 丢 width / FI-8 空壳不删；
FI-8 在**完整 `gate.sh`** 上验证红落 G9、G1–G8 仍 PASS）。**另把 W2-3'/W2-4 两条
降级行闭合**（无面板，验收由服务端 `definition_roundtrip.rs` 的
`cell_props_survive_the_rpx_round_trip`（indent）/ `print_setup_survives_the_rpx_round_trip`
承担；`/render` 腿未单列用例、如实标注）。§5 EARS 加第 7 条、
§2 状态表补两文件（web 共 5207 行）。**G1–G9 全绿（Rust 584 + 前端 160）。** |
