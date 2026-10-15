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
| M3-03 | `rpx-ds`：`trait DataSource` / 方言层 / 数据集物化 / cross（蓝图 §七）✅ **骨架+sqlite 冒烟已达成**（2026-10-15）；PG/MySQL 真连冒烟带账；模板级 `SqlDataSetConfig` 接入待反逆向 | `rpx-ds`                   |
| M3-04 | 报表资产库：本地 SQLite（`.rpx` BLOB + 元信息 + 参数清单）✅ **已达成**（2026-10-15；libSQL→sqlx/sqlite 偏离定案见 §3.5） | `rpx-ds`（AssetStore）+ `rpx-server`（三路由） |
| M3-05 | `rpx-server`：axum 路由 + `POST /render` 主链路                                 | 新 crate                   |
| M3-06 | 分页真实口径：`/render` 按 `print_setup` 驱动 `pagination`（探针口径仅为对拍保留）               | `rpx-engine` 接口 + server 接线 |
| M3-07 | 门禁扩展（G1–G7 覆盖新 crate + **G8 分层矩阵**）+ e2e 冒烟 ✅ **已达成**（2026-10-15） | `scripts/gate.sh`（G8）+ `scripts/e2e_smoke.sh` |
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
  依赖：`rpx-core` + `sqlx(any)`。
  ✅ **已定案**（§3.4）：物化契约 `Value` + `RowSet` 下沉 `rpx-core`，
  `rpx-ds` **不依赖** `rpx-engine`。
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

### 3.4 `rpx-ds` 类型归属定案（M3-03 设计评审，2026-10-15）

**裁决**：物化契约**下沉 `rpx-core`**——`Value`（值模型）与 `RowSet`（行集）迁入；
`rpx-ds` 只依赖 `rpx-core` + `sqlx(any)`，**不依赖 `rpx-engine`**。

```text
rpx-core:  Value（十一变体，契约 = CalcJson.typeName 封闭集）
           RowSet { name, cols: Vec<String>, rows: Vec<Vec<Value>> }   ← 物化产物，纯数据
rpx-ds:    trait DataSource；SqlSource（AnyPool）取行 → RowSet          ← 供数方
rpx-engine: expr::ds::DataSet { name/cols/rows + current/selected }     ← 运行时态
            impl From<RowSet> for DataSet（物化 → 求值态的唯一入口）
```

**理由**（对照 §8 旧问句「下沉 `DataSet` vs `rpx-ds` 依赖 `rpx-engine`」）：

1. **依赖方向**：数据源层是引擎的**供数方**，供数方不依赖消费方。`rpx-ds → rpx-engine`
   会把 arena/expr/pager 全拉进取数 crate，且上层（cli/server/测试）想单独取数也无法。
2. **`DataSet` 整体下沉不可行**：它带求值期指针态（`current: DsCur`、
   `selected: Vec<Option<Value>>`——`DsValue` 缓存语义，`expr/ds.rs` 模块文档）。
   物化契约与运行时态**必须分离**，这正是两个类型的存在理由。
3. **`Value` 可干净下沉**：零依赖纯数据，十一变体对 `CalcJson.typeName` 封闭
   （`value.rs` 模块文档）；core 已有同构先例（`ObjValue` 单列 `Float`）。
   下沉后 `rpx-engine::value` 改 **re-export**，引擎内部路径零改动。
4. **`RowSet` 与 `ReportDefine`/`PrintSetup` 同层**：都是「引擎输入契约」，
   归 core 是本仓库既定分层（M1 起的纪律）。

**取数形态**（蓝图 §7.2 的落地口径）：`sqlx::any::install_default_drivers()` 先行；
`AnyPool::connect(url)` 按 URL 前缀选驱动（postgres/mysql/sqlite）；
行值经 `DsValue` 映射收敛到 `Value`（Null/Bool/I64/F64/Str/Blob/Decimal→规范字面量）。
方言差异（占位符/引号/分页/布尔字面量）在本期**只立表不实现改写**——
M3-03 验收 = 模板 SQL 原样透传（报表 SQL 是用户资产，单实例可信输入，§8 SQL 注入面同口径）。
sqlite:// 冒烟用内存库真连；postgres/mysql 真连冒烟**带账**（沙箱无服务器）。

