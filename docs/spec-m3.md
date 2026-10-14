# Spec — ReportLite Rust 重实现 · M3（rpx-ds 数据源 + rpx-server 主链路）v0.4

> 状态：**阶段 1 已完成 + M3-02 GBK 已闭合**（2026-10-14 深夜：M3-01 参数通道闭环 —— `engine_param` 夹具
> 进 16 夹具主对拍全绿，RUNLOG §24）。上游：M2 已按「务实路」定案（`spec-m2.md` §6.2 末条）。
> 设计蓝图（L1 层）：`docs/reportlite-rust全链路设计.html` 的 **§七（数据源）/ §八（服务端 API）**；
> 本文件是其「任务与验收」下位文档 —— **冲突时以证据（夹具 > 源码 > 契约文档）为准**。

## 0. 本文件的可信度声明

与 `spec-m2.md` §0 同规：凡未附证据的断言一律标注「待验证」，不得当结论引用。
本文件定义 **M3 做什么、怎么验收、边界在哪**；实现细节与算法以 `docs/` 正文与夹具为准。

## 1. 产品定义

M3 = 把 M1/M2 的**库能力**变成可用的**服务**：

> **POST /render：Turso 读报表资产 + AnyPool 连业务库取数 → 出快照；参数注入生效**
> （`reportlite-rust全链路设计.html` §13 的 M3 行原文）

三个「接线 / 扩展」（都不是重写引擎）：

1. **参数通道**（引擎接线）：M2 的实现里参数值通道是**空的**（`eval_ctx.rs:198` 返回 `None`），
   而 M3 验收要求「参数注入生效」⇒ 第一号任务（M3-01）。
2. **真实分页口径**：M2 的 `pagination` 段是**探针口径**（纸张被 `ReportUtils2.getPagerInfo`
   覆盖为 595×842、边距清零，`pager.rs` 常量区有逐条注释）⇒ `/render` 需按模板 `print_setup` 出分页（M3-06）。
3. **数据集中文编码**：M2 只解 ASCII（`dataset.rs` 模块文档），GBK 在财政场景必现 ⇒ M3 承接（M3-02）。

## 2. 范围切分

### 2.1 做（按依赖序）

| 编号   | 任务                                                                       | 落点                        |
| ---- | ------------------------------------------------------------------------ | ------------------------- |
| M3-01 | 参数通道：`calculate_with()` 参数入口 + **建树期内联**（`parse_with`）+ 参数夹具 ✅ | `rpx-engine`              |
| M3-02 | 数据集编码：GBK/GB18030 解码 + `FileDataSetConfig` 高级槽位核对                        | `rpx-engine`（或随取数迁移至 `rpx-ds`） |
| M3-03 | `rpx-ds`：`trait DataSource` / 方言层 / 数据集物化 / cross（蓝图 §七）                 | 新 crate                   |
| M3-04 | 报表资产库：libSQL 本地模式（`.rpx` BLOB + 元信息 + 参数清单）                                | 新 crate（server 内或 `rpx-ds`） |
| M3-05 | `rpx-server`：axum 路由 + `POST /render` 主链路                                 | 新 crate                   |
| M3-06 | 分页真实口径：`/render` 按 `print_setup` 驱动 `pagination`（探针口径仅为对拍保留）               | `rpx-engine` 接口 + server 接线 |
| M3-07 | 门禁扩展（G1–G7 覆盖新 crate + 依赖分层 G5）+ e2e 冒烟                                  | `scripts/gate.sh` + tests |
| M3-08 | 快照协议：Univer `IWorkbookData` 映射（含**样式直通**；动态样式宏后置）⚠ **提为关键路径，v1 先行**（契约已定 `spec-m4.md` §4） | `rpx-server`（dev bin + HTTP 双通道均已就绪） |

### 2.2 明确不做

- **不做远端同步 / replica**：libSQL 只用本地单文件（`new_local`）——蓝图 §7.1 的部署边界：
  单实例、私有化、离线可用。
