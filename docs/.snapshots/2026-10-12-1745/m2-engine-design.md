# m2-engine-design — rpx-engine P0 核心设计契约

> 作者：首席架构师（高见远）· 2026-10-12
> 上游：`docs/spec-m2.md` v0.3（§4.1 模块布局、§5 引擎契约摘要）、`docs/format-contract.md` 附件 F/G/H
> 证据基线：`F:/project/report/re_work/src/reportlite/**`（CFR 0.152）+ `tests/fixtures/engine_*.expected.json`
> 状态：**设计稿**。本文件只做 P0 核心；分页（P1）只在"合并必须发生在分页之后"这一点上被提及。
>
> **本文的纪律**：每条非平凡结论都给 `文件:行号` 或夹具路径。**没有**读透的东西一律标 `⚠ 待验证` 并附
> 具体验证方法。上一版规格被驳回，正是因为把两条"想当然"当成了契约（`sum` 区域求和、`extend=16` 不扩展）
> —— 两条都实测为假。**不要凭直觉补洞。**

---

## 0. 阅读顺序与符号表

先读 §1（数据模型）与 §2（三个阶段），再按需读 §3~§7。§8 是模块拆分，§9 是"已核实 vs 待验证"总表，
**§10 是待验证项导航（该读哪一节 / 证据在哪）**，§11 是定案与修复记录，
**§12 是扩展算法的实现执行顺序清单（动工前读这一节）**，
**§13 是表达式切片的契约（词法 / 语法 / `is_extend`）—— §12.1 第 3 步的正文就在那里**。
`extendRow`/`extendCol` 的**逐行算法正文**在 `docs/format-contract.md` **附件 I**，
分组头（`-90`）与动态分组在**附件 J**；本文件 §6 保留**设计视角**（判定式与易错点），两处冲突以附件为准。

**符号**

| 符号 | 含义 | Java 对应 |
|------|------|-----------|
| `_$26` | **计算矩阵**：装 `ExtCell`（算出来的格），会被扩展改写 | `ExtCellSet._$26`（字段声明 `ExtCellSet.java:95`） |
| `_$25` | **引擎矩阵**：装 `NormalCellEngine`/`RowCellEngine`/`ColCellEngine`（每模板格的"气缸"），**扩展不改它** | `ExtCellSet.java:96` |
| `cell00` / `ROOT` | 矩阵原点格（0,0），一个哨兵 `ExtExtendCell` | `_$10`，`ExtCellSet.java:112,148` |
| `_$19` | **主格判定器**（`MainCell`） | `lIlIlllIIllllIII`，`ExtCellSet.java:103` |
| `_$21`/`_$20` | `current` / `savedCurrent`（`ExtCell`） | `ExtCellSet.java:100-101,586-596` |
| `_$16`/`_$15` | 计算矩阵当前行数/列数（会被扩展更新） | `ExtCellSet.java:106-107` |
| `leftHead`/`topHead` | 一格所属的"左主格/上主格"实例（`ExtCell` 引用） | `ExtNormalCell.java:66-67` |
| `extensible` | 运行时**生效**的扩展方式（16/17/18/19） | `NormalCellEngine.extensible`，`NormalCellEngine.java:32` |

> `MainCell.java` 与 `lIlIlllIIllllIII.java` 是**同体两份**（逐行相同，`:21-39`）。产品实际用的是后者
> （`_$19` 的静态类型是 `lIlIlllIIllllIII`，`ExtCellSet.java:103`）。`crates/rpx-engine/src/main_cell.rs`
> 已按此移植，无需改。

### 0.1 一个必须最先接受的坑：`expected.json` 的 `source` 不是"这个格的出处"

`CalcJson.cellJson` 用 `ecs.getSource(r,c)` 取 `source`（`CalcJson.java:74-75`），而
`ExtCellSet.getSource(row,col)` 读的是**引擎矩阵**：`((NormalCellEngine)this._$25.get(row,col)).srcCurrent`
（`ExtCellSet.java:623-629`）。`_$25` **在扩展过程中从不被插入/改写**（只有 `_$26` 被 `insertRows/insertCols`，
`ExtCellSet.java:810,881,903`）。

**可实测的推论（已在夹具上逐格验证）**：`source` 非空 ⟺ `1 <= r <= template_rows && 1 <= c <= template_cols`
（外加 `(0,0)` 特例，`ExtCellSet.java:616`）。例：

| 夹具 | 计算态 8×7（`engine_ext`，模板 4×3） | 有 `source` 的坐标 |
|------|--------------------------------------|--------------------|
| `engine_ext` | 行 1..8、列 1..7 | 仅 `r∈1..4 且 c∈1..3`；`(1,4)(1,5)(2,4)(2,5)(3,4)(3,5)(4,4)(4,5)` 与全部 `r>=5` 都**无** `source`。**计数以落盘格为准：42 格中 30 格无 `source`**（旧文写的「24 格中 12 格无」是夹具重排**前**的数字，2026-10-12 改正，见 §11.5 ②） |

**因此**：`source.src_row/src_col/row_merge/col_merge/extend/sub_cells` 描述的是**模板格在 (r,c) 的那个"气缸"**，
**往往不是**同一坐标上那个算出来的值所属的实例。例：`engine_ext` 计算态 `(4,1)` 的值来自"第 4 行克隆"，
但 `source` 给的是模板 `(4,1)` 的 19 锚点（它的 `src_row=4, src_col=1, extend=19, sub_cells=5, col_merge=1`）。

> ⛔ 实现必须建**两张矩阵**（引擎矩阵 + 计算矩阵），不能让 `getSource` 去查计算矩阵。这是
> `engine_pager` 124 格里 114 格没有 `source` 的原因，也是"逐格断言不得无条件取 source"的根因
> （`spec-m2.md:143-144`）。⚠ 若有人想把两张矩阵合并成一张省事 —— 那会让除锚点外的 `source` 全部错位。

---

## 1. 数据模型（计算态）

```
CalcState
├─ engine: Vec<EngineCell>          // 引擎矩阵（_$25）：模板气缸，扩展期不改
├─ calc:   Vec<CalcCell>            // 计算矩阵（_$26）：算出来的格，会被 insert/改写
├─ grid / cells                     // 便于按 (r,c) 取；见 arena.rs
├─ main:   MainCell                 // _$19
├─ current: CellId                  // _$21
├─ saved:   CellId                  // _$20
└─ state:   u8                      // _$14：调度阶段（见 §5）
```

**一致性与"同一性"约定**（Rust 用 `CellId` 整数替代 Java 对象引用）

| Java 语义 | Rust 表达 | 证据 |
|-----------|-----------|------|
| 一个格对象的"身份"就是它的地址 | `CellId(u32)` 索引；同一格 = 同 id | `arena.rs:15` |
| 合并区"铺满"：一个对象塞进整个矩形 | 计算矩阵 `cells` 里同一矩形**指向同一个 `CellId`** | `ExtCellSet.java:177-183`、`_$1(startRow,startCol,endRow,endCol,head):706-712` |
| `clone()` 共享 `engine`（浅拷贝） | 派生格与源格**共享 `engine` 句柄**，只有 `cellValue` 是各自的 | `ExtNormalCell.clone:132-148`（`super.clone()`）、`ExtExtendCell.clone:48-54` |
| 引用同一性比较（`left == cell00`） | `CellId` 相等；`null` = `CellId::NONE`，**`NONE != ROOT`** | `MainCell.java:25`、`arena.rs:19-21` |

> ⚠ 末行是**已知会踩的雷**：`ExtExtendCell.prepareCalculate:321` 比较的是 `leftHead == cell00`，
> `leftHead` 为 `null` 时结果是 **false**（落进"非 cell00"一侧，→ 17）。已用单测钉死：
> `crates/rpx-engine/src/arena.rs:135` `cell_id_sentinels_are_distinct_and_copyable`。`NONE == ROOT` 必须为假。

---

## 2. 三个阶段（总览）

`Engine.calc()` 的链路（`Engine.java:75-122`）：

```
calcDataSet(rd, ctx)              // 先把数据集建好放进 ctx —— 缺数据集是致命错误
  → new ExtCellSet(rd)            // 阶段 A：模板态 → 引擎矩阵 + 初始计算矩阵
  → ecs.calculate(ctx)            // 阶段 B + C（见下）
  → ecs.reflush()                 // 阶段 D：收尾（对 P0 无实质影响）
```

| 阶段 | 入口 | 干什么 | 证据 |
|------|------|--------|------|
| **A. 建格** | 构造器 | 从 `ReportDefine` 建 `_$26`/`_$25`；合并区锚点铺满矩形 | `ExtCellSet.java:139-193` |
| **B. 准备** | `prepareCalculate(ctx,true)` | 决定谁是扩展格、**算 `leftHead`/`topHead`**、把 16 改写成 17/18/19 | `ExtCellSet.java:721-807` |
| **C. 求值** | `_$2(ctx)` → 三趟 pass + `calcProperties` | 显式状态机：19→18→17→special | `ExtCellSet.java:1483-1543`、`:1297-1446` |
| **D. 收尾** | `reflush()` | 页面级重算入口（P1） | `ExtCellSet.java:2258` |

**P0 必须复刻的是 A/B/C 的输入输出语义。** D 只在分页路径上需要。

---

## 3. 阶段 A：模板 → 计算矩阵（已核实）

| 步骤 | 规则 | 证据 |
|------|------|------|
| 尺寸 | `irow = rd.getRowCount()+1`, `icol = rd.getColCount()+1`（含 0 号头行/头列） | `ExtCellSet.java:144-147` |
| `(0,0)` | 一个 `ExtExtendCell`，既进 `_$26` 也进 `_$25` | `:148-150` |
| 行头 `(i,0)` | `ExtRowCell`；`rowType==-90` 时置"有分组头"位（`_$12 \|= 2`） | `:151-159` |
| 列头 `(0,j)` | `ExtColCell` | `:160-165` |
| 合并锚点 | 若 `cell.isMerged()` 且是锚点 → 建 `ExtNormalCell(rowMerge=endRow-row+1, colMerge=endCol-col+1)`，**铺满整个矩形**（`_$26` 和 `_$25` 都铺） | `:170-187` |
| 合并非锚点（浅合并） | 该槽的 `rd.getCell` 若是**普通默认格**（`isMerged()==false`）→ 走下一轮，**用自己的默认格覆盖前面铺的锚点** | `:188-191` |

> **浅合并 vs 真合并**：`ReportDefine.fillRecord` 把非锚点写空记录（`-1`）时，读回模型里非锚点槽持有锚点对象
> → 装配后全矩形持同一 `CellId`；若非锚点写的是完整默认记录（本批 `engine_merge` 就是），装配后非锚点持自己的默认格。
> 两条路径都必须是"锚点先铺满、后续格按自身内容覆盖"。证据：附件 E.8、`ExtCellSet.java:166-192`。
> 实测：`engine_merge` 的 `A2` 合并 `A2:A3` 时，计算态 `(3,1)` 的 `source` 是**它自己**（`src=(3,1) rm=1 ext=16`），
> 而不是锚点 —— 这正是"后续覆盖"在起作用。

> **行头 `rowType` 与分组头**：`-90`（`IRowCell.TYPE_GROUP_HEADER`，`IRowCell.java:21`）是**纯模板期静态属性**，
> 由 `ExtCellSet` 构造器一次性 latch（`:151-159` → `_$12 |= 2` → `hasGroupHeader()`）。
> ⛔ **引擎从不合成 `-90`**；`-90` 的复制只来自 `extendRow` 的行带克隆，与 `group()` 无关 ——
> `group()` 的唯一真实作用是 `DSGroup.isExtended() == true`（`DSGroup.java:23-25`）⇒ 成为扩展源 ⇒ **只决定行数**。
> 完整论证（含判决性探针与 NPE 触发条件）在 `docs/format-contract.md` **附件 J**，本文件不重复。

---

## 4. 阶段 B：prepareCalculate —— head 图与 16 改写（**最难，顺序即契约**）

`ExtCellSet.prepareCalculate`（`:721-807`）分**四步**，顺序不可换：

```
第 1 步  行优先遍历，决定"谁是 ExtExtendCell（扩展格）"            :745-758
第 2 步  再次行优先遍历：算左 head、算上 head、执行 16→17/18/19 改写 :771-782
第 3 步  把 head 链"归一"到最近的 ExtExtendCell                     :783-806
```

### 4.1 第 1 步：谁会成为 ExtExtendCell

对每个 `(i,j)`，只有当 `cell.getRow()==i && cell.getCol()==j`（它是该槽的**拥有者**，不是被合并铺满的重复引用）时：

```java
this._$21 = cell;
if (!cell.checkExtend(ctx)) continue;      // 不通过 → 保持 ExtNormalCell
cell = this._$1(cell);                     // 通过 → 换成 ExtExtendCell（_$26.set(i,j,cell)）
```
证据：`ExtCellSet.java:749-755`；包装函数 `_$1(ExtNormalCell)`:1167-1186。

`checkExtend`（`ExtNormalCell.java:492-514`）：

| 声明 `extensible` | 有表达式时返回 | 无表达式时返回 |
|-------------------|----------------|----------------|
| 16 | `expression.isExtend()`（表达式**可扩展**才 true） | `false`（落到 `:513`） |
| 17 | `false` | `false` |
| 18 / 19 | `true` | `true`（`:513`） |
| 任何值 | `cellType ∈ {-61,-57,-60}` 一律 `false`（`:495-497`） | 同 |

`isExtend()` 的**完整**定义链（每一行的函数体都逐字读过，**不是**"是不是 DS 函数"的近似）：

```
Expression.isExtend()  →  home.isExtended()                          Expression.java:561-566
Node.isExtended()          = false          （基类）                  Node.java:82-84
Operator.isExtended()      = left||right    （二元/一元运算符）         Operator.java:51-56
SpecialFunction.isExtended() = left || super （一元特殊算子）           SpecialFunction.java:27-32
CSVariable.isExtended()    = false          （B2 / $B2 这类引用）      CSVariable.java:379-381
Constant.isExtended()      = value instanceof List（字面量是 List 才真） Constant.java:22-24
ValueList.isExtended()     = param == null || param.getType() != ':'  ValueList.java:30-32
Link.isExtended()          = false                                    operator/Link.java:100-102
DSSelect/DSGroup/DSFilterCross/DSPlot.isExtended() = true              function/dsfunction/*.java:23/23/38/21
Sort/SubList/To/SplitString.isExtended()           = true              function/{Sort:19,SubList:18,To:17}.java、function/string/SplitString.java:18
```

**这一条的性质（决定它归谁实现）**：`isExtend()` 是**解析树上的一次纯递归谓词**，不做任何求值 ——
`true ⟺ 树里至少有一个"扩展源节点"`（DS 选择类函数 / 排序切分类函数 / 值是 `List` 的字面量）。
它对**表达式切片**有强依赖：必须先有 AST（`Node`/`Operator`/`CSVariable`/`Constant`/函数节点）才能算，
且 `checkExtend:498-504` 在调用它**之前**会先 `optimize(ctx)`（常量折叠）——
所以"折叠后才出现的 `List` 常量"也会让它变真。结论：**`isExtend()` 必须随表达式切片一起交付，
不能塞进 `head`/`extend` 模块**。`crates/rpx-engine/src/extend/role.rs:70-72` 现在把
`expr_is_extend` 作为**入参**要求调用方传入，正是这个依赖边界的显式化 —— 实现表达式切片时补上它即可，
**不要**用"是不是 DS 函数"（`isDSFunction()`）来近似 —— 两条链的**传播规则相反**，已核实：

- `Function.isDSFunction()`（`Function.java:55-57`）**向参数透传**（`param.isDSFunction()`）；
- 而 `Function` / `Sum` / `DSSum` **都没重写** `isExtended()`（`Sum extends Function extends Node`，
  `Function.java:17-18`），于是 `sum(ds1.select(qty))` 的 `isDSFunction()=true` 但
  `isExtended()=Node` 的 **`false`**。