### 3.5 资产库选型偏离定案（M3-04，2026-10-15）

蓝图 §7.1 选 libSQL（Turso 本地模式）；本期**用 sqlx 的 sqlite 驱动承载**
（`AssetStore` 住 `rpx-ds`——蓝图 §7 本就把平台库（7.1）与业务数据源（7.2）
放同一章）。理由（硬事实）：① `libsql` crate **不在本机离线缓存**，沙箱无法
下载验证——按「禁止无基准对拍」纪律不引跑不起来的依赖；② 蓝图自注：本地模式
下 libSQL 增量价值仅 async API + Turso CLI 生态，**文件格式仍与 SQLite 兼容**
——sqlx sqlite 给出同样的 async API 与文件格式；③ sqlx 已因 M3-03 入仓入
白名单，零新增依赖面；④ 升级路径保留：日后换 libSQL 只动 `asset.rs`（构造与
执行层），调用方零改动——与蓝图「new_local 换 new_remote_replica 一行」同精神。

schema（蓝图 §8.2 草稿落地）：`reports(id, name, rpx BLOB, version, created_at)`
+ `report_params(report_id, name, param_type, data_type, default_val, description)`
（比草稿多存 `param_type`/`data_type`/`description` 原样——语义不丢，
`is_static` 只是 `param_type==0` 的派生）。参数清单**上传时**解析落库 ⇒
`GET /meta` 读库即答，前端生成参数表单不依赖解析器。

## 4. 验收标准（EARS）