- **不做鉴权体系**：私有化单实例内网部署；如后续需要再立任务。
- **M4（Univer 前端）并行推进**（2026-10-14 总监令「M3 M4 一起推进，这样可以看到报表显示」；
  前端立档 `docs/spec-m4.md`）——本 M3 spec 仍不产前端代码，但 **M3-08 快照 v1 提为关键路径**
  （M4 的唯一输入接口；开发期由 `rpx-server` dev bin 直出，见 `spec-m4.md` §2/§6）。
- **不做 xlsx/PDF 导出**（M5）、**不做 BigEngine 大报表分页**（M6）。
- **不实现样式宏**（`_$9()` / 图表宏 / `perfect`）：快照先做**模板静态属性直通**；
  动态样式宏与 M2-22 残余同账（见 §5）。

## 3. 技术架构

### 3.1 crate 布局（新增两个）

```text
rpx-core ── rpx-engine ──┬── rpx-cli（已有，M1/M2 对拍入口）
   │                     └── rpx-server（M3-05 起含 HTTP 面：axum `POST /render` + dev bin）
   └── rpx-ds（新，数据源层）
```

- `rpx-ds`：`trait DataSource` + SQL 方言层 + 数据集物化 + cross（蓝图 §七：
  AnyPool 换库只换 URL；`SqlDialect` 收敛占位符/引号/LIMIT/布尔/日期差异）。
  依赖：`rpx-core` + `sqlx(any)` + `libsql`。
  ⚠ **待定**：物化产物类型 `DataSet` 当前住在 `rpx-engine::expr::ds`——
  「下沉到 `rpx-core`」vs「`rpx-ds` 依赖 `rpx-engine`」二选一，见 §8。
- `rpx-server`：axum 路由 + libSQL 资产库 + 主链路编排（蓝图 §八 的路由表）。
- 门禁 G5（依赖分层）**必须扩展**：为新 crate 定义允许依赖集（M3-07；⛔ 零依赖约束只对
  `rpx-core`/`rpx-engine` 成立，`rpx-ds`/`rpx-server` 本就有依赖）。

### 3.2 主链路（`POST /render` 十步）

蓝图 §8.1 的时序：① 读 bytea → ② `RpxReader` → ③ 建 `ExtCellSet` → ④ fetch 数据集 →
⑤ prepare → ⑥ 扩展 → ⑦ 求值调度 → ⑧ 同值合并 → ⑨ 分页 → ⑩ 快照 JSON。

Rust 侧对应：① libSQL 取 BLOB → ② `rpx_core::open_bytes`（已有）→ ③④ `rpx-ds` 物化
（新）→ ⑤⑥⑦⑧ = `rpx_engine::calculate`（M2 已闭合，**含 pager 守卫**）→
⑨ 真实口径分页（M3-06，替换 `calculate()` 内的探针常量接力）→ ⑩ 快照（M3-08）。

### 3.3 参数通道设计要点（M3-01，**已落地**）

设计论证与实现定案（2026-10-14 晚闭环；证据 RUNLOG §24）：

- 产品是**建树期内联**：`new Expression(cs, null, ctx, expStr, false)` 从 `ctx` 取参数值，
  树根是带值 `Constant`；`@p` 过 `isParamName`、未登记报错（`Expression.java:183-188`；
  真机证据 RUNLOG §15 + §24.1）。
- **实现定案 = 建树期内联**：`parse_with(src, &ParamTable)` 把 `@p` / `@@p` / 裸名
  三种路径全部内联成 `Expr::Const(值)`；`calculate_with(rd, ds, params)` 是带参入口
  （`calculate` 保留为无参转发，等价 `ParamTable::EMPTY`）。
- ⛔ **被证伪的早期设计**（删除）：`Expr::Param { value: Option }` + 求值期 `ctx.param()`
  兜底。`is_extend` 是准备期判定，求值期拿到值救不了扩展 ⇒ 兜底只会掩盖"建树期没绑定"
  的 bug。完整论证见 `params.rs` 模块文档。
- **探针先行**（`ProbeParamFull.java`，RUNLOG §24.1）：空 List / `@@` 未登记 /
  `@p` 未登记 / Types 转换四个缺口的真机答案先于实现钉死。
