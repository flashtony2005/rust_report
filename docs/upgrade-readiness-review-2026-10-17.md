# RPX / rust_report 架构、算法、实现与 CLI 升级就绪度评估

> 评估日期：2026-10-17（Asia/Shanghai）
> 评估对象：当前工作树（`/Users/lushaohui/project/rust_report/rust_report`），不是 `HEAD`
> 代码基线：`HEAD = d037719`（2026-10-16），工作树另有 **173 项未提交变更**（30 删除 / 75 修改 / 68 未跟踪）
> 评估方式：读源码与规格、构建并跑测试、真实 CLI/HTTP 进程实测、故障与边界试探、文档与代码交叉核对
> 本次只做评估，未修改任何业务代码

---

## 0. 结论摘要

### 0.1 总评

| 维度 | 分数（10 分制） | 结论 |
| --- | ---: | --- |
| 架构分层 | 8.5 | 依赖方向清晰且有机器门禁；`rpx-runtime` 已成为新的中心层 |
| 算法忠实度 | 9.0 | 对产品引擎的移植非常认真：夹具、tripwire、显式不支持出口都在 |
| 算法可演进性 | 6.5 | 高度依赖 Java 反编译语义与外部夹具生成链，升级产品版本的成本高 |
| 实现质量 | 8.0 | 无 `todo!`/`unimplemented!`，测试量大，clippy/fmt 干净；仍有健壮性与平台化缺口 |
| CLI 功能面 | 8.5 | 文档列出的 27 个命令全部存在，主流程实测可跑通 |
| CLI 健壮性 | 6.0 | `print-xlsx`、`import-rpx`、断管、错误文件校验、运行记录排序存在真实缺陷 |
| 文档 | 7.0 | 规格密度罕见地高，但**当前态与历史态混在一起**，多处路径/端点已失效 |
| 发布与运维 | 4.5 | 核心成果尚未提交、无 CI、无 LICENSE/MSRV、无生产部署形态 |
| **综合（作为工程原型）** | **8.0** | **这是一份质量明显高于平均水平的原型** |
| **综合（作为可交付产品）** | **6.0** | **功能已成型，生产化还差「最后一公里」** |

一句话结论：

> **架构与算法是强项，CLI 的「功能面」已经实现，但「功能边界」和「交付工程」还没有收口。**
> 当前最该做的不是重写，而是：**先提交并冻结一个可复现基线，再用一组真实失败场景把 CLI 边界补齐，最后把文档和 CI 分成「当前架构」与「历史里程碑」两套。**

### 0.2 五条最重要的结论

1. **架构方向正确，且被门禁守住了。**
   `rpx-core` / `rpx-engine` 保持零第三方依赖，`rpx-ds` / `rpx-export` / `rpx-runtime` / `rpx-cli` / `rpx-server` 分层明确；`scripts/gate.sh` 的 G5（依赖白名单）和 G8（crate 分层矩阵）不是纸面约定，而是会失败的检查。

2. **算法质量是项目最强的资产。**
   `rpx-engine` 约 2.1 万行生产代码、421 个测试属性，主对拍链路对 23 个产品夹具逐格比较且**不再设夹具级豁免**；合并、分页、属性表达式、中文排序、BigDecimal、GBK 都有专门证据。代码里没有 `todo!`/`unimplemented!`，不支持的能力基本都显式报错。

3. **CLI 确实实现了，而且不是「壳」。**
   26 个文档命令全部存在（含新增的 4 个 `format` 迁移命令）；`crates/rpx-cli/tests` 用真实二进制进程测试了创建、校验、运行、查询、修订冲突、补丁原子性、stdin、RPX 导入等关键路径；`scripts/e2e_smoke.sh` 也验证了 CLI + HTTP 的完整闭环。
   但实测发现 **一组会让真实用户/agent 直接失败或误判的边界缺陷**（见 §5），所以不能把「命令存在」当成「功能完成」。

4. **当前成果没有进入 Git，这是升级前最大的单点风险。**
   `crates/rpx-runtime/`、`examples/`、`README.md`、`Makefile`、`skills/` 以及新的 CLI 源码全部是未跟踪文件；`Cargo.toml`、`Cargo.lock` 和 75 个文件处于修改状态。任何从 `HEAD` 出发的升级、复制、CI 或交接都会得到**旧的 CLI 架构**，而不是当前这套。

5. **文档是「高质量但混合版本」。**
   当前态文档（`README.md`、`docs/agent-cli.md`、`docs/agent-cli-design.md`）是新的；但规格文档里仍有大量 `/render`、`/export`、`/meta`、`rpx-server/src/reportdef/`、BLOB 入库的引用。实测统计：`/render` 62 处 / 14 份文件，`/export` 25 处 / 6 份，`/meta` 23 处 / 8 份，`reportdef/` 20 处 / 4 份。升级时如果不先做「当前架构索引」，后来的人会把历史规格当成现状。

---

## 1. 基线事实（可复现）

### 1.1 构建、测试与门禁

| 项 | 实测结果 | 说明 |
| --- | --- | --- |
| `cargo test --workspace --no-fail-fast` | **689 passed / 0 failed** | 第二次完整运行；首次运行有一次 `library_http` 的瞬时 `ConnectionReset`，见 §4.1 |
| `cargo fmt --all --check` | 通过 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 | 零告警 |
| `scripts/gate.sh` G1–G8 | 全部通过（使用本地 `CARGO_TARGET_DIR`） | G9 见下 |
| `scripts/gate.sh` G9 | 本沙箱内红 | Vitest 试图写 `/Users/lushaohui/node_modules/.vitest-cache`，被文件沙箱拒绝（EPERM）；**不是测试断言失败** |
| Web 单测 | 264 个已加载用例通过 / 0 失败 | 另有 1 个 suite 因 Vitest 缓存 EPERM 未加载；G9 失败时计数会变成 251/238，因此 264 不是完整总数 |
| Web 构建 | 通过 | `tsc --noEmit && vite build` |
| `scripts/e2e_smoke.sh` | `E2E PASS` | CLI run 23×25、共享源码保存、412/428、固定 workbook/source |
| `cargo install --path crates/rpx-cli --locked` | 通过 | 生成约 10 MB 的 `rpx` release 二进制 |

> 环境备注：本机 `~/.cargo/config.toml` 把 target-dir 指到 `~/.cargo/target`，该路径在沙箱内不可写，因此直接跑 `cargo`/`gate.sh` 会在 G2/G3 报 `failed to open ...cargo-build-lock`。改用 `CARGO_TARGET_DIR=$PWD/target` 后 G1–G8 全绿。这是**环境**问题，不是代码问题；但它暴露了「gate 不是 hermetic 的」这一交付问题。

### 1.2 代码规模

| crate | 全部 Rust 行 | 测试行 | 生产行 | 主要职责 |
| --- | ---: | ---: | ---: | --- |
| `rpx-engine` | 32,568 | 11,319 | 21,249 | 扩展、求值、合并、分页、导出网格 |
| `rpx-runtime` | 10,943 | 4,244 | 6,699 | 定义 JSON/运行库/快照/导入/编排 |
| `rpx-core` | 4,851 | 1,586 | 3,265 | `.rpx` 二进制读写 |
| `rpx-export` | 1,717 | 840 | 877 | CSV / XLSX / 打印 XLSX |
| `rpx-server` | 1,089 | 144 | 945 | 8 个 HTTP 端点 |
| `rpx-cli` | 1,257 | 578 | 679 | 27 个 CLI 命令 |
| `rpx-ds` | 787 | 218 | 569 | CSV/Excel/SQL 数据源 |
| **合计** | **52,908** | **18,743** | **34,165** | |

其他：`web/src` 约 8,164 行 TypeScript；`docs/*.md` 约 11,809 行；`tests/fixtures` 有 40 个 `.rpx`、28 个 `.expected.json`、59 个 Java 探针源文件。