| 编号   | 功能       | 验收标准                                                                                                                                                                | 级别  |
| ---- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- |
| M3-01 | 参数注入生效   | While `POST /render` 携带 `params`，引擎**必须**按参数值计算：标量参数格不变；**List 参数必须**把该格包成扩展格（矩阵按子格数放大）；未登记参数**必须**报错（等价产品 `unknownParam`）；全部断言**必须**有产品夹具逐格对拍 | P0  |
| M3-02 | 数据集编码    | ✅ **已达成**（2026-10-14）：While CSV 为 GBK 且含中文，列名与值**必须**正确解码（`encoding.rs` 按 `FileDataSetConfig.encoding` 缺省 GBK 解码；`engine_gbk` 夹具中文全链路逐格对拍）；⛔ 不得静默乱码（非法字节显式 `MalformedEncoding`）——GB18030 四字节序列**带账**（§8），与产品默认 `GBK` charset 行为一致 |
| M3-03 | rpx-ds 取数 | ✅ **已达成**（2026-10-15 两轮）：`rpx-ds` 骨架（`DataSource` trait + `SqlSource`/`AnyPool` + 方言表）——sqlite:// 内存库**真连真取** 5/5；金仓 KingbaseES 真连（`smoke_pg.rs` env 门控，整数族统一 Long/BOOL→Bool/NUMERIC 响亮失败三条实锤）；**SQL 数据集链路闭环（探针 `ProbeSql.java` + 第 19 份夹具 `engine_sql`）**：产品引擎真机连金仓出真值（select 扩展+裸引用跟随+sum 退化全实证）→ `rpx-core` 解析 `SqlDataSetConfig`（凭据与模板分离契约）→ `load_datasets_full` 委托取数（G8：SQL 执行由调用方闭包注入）→ 金仓真机逐格对拍全绿（`sql_fixture.rs`）。带账：MySQL 真连（无服务器）；SQL 数据集的 `?` 占位参数/`begin_row`/`end_row` 行窗口/宏替换未建模（探针未覆盖，解析已保留） | P0  |
| M3-04 | 报表资产     | ✅ **已达成**（2026-10-15）：`POST /reports`（multipart `file`，`rpx_core::open_bytes` 魔数预检 → BLOB + 参数清单**上传时**落库）/ `GET /reports` / `GET /reports/{id}/meta`（读库即答；`is_static` 按 `Param.paramType==0`（`PARAM_NORMAL`，`Param.java:22-24`）派生，原样存 `param_type` 不归并）；坏魔数 400 / 缺 id 404 与空清单可区分；单测 4 + HTTP 3（`full.rpx` 的 startDate/topN 真实参数） | P0  |
| M3-05 | `/render` 快照 | ✅ **已达成**（2026-10-14）：`POST /render` 返回快照 JSON（multipart：`report`/`dataset`/`params`）；同输入下与引擎库函数直调**逐字节一致**（`tests/render_http.rs` 6 用例：真 socket + 对拍基准）；GBK/参数/缺文件/坏魔数/未知字段全覆盖；浏览器端到端实测通过（截图 `tests/fixtures/out/m4-render-http.png`） | P0  |
| M3-06 | 分页真实口径   | ✅ **已达成**（2026-10-14）：`/render` 的 `pagination` = **模板真值口径**（`paginate_with_setup`：纸码表 80 码 + 模板边距 → 可用 pt，`PagerInfo.getPageFormat(true)` 同构；快照 `pagination` 块见 `spec-m4.md` §4.5）；`pagerStyle != 2`/多栏（`columnMode`/`tableColumnNum`/重复组头）/列区带（`colType != -79`）/可用尺寸非正全部显式 `UnsupportedPaging`；探针口径保留为夹具对拍专用（双口径共用同一套 `split_rows`/`split_cols`）；换行回填**静态守卫不可行**（`-64` = `INormalCell.TYPE_NORMAL` 普通格默认类型，`guard.rs` 尾注） | P0  |
| M3-07 | 门禁与端到端   | ✅ **已达成**（2026-10-15）：G1–G4/G6/G7 本就全仓扫描（fmt/clippy/test/行数/bash 陷阱/文档引用天然覆盖新 crate）；**G5** 白名单含 rpx-ds（sqlx+tokio）与 rpx-server（axum+tokio+serde_json）且死条目同查；**新增 G8 crate 分层矩阵**（path 依赖方向越界即 FAIL，`cli->ds` 变异必红/撤后必绿）；**e2e 冒烟** `scripts/e2e_smoke.sh`：真二进制 serve --db → 上传 full.rpx → meta 静态参数 → render GBK 断言快照键集（id/sheetOrder/mergeData/cellData/rowData/columnData/pagination/page_w_pt）+ 中文数据 → 坏魔数 400 不入库 | P0  |
| M3-08 | 快照协议     | 快照**必须**含样式槽（模板静态属性直通）；动态样式宏**必须**在响应中显式标注「未应用」而非静默缺失                                                                                                              | P1  |

## 5. 支持范围声明（继承 M2 带账）

**原则：命中的模板会显式报错或显式标注，不会静默出错。**