- 已知限制：参数名形如 `[A-Za-z]+\d+`（`p1`）被 `is_cell_id` 形状判定截胡
  （与 `qty1` 列名同类，等 M2-06 矩阵重解析一并修正；RUNLOG §24.2 末段）。

## 4. 验收标准（EARS）

| 编号   | 功能       | 验收标准                                                                                                                                                                | 级别  |
| ---- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- |
| M3-01 | 参数注入生效   | While `POST /render` 携带 `params`，引擎**必须**按参数值计算：标量参数格不变；**List 参数必须**把该格包成扩展格（矩阵按子格数放大）；未登记参数**必须**报错（等价产品 `unknownParam`）；全部断言**必须**有产品夹具逐格对拍 | P0  |
| M3-02 | 数据集编码    | ✅ **已达成**（2026-10-14）：While CSV 为 GBK 且含中文，列名与值**必须**正确解码（`encoding.rs` 按 `FileDataSetConfig.encoding` 缺省 GBK 解码；`engine_gbk` 夹具中文全链路逐格对拍）；⛔ 不得静默乱码（非法字节显式 `MalformedEncoding`）——GB18030 四字节序列**带账**（§8），与产品默认 `GBK` charset 行为一致 |
| M3-03 | rpx-ds 取数 | While 数据源配置指向 `postgres://` / `mysql://` / `sqlite://`，**必须**经 AnyPool 取到行集并物化为引擎 `DataSet`；换库只换 URL（蓝图 §7.2）                                                        | P0  |
| M3-04 | 报表资产     | While 上传 `.rpx`，**必须**魔数校验后存 libSQL BLOB；`GET /meta` **必须**返回参数清单（静态/动态两套 MetaData）                                                                                  | P0  |
| M3-05 | `/render` 快照 | ✅ **已达成**（2026-10-14）：`POST /render` 返回快照 JSON（multipart：`report`/`dataset`/`params`）；同输入下与引擎库函数直调**逐字节一致**（`tests/render_http.rs` 6 用例：真 socket + 对拍基准）；GBK/参数/缺文件/坏魔数/未知字段全覆盖；浏览器端到端实测通过（截图 `tests/fixtures/out/m4-render-http.png`） | P0  |
| M3-06 | 分页真实口径   | ✅ **已达成**（2026-10-14）：`/render` 的 `pagination` = **模板真值口径**（`paginate_with_setup`：纸码表 80 码 + 模板边距 → 可用 pt，`PagerInfo.getPageFormat(true)` 同构；快照 `pagination` 块见 `spec-m4.md` §4.5）；`pagerStyle != 2`/多栏（`columnMode`/`tableColumnNum`/重复组头）/列区带（`colType != -79`）/可用尺寸非正全部显式 `UnsupportedPaging`；探针口径保留为夹具对拍专用（双口径共用同一套 `split_rows`/`split_cols`）；换行回填**静态守卫不可行**（`-64` = `INormalCell.TYPE_NORMAL` 普通格默认类型，`guard.rs` 尾注） | P0  |
| M3-07 | 门禁与端到端   | G1–G7 **必须**扩到新 crate（含依赖分层）；e2e 冒烟：上传 → render → 断言快照键集                                                                                                               | P0  |
| M3-08 | 快照协议     | 快照**必须**含样式槽（模板静态属性直通）；动态样式宏**必须**在响应中显式标注「未应用」而非静默缺失                                                                                                              | P1  |

## 5. 支持范围声明（继承 M2 带账）

**原则：命中的模板会显式报错或显式标注，不会静默出错。**