- ⇒ 用 `isDSFunction()` 近似会把 `sum(...)` 误判为"可扩展"（该格随即被错包成扩展格并被拓扑改写成 18）；
  反过来，`isExtend()` 的 `true` 也不都来自 DS 函数（`Sort`/`To`/`SubList`/`SplitString` 与
  `Constant(List)` 都是）。**两个谓词各管一段，都要实现。**
- ⛔ **还有一条更硬的约束（本轮新增，见 V27）**：`checkExtend` 构造表达式时 `ds` **写死 `null`**
  （`ExtNormalCell.java:498-504`）⇒ 解析期**不会**把裸 `select(qty)` 认成 `DSSelect`
  ⇒ 上表里的"DS 函数"只在 **`ds.名(...)` 形态**下才成立。完整的名字解析顺序、14 个重写点的
  逐节点真值表、以及 4 处本仓实现偏差，见 **§13**。

> ⛔ **这解释了一条反直觉的实测**：`engine_sum` 行 2 的 9 个公式格里，只有 `ds1.select(region)`/`ds1.select(qty)`
> 是"可扩展"的（→ 被包装 → 改写为 18）；`B2*2`、`sum(B2:B4)`、`ds2.sum(qty)`、`ds2.count()` 都**不**可扩展
> （→ 不被包装）。但它们在计算态里 **`extend=17` 而不是 16** —— 见 §4.3。

### 4.2 第 2 步：head 的赋值（**U1 已解**；顺序即契约）

对每个拥有者格，**先算 head，再改写**：

```java
this._$21 = cell;
cell.setCellLeftHead(this._$3(cell));   // 左 head
cell.setCellTopHead (this._$2(cell));   // 上 head
cell.prepareCalculate(ctx, macros);     // 16→17/18/19 改写（就在这一句里读上面两个 head）
```
证据：`ExtCellSet.java:771-782`。扫描是**行优先 `i` 升序、`j` 升序**（`:771-772`），
所以算 `cell` 的 head 时，"它左边/上边"的格**已经算完 head、也已完成 16 改写** —— 这条顺序是后面所有结论的前提。

**`isLeftHeadOf` / `isTopHeadOf` 的方向（先钉死，两个函数都靠它）**

```java
// ExtNormalCell.java:390-406
boolean isLeftHeadOf(ExtCell sub) {                  // this == cell 时：
    for (ExtCell h = sub; h != null; h = h.getCellLeftHead()) if (this == h) return true;
    return false;
}   // ⇒ cell.isLeftHeadOf(sub)  ⟺  "从 sub 沿 leftHead 上溯能走到 cell"（cell 是 sub 的祖先）
```
即**参数是子孙、receiver 是祖先**。`cell.isLeftHeadOf(head12) == true` ⟹ `cell` 在 `head12` 的链上
⟹ 不能再把 `head12` 挂到 `cell` 下（会成环）—— 这正是两处 `continue`/`throw` 的判据。

**`_$3(cell)`：左 head（`ExtCellSet.java:631-666`）—— 可照写的伪码**

```
fn find_left_head(cell) -> CellId:
    // —— 分支 A：模板声明了 leftHead 字符串（属性码 30）——
    if let Some(h1) = cell.declared_left_head()           // 非空、trim 后非空、getSource 命中
    //  ★ declared_left_head() 读的是模板格 `engine.srcPrepare.getLeftHead()`（ExtNormalCell.java:1420-1421），
    //    不是 `left_head` 引用字段（那个在阶段 B 开始时还是 null）。两个存储必须分开建模。
    {
        if !cell.is_left_head_of(h1) { return h1 }        // 有向无环 → 直接用
        // 否则声明本身成环：把 h1 的 leftHead 链走到 cell 为止，压栈后抛循环引用
        throw CircularRef { stack: [cell] + walk(h1 .. cell), thrower: cell }   // :641-646
    }
    // —— 分支 B：同行向左扫（列号 cell.col-1 → 1，**不含 0 号头列**）——
    for col in (1 .. cell.col).rev()                      // :649  递减！
    {
        let h = calc_matrix.get(cell.row, col)            // 注意：读 _$26（计算矩阵）
        if h.is_none() { continue }
        let ext = h.extensible
        if h.is_merged() && ext != 18 && h.row != cell.row { continue }        // :653
        if h.contain_row(cell) && ext == 18 {             // :654  ★ 只有 18 走这条
            if cell.is_left_head_of(h) { continue }       //      ★ 用 containRow(ExtCell)
            return h                                      //      ★ 直接返回 h 本身
        }
        let mut hoh = h.left_head                         // :658  “head of head”
        if let Some(x) = hoh {
            // _$21 = 正在处理的这个格（就是 cell）；ROOT = _$10
            if x != cell && x != ROOT && x != cur_cell && x.left_head.is_none() {
                set_left_head(x, find_left_head(x))       // :660  ★ 递归，先自愈
            }
        }
        if hoh.is_none() || cell.is_left_head_of(hoh) { continue }
        return hoh                                        // :663  ★ 返回的是 h.left_head，不是 h
    }
    return ROOT                                           // :665

fn find_top_head(cell) -> CellId:                         // ★ 镜像，4 处不对称，逐条见下
    ... 同行代码，把 left↔top、col↔row、18↔19、contain_row↔contain_col 互换 ...
```
**证据**：`_$3` `:631-666`、`_$2` `:668-704`、`sameRowTypeAs` `ExtNormalCell.java:255-270`、
`containRow(ExtCell)` `:278-280`、`containCol(ExtCell)` `:283-285`、`isMerged` 见 `ExtCellSet.java:653` 用法。

**`_$3` vs `_$2` 的四处不对称（U1 的"跳过/命中"细节，逐条已核实）**

| 维度 | `_$3`（左） | `_$2`（上） | 依据 |
|------|------------|------------|------|
| 扫描方向/范围 | `col` 从 `cell.col-1` **递减**到 `1` | `row` 从 `cell.row-1` **递减**到 `1` | `:649` / `:686` |
| 合并跳过条件判的 extend | `18 != ext` 才跳 | `19 != ext` 才跳 | `:653` / `:690` |
| **是否调 `sameRowTypeAs`** | **从不调用** | **调两次**（`:691` 命中分支 / `:695` 硬门槛） | grep 全文件 |
| 命中"本格包住它"分支时的判据 | `head12.containRow(cell)` | `cell.sameRowTypeAs(head12) && head12.containCol(cell)` | `:654` / `:691` |

> ⛔ **`_$3` 真的不调 `sameRowTypeAs`** —— 它用 `containRow`（行包含，`ExtNormalCell.java:278-280`，与行类型无关）。
> `sameRowTypeAs` 只在 `_$2` 里出现。`containCol(ExtCell)`（`:283-285`）**内部会先调 `sameRowTypeAs`**，
> 所以 `_$2` 的命中分支同时受"行类型同族"和"列包含"两道约束；`_$3` 的对应分支只受"行包含"约束。

**净效果（这是 U2 的解释，探针实测）**：分支 B 在**非**命中分支时返回 `h.left_head`/`h.top_head`（head 的 head），
不是 `h`。由于行优先自上而下扫描，被扫到的格**已经算完自己的 head**，于是这条链把**同一 rowType 族、
同一列**的所有格"压平"成**同一个** head 值；扫到最上（`row`/`col` 到 1 仍无命中）时拿到的是 `cell00`。
实测（探针 `ProbeHeads`，见 §12.2，`engine_ext` 全部 12 个模板格）：

| 断言 | 实测 |
|------|------|
| `top_head == cell00` | **12/12 成立**（每个模板格的上 head 链都只有一节 = `cell00`） |
| `left_head == cell00` | **10/12 成立**；两个例外是 `(3,2)` 与 `(3,3)` —— 它们指向 18 锚点 `(3,1)` |

两个例外正是"分支 B 命中分支"（`:654-657`）在起作用：`_$3((3,2))` 扫到 `(3,1)`（`ext=18` 且
`(3,1).containRow((3,2))` 为真）→ **直接返回 `(3,1)` 本身**，不再取 head-of-head。所以"全归 `cell00`"只对
上 head 成立，**左 head 会被上方/左方的 18 锚点截断**。⚠ 实现时别把两个方向写成同一份逻辑（见 §12.1 第 5 条）。

> ★ **分支 B 的"自愈递归"两函数**逐字对称**（`:658-663` vs `:696-701`，连 `x != cell / x != ROOT /
> x != _$21 / 子指针为 null` 四个前置条件都一模一样），唯一差别是 `_$2` 在进分支 B 后多一道**硬门槛**
> `if (!cell.sameRowTypeAs(head12)) continue;`（`:695`）—— 它排在读取 `headOfHead1` **之前**。
> 其余（`isLeftHeadOf` vs `isTopHeadOf`、`getCellLeftHead()` vs `getCellTopHead()`）只是名字不同。
> 因此**可以写一份泛化的 `find_head(dir)`**：把 `18↔19`、`containRow↔containCol`、`left↔top` 参数化，
> 并让 `sameRowTypeAs` 那道门槛只在上（Top）方向生效。**别把这道门槛也套到左方向**（会改坏 `engine_ext` 的
> `(3,2)/(3,3)`：它们的 `left_head` 必须指到 18 锚点 `(3,1)`，实测 `genuine`）。

### 4.3 第 3 步：`prepareCalculate` 对 extensible 的两条写入路径（**新增核实**）

这是"所有公式格的计算态 `extend != 16`"的**真正机制**，两条路：

| 格 | 走哪条 | 结果 |
|----|--------|------|
| 被包装成 `ExtExtendCell` | `ExtExtendCell.prepareCalculate`（`:314-353`） | 声明 16 的按拓扑改写；声明 17/18/19 原样 |
| 未被包装，仍是 `ExtNormalCell` | `ExtNormalCell.prepareCalculate`（`:528-563`） | **`if (expression != null) setExtensible(17)`**（`:539-541`） |

```java
// ExtNormalCell.prepareCalculate  :539-541
if (this.engine.expression != null) {
    this.setExtensible((byte)17);            // ← 公式格一律变 17
} else {
    this.setValue(this.engine.srcPrepare.getValue());   // 字面量格：值直接搬，extensible 保持 16
}
```

**拓扑改写**（`ExtExtendCell.prepareCalculate:317-329`，与 `extend.rs::topology_default` 一致）：

```java
if (this.engine.expression != null) {          // 门禁：无表达式不改写
    byte extend = this.getExtensible();
    if (16 == extend) {
        ExtCell cell00 = this.engine.cs.getCell00();
        if (leftHead == cell00 && topHead != cell00)      setExtensible(19);
        else if (leftHead != cell00 && topHead != cell00) setExtensible(17);
        else                                              setExtensible(18);
    }
    this.setCalc(false);                        // 清值，等求值
}
```

> **合并两条路径后的不变式（六夹具实测成立）**：计算态 `source.extend ∈ {17,18,19}` ⟺ 该坐标的模板格**有表达式**。
> 字面量格恒为 16。证据：`engine_sum` 行 2 全 9 格为公式 → 全 ≠16；`engine_expr` 的 `(2,3)(2,4)(2,5)` 为公式 → 17。
>
> ✅ **已修复，指向新位置**：原先 `crates/rpx-engine/src/extend.rs` 只实现了第 1 条路径
> （一个裸 `resolve()`），第二条路径没有家。现在拆成
> `crates/rpx-engine/src/extend.rs`（`ExtendMode`/`topology_default:130-140`/`calc_row_count:175-178`）
> + `crates/rpx-engine/src/extend/role.rs`（`CellRole:34-45`、`EXTEND_EXCLUDED_CELL_TYPES:57`、
> `cell_role:74-103`、`ExtendMode::resolve:113-131`）。`resolve()` 现在按角色三分支，
> 覆盖两条路径；见 §11.1 / §11.3。**判据不变**：`B2*2` 这类格必须落在 `PlainFormula` → 17。

### 4.4 第 4 步：head 链"归一"到最近的 ExtExtendCell

最后一遍遍历（`ExtCellSet.java:783-806`）把每个格的 head 沿链上溯到**最近的 `ExtExtendCell`**；若上溯回到自己，
则置为 `cell00`。意义：让非扩展格"挂"在它所在扩展区的锚点上，从而 `MainCell` 能正确分组。

```java
ExtCell left = sub.getCellLeftHead();
if (!(left instanceof ExtExtendCell)) {
    for (left = left.getCellLeftHead(); !(left instanceof ExtExtendCell) && left != sub; left = left.getCellLeftHead()) {}
    sub.setCellLeftHead(sub == left ? this._$10 : left);
}
```
证据：`:783-806`。

`setCellLeftHead`/`setCellTopHead` 有副作用：会把本格**登记进 head 的 sub 列表**
（`head.addCellLeftHead(this)`，`ExtNormalCell.java:357-378`），并检测 `head==this` 自环（`:358-361`）。
`ExtExtendCell.addCellLeftHead` 建 `_$6` 列表并给子格清 `firstLeft`（`ExtExtendCell.java:99-107`）。

**阶段 B 的产出（P0 需要落地的状态）**

| 产出 | 说明 |
|------|------|
| `kind: {Normal, Extend}` | 该格是否是扩展格 |
| `extensible ∈ {16,17,18,19}` | 运行时生效值（≠ 模板声明值） |
| `left_head / top_head: CellId` | 已归一 |
| `left_subs / top_subs: Vec<CellId>` | 挂在自己名下的子格（`_$6`/`_$5`） |
| `ext_cells: Vec<CellId>` | 扩展实例数组（`_$7`），扩展时填 |

---

## 5. 阶段 C：求值调度器（**显式状态机；Java 的异常协程不许照搬**）

`ExtCellSet.calculate(ctx)`（`:1448-1458`）→ `_$2(ctx)`（`:1483-1543`），编排如下：

```
prepareCalculate(ctx, true)                       // 阶段 B
_$16/15 = 当前矩阵尺寸                             // 缓存！（扩展会 += 更新）
_$10.setValue(null)
stack = new Stack(); special = new Queue(); _$24 = new Queue()
① _$1(ctx, stack)                                 // 状态 4：先 19 趟，再 18 趟    :1297-1364
② resetDSCurrent + setDSCurrent + _19.reset()
③ _$2(ctx, special, stack)                        // 状态 5：17 趟                 :1366-1426
④ resetDSCurrent + setDSCurrent + _19.reset()
⑤ _$1(ctx, false)                                 // calcProperties（属性码表达式）
⑥ _$1(ctx, special, stack)                        // 状态 6：special 队列排空       :1428-1446
_$14 = 7                                           // 完成
```

**状态表**

| `state` | 谁在这一趟 | 备注 |
|---------|-----------|------|
| 2 | 第 1 步（决定 extender） | `:744` |
| 3 | 第 2/3/4 步（head + 改写） | `:759` |
| 4 | **19 趟 → 18 趟** | `:1303` |
| 5 | **17 趟** | `:1367` |
| 6 | special 队列排空 | `:1429` |
| 7 | 完成 | `:1526` |
| 8 | 页面态（P1） | `:1704` |

> ⛔ **顺序是硬约束**：`19` 先于 `18`。`engine_ext` 里 19 在模板行 4、18 在模板行 3 —— 19 先把列结构
> 放大到 7 列，18 再把行插入 4 行，于是 19 锚点的子格落在**第 8 行**（夹具 `sub_values` 实测
> `(8,1)..(8,5)`）。若反过来先扩 18，子格会落在第 4 行 → 与夹具不符。

**依赖协议（异常即协程）**：格 A 求值需要格 B，而 B 的"表达式状态"比当前 pass 高时，抛出
`IIllIIIIIIIIIlll(B, B.state)`（诊断器，`ExtNormalCell.java:636-639`、`ExtExtendCell.java:362-364`）。
调度器 catch 后 `stack.push(当前格); cell = 需要先算的格;` —— 就是把"依赖栈"显式化：