| 项                                      | 现状                                | 出处                               |
| -------------------------------------- | --------------------------------- | -------------------------------- |
| 分页 `pagerStyle 0/1` / 多栏两开关           | ✅ 守卫：`UnsupportedPaging`          | `pager.rs::ensure_supported`（2026-10-14） |
| 跨页合并拆分                                 | ✅ **产品拆分语义已实现**（2026-10-15）：页矩形 × 全局块求交，延续页注**克隆锚点**（值/样式取原锚点，deepClone 快照等价）；源码实锤 `SimplePager.java:1624-1638`（行向）/`:2316-2335`（列向）；专项测试 `paged_snapshot_splits_cross_page_merge_with_cloned_anchor` | `snapshot/paged.rs` 模块文档 + `spec-m4.md` §4.6 |
| 文本换行回填                                 | ⏳ 带账（测量分支需字体度量，静态不可判——`-64` 是普通格默认类型） | `guard.rs` 尾注（M3-06 核对结论） |
| M2-22 残余（`customPropEngMap` / cell-graph / state 8） | ⏳ **语义已定案的有据带账**（2026-10-15 源码定案：属性表达式求值 → 值覆盖进 `customPropSetMap`，仅渲染期元数据透传，对计算网格零影响；探针 `CalcJson` 不导出自定义属性 ⇒ 无产品基准可对拍，维持不实现） | `spec-m2.md` §6.2（2026-10-15 条目） |
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
| `DataSet` 类型归属     | ✅ 已定案（2026-10-15，§3.4） | 物化契约 `Value` + `RowSet` 下沉 `rpx-core`；`rpx-ds` 只依赖 `rpx-core`（供数方不依赖消费方）；引擎 `DataSet` 保留运行时态（`DsCur`/`selected`）并 `From<RowSet>` |
| GBK 实现路径           | ✅ 已定案 | 冻结映射表（`collation.bin` 先例：`include_bytes!` 保零依赖）：`ProbeEncoding.java` dump JDK 全定义域（**23,940 对**，比常见标准表 21,886 项多——JDK 私有区映射）→ `scripts/gen_gbk_table.py` 只加头排序 → `data/gbk.bin`；Rust 单测全表逐对对拍 |
| GB18030 四字节序列       | ⏳ 带账   | 产品默认 `encoding="GBK"` 时 JDK GBK charset 同样不接受四字节（`MalformedInputException`）；模板显式写 `GB18030` 的场景**无夹具**，按"禁止无基准对拍"带账，命中报 `UnsupportedEncoding` |
| 样式直通范围             | 待分类  | 模板静态属性 → 快照样式字段的映射表；样式宏后置（§5）                                                                                              |
| 列区带 / 换行守卫         | ✅ 已核对（M3-06） | 列区带：**已装守卫**（`ensure_data_columns`，`colType != -79` 显式报错）；换行回填：**静态守卫不可行**（`-64` 是 `INormalCell.TYPE_NORMAL` 普通格默认类型，测量分支条件「值为 String 且页内放不下」断页期才可知——静态拦截会拒绝所有真实模板；运行时风险带账，`pager/guard.rs` 尾注） |
| sqlite BOOLEAN 列     | ✅ 已定案+钉子（2026-10-15） | Any 层（sqlx 0.9）对 sqlite 声明 `BOOLEAN` 的列 fetch 期即报 `AnyDriverError`（eager，级联 try_get 无机会）——布尔列在 sqlite 上用 INTEGER 0/1（`Long`）承载；钉子测试 `sqlite_declared_boolean_column_fails_loudly` 钉死「响亮失败」契约 |
| PG/MySQL 真连冒烟     | ✅ **PG 半边已清偿（2026-10-15，金仓真机）** | 公司服务器 192.168.1.77:54323 自报 **KingbaseES V008R006C008B0014**（人大金仓，PG 线协议兼容）；`tests/smoke_pg.rs`（env 门控 `RPX_PG_URL`）：类型物化 9 列逐格断言 + **BOOL→Bool 分支实锤** + NUMERIC fetch 期响亮失败实锤 + **整数族统一 Long**（sqlx Any `i64::compatible = is_integer`，int.rs:15/43/71）。MySQL 仍带账（无服务器） |
| SQL 注入面            | 待定   | 报表模板自带 SQL（用户资产）⇒ 单实例私有化下视为可信输入；如未来外网暴露需重评估                                                                             |
| CLI 无自动化断言         | 沿用   | `spec-m2.md` §9 的待办；M3-07 顺带处理                                                                                              |

## 9. 变更记录