### 1.3 工作树状态（升级前必须先处理）

| 状态 | 数量 | 关键内容 |
| --- | ---: | --- |
| `D` 删除 | 30 | 旧 `rpx-server/src/reportdef/*`、`snapshot/*`、`export/*`、旧 HTTP 测试等 |
| `M` 修改 | 75 | `Cargo.toml`/`Cargo.lock`、CLI、engine、web、docs、gate |
| `??` 未跟踪 | 68 | **`crates/rpx-runtime/` 全量**、`examples/`、`README.md`、`Makefile`、`skills/`、新 CLI `args.rs`/`commands.rs`/测试、`demo_data/`、`rpx.db` 等 |

这意味着：

- `git clone` + `git checkout main` 得到的是 **旧架构**：CLI 只有 `print`/`render`，server 仍走 `/render`/`/export`。
- 当前 `rpx-runtime` 的 `catalog.rs` 用 `include_str!`/`include_bytes!` 直接引用 `examples/tutorial/*`；如果只提交 runtime 而不提交 `examples/`，**干净克隆会编译失败**。
- `README.md`、`Makefile`、`skills/rpx/SKILL.md` 都是当前交付物的一部分，也仍未跟踪。
- 根目录 `rpx.db` 是旧 BLOB 资产库遗留的 SQLite 文件（表 `reports`/`report_params`），**未被 `.gitignore` 忽略**，容易被误提交。

**建议的第一动作：把当前工作树整理成一个明确的 checkpoint commit（含 `examples/`、runtime、web、docs、tests、Cargo.lock），打 tag，再开始任何升级。** 在此之前，「升级」没有可回的基线。

---

## 2. 架构分析

### 2.1 分层与数据流

```text
                    ┌──────────────────────────────┐
                    │  web/ (React 19 + Univer)    │
                    │  只读运行快照 / 编辑定义 JSON  │
                    └──────────────┬───────────────┘
                                   │ HTTP (If-Match)
                    ┌──────────────▼───────────────┐
                    │  rpx-server (axum, 8 routes) │
                    │  目录库读写 / 运行记录 / 产物  │
                    └──────────────┬───────────────┘
                                   │
┌──────────────┐        ┌──────────▼───────────┐
│  rpx-cli     │───────▶│  rpx-runtime         │
│  (rpx, 22 命令)│        │  定义解析/校验/运行库 │
└──────────────┘        │  运行编排/快照/导入   │
                        └───┬───────┬───────┬───┘
                            │       │       │
                  ┌─────────▼─┐ ┌───▼─────┐ ┌▼──────────┐
                  │ rpx-ds    │ │rpx-export│ │rpx-engine │
                  │CSV/Excel/SQL│ │CSV/XLSX │ │扩展/求值/ │
                  └─────┬─────┘ └────┬─────┘ │合并/分页   │
                        │            │       └─────┬─────┘
                        └────────────┴─────────────┘
                                   │
                             ┌─────▼─────┐
                             │ rpx-core  │
                             │ .rpx 读写 │
                             └───────────┘
```

关键设计：

- **两条入口，一个出口**：`.rpx` 字节与定义 JSON 都归一到 `ReportDefine`，下游计算链路不分叉。
- **计算与展示分离**：`rpx-engine` 只做计算态 `CalcGrid`；`rpx-runtime::snapshot` 负责 Univer 工作簿；Web 不执行引擎。
- **共享运行层**：CLI 与 server 都调用 `rpx-runtime`，避免「服务端一套、CLI 一套」的语义分叉。
- **目录库不是数据库**：报表源码在目录中，运行记录在 `.runs/<id>/<run-id>/`；修订号是 manifest/template/默认参数的 SHA-256；跨进程文件锁 + `If-Match` 乐观并发；写侧走暂存目录 + 原子替换。
- **运行不可变**：一次运行捕获源码、参数、输入哈希，先登记 `run.json`，再取数/计算/发布产物；失败保留失败记录，旧成功结果仍可读。

### 2.2 架构优点

1. **依赖方向有机器判据。**
   G5 的零依赖层 + 白名单，G8 的 path 依赖矩阵，能真实拦住「engine 悄悄引第三方库」「cli 反向依赖 server」这类腐化。

2. **零依赖计算核是长期资产。**
   `rpx-core`/`rpx-engine` 不用 serde、不用时间库、不用异步 runtime；算法与 IO/框架解耦，未来换 Web 框架、换存储、做 WASM 都不会被拖住。

3. **运行层抽取方向正确。**
   `rpx-runtime` 把原先散在 server 的解析、快照、导入、运行编排收拢后，CLI 与 server 共享同一套语义；server 退化成「目录库 API」，职责明显更窄。

4. **写入协议考虑过失败路径。**
   `files::create` 用 `create_new` + `sync_all`；`atomic_json` 用 `.pending` + rename；复制/导入/初始化用暂存目录，失败清理；补丁先在内存校验再落盘。这些不是「顺手写的」，是专门设计的。

5. **错误面有统一出口。**
   CLI 协议统一为单行 JSON envelope，退出码 0–7 有明确含义；`engine` 的不支持项用 `UnsupportedExpr`/`UnsupportedPaging` 等显式错误，而不是静默降级。

### 2.3 架构风险

1. **`rpx-runtime` 正在变成「第二引擎 + 应用层」的大杂烩。**
   生产代码约 6,700 行，同时负责：JSON Schema/定义解析、参数、数据集依赖、运行库、运行记录、快照映射、图形、RPX 导入、目录库管理。它是当前最值得拆分的一层（见 §8）。

2. **CLI 与 SQL/Excel/导出强耦合。**
   `rpx-cli -> rpx-runtime -> rpx-ds(sqlx all drivers) + rpx-export(rust_xlsxwriter)`。一个只需要 JSON 校验的 agent，也要拖上 PostgreSQL/MySQL/SQLite 驱动和 XLSX 写库；release 二进制约 10 MB，编译时间也被拉长。未来应按 feature 拆：`sqlite` 默认、`postgres/mysql` 可选、`export` 可选。

3. **目录库的锁是「阻塞且无超时」的。**
   `library::lock` 用 `File::lock()` 阻塞等待；进程崩溃/挂起时，其他 agent 可能无限等待。应增加 `try_lock` + 超时 + 明确诊断，或至少在文档中说明。

4. **运行记录无保留策略。**
   每次 `report run` 都保留 source/inputs/workbook/导出；没有 `report prune`/保留 N 条/体积配额。长期使用会无限增长。

5. **产品版本耦合。**
   engine 的语义来自对 ReportLite Java 的反编译与夹具；一旦产品升级，哪些行为变了、哪些夹具仍有效，目前没有「产品版本 → 夹具版本」的映射，也没有可重跑的 Java 生成链（`re_work/` 不在仓库内）。

---

## 3. 算法分析

### 3.1 计算主链

`rpx-engine` 按产品 `ExtCellSet` 的三阶段模型实现：

1. **阶段 A**（`stage_a.rs`）：模板 `ReportDefine` → 计算矩阵 + 引擎矩阵（两张矩阵，含 0 号脚手架行/列）；合并锚点铺满矩形，延续槽的语义按 Java 实测分别处理。
2. **阶段 B**（`stage_b.rs`）：角色判定 → 主格图（left/top head）赋值 → 拓扑改写（`extendMode` 16/18/19 的运行时归一）→ 属性表达式/字面量取值。
3. **扩展算法**（`ext_algo/`）：`extendRow`/`extendCol` 两趟，含包络、部分平移插行/列、合并区扩展、`expand*Head`；关键易错点（克隆保留 bit30、`disp_override` 清空、`instance` 清空）逐条写进了模块文档和测试。
4. **求值调度**（`eval/`）：四通道 trampoline（channel4/5/6 + calcProperties），禁止递归求值；special 队列有 `DEFAULT_MAX_SPECIAL_REQUEUES = 100_000` 的活锁上限。
5. **合并**（`merge/`）：`mergeSame` 四方向 + `mergeNull` 四趟，含 `_$5`/`_$6` 守卫和 Java 的残值行为；P2/P3/P4 现在有合成矩阵测试，但仍缺产品夹具。
6. **分页**（`pager/`）：按行高断页、分组头重复、横向按列宽断页、纸张/页边距、手动断页；`pager_style != 2`、多栏等不支持项显式报错。
7. **属性表达式**：行/列/普通格的 `expMap` 在分页前求值，`engine_rowheight` 的 17 个行高与产品逐值对拍。