| 项                                      | 现状                                | 出处                               |
| -------------------------------------- | --------------------------------- | -------------------------------- |
| 分页 `pagerStyle 0/1` / 多栏两开关           | ✅ 守卫：`UnsupportedPaging`          | `pager.rs::ensure_supported`（2026-10-14） |
| 跨页合并拆分 / 文本换行回填                        | ⏳ 带账（**已声明偏离**：跨页合并 `merged.cells` 全空不可辨；换行测量分支静态不可判——`-64` 是普通格默认类型） | `pager.rs` 模块文档「未实现」清单 + `guard.rs` 尾注（M3-06 核对结论） || M2-22 残余（`customPropEngMap` / cell-graph / state 8） | ⏳ 显式带账（无夹具，不实现）                  | `spec-m2.md` §6.2 末条             |
| 样式宏 / 图表宏 / `perfect`                  | ⏳ 未建模（快照不含其产物）                    | `calc/run.rs` 缺口 2               |
| 空 List 参数                              | ✅ 已实证：矩阵**不变**、锚点 `extend=18 subCells=1`、值 `null` | RUNLOG §24.1（A 组）；`engine_param` 夹具未含空 List（探针已覆盖） |
| **两个扩展格同行**（`@p` + `@@p` 同值为 List）       | ⏳ 显式带账：产品行带交互已实证（11×5，RUNLOG §24.4），本 crate 扩展算法未建模 | `EngineModelsM3.java` 模块文档（为何夹具去掉 `@@p` 列） |

## 6. 依赖与前置

- ✅ M2 出口：15 产品夹具逐格对拍 + G1–G7 + pager 守卫（2026-10-14）
- ✅ 参数语义实证：RUNLOG §15（#17 消歧）+ §24.1（四缺口全景探针，真机已验证）
- ✅ 产品模板参数样本：`full.rpx` 2 参数（`startDate` / `topN`，`Fixtures.java:260-266`）
- ✅ **参数产品夹具**：`engine_param`（第 16 个夹具，注入 `p=List[1,2,3]` / `s=7`；
  `engine_grid_compare` 逐格对拍全绿；Java 侧 round-trip `VERIFY OK: 16`）
- ⏳ 真实口径分页夹具（M3-06）

## 7. 阶段划分

| 阶段 | 内容                                        | 出口                    |
| -- | ----------------------------------------- | --------------------- |
| 0  | 本档 + 参数通道设计 + `rpx-ds` 设计评审               | 本文件经总监确认              |
| 1  | ~~M3-01 参数通道（引擎侧，不依赖新 crate）~~ ✅ 2026-10-14 晚闭环        | ~~参数夹具 + 单测 + 对拍~~：`engine_param` 夹具 + 255 单测 + 7/7 对拍 |
| 2  | ~~M3-02 GBK~~ ✅ + M3-03 `rpx-ds` + M3-04 资产库 | `rpx-ds` 单测 + 取数冒烟    |
| 3  | M3-05 `/render` + M3-06 真实口径（+ M3-08 接 HTTP） | e2e 冒烟                |
| 4  | M3-07 门禁扩展 + 验收                           | G1–G7 扩展后全绿           |
| ∥  | **并行轨 M4**（`docs/spec-m4.md`）：M3-08 v1 先行 → 脚手架 → Univer 装载 → HTTP 联调 | **浏览器看到报表**（M4-01..03） |

## 8. 已知坑与待验证项

