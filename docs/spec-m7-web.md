# Spec — ReportLite Rust 重实现 · M7-W（Univer 前端设计器）v0.1

> 状态：**立档**（2026-10-16）。上游决策：`docs/report-def-json.md` §7.2 明写
> 「在线设计器（`web/` 侧）—— Univer 目前仍是**纯渲染终端**，在它上面做『定格式』
> 是**新阶段**，须另立 spec」—— 本文件就是那份 spec。
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

## 2. 当前状态（2026-10-16 实盘）

`web/` 共 **306 行** TypeScript，4 个源文件：

| 文件 | 行数 | 职责 |
| --- | --- | --- |
| `App.tsx` | 222 | 顶栏（选文件 + 渲染按钮）+ 参数表单 + 状态栏 + Univer 容器 |
| `report.ts` | 73 | `fetchSnapshot`：文件 → multipart → `POST /api/render` → 快照 JSON |
| `main.tsx` | 10 | 挂载入口 |
| `vite-env.d.ts` | 1 | 类型声明 |

技术栈（`package.json`）：Vite 8 + React 19 + **Univer 1.0.4**
（`@univerjs/preset-sheets-core` + `@univerjs/presets`，zh-CN locale）。

**M7-1 ~ M7-6 已在服务端备好的能力**（本 spec 的立足点）：

| 服务端能力 | 落地 | 前端现状 |
| --- | --- | --- |
| 定义 JSON 是 `/render` 的一等输入 | M7-1 | ❌ 只发 `.rpx`（`report.ts` 只找 `.rpx` 后缀） |
| 定义 JSON 可归一化为 `.rpx` 入库 | M7-2（`POST /reports`） | ❌ 前端不调 `/reports` |
| 参数清单可由 `/reports/{id}/meta` 读回 | M7-4 | ❌ 参数表单仍是**手工填名值行**（M4-04） |
| 单元格样式 7 字段 ↔ 快照严格互逆 | M7-3 | ✅ 只读展示，无编辑 |
| 边框 / 格式 / 缩进 / 旋转 / `extend_mode` | M7-5 | ❌ **快照不暴露**（`spec-m4.md` §4.3 后置） |
| 打印设置覆盖（17 字段） | M7-6 | ❌ 快照不暴露 |

⇒ **前端的缺口不是「服务端还不支持」，而是「服务端已支持、前端没接」**。
M7-W 的第一阶段因此不是「做设计器」，而是**把已有通道接上**（§4 的 W1）。

## 3. 阻塞项（先解决，否则无法验收）

⚠ **本机 `web/` 无 `node_modules`**（2026-10-16 实测：目录不存在，
`@univerjs/presets` 未安装）⇒ **前端无法 `tsc` / `vite build` / 跑任何测试**。
且 `package.json` **没有测试运行器**（无 vitest / jest）。

这与本仓的验收纪律直接冲突：**「判据有效性一律用故障注入证明」**
（`docs/code-review.md`）。没有可跑的 TS 测试，前端改动就是**不可验证的改动**。

⇒ **M7-W 的准入条件是先补上前端可验证性**（§4 的 W0），否则任何后续阶段都
无法按本仓标准收口。这一步不做完，后面全是「看着对但没证据」。

## 4. 分期计划

### W0 前端可验证性地基（**准入条件**）

| 项 | 内容 | 验收 |
| --- | --- | --- |
| W0-1 | `npm install` 装齐 Univer 1.0.4 / Vite 8 / React 19 | `npm run build`（`tsc --noEmit && vite build`）退出 0 |
| W0-2 | 引入 vitest + 一个纯逻辑测试（如 `report.ts` 的参数行推断） | `npx vitest run` 有 ≥1 条绿 |
| W0-3 | 把 `npm run build` + `npx vitest run` 挂进 `scripts/gate.sh`（新 gate **G9**） | 故意改坏一个纯函数 ⇒ G9 红 ⇒ 改回 ⇒ 绿（**故障注入证明 G9 有牙齿**） |

W0-3 的判据有效性必须用故障注入证明，与 G1–G8 同规。**G9 未绿之前 M7-W 不进 W1。**

### W1 定义 JSON 通道（前端接入既有服务端能力）

| 项 | 内容 | 验收 |
| --- | --- | --- |
| W1-1 | `report.ts` 接受 `.json` 定义文件（与 `.rpx` 并列选一个） | 选中 `.json` ⇒ `POST /api/render` 200 且渲染出报表 |
| W1-2 | 参数表单改读 `GET /reports/{id}/meta`（M7-4 的清单），不再手工填名 | 清单非空 ⇒ 自动出行；`is_static=false` 的行标注「动态」 |
| W1-3 | 「另存为」：定义 JSON → `POST /api/reports` → 拿 id → 提示入库成功 | 响应 200 且 `/reports` 列表可见 |