```java
// 状态 4（19/18 趟），ExtCellSet.java:1319-1329 与 :1349-1359（两趟同构）
catch (IIllIIIIIIIIIlll e) {
    state = e.getState(); c = e.getCell();
    if (state == 4) { st.push(cell); cell = c; continue; }   // 先算 c
    st.push(cell); throw e;                                  // 其它：交给上层
}
// 状态 5（17 趟），:1390-1421 —— 多处理 state 6 / state 8
catch (IIllIIIIIIIIIlll e) {
    byte state = e.getState(); ExtCell c = e.getCell();
    if (state == 4 || state == 5) { st.push(cell); cell = c; continue; }
    if (state == 6) { c.reset(); special.push(cell); continue block3; }        // 交给 special 队列
    if (state == 8) { /* 标记双方 state=8，压入 _$24，跳出本格 */ }
    st.push(cell); throw e;
}
```

**表达式状态（`engine.state`）取值**（`getState()` 的语义：需要哪一趟才能算）

| 值 | 来源 | 证据 |
|----|------|------|
| 4 | 默认（`Node.getState`）；`Operator`/`Function`/`SpecialFunction` 取子树最大值 | `Node.java:107`、`Operator.java:39-47`、`Function.java:48-53` |
| 5 | `GetCell`(`:202`) / `CalcSubRpt`(`:194`) / `McOrder`(`:114`) / `CellRowNo`(`:28`) / `CellColNo`(`:28`) | `function/` 下各文件 |
| 8 | `CellSum`(`:16`) / `CellCount`(`:16`) / `PageCount`(`:22`)（依赖分页结果） | `function/` 下各文件 |

**循环引用**（M2-07，**U3 已解**）：不是"维护已访问集合"，而是 `testValue()` 里一次**带记忆的自抛**。
`testValue()`（`ExtNormalCell.java:300-319`）**两态**抛出：

```java
if (this.nonCalc()) {
    if (this.engine.expression != null) {
        if (this.rowMerge > 0) { this.rowMerge |= Integer.MIN_VALUE; }  // :304-305 首次访问本格 → 打标
        else { throw new IIllIIIIIIIIIlll(this, 1); }                   // :306-307 二次访问 → state=1（终止）
        throw new IIllIIIIIIIIIlll(this, this.engine.state);            // :309 首次 → state=依赖等级（通常 4）
    }
    this.setValue(null);
}
```
> ⚠ 细节别写错：`nonCalc()` = `(rowMerge >> 30 & 1) == 0`（`:287-289`）；置 `Integer.MIN_VALUE`（bit31）后
> `0x80000001 >> 30 == -2`，`-2 & 1 == 0` ⇒ `nonCalc()` **仍为真**。"二次访问"靠的是 `rowMerge > 0` 变假，
> 不是 `nonCalc` 变假。所以标记位用一个 `i32` 的符号位即可，Rust 侧建议用独立的 `in_progress: bool`
> 而不是复用 `row_merge` 的位（产品是把两个语义塞进同一个 int）。

state 分派：`{4,5}` → `st.push(当前格); cell = 抛出格;` 继续（`:1322-1326`、`:1393-1397`）；
`state==1` **不被任何分支消化** → `:1419-1420` `st.push(cell); throw e;` → 冒泡到 `_$2(ctx)` 的
`:1536-1538`，转 `ReportError`。

**链字符串：4 段的来源 = 栈 3 段 + 抛出格 1 段（上一版"3 段"是漏了后半截，不是少了一次 push）**

```java
// IIllIIIIIIIIIlll.java:30-32   setStack —— 由 ExtCellSet.java:1537 调用，入参就是那个依赖栈
void setStack(Stack st, MessageManager mm) {
    this._$1 = _$1(st) + this._$3.getSourceId() + mm.getMessage("CircularRefException.circle");
}   //          └ 栈里所有格 ┘ └ 抛出异常的格 ┘ └ “存在循环引用” ┘
// :42-51   _$1 —— 逐个 pop 并**前插**，故渲染顺序 = 压栈顺序的**逆序**；每条带尾随 "-"
String _$1(Stack st) { String r = ""; while (!st.empty()) r = ((ExtCell)st.pop()).getSourceId() + "-" + r; return r; }
// ⇒ 前半截已是 "id…id-"，抛出格的 id 直接续上（于是首尾会重一个，看起来"多一段"）
```

**`engine_cycle`（`B2=C2`、`C2=B2`）逐步推演**（行走 17 趟 `_$2(ctx, special, st)`，`:1366-1426`）：

| 步 | 调度器动作 | 依赖栈（底→顶） |
|----|-----------|-----------------|
| 0 | 行优先取到 `(2,2)=B2`（extend=17）；17 趟 `st` 开始为空（`:1368`） | `[]` |
| 1 | `B2.calculate` 求 `C2` → `C2.testValue()` **首次** → 抛 `IIll(C2, 4)`；catch `state==4` → push **B2**，`cell=C2` | `[B2]` |
| 2 | `C2.calculate` 求 `B2` → `B2.testValue()` **首次** → 抛 `IIll(B2, 4)`；catch `state==4` → push **C2**，`cell=B2` | `[B2, C2]` |
| 3 | `B2.calculate` 再求 `C2` → `C2.testValue()` **二次** → 抛 `IIll(C2, 1)`；catch 无分支接管 → push **B2**，`throw` | `[B2, C2, B2]` |
| 4 | `_$2(ctx)` catch（`:1536-1538`）→ `setStack([B2,C2,B2], mm)`，抛出格 = `C2` | — |

渲染 = `_$1(pop 序 B2,C2,B2)` = `"B2-C2-B2-"` ⊕ `thrower "C2"` ⊕ `"存在循环引用"` =
**`B2-C2-B2-C2存在循环引用`** —— 与 `engine_cycle.expected.json` 的 `cause_message` **逐字相同**。

**通则（Rust 侧照此产出）**：`链 = reverse(压栈顺序) 以 "-" 连接 + "-" + 抛出格 id`（再拼本地化文案）。
压栈顺序 = 调度器"进入某格"的顺序。长度 k 的引用环 → 压栈 k+1 次（回到环首时二次 `testValue` 抛 `state=1`
即中止），**链共 k+2 段**。

**三段实测锚点**（探针 `ProbeP0` 本机重跑，2026-10-12；见 §12.3）

| 场景 | 依赖栈（底→顶） | 抛出格 | 链 |
|------|----------------|--------|-----|
| `B2=C2`,`C2=B2`（k=2，`engine_cycle`） | `[B2,C2,B2]` | C2 | `B2-C2-B2-C2` |
| `B2=B2` 自引用（k=1） | `[B2,B2]` | B2 | `B2-B2-B2` |
| `leftHead` 成环 `B2.leftHead=C2, C2.leftHead=B2` | `[C2,B2]` | C2 | `C2-B2-C2` |

> `leftHead` 成环走的是**另一条路径**（阶段 B 的 `_$3:641-646`，不是 `testValue`）：`st.push(cell)` 后沿
> `head1` 的链 push 到 `cell` 为止再 `throw`，同样由 `setStack` 渲染。**抛出格是行优先下"后一个"的格**：
> `_$3(B2)` 时 C2 的 `leftHead` **字段**还是 `null`（`getLeftHead()` 读的是模板格
> `engine.srcPrepare.getLeftHead()` 的**字符串**，`:1420-1421`，与 `leftHead` 引用字段是两回事），
> 于是 `B2.isLeftHeadOf(C2)` 为假 → 先直挂成功；轮到 `_$3(C2)` 时 B2 的字段已指向 C2 → 成环。
> ⛔ **照写时别把两者混成一体**：`declared_left_head()`（模板字符串，属性码 30）与 `left_head`（运行时引用）
> 是两个独立的存储；`_$3`/`_$2` 的分支 A 读前者，分支 B 与归一化读写后者。

**Rust 侧如何不照搬异常协程**：Java 用 `throw` 做栈跳转，Rust 里改成显式的
`NeedCell { cell, state }` 返回值 + 循环。骨架：

```rust
enum Step { Done, Need(CellId, u8) }          // 取代 throw IIllIIIIIIIIIlll
fn eval_pass(calc: &mut CalcState, state: u8, filter: impl Fn(ExtendMode)->bool) -> Result<()> {
    let mut stack: Vec<CellId> = Vec::new();
    for r in 1..=calc.rows { for c in 1..=calc.cols {
        let Some(mut cur) = calc.owner(r, c) else { continue };
        if !filter(calc.extensible(cur)) { continue }
        loop {
            match calc.calculate_cell(cur) {
                Step::Done    => match stack.pop() { Some(x) => { cur = x; continue } None => break },
                Step::Need(x, s) if s == state => { stack.push(cur); cur = x }
                Step::Need(x, s)               => return Err(/* 交上层；s==1 → CircularRef */),
            }
        }
    }}
    Ok(())
}
```

---

## 6. 扩展算法：`extendRow` / `extendCol`（P0 核心）

> **算法正文（逐行、带全部 `file:line`）在 `docs/format-contract.md` 附件 I** —— 本节只留
> 决策所需的骨架与"最容易做错"的判定式；两处如有冲突，**以附件 I + 产品实测为准**（附件 I 由总监维护，
> §6.3 的 U2 结论与 §5 的 U3 结论已交叉记入附件 I）。

触发点在 `ExtExtendCell.calculate`（`:358-483`）：表达式求值结果若是 `List` →
`18==extensible ? extendRow(this,len) : extendCol(this,len)`（`:425-430`）→ 再逐个 `_$7[i].setValue(...)`。
⛔ 求值结果 `List` **为空**时在 `:376-384` 就 `return` 了（不扩展、值置空）—— 见 §12.4 的 U4。

### 6.1 行带 / 列带的定义（anchor 的"包络"）

| 函数 | 定义 | 证据 |
|------|------|------|
| `beginRow = _$4(cell)` | `cell.row` 与其 `leftSub` 递归中的最小行 | `:928-938` |
| `endRow = _$3(cell)` | `cell.row + cell.rowMerge` 与其 `leftSub` 递归中的最大行 | `:940-950` |
| `rows = endRow - beginRow` | **块高**（无合并=1；`engine_merge` 的 `A2:A3` → 2） | `:991-993` |
| `beginCol = _$2(cell)` / `endCol = _$1(cell)` | 上同构（用 `topSub`） | `:1059-1081` |

### 6.2 `extendRow(cell, count)` 步骤（`:981-1057`）

```
1. beginRow/endRow/rows 如上；extCells = new ExtCell[count]; extCells[0]=cell; cell.setExtCells(extCells)
2. _$3(endRow, rows*(count-1))            // 在 endRow 处插入 rows*(count-1) 行，并修正后续格的 row
3. cell.expandLeftHead(rows*(count-1))    // 把"包含本行带"的 head 的 rowMerge 撑大（合并跨度不下沉）
4. 对 m=1..count-1:
     对行带 [beginRow,endRow) 每行 i：把 (i, *) 整行克隆到 (i + m*rows, *)：
       - cells2[0] = cells1[0].clone(); addRow(k)                       // 行头
       - 对 j：若 sub 是"本格拥有、且 cell.isLeftHeadOf(sub)" → 克隆 newSub，按其 rowMerge/colMerge
         铺满矩形；若 sub==cell → extCells[m]=newSub; newSub.setExtCells(extCells)
5. 对 m=1..count-1（第二遍，修正 head）:
       - 非锚点克隆：newSub.setCellLeftHead( matrix.get(head.row + k, head.col) )   // 平移左 head
       - 若 sub.topHead != cell00 → newSub.setCellTopHead(sub.topHead)              // 上 head 原样
```

**关键点（逐条已核实）**

| 点 | 规则 | 证据 |
|----|------|------|
| 插入位置 | 在 `endRow`（行带**之后**）插入，不是行带内 | `:997` |
| 插入量 | `rows*(count-1)`（不是 `count-1`）→ 块高被乘进去 | `:997` |
| 行数公式 | `计算态行数 = 模板行数 + (数据集行数−1) × 块高` | `:997` + `engine_merge`（4+2×2=8） |
| 左 head 平移 | 克隆的 leftHead 指向 `head + k`（同列、下移 k） | `:1046-1048` |
| 上 head 继承 | 非 cell00 时原样继承（同一列带） | `:1049-1051` |
| `expandLeftHead` | 对 chain 上 `containRow(this)` 的 head 做 `addRowMerge(mergeInc)` 并重铺 `_$1(...)` | `ExtNormalCell.java:462-475`、`ExtExtendCell.java:213-224` |

### 6.3 `extendCol(cell, count)` 步骤（`:1083-1165`）

镜像但**多两件事**：

```
1. 前置断言：!this._$2(cell.getRow())  → 抛 ReportError（该行不是"可扩展行"就不许横向扩）
   （_$2(row) 判 rowType ∈ {-91,-95,-94,-90}；:846-856）
2. beginCol/endCol/cols；extCells[0]=cell; setExtCells
3. _$2(endCol, cols*(count-1))            // = _$1(endCol,count,$_7(),$_6(),true)：插列 + 修 col 索引 + 对齐列头
4. 克隆列头行 (0, j2)：cl = sub2.clone(); cl.addCol(m*cols); _$26.set(0, j2+m*cols, cl)      // :1098-1105
5. cell.expandTopHead(cols*(count-1))     // ★ 见下方"真实机制"：撑大的是 cell00 的 **topSubs 列表** // :1106
6. 对每行/每列，若 cell.isTopHeadOf(sub) → 克隆 newSub 并铺满；sub==cell 时填 extCells[m]     // :1108-1135
7. 第二遍修正：非 cell00 的 leftHead 若落在 [beginCol,endCol) 内 → 平移 cols*m，否则原样        // :1136-1164
```