这条链路的质量证据：

- `crates/rpx-engine/tests/engine_grid_compare.rs` 对 23 个夹具做值/类型/`disp_value`/`source` 六子字段/`sub_values`/`left_head`/`top_head`/网格尺寸/合并跨度/分页的逐项比较，**已移除夹具级豁免**。
- 反向控制：先断言未变异夹具干净，再断言变异必被报出并精确定位；这是「判据有牙齿」的证明，而不是「测试通过了」。
- `engine_cycle` 的循环引用链 `"B2-C2-B2-C2"` 与产品一致；`engine_collate` 用 34 条公式钉住中文排序（含码点序与产品序相反的例子）。
- `literal/decimal.rs` 复刻 `BigDecimal.toString`，并用 JRE8/JDK21 两份 oracle 验证「换 JVM 结果一致」。

### 3.2 表达式与数据集

- 词法 + Pratt 解析，`names.rs` 完整转录产品 `REGULAR` 124 + `DS` 15 个名字；`FUN_PRIORITY_*` 表虽已落地，但**当前没有接线**，代码里用醒目横幅声明「无人消费」——这是很诚实的做法，但也是升级时的隐患：名字表与求值器不同步。
- 求值器实际只覆盖约 20 个函数（`catalog` 也只列这 20 个），其余解析得到但求值会报 `UnsupportedExpr`；`catalog` 没有把 parser 表伪报成「支持」，这点做得对。
- 数据集：文件（CSV/TSV/Excel）与 SQL（AnyPool）统一物化为 `RowSet`；`rpx-ds` 不依赖 engine，符合「供数方不依赖消费方」。
- DS current 生命周期（`calc/ds_current.rs`）是全 ctx 共享可变状态；模块文档用「谁把 `ds.current` 变成 `Row`」把产品语义讲透，并钉住「只有扩展格会灌 Row」等 6 条易错点。

### 3.3 算法残余缺口（项目自己已声明，我复核后仍成立）

| 缺口 | 影响 | 证据 |
| --- | --- | --- |
| `mergeSame` P2/P3/P4 与 `mergeNull` 无产品夹具 | 只能靠合成矩阵；Java 残值行为无法用产品真值区分 | `merge/matrix.rs`、`merge.rs` 模块文档 |
| 横向扩展克隆的行号归属 | 只保证「同属一行 + 列走向」，不拟合行号 | `ext_algo.rs` 模块文档 |
| 二维合并对角槽 | 按纵向规则处理，无产品夹具 | `stage_b/merge_alias.rs` |
| `pagerStyle != 2`、多栏、跨页合并拆分、文本换行 | 显式报错或未实现 | `pager.rs`、`EngineError::UnsupportedPaging` |
| M2-22 的 `customPropEngMap` / state 8 属性队列 | 纯渲染期元数据，当前无夹具；显式带账 | `spec-m2`、`calc/run.rs` |
| 日期字面量 | 引擎不计算日期对象，只保留定义；执行会明确拒绝或按文本处理 | `literal.rs` 模块文档 |
| 函数面 20 / 解析表 139 | 大量产品函数「认识但不算」；升级时要补 | `catalog.rs` / `names.rs` |

这些缺口的处理方式总体是「显式报错/带账，不静默」，符合项目纪律；但**升级产品版本时，它们就是第一批要重新对拍的地方**。

### 3.4 导出与快照

- CSV：满矩阵展开、RFC 4180 子集、`disp_value` 优先、无 BOM、末尾无换行；规则有单测和读回自洽。
- XLSX：单 sheet、合并区锚点重建、Excel 硬上限裁剪（>1,048,576 行 / >16,384 列）、超长文本截断、确定性字节（固定 zip 时间戳）。
- 打印 XLSX：按引擎页映射，一页一 sheet，保留页眉/页脚/合并/图形；但自定义纸张（`paper=256`）、工作表背景、部分 3D 图表**明确不支持**。
- 快照：`CalcGrid` → Univer `IWorkbookData`，含 `cellData`/`mergeData`/`rowData`/`columnData`/`pagination`/`rpxGraphics`；多页时每页一个 sheet。
- **精度边界**：`result.json` 对 Long/Decimal 用字符串保真；但 `workbook.json` 把 Long 写成 JSON number、Decimal 写成 f64。浏览器/JS 超过 2^53 会丢精度，Decimal 会丢尾数。项目文档声明了 Decimal 的近似，但没有对 Long 的同一问题给出声明。

### 3.5 性能

本机 release 实测（demo 数据，非基准）：

| 报表 | 规模 | 耗时 | 备注 |
| --- | --- | ---: | --- |
| `demo_data/report_sales_2026.json` | 645×7，7 页 | 0.46 s | 含 SQLite 取数 |
| `demo_data/report_pivot_2026.json` | 85×17，6 页 | 0.43 s | 大量 `ds.select` 过滤聚合 |
| `examples/tutorial/complex-master-expansion.json` | 23×25，1 页 | < 0.1 s | 交叉扩展 |
| 999×999 空网格 validate | ~1e6 槽 | 0.02 s / 50 MB RSS | 网格上限附近可接受 |

结论：**当前报表规模下性能不是瓶颈**；但：

- 没有基准测试、没有性能回归门禁；
- `ds.select` / `sum` 这类格间聚合在「大网格 × 大分组」下可能出现 O(格数 × 数据行数) 的放大；
- SQL 用 `fetch_all` 一次性载入内存，CSV/Excel 也整文件读入；没有流式、行数上限或内存保护；
- debug 构建下 pivot 约 2.5 s，release 0.43 s；对外发布应只用 release，并在 CI 里跑 release 性能冒烟。

---

## 4. 实现质量

### 4.1 测试

- **689 个 Rust 测试通过**，`rpx-engine` 一个 crate 就有 421 个测试属性；测试行数 18.7k，约占生产代码的 55%。
- `rpx-cli` 的测试是**真实二进制进程**（`CARGO_BIN_EXE_rpx`），不是只测库函数；覆盖 `run` 的 shape、`result inspect` 的 range/origin、重复 out-dir、未知字段 pointer、修订冲突、并发补丁、stdin 参数、RPX 导入 dry-run 等。
- `rpx-server` 用真实 socket + 真 HTTP + 磁盘库测试保存/读取/412/428/404，并断言 `/render`、`/export` 已 404。
- Web 有 13 个测试文件、215 个 `it/test` 声明（部分参数化；缓存可写时已加载的 264 个用例通过），覆盖 designer/stylemap/cellprops/rowcol/datasets/graphics 等纯逻辑。本轮 G9 中另有 1 个 suite 因缓存 EPERM 未加载，所以「264」不是完整总数。
- 故障注入脚本存在（`scripts/fault_inject_pager.py`、`fault_inject_tutorial.py`），并有「锚点必须恰好 1 处、基线先绿、还原后重跑绿、对照组」的纪律；**但它们不在 `gate.sh` 里，也不在 CI**。

**瑕疵：**

