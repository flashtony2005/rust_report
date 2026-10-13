# Spec — ReportLite Rust 重实现 · M2（rpx-engine：扩展计算 / 求值 / 同值合并 / 分页）v0.9

> 生成日期：2026-10-12 · 项目总监  
> 上游：`docs/spec-m1.md` v1.4（已交付并经独立验证放行）、`docs/format-contract.md` 附件 A-L  
> 设计依据：`re_work/reportlite-rust全链路设计.html` §5（arena / 主格图 / 扩展包络 / 附属格链）与 §6（6.1 扩展算法 / 6.2 求值调度 / 6.3 同值合并与分页）  
> 状态：**草案待用户确认**（原定由首席架构师产出，其运行因 API 429 限流中断，由总监接管）

---

## 0. 本文件的可信度声明

M1 的规格由架构师在源码级核实后产出；**本文件不同**——原架构师在写出任何内容前即因 429 失败（`tests/fixtures/` 与 `docs/` 盘面确认零产出）。因此：

- ✅ **已核实**（附 `文件:行号` 或实测证据）：主格判定规则、行列引擎字段、headless 计算链路、extendMode 行为、真合并编码、**`$` 引用语义**（附件 G.1）、**源格查找 `getSource` 契约**（附件 G.2）、**循环引用检测点**（附件 G.3）、**PropQueue 是死代码**（附件 F.2）、**求值语义三条规则**（附件 H.1）、**聚合的 List 判定与数据集引用关系**（附件 H.2）、**类型即契约**（附件 H.3）、**extendMode 运行时改写与行数公式**（附件 H.4）、**分页基准已跑通**（附件 H.5）、**行类型常量表**（附件 H.6）、**求值调度四通道与状态机**（附件 K.1）、**`mergeSame`/`mergeNull` 四趟扫描源码级规则**（附件 K.2/K.3）、**属性表达式与行高来源**（附件 L.1）、**源坐标 `src_row` 语义**（附件 I.9）、**表达式分类与 `is_extend` 真值表**（§5.1）。
- ⚠ **待验证**（禁止当结论用）：`SimplePager._$5:274+` 主循环的分组/分栏分支、`prepProperties`、`perfected`（`llllllIIlIIIlllI`）、函数注册时机、**按分组键动态生成分组头**（`ds1.group(...)` + `DSGroup`）这条路径未验证。

## 1. 产品定义

M2 把 M1 的**模板态**（`ReportDefine`：矩阵 + 行高列宽 + 单元格属性）推进到**计算态与分页态**：

- **一句话**：实现 `rpx-engine`，把模板态矩阵按扩展方式放大、按表达式求值、按同值规则合并、并按行高分页，产出可直接渲染的扁平矩阵 + 分页水位。
- **消费者**：M3 的 axum 服务（`POST /api/reports/{id}/render`）、M4 的 PDF 排版（分页语义引擎里已有，见设计文档）。
- **为什么现在做**：设计文档的结论是整个 Rust 工程的难点被压缩到两件事——`.rpx` 解析（M1 已完成）与**扩展计算**（M2）。

## 2. 范围切分（证据驱动）

源码规模决定了这不是一个能一次做完的里程碑：`ExtCellSet` 3781 行、`SimplePager` 3403 行、`ExtNormalCell` 2719 行、`Perfected` 1643 行、`llllllIIlIIIlllI` 1406 行；表达式侧类目过百（`DS*` 系列 + 数学/字符串/日期）。

| 级别                  | 内容                                                                                                                                             | 判据                                                                                                                                            |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| **P0（M2 必做）**       | ① 主格图 + 扩展算法（extendRow / extendCol，含 extendMode 运行时改写）② 附属格（subCells）③ 表达式**算术与单元格相对引用**子集 ④ 聚合的 **List 判定**（非 Excel 区域求和）⑤ 同值合并基础 ⑥ **值类型保真** | **对拍已验证可行**：探针能拿到 value / src / rowMerge / colMerge / extend / subCells，且 4 个 `engine_*.expected.json` 已含 `calculated` 段（`RUNLOG.md` §10-§11） |
| **P1（M2  stretch）** | ① 分页（SimplePager 等价）② 函数表达式面（`sum` 等，按 List 语义）③ 分组与分组头                                                                                        | **分页基准已跑通**（`engine_pager`：4 页 × 14 行，分组头每页重复），但 `SimplePager` 3403 行契约未逐行读 → 实现放在 P1 更稳；分组头目前只验证了「独立 `-90` 行」一种构造                            |
| **P2（明确延后）**        | 子报表递归展开、`BigEngine` 大报表路径、`Perfected` / `llllllIIlIIIlllI` 的调度与性能优化、全函数面、按分组键动态生成分组头                                                           | 契约工作量与收益不匹配 P0；M3/M4 再评估                                                                                                                      |

> **分页为什么仍在 P1（v0.3 复议）**：原本放 P1 的理由是「基准未验证、无法证明正确」。该理由**已消除**  
> ——`ReportUtils2.getPagerInfo` 链路跑通，`engine_pager` 给出了可逐页断言的基准（每页 14 行 =  
> 297mm/20mm，分组头重复）。但 `SimplePager` 3403 行尚未逐行读，剩余工作量仍是 M2 里最大的一块；  
> 若要把分页拉进 P0，需要先补读契约——这是一次范围决策，交用户定。

## 3. 明确不做

| 不做             | 原因                            |
| -------------- | ----------------------------- |
| 表达式下推前端        | 设计文档已定：MVP 全部服务端算（保真）         |
| 并发 / 异步        | M2 纯同步；M3 按需引入                |
| 分栏分页           | 依赖 P1 的分页完成后再评估               |
| 手写 .rpx writer | 沿用 M1 纪律：writer 由 Java 夹具程序承担 |

## 4. 技术架构

| 项     | 值                                       | 说明                                                           |
| ----- | --------------------------------------- | ------------------------------------------------------------ |
| crate | `crates/rpx-engine`（新增）                 | 只依赖 `rpx-core`（path），**零第三方依赖**                              |
| 表示    | arena（`Vec<Cell>` + `Vec<Vec<CellId>>`） | 沿用 M1 的 `store`/`grid` 形态，M2 换成扩展态；设计上 `main_of` 图与 arena 并存 |
| 求值    | **显式状态机**（Java 用异常协程，Rust 不学）           | 见附件 F.4                                                      |
| 表达式   | 手写 lexer + Pratt parser + 求值器（零依赖）      | P0 只支持算术 + 相对引用；P1 扩函数面                                      |
| 错误    | 复用 `rpx_core::RpxError` + 新增引擎变体        | 不引 thiserror                                                 |

### 4.1 crate 布局

```
crates/rpx-engine/src/
├─ lib.rs          # pub use 公开面；pub fn calculate(...) / paginate(...)
├─ error.rs        # 引擎错误（循环引用、未支持函数、分页不可能等）
├─ arena.rs        # 扩展态 arena：cells: Vec<Cell> + grid: Vec<Vec<CellId>> + main_of: Vec<MainOf>
├─ main_cell.rs    # 主格判定（MainCell.java:21-39 忠实移植）
├─ extend.rs       # extendRow / extendCol + 扩展包络
├─ sub_cells.rs    # 附属格（probe 的 subCells 语义）
├─ merge.rs        # 同值合并（mergeSame / mergeNull，属性码 97-100）
├─ expr/
│  ├─ lexer.rs     # 改良 token 流
│  ├─ parser.rs    # Pratt parser → AST
│  └─ eval.rs      # 求值 + 相对引用解析 + 循环引用检测
└─ pager.rs        # P1：SimplePager 等价（届时再加）
```