> **附件 H.4 补遗的机制解释（U2 已解；探针 `ProbeHeads` 实测，见 §12.2）**
>
> 上一版的措辞（"把锚点 `topHead` 链上包含锚点的格 `colMerge += 4`"）**方向对、对象错**：
> 锚点的 `topHead` 链**只有一节**，就是 `cell00`；受害格 `(1,1)(2,1)(3,1)` **不是链成员**。
> 被撑大的是 **`cell00` 自己的"上子格列表" `_$5`（topSubs）**。两步分开看：
>
> 1. `ExtNormalCell.expandTopHead(mergeInc)`（`ExtNormalCell.java:477-490`）：从 **`this.topHead`** 起步
>    （遍历起点 = 锚点的 `topHead` 字段），沿 `getCellTopHead()` 走；
> 2. 对链上**每个**节点调它的**虚方法** `expandTopHead(sub, cur, mergeInc)`。`ExtExtendCell` 那份
>    （`ExtExtendCell.java:226-237`）**遍历该节点自己的 `_$5`**：对每个 `sub1`，若
>    `sub1 != sub && sub1.containCol(cur)` 成立 → `sub1.addColMerge(mergeInc)` 并 `_$1(...)` 把该格重铺满矩形。
> 3. 回到 `ExtNormalCell` 那份（`:482-487`）：链节点自身若 `head.containCol(this)` 也 `addColMerge` 并重铺。
>
> 代入 `engine_ext`（`cols=1, count=5 ⇒ mergeInc=4`）：链 = `[cell00]` ⇒ 只有
> `cell00.expandTopHead(sub=<anchor>, cur=<anchor>, 4)` 被调 ⇒ 扫 `cell00._$5` ⇒ 命中 `(1,1)(2,1)(3,1)`，
> 各 `addColMerge(4)` → `cm = 1+4 = 5`。两处"自己人"被排除：
> ① **锚点本身** 由 3 参版开头的 `sub1 == sub` 挡掉（首轮 `sub` 就初始化为锚点，见 `:479`）——
> 这道守卫是必要的，否则 `anchor.containCol(anchor)` 为真会把锚点自己也撑大（实测 `(4,1)` 的 `cm` 恰为 1）；
> ② **`cell00`** 由 1 参版里的 `head.containCol(this)` 挡掉（`:482`，第 0 行与数据行不同 rowType 族）。
> **判定式**：
>
> ```
> 受害格集合 = { c ∈ cell00._$5 : c != anchor ∧ c.containCol(anchor) }
> containCol(sub) = sameRowTypeAs(sub) ∧ this.col <= sub.col ∧ sub.col+sub.colMerge <= this.col+this.colMerge
> ```
>
> **`cell00._$5` 实测内容**（`engine_ext`，探针原样 dump；n=12；★ = 被撑到 `cm=5` 的格）：
>
> | 成员 | `cell00._$5` 全部成员（坐标 = 计算态 `getRow/getCol`） |
> |------|------------------------------------------------------|
> | 列 1 | ★`(1,1)` `(2,1)` `(3,1)`、`(8,1)`=19 锚点本身 |
> | 列 2 | `(1,6)` `(2,6)` `(3,6)` `(8,6)` |
> | 列 3 | `(1,7)` `(2,7)` `(3,7)` `(8,7)` |
>
> 验证要点（全部由探针的**引用相等**断言给出，不是"看着像"）：
> `ecs.getSource(0,0) == cell00`（`_$10`）；`(1,1)(2,1)(3,1)(4,1)` 的
> `getCellTopHead() == cell00` 为真、且都在 `cell00._$5` 里；`(3,2)(3,3)` 的 `leftHead` 指向
> 18 锚点 `(3,1)` 而**不在** `_$5` 的受影响集（`cm` 保持 1）—— 这条排除了"是 `leftHead` 链在做扩列"的误判。
>
> **反向交叉验证**（变体 D：`18@(2,1)` 在 `19@(3,1)` 上方）：受害格是 `(1,1)` **和 18 锚点 `(2,1)`**，
> 两者 `cm=5`；机制同一（都是 `cell00._$5` 成员 + `containCol`）。⇒ **结论：19 会把"它上方同族、
> 列区间被它覆盖"的格一律撑成 `cm=1+(count-1)*cols`，不论那些格自己是字面量还是 17/18 扩展格。**
>
> **这是"19 只影响单元格、不影响列结构"错误的第二个证据**：计算态列数 3→7，且锚点上方 3 格被改变
> 宽度并重铺矩形（`_$1` 覆盖 `(1,1)..(1,5)`）——所以计算态 `(1,2)` 在**计算矩阵**里是 `(1,1)` 那个对象，
> 而 `getSource(1,2)` 给出的却是被挤到 `(1,6)` 的模板格 B1（§0.1 的双矩阵差异在这里第二次显形）。

### 6.4 合并区 × 扩展（M2-04）—— 已核实

| 观测（`engine_merge`：`A2:A3` 合并 + `extend=18`；`C2:D2` 合并） | 值 |
|------|-----|
| 计算态网格 | **8×4**（模板 4 + (3−1)×**2**） |
| 锚点 `(2,1)` | `row_merge=2`、`col_merge=1`、`extend=18`、`sub_cells=3` |
| 子格位置 | `(2,1)/North`、`(4,1)/East`、`(6,1)/South` —— **每个实例占 2 行** |
| 横向合并 | `(2,3)`/`(2,4)` 都 `col_merge=2`，复制进每个实例 |
| 机制 | `expandLeftHead` 撑大锚点链的 `rowMerge`；`extendRow` 按 `rowMerge/colMerge` 铺满矩形（`:1015-1021`） |
| 公式 | `计算态行数 = 模板行数 + (数据集行数−1) × rowMerge` |

> 契约一句话：**合并跨度不因扩展而收缩或分摊；扩展插入的是"整个合并块"。** 证据：`ExtCellSet.java:998,1015-1021`、
> `engine_merge.expected.json`。`extend.rs::calc_row_count` 已含块高参数。

### 6.5 17 的"整列 List"—— 已核实（与 18/19 的差别在"是不是 List 分支"）

| 模式 | 是 `ExtExtendCell` 吗 | `calculate` 走哪条 | 结果 |
|------|----------------------|--------------------|------|
| 17（带可扩展表达式，如 `ds1.select(region)`） | **否**（`checkExtend`→false；但 `ExtNormalCell.prepareCalculate` 把它写成 17） | `ExtNormalCell.calculate`（`:633-676`）只 `setValue(...)` | 整个 `List` 塞进一格（`engine_ext (2,1)` = `["R1".."R5"]`，`sub_cells=1`） |
| 17（不可扩展表达式，如 `B2*2`） | 是（被包装后改写为 17） | `ExtExtendCell.calculate` 的非 List 分支 | 标量（`engine_sum (2,3)=200`） |
| 18 / 19 | 是 | `ExtExtendCell.calculate` 的 **List 分支** → `extendRow`/`extendCol` | 逐行/逐列铺开，`sub_cells=count` |

> ⛔ 判定硬约束（M2-15）：17/18/19 必须在**不同行**才可区分。旧布局（同行）里三者 `sub_cells` 全是 1，
> 根本不可区分。`engine_ext` v0.5 已重排，实现必须用重排后的基准。

---

## 7. `sub_cells` 语义（对齐探针 `subCells` / `extCellSize()`）

**定义**：`sub_cells = src.extCellSize()`（`CalcJson.java:82-83`）。

| 类 | `extCellSize()` | 含义 | 证据 |
|----|-----------------|------|------|
| `ExtCell`（默认） | 1 | 非扩展格 | `ExtCell.java:172-174` |
| `ExtNormalCell` | 继承默认 → 1 | 17 格 | 实测 `engine_ext (2,1) sub_cells=1` |
| `ExtExtendCell` | `_$7 == null ? 1 : _$7.length` | **扩展实例数** = `count` = 数据集行数 | `ExtExtendCell.java:126-128` |

**`_$7` 的填法**：`extendRow`/`extendCol` 里 `extCells = new ExtCell[count]; extCells[0]=cell; ...; extCells[m]=克隆`
（`:994-996,1022-1025` / `:1094-1096,1125-1128`），最后 `setExtCells(extCells)`。

**三种形态的坐标规律（夹具实测）**

| 模式 | 锚点坐标 | `sub_cells` | 子格坐标 | 值 |
|------|----------|-------------|----------|-----|
| 17 | `(2,1)` | 1 | — | `type=list`，`["R1".."R5"]` |
| 18 | `(3,1)` | 5 | `(3,1)(4,1)(5,1)(6,1)(7,1)` 同列向下 | `R1..R5` |
| 19 | `(4,1)`（扩展前）/`(8,1)`（最终） | 5 | `(8,1)(8,2)(8,3)(8,4)(8,5)` 同行向右 | `R1..R5` |
| 18 + 合并块高 2 | `(2,1)` | 3 | `(2,1)(4,1)(6,1)` | North/East/South |
| 18 + 40 行 | `(3,1)` | 40 | `(3,1)..(42,1)` | `R1..R40` |

**探针对应**：`ProbeP0.dumpSubs` 用 `src.extCellSize()` 与 `src.getExtCell(i)`（`ProbeP0.java:248-266`），
`src.getRow()/getCol()` 是**计算态坐标**；而 `expected.json` 的 `source.src_row/src_col` 是**模板态坐标**
（`getSourceRow/getSourceCol`）。**两套坐标不要混用** —— 见 §0.1。

**夹具的 `sub_values` 只在 `sub_cells > 1` 时落盘**（`CalcJson.java:84-97`）。`sub_values` 挂在下
`source` 里，元素 `{r,c,value,type}`，其中 `sub.getValue(false)`（`:91`）。

---

## 8. 模块拆分（`crates/rpx-engine/src/`，P0）

对齐 `spec-m2.md` §4.1 并**补两处**（head 图与调度器在原布局里没有家，硬塞进 `extend.rs` 会让它超 300 行）：

| 文件 | 职责 | 状态 | 目标 ≤300 行 |
|------|------|------|-------------|
| `lib.rs` | `pub use` 公开面；`pub fn calculate(template, datasets) -> Result<CalcGrid>` | 已有，需扩 | ✔ |
| `error.rs` | `EngineError`（`CircularRef{chain}` / `UnknownCell` / `MissingDataset` / ...） | 已有 | ✔ |
| `value.rs` | `Value`：类型保真（int/long/double/float/decimal/string/boolean/list）；M2-14 | 已有 | ✔ |
| `arena.rs` | `CalcState`：引擎矩阵 + 计算矩阵 + `CellId`/`SourceRef`；`getSource`/`getCell` | 已有，需扩（加引擎矩阵） | ✔ |
| `main_cell.rs` | `MainCell` 判定（`lIlIlllIIllllIII:21-39`） | 已有 | ✔ |
| `extend.rs` | `ExtendMode` + `topology_default:130-140` + `calc_row_count:175-178`（原名 `calc_matrix_rows`）+ **`extend_row`/`extend_col`/带包络** | 前半已有；**缺扩展算法** | 需拆 |
| `extend/role.rs`（已有） | `CellRole:34-45` + `EXTEND_EXCLUDED_CELL_TYPES:57` + `cell_role:74-103` + `ExtendMode::resolve:113-131`（§4.3 两条写入路径） | **已有** | ✔ |
| **`heads.rs`**（新增） | head 图：`find_left_head`/`find_top_head`（`_$3`/`_$2`，伪码见 §4.2）+ `set_cell_left/top_head`（含 `_$6`/`_$5` 副作用）+ 归一（§4.4）+ **循环检测**（附件 G.3） | 待写 | 拆细 |
| **`sched.rs`**（新增） | 调度器：三趟 pass、状态常量、`NeedCell` 协议、`MainCell` 维护、依赖栈 → 链（§5） | 待写 | 拆细 |
| `sub_cells.rs` | `_$7`/`_$6`/`_$5`：扩展实例数组与左/上子格列表（**`_$5` 是 §6.3 扩列的施力点**） | 待写 | ✔ |
| `expr/{ast,names,lexer,classify,parser}.rs` | 手写 lexer + Pratt；`ast.rs` 提供 `is_extend()`（AST 谓词，§4.1）喂给 `role.rs::cell_role` —— 否则"声明 16 的格是不是扩展格"无处可算。相对/绝对引用（M2-06）与 `$` 语义（附件 G.1）的**求值**部分未落 | **切片已在写**（契约见 §13；`eval` 仍是显式未实现出口） | 拆分 |
| `merge.rs` | `mergeSame`/`mergeNull`（**只能在分页后调用**，见 §8.1） | P1 实现 | ✔ |
| `pager.rs` | SimplePager 等价（P1） | P1 | ✔ |

**公开面（`lib.rs` 建议）**

```rust
pub fn calculate(rd: &ReportDefine, ds: &DataSetMap) -> Result<CalcState, EngineError>;
pub fn paginate(state: &CalcState, paper: (f32, f32), cols: u8) -> Result<Pages, EngineError>; // P1
pub use {CalcState, CalcGrid, CalcCell, SourceRef, CellId, Value, ExtendMode, MainCell, EngineError};
```

### 8.1 `mergeSame` 的位置（**必须写进模块注释，否则必踩**）

`ExtCellSet.mergeSame()` **不在 `Engine.calc()` 里**。全量调用点只有三处：`SimplePager.java:133`（单页分支）、
`SimplePager.java:179`（多页循环，对每页克隆调用）、`CalcSubRpt.java:138`。因此：

| 事实 | 证据 |
|------|------|
| `mergeSame` 只在 `PageBuilder.createPages()` 之后的页网格上可见 | `SimplePager.java:131-135`、`:175-181` |
| 码值：89=`mergeSame`（默认 **97=关**）、90=`mergeNull`、91=`mergeSameMode` | 附件 H.7.2；`ExtCellSet.java:1856,1908,1955` |
| 纵向判定：`getMergeSame() ∈ {98,100}` 且 `getMergeSameMode() != 120` | `ExtCellSet.java:1856` |
| 横向判定：`getMergeSame() ∈ {99,100}` 且 `getMergeSameMode() != 120` | `ExtCellSet.java:1955` |
| mode 影响"基准格"取法：118=取 `getSource()`；119=取相邻列/行；120 跳过某一分支 | `ExtCellSet.java:1863-1868,1908,1961-1966` |

⛔ **M2-08（P0）与分页（P1）的依赖冲突 → 已在 §11 定案**：`mergeSame` 的**唯一可观测位置**是分页后的
页网格，所以合并对拍必须以"**单页夹具**"为前提（`engine_samemerge` 的 `page_count=1`）。超出单页的
`mergeSame` 需要逐页调用 `SimplePager` 才能观测，**不属 P0**。

---

## 9. 已核实 vs 待验证

### 9.1 ✅ 已核实（附证据）