- 首次完整测试时 `crates/rpx-server/tests/library_http.rs` 出现过一次 `ConnectionReset`；单独重跑 5 次均通过。可能是并行/时序型 flake，但测试没有重试、没有服务端 panic 日志断言，出现时很难定位。
- 没有覆盖率门禁、没有 property-based/fuzz 测试。我额外做了 300 个畸形定义 JSON + 500 个随机 `.rpx` 的冒烟 fuzz，**没有 panic**；但这只是抽查，不是持续保障。
- 没有 benchmark/性能回归。
- G9 的 Web 测试在沙箱内不可复现（Vitest 缓存写到 home），说明 gate 的「可移植性」还不够。

### 4.2 门禁与工具链

`scripts/gate.sh` 的 G1–G9 设计质量很高：

- G1 fmt、G2 clippy `-D warnings`、G3 tests、G4 每文件 ≤500 行、G5 依赖白名单/零依赖层、G6 shell 陷阱、G7 文档 `§N.M` 引用完整性、G8 crate 分层、G9 Web build+test。
- G7 确实有价值：当前 342 个 `*.md §N.M` 引用全部解析成功；这拦住了「注释指向不存在的规格章节」。
- G4 当前最大文件 484 行（`calc/run.rs`），有 6 个文件在 450–500 的预警区。

缺口：

- **没有 CI**（无 `.github/`），所有门禁只能靠人记得在本地跑。
- gate 不 hermetic：依赖 `~/.cargo/target` 可写、依赖 `node_modules` 存在且可写；在沙箱/CI/容器里会红，且红的原因不是代码。
- 没有 `cargo audit` / `cargo deny` / `cargo machete`；依赖白名单只管「有没有登记」，不管「有没有漏洞/是否还在用」。
- 没有 MSRV 声明（README 说 Rust 1.89+，`Cargo.toml` 没有 `rust-version`），没有 license 文件。

### 4.3 错误处理与协议

优点：

- CLI 默认单行 JSON，`ok`/`error`/`diagnostics`/`meta.duration_ms`/`protocol_version` 结构一致；参数解析错误也输出 JSON；退出码 0–7 与文档一致。
- 服务端错误映射：7→412、6→404、1→500、其余→400；缺 `If-Match` 返回 428。
- 不支持的能力多数有显式错误（`UnsupportedExpr`、`UnsupportedPaging`、`UnsupportedEncoding`、`MalformedEncoding`、`RPX_IMPORT_UNSUPPORTED`），而不是静默丢失。
- 错误里常带 `json_pointer`、`template_cell`、`run_file`，对 agent 修模板很有用。

缺陷：

1. **`RenderError` 的 `Display` 直接 `write!(f, "{self:?}")`。**
   于是 `run` 的错误消息会变成 `Engine(UnsupportedExpr { expr: "sort(...)" })`、`NoDataSource { dsn: "demo" }`、`SqlFetch(Sql(...))` 这类 Rust Debug 结构，而不是人类可读的一句话。
   位置：`crates/rpx-runtime/src/execution.rs:175-178`；被 `crates/rpx-runtime/src/run.rs:142-146` 用于 `Error::new(..., e)`。

2. **断管未处理。**
   `println!` 在 stdout 被提前关闭时 panic，退出码 101，违反 0–7 协议。
   实测：`rpx report show complex-master --library ... | head -c 100`、`rpx template inspect ... | head -c 100` 均 panic。
   位置：`crates/rpx-cli/src/main.rs:36`、`crates/rpx-cli/src/commands.rs:227-236`。

3. **`result inspect` 不校验「结果文件」身份。**
   传入模板 JSON 时，因为模板也有 `cells` 数组，命令返回 `ok:true`，把模板 cell 当作结果 cell 输出（没有 `value`/`origin`），退出码 0。
   位置：`crates/rpx-runtime/src/result.rs:63-80`；只检查 `cells` 是否为数组，没有检查 `version`、`rows/cols`、cell 的 `value/origin` 结构。

4. **运行历史按字符串排序。**
   `started_at` 是 RFC3339，精度可变（实测有 `.362614Z` 和 `.99143Z`）；直接字符串比较会在「同一秒、不同小数位」时排错。
   实测构造 `...07.99Z` 与 `...07.9901Z`，`report runs` 把较早的 `.99` 排在 `.9901` 前面。
   位置：`crates/rpx-runtime/src/library.rs:179-184`；`state()` 用 `records.first()` 取「最新」，因此也会选错 `latest_attempt`/`display_run`。

5. **HTTP 层的 JSON 解析错误不是 JSON。**
   `PUT /reports/{id}/template` 发送非法 JSON 时，axum 的 `Json` extractor 直接返回 `text/plain` 400；错误 content-type 返回 415 文本。Web 的 `request()` 无条件 `response.json()`，遇到这类响应会抛 `SyntaxError`，而不是 `LibraryError`。
   位置：`crates/rpx-server/src/http.rs:77-96`、`web/src/library-api.ts:12-16`。

### 4.4 文档

这是项目最矛盾的一项：

**优点**

- 规格密度极高：`docs/format-contract.md`、`docs/m2-engine-design.md`、`docs/spec-m1..m7` 把反直觉语义、`file:line` 出处、已核实/待验证、变更史都写下来了。
- 关键代码模块有 `//!` 长文档，且明确写「哪些能力未接线」「哪些变异存活是被接受的」，这在真实项目里很少见。
- G7 保证 342 个章节引用可解析。

**问题**

1. **当前架构与历史架构混在一起。**
   `docs/report-def-json.md:3-4` 仍写实现位置 `crates/rpx-server/src/reportdef/`；实际已迁到 `crates/rpx-runtime/src/reportdef/`。
   `docs/spec-m6.md:4` 仍写 server `POST /export`、CLI `render --out`；当前 server 已无 `/export`，CLI 已无 `render`。
   `docs/spec-m7-web.md:32` 仍写「经既有 `POST /render` / `POST /reports` 入库渲染」；当前两者都不存在。
   `web/src/App.tsx:551` 的「关于」文案仍写 `/render → Univer 快照，/export → 文件字节`。

2. **文档内部自相矛盾。**
   `docs/report-def-json.md:48` 说「未知的顶层键被忽略」，但同一文档 `:194` 和实际实现都要求未知字段 400；实测 `{"foo":1}` 返回 `TEMPLATE_INVALID /foo`。
   `docs/code-review.md:59-63` 说 oracle `.txt` 不落库，但 `git ls-files tests/fixtures/_bdo` 显示两个 oracle 已跟踪。

3. **没有文档索引/状态页。**
   新读者无法一眼判断哪份是当前架构、哪份是历史里程碑；`docs/.snapshots/` 也没有说明。

### 4.5 依赖、安全、可移植性

- 依赖版本整体较新：`sqlx 0.9`、`axum 0.8.9`、`clap 4.6.7`、`rust_xlsxwriter 0.99.1`、`time 0.3.55`、`tokio 1.53.1`。`cargo tree -d` 有少量重复版本（来自 sqlx 的 digest/hashbrown），属正常。
- **未跑成 `cargo audit`**：沙箱阻止 `~/.cargo/advisory-db..lock`，不是项目问题；但升级前应在 CI 补一次。
- 安全设计有意识：SQL 凭据不进模板；`run.json` 只存 dsn/query hash；路径库拒绝绝对路径/`..`/符号链接复制；图片只接受 data URI；ECharts 脚本在 `sandbox="allow-scripts"` 的 iframe 内执行；服务端默认绑 127.0.0.1。
- 风险：`rpx-server` **没有认证**，`--addr 0.0.0.0` 会把「读/写报表源码 + 下载产物」暴露给网络；生产部署还需要 TLS/反向代理/静态资源托管方案。当前只适合本机。
- **没有 LICENSE**；`Cargo.toml` 只有部分 crate 有 description，没有 authors/repository/rust-version；对外分发前必须补。
- 可移植性：`File::lock` 要求 Rust 1.89；Windows 未在本机验证；`fs::rename` 覆盖、路径大小写、GBK 文件路径等仍是未验证面。