| 项                  | 状态   | 说明 / 消歧方法                                                                                                                  |
| ------------------ | ---- | -------------------------------------------------------------------------------------------------------------------------- |
| 参数全景               | ✅ 已消歧 | `ProbeParamFull.java` 四缺口真机答案（RUNLOG §24.1）；「建树期绑定」落点定案为 `parse_with` 期内联（§3.3）                                    |
| 空 List 参数          | ✅ 已实证 | 矩阵不变、锚点 `extend=18 subCells=1`、值 `null`（RUNLOG §24.1 A 组；修正了 §15.3「无子格」的不精确说法）                                  |
| `DataSet` 类型归属     | 待定   | 物化产物住在 `rpx-engine::expr::ds`：下沉 `rpx-core` vs `rpx-ds` 依赖 `rpx-engine`——阶段 0 评审定案（倾向下沉：引擎输入契约属 core 层）                        |
| GBK 实现路径           | ✅ 已定案 | 冻结映射表（`collation.bin` 先例：`include_bytes!` 保零依赖）：`ProbeEncoding.java` dump JDK 全定义域（**23,940 对**，比常见标准表 21,886 项多——JDK 私有区映射）→ `scripts/gen_gbk_table.py` 只加头排序 → `data/gbk.bin`；Rust 单测全表逐对对拍 |
| GB18030 四字节序列       | ⏳ 带账   | 产品默认 `encoding="GBK"` 时 JDK GBK charset 同样不接受四字节（`MalformedInputException`）；模板显式写 `GB18030` 的场景**无夹具**，按"禁止无基准对拍"带账，命中报 `UnsupportedEncoding` |
| 样式直通范围             | 待分类  | 模板静态属性 → 快照样式字段的映射表；样式宏后置（§5）                                                                                              |
| 列区带 / 换行守卫         | ✅ 已核对（M3-06） | 列区带：**已装守卫**（`ensure_data_columns`，`colType != -79` 显式报错）；换行回填：**静态守卫不可行**（`-64` 是 `INormalCell.TYPE_NORMAL` 普通格默认类型，测量分支条件「值为 String 且页内放不下」断页期才可知——静态拦截会拒绝所有真实模板；运行时风险带账，`pager/guard.rs` 尾注） |
| SQL 注入面            | 待定   | 报表模板自带 SQL（用户资产）⇒ 单实例私有化下视为可信输入；如未来外网暴露需重评估                                                                             |
| CLI 无自动化断言         | 沿用   | `spec-m2.md` §9 的待办；M3-07 顺带处理                                                                                              |

## 9. 变更记录

| 日期         | 变更                       | 原因                                                          |
| ---------- | ------------------------ | ----------------------------------------------------------- |
| 2026-10-14 | v0.1 立档（team-lead，阶段 0）  | 总监批准「务实路」后启动 M3；本档承接 `spec-m2.md` §6.2 的准入 v2；阶段 0 出口 = 本文件经总监确认 |
| 2026-10-14 | v0.2：M4 并行推进 + M3-08 提关键路径 | 总监令「M3 M4 一起推进，这样可以看到报表显示」；前端立档 `docs/spec-m4.md`，快照契约 v1 已定 |
| 2026-10-14 | v0.6：**M3-06 闭合**（分页真实口径） | `pager/paper.rs`（`gen_paper_table.py` 生成 80 码纸码表 + `usable_page_pt`）+ `paginate_with_setup`（真值口径，与探针口径共用 `split_rows`/`split_cols`）；`/render` 与 dev bin 的 `pagination` 切真值 + 快照 `pagination` 块（`spec-m4.md` §4.5）；守卫补 `tableColumnNum`/列区带；换行回填核对结论 = 静态守卫不可行（带账）；`PageInfo` 加 `row_range`/`col_range`（emit 不序列化，对拍不变）；RUNLOG §27 |
| 2026-10-14 | v0.5：**M3-05 闭合**（`POST /render` 主链路） | axum 0.8 + tokio 入 `rpx-server`（库/bin 分层：`lib.rs` + `http.rs`/`form.rs`，集成测试可 import 主链路纯函数与真 socket 双轨对拍）；引擎加 `load_datasets_with` 闭包字节源（磁盘/HTTP 上传共用一份口径）；上传完整性预检按**文件名**报缺；错误 400/500 分级；G5 登记 axum/tokio（理由见 `code-review.md` §3.2）；RUNLOG §26 |
| 2026-10-14 | v0.4：**M3-02 GBK 闭合**（`encoding.rs` + `gbk.bin` 冻结表 + `engine_gbk` 夹具 17 对拍全绿）；M3-01 状态同步 | `ProbeEncoding.java` dump JDK 全定义域 23,940 对 → 生成器只加头 → `include_bytes!`；`parse_csv` 按 `FileDataSetConfig.encoding` 缺省 GBK 解；GB18030 四字节显式带账；RUNLOG §25 |
| 2026-10-14 | v0.3：**M3-01 闭合**（阶段 1 出口达成） | 参数全景探针（`ProbeParamFull`）+ 建树期内联定案（`parse_with`/`calculate_with`/`ParamTable`，删除被证伪的求值期兜底）+ `engine_param` 夹具（16 夹具主对拍全绿）+ 两扩展格同行实证带账（RUNLOG §24） |