| # | 结论 | 证据 |
|---|------|------|
| V1 | 计算链路 `Engine.calc` = `calcDataSet` → 建 `ExtCellSet` → `calculate` → `reflush` | `Engine.java:75-122` |
| V2 | 两张矩阵：`_$26` 计算矩阵、`_$25` 引擎矩阵；扩展只改 `_$26` | `ExtCellSet.java:95-96,810,881,903` |
| V3 | `expected.json` 的 `source` = `_$25.get(r,c).srcCurrent`；非空 ⟺ `r<=template_rows && c<=template_cols` | `ExtCellSet.java:623-629`；六夹具逐格验证 |
| V4 | 阶段 B 顺序：包装 → 算 head → 改写 → 归一 | `ExtCellSet.java:745-806` |
| V5 | `checkExtend`：16→`isExtend()`；17→false；18/19→true；`cellType∈{-61,-57,-60}`→false | `ExtNormalCell.java:492-514` |
| V6 | **公式格一律 `extensible≠16`**：未被包装的走 `ExtNormalCell.prepareCalculate:539-541` 写 17 | `ExtNormalCell.java:539-541` |
| V7 | 16 改写：`(cell00,非cell00)→19`、`(非cell00,非cell00)→17`、其余→18；只对带表达式生效 | `ExtExtendCell.java:317-329` |
| V8 | `null` head ≠ cell00（落"非 cell00"侧） | `ExtExtendCell.java:321`；`arena.rs:135` `cell_id_sentinels_are_distinct_and_copyable` 已单测 |
| V9 | 调度趟次：**19 → 18 → 17 → special** | `ExtCellSet.java:1304,1334,1369,1431` |
| V10 | 依赖协议：`IIllIIIIIIIIIlll(state)` + 依赖栈；state6 进 `special`，state8 进 `_$24` | `ExtCellSet.java:1319-1329,1390-1421` |
| V11 | 循环引用由 `testValue()` 的符号位检测（`state==1`）→ 链字符串 | `ExtNormalCell.java:300-319`；`IIllIIIIIIIIIlll.java:30-32` |
| V12 | `extendRow`：插 `rows*(count-1)` 行于 `endRow`；克隆行带；平移左 head；`expandLeftHead` | `ExtCellSet.java:981-1057` |
| V13 | `extendCol`：插 `cols*(count-1)` 列；克隆列头；`expandTopHead`；平移左 head | `ExtCellSet.java:1083-1165` |
| V14 | 行数公式 `= 模板行数 + (数据集行数−1)×块高`（块高=`rowMerge`） | `ExtCellSet.java:997`；`engine_merge` 4+2×2=8 |
| V15 | 19 会放大**列结构**：受害格 `col_merge=N` | `engine_ext` `(1,1)(2,1)(3,1) cm=5`；`expandTopHead` `ExtNormalCell.java:477-490` |
| V16 | `sub_cells = extCellSize()`；`ExtExtendCell._$7` 长度 = count | `ExtExtendCell.java:126-128`；夹具 |
| V17 | 17 = 整列 `List` 落一格；18/19 = 逐行/逐列铺开 | `engine_ext`；`ExtNormalCell.calculate:633-676` vs `ExtExtendCell.calculate:376-454` |
| V18 | `mergeSame` 只在 `createPages()` 后可见 | `SimplePager.java:133,179` |
| V19 | `$` 是"用保存的 current"（isSaved），不是绝对引用；`$B$2` 必须报错 | 附件 G.1；`Expression.java:193-203`；`CSVariable.java:31-44,116-133` |
| V20 | 相对引用**不是算术偏移**：`CSVariable.calculate` 用 `current` 的 head 链解析到"对应实例" | `CSVariable.java:116-133,176-235` |
| V21 | `mergeNull()` 与 `mergeSame()` 同族、同调用点 | `ExtCellSet.java:2051`；`SimplePager.java:134,180` |
| V22 | `_$3`（左 head）**从不**调 `sameRowTypeAs`（改用 `containRow`）；`_$2`（上 head）在 `:691,:695` 两处调；且 `_$2` 的命中条件用 19、`_$3` 用 18 | §4.2；`ExtCellSet.java:649-665` vs `:686-703`；`ExtNormalCell.java:283-285` |
| V23 | 19 扩列的受害格 = `cell00._$5`（topSubs）中 `containCol(anchor)` 的成员，**不是**锚点 `topHead` 链成员；`engine_ext` 全部模板格 `topHead == cell00`（链只 1 节） | §6.3；`ExtNormalCell.java:477-490` + `ExtExtendCell.java:226-237`；探针 `ProbeHeads`（引用相等断言） |
| V24 | 循环链 = `reverse(依赖栈) 加 "-" 连接 + "-" + 抛出格 id`；长度 k 的环 → **k+2** 段（栈 k+1 次） | §5；`IIllIIIIIIIIIlll.java:30-32,42-51`；探针 `ProbeP0` 三场景 3/3 |
| V25 | `Expression.isExtend()` 是**解析树的纯递归谓词**（与 `isDSFunction()` 是两条链），必须随表达式切片交付 | §4.1；`Node.java:82`、`Operator.java:51-56`、`CSVariable.java:379-381`、`Constant.java:22-24`、`ValueList.java:30-32`、`operator/Link.java:100-102`、各 `function/*.java` |
| V26 | `testValue()` 的两态抛：首次 `rowMerge>0` → 置 `MIN_VALUE` 后抛 `engine.state`；二次 `rowMerge<0` → 抛 `state=1`（终止信号）；`nonCalc()` 在两次间**不变** | §5；`ExtNormalCell.java:287-289,300-319` |
| V27 | **`checkExtend` 构造表达式时 `ds` 恒为 `null`**（`new Expression(cs, null, ctx, expStr, false)`）⇒ `Expression.createNode:225` 的 DS 函数回退分支**在本项目不可达** ⇒ **裸 `select(qty)`/`group(...)`/`filtercross(...)` 永远不是 DS 函数**，落 `UnknownSymbol` | §13.0 / §13.4；`ExtNormalCell.java:498-504` |
| V28 | `isExtend()` 在整个产品里**只有一个调用点**（`reportlite` 线），另一处在姊妹产品 `report/` 线 | §13.0；全 `re_work/src` grep |
| V29 | `disp_value` 的落盘判定是 **`\|\|` 不是 `&&`**：`engine_disp` 的 `(2,3)` 是 `value=null` + `type="null"` + `disp_value="仅显示值"` 且**照常落盘** ⇒ **`disp_value` 不是死键**（推翻 7 夹具时代的"出现 0 次"结论） | §11.5 ②；`engine_disp.expected.json`、`CalcJson.java:51-53,71-73` |
| V30 | `reportlite` 表达式包的 `priority` 实际取值只有 **1,2,4,5,6,10,11,13,14,16,17,18,19** ⇒ `Node.java` 里的 `PRI_ASS/BOR/BXOR/BAND/SHIFT/NEW`（3/7/8/9/12/15）**无任何类使用** ⇒ 不存在位运算/移位表达式 | §13.1；全量 grep `priority = ` |

### 9.2 ⚠ 待验证（**禁止当结论用**，附验证方法）

| # | 项 | 现状 | 验证方法 |
|---|----|------|----------|
| ~~U1~~ | `_$3`/`_$2` 的"跳过/命中"细节与 `sameRowTypeAs` 作用面 | ✅ **已解 → §4.2**（可照写伪码 + 四处不对称表）。**结论**：`_$3` 确实不调 `sameRowTypeAs`（用 `containRow`）；`_$2` 调两次；`_$2` 命中用 19、`_$3` 用 18 | 源码逐行 + 探针 `ProbeHeads` 交叉验证（`(3,2)/leftHead=(3,1)` 与 `(1,1)/topHead=cell00` 两种形态都对上） |
| ~~U2~~ | 19 锚点 `topHead` 链的确切成员 | ✅ **已解 → §6.3**。**结论**：链只 1 节 = `cell00`；受害格是 `cell00._$5` 里 `containCol(anchor)` 的成员 | 探针 `ProbeHeads` 直接读 `cell00._$5` + 引用相等断言（`ecs.getSource(0,0)==cell00`） |
| ~~U3~~ | 循环链的确切段数与来源 | ✅ **已解 → §5**。**结论**：4 段 = 栈 3 段 + 抛出格 1 段；`k` 环 → `k+2` 段。上一版"少一次 push"是误判 | 探针 `ProbeP0` 三场景（互引用/自引用/head 成环）全部复现 |
| ~~U4~~ | 空数据集（0 行）时的扩展行为 | ✅ **已解（源码级，不需要新夹具）→ §12.4**。**结论**：空 `List` 在 `ExtExtendCell.calculate:376-384` 就被拦住 `return`（锚点置**空值**、**不扩展**、无子格）；`count` 的语义是"表达式返回的 `List` 长度"，空集就是 0 ⇒ 行数 = 模板行数。⚠ 但 `extendRow/extendCol` 里 `new ExtCell[count]` 后紧接 `extCells[0] = cell`（`ExtCellSet.java:994-995`/`:1094-1095`）⇒ **直接传 `count=0` 会 AIOOBE**，只是上游永远拦得住 | `ExtExtendCell.java:376-384` + `ExtCellSet.java:994-995,1094-1095`（本轮据总监提供的源码定位复核，**未造夹具**；Rust 侧定案见 §12.4） |
| U5 | `calcProperties` 与属性码表达式（`rowVisibleExp` 等）的调度细节 | 只知它排在求值之后（`_$1(ctx,false):1188-1221`） | P0 属性面未涉及；留 P1 |
| U6 | `reflush()` / 页面态（state 8）的重算范围 | P1 | 分页实现时读 `reflush:2258` |
| U7 | `_$2()` 无参方法（`:1520` 调用）的作用 | 未读 | grep + 读该方法 |
| U8 | 阶段 B 第 3 步（head 归一）里 **`topHead` 分支与 `leftHead` 分支不对称**（`:798-804` 缺 `else`）：若上溯回到 `sub` 自身，会先置 `cell00` 再**无条件** `setCellTopHead(sub)` → `topHeadCircle` 异常 | 本批夹具未触发（所有 head 已是 `cell00`，在 `:798` 就 `continue` 了）；**不排除是 CFR 反编译假象**。**总监 2026-10-12 决定：按 `leftHead` 分支的对称语义实现，`javap -c` 复核原始控制流后再决定要不要补夹具** | ① **`javap -c`** 反编译 `ExtCellSet` 的 `create` 方法，看 `:798-804` 的字节码分支（`ifne/ifnull` 序列）有没有那个 `else`；② 若证实缺 `else`：造夹具 —— 让某格 `topHead` 指向一个**非**扩展格、且该非扩展格的上溯会回到它自己。⚠ **`docs/format-contract.md` 附件 I 未涉及这一点**（附件 I 只覆盖 `extendRow`/`extendCol`），所以它不能作为本条的旁证 |

---

## 10. 待验证项导航（**该读哪一节 / 证据在哪**）

> 本节**不含任何新结论**，只做导航：每个待验证项的「结论与证据」在 §9.2、「实现落点（哪一步）」在 §12.1、
> 「已被核实的前置事实」在 §9.1。**动某一步之前先顺着这张表读一遍，别只读结论那一栏。**
>
> ⚠ **历史索引**：本节原来是"交给人类决策的 top-3 不确定项"，三项**已于 2026-10-12 定案**（见 §11），
> 原表与定案前的论证保留在文末的 `<details>` 里，**不要照那儿的措辞实现**。

| 项 | 结论与证据 → | 实现落点 → | 已核实的前置 → | 证据位置（探针 / 命令） |
|---|---|---|---|---|
| U1 `_$3`/`_$2` 的跳过-命中细节 | §9.2（✅ 已解） | §12.1 第 5 步 | §9.1 V22 | 源码 `ExtCellSet.java:631-666` / `:668-704` + 探针 `ProbeHeads`（配方见 §12.2） |
| U2 19 锚点 `topHead` 链 | §9.2（✅ 已解） | §12.1 第 6、8 步 | §9.1 V23 | 探针 `ProbeHeads`：反射读 `cell00._$5`（配方见 §12.2） |
| U3 循环链段数与来源 | §9.2（✅ 已解） | §12.1 第 10 步 | §9.1 V24、V26 | 探针 `ProbeP0` 的 S3/S4/S5（配方见 §12.3） |
| U4 空数据集（`count=0`） | §9.2（✅ 已解，源码级） | §12.1 第 8、11 步 | §9.1 V14 | **无需夹具**：`ExtExtendCell.java:376-384`（空 `List` 在扩展前 return）；`ExtCellSet.java:994-995,1094-1095`（`count=0` 会 AIOOBE） |
| U5 属性码表达式调度 | §9.2（⚠ 未解） | §12.1 第 7 步之后（P1） | — | 探针 `ProbeExpr`（**待建**，配方见下） |
| U6 `reflush()` / 页面态 | §9.2（⚠ 未解，P1） | P1（分页） | — | 读 `ExtCellSet.java:2258` |
| U7 `_$2()` 无参方法 | §9.2（⚠ 未读） | §12.1 第 7 步 | — | 探针 `ProbeExpr`（**待建**，配方见下） |
| U8 `topHead` 分支缺 `else` | §9.2（⚠ 保留，**按 `leftHead` 对称语义实现**） | §12.1 第 5、6 步 | — | `javap -c` 看原始控制流（⚠ 附件 I 未涉及，不能当旁证） |
| 分组头动态生成 | `docs/format-contract.md` **附件 J**（✅ 已解：引擎**从不**合成 `-90`） | §12.1 第 8 步 | — | 探针 `ProbeGroup`（S1–S8）+ `tests/fixtures/probe_group_out.txt` |
| `count` 的确切语义 | `docs/format-contract.md` 附件 H.4 + §12.4 的 U4 | §12.1 第 8 步 | — | `ExtExtendCell.java:379-380,425-430` |

**U5 / U7 的探针配方（`tests/fixtures/ProbeExpr.java`，**尚未创建**）**
⚠ 这两个项**当前没有证据路径**，实现到 §12.1 第 7 步时必须先把探针补上，否则只能靠"读源码猜"：

| 目的 | 怎么做（照 §12.2 的 `ProbeHeads` 模板抄） |
|------|------|
| U7 `_$2()` 无参 | `getDeclaredMethods()` 按 `getParameterCount()==0` 挑出无参那一支（`_$2` 有 5 个重载：`(int row)`/`(int col,int count)`/`(ExtNormalCell)`/`(ExtCell)`，见 §6 的多重载表）→ 在 `calculate()` **前后各调一次**，打印返回值类型 + `_$26` 的 `getRowSize()`/`_$16`/`_$19` 状态，diff 出它到底改了什么 |
| U5 属性码表达式 | 造一个 `RowCellEngine.rowVisibleExp` 非空的模板（夹具生成器在 `tests/fixtures/EngineModels.java`），在 `_$1(ctx,false)`（`ExtCellSet.java:1188-1221`）前后 dump `RowCellEngine` 的 5 个表达式槽（visible/height/breakPage/breakColumn/adjustRowHeight） |

> ⚠ **编号说明**：总监提到的 `E4`/`E5` 在本文件里**没有对应编号**（`docs/format-contract.md` 附件 E 的
> E.4/E.5 是 M1 的记录格式项 `FileDataSetConfig`/`DataSetMetaData`，与引擎无关）。我把这条要求理解为
> "给 §9.2 里**尚无证据路径**的行补上探针配方"，已落在上表最后两行 + 上面的配方块。
> 若 `E4`/`E5` 指的是别处的编号，请把来源贴一下，我按真实编号补。

<details><summary>历史索引：交给人类决策的 top-3（已全部定案，见 §11）</summary>

| # | 当时的疑虑 | 定案（见 §11） |
|---|-----------|----------------|
| 1 | M2-08（`mergeSame`）在 P0 无法对拍？ | **保持 P0**：`engine_samemerge` 单页 ⇒ 页网格 ≡ 计算态网格，不必先做分页 |
| 2 | 相对引用解析的保真度？ | **逐行移植 `CSVariable.getCell()`**，不做偏移式简化 |
| 3 | `source` 是否照搬"双矩阵"？ | **照搬**：引擎矩阵只读、不随扩展增删（6/6 夹具实测） |

### 定案前的原始论证（留档，已消解）

1. **M2-08（`mergeSame`，标 P0）在 P0 无法对拍。** 它的唯一可观测位置是 `PageBuilder.createPages()`
   之后的页网格（`SimplePager.java:133,179`），而分页在 spec 里是 **P1**。二者要么一起进 P0（把 P1 的分页
   提前），要么把 M2-08 降级为 P1。**建议**：M2-08 降 P1，P0 只保证 `merge.rs` 的接口与 `mergeSame` 码值字典
   就位；否则 P0 的定义自相矛盾。

2. **相对引用解析要做到什么保真度。** `CSVariable.getCell()`（`CSVariable.java:176-235`）是"沿 head/sub 图
   找对应实例"，不是 `行±k` 的算术；它同时是 M2-06 的正确性与循环链（M2-07）的共同底座。逐行移植它
   （≈60 行 + `_$1.._$4` 与 `getCells` 家族）成本高。**决策**：P0 逐行移植，还是先做"同列/同带找实例"的
   简化版并接受 `engine_expr` 之外的表达式可能分叉？**建议**：逐行移植 —— `engine_expr` 的
   `(2,3)=B2`、`(3,3)=B3` 已经证明"偏移式解析"会挂。

3. **`source` 语义要不要照搬"双矩阵"。** §0.1 表明 `expected.json` 的 `source` 来自引擎矩阵，与计算矩阵
   **不同步**；照搬要求 Rust 维护两张矩阵（引擎矩阵不随扩展增删，只在坐标越界时返回 `None`）。若为省内存
   把 `source` 改成"查计算矩阵"，会与全部六个夹具的 `source` 段不符。**决策**：确认"双矩阵"是硬约束
   （我的读法与夹具一致），还是接受把 `source` 断言放宽。**建议**：保留双矩阵；`_$25` 在本批夹具里恒等于
   模板尺寸，实现成本很低（一张只读的模板坐标表 + 每格的 `srcCurrent` 状态）。

</details>

---

*（附：`crates/rpx-engine/src/extend/tests.rs:207` 的单测 `row_count_matches_every_fixture` 已把 V14 的六个
夹具数值钉死；实现扩展算法时不要动那组数值。）*

---

## 11. 总监定案（2026-10-12，附证据）

§10 的三项不确定项已定案，**不需再问用户** —— 都由本轮证据消解：

| # | 议题 | 决定 | 证据 |
|---|------|------|------|
| 1 | M2-08 是否降级 P1 | **保持 P0**（不降级） | `engine_samemerge.expected.json` 的 `pagination.page_count = 1` ⇒ 页网格 ≡ 计算态网格。故可先实现 `merge.rs` 的合并 pass、直接对拍 `merged` 段，**不必先做分页**。⛔ 前提写死：**合并夹具必须单页**；将来出现跨页的 `mergeSame` 夹具，必须改为逐页调用（产品调用点 `SimplePager.java:133,:179`） |
| 2 | 相对引用解析保真度 | **逐行移植 `CSVariable.getCell()`**（≈60 行），不做偏移式简化 | `engine_expr` 的 `(2,3)=B2` / `(3,3)=B3` 已证偏移式解析会挂；且它是 M2-06 与循环链 M2-07 的共同底座 |
| 3 | `source` 是否照搬双矩阵 | **照搬**（引擎矩阵只读、不随扩展增删） | 6/6 夹具实测 `source != null ⟺ r ≤ 模板行数 && c ≤ 模板列数`，越界 0 个、模板内缺 0 个；改成查计算矩阵会全表不符。实现成本低（一张只读模板坐标表 + 每格 `srcCurrent` 状态） |