### 4.6 代码规模与可维护性

- 52.9k Rust 行、34.2k 生产行，最大单文件 484 行；整体没有「单文件上帝类」。
- `rpx-engine` 的模块文档非常重，阅读门槛高，但换来了语义可追溯；这是「移植型项目」合理的取舍。
- `rpx-runtime` 的 `reportdef/parse.rs`（481 行）、`cellprops.rs`（462 行）已接近上限，继续加字段需要拆分。
- `web/src/App.tsx` 1088 行、`panes.tsx` 657 行、`stylemap.ts` 648 行；没有 Web 侧文件长度门禁，也没有 ESLint/Prettier。
- 全仓约 1.1k 处 `unwrap()/expect()`（多数在测试文件和内联测试模块；生产路径里也有少量基于「已校验」的 unwrap）。当前 fuzz 抽查未发现 panic，但长期应把「已校验」变成类型/构造器保证，或至少集中审计。

---

## 5. CLI 功能核查

### 5.1 结论先说

**文档列出的 26 个命令全部实现，且主流程真实可跑；但仍有一组边界缺陷会让真实使用失败或误判。**
所以答案是：

> **CLI 功能「已实现」，但还不能说「已交付」。**

### 5.2 命令矩阵

| 命令 | 文档 | 实现 | 实测 | 结论 |
| --- | :---: | :---: | :---: | --- |
| `doctor` | ✅ | ✅ | ✅ | 能用，但只是静态能力声明，不做真实体检 |
| `schema template` | ✅ | ✅ | ✅ | 返回 bundled schema |
| `template inspect` | ✅ | ✅ | ✅ | JSON/`--cell`/分页正常 |
| `template validate` | ✅ | ✅ | ✅ | 支持 `--params-file -`、deferred 列表 |
| `template create` | ✅ | ✅ | ✅ | 5 个 preset 可写；目录已存在会报错 |
| `template patch` | ✅ | ✅ | ✅ | RFC 6902、dry-run、stdin、原子写 |
| `template compile` | ✅ | ✅ | ✅ | JSON → `.rpx`，legacy 入口 |
| `format to-json` | ✅ | ✅ | ✅ | RPX → JSON v1；preserve 带原始快照 |
| `format to-rpx` | ✅ | ✅ | ✅ | 未编辑快照原样回写；编辑后字段级 overlay 合并 |
| `format verify` | ✅ | ✅ | ✅ | 40/40 Preserve + 快照字节往返已实测 |
| `format check` | ✅ | ✅ | ✅ | v1/v0 reader 校验 |
| `format normalize` | ✅ | ✅ | ✅ | v0/v1 → v1 标记 |
| `run` | ✅ | ✅ | ✅ | 文件模式、SQL、参数、导出、运行记录 |
| `result inspect` | ✅ | ✅ | ✅ | range/origin/分页正常；**错误文件校验缺失** |
| `dataset inspect` | ✅ | ✅ | ✅ | CSV/Excel/SQL 都能取；**空 SQL 结果列名为空** |
| `catalog functions` | ✅ | ✅ | ✅ | 只列 20 个已实现函数，诚实 |
| `catalog examples` | ✅ | ✅ | ✅ | 5 个 preset |
| `library init` | ✅ | ✅ | ✅ | `--presets`、暂存目录、`.gitignore` |
| `report list` | ✅ | ✅ | ✅ | query/tag/分页；**一个坏包会让整表失败** |
| `report show` | ✅ | ✅ | ✅ | manifest/template/revision/result_state |
| `report runs` | ✅ | ✅ | ✅ | 运行记录/分页；**时间字符串排序有 bug** |
| `report copy` | ✅ | ✅ | ✅ | 目录复制、manifest id 改写 |
| `report patch` | ✅ | ✅ | ✅ | If-Match、dry-run、并发冲突 7 |
| `report validate` | ✅ | ✅ | ✅ | 使用 manifest 默认参数 |
| `report run` | ✅ | ✅ | ✅ | 修订校验、库内 `.runs`、导出 |
| `report update` | ✅ | ✅ | ✅ | 改 name/description，带 If-Match |
| `report import-rpx` | ✅ | ✅ | ⚠️ | **自产 `.rpx` 5/5 成功；40 个产品夹具 0/40** |

### 5.3 实测成功路径

```sh
rpx doctor
rpx schema template
rpx template create --example complex-master --out-dir /tmp/rpx-complex
rpx template validate /tmp/rpx-complex/template.json
rpx template inspect /tmp/rpx-complex/template.json --cell C6
rpx run /tmp/rpx-complex/template.json --out-dir /tmp/rpx-run --export xlsx --export csv
rpx result inspect /tmp/rpx-run/artifacts/result.json --origin G9
rpx library init /tmp/rpx-lib --presets
rpx report list --library /tmp/rpx-lib
rpx report show complex-master --library /tmp/rpx-lib
rpx report run complex-master --library /tmp/rpx-lib --if-match "$rev" --export csv
rpx report runs complex-master --library /tmp/rpx-lib
rpx report copy complex-master --as monthly --library /tmp/rpx-lib
rpx report patch monthly --library /tmp/rpx-lib --if-match "$rev2" --patch-file patch.json --dry-run
rpx report update monthly --library /tmp/rpx-lib --if-match "$rev3" --name 月度经营
```

结果：shape `23×25`、`G9=33975`、修订冲突退出 7、文本模式可用、stdin 参数/补丁可用、`--output text` 可用、默认 JSON 单行且 stdout 无额外日志。

### 5.4 CLI 缺陷清单（按严重度）

#### P0-1：`print-xlsx` 在多个官方 preset 上直接失败，且会拖垮整次运行

- 复现：

```sh
rpx run examples/tutorial/complex-master-expansion.json \
  --out-dir /tmp/rpx-print --data-dir examples/tutorial --export print-xlsx
# exit 6: EXPORT_FAILED: Excel 不支持此自定义纸张代码...
```

- 原因：`print_setup` 一旦声明 `paper_width`/`paper_height`，解析层把 `paper` 置为 **256**（自定义纸张哨兵）；`to_print_xlsx` 用 `u8::try_from(setup.paper)`，256 转换失败。
  位置：`crates/rpx-runtime/src/reportdef/printsetup.rs:132-134`、`crates/rpx-export/src/print_xlsx.rs:15-16`。
- 影响面实测（5 个 preset）：
  - `complex-master`：自定义纸张 → 失败
  - `chart-gallery`：`column_3d` 图表 → 失败
  - `images`：工作表背景 → 失败
  - `grouped-sales`、`multi-column`：成功
  - ⇒ **2/5 成功，3/5 失败**
- 更严重的是 `run::generate` 先写 `result.json`/`workbook.json`，然后在导出循环里返回错误；`run::execute` 失败时把整个 `.artifacts` 删掉并把 `artifacts` 置空。因此**一个打印导出失败会把已经算好的 result/workbook 一起丢掉**，run 直接 failed。
  位置：`crates/rpx-runtime/src/run.rs:151-181`、`run.rs:72-80`。
- 文档没有提示这些限制：`skills/rpx/SKILL.md:22`、`docs/agent-cli.md` 只说 `--export csv|xlsx|print-xlsx`；`doctor` 也把 `print-xlsx` 列为可用。
- CLI 测试只覆盖 `csv`/`xlsx`（`crates/rpx-cli/tests/workflow.rs:58-67`），没有 `print-xlsx` 成功/失败用例。

**建议**：短期二选一——① 按 `paper_width/height` 映射到标准纸张（420×297→A3），或② 在 CLI 入口做 preflight，明确报「该模板的打印布局不支持 print-xlsx」，同时**仍发布 result/workbook**，把导出失败作为 `warnings`/`artifacts_missing` 而非整体失败。中期把所有能力差异写进 `doctor`/`--help`。