| 日期         | 变更                       | 原因                                                          |
| ---------- | ------------------------ | ----------------------------------------------------------- |
| 2026-10-15 | v1.3：**`/render` 服务面接数据源注册**（M3-05 的 SQL 延伸） | `serve --datasources <path.json>`（JSON：`dsn → url / {url,user,pass}`）建 `DataSourceRegistry`（启动期建池 fail fast；`Debug` 手写凭据不入日志）；`http.rs` 状态升级 `AppState{store,dsr}`，主链路拆 `render_request`（同步纯函数）/ `render_request_ds`（async 预取 SQL 数据集 `(dsn,sql)→RowSet`）/ `render_core`（G8 分层不动）；错误面 `NoDataSource`/`UnknownDataSource`/`SqlFetch` 全 400；金仓 HTTP 端到端逐字节对拍 + 两条零环境负路径；RUNLOG §35 |
| 2026-10-15 | v1.2：**SQL 数据集链路闭环（M3-03 升级为已达成）** | 总监令「直接找 github 的实现」→ 调研结论：无 rpx 开源实现、官方开源 esProc SPL 同源（`docs/lecture-notes.md` 另录）；反逆向定案 `SqlDataSetConfig`（模板只存数据源名引用，凭据分离）→ 探针 `ProbeSql.java`（PG 驱动连金仓，产品真机 `VERIFY OK`）→ 第 19 份夹具 `engine_sql` → `rpx-core` 解析（`params` size==0 保持 null 口径）→ `load_datasets_full` 委托管线（新错误 `SqlDatasetNoConnection`/`SqlFetch`）→ 金仓真机逐格对拍全绿；RUNLOG §34 |
| 2026-10-15 | v1.1：**第 18 号夹具 `engine_emp`（员工信息查询）** | 总监以设计器截图问「能实现吗」：`=ds1.select(编号)` + 同行四条裸 `ds1.列名`。答案**能**（engine_gbk 已钉跟随语义）；本轮把截图原样做成夹具：产品真机 7×5（五条裸引用**逐行跟随**）+ `VERIFY OK: 18` + 引擎对拍（tripwire 97→102，全 state 4）+ 服务端快照内容断言 + Univer 浏览器截图 `m4-emp-demo.png`；RUNLOG §33 |
| 2026-10-15 | v1.0：**带账清偿轮**（总监令「③ 带账清偿」） | ① **PG 真连冒烟清偿**：总监提供公司服务器（192.168.1.77:54323），自报 **KingbaseES V008R006C008B0014**（人大金仓，PG 线协议兼容）——`tests/smoke_pg.rs` env 门控真连往返：类型物化 9 列逐格断言全绿、**BOOL→Bool 实锤**、**NUMERIC fetch 期响亮失败实锤**（`DsError::Sql(ColumnDecode "does not support ... Numeric")`，到不了级联——与 sqlite BOOLEAN 同构）、**整数族统一 Long**（sqlx Any `i64::compatible = is_integer`，sqlx-core int.rs:15/43/71）；② sqlite BOOLEAN **钉子** `sqlite_declared_boolean_column_fails_loudly`；③ **CLI 自动化断言**清偿（e2e_smoke.sh 步骤 ⑤：`rpx-cli print` 正路径 ASCII 锚 + 缺文件负路径）；④ **跨页合并拆分清偿**：产品拆分语义源码实锤（`SimplePager.java:1624-1638` 行向深克隆锚点 / `:2316-2335` 列向同构）→ `paged.rs` 页矩形求交 + 延续页克隆锚点注入 + 专项测试；⑤ **M2-22 残余语义定案**（customPropEngMap=模板自定义属性表达式表，求值覆盖 customPropSetMap，仅渲染元数据、对计算网格零影响；探针不导出 ⇒ 有据带账，spec-m2 §6.2）；MySQL 真连仍带账（无服务器）。rpx-ds 10+1、rpx-server 16 全绿 |
| 2026-10-14 | v0.1 立档（team-lead，阶段 0）  | 总监批准「务实路」后启动 M3；本档承接 `spec-m2.md` §6.2 的准入 v2；阶段 0 出口 = 本文件经总监确认 |
| 2026-10-14 | v0.2：M4 并行推进 + M3-08 提关键路径 | 总监令「M3 M4 一起推进，这样可以看到报表显示」；前端立档 `docs/spec-m4.md`，快照契约 v1 已定 |
| 2026-10-15 | v0.9：**M3-07 闭合（M3 全任务验收达成）** | gate.sh 新增 **G8 crate 分层矩阵**（path 依赖方向 = 架构；awk 段落模式 bug 被变异测试当场抓出——RS= 使逐行匹配全失效，改必红/撤必绿闭环）；**e2e 冒烟** `scripts/e2e_smoke.sh`（真二进制全链路 + 负路径）；AssetStore 改收**纯文件路径**（sqlite URL 两坑：`//target` host 化、`///relative` 绝对化，`SqliteConnectOptions::filename()` 绕开解析层）；417 测试全绿 + G1–G8 全绿 + e2e 全绿；RUNLOG §30 |
| 2026-10-15 | v0.8：**M3-04 闭合**（资产库） | `rpx-ds::asset::AssetStore`（sqlx/sqlite，**libSQL→sqlx 偏离定案 §3.5**：libsql 不在离线缓存，文件格式同 SQLite 兼容）+ `rpx-server` 三路由（POST /reports 上传预检魔数 / GET /reports / GET /reports/{id}/meta 读库即答）；参数清单上传时落库（`is_static` = `param_type==0` 派生，`param_type`/`data_type` 原样保存）；`serve --db`（默认 rpx.db）；417 测试全绿 + G1–G7 全绿；RUNLOG §29（含 **Rust 字符串续行吃前导空白** 与 axum 0.8 `{id}` 路由两坑） |
| 2026-10-15 | v0.7：**M3-03 定案 + rpx-ds 骨架闭合** | 类型归属定案（§3.4）：`Value`+`RowSet` 下沉 `rpx-core`，`rpx-ds` 只依赖 core（供数方不依赖消费方）；`rpx-ds` 新 crate（`DataSource` trait + `SqlSource`/`AnyPool` + 方言表，sqlx 0.9 过 G5 登记）；sqlite 内存库真连冒烟 5/5；`From<RowSet>` 桥（`ds.rs` 单测）；410 测试全绿 + G1–G7 全绿 |
| 2026-10-14 | v0.6：**M3-06 闭合**（分页真实口径） | `pager/paper.rs`（`gen_paper_table.py` 生成 80 码纸码表 + `usable_page_pt`）+ `paginate_with_setup`（真值口径，与探针口径共用 `split_rows`/`split_cols`）；`/render` 与 dev bin 的 `pagination` 切真值 + 快照 `pagination` 块（`spec-m4.md` §4.5）；守卫补 `tableColumnNum`/列区带；换行回填核对结论 = 静态守卫不可行（带账）；`PageInfo` 加 `row_range`/`col_range`（emit 不序列化，对拍不变）；RUNLOG §27 |
| 2026-10-14 | v0.5：**M3-05 闭合**（`POST /render` 主链路） | axum 0.8 + tokio 入 `rpx-server`（库/bin 分层：`lib.rs` + `http.rs`/`form.rs`，集成测试可 import 主链路纯函数与真 socket 双轨对拍）；引擎加 `load_datasets_with` 闭包字节源（磁盘/HTTP 上传共用一份口径）；上传完整性预检按**文件名**报缺；错误 400/500 分级；G5 登记 axum/tokio（理由见 `code-review.md` §3.2）；RUNLOG §26 |
| 2026-10-14 | v0.4：**M3-02 GBK 闭合**（`encoding.rs` + `gbk.bin` 冻结表 + `engine_gbk` 夹具 17 对拍全绿）；M3-01 状态同步 | `ProbeEncoding.java` dump JDK 全定义域 23,940 对 → 生成器只加头 → `include_bytes!`；`parse_csv` 按 `FileDataSetConfig.encoding` 缺省 GBK 解；GB18030 四字节显式带账；RUNLOG §25 |
| 2026-10-14 | v0.3：**M3-01 闭合**（阶段 1 出口达成） | 参数全景探针（`ProbeParamFull`）+ 建树期内联定案（`parse_with`/`calculate_with`/`ParamTable`，删除被证伪的求值期兜底）+ `engine_param` 夹具（16 夹具主对拍全绿）+ 两扩展格同行实证带账（RUNLOG §24） |