### 11.1 §4.3 的缺陷已修复（与独立验证报告一致）

§4.3 指出 `extend.rs` 只实现了路径①。**已修复**：新增子模块
`crates/rpx-engine/src/extend/role.rs`，内含
`CellRole`（`Literal` / `Extender` / `PlainFormula`）+ `cell_role()`（对齐 `checkExtend:492-514`），
`ExtendMode::resolve()` 改为按角色三分支（`role.rs:113-131`）：

| 角色 | 规则 | 源码 |
|------|------|------|
| `Literal`（无表达式） | 不改写，保留声明值 | 两条路径的 `else` 分支 |
| `Extender` | 声明 16 → `topology_default`；17/18/19 不动 | `ExtExtendCell.java:315-329` |
| `PlainFormula` | **无条件 → 17** | `ExtNormalCell.java:539-541` |

新增单测 `a_plain_formula_cell_is_pinned_to_17_regardless_of_topology` 专门钉住
"heads 落在 `(cell00, cell00)` 的普通公式格仍须 17"这一条 —— 这是旧实现唯一会错的情形。

### 11.3 类型短路（第二轮独立验证发现，已修复）

第一轮验证指出 `checkExtend` **最开头**还有一句被省略的短路：

```java
// ExtNormalCell.checkExtend :494-496   —— 排在所有 extend 判断之前
if (type == -61 || type == -57 || type == -60) { return false; }
```

常量取自 `INormalCell.java:24-28`：`TYPE_GRAPH=-61` / `TYPE_SUBREPORT=-60` / `TYPE_BARCODE=-57`。
这句为真 ⇒ 该格**不会**被包成 `ExtExtendCell` ⇒ 退回 `ExtNormalCell` 规则（有表达式置 17）。
所以它必须**优先于**"声明 18/19 恒为扩展格"。

⚠ 这是**可构造但无夹具覆盖**的分叉 —— 当前 7 个夹具都没有这三种类型的格，
所以**漏掉它没有任何测试会失败**，这正是它危险的地方。构造方法：
`setCellType((byte)-57)` + `setExtendMode(18)` + 任意表达式 → Java 计算态 **17**，
只看声明的实现会给 **18**。

已修复：`extend/role.rs` 的 `EXTEND_EXCLUDED_CELL_TYPES` + `is_extend_excluded_type()`，
`cell_role()` 把它放在第一位；单测 `an_excluded_cell_type_is_never_an_extender`
（三种类型 × 声明 18/19 × 有/无表达式 + 对照组 `TYPE_NORMAL=-64` 不受影响）。

### 11.4 夹具口径测试去常量（第二轮独立验证发现，已修复）

原 `tests/units_contract.rs` 的 `expected_calc_rows` 是**转录**自 `expected.json` 的常量
（注释说"取自 expected.json"但运行时并未读取）→ 夹具改了而测试不跟着改会**静默通过**。

已重写为**零转录常量**（只留夹具名）：`template_rows` / `template_cols` /
`dataset.row_count` / `calculated.rows` 从 `expected.json` 现推，数据集行数从 `.csv` 现推，
扩展块高从计算态 `source` 段现推（= 非 16 格的 `row_merge` 最大值）。自带一个极简零依赖
JSON 取值器，形状不符一律 panic（不静默返回默认值）。

> **为什么块高不能从 `merged` 段推**：`engine_merge` 与 `engine_samemerge` 的 `merged`
> 都写 `row_merge=2`，但前者的**计算态**锚点 `row_merge=2`（模板合并区，块高 2）、后者是 1
> （`mergeSame` 只在分页后生效）。用 `source` 段才分得开 —— 单测
> `block_height_reflects_template_merges_not_merge_same` 把这两个夹具钉成对照。

### 11.2 口径修正（独立验证发现，已修复）

原 `calc_matrix_rows` 的文档声称返回"含 0 号头行的矩阵行数、与 `ReportDefine2` 第一参数同口径"，
且据此推断 `expected.json` 的 `calculated.rows` 需要 `+1` —— **两处都错**，6/6 夹具一次性证伪：

- `ReportDefine2.java:56` 是 `new Matrix(++nRow, ++nColumn)`，**第一参数本身就已是 `getRowCount()` 口径**；
- `ReportDefine.getRowCount()`（`:359-361`）与 `ExtCellSet.getRowCount()` 都是 `getRowSize() - 1`
  ⇒ 模板态与计算态**同一口径**，**不需要 +1**（`F ≡ calculated.rows`，不是 `F ≡ calculated.rows + 1`）。

已改名 `calc_matrix_rows` → **`calc_row_count`**（返回的是 `getRowCount()` 口径，不是矩阵行数），
文档写明 ⚠ `rpx_core::ReportDefine.rows` 是裸矩阵行数、必须先 `-1`，
并新增 `crates/rpx-engine/tests/units_contract.rs`：从 `.rpx`（rpx-core 解析）与 `.csv` 现推，
断言 `define.rows - 1 == 模板行数` 与行数公式，把这条换算钉死。

### 11.5 集合形状口径修正（2026-10-12，`engine_disp` 落地后定稿）

⛔ **写"能逐格断言 `calculated`"的基座（§12.1 第 1–2 步）之前必须先读本节** —— 这里的每一条
都会让"坐标缺失"被误判成断言失败，或者反过来把稀疏当稠密**静默放行**。
数据来源：9 个 `tests/fixtures/engine_*.expected.json`（本轮逐文件重算，不是引用旧文）。

| # | 口径 | 结论（可当契约） | 证据 |
|---|------|------------------|------|
| ① | `calculated.cells` 是**稀疏数组**，落盘判定是 `\|\|` | 只有 `value` **与** `disp_value` **双 null** 的格整条不落盘 ⇒ **「有显示值的空值格照样落盘」**。「坐标不在 `cells` 里」= 该格为空，**不是**断言失败；逐格比对**必须先按 `(r,c)` 建索引，不能按下标对位** | `CalcJson.java:51-53`；Rust 侧 `crates/rpx-engine/src/arena/calc_grid.rs` 的 `CalcCell::is_emitted()` 已按 `!value.is_null() \|\| disp_value.is_some()` 实现（含 2 个单测，其中一个就是下面的反例） |
| ② | **`disp_value` 不是死键**（推翻 2026-10-12 早先的"出现 0 次"结论） | 第 8 个夹具 `engine_disp`（计算态 6×4、落盘 11 格）里 **7 格带 `disp_value`**，其中 `(2,3)` 是 **`value=null` + `type="null"` + `disp_value="仅显示值"`** ⇒ 它**单独证伪**了"`value` 为 null 就整条不落盘"的旧模型。P0 **可断言** `disp_value`，不再是"只留槽位" | `tests/fixtures/engine_disp.expected.json` 的 `calculated.cells[2]`（`(2,3)`）；`CalcJson.java:71-73`（只有 `disp_value != String.valueOf(value)` 才写 ⇒ 它是独立覆盖项，不是 `value` 的格式化结果） |
| ②b | ⛔ **`engine_disp` 是唯一能测出 `\|\|` 与 `&&` 差别的夹具**（其余 8 个夹具 `disp_value` 全为 0 ⇒ 两种写法行为完全相同、测不出差别） | **防线已落地**：`crates/rpx-engine/tests/disp_assert.rs` 的 `null_value_cell_is_emitted_because_the_rule_is_or` 断言「存在 `value==null` 的落盘格、其原文 token 是 `null`、`disp_value` 非空」，并反向断言「不存在两值同时为 null 的落盘格」；该测试还先断言「夹具必须含 `value==null` 的格，否则本测试是永真式」⇒ **夹具若哪天不再含反例格，它会自己失败**。加载器侧 `CalcCell.disp_value` 由 `arena/calc_grid.rs` 持有、`tests/common/loader.rs` 读入。**保留要求**：任何新增的加载/落盘入口都必须透传 `disp_value`，否则这条唯一防线会被旁路 | `tests/disp_assert.rs`；`arena/calc_grid.rs::CalcCell::disp_value` / `is_emitted()` |
| ③ | `source` 是**可选键** + **计数已更正** | `source != null ⟺ r ≤ template_rows && c ≤ template_cols`（§0.1）。实测无 `source` 的落盘格数：`engine_ext` **42 格中 30 格无**、`engine_pager` 124 中 114、`engine_merge` 19 中 7、`engine_samemerge` 12 中 6、`engine_disp` 11 中 1；`expr/sum/types` **全有**。⚠ 旧文写的「`engine_ext` 24 格中 12 格无」是**夹具重排前**的数字，已作废（§0.1 同步改过） | `ExtCellSet.java:623-629`；9 夹具逐格重算 |
| ④ | `merged` 段**反向不成立** | 9 个夹具的 `merged` **全部非 `null`**（`engine_cycle` 的 `merged=null` 是因为它是错误型夹具）；但 `merged.cells` 非空只出现在 `engine_ext`(35，`col_merge=5`、**是 extendMode 19 放大列结构的痕迹，与 `mergeSame` 无关**)、`engine_merge`(12)、`engine_samemerge`(4) ⇒ **禁止用「`merged` 是否非空」筛 merge 夹具**，只能用「`mergeSame` 夹具自身的 `merged.cells`」 | `SimplePager.java:133,179`；9 夹具重算 |
| ⑤ | `Page.rows ≠ col1_values.len()` | `col1_values` **跳过第 1 列的空值** ⇒ 逐页断言必须**分别**对 `rows` 与 `col1_values`，不能互相替代。实测不等的有：`engine_sum` P1/P2 `8` vs `4`、`engine_expr` P1 `8` vs `4`、`engine_pager` P4 `4` vs `3`、`engine_samemerge` P1 `7` vs `6`、`engine_disp` P1 `6` vs `5`；相等的：`engine_ext/merge/types` | `CalcJson.java:121-132`；9 夹具逐页重算 |

**M2-14 类型面现状**：第 8 个夹具 `engine_types` 已把类型面补到
`string` / `int` / `list` / `short` / `byte` / `double` 等（`tests/fixtures/engine_types.expected.json`，
配套探针 `ProbeTypes.java` + `probe_types_out.txt`），`value.rs::type_name()` 已有对应单测。
⚠ **仍未闭合的只有 `"Short"`/`"Byte"` 这两个夹具级断言** —— Java 的 `CalcJson.typeName` 末尾有
`return o.getClass().getSimpleName();` 兜底（`CalcJson.java:252`），会把它们写成首字母大写的类名；
`value.rs` 模块文档已记该缺口与验证方法。**除此之外不要再说"类型面无覆盖"。**

---

## 12. 扩展算法的实现执行顺序清单（**动工前读本节**）

### 12.1 顺序清单

按序号做；每一步都给出"做完就能跑的验收"，**不要跳步**（第 5/6 步是 U1/U2 的落点，第 8 步是 P0 的核心）。

| # | 做什么 | 依赖 | 做完就能跑的验收 |
|---|--------|------|------------------|
| 1 | **两张矩阵**（`arena.rs`）：引擎矩阵（只读模板坐标）+ 计算矩阵（可变）；`CellId` 三态 `ROOT`/`NONE`/普通 | — | §0.1：`source != null ⟺ r ≤ 模板行 && c ≤ 模板列`，6/6 夹具 |
| 2 | **阶段 A 建格**（§3）：合并锚点"先铺满矩形、后续格按自身内容覆盖" | 1 | `engine_merge` 的 `(3,1)` 的 `source` 是它自己（非锚点） |
| 3 | **表达式切片的 `is_extend()`**（**正文见 §13**；§4.1 V25 是结论摘要）：AST 上的纯递归谓词 | `expr/*` 存在 | 单测：`ds1.select(x)`→true；`B2*2`→false；`sort(...)`/`to(...)`→true；`Constant(List)`→true；**`sum(ds1.select(qty))`→false**（`isDSFunction` 反例，⛔ 不许删这条） |
| 4 | **`cell_role` + `resolve`**（`extend/role.rs`）：`cell_type∈{-61,-57,-60}` 先短路 → 声明 17/18/19 → 有/无表达式 → 声明 16 且 `is_extend()` | 3 | `extend/tests.rs` 现有 3 个单测（含类型短路、`PlainFormula→17`） |
| 5 | **`find_left_head`/`find_top_head`**（§4.2 伪码；**行优先 `j` 升序**调用）：先 left 再 top，再 `resolve` | 1,2,4 | 对 `engine_ext` dump：`top_head == cell00` **12/12**；`left_head == cell00` **10/12**（例外 = `(3,2)(3,3)`→18 锚点 `(3,1)`）（V23） |
| 6 | **head 归一（§4.4）+ `_$5`/`_$6` 副作用**：`set_cell_*_head` 必须把本格登记进 head 的 sub 列表 | 5 | `cell00._$5` 成员数与 §6.3 表一致（engine_ext = 12） |
| 7 | **调度器（§5）**：`NeedCell{cell,state}`；19→18→17→special；`state ∈ {4,5}` 压栈换格、`1` 终止、`6`→special、`8`→`_$24` | 5,6 | `engine_sum`/`engine_expr`/`engine_pager` 的值全对（先不验 extend 结构） |
| 8 | **扩展本体**：**算法正文以 `docs/format-contract.md` 附件 I 为准**，本文件 §6 保留设计视角（判定式 + 易错点）。`extend_row` 与 `extend_col` **分别**实现，且 `expand_left_head` / `expand_top_head` 的施力点不同 | 1,6,7 | `engine_ext` 行数 8、列数 7、`sub_cells` 3 例全对（V14/V15/V16/V17）；`count==0` 走"不扩展 + 值置空"（§12.4 的 U4） |
| 9 | **`sub_cells` = `_$7.len()`**（§7）；`sub_values` 只在 `>1` 时落盘 | 8 | 与 §7 表逐行对 |
| 10 | **循环链（§5）**：`test_value` 两态抛 + `chain = reverse(stack)` + `"-" + thrower` | 7 | `engine_cycle` 的 `chain == "B2-C2-B2-C2"` |
| 11 | **收尾**：6 夹具逐格对拍（值 + `source` 段 + `sub_values`） | 全部 | 全绿；空数据集（U4）已由 `ExtExtendCell.java:376-384` 定案（空 `List` 不扩展，见 §12.4）|

**最容易做错的 5 件事（都是本轮实测踩实的，不是猜测）**

1. ⛔ **"公式格一律 ≠16" 不等于"所有格都 ≠16"**：`extensible≠16` 只对**带表达式**的格成立，字面量格保留
   声明值 16。忘记这点会把 **6 个夹具里 72 个 `extend=16` 的格**改错（逐夹具：`engine_sum` 27、
   `engine_expr` 15、`engine_merge` 10、`engine_ext` 9、`engine_pager` 7、`engine_samemerge` 4；
   `engine_cycle` 无网格）。（§4.3）
2. ⛔ **`extend_col` 的施力点是 `cell00._$5`，不是"锚点的 head 链成员"**（§6.3，U2）。写成"沿链给每个 head
   `addColMerge`"会给出 `cm=(4,1)=5`、`cm=(1,1)=1` —— 与夹具**完全相反**。