#### P0-2：`report import-rpx` 对产品 `.rpx` 语料 0/40 成功

- 对 `tests/fixtures/*.rpx` 全量跑 `report import-rpx --dry-run`：
  - `ok=0 / fail=40`
  - `23 × /row_cells`、`15 × /cells/1`、`1 × /cells/7`、`1 × /cells/26`，全部 `RPX_IMPORT_UNSUPPORTED`
- 但对 Rust 自己 `template compile` 出来的 5 个 preset：**5/5 成功**。
- 测试 `all_bundled_examples_survive_explicit_rpx_to_json_conversion` 走的是「examples → compile → import」的自循环，不能证明产品 `.rpx` 可导入。
- 文档承诺「已有 RPX 使用显式导入命令管理」「源 RPX 保持原样」「已覆盖字面量、公式、显式主格、合并、样式、行列属性、参数、文件/SQL 数据集和打印配置」，与当前语料表现不一致。

**建议**：把 import 明确定位为「本工具生成 `.rpx` 的往返校验」还是「产品 `.rpx` 迁移」；如果是后者，先选 3–5 个产品夹具做逐字段差分，修 `row_cells`/cell 解码，再扩到 40 个；在 README/SKILL 里写清覆盖率和已知不支持项。

#### P1-1：断管 panic（退出码 101）

- 复现：`rpx report show ... | head -c 100` → `failed printing to stdout: Broken pipe`，退出 101。
- 位置：`main.rs:36`、`commands.rs:227-236`。
- 影响：agent 常用的 `| jq`、`| head`、`| grep -q` 都可能触发；101 不在协议退出码表里，调用方无法按码处理。
- 修法：用 `write_all` 写 stdout，遇到 `ErrorKind::BrokenPipe` 静默退出 0（或 141），不要 `println!`。

#### P1-2：`result inspect` 把模板文件当结果文件，返回 ok:true

- 复现：

```sh
rpx result inspect /tmp/rpx-complex/template.json
# exit 0; data.total=47; items 里是模板 cell 的 bold/font/text...
```

- 位置：`result.rs:63-80`，只检查 `cells` 是数组。
- 影响：agent 传错文件时不会得到「这不是结果」的明确错误，反而会基于错误数据继续推理。
- 修法：校验 `version == 1`、`rows/cols` 存在、每个 cell 至少含 `value` 与 `origin`；不满足返回 `RESULT_INVALID`（3）。

#### P1-3：`report runs`/`latest_attempt` 的时间排序错误

- 已用构造数据实测：`.99Z` 排在 `.9901Z` 前，后者才是更晚的时间。
- 位置：`library.rs:179-184`。
- 影响：`result_state`、`display_run_id`、`latest_attempt` 都可能选错运行。
- 修法：解析为 `time::OffsetDateTime` 比较，或在写 run 时同时存 `started_at_unix_nanos`；字符串只用于展示。

#### P1-4：一个损坏报表/运行记录会让 `report list`/`report show` 整体失败

- 在库中放一个 `manifest.json` 内容为 `{bad` 的目录，`report list` 直接 `exit 3 / JSON_INVALID`，其他正常报也被挡住。
- 位置：`library.rs:222-228` 的 `capture(root, &name)?`；`show_captured` 里 `runs` 也是 `?`。
- 影响：多 agent 共享库时，一次坏写会让所有人「看不到库」，排障成本高。
- 修法：`list` 对每个条目做隔离错误，返回 `items` + `diagnostics`（或 `invalid` 列表）；`runs` 跳过坏记录并报告。

#### P1-5：错误消息泄漏 Rust Debug 结构

- 复现：`rpx run` 一个含 `sort()` 的模板 → `CALCULATION_FAILED: Engine(UnsupportedExpr { expr: "sort(...)" })`；缺 SQL 配置 → `NoDataSource { dsn: "demo" }`。
- 位置：`execution.rs:175-178`。
- 修法：`RenderError` 的 `Display` 逐 variant 写人话，内部错误用 `{}` 而不是 `{:?}`；保持 `code`/`stage`/`pointer` 稳定。

#### P1-6：默认字符集/分隔符的静默误读

- 定义 JSON 的默认值是 **GBK + TAB**（`report-def-json.md:125-126`，`parse.rs:379`）。
- 实测一个 UTF-8、逗号分隔的 `data.csv`：
  - 未写 `encoding`/`delimiter` → `dataset inspect` 显示 `columns: ["code,name"]`、中文变成 `寮犱笁`，但命令 `ok:true`；`run` 直到公式找不到列才报 `UnknownCell`，错误信息也不指向「编码/分隔符」。
  - 写 `"delimiter": ","` 后，中文仍因默认 GBK 变 mojibake；写 `"encoding":"UTF-8"` 才正确。
- 这不是实现 bug（默认值有文档），但**是 CLI 的可用性缺陷**：最常见的 UTF-8/逗号 CSV 会静默产出错误数据。
- 建议：`dataset inspect` 在「`.csv` 且未显式 delimiter」时检查是否只有 1 列并提示；在「未显式 encoding」且字节是合法 UTF-8 且含非 ASCII 时提示「可能是 UTF-8，需显式声明」；`agent-cli.md` 把两个默认值写到显眼位置。

#### P1-7：SQL 空结果集丢列名

- `fetch_sql` 从 `rows.first()` 取列名；查询返回 0 行时 `cols=[]`。
- 实测 `dataset inspect` 对 `WHERE 1=0` 返回 `columns: []`，但 `ok:true`。
- 位置：`crates/rpx-ds/src/sql_source.rs:96-103`。
- 影响：空结果报表无法得到列结构；下游按列名绑定的公式/诊断会退化成「未知列」。
- 修法：用 `sqlx` 的 describe/列元数据（或 `LIMIT 0` 预查询）取列名，而不是依赖首行。

#### P1-8：HTTP 非 JSON 错误破坏 Web 错误处理

- `PUT` 非法 JSON → `text/plain` 400；错误 Content-Type → 415 文本。
- Web `request()` 无条件 `response.json()` → `SyntaxError`，而不是 `LibraryError`，用户看到的是解析异常而不是服务端原因。
- 位置：`http.rs:77-96`、`web/src/library-api.ts:12-16`。
- 修法：注册 `JsonRejection`/`DefaultBodyLimit` 的 JSON 错误处理器，统一返回 `{"error":{...}}`；Web 先看 content-type/文本兜底。

#### P2-1：`doctor` 名不副实

- 它返回的是静态 JSON：版本、支持格式、`offline:true`、`authentication:not_required`；不检查库是否可写、数据源配置是否可连、`node_modules`/Web 是否可用、`print-xlsx` 的已知限制。
- 建议：`doctor` 至少检查当前目录/library 可读、`--datasources` 文件可解析（不连接）、Rust/Node 版本；把能力差异写进 `capabilities` 字段。

#### P2-2：帮助文本过于简陋

- `rpx template --help`、`rpx report --help` 的子命令全是空白描述；`rpx run --help` 的参数也没有一句说明。
- 对一个「给 agent 用」的 CLI，`--help` 是第一发现入口；建议在 `args.rs` 给每个 `Command`/`Args` 加 `///` 文档、`value_name`、示例。

#### P2-3：没有「从 JSON 创建库内报表」的直接命令

- 当前只能：`template create/copy` → `template compile` → `report import-rpx`，或者先 `report copy` 再 patch。
- `import-rpx` 对产品 `.rpx` 又不可用，导致「agent 生成一份全新 JSON 报表放进库」的路径很绕。
- 建议：`report import-json <file> --id --library` 或 `report create --id --library --template`；`report delete`/`report prune` 也建议一并补。

#### P2-4：没有 PDF 导出