W1 的全部判据是**端到端**（起真服务端 + 真 HTTP），与 `params_http.rs` 同口径。

### W2 编辑面（真正的「设计器」）

| 项 | 内容 | 验收 |
| --- | --- | --- |
| W2-1 | Univer 开编辑（`readonly: false`），单元格内容/合并可改 | 改完导出定义 JSON，`open_bytes(to_bytes(build(json))) == build(json)` 往返成立 |
| W2-2 | 样式面板：字体/字号/颜色/粗斜下划线（M7-3 的 7 字段） | 面板设的值原样进 JSON |
| W2-3 | 边框 / 格式 / 缩进 / 旋转面板（M7-5 的 16 槽） | 同上；**往返判据**（快照不暴露，见 §2 表） |
| W2-4 | 打印设置面板（M7-6 的 17 字段） | 同上 |
| W2-5 | 网格尺寸 / 行列属性 / 数据集绑定 | 与 `report-def-json.md` §3 字段一一对应 |

W2 的判据形态是 **M7-5 已立下的模型往返**：面板写出的 JSON 经
`parse → to_bytes → open_bytes` 必须逐字段相等（`tests/definition_roundtrip.rs`
的同一条判据，只是输入从手写 JSON 变成面板产物）。

### W3 越界项（本 spec **不承诺**）

Univer 是**表格**模型，而 rpx 有它没有的概念：`extend_mode` 的扩展语义
（横向扩展会把**列结构本身放大 N 倍**，`format-contract.md` 的扩展矩阵）、
同值合并的运行时判定、`merge_same` 的三态。这些**无法在 Univer 编辑态里表达**，
只能由 JSON 声明（M7-5 已支持）。⇒ W2 不做「在 Univer 里画扩展」，
只做「在 JSON 里声明、在预览里看效果」。

## 5. 验收标准（EARS）

1. **当** 用户选中一个定义 JSON 并点渲染，**系统应**渲染出与同内容 `.rpx` 逐字节等价的快照
   （W1-1；服务端已有判据 `reportdef_http.rs::json_definition_renders_snapshot_with_merge_anchors`）。
2. **当** 报表在服务端声明了参数，**系统应**自动从 `/meta` 生成参数表单行，
   **而非**要求用户手填参数名（W1-2）。
3. **当** 用户在编辑面改完点「另存为」，**系统应**产出可被 `POST /reports` 接受的
   定义 JSON，且该 JSON 经 `.rpx` 往返后逐字段相等（W2-1）。
4. **当** 前端纯逻辑被改坏，**系统应**在 `scripts/gate.sh` 的 G9 上报红
   （W0-3；判据本身须用故障注入证明）。

## 6. 本地开发工作流

```bash
# 1) 起报表服务端（M3-05）：默认 127.0.0.1:8090
cargo run -p rpx-server

# 2) 起前端（--host 127.0.0.1 必须，否则绑 IPv6 ::1）
cd web && npm run dev          # vite --port 5288 --strictPort

# 3) 浏览器打开 http://127.0.0.1:5288
```

（②的 `--host` 口径沿用 `spec-m4.md` §6，本机实测结论，勿改。）

## 7. 已知坑与待验证项

* ⚠ **本机 `web/` 无 `node_modules`** —— 这是 §3 的阻塞项，也是 W0 的第一件事。
  2026-10-16 实测：`ls node_modules` 无输出、`@univerjs/presets` 未安装。
* ⚠ **`web/` 不在 Rust gate 的覆盖范围内**（G1–G8 全是 cargo）⇒ 前端改动目前
  **完全无机器判据**。W0-3 的 G9 就是为了补这个缺口。
* **待验证**：Univer 1.0.4 的 `readonly: false` 是否支持本 spec W2 所需的全部
  编辑面（合并单元格的 API、`IStyleData` 的写入口径）。**装完依赖后先做一次
  API 可用性探针**，再决定 W2 的分期是否要调整 —— 不要先写代码再发现 API 不支持。
* **待验证**：Univer 的撤销栈与「导出定义 JSON」如何对齐（用户在 Univer 里的
  撤销，要不要反映到 JSON 草稿）。

## 8. 变更记录

| 日期 | 变更 | 原因 |
| --- | --- | --- |
| 2026-10-16 | v0.1 立档 | M7-1~M7-6 已把定义 JSON 的服务端能力备齐（含 `.rpx` 产出与写回），
`report-def-json.md` §7.2 要求为设计器另立 spec；本文件即该 spec。
**先交 W0（前端可验证性地基），再谈 W1/W2** —— 没有 G9 的设计器改动无法按本仓标准收口。 |