## 5. 引擎契约摘要

完整契约见 `docs/format-contract.md` **附件 F / G / H / I / J / K / L**（含逐条 `文件:行号`）。要点：

| 机制                 | 契约                                                                                                                                                                                   | 证据                                                                                                                                    |                                                              |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| 主格（MainCell）       | 记录 `(leftHead, topHead)` + `changed`；`changed==false` 时若 `(left==cell00 && top==cell00) \|\| left != leftHead` → 两头都更新且 `changed=true`；否则仅 `top` 变才更新；都不变则 `changed=false`（复用同一主格对象） | 附件 F.1、`MainCell.java:21-39`                                                                                                          |                                                              |
| **`$` 引用**         | **`$` 是整格绝对引用前缀（`$B2` 可用）；产品不支持 Excel 的 `$B$2`** —— 代码只剥一个开头 `$`，剩余串交 `CellLocation.parse`                                                                                           | 附件 G.1、`Expression.java:193-203`                                                                                                      |                                                              |
| 源格查找               | `getSource(id)`：`CellLocation.parse` → 边界 `0<row<rowSize && 0<col<colSize`（或 `row==0&&col==0`）→ 返回 `NormalCellEngine.srcCurrent`；失败返回 `null` 不抛                                      | 附件 G.2、`ExtCellSet.java:610-621`                                                                                                      |                                                              |
| 循环引用检测             | 沿 `getCellLeftHead()` 链用栈走回自己 → 构造诊断器 `IIllIIIIIIIIIlll(cell,3)` 抛 `ReportError`                                                                                                     | 附件 G.3、`ExtCellSet.java:641-646`                                                                                                      |                                                              |
| 行列引擎               | `cs`/`srcPrepare`/`srcCurrent`/\`sourceRow                                                                                                                                           | Col`+ 表达式槽（行：visible/height/breakPage/breakColumn/adjustRowHeight；列：visible/width/breakPage/breakColumn）+`customPropEngMap\`(ByteMap) | 附件 F.3、`RowCellEngine.java:25-32`、`ColCellEngine.java:24-31` |
| 表达式落盘形态            | 引擎态序列化时表达式写 `toString()`（读回是字符串、运行时再 parse）→ **.rpx 里表达式就是字符串**（如 `=B2*2`）                                                                                                           | 附件 F.3、`RowCellEngine.java:47-61`                                                                                                     |                                                              |
| extendMode         | 16=默认 / 17=不扩展 / 18=垂直扩展 / 19=水平扩展；实测 18 的主格带 3 个 subCells                                                                                                                           | 附件 F.5、`props.rs::def`、`probe_out.txt` [C]                                                                                            |                                                              |
| 合并区                | 真合并（`-1` 空记录）下全区持锚点 id；浅合并下非锚点持自己的默认格                                                                                                                                                | 附件 E.8、`realmerge.rpx` 实测                                                                                                             |                                                              |
| ~~扩展队列 PropQueue~~ | **死代码**：reportlite 全量 grep 无任何使用者 → **不属于 M2 结构**，用 `Vec<(CellId,u8)>` 即可                                                                                                            | 附件 F.2                                                                                                                                |                                                              |
| **求值语义**           | **不是 Excel 语义**：`sum(A:B)` ≡「把 B 设为 current 后求值 A 一次」；两操作数形式必须报错；**只有操作数求值为 `List` 才聚合**（⛔ 具名以 `engine_sum` 夹具为准：**未被**扩展格引用的 `ds2` ⇒ `ds2.sum(qty)`=450；**被引用的** `ds1` ⇒ `ds1.sum(qty)`=100/200/150。旧文写的"`ds1.sum(qty)`=450"是**早期单数据集实验的口径**，与第 94 行冲突，已于 2026-10-13 更正；机制见 `docs/m2-engine-design.md` §5.2）                                                                         | 附件 H.1、`Sum.java:104-106,112-121,174-190`                                                                                             |                                                              |
| **聚合上下文**          | 聚合结果取决于**数据集是否被扩展格引用**：被引用则逐行（100/200/150），未被引用则全量（450/450/450）                                                                                                                      | 附件 H.2、`engine_sum` 双数据集实测                                                                                                            |                                                              |
| **类型即契约**          | 求值结果**禁止统一提升 f64**：`Integer(100)` 必须产出 `int`，否则全表 FAIL                                                                                                                               | 附件 H.3、expected.json 的 `type` 字段                                                                                                      |                                                              |
| **extendMode 改写**  | 声明 16/18/19/17 **运行时按 leftHead/topHead 拓扑改写**为实际值（16 自动升 18）；extend=18 行数 = 模板行数 + (数据集行数 − 1)                                                                                       | 附件 H.4、`ExtExtendCell.prepareCalculate:314-328`                                                                                       |                                                              |
| **分页基准**           | 每页行数 = 页高 / 行高累加（实测 14 行），`row_num_per_page=20` 只是 propMap 槽 33 提示值不参与切页；独立 `-90` 分组行每页重复且计入页高                                                                                       | 附件 H.5、`engine_pager` 实测 4 页                                                                                                          |                                                              |
| **行类型常量**          | `TITLE_HEADER=-96` / `TABLE_HEADER=-95` / `TABLE_FOOTER=-94` / `PAGE_HEADER=-93` / `PAGE_FOOTER=-92` / `NORMAL=-91` / `GROUP_HEADER=-90`                                             | 附件 H.6                                                                                                                                |                                                              |
| **求值调度（异常协程）**    | Java 用**抛异常当 switch**：`testValue`/`calculate` 抛 `(cell, state)`，调度方 catch 后换格。四条通道：state4 扩展格（先 19 后 18）→ state5 表达式格 → state6 special 队列 → state8 级联失效；state 1/3 是**错误态**。⛔ Rust **必须**写成 trampoline + 显式 `loop`，**禁止递归求值**；special 重入**必须**加上限 | 附件 K.1、`Node.java:45-53`、`ExtCellSet.java:1297-1458` |
| **`nonCalc` 位语义**      | `rowMerge` 的 **bit30 = 已计算**、**bit31 = 正在算**；`serialize` 时被 `& 0x7FFFFFFF` 抹掉 ⇒ **对拍永远看不到这两位**，不得计入期望值 | 附件 K.1、`ExtNormalCell.java:287-298,322-324,2579` |
| **`mergeSame` 四趟扫描**   | 锚点谓词 = `value != null && 自身即左上角`；四趟 = 纵正/纵反/横正/横反，值域 `{98,100}`（纵向）/`{99,100}`（横向），`mode == 120` 走反向两趟。`mode` 118 比 `getSource()` 对象同一、119 比**左邻格**、120 反向 | 附件 K.2、`ExtCellSet.java:1832-2049` |
| **合并互斥守卫**          | `mode == 118` 时 `_$6/_$5`（:1790-1830）检查邻列/邻行是否有**跨边界**的合并格，命中即回退一格。⛔ 必须保留，否则会把两个独立合并区并成一块且**不报错** | 附件 K.2、`ExtCellSet.java:1790-1830,1890-1892` |
| **`mergeNull` 方向语义**   | 属性码标在**空值格**上，含义是「空格往哪个方向找值」：114=空格向上找（锚点向下吞）→ 116 反向（锚点 `addRow(-rm)` **上移**）→ 115 向左找 → 117 反向（锚点 `addCol(-cm)` **左移**）。锚点谓词**不含** `value != null` | 附件 K.3、`ExtCellSet.java:2051-2159`、`INormalCell.java:120-124` |
| **源坐标语义**          | `source.src_row`/`src_col` 是**模板态坐标**（`engine.sourceRow` 只在构造时写一次，`ExtNormalCell.java:127`；扩展**只改 `cell.row`**）⇒ ⛔ 不得用它索引计算态网格、不得用它反查模板行。断言 `calculated` 时必须与**模板网格**比对 | 附件 I.9、`RUNLOG.md` §19.5 |
| **属性表达式（行高/可见/分页符）** | 模板 `RowCell.expMap` 码 **1=行高、2=可见、13=分页符、15=分栏符、7=调行高**；`ColCell.expMap` 码 **11=列宽、12=列可见**。求值在 `calculate()` 内、分页**之前** ⇒ **行高可由表达式算出，是分页正确性的前提**。NormalCell 侧用 `getSingleValue`（List 取首元素）且**跳过 key 49/41**；行列侧用 `getValue`（不拆 List） | 附件 L、`ExtRowCell.java:66-91`、`ExtColCell.java:70-90` |


### 5.1 表达式切片契约：`is_extend` 与分类真值（v0.7 新增，架构师复核）

`is_extend` 决定一个格是否成为扩展格，**M2-02 / M2-15 直接依赖它**；判错不会报错、只会静默少扩展或多扩展，故单列为契约。

#### 5.1.1 `is_extend` 的节点级规则（**最根本的一层，先看这个**）

入口 `Expression.isExtend()` = `home.isExtended()`（`Expression.java:561-566`）。全包 `isExtended()` **只有 17 处覆写**，穷举如下：

| 节点类 | 规则 | 证据 |
| ------ | ---- | ---- |
| `Node`（基类） | **false** | Node.java:82-84 |
| `CSVariable` | **false**（格引用本身永不扩展） | CSVariable.java:379-381 |
| `Constant` | `value instanceof List` | Constant.java:22-24 |
| `ValueList` | `param == null \|\| param.getType() != ':'` | ValueList.java:30-31 |
| `Operator` | `left.isExtended() \|\| right.isExtended()` —— **任一操作数扩展即扩展** | Operator.java:51-55 |
| `SpecialFunction`（**所有函数**的基类） | **只**看 `left`，`super` 分支恒 false ⇒ **参数里的扩展不参与判断** | SpecialFunction.java:27-32 |
| `Link` | **false**（显式覆写掉 `Operator` 的「或」规则） | operator/Link.java:100-102 |
| `DSSelect`/`DSGroup`/`DSPlot`/`DSFilterCross` | **恒 true** | DSSelect.java:23-25、DSGroup.java:23-25、DSPlot.java:21-23 |
| `Sort`/`To`/`SubList`/`SplitString` | **恒 true** | Sort.java:19-21、To.java:17-19、SubList.java:18-20 |

⛔ **最容易实现错的一处**：`Operator` 是「**或**」（参数里有扩展就扩展），`SpecialFunction` 是「**只看 left**」（参数里有扩展也**不**扩展）。
由上表可直接推出 5 条验收真值：`ds1.select(x)`=**true**、`B2*2`=**false**、`sort/to`=**true**、`Constant(List)`=**true**、`sum(ds1.select(qty))`=**false**（`DSSum` 未覆写 ⇒ 继承 false；`SpecialFunction` 只看 left 且 left 为 null）。

#### 5.1.2 语法 → 节点 → 真值

| 语法 | 产品真实节点 | `is_extend` 真值 | 证据 |
| ---- | ------------ | ---------------- | ---- |
| `{…}` | `Select`：内容进 `param`，`left` 是**前一个操作数** | 走 `SpecialFunction` 的「只看 left」⇒ `{ds1.select(x)}` = **false** | `Expression.java:131-132`、`:445`；Select.java:21-22；`SpecialFunction.java:27-32` |
| `$[…]` | **`Constant(Escape.remove(内容.trim()))`**——字符串常量 | 恒 false | `Expression.java:169-171` |
| `@@名` / `@名` | `Constant(ctx.getParamValue(id))` | **参数值是 `List` 时 = true** | `Expression.java:185`、`:188`；`Constant.java:22-24` |
| 裸参数名 | `ParamNode extends Constant` | 同上 | `Expression.java:233`；`ParamNode.java:8-9` |
| `[a:b,c]` vs `[a:b]` | `ValueList` | 升级序 `\0`→`;`→`,`→`:` 取**最粗** ⇒ `[a:b,c]`=**true**、`[a:b]`=**false** | `ValueList.java:30-31`；`ParamParser.java:66-78` |
| `$B2` | `CSVariable(cell, true)` | 恒 false；`$` 是整格前缀，无 Excel 双 `$` | `Expression.java:195-196`；`CSVariable.java:379-381` |

#### 5.1.3 标识符的**完整判定次序**（`Expression.createNode`，9 步，顺序即优先级）

| 次序 | 判据 | 落点 | 行号 |
| ---- | ---- | ---- | ---- |
| 1 | 关键字 `and`/`or`/`not`/`in`（`equalsIgnoreCase`） | `And/Or/Not/In` | :157-168 |
| 2 | `$` + `[…]` | `Constant` | :169-172 |
| 3 | **`前缀.名`（前缀是已注册数据集）** | `createDSNode(..., true)` | **:174-177** |
| 4 | `@` 单独 / `@名` / `@@名` | `CSCurrent` / `Constant` | :178-192 |
| 5 | `$格号` | `CSVariable(cell, true)` | :193-200 |
| 6 | **格号**（`cs.getSource(id) != null`） | `CSVariable(cell, false)` | :201-203 |
| 7 | 数据集列 | `DSVariable` | :204-206 |
| 8 | `preNode instanceof DSMember` 回退 | `createDSNode(..., true)` | :207-209 |
| 9 | **函数调用**：必须**紧跟 `(`**；先查普通函数表，失败再试 DS 函数 | `Function` | :210-228 |

⚠ 准确表述是「**带点名 → `@`/`$` → 格号 → 数据集列 → 函数**」，**不是**「格号优先」。函数名支持 `名@选项`（:212-218）。`in` 等关键字的 `equalsIgnoreCase`（:157-168）与函数名查表的**大小写敏感**（下）是**两套机制**，勿混。

#### 5.1.4 函数面规模与查表纪律

- 注册总量 **139** = **15 个 DS 函数**（`FunctionLib.java:106-120`）+ **124 个普通函数**（`:121-254` 实测 124 次）；`barcode`（`:157`）与 `json`（`:245`）包在 `try/catch` 内，按「注册成功」计。
- 查表 `isFunName` → `_$4.containsKey(id)`（`FunctionLib.java:20` 声明 `HashMap(250)`、`:64-66`）⇒ **大小写敏感**，登记名全小写。
- **`priority` 不是一律 19**（`Node.java:54`）：`lefthead`=**17**（`function/LeftHead.java:22`）、`tophead`=**17**（`function/TopHead.java:22`）、`mcorder`=**16**（`function/McOrder.java:110`）；其余 **136** 个未赋值。
- ⛔ **不实现位运算/移位**：`Node.java:13-44` 的 `PRI_ASS/BOR/BXOR/BAND/SHIFT/NEW` 在 reportlite 无使用者。

## 6. 验收标准（EARS）

| 编号    | 功能                   | 验收标准                                                                                                                                  | 级别     |
| ----- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| M2-01 | 主格图                  | While 模板含相邻同区域格，扩展后的 `main_of` 分组 **必须**与 `MainCell.java:21-39` 的判定逐步一致                                                               | P0     |
| M2-02 | 扩展算法                 | While 单元格 `extendMode=18` 且数据集有 N 行，扩展态矩阵行数 **必须** = 锚点行 + N，附属格集合 **必须**与 Java 探针 `subCells` 逐一相等                                    | P0     |
| M2-03 | 非扩展格                 | While `extendMode=17`，**必须**不产生附属格、矩阵不放大                                                                                              | P0     |
| M2-04 | 合并区扩展                | While 锚点格带 Area，扩展后合并区跨度 **必须**与模板态一致，且**必须**与 Java 探针的 `rowMerge`/`colMerge` 相等                                                      | P0     |
| M2-05 | 算术求值                 | While 表达式为 `B2*2` 形态，计算值 **必须**与 `engine_expr.expected.json` 逐格相等（**含类型**）                                                            | P0     |
| M2-06 | 相对/绝对引用              | While 表达式引用同区地址，**必须**按扩展实例各自解析（不得全表共用一个结果）；`$` 前缀**必须**按 ReportLite 语义实现为「整格绝对引用」，**禁止**实现成 Excel 的行列双 `$`（附件 G.1）                   | P0     |
| M2-07 | 循环引用                 | While 表达式形成环，**必须**返回引擎错误（等价 `CircularRefException`），**不得** panic 或死循环                                                                | P0     |
| M2-08 | 同值合并                 | While 相邻格值相同且启用 mergeSame，**必须**合并为同一 CellId（对照属性码 97-100）                                                                            | P0     |
| M2-09 | 对拍链路                 | `engine_*.expected.json` 的 `calculated` 段**必须**被逐格断言（值 + 类型 + `src_row`/`src_col`/`extend`/`sub_cells`），**不得**以"测试通过"替代比对             | P0     |
| M2-10 | 零依赖与门禁               | `rpx-engine` **必须**零第三方依赖；所有 `.rs` ≤300 行；`clippy --all-targets -- -D warnings` 零告警                                                   | P0     |
| M2-13 | **聚合语义（非 Excel）**    | While 公式为 `sum(A:B)`，**必须**等于「把 B 设为 current 后求值 A 一次」（`sum(B2:B4)`=100，**不是 450**）；两操作数形式**必须**报错；`ds2.sum(qty)`（**未被**扩展格引用的数据集，List 操作数）**必须**=450，而**被引用的** `ds1.sum(qty)` **必须**=100/200/150（`engine_sum` 夹具实测；机制 `docs/m2-engine-design.md` §5.2） | P0     |
| M2-14 | **类型保真**             | While 求值产出 `Integer`/`Long`/`Double`/`String`/`List`，**必须**原样保留类型；**禁止**统一提升为 f64                                                     | P0     |
| M2-15 | **extendMode 运行时改写** | While 声明 `extendMode=16` 且拓扑为竖向，**必须**按 18 处理（矩阵 = 模板行数 + 数据集行数 − 1）；`17` **必须**产出整列 `List` 而非逐行展开                                    | P0     |
| M2-16 | **`is_extend` 判定** | **必须**按 §5.1.1 节点级规则实现（`Operator`=或；`SpecialFunction`=只看 left；`CSVariable` 恒 false；`Constant`=List）。5 条基准真值必须逐条为真：`ds1.select(x)`=true、`B2*2`=false、`sort/to`=true、`Constant(List)`=true、`sum(ds1.select(qty))`=**false**。⛔ 禁止用「出现数据集引用」等形态近似替代 | P0 |
| M2-17 | **分类顺序** | **必须**按 §5.1.3 的 9 步次序，函数分支**必须**要求紧跟 `(`；⛔ 不得把「格号优先」当全局规则 | P0 |
| M2-18 | **函数表** | 登记名**必须**全小写且**大小写敏感**（139 个）；`priority` **必须**实现三档例外（`lefthead`/`tophead`=17、`mcorder`=16、其余=19） | P0 |
| M2-19 | **不实现位运算/移位** | 表达式含 `&`/`|`/`^`/`>>` **必须**返回「不支持的语法」错误 | P0 |
| M2-20 | **求值调度等价** | 调度**必须**与四条通道等价（state4→5→6→8）；state 1/3 **必须**映射为 `EngineError`；special 重入**必须**有上限；⛔ **禁止递归求值** | P0 |
| M2-21 | **合并四趟与守卫** | `mergeSame ∈ {98,99,100}` **必须**按四趟扫描（120 反向、119 比左邻、118 比 `getSource()`）并保留 `_$6/_$5` 守卫；`mergeNull` 114/116/115/117 **必须**按「空值格指向值格」实现，第 2/4 趟**必须**先移动锚点再回填 | P0 |
| M2-22 | **属性表达式** | 行/列属性表达式**必须**在分页前求值并写回；NormalCell **必须**跳过 key 49/41 且用 `getSingleValue`，行列侧**必须**用 `getValue` 不拆 List；`Suspend{cell,state}` **必须**与值通道共用同一套 trampoline | **P1** |
| M2-23 | **源坐标语义** | 断言 `calculated` 的 `src_row`/`src_col` **必须**与**模板网格**比对；⛔ 不得用 `src_row` 索引计算态网格或反查模板行；同一模板格的 N 个实例 `src_row` **必须**相等；横向（19）行号**禁止**拟合 | P0 |
| M2-11 | 分页                   | While 行高累加超出页高，**必须**按页高/行高断页（实测每页 14 行），**不得**使用 `row_num_per_page`；与 `engine_pager` 基准逐页相等；分组头每页重复                                  | **P1** |
| M2-12 | 函数面                  | While 表达式为 `ds1.avg/max/count(...)`，计算值 **必须**与 Java 基准相等                                                                             | **P1** |

## 7. 边界与约束

- **对拍基准链路（已验证）**：`IReport rd = ReportUtils.read(path); IReport rpt = new Engine(rd, new Context()).calc();`  
  —— 证据 `tests/fixtures/ProbeCalc.java:76-77,91-92,127,146` 与 `probe_err.txt` 栈帧 `Engine.calc(Engine.java:85)` → `calcDataSet(:131/:147)`。
- **数据集**：内存注入（`new Context()` + `rd.setDataSetMetaData(dsmd)`，Phase D）或落 CSV 走真实 `FileDataSetFactory`（Phase C）。**缺数据集是致命错误**（Phase B 实测 `ReportError`）。
- **禁止手搓字节**：所有夹具由产品 writer 产出。
- **数值**：Java 侧 Integer/Long/Double 区分必须保留（M1 已有 `ObjValue` 对应）。

**M2 基准夹具（9 个，均产品 writer 产出 + `--verify` PASS；2026-10-12 独立重编译后与仓库副本逐字节相同）**

| 夹具                            | 模板  | 数据集                 | 计算态  | 覆盖点                                                                                              |
| ----------------------------- | --- | ------------------- | ---- | ------------------------------------------------------------------------------------------------ |
| `engine_sum.rpx`（9413B）       | 6×9 | 双数据集 ds1/ds2（各 3 行） | 8×9  | 聚合 List 判定、被引用/未被引用数据集的语义差异（M2-13）                                                               |
| `engine_ext.rpx`（2494B）       | 4×3 | 5 行                 | 8×7  | extendMode **17/18/19 各占一行**（v0.5 重排）：17→整列 `ArrayList`、18→同列 5 子格、19→同行 5 子格（并把列结构放大 5×）（M2-15） |
| `engine_expr.rpx`（5363B）      | 6×5 | 3 行                 | 8×5  | 算术与相对/绝对引用（M2-05/M2-06）                                                                          |
| `engine_pager.rpx`（2483B）     | 4×3 | 40 行                | 43×3 | 分页 4 页 × 14 行 + 分组头每页重复 + `-90` 行类型（M2-11）                                                       |
| `engine_merge.rpx`（3142B）     | 4×4 | 3 行                 | 8×4  | **M2-04**：纵向合并跨进扩展行 → 锚点 `row_merge=2`（每个实例占 2 行）；横向合并 `col_merge=2` 复制到每个实例                     |
| `engine_cycle.rpx`（1930B）     | 3×3 | 3 行                 | —    | **M2-07**：`B2=C2`/`C2=B2` → `ReportError`，无网格（见下"错误型夹具"）                                         |
| `engine_samemerge.rpx`（1969B） | 3×3 | 5 行 A,A,B,B,C       | 7×3  | **M2-08**：`mergeSame=98` → **分页后** A/B 各 `row_merge=2`，末尾 C 不合并                                  |

`engine_*.expected.json` 顶层键：  
`unit` / `dataset` / `formulas[]`（含可选的 `extend_mode`、`merge_same`）/ `row_cells[]` /  
`calculated` / `pagination` / `merged`。

- `calculated`：正常为 `{rows, cols, cells[{r,c,value,type[,disp_value],source{src_row,src_col,row_merge,col_merge,extend,sub_cells[,sub_values]}}]}`。  
  ⚠ **`cells` 是稀疏的**：`value` 与 `disp_value` **双 null** 的格**整条不落盘**（`CalcJson.java:51-53`）。

  故「坐标不在 `cells` 里」=「该格为空」，**不是**「断言失败」；逐格比对必须先按 `(r,c)` 建索引再查，**不能按下标对位**。

  7 夹具实测落盘率：`engine_expr` 20/40、`engine_ext` 42/56、`engine_merge` 19/32、`engine_pager` 124/129、`engine_samemerge` 12/21、`engine_sum` 36/72。
- **`source` 是可选键**（`engine_ext` 42 格中 **30** 格无、`engine_pager` 124 格中 **114** 格无），逐格断言不得无条件取。  
  已核实契约（六网格夹具 `fn=fp=0`）：**`source != null` ⟺ `r ≤ template_rows 且 c ≤ template_cols`**（`ExtCellSet.java:623-629`）。⇒ 必须维护「引擎矩阵 + 计算矩阵」两张矩阵。
- **`source.src_row` 是模板态坐标**（v0.9 补）：M2-09/M2-23 断言它时必须与**模板网格**比对；⛔ 不得用它索引计算态网格或反查模板行。详见附件 I.9。
- **`disp_value` 是死键**：仅当 `disp_value != String.valueOf(value)` 时才写（`CalcJson.java:71-73`），7 个夹具**出现 0 次** ⇒ P0 不可断言，实现侧只需保留槽位。
- `pagination`：`{paper_w, paper_h, columns, page_count, x_pages, y_pages, row_num_per_page, pages[{page,rows[,first_value][,last_value],col1_values[]}]}`。  
  ⚠ **`rows` ≠ `col1_values.len()`**：`col1_values` 跳过第 1 列的空值（`CalcJson.java:121-132`）。实测 `engine_pager` 第 4 页 `rows=4` 而 `col1_values` 只有 3 个；`engine_sum` 第 1/2 页 `rows=8` 而各只有 4 个。`first_value`/`last_value` 同样可缺。
- `merged`：**分页后**每页中 `row_merge>1` 或 `col_merge>1` 的格（`{page_count, cells[{page,r,c,value,row_merge,col_merge}]}`）。  
  这是 **`mergeSame` 的唯一可观测位置**——`ExtCellSet.mergeSame()` 只被 `SimplePager.createPages()`（`:133,:179`）与 `CalcSubRpt` 调用，`Engine.calc()` 从不调用。  
  ⛔ **反向不成立**：`merged.cells` 非空 **≠** 发生过 `mergeSame`。`engine_ext` 的 `merged` 有 **35** 格（`col_merge=5`、`row_merge=1`），

  那是 extendMode 19 放大列结构留下的痕迹，与 `mergeSame` 无关；反过来 `engine_expr`/`engine_pager`（各 0 格）与 `engine_sum`（0 格）的 `merged` 也**非 `null`**，

  只是 `cells` 为空数组（`page_count` 分别 1/4/2）。⇒ 判据只能用「`mergeSame` 夹具自身的 `merged.cells`」，**不能用「`merged` 是否非空」当筛选条件**。
- **错误型夹具**：计算必须失败时 `calculated` = `{"error":{class,message,cause_class,cause_message,chain}}`，`pagination` 与 `merged` 为 `null`。  
  `chain` 是引擎打印的引用链（如 `B2-C2-B2-C2`）；`cause_class` 为诊断器 `IIllIIIIIIIIIlll`。

> **M2-09 要求**：`calculated` 段必须**逐格断言**（值 + `type` + `source` 六个子字段，无 `source` 的格跳过该键），  
> `pagination` 段必须**逐页断言**（页数 + 每页 `first_value`/`last_value`/`col1_values`），  
> `merged` 段必须**逐格断言**；错误型夹具必须断言**异常类型 + 引用链**，不得以"没崩"替代。  
> 仅比对矩阵尺寸不算通过。


## 8. 已知坑与待验证项

| 项                                   | 状态                  | 说明 / 消歧方法                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ----------------------------------- | ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `sum(range)` 语义                     | ✅ **已解（反直觉）**       | **不是区域聚合**：`sum(B2:B4)`=100（=操作数1）、`sum(B3:B4)`=200；两操作数形式直接 `ReportError`；**只有操作数求值为 List 才聚合**（`engine_sum`：未被引用的 `ds2` ⇒ 450；被引用的 `ds1` ⇒ 100/200/150）。机制见附件 H.1（`Sum.java:104-106,112-121,174-190`）。⛔ 禁止实现 Excel 式区域求和                                                                                                                                                                                                                                                                                         |
| extendMode=16 扩展行为                  | ✅ **已解（反直觉）**       | **会扩展**，且运行时按 leftHead/topHead 拓扑被**改写为 18**（`ExtExtendCell.prepareCalculate:314-328`）。⚠ 该改写**只对带表达式的格生效**（`engine.expression != null`）——无表达式的格在计算态里原样保留 `16`（6 个夹具交叉验证：`source.extend != 16` 的格集合恰好等于 `formulas[]` 集合）。行数公式 = 模板矩阵行数 + (数据集行数 − 1) × 块高（块高 = 锚点 `row_merge`，无合并为 1）。**v0.5 用重排后的 `engine_ext` 把 17/18/19 证死**：17→单格持有整列 `ArrayList`；18→同列 5 个子格；19→同行 5 个子格并把**列结构放大 5×**（放大出的兄弟列记 `col_merge=5`）。⚠ 旧布局（四态同行）里 17/18/19 全为 `sub_cells=1`，**根本不可区分**。见附件 H.4  |
| **合并区 × 扩展（M2-04）**                 | ✅ **已解（v0.5 新增基准）** | 模板合并区跨进扩展行时**跨度保留在锚点上**：`A2` 合并 `A2:A3` 且 `extend=18` → 每个扩展实例占 2 行，锚点 `row_merge=2`；横向合并 `col_merge=2` 复制到每个实例。见 `engine_merge`、附件 H.7                                                                                                                                                                                                                                                                                                                                       |
| **同值合并（M2-08）**                     | ✅ **已解（v0.5 新增基准）** | ⚠ **`mergeSame` 不在 `Engine.calc()` 里生效**——`ExtCellSet.mergeSame()` 仅被 `SimplePager.createPages()`（`:133,:179`）与 `CalcSubRpt` 调用（全量 grep 确认）。因此必须在**分页后**的页网格上观测：`engine_samemerge` 中 A/B 各得 `row_merge=2`、末尾 C 不合并；码值 **98=纵向 / 99=横向 / 100=双向**，默认 97=关（`NormalCell` 槽 89）。见附件 H.7                                                                                                                                                                                           |
| **循环引用（M2-07）**                     | ✅ **已解（v0.5 新增基准）** | 表达式成环**由引擎自身检出**（不止 leftHead 链）：`B2=C2`/`C2=B2` → `ReportError`，cause 为 `IIllIIIIIIIIIlll`，消息含引用链 `B2-C2-B2-C2`；自引用 `B2=B2` 同样报错。见 `engine_cycle`、附件 H.8                                                                                                                                                                                                                                                                                                                      |
| 分页基准                                | ✅ **已跑通**           | `ReportUtils2.getPagerInfo` → `PageBuilder.createPages()` → `getAllPages()` 可用；`engine_pager` 40 行 → **4 页 × 14 行**（297mm/20mm），分组头每页重复且计入页高；`row_num_per_page=20` 只是 propMap 槽 33 提示值、不参与切页。见附件 H.5                                                                                                                                                                                                                                                                          |
| **`cell_type` 短路（extendMode 判定前置）** | ✅ **已解（v0.5）**      | `ExtNormalCell.checkExtend` **最开头**（`:494-496`）就把 **图形 / 子报表 / 条形码** 三种格排除出扩展格集合：`type ∈ {-61 GRAPH, -57 BARCODE, -60 SUBREPORT}`（常量见 `INormalCell.java:24-28`）。它排在**所有** extend 判断之前，故**优先于**「声明 18/19 恒为扩展格」。落成 `ExtNormalCell` 后走无条件置 17 ⇒ `setCellType(-57)` + `setExtendMode(18)` + 表达式 → 计算态 **17**（不是 18）。⚠ **当前 7 个夹具都没有这三种类型的格 ⇒ 漏掉它没有任何测试会失败**，是可构造的真实分叉（构造方法见 `docs/m2-engine-design.md` §11.3）。实现：`extend/role.rs` 的 `cell_type` 参数与 `EXTEND_EXCLUDED_CELL_TYPES` |
| **扩展块高的可推导性**                       | ✅ **已解**            | 块高 = **计算态** `source` 段里非 16 格的 `row_merge` 最大值（6 夹具无一例外）。⛔ **不能**从 `merged` 段推：`engine_merge` 与 `engine_samemerge` 的 `merged` 都写 `row_merge=2`，但前者计算态锚点为 2、后者为 1（`mergeSame` 只在分页后生效）。见 `docs/m2-engine-design.md` §11.4                                                                                                                                                                                                                                                     |
| **夹具类型面（M2-14）无真实数据覆盖**             | ⚠ **无基准（P0 风险）**    | 6 个网格夹具的类型集**只有** `string`(124) / `int`(124) / `list`(5)；`double` / `float` / `long` / `decimal` / `boolean` **一次都没出现**。⇒ `Integer(100)` vs `Double(100.0)` 这条 M2-14 核心分叉**在真实数据上不可断言**（`CalcJson.typeName:224-253` 的九分支只被走到 3 支）。`value.rs` 的类型保真目前**只有合成单测，无产品夹具背书**。**补齐方法**：新增夹具使公式产出 `qty/3`（Double）与 `count(...)`（Integer/Long）等，重跑 `GenEngineFixtures`；在此之前 M2-14 只能声明「接口就位」，**不得声明「已验证」**                                                                             |
| **集合形状（口径）**                        | ✅ 已解（v0.6 复核）       | 三条独立复核（Python 侧脚本 `tests/fixtures/_shape_audit.py` / `_source_audit.py`，不依赖 Rust 加载器）：① `cells` 稀疏（双 null 不落盘）；② `merged` 段在 7 夹具全部非 `null`，`cells` 非空只出现在 `engine_ext`(35)/`engine_merge`(12)/`engine_samemerge`(4)，⇒ **不能用「`merged` 非空」筛 merge 夹具**；③ `Page.rows ≠ col1_values.len()`（`engine_pager` P4 `rows=4`/3 个，`engine_sum` P1 `rows=8`/4 个）。详见 §7                                                                                                                    |
| **求值调度（异常协程 → Rust）** | ✅ **已解（源码级）** | ⛔ **禁止用 Rust 递归求值**：产品是「抛 `(cell,state)` 异常当 switch」，四条通道 state4（先 19 后 18）→ state5 → state6 special → state8 **级联失效**（依赖栈里所有格一起标 8 入队，`reflush` 收尾）。special 重入在产品里**无上限**会活锁，Rust **必须**加上限报 `EngineError`。`rowMerge` 的 bit30/bit31 是内部标志，`serialize` 时被 `& 0x7FFFFFFF` 抹掉 ⇒ **对拍看不到**。见附件 K.1、M2-20 |
| **`mergeNull` 方向语义（M2-21）** | ✅ **已解（源码级）** | ⛔ 属性码标在**空值格**上，含义是「空格往哪个方向找有值的格」——114=向上找（锚点向下吞）、116 反向（锚点 `addRow(-rm)` **上移自身**）、115/117 同构。锚点谓词**不含** `value != null`。第 2/4 趟**必须先移动锚点再回填**。见附件 K.3 |
| **属性表达式 = 分页前提（M2-22）** | ✅ **已解（源码级，v0.8 勘误）** | ⚠️ **此前把 `calcProperties` 判为 P2 是错的**——模板 `RowCell.expMap` 码 1 就是**行高表达式**（`ExtRowCell.java:66-91`），求值在分页之前 ⇒ 行高错则**分页全错**。✅ M1 已把三类 expMap 解析成 `Option<Vec<(u8, ObjValue)>>` ⇒ **不要改 M1**。见附件 L |
| 分组头动态生成                             | ⚠ **未验证**           | 只验证了「独立 `-90` 行」一种构造；按分组键动态生成（`ds1.group(...)` + `DSGroup`）未测。另：`-90` 打在**会扩展的锚点行**上 → 扩展把 `-90` 复制到每个插入行 → `SimplePager.java:261` **NPE**                                                                                                                                                                                                                                                                                                                                    |
| 函数注册时机                              | ⚠ 待查                | Phase B 报「ServletContext 为空」；P1 函数面目前只被 `ds1.sum` 与 `ds2.count()` 覆盖（`avg/max/min` 尚无基准），更大函数面仍需确认注册机制                                                                                                                                                                                                                                                                                                                                                                        |
| **空数据集（count=0）**                   | ✅ **已解（源码级）**       | ⚠ `extendRow`/`extendCol` 的 `count=0` 会**崩**：`ExtCell[count]` 后紧接 `extCells[0] = cell`（`ExtCellSet.java:994-995` 与 `:1094-1095`）⇒ 零长数组赋值抛 `ArrayIndexOutOfBoundsException`。**但唯一调用点上游已拦住**：`ExtExtendCell.calculate` 求出 List 后先判 `if (len == 0) { setValue(null); return; }`（`ExtExtendCell.java:379-384`）才轮到 `extendRow`/`extendCol`（`:425-430`）。⇒ **空数据集不发生扩展**，行数 = 模板行数；锚点格被置为**空值**、无子格。`calc_row_count` 的 `max(dataset_rows,1)` 得到同样结果，但从源码看它是「不适用」而非「防御」               |
| **`count` 的确切语义**                   | ✅ **已解（源码级）**       | `count = ((List)expression.calculate(ctx)).size()`（`ExtExtendCell.java:379-380` 取值、`:425-430` 分派）——**是「表达式求值出的 List 长度」，不是「数据集行数」**。`ds1.select(region)` 二者恰好相等，但带条件/多列的 `select` 会不等。⛔ 行数公式里的「数据集行数」应严格读作「锚点表达式返回的 List 长度」                                                                                                                                                                                                                                                  |
| `$` 绝对引用                            | ✅ **已解**            | ReportLite 的 `$` 是**整格前缀**（`$B2` 可用），**不支持 Excel 的 `$B$2`** —— `Expression.java:193-203` 只剥一个开头 `$`；失败真因是 `CellLocation.parse("B$2")` 返回 null。**别按 Excel 习惯实现**                                                                                                                                                                                                                                                                                                               |
| `PropQueue.index1`                  | ✅ **已解**            | `PropQueue` 在 reportlite **无任何使用者 = 死代码**；`index1` 是从 `commonl.Queue` 复制来的未使用字段（后者有 `poll()` 才推进）。**不属于 M2 结构**                                                                                                                                                                                                                                                                                                                                                               |
| M1 遗留② NormalCell 默认槽               | 延后                  | 属 M1 解析层，P2 随全槽位覆盖一起补                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `cli_crosscheck.py` 不适用引擎夹具         | ✅ 已确认               | 该脚本读的是**结构态**键（`rows/cols/cells/row_heights/...`），对 `engine_*.expected.json` 直接 `KeyError: 'rows'`。它只能跑 M1 的 `simple/merged/full/realmerge`                                                                                                                                                                                                                                                                                                                                   |
| M1 遗留③ CLI 无自动化断言                   | 部分闭合                | 后端留下 `tests/fixtures/cli_crosscheck.py`（83 行，Python 侧逐字段断言）；Rust 侧断言仍缺                                                                                                                                                                                                                                                                                                                                                                                                        |
| M1 遗留④ readObject 多标签               | 延后                  | 属 M1 解析层                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| M1 遗留⑤ readBoolean EOF              | 延后                  | 属 M1 解析层                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |

## 9. 端到端验证步骤

```bash
export LIB='C://Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.43.34808\lib\x64;C://Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64;C://Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\ucrt\x64'

# 1. 重跑 Java 侧基准探针（产出 ground truth）
#    全部 ≤300 行：CalcProbe / ProbeDisambig / ProbeExtend / ProbeP0 /
#    GenEngineFixtures / EngineModels / EngineModelsP0 / CalcJson / CalcMergeJson
cd F:/project/report/rpx/tests/fixtures
JAVAC="D:/Tools/jdk-21.0.2/bin/javac --release 8 -cp F:/project/report/reportlite/lib/* -d out"
JAVA="F:/project/report/common/jre/bin/java.exe -cp F:/project/report/reportlite/lib/*;out"
$JAVAC *.java
$JAVA rpx.fixtures.GenFixtures . --verify        # M1 三夹具（simple/merged/full）
$JAVA rpx.fixtures.GenRealMerge . --verify       # 真合并夹具 realmerge
$JAVA rpx.fixtures.GenEngineFixtures . --verify  # 七个引擎夹具 + expected.json（计算态+分页态+合并）
$JAVA rpx.fixtures.CalcProbe .                   # headless 计算链路（附件 F.4）
$JAVA rpx.fixtures.ProbeDisambig .               # 消歧：sum 语义（附件 H.1/H.2）
$JAVA rpx.fixtures.ProbeExtend .                 # 消歧：extendMode 改写 + 分页 + 分组头（附件 H.4/H.5）
$JAVA rpx.fixtures.ProbeP0 .                     # P0 基准实验：合并/循环/mergeSame/17-18-19（附件 H.7/H.8）
$JAVA rpx.fixtures.VerifyDollar .                # $ = isSaved 单次运行并排实测（附件 G.1）

# 2. 结构态交叉校验（**仅 M1 四个结构夹具**；引擎夹具是计算态，cli_crosscheck 会 KeyError）
cd F:/project/report/rpx
for f in simple realmerge; do
  python tests/fixtures/cli_crosscheck.py $f || echo "WARN $f"
done
#    注：merged/full 是「浅合并」，非锚点槽持自己的默认格，cli_crosscheck 的 grid fill
#    检查会报 missing —— 属附件 E.8 的已知语义差异，不是缺陷；Rust 侧 golden 双向校验正确处理。

# 3. 构建与测试
cargo test --workspace          # M1 23 测试 + M2 新增
cargo clippy --all-targets -- -D warnings
```


## 10. 变更记录

| 日期         | 变更内容                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | 原因                                                                                                                      |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| 2026-10-12 | **v0.7/v0.8/v0.9（架构师：补 4 处契约空洞 + 1 处自我勘误；本轮为丢失后重建）**：**① 附件 K**（159 行）求值调度四通道 + `mergeSame`/`mergeNull` 四趟源码级规则 + `reflush`/页壳/**计算态序列化=对拍桥**；**② 附件 L** 属性表达式与行高来源（**撤销**此前「`calcProperties` 属 P2」的判断——它是 M2-11 分页前提）；**③ §5.1** 表达式切片契约四层（`is_extend` 节点级规则表 / 语法真值 / 9 步分类次序 / 函数面 139 个与 priority 例外）；**④ 附件 I.9** `src_row` 是模板态坐标（由 backend-2 与 team-lead 的 RUNLOG §19.5 实测**升级为源码级规则**）。新增 **AC M2-16..M2-23**（共 23 条）。⚠ v0.7-v0.9 曾被并发覆盖丢失（`spec-m2.md` 回滚到 v0.6，无 git 无备份），本版按契约记录重建 | 任务书第 4、5 问（求值调度、合并）此前只有实测口径没有源码级规则；`is_extend`、`src_row`、`calcProperties` 三项都属「按直觉写必挂且不报错」，必须落在契约层而非实验记录 |
| 2026-10-12 | v0.1 创建（总监接管）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | 原架构师在产出任何内容前因 API 429 失败；盘面核实 `docs/` 零产出。已核实项附证据，未核实项一律标「待验证」，不冒充                                                      |
| 2026-10-12 | v0.2（总监源码级消歧 x2）：**1. `$` 引用语义查清**——`Expression.java:193-203` 只剥一个开头 `$`，剩余串交 `CellLocation.parse`，故产品支持 `$B2` 而**不支持 Excel 的 `$B$2`**；探针 Phase F 的报错由此解释。M2-06 加硬约束「禁止实现成 Excel 双 `$`」。**2. `PropQueue` 判定为死代码**——全 `re_work/src` grep 无任何使用者，`index1` 是继承自 `commonl.Queue` 的未使用字段（后者才有 `poll()` 推进）；从 M2 结构中移除，附件 F.2 改写为勘误。新增**附件 G**：G.1 `$` 引用语义 / G.2 `ExtCellSet.getSource` 契约 / G.3 左邻主格追溯与循环引用检测点                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | 两项「不可区分/待查」被源码级证据关闭；其中 `$` 语义若按 Excel 习惯实现会与产品**行为不一致**，属会埋雷的项                                                          |
| 2026-10-12 | v0.3（消歧实验完成，两条结论**反直觉**）：**1. `sum` 不是区域聚合**——`sum(B2:B4)`=100（=操作数 1）、`sum(B3:B4)`=200、两操作数形式直接 `ReportError`；**只有操作数求值为 List 才聚合**（原文写的 `ds1.sum(qty)`=450 是**早期单数据集实验的口径**，2026-10-13 已按 `engine_sum` 夹具更正：**未被**扩展格引用的数据集才是 450，被引用的 `ds1` 逐行 100/200/150）；且聚合结果取决于**数据集是否被扩展格引用**（被引用则逐行 100/200/150，未被引用则全量 450）。**2. extendMode=16 不是「不扩展」，而是运行时按 leftHead/topHead 拓扑改写为 18**（`ExtExtendCell.prepareCalculate:314-328`），行数 = 模板行数 + (数据集行数 − 1)。**分页基准首次跑通**（4 页 x 14 行，`row_num_per_page=20` 只是 propMap 槽 33 提示值）。新增 4 个引擎基准夹具（`engine_sum`/`engine_ext`/`engine_expr`/`engine_pager`）与**附件 H**（H.1 求值语义 / H.2 聚合上下文 / H.3 类型即契约 / H.4 extendMode 改写 / H.5 分页基准 / H.6 行类型常量）；新增 M2-13/14/15 三条 P0 验收                                                                                                                                                                                                                                                                                                                                                                           | 后端消歧实验回传（`RUNLOG.md` §10-§11）；两条结论若按直觉实现必挂                                                                              |
| 2026-10-12 | v0.5（补齐三个 P0 空缺基准 + 证死 17/18/19）：**1. M2-04/M2-07/M2-08 三个「无计算态基准」的 P0 项各自补上产品产出的夹具**——`engine_merge`（模板合并区跨进扩展行：`A2:A3` 且 `extend=18` → 每实例占 2 行、锚点 `row_merge=2`，横向合并 `col_merge=2` 复制到每个实例；行数公式升级为 `模板行数 + (数据集行数−1) × 块高`）、`engine_samemerge`（`mergeSame=98` 在**分页后**把 A/A 与 B/B 各合成 `row_merge=2`，末尾 C 不合并）、`engine_cycle`（`B2=C2`/`C2=B2` → `ReportError`，cause `IIllIIIIIIIIIlll`，链 `B2-C2-B2-C2`）。**2. `engine_ext` 重排为「一行一个模式」**（模板 4×3、数据 5 行）——旧布局四态同行时竖向扩展吞掉整条行带，17/18/19 全为 `sub_cells=1`、**根本不可区分**；重排后 17→单格持整列 `ArrayList`、18→同列 5 子格、19→同行 5 子格并**把列结构放大 5×**（计算态 8×7 而非 8×3）。**3. 关键发现：`mergeSame` 不在计算链路里**——`ExtCellSet.mergeSame()` 全量 grep 只有 `SimplePager.createPages():133,:179` 与 `CalcSubRpt:138` 三个调用方，`Engine.calc()` 从不调用，故 M2-08 必须在分页后观测。新增 3 个夹具（共 7 个）与 **附件 H.7/H.8**、**H.4 补遗**；复核修正 H.5 的 P4 行数（13→4）。夹具可复现性已独立证明：`*.java` 重编译后重跑生成器，8 `.rpx` + 8 `.expected.json` 与仓库副本**逐字节相同**（29/29 文件）                                                  | 审计结论（`docs/m2-verification.html`）指出 M2-04/07/08 三项 P0 判据**无计算态基准**、`engine_ext` 无法区分 17/18/19；三处均为「按直觉实现必挂」的项，必须在实现前补基准 |
| 2026-10-12 | v0.6（**集合形状口径复核**，写契约前的最后一道核对）：用 Python 侧独立脚本（`tests/fixtures/_shape_audit.py` + `_source_audit.py`，**不经过 Rust 加载器**，避免同一 bug 两侧同错）核完 7 个夹具的收集段，修正/补入四条口径：**① `cells` 是稀疏数组**——`value` 与 `disp_value` 双 null 的格整条不落盘（`CalcJson.java:51-53`），落盘率 `expr 20/40`、`ext 42/56`、`merge 19/32`、`pager 124/129`、`samemerge 12/21`、`sum 36/72`；「坐标不在 `cells` 里」=「该格为空」，逐格断言必须先按 `(r,c)` 建索引，**不能按下标对位**。② `source` 契约确认无误并把过期计数改正（`engine_ext` 42 格里 30 格无、`engine_pager` 124 格里 114 格无；旧文写「24 格中 12 格」是夹具重排前的数），六网格夹具 `fn=fp=0` 成立。③ **`merged` 的反向不成立**——7 个夹具 `merged` 全部非 `null`（错误型为 `null`），`cells` 非空只出现在 `engine_ext`(35,`col_merge=5`，是 extendMode 19 放大列结构的痕迹) / `engine_merge`(12) / `engine_samemerge`(4)，⇒ **禁止用「`merged` 非空」筛 merge 夹具**。④ `Page.rows ≠ col1_values.len()`（`col1_values` 跳过第 1 列空值）；`disp_value` 在 7 夹具**出现 0 次 = 死键**，P0 不可断言。另记入一条**未闭合的 P0 风险**：夹具类型面只有 `string`/`int`/`list` 三种，`double`/`float`/`long`/`decimal`/`boolean` 零覆盖 ⇒ **M2-14 目前无法在真实数据上验证** | 实现扩展算法前必须先有「能逐格断言 `calculated`」的基座（M2-09）；基座若照着**错误**的集合形状描述去建，「坐标缺失」会被当成断言失败，或反过来把稀疏当稠密静默放行                            |
| 2026-10-12 | v0.6 续：`docs/m2-engine-design.md` §11.5 记录同一批口径修正；`crates/rpx-engine/tests/common/`（夹具加载器）已按此形状建模（`Grid::cell()` 空格返回 `None`、`Page.col1_values` 注明可少于 `rows`）                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | 口径修正要落在**契约层**（spec + 设计文档）而不只落在代码注释里，否则下一个实现者仍会重踩                                                                      |