- `print-xlsx` 的错误提示反复说「请使用图形与打印预览保存 PDF」，但 CLI 没有 PDF 出口；只有 Web 的 `window.print()`。
- 如果 PDF 是产品能力，应在 CLI/服务端补；如果不是，错误文案不应引导到一个 CLI 做不到的动作。

#### P2-5：运行记录/产物没有保留策略，也没有删除命令

- `.runs` 会持续增长；每次包含 source、inputs、workbook、导出。多 agent 长跑会占满磁盘。
- 建议：`report prune --keep N/--older-than` + 库级配额 + 文档。

### 5.5 CLI 结论

| 问题 | 判定 |
| --- | --- |
| 命令是否实现？ | **是，27/27** |
| 主流程是否可用？ | **是**（创建、校验、运行、查询、库、修订、导入自产 RPX） |
| 是否达到可交付？ | **否**：`print-xlsx` 边界、`import-rpx` 覆盖、断管、错误文件校验、时间排序、错误文案 |
| 最该先修的三件事 | `print-xlsx` 的「失败不拖垮运行」；`import-rpx` 的承诺与语料；断管/错误文件/时间排序三个小修 |

---

## 6. Web 与 Server 核查

### 6.1 当前服务端

`crates/rpx-server/src/http.rs` 只有 8 个路由：

```text
GET  /health
GET  /reports
GET  /reports/{id}
PUT  /reports/{id}/template          # 必须 If-Match
GET  /reports/{id}/runs
GET  /reports/{id}/runs/{run}
GET  /reports/{id}/runs/{run}/source/{*path}
GET  /reports/{id}/runs/{run}/artifacts/{name}
```

实测：

- 缺 `If-Match` → 428；旧修订 → 412；成功 → 200。
- `/render`、`/export` → 404（符合新架构）。
- 运行记录/源码/产物在业务数据文件被删除后仍可读（固定产物）。
- 非法 JSON → 400 `text/plain`（见 §5.4 P1-8）。

**评价**：服务端职责收敛得很好（「目录库 API + 固定产物读取」），比旧版「HTTP 里跑引擎」清晰；但生产化缺认证、TLS、静态资源、请求超时、CORS/CSRF 策略、可观测性。

### 6.2 Web

- `web/src` 约 8.1k 行，13 个测试文件；构建通过。
- 设计上 Web 只做：读定义、编辑、保存（If-Match）、读运行记录/产物、打印预览；**不执行引擎**，与 `doctor.web_executes_engine=false` 一致。
- 样式/属性/行列/数据集面板都有逐字段补丁与纯逻辑测试；保存冲突保留草稿；`PrintPreview` 支持镜像和浏览器打印/PDF。
- 明显的历史残留：
  - `web/src/App.tsx:551` 的「关于」仍写 `/render`、`/export`；
  - `App.tsx:870/968` 的「另存为」注释仍写 `/meta`，但代码已不再调用；
  - `TemplateDatasets` 仍提示「运行文件数据集前，请上传模板引用的数据文件」，但 Web 已不执行运行，上传的文件只在浏览器内存里，没有服务端通道；
  - `library-api.ts:14` 无条件 `response.json()`，对服务端文本错误不健壮；
  - 「新建模板」后 `save()` 要求已有 `saved.id`，会提示「请用 CLI 创建或导入报表包」——这暴露了「Web 不能新建库内报表」的产品缺口。
- Web 文件规模：`App.tsx` 1088 行、`panes.tsx` 657 行、`stylemap.ts` 648 行；没有 ESLint/Prettier/文件长度门禁。

---

## 7. 风险登记表

| ID | 严重度 | 风险 | 证据 | 升级影响 |
| --- | :---: | --- | --- | --- |
| R1 | **P0** | 当前成果未提交 | `git status`：30 D / 75 M / 68 ??；`rpx-runtime`、`examples`、README/Makefile/skills 均未跟踪 | 从 HEAD 升级会丢失整套新架构；干净克隆无法构建当前 runtime |
| R2 | **P0** | `print-xlsx` 在 3/5 preset 上失败且拖垮运行 | `print_xlsx.rs:15-16`、`printsetup.rs:132-134`、`run.rs:151-181` | 用户请求打印导出会丢失已算好的 result/workbook |
| R3 | **P0** | `import-rpx` 对产品 `.rpx` 0/40 | 40 个 `tests/fixtures/*.rpx` 全失败；自产 5/5 成功 | 「已有 RPX 迁移」承诺不成立 |
| R4 | **P0** | 文档当前态/历史态混杂 | 62 `/render`、25 `/export`、23 `/meta`、20 `reportdef/` 引用；`report-def-json` 实现路径已过期 | 升级时按错文档改代码，风险极高 |
| R5 | P1 | 断管 panic 101 | `main.rs:36`、`commands.rs:227-236` 实测 | agent 管道使用不可靠 |
| R6 | P1 | `result inspect` 接受模板文件 | `result.rs:63-80` 实测 | 错误输入被当成功，污染 agent 推理 |
| R7 | P1 | 运行记录字符串排序错误 | `library.rs:179-184` 构造数据实测 | `latest_attempt`/显示结果可能选错 |
| R8 | P1 | 一个坏包阻塞整个 `report list` | `library.rs:222-228` 实测 | 多 agent 共享库可用性风险 |
| R9 | P1 | 错误文案是 Rust Debug | `execution.rs:175-178` 实测 | 难读、不稳定、无法给 agent 可靠修复提示 |
| R10 | P1 | 默认 GBK+TAB 静默误读 | `parse.rs:379`、`encoding.rs:54-68`；UTF-8/逗号 CSV 实测 mojibake/单列 | 最常见 CSV 直接产出错误数据 |
| R11 | P1 | SQL 空结果丢列名 | `sql_source.rs:96-103` 实测 | 空报表列结构不可用 |
| R12 | P1 | HTTP 非 JSON 错误 / Web `.json()` | `http.rs:77-96`、`library-api.ts:12-16` 实测 | Web 错误处理退化 |
| R13 | P1 | 无 CI / gate 非 hermetic | 无 `.github/`；本沙箱 G2/G3/G9 因环境红 | 升级回归无法自动守住 |
| R14 | P1 | workbook 大 Long/Decimal 精度损失 | `snapshot/value.rs:23-40` 实测 9007199254740993 → JS 不可保真 | Web 显示与 result.json 不一致 |
| R15 | P2 | `rpx-runtime` 过大 | 6.7k 生产行，职责 7+ 类 | 后续功能继续堆叠，测试/审查成本上升 |
| R16 | P2 | 无创建/删除/prune/PDF | CLI 命令面缺口 | 报表生命周期不完整 |
| R17 | P2 | Web 无 E2E 门禁 | G9 只 build+vitest | 真实浏览器/API 集成回归无自动保护 |
| R18 | P2 | 无 LICENSE/MSRV/发布元数据 | 无 `LICENSE*`；`Cargo.toml` 无 `rust-version` | 对外分发/合作/合规受阻 |
| R19 | P2 | 无性能/内存/大报表限制 | `fetch_all`、整文件读入；无 benchmark | 大报表可能 OOM/超时 |
| R20 | P2 | 无运行记录保留策略 | `.runs` 无限增长 | 长期 agent 库磁盘风险 |

---

## 8. 升级路线图

### 8.1 立即（0–2 周，先让升级有基线）

1. **整理并提交当前工作树**：`rpx-runtime`、`examples/`、`README.md`、`Makefile`、`skills/`、`web` 新文件、`Cargo.toml`/`Cargo.lock`；删除/忽略 `rpx.db`；打 `v0.2.0-baseline` tag。
2. **加 CI**：先跑 G1–G8 + Web build/test；把 `CARGO_TARGET_DIR`、`web/node_modules`、Vitest `cacheDir` 显式固定，保证 CI/沙箱可复现。
3. **修 P0 CLI 边界**：
   - `print-xlsx`：自定义纸张映射或 preflight；导出失败不删除 result/workbook，改为 `warnings` + `artifacts_missing`；补 5 个 preset 的 CLI 测试。
   - `import-rpx`：明确承诺；至少选 3 个产品 `.rpx` 做逐字段差分，把 `row_cells`/cell 解码补上；文档写清覆盖率。