3. ⛔ **`extend_col` 必须先于 `extend_row`**（§5，V9）：顺序反了 19 锚点子格会落在第 4 行而不是第 8 行。
4. ⛔ **插行位置是行带之后 `endRow`**、量是 `rows*(count-1)`（不是 `count-1`）（§6.2，V14）。
5. ⛔ **`_$3`（左 head）不调 `sameRowTypeAs`**，`_$2`（上 head）调两次；且两者命中分支判的 extend 分别是
   18/19（§4.2，U1/V22）。给两函数写一份共用代码会同时踩两个坑。

### 12.2 探针与证据（可复现；用于复核 U1/U2）

本节的 U1/U2 结论**不是读源码推的**，是拿产品引擎跑出来的。探针是**一次性 scratch 文件，刻意放在 rpx 仓库外**
（不污染交付物）：

```
源码   F:/project/report/re_work/tmp_probe/heads/ProbeHeads.java     （package rpx.fixtures，复用 EngineModels/CalcProbe）
编译   cd F:/project/report/rpx/tests/fixtures
       "D:/Tools/jdk-21.0.2/bin/javac.exe" --release 8 -Xlint:none \
         -cp "F:/project/report/reportlite/lib/*" *.java \
         "F:/project/report/re_work/tmp_probe/heads/ProbeHeads.java" -d "F:/project/report/re_work/tmp_probe/out"
运行   cd F:/project/report/rpx/tests/fixtures      # cwd 必须在这里，FileDataSetFactory 要读 engine_ext.csv
       "F:/project/report/common/jre/bin/java.exe" \
         -cp "F:/project/report/reportlite/lib/*;F:/project/report/re_work/tmp_probe/out" rpx.fixtures.ProbeHeads
```

探针用到的 **API 与打印项**（将来要复现/扩展就照这个）：

| 目的 | 调用 | 说明 |
|------|------|------|
| 取某格的"源格对象" | `ecs.getSource(r,c)` | 读引擎矩阵；`(0,0)` 返回 `cell00` |
| 取 head 指针 | `src.getCellLeftHead()` / `src.getCellTopHead()` | 逐节走，`System.identityHashCode` 或 `==` 判同一性 |
| 读 `cell00` 的子格列表 | 反射 `ExtExtendCell` 的私有字段 `_$5`(topSubs) / `_$6`(leftSubs) | `setAccessible(true)` + `ExtList.size()/get(int)`；**没有公开 API** |
| 引用相等断言 | `ecs.getSource(0,0) == cell00`；`src.getCellTopHead() == cell00` | 本次全部为 `true` |

**实测输出（`engine_ext`，2026-10-12 本机运行）**

```
[A] grid 8x7
[A] ROOT src(0,0)=ExtExtendCell id=`0                       ← cell00 = _$10，与 getSource(0,0) 同一对象
[A] ROOT topSubs n=12: (1,1)cm=5 (1,6)cm=1 (1,7)cm=1 (2,1)cm=5 (2,6)cm=1 (2,7)cm=1
                        (3,1)cm=5 (3,6)cm=1 (3,7)cm=1 (8,1)cm=1ext=19 (8,6)cm=1 (8,7)cm=1
[A]   (1,1) src=(1,1) cm=5 rm=1 ext=16   topHead==ROOT? true  leftHead==ROOT? true  inROOT._$5? true
[A]   (2,1) src=(2,1) cm=5 rm=1 ext=17   topHead==ROOT? true  leftHead==ROOT? true  inROOT._$5? true
[A]   (3,1) src=(3,1) cm=5 rm=1 ext=18   topHead==ROOT? true  leftHead==ROOT? true  inROOT._$5? true
[A]   (4,1) src=(8,1) cm=1 rm=1 ext=19   topHead==ROOT? true  leftHead==ROOT? true  inROOT._$5? true   ← 19 锚点
[A]   (3,2) src=(3,6) cm=1 rm=1 ext=16   topHead==ROOT? true  leftHead==(3,1)          ← 反例：不受扩列影响
```

> `(4,1)` 的 `src=(8,1)` 是**计算态位置**；而 `source.src_row/src_col` 仍是模板 `(4,1)`
> （`getSourceRow/getSourceCol` = `engine.sourceRow/sourceCol`，夹具实测 `src_row=4, src_col=1`，§0.1）。

四个变体（`engine_ext` / 2×3 / 3×3 / 18 在 19 上方）的 `cell00._$5` 与 `cm` 结果见 §6.3；四条都符合
"受害格 = `cell00._$5` 中 `containCol(anchor)` 的成员"这一条判定式。

### 12.3 循环链证据（可复现；用于复核 U3）

直接跑现成的 `ProbeP0`（`tests/fixtures/ProbeP0.java`，S3/S4/S5 三个场景）：

```
cd F:/project/report/rpx/tests/fixtures
"D:/Tools/jdk-21.0.2/bin/java.exe" --release 8 -Xlint:none -cp "F:/project/report/reportlite/lib/*" *.java -d out
"F:/project/report/common/jre/bin/java.exe" -Dfile.encoding=UTF-8 -cp "F:/project/report/reportlite/lib/*;out" rpx.fixtures.ProbeP0
```

实测（2026-10-12）：

```
== S3 cycleExpr  B2=C2, C2=B2   → chain B2-C2-B2-C2     （与 engine_cycle.expected.json 一致）
== S4 selfExpr   B2=B2          → chain B2-B2-B2
== S5 cycleLeft  leftHead(B2)=C2, leftHead(C2)=B2 → chain C2-B2-C2
```

### 12.4 尚未验证（与 §9.2 同源；**别当结论**）

- ~~**U4 空数据集（0 行）**~~ → ✅ **已解（源码级）**：`ExtExtendCell.calculate:376-384` 在 `(result = expression.calculate(ctx)) instanceof List` 之后立刻判 `if (len == 0) { this.setValue(null); return; }` ——
  **空 `List` ⇒ 闸门在扩展之前就返回**：不插行、锚点置**空值**、无子格；所以「行数 = 模板行数」不是我们的**约定**，而是产品行为。
  另外 `count` 的口径是「锚点表达式返回的 `List.size()`」（`ExtExtendCell.java:379-380`、`:425-430`），空集就是 `0` ⇒ 它与"数据集 0 行"在 `ds1.select(...)` 形态下等价，但**带条件的 `select` 会不等**（`spec-m2.md` 的「`count` 确切语义」条目已记）。
  ⚠ **`count=0` 不许进 `extend_row`/`extend_col`**：`new ExtCell[count]` 之后**无条件**写 `extCells[0] = cell`
  （`ExtCellSet.java:994-995` / `:1094-1095`）⇒ 传 0 会 AIOOBE。产品靠上游那句 `return` 兜住；
  **Rust 侧必须自己也兜**（我们的调用方是我们自己的代码，不能继承"上游保证 `count≥1`"的假设）：
  `count == 0` ⇒ 不扩展 + 该格值置空（镜像产品），**并且绝不 panic**。
  ⇒ **U4 不需要新夹具，也不需要声明"P0 不含空数据集"。**
  （`extend.rs:174-178` 的 `dataset_rows.max(1)` 注释里那句"这是约定不是实测结论"可以改成直引 `ExtExtendCell.java:376-384`；该文件归总监，我不改。）
- **U5/U6/U7**：属性码表达式调度、`reflush`/页面态、`_$2()` 无参方法 —— 见 §9.2。
- **U8**：§4.4 第 3 步 `topHead` 分支缺 `else` 的反编译可疑点 —— 见 §9.2（用 `javap -c` 看原始控制流）。

**P0 开工不需要人类再拍板**：§10 的三项已由 §11 定案，§9.2 的 U1/U2/U3 已由本节证据消解。

---

## 13. 表达式切片契约（词法 / 语法 / `is_extend`）

> 本节是**产品契约**（写的是 ReportLite 的真实语义），与"本仓用哪种解析实现"无关；
> 本仓实现与产品的**已知偏差**单列在 **§13.6**，不进契约正文。
> 落点：`crates/rpx-engine/src/expr/`（词法 + Pratt + `is_extend`；**求值不在本切片**）。

### 13.0 谁需要它（为什么它排在扩展算法之前）

```
checkExtend（ExtNormalCell.java:492-514）
   └─ 声明 extendMode == 16 ⇒ 问 ((Expression)expression).isExtend()   ← 唯一的判定入口
        └─ Expression.isExtend() = home.isExtended()                   （Expression.java:561-566）
             └─ 决定该格是否被包成 ExtExtendCell（:749-755）
                  └─ 决定 16→17/18/19 的拓扑改写（§4.3）
                       └─ 决定计算矩阵的行列结构（§6）
```

⛔ **`isExtend()` 在整个产品里只有一个调用点**：`reportlite/.../engine/ExtNormalCell.java:510`
（全 `re_work/src` grep 确认；另一处 `report/.../ExtNormalCell.java:514` 属姊妹产品，不在本项目范围）。
这条很关键：它意味着这个谓词**只在"`checkExtend` 那一次构造出来的表达式"上被问**，而那次构造是

```java
// ExtNormalCell.java:498-504
this.engine.expression = MacroResolver2.replaceMacros((String)this.engine.expression , ctx);
this.engine.expression = new Expression(this.engine.cs, null, ctx, (String)this.engine.expression, false);
//                                                      ^^^^  (IReport cs, DataSet ds, Context ctx, String, boolean)
```

⇒ **`ds` 参数恒为 `null`**、`opt=false`（不做常量折叠）。这两条直接决定了 §13.4 的名字解析规则。

### 13.1 token 集与识别优先级

**识别顺序（先到先得，产品只有这一条扫描循环 `Expression.create:289-456`）**

| 步 | 判据 | 产出 | 产品出处 |
|----|------|------|----------|
| 1 | `Character.isWhitespace(c)` | 跳过 | `:296-299` |
| 2 | `c` 是 `"` 或 `'` | **字符串字面量**（转义 `\X` 跳过两个字符） | `:115-124`；`Sentence.scanQuotation:77-96` |
| 3 | `c` 在 19 个分隔符里 | **运算符/标点**（**两字符优先**：`==` `!=` `>=` `<=` `&&` `\|\|`） | `:301-434` 的 `switch` |
| 4 | 其余 | **原子**（`scanId`：一直吃到下一个分隔符或空白） | `:70-78` |

⛔ **分隔符集合里没有 `.`**（`Expression.java:74` 的 `&&` 串里没有 `46`）
⇒ `ds1.select` 是**一个**原子，不是三个 —— 这正是 `:174` 能用 `id.indexOf('.')` 切出数据集名的前提。
⛔ **分隔符集合里也没有 `;`**：`;` 是**参数级**分隔符（`ParamParser.java:66-79` 的层级 `'\0' → ';' → ',' → ':'`），
参数串**先按它切分、之后**才逐段交给 Expression 解析 ⇒ **`;` 永远不会作为一个 token 出现在表达式内部**。

> ⚠ **参数层的"取哪一级"是固定次序，不是"哪个先出现"**：`ParamParser._$1:48-57` 是
> `level=';'` → 不含则 `level=','` → 仍不含则 `level=':'` → **仍不含才回落成叶子**，
> 所以 `getType()` = **最粗的那一级**，固定次序 `;` > `,` > `:`。
> ⇒ `a:b,c` 的 `getType()` 是 **`','`**（不是 `':'`），`a,b:c` 的也是 `','`；只有**整串没有 `,` 也没有 `;`**
> 时，`:` 才会成为 `getType()`。
> 这条直接决定 `ValueList.isExtended()`（§13.3）—— `[a:b,c]` 是 **`true`**，而 `[a:b]` 是 `false`。
> **写成"第一个出现的分隔符"是错的**（本仓代码 review 时发现过这个措辞，代码本身是对的、注释反了）。

**原子内部的分类顺序**（照抄 `Expression.createNode:105-240`，顺序本身是契约）

| 步 | 形态 | 落成 | 出处 | ⚠ 踩点 |
|----|------|------|------|--------|
| 1 | `"…"` / `'…'` | `Constant(字符串)` | `:115-124` | |
| 2 | `{…}` | `Select`（**内容进 `param`，不是 `left`**） | `:125-135` | 见 §13.6 ① |
| 3 | `[…]` | `Move` / `DSOffset` / `ValueList`（按"前一个节点是不是 `CSVariable`"三分） | `:136-155` | |
| 4 | 关键字 `and`/`or`/`not`/`in`（**大小写不敏感**） | `And`/`Or`/`Not`/`In` | `:157-168` | |
| 5 | `$` + `[…]` | `Constant(去转义后的串)` | `:169-172` | 是**字符串常量**，不是格引用 |
| 6 | `前缀.名`（前缀是已注册数据集） | DS 函数 / `DSVariable` | `:173-177` | |
| 7 | `@名` / `@@名` / `@` | `Constant(参数值)` / `Constant(宏参数值)` / `CSCurrent` | `:178-192` | 值是 `List` ⇒ `is_extend` 为真，见 §13.3 |
| 8 | `$格号` | `CSVariable(cell, saved=true)` | `:193-200` | `$` 只剥**一个**（`$B$2` → 报错，附件 G.1） |
| 9 | `格号`（`cs.getSource(id) != null`） | `CSVariable(cell, saved=false)` | `:201-203` | ⛔ **这一步排在"函数调用"之前** ⇒ `B2(1)` 是 `Offset(CSVariable)`，**不是**调用；⛔ **也排在"带点名"之后**（第 6 步）⇒ 判序是「**带点名 → `@`/`$` → 格号 → 数据集列**」，不是"格号优先" |
| 10 | `名(` | 函数（**先查普通函数表**） | `:210-228` | 见 §13.4 |
| 11 | `Variant2.parse` 能解析 | `Constant(字面量)` | `:235-239` | `null`/`true`/`false`/`0x…`/int/long/double；日期字面量走 `parseDate`（本切片不做） |
| 12 | 兜底 | `UnknownSymbol(名)` | `:236-238` | `is_extend` = **false** |

**优先级表（= 绑定力）**。常量在 `Node.java:13-44`，各类的赋值点逐条列在右列；
**未赋值的类用 `Node.java:54` 的字段初值 `19`**。

| 优先级 | 记号 | 类 | 赋值点 | 类别 |
|-------|------|----|--------|------|
| 1 | `,` | `Comma` | `Comma.java:16` | 中缀 |
| 2 | `=`（单个） | `Evaluate` | `Evaluate.java:18` | 中缀 |
| 3 / 7 / 8 / 9 / 12 / 15 | — | `PRI_ASS/BOR/BXOR/BAND/SHIFT/NEW` | **reportlite 的表达式包里没有任何类用这六档**（`priority = ` 全量取值只有 1,2,4,5,6,10,11,13,14,16,17,18,19）⇒ **本切片不实现位运算/移位** | — |
| 4 | `:` | `Link` | `Link.java:25` | 中缀，**覆写 `isExtended()=false`** |
| 5 | `\|\|` / `or` | `Or` | `Or.java:17` | 中缀 |
| 6 | `&&` / `and` | `And` | `And.java:18` | 中缀 |
| 10 | `==` / `in` / `!=` | `Equals` / `In` / `NotEquals` | `Equals.java:17`、`In.java:18`、`NotEquals.java:17` | 中缀 |
| 11 | `>` / `>=` / `<` / `<=` | `Greater` / `NotSmaller` / `Smaller` / `NotGreater` | 各 `:17` | 中缀 |
| 13 | 二元 `+` / `-` | `Add` / `Subtract` | `Add.java:23`、`Subtract.java:22` | 中缀 |
| 14 | `*` / `/` / `%` | `Multiply` / `Divide` / `Mod` | 各 `:21` | 中缀 |
| 16 | `!` / `not` / `&`（单） | `Not` / `Address` | `Not.java:17`、`Address.java:24` | **前缀** |
| 17 | `[…]` / `(…)` 后缀 / `{…}` / `lefthead` / `tophead` | `Move` / `Offset` / `DSOffset` / `Select` / `LeftHead` / `TopHead` | `Move.java:54`、`Offset.java:18`、`DSOffset.java:18`、`Select.java:26`、`LeftHead.java:22`、`TopHead.java:22` | **后缀** |
| 16 | `mcorder` | `McOrder` | `McOrder.java:110` | 原子（函数类里唯一把优先级降到 16 的） |
| 18 | 一元 `-` / `+` | `Negative` / `Plus` | `Negative.java:21`、`Plus.java:18` | **前缀** |
| 19 | 字面量 / 格引用 / 数据集列 / 参数 / 未知符号 / 绝大多数函数 | `Constant` / `CSVariable` / `DSVariable` / `ParamNode` / `UnknownSymbol` / `Function` | 未赋值 ⇒ `Node.java:54` | 原子 |

**结合性**：产品的 `create()` 沿右脊下探的条件是**严格小于**（`Expression.java:442`
`right.getPriority() < node.getPriority()`）⇒ **所有中缀运算符左结合**，
等价于 Pratt 的 `(lbp, rbp) = (pri, pri + 1)`。括号用 `setInBrackets` 加 `inBrackets*20`
（`Node.java:56-58`）实现隔离，等价于 Pratt 的"前缀位置遇到 `(` 即递归"。

**两条可推出来的树形（不是约定，是从上面那张表算出来的）**

| 表达式 | 树形 | 为什么 |
|--------|------|--------|
| `not B2[1]` | `not ( B2[1] )` | 前缀 `not`=16，后缀 `[`=17 ≥ 16 ⇒ 后缀先绑 |
| `-B2[1]` | `( -B2 )[1]` | 前缀 `-`=18 > 后缀 17 ⇒ 后缀被**前缀整体**吸收的外面 |

### 13.2 节点清单（AST ↔ 产品类）

| 本仓 AST | 产品类（`extends`） | `priority` | `is_extend` 的**自身**规则 |
|----------|--------------------|-----------|------------------------------|
| `Const(值)` | `Constant extends Node`（`Constant.java:11`） | 19 | `值 instanceof java.util.List`（`:22-24`） |
| `CellRef{id, absolute}` | `CSVariable extends Node`（`:19`） | 19 | **恒 `false`**（`:379-381`） |
| `Param{name, is_macro}` | `ParamNode extends Constant`（`ParamNode.java:9`）/ `Constant`（`Expression.java:183-189,232-234`） | 19 | 继承 `Constant` ⇒ **值是 `List` 时为 `true`** |
| `CurrentCell` | `CSCurrent extends Function` | 19 | `Function` 未覆写 ⇒ `false` |
| `DsColumn{ds,col}` | `DSVariable extends Node`（`:16`） | 19 | `false` |
| `Unknown(名)` | `UnknownSymbol extends Node`（`:15`） | 19 | `false`（但 `isDSFunction()` 是 **`true`**，`:39-41`） |
| `ValueList{kind,items}` | `ValueList extends Function`（`:22`） | 19 | `param == null \|\| param.getType() != ':'`（`:30-32`） |
| `Select{arg}` | `Select extends SpecialFunction`（`:22`） | 17 | **只看 `left`（前一个操作数）**，不看 `{…}` 内容（§13.6 ①） |
| `Index{target}` | `Move` / `DSOffset` / `Offset`，都 `extends SpecialFunction` | 17 | `left.isExtended()`（= `target`） |
| `Call{kind,…}` | `Function`（`function/*`）或 `DSFunction extends Function`（`function/dsfunction/*`） | 19（`lefthead`/`tophead`=17、`mcorder`=16） | **只看函数类别，不看参数**（`Function` 未覆写 `isExtended`） |
| `Unary{op,rhs}` | `Not` / `Negative` / `Plus` / `Address`，都 `extends Operator`，**只挂 `right`** | 16 / 18 | `Operator.isExtended() = left \|\| right`（`Operator.java:51-56`） |
| `Binary{op,lhs,rhs}` | `Operator` 的 18 个子类（`operator/*`） | 见 §13.1 | 同上；**唯一例外是 `Link`（`:`）覆写成 `false`**（`operator/Link.java:100-102`） |

> ⛔ **`setLeft`/`setRight` 在 `Node` 上是空实现**（`Node.java:64-76`）：只有 `Operator`（left+right）与
> `SpecialFunction`（**只有 left**）真的存下来。⇒ 产品在某些相邻无运算符的写法上会**静默丢掉一个操作数**。
> 本仓用严格的二叉树表达同样的树形，该畸形不成立 ⇒ 影响面见 §13.5。

### 13.3 `is_extend()` 真值表

**三条传播规则（唯一的传播机制，其余全靠它们）**

| 规则 | 内容 | 出处 |
|------|------|------|
| R1 | `Operator`：**左 \|\| 右** | `Operator.java:51-56` |
| R2 | `SpecialFunction`：**左**（`super` 是 `Function`，未覆写 ⇒ 恒 `false`） | `SpecialFunction.java:27-32` |
| R3 | `Function` / `DSFunction`：**自身**，**不向参数透传** | 两处都没有 `isExtended` 重写（全量 grep 只有 14 处） |

**逐节点真值表**（全量 grep `isExtended` 的 14 个重写点，无遗漏）

| 节点 | `is_extend` | 依据 |
|------|-------------|------|
| `Constant(值)` | `值 instanceof List` | `Constant.java:22-24` |
| `ParamNode` / `@名` 常量 | 同上（**参数值是 `List` ⇒ 真**） | 继承 `Constant` |
| `CSVariable`（`B2`/`$B2`） | `false` | `CSVariable.java:379-381` |
| `DSVariable`（`ds1.qty`） | `false` | 无重写 ⇒ `Node.java:82-84` |
| `UnknownSymbol` | `false` | 无重写 |
| `ValueList`（`[…]` / `list(…)`） | `param==null \|\| getType() != ':'` ⇒ **`[…]`、`[1,2]`、`[a,b:c]`、`list(1,2)` 为真；`[1:3]`、`list(1:3)` 为假** | `ValueList.java:30-32` |
| `Select`（`{…}`） | 只看**前一个操作数**；`{…}` 在表达式开头 ⇒ `false` | `SpecialFunction.java:27-32` |
| `Move`/`DSOffset`/`Offset`（`x[…]` / `x(…)`） | 只看 `left` | 同上 |
| `Link`（`:`） | **`false`**（`Operator` 里唯一的覆写） | `operator/Link.java:100-102` |
| 其余全部运算符 | `left \|\| right` | `Operator.java:51-56` |
| 函数：`sort` / `sublist` / `to` / `split` | **`true`** | `function/Sort.java:19`、`SubList.java:18`、`To.java:17`、`string/SplitString.java:18` |
| DS 函数：`select` / `group` / `filtercross` / `plot` | **`true`** | `dsfunction/DSSelect.java:23`、`DSGroup.java:23`、`DSFilterCross.java:38`、`DSPlot.java:21` |
| **其余 120 个普通函数 + 11 个 DS 函数**（含 `sum`/`avg`/`max`/`min`/`count`/`field`/…） | **`false`** | 无重写 ⇒ `Node.java:82-84` |

**⛔ 头号回归（`isDSFunction` 反例，务必保留这条单测）**

```
sum(ds1.select(qty))
   外层 Sum（普通函数，Function.java:17-18 未重写 isExtended） ⇒ 恒 false
   内层 DSSelect（dsfunction/DSSelect.java:23 return true）      ⇒ true
   ⇒ isExtend() = false        （R3：不透传）
   ⇒ isDSFunction() = true     （Function.java:55-57 向参数透传）
```

两条链的**传播规则相反**，所以 `is_extend()` **不能**用"是不是 DS 函数"近似。
这条同时是夹具 `engine_sum` 的 `(2,6)` 实测：它没有成为扩展格，落 `PlainFormula → 17`。

**三条可直接推出来的结论**（写实现时照抄，别重新推）

1. `ds1.select(region)`、`ds1.select(qty)` ⇒ **真**（夹具里唯二的扩展源）；
2. `B2*2`、`$B2*3`、`B2+$B2`、`6*1` ⇒ **假**（运算符两侧都不是扩展源）；
3. `sum(B2:B4)`、`ds1.sum(qty)`、`ds2.count()`、`plot(x)` ⇒ **假**
   （`Link` 恒假；`DSSum`/`DSCount` 未重写；裸 `plot` 解析成**普通** `Plot`，见 §13.4）。

### 13.4 函数名表（`FunctionLib`）

两张表**互不覆盖**，登记名字全小写，查表是 `HashMap.containsKey` ⇒ **大小写敏感**
（`FunctionLib.java:64-70`）。共 **15 个 DS 函数 + 124 个普通函数**，本仓已全量转录
（`src/expr/names.rs`；**全量转录不是装饰** —— 漏一个名字它就退化成 `UnknownSymbol`，`is_extend` 静默变假）。

**裸调用 vs `ds.` 限定的解析顺序**（`Expression.java:210-228`）

| 步 | 条件 | 结果 |
|----|------|------|
| 1 | `FunctionLib.isFunName(名)`（普通表 `_$4`） | **普通函数**（命中即停） |
| 2 | 只有第 1 步不中、**且 `ds != null`** 才回退 `FunctionLib.isDSFunName(名)`（`_$3`） | DS 函数 |

⇒ **6 个"双表"名字在裸调用时一律解析成普通函数**：`avg` / `count` / `max` / `min` / `plot` / `sum`。
所以 `plot(x)` = 普通 `Plot`（**假**），只有 `ds1.plot(x)` = `DSPlot`（**真**）。

⇒ **第 2 步在本项目不可达**：`checkExtend` 构造表达式时 `ds` 写死 `null`（`ExtNormalCell.java:498-504`，见 §13.0）
⇒ **裸 `select(qty)` / `group(qty)` / `filtercross(qty)` 永远不会成为 DS 函数**，
它们落成 `UnknownSymbol`（**假**）。写"按名字猜 DS 函数"的实现会把这一类判错。

**优先级例外**：`lefthead` / `tophead` 的函数节点是 **17**、`mcorder` 是 **16**，
其余函数走默认 19（见 §13.1 表末三行）。

### 13.5 目前不支持 / 未核实的写法（诚实清单）

⛔ 本节列的每一条都**不许当结论外推**；有 ⚠ 的给了具体验证方法。

| # | 写法 | 产品行为 | 本切片 | 影响面 | 怎么补 |
|---|------|---------|--------|--------|--------|
| 1 | 裸 `select(...)`/`group(...)`/`filtercross(...)` | `ds==null` ⇒ `UnknownSymbol` + `Offset` | **同**（已实现，不是缺口） | `is_extend` 假 ⇒ 该格落 `PlainFormula→17` | 已闭合（§13.4） |
| 2 | `@名` / `@@名` 的**参数值** | 值是 `java.util.List` ⇒ `is_extend()==true` ⇒ 该格成为**扩展格** ⇒ 拓扑改写成 18 | ⚠ 取 `false`（拿不到参数值） | **真实分叉**：多选参数报表的扩展行数会算错；7+2 个夹具**零覆盖** | 需要一个"参数值查询"入口（`ParseEnv` 之类）：`is_extend(env, src)`。验证配方见下 |
| 3 | `ctx.getDataSet(前缀) != null` 校验 | 不存在 ⇒ 整串退化成 `UnknownSymbol` | ⚠ 不校验，形态像就算 DS 函数 | 拼错的数据集名会被当成 DS 函数 | 同 #2 的 env 入口 |
| 4 | `cs.getSource(格号) != null` 校验 | 不存在 ⇒ 不是 `CSVariable` | ⚠ 按**形态**（字母+数字）判定 | 列名形如 `qty1`/`amount2` 会被误判成格引用（`is_extend` 两边都假，不炸测试，但求值期必错） | 同 #2 的 env 入口 |
| 5 | 嵌套 `IParam` 树（`;` 里套 `,` 里套 `:`） | `ParamParser` 递归建树（`:35-64`） | 只建模**顶层一层** + 顶层类别 | `is_extend` **不受影响**（`ValueList` 只看顶层 `getType()`）；求值期受影响 | 定稿求值切片按 `ParamParser` 逐层重建 |
| 6 | `{…}` 出现在非前缀位置（`B2:B4{cond}`） | `Select` 挂到前一个操作数上 | 只支持前缀位置的 `{…}` | 该形态会报"不支持"而不是错算 | 见 §13.6 ① |
| 7 | `$[…]` | `Constant(去转义后的串)` | ⚠ 当成格引用解析 | `is_extend` 两边都假；求值期会取错值 | 见 §13.6 ③ |
| 8 | `Variant2.parseDate`（日期字面量） | `new DateFormat…` 解析失败时返回原文 | 不做 ⇒ 落 `UnknownSymbol` | `is_extend` 两边都假 | 需 `DateFactory2` 等价实现 + 夹具；当前无夹具 |
| 9 | `"Short"`/`"Byte"` 的 `typeName` 兜底 | `CalcJson.java:252` 返回类名 | `value.rs` 未加变体 | 只影响落盘字符串 | 见 §11.5 末段 |
| 10 | 字符串反转义的完整规则 | 产品走 `Escape.removeEscAndQuote`（本轮未读） | 只做 `\n\r\t` 与 `\X→X` | **不影响 `is_extend`**（谓词只看树形） | 读 `Escape` 后补；需要含引号/逗号的夹具 |
| 11 | 位运算 / 移位（优先级 3/7/8/9/12/15） | reportlite 表达式包里**无任何类实现** | 不实现（正确） | 无 | 已闭合（§13.1 表） |
| 12 | 相邻无运算符（`sum(x) B2`） | `setLeft` 在 `Node` 上是空实现 ⇒ 静默丢一个操作数 | 严格二叉树 ⇒ 直接报错 | 属产品畸形写法 | 不必实现（报错比静默丢弃安全） |
| 13 | 求值（`calculate`） | — | **本轮不做**，`expr::evaluate()` 是结构化错误出口 | 依赖 head 图与调度器 | §12.1 第 7 步 |

**#2 的验证配方**（一旦要闭合，必须在产品上跑）：造一个报表参数其值为一个数组（多选参数），
该参数格里写 `@参数名`，观察计算态 `source.extend` 是 18（可扩展）还是 17（`PlainFormula`）。

### 13.6 本仓实现与产品的**已知偏差**（不在契约正文里）

| # | 偏差 | 位置 | 后果 | 修法 |
|---|------|------|------|------|
| ① | `Select` 的 `is_extend()` 读了 **`{…}` 的内容**，产品读的是 **`left`（前一个操作数）** | `src/expr/ast.rs::Expr::is_extend` 的 `Select` 分支 | `{ds1.select(x)}` 本仓给 `true`、产品给 **`false`** | 改成"看 `left`"；并把 `{…}` 降为**后缀位置**才合法（支持 `B2:B4{cond}`） |
| ② | `@名`/`@@名` 的参数值（尤其 **List**）不可见，一律取 `false` | `src/expr/parser.rs` 的 `@` 分支 + `ast.rs` 的 `Param` 分支 | 多选参数报表会把扩展格判成 `PlainFormula`（行数算少） | 引入参数值查询入口；见 §13.5 #2 |
| ③ | `$[…]` 被当成格引用 | `src/expr/parser.rs` 的 `$` 分支 | `is_extend` 不受影响；求值期取错值 | 改成 `Constant(去转义、去空白后的串)`（`Expression.java:169-172`） |
| ④ | 格号按**形态**判定（无 `cs.getSource` 校验） | `src/expr/classify.rs::is_cell_id` | `qty1` 这类列名被误判成格引用 | 引入 `cs` 存在性查询；见 §13.5 #4 |

**为什么把这四条单列而不是写进 §13.3 的真值表**：真值表是**产品契约**，实现有偏差就改实现；
把偏差混进契约会让下一个人拿"实现的行为"当"产品的行为"，那正是本项目被驳回两次的失败模式。

---

*（附：本节由 `crates/rpx-engine/src/expr/` 的实现反向校对过一遍 —— 上面每条 `file:line` 都是
`F:/project/report/re_work/src/reportlite/03-model/com/raqsoft/reportl/model/expression/` 下的实际行号。）*