4. **修 3 个小而致命的点**：断管（EPIPE）、`result inspect` 结果结构校验、`report runs` 时间解析。
5. **文档止血**：新增 `docs/README.md` 或 `docs/current-architecture.md`，标明「当前架构 / 历史里程碑」；给 `report-def-json.md`、`spec-m6.md`、`spec-m7-web.md` 加「已过期，当前实现见…」横幅；更新 `code-review.md` 的 oracle 描述。

### 8.2 近期（1–2 个月，让 CLI 可交付）

1. **CLI 完整化**：
   - `report import-json` / `report create` / `report delete` / `report prune`；
   - `doctor --deep`（库可写、datasource 可解析、Web/node 状态、能力矩阵）；
   - 所有命令的 `--help` 描述与示例；
   - `--export` 的每格式可用性说明（哪些模板类型不支持）。
2. **数据入湖诊断**：
   - `dataset inspect` 对 `.csv` 单列/UTF-8 疑似误读给出 warning；
   - SQL 用 describe 获取空结果列名；增加行数上限/流式选项；
   - 明确 `encoding` / `delimiter` 默认值在 `agent-cli.md`、SKILL、`--help` 中可见。
3. **库健壮性**：
   - `report list`/`runs` 隔离坏条目，返回 `diagnostics`；
   - 运行记录保留策略与 `report prune`；
   - 锁超时/诊断；
   - 运行 ID/时间戳排序用强类型时间。
4. **Web/Server**：
   - 统一 JSON 拒绝处理；Web 的 `request()` 兼容文本错误；
   - 清理 `/render`、`/export`、`/meta` 注释与「关于」文案；
   - 修正「上传数据文件」与「Web 不执行引擎」的 UX 矛盾；
   - 把 Playwright 关键 E2E（打开→编辑→保存→读运行→下载）挂进 CI；
   - 评估 `App.tsx`/`panes.tsx` 拆分与 ESLint。
5. **质量门**：
   - 覆盖率报告（行/分支）与阈值；
   - parser/expression 的 property-based/fuzz 测试（可用 `cargo-fuzz`/`proptest`）；
   - release 性能冒烟（大网格、大 CSV、pivot）；
   - `cargo audit`/`cargo deny`/`cargo machete`。

### 8.3 中期（1–2 个季度，让架构可持续）

1. **拆分 `rpx-runtime`**：
   - `rpx-definition`（JSON 定义 schema/parse/build/validate）；
   - `rpx-store`（目录库、锁、修订、运行记录）；
   - `rpx-app`（编排、快照、导入）；
   - 这样 CLI 只依赖 definition/store，不必拖所有 DB/导出。
2. **特性开关**：`sqlite` 默认、`postgres`/`mysql`/`xlsx`/`print` 可选；减小二进制和编译时间。
3. **产品版本升级沙盘**：
   - 记录当前 `.rpx` 夹具由哪个产品版本生成；
   - 把 `re_work`/产品 JRE/生成器作为可选外部依赖写进 `docs/conformance.md`；
   - 每次产品升级先跑「夹具重生成 + 差异清单」，再改 engine。
4. **发布工程**：LICENSE、`rust-version`、CHANGELOG、release profile（LTO/strip）、语义版本、制品签名；服务端认证/TLS/静态资源部署方案。
5. **可观测性**：结构化日志、运行 ID 贯穿 CLI/HTTP、慢查询/慢计算指标、错误码字典。

### 8.4 升级验收门（建议写进 CI）

- 干净克隆 + `cargo test --workspace --locked` 全绿；G1–G9 全绿。
- CLI 22 命令的 happy path + 失败路径全部有真实进程测试。
- 40 个产品 `.rpx` 的 import 成功率有明确基线（不是 0/40）。
- 5 个 preset 的 `csv`/`xlsx`/`print-xlsx` 成功/失败矩阵有测试且与文档一致。
- 断管、错误文件、坏包、空 SQL、时间排序、编码/分隔符均有回归测试。
- 文档当前态索引存在；`grep -r '/render' docs` 要么清零，要么明确标注历史。
- release 性能基线（大网格、645 行 CSV、pivot）不退化。
- `cargo audit` 无未处理高危；LICENSE/MSRV/发布元数据齐全。

---

## 9. 附录

### 9.1 复现命令

```sh
# 构建与测试（本机沙箱需要覆盖 target dir）
CARGO_TARGET_DIR="$PWD/target" RUSTC_WRAPPER="" cargo test --workspace --no-fail-fast
CARGO_TARGET_DIR="$PWD/target" RUSTC_WRAPPER="" cargo clippy --workspace --all-targets -- -D warnings
CARGO_TARGET_DIR="$PWD/target" RUSTC_WRAPPER="" cargo fmt --all --check

# 门禁（G9 在可写 node_modules 缓存的环境里才绿）
CARGO_TARGET_DIR="$PWD/target" RUSTC_WRAPPER="" bash scripts/gate.sh

# 端到端
CARGO_TARGET_DIR="$PWD/target" RUSTC_WRAPPER="" bash scripts/e2e_smoke.sh

# CLI 关键实测
target/debug/rpx run examples/tutorial/complex-master-expansion.json \
  --out-dir /tmp/rpx-print --data-dir examples/tutorial --export print-xlsx
target/debug/rpx report import-rpx tests/fixtures/simple.rpx --id x --library /tmp/l --dry-run
target/debug/rpx result inspect examples/tutorial/complex-master-expansion.json
target/debug/rpx report show complex-master --library /tmp/lib | head -c 100
```

### 9.2 关键文件索引

| 主题 | 文件 |
| --- | --- |
| crate 分层/门禁 | `scripts/gate.sh`、`scripts/dep_allowlist.txt`、`docs/code-review.md` |
| CLI 入口/参数 | `crates/rpx-cli/src/main.rs`、`args.rs`、`commands.rs` |
| 运行编排 | `crates/rpx-runtime/src/run.rs`、`execution.rs`、`data.rs` |
| 目录库/修订 | `crates/rpx-runtime/src/library.rs`、`library/manage.rs`、`files.rs` |
| 定义 JSON | `crates/rpx-runtime/src/reportdef/*`、`template.schema.json` |
| 引擎主链 | `crates/rpx-engine/src/stage_a.rs`、`stage_b.rs`、`ext_algo/`、`eval/`、`merge/`、`pager/` |
| 表达式 | `crates/rpx-engine/src/expr/*`、`crates/rpx-engine/src/expr/names.rs` |
| 数据源 | `crates/rpx-ds/src/file_source.rs`、`sql_source.rs`、`dialect.rs` |
| 导出 | `crates/rpx-export/src/{csv.rs,xlsx.rs,print_xlsx.rs}` |
| 快照 | `crates/rpx-runtime/src/snapshot/*` |
| HTTP | `crates/rpx-server/src/http.rs`、`main.rs` |
| Web | `web/src/App.tsx`、`library-api.ts`、`PrintPreview.tsx`、`designer.ts`、`stylemap.ts` |
| 当前 CLI 文档 | `docs/agent-cli.md`、`docs/agent-cli-design.md`、`skills/rpx/SKILL.md` |

### 9.3 一句话给下一次升级

> **把「当前树」先变成一个可回滚的 release baseline；然后用真实失败场景（print-xlsx、import-rpx、断管、错误文件、空 SQL、坏包、时间排序）把 CLI 的边界补齐；最后再拆 runtime、补 CI/文档。**
> 这套项目的算法底子值得保留，不要为了「重写得更现代」而丢掉已经对拍过的语义。
