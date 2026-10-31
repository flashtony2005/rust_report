# re_work Java 参考源码地图（2026-10-13）

> 本文只做一件事：把 `re_work/`（Java 反编译参考）里**与 Rust 移植有关**的结构讲清楚，  
> 并给出 `file:line` 坐标，供 M2-08 / M2-21 的实现直接引用。  
> 本文**不是**算法规格 —— 规格在 `docs/m2-engine-design.md` 与 `docs/format-contract.md`；  
> 本文是「去 Java 里哪儿看」的索引。

---

## 1. 来源与可信度边界

| 项         | 值                                                                                                 |
| --------- | ------------------------------------------------------------------------------------------------- |
| 反编译器      | CFR 0.152                                                                                         |
| 反编译所用 JRE | 产品自带 JRE **1.8.0_151**（`Contents/Home/` 布局）                                                       |
| 语料规模      | **5646 个 `.java`，1,054,227 行**（`find src -name '*.java'`，2026-10-13 实测）                           |
| 分段        | 4 个 JAR：`reportlite.jar` / `report-20260106.jar` / `nlr-20260106.jar` / `esproc-bin-20260106.jar` |
| 索引        | `re_work/INDEX.md`（与上面统计一致）                                                                       |

**可信度边界（三条硬约束）**：

1. **反编译不是源码**。CFR 会重排、会丢泛型、会把 `try/finally` 拉直。  
   **只信「语义可判读」的部分**；凡是看到 `goto` 化的 `while`、重复的 `_$N` 重载、  
   或明显被常量折叠过的分支，都要回到「对拍」夹具去验证行为，不能只读代码下结论。
2. **混淆名不可当语义**。`_$26`、`IllIllllllllllll`、`lIlIlllIIllllIII` 是混淆后的名字，  
   它们**只表示「同一个槽位」**，不表示任何含义。含义靠调用点反推（本文第 3、4 节做的就是这件事）。
3. **同一逻辑在多个 root 里有副本**。`reportlite` 与 `report` 两个 root 有**几乎逐行相同**的  
   `SimplePager` / `CalcSubRpt`（见第 6.1 节）。**移植目标只有 `reportlite`**，  
   另一份是产品线里的孪生体，不要看错。

---

## 2. 目录骨架：5 个 root，只有 `reportlite` 是目标

`re_work/src/` 下 5 个 root（`find src -name '*.java' | wc -l` 分桶）：

| root             | 文件数  | 是什么                                             | 是否移植目标    |
| ---------------- | ---- | ----------------------------------------------- | --------- |
| `reportlite`     | 1333 | 包名 `com.raqsoft.reportl` —— **ReportLite 引擎本体** | ✅ **是**   |
| `reportlite-cli` | 10   | ReportLite 命令行外壳                                | 否（CLI 自研） |
| `report`         | 2292 | 老版报表（`com.raqsoft.report`）—— 与 reportlite 是孪生   | ❌ 否（对照用）  |
| `esproc`         | 1923 | SPL 集算引擎                                        | ❌ 否       |
| `nlr`            | 88   | 其它                                              | ❌ 否       |

`reportlite` 内部按 `NN-` 前缀分层（数字是构建期拼接序）：

```
src/reportlite/
├── 03-model/com/raqsoft/reportl/model/          ← 模型 + 引擎（**移植主战场**）
│   ├── engine/          ExtCellSet / SimplePager / Ext*Cell / NormalCellEngine …
│   ├── expression/      Expression / 函数库（FunctionLib、CalcSubRpt …）
│   └── NormalCell.java  模型层单元格（含 merge* 原始字段）
├── 05-core-runtime/com/raqsoft/reportl/usermodel/   INormalCell 等接口
└── …
```

**Rust 侧引用坐标一律写 `ExtCellSet.java:LINE` 这种「文件名 + 行号」**（不写全路径），  
因为 `reportlite` 内**类名唯一**，全路径太长且会随 root 变动。第 7 节列出已建立的对应表。

---

## 3. 核心类 `ExtCellSet` 与它的两张矩阵

`ExtCellSet`（`…/model/engine/ExtCellSet.java`，**3781 行**）是计算态报表对象，`implements IReport`。  
它的**第一件必须记住的事**：内部有**两张同尺寸矩阵**（`ExtCellSet.java:95-96`）：

| 字段     | 类型                     | 语义                                  | 谁在读                                                |
| ------ | ---------------------- | ----------------------------------- | -------------------------------------------------- |
| `_$26` | `Matrix`（元素 `ExtCell`） | **计算矩阵** —— 值 / head / 合并别名         | `getCell(...)`、`stage_a` 的 `calc`、CalcJson 的值/head |
| `_$25` | `Matrix`（元素 `…engine`） | **引擎矩阵** —— 每格的引擎句柄（`getSource` 地址） | `getSource(...)`、CalcJson 的 `source`               |

两者在构造器里**同尺寸、同步铺**（`ExtCellSet.java:144-147`）：

```java
int irow = rd.getRowCount() + 1;      // :144  ← 含 0 号脚手架行
int icol = rd.getColCount() + 1;      // :145
this._$26 = new Matrix(irow, icol);   // :146
this._$25 = new Matrix(irow, icol);   // :147
```

> ⚠ `irow` 比 `expected.json` 的 `calculated.rows` **多 1**（0 号脚手架）。  
> Rust 的 `rpx_core::ReportDefine.rows/cols` **已经**是含脚手架的矩阵尺寸，**直接用，不要再 +1**  
> （见 `stage_a.rs:18-20` 的告诫）。

**合并区在构造期就铺满两张矩阵**（`ExtCellSet.java:177-181`）—— 这是 M2-08 的地基：

```java
ExtNormalCell extCell2 = new ExtNormalCell(this, i, j, endRow-i+1, endCol-j+1, cell2); // :177
for (m = startRow; m <= endRow; ++m)
  for (n = startCol; n <= endCol; ++n) {
    this._$26.set(m, n, extCell2);        // :180  计算矩阵：锚点别名
    this._$25.set(m, n, extCell2.engine); // :181  引擎矩阵：同一 engine 句柄
  }
j += ar.getEndCol() - startCol;           // :185  模板迭代**跳过**横向延续槽
```

`ExtCellSet` 里还有一批**高频访问器**（合并算法全靠它们）：

| 方法                               | 行          | 用途                                        |
| -------------------------------- | ---------- | ----------------------------------------- |
| `getRowCell(int r)`              | `:494`     | 取行头 `IRowCell`（`getRowType()` 用于「同行类型」判定） |
| `getColCell(int c)`              | `:509`     | 取列头 `IColCell`（`getColType()` 用于「同列类型」判定） |
| `void _$1(sr, sc, er, ec, head)` | `:706-712` | **矩形区域铺 head**（`_$26` 的批量 set）            |

---

## 4. 单元格类层次：模型层 vs 引擎层

移植时最容易搞混的是「同一个『格』有两套类」。分清楚：

```
                 ┌─ ExtCell (abstract, ExtCell.java:13)          ← 引擎层基类
                 │     getRow()/getCol() → 0        (:16/:20)
                 │     getRowMerge()/getColMerge() → 1  (:24/:28)  ← 注意默认是 1，不是 0
                 │     setRow/setCol/setRowMerge/setColMerge → 空实现 (:40-50)
                 │     getValue(boolean) → null      (:56)
                 │     abstract getSource()          (:254)
                 │     abstract clearProperties(...)  (:266)
                 │
                 ├─ ExtNormalCell  (ExtNormalCell.java:57, 2719 行)  ← 真正的普通格
                 │     field row/col/rowMerge/colMerge；getSource() (:151)
                 │     getValue(boolean) (:327)；getCellType() (:1213)
                 │     clearProperties() (:802)
                 │     merge 取数 :2005-2072（**代理到模型层**，见下）
                 │
                 ├─ ExtRowCell / ExtColCell / ExtExtendCell         ← 行头 / 列头 / 扩展锚
                 │
                 └─ NormalCell (model/NormalCell.java, 1695 行)      ← **模型层**（解析直出）
                       _$4 = mergeSame        get :1213
                       _$3 = mergeNull        get :621
                       _$2 = mergeSameMode    get :629
                       _$1 = mergeNullMode    get :637
                       ↑ 这四个就是 rpx-core/src/cell.rs 里 merge_same/merge_null/
                         merge_same_mode/merge_null_mode 的 Java 出处
```

**关键：引擎层的 merge 取数不是直接读模型值，而是「先查本格覆盖表，再落回模型」**  
（`ExtNormalCell.java:2005-2072`，四个属性同构）：

```java
public byte getMergeSame() {                                  // :2005
    Number b; int index;
    if (this.propSetMap != null
        && (index = this.propSetMap.getIndex((byte)89)) >= 0
        && (b = (Number)this.propSetMap.getValue(index)) != null)
        return b.byteValue();                                  // 本格覆盖
    return this.engine.srcPrepare.getMergeSame();              // 落回模型层 NormalCell
}
```

`propSetMap` 的**属性码**（`ExtNormalCell` 内部约定）：

| 码  | 含义            |
| -- | ------------- |
| 89 | mergeSame     |
| 90 | mergeNull     |
| 91 | mergeSameMode |
| 92 | mergeNullMode |

`setMergeSame(byte)`（`:2015`）只在**新值 ≠ 模型值**时才写覆盖表 —— 即引擎层默认「继承模型层」。  
`NormalCell` 侧由接口 `INormalCell`（`…/usermodel/INormalCell.java:316-328`）声明这四个 getter。

> **对移植的意义**：Rust 的 `rpx-core::cell` 已经把四个槽解析出来了（`cell.rs:239-246`），  
> 引擎侧只要**照抄「覆盖表 → 落回模型」**&#x8FD9;个两级结构即可；不要假设合并属性在计算期会变。

---

## 5. 计算管线：构造 → `calculate` → 六趟

**入口**：`calculate(Context ctx)`（`ExtCellSet.java:1448-1458`）—— 设置数据集阈值后**直接**转调  
私有 `_$2(ctx)`（`:1483-1543`）。`Rust` 侧对应 `crates/rpx-engine/src/calc.rs`（其头注释即写  
「Java 源码 `ExtCellSet.java:1448-1543`」）。

`_$2(ctx)` 的**顺序是硬约束**，被 `_$14 <= 4` 幂等门把守（`:1484`）：

| 序  | 调用                                                 | 行            | Rust 对应                               |
| -- | -------------------------------------------------- | ------------ | ------------------------------------- |
| 1  | `prepareCalculate(ctx, true)`                      | `:1487`      | `calc.rs` 前置                          |
| 2  | `_$1(ctx, dependCell)`                             | `:1496`      | 第 1 趟                                 |
| 3  | `resetDSCurrent` / `setDSCurrent` / `_$19.reset()` | `:1497-1499` | DS 复位                                 |
| 4  | `_$2(ctx, special, dependCell)`                    | `:1500`      | 第 2 趟（`eval/sched.rs` ← `:1297-1446`） |
| 5  | `resetDSCurrent` / `setDSCurrent` / `_$19.reset()` | `:1501-1503` | DS 复位                                 |
| 6  | `_$1(ctx, false)`                                  | `:1504`      | 第 3 趟                                 |
| 7  | `_$1(this._$9, ctx)`                               | `:1505`      | 第 4 趟                                 |
| 8  | `_$9()`                                            | `:1506`      | —                                     |
| 9  | chart style 解析                                     | `:1507-1519` | —                                     |
| 10 | `_$2()`（无参）                                        | `:1520`      | 第 5 趟                                 |
| 11 | `llllllIIlIIIlllI.perfect(this, ctx)`              | `:1521`      | —                                     |
| 12 | `resetDSCurrent` / `setDSCurrent` / `_$19.reset()` | `:1522-1524` | DS 复位                                 |
| 13 | `_$1(ctx, special, dependCell)`                    | `:1525`      | 第 6 趟                                 |
| 14 | `this._$14 = (byte)7`                              | `:1526`      | 置计算完成态                                |

> `_$14` 是状态机：`<=4` 才允许进计算，跑完置 **7**。审计里那条「`_$14<=4` 幂等偏离」  
> 说的就是这扇门（重复 `calculate` 在 `_$14==7` 时**静默跳过**）。

---

## 6. 合并管线（M2-08 / M2-21 的实现目标）

> **权威规格在别处，本节只是导航**：`docs/format-contract.md` 附件 **K.2**（`mergeSame`，`:1585-1621`）  
> 与 **K.3**（`mergeNull`，`:1623-1640`）才是**算法规格**（含 `Variant2.equals` 异常即视为不等、  
> `mode` 三分支语义、守卫回退公式等）。下面表格只提供「去 Java 哪一行看」的坐标，  
> **若与 K.2/K.3 冲突，以 K.2/K.3 为准。**

### 6.1 驱动：谁在什么时候调 `mergeSame` / `mergeNull`

合并**不是** `calculate` 的一部分 —— 它是**分页之后**、逐页跑的（`grep '\.mergeSame('` 全树仅两处调用方）：

| 调用点                         | 行                       | 场景    |
| --------------------------- | ----------------------- | ----- |
| `SimplePager.createPages()` | `:133-134` / `:179-180` | 分页主路径 |
| `CalcSubRpt`（子报表函数）         | `:138-139`              | 子报表   |

`SimplePager.createPages()`（`SimplePager.java:118-186`）两条分支：

```java
// —— 单页分支（整张网格一页）:127-143 ——
if (pagerStyle==0 || (pageWidth==9.99999999E8 && pageHeight==9.99999999E8 && columns==1)) {
    ExtCellSet ecs = (ExtCellSet) this.report.deepClone();   // :128
    Area page = new Area(0, 0, 1, 1);                        // :131
    ecs._$1(ecs, page);                                      // :132  置页上下文/背景图
    ecs.mergeSame();                                         // :133
    ecs.mergeNull();                                         // :134
    this.pages.set(0, 0, ecs);
    return;
}
// —— 多页分支：逐页 :174-182 ——
for (y …) for (x …) {
    Area page = new Area(y, x, ySize, xSize);
    ExtCellSet ecs = (ExtCellSet) this.pages.get(y, x);
    ecs._$1((ExtCellSet) this.report, page);                 // :178
    ecs.mergeSame();                                         // :179
    ecs.mergeNull();                                         // :180
}
```

**固定次序**：`deepClone` → `_$1(page)` → **`mergeSame()` → `mergeNull()`**。  
即 **先同值合并，后空值合并**，顺序不可换。

> **为什么 M2-08 能在单页前提下定为 P0**：`engine_samemerge` 夹具 `page_count = 1`，  
> 走的是上面**单页分支**（`:127-143`）—— 页网格 ≡ 计算网格，合并直接作用在整张 `_$26` 上，  
> **不需要先实现分页**（`docs/m2-engine-design.md:1196` 的定案）。

### 6.2 `mergeSame()` 四趟（`ExtCellSet.java:1832-2049`）

四个「方向趟」按 **列/行升序、降序** 各扫一遍。锚点谓词 + 延续判据 + 游标：

| 趟  | 扫描    | 行            | 锚点谓词（`cell`）                                               | 延续要求（`other`）                                                   | 游标              |
| -- | ----- | ------------ | ---------------------------------------------------------- | --------------------------------------------------------------- | --------------- |
| P1 | 列↑ 行↑ | `:1851-1902` | `value!=null` 且 `mergeSame ∈ {98,100}` 且 `mode != 120`     | 同值、同 `colMerge`、`mergeSame ∈ {98,100}`、同行类型、(mode118/119) 同 src | `i3 += max2-i3` |
| P2 | 列↓ 行↓ | `:1903-1948` | `value!=null` 且 `mergeSame ∈ {98,100}` 且 **`mode == 120`** | 同上；`src` = **右侧邻格**                                             | `i3 -= max-k-1` |
| P3 | 行↑ 列↑ | `:1949-2002` | `value!=null` 且 `mergeSame ∈ {99,100}` 且 `mode != 120`     | 同值、同 `rowMerge`、`mergeSame ∈ {99,100}`、同列类型、(118/119) 同 src     | `j += max2-j`   |
| P4 | 行↓ 列↓ | `:2003-2048` | `value!=null` 且 `mergeSame ∈ {99,100}` 且 **`mode == 120`** | 同上；`src` = **下方邻格**                                             | `j -= max-k-1`  |

**语义**：

- `mergeSame` 值 **98 = 纵向**、**99 = 横向**、**100 = 双向**。
- `mergeSameMode` 值 **118 = 用 `cell.getSource()`**、**119 = 用左/上邻格**、**120 = 只走反向趟（P2/P4）**。
- P1/P3 是「正向合并」，P2/P4 专门处理 `mode==120` 的**反向**情形（锚点在下/右）。

**写入动作**（每趟一致）：把锚点 `cell` 铺满合并矩形，并记跨距：


```java
for (j2 = j3; j2 < end; ++j2)
  for (i22 = i3; i22 < max2; ++i22)
    this._$26.set(i22, j2, cell);   // :1896  计算矩阵铺锚点别名
cell.setRowMerge(max2 - i3);        // :1899  记纵向跨距
```

**互斥守卫**（P1 用 `_$6`、P3 用 `_$5`，定义在 `:1790-1830`）：

```java
private boolean _$6(int start, int end, int col) {   // :1790  纵向重叠检查
    for (j = 1; j < col; ++j) {
        ExtCell c1 = (ExtCell) this._$26.get(start, j);
        if (c1 != null) { row1=c1.getRow(); row2=row1+c1.getRowMerge();
                          if (row1 < start && row2 < end) return false; }   // :1798-1800
        if ((c1 = (ExtCell) this._$26.get(end-1, j)) == null) continue;
        row1=c1.getRow(); row2=row1+c1.getRowMerge();
        if (row1 <= start || row2 <= end) continue;
        return false;                                                        // :1806
    }
    return true;
}
```

`_$5(start,end,row)`（`:1811-1830`）是它的**横向镜像**（用 `getColMerge`）。  
作用：当 `mode==118` 且待扩区间**与别处已有的合并跨距相交**时，回退扩边界（`:1890-1892` / `:1990-1992`）。

### 6.3 `mergeNull()` 四趟（`ExtCellSet.java:2051-2159`）

与 `mergeSame` **结构对称但语义不同**：`mergeNull` 没有 src 概念，只有**方向**，  
且**锚点谓词里没有 `value != null`** —— 它合并的是**空值格**。  
⚠ 方向码标在**被吞的空值格**上，含义是「我这个空格往哪个方向去找有值的格」  
（所以 `114 MN_UP` 对应的是**下锚**趟 —— 反直觉，别按字面理解；详见 `format-contract.md` K.3）：

| 趟  | 扫描    | 行            | 延续要求（`other`）                                        | 方向码             | 锚点动作                                                    |
| -- | ----- | ------------ | ---------------------------------------------------- | --------------- | ------------------------------------------------------- |
| P1 | 列↑ 行↑ | `:2069-2090` | `value==null` 且 `mergeNull==114` 且同 `colMerge` 且同行类型 | **114 = UP**    | `cell.addRowMerge(rm)`（向下长大）                            |
| P2 | 列↑ 行↓ | `:2091-2113` | 同上但 `mergeNull==116`                                 | **116 = DOWN**  | **`cell.addRow(-rm)` 先把锚点上移**，再铺 `[other.getRow(), i3)` |
| P3 | 行↑ 列↑ | `:2114-2135` | `value==null` 且 `mergeNull==115` 且同 `rowMerge` 且同列类型 | **115 = LEFT**  | `cell.addColMerge(cm)`（向右长大）                            |
| P4 | 行↑ 列↓ | `:2136-2158` | 同上但 `mergeNull==117`                                 | **117 = RIGHT** | **`cell.addCol(-cm)` 先把锚点左移**，再铺 `[other.getCol(), j)`  |

**锚点谓词**（四趟同构，注意**没有 value 判空**）：

```java
cell = (ExtNormalCell) this._$26.get(i3, j3);
if (cell == null || cell.getRow() != i3 || cell.getCol() != j3) { ++i3; continue; }  // :2073
```

**延续判据**（P1 为例，`:2079`）：

```java
for (k = i3 + cell.getRowMerge();
     k < rowSize
       && (other = (ExtNormalCell) this._$26.get(k, j3)) != null
       && other.getValue() == null            // ← 只吃空值格
       && other.getMergeNull() == 114
       && other.getColMerge() == colMerge
       && this.getRowCell(k).getRowType() == rt;
     k += rm) { … }
```

> P2/P4 的「先移动锚点」是**破坏性的**：它改的是 `cell` 自己的 `row`/`col`。  
> 移植时若先算后移，坐标会错 —— 必须**严格按原文顺序**（`addRow(-rm)` → 铺矩形 → `addRowMerge(rm)`）。

### 6.4 产品怪癖（照抄，别「修」）

`mergeSame` P3 开头（`ExtCellSet.java:1968`）有一句**看起来像 bug 的守卫**：

```java
if (max2 >= colSize) continue block19;   // :1968
for (max2 = j + cell.getColMerge(); max2 < colSize; max2 += other2.getColMerge()) { … }  // :1969
```

`:1968` 判的 `max2` 是**上一轮循环残留的值**（`max2` 在方法级声明，此处尚未被 `:1969` 赋值）。  
这是**产品自身的怪癖**，不是我们的笔误。**移植时要么逐字保留，要么用夹具证明它不影响输出**，  
不许「顺手改成 `j + cell.getColMerge()` 更合理」——那会改变行为且无对拍依据。

### 6.5 合并方向码速查

| 属性              | 码                     | 含义                        |
| --------------- | --------------------- | ------------------------- |
| `mergeSame`     | 98 / 99 / 100         | 纵向 / 横向 / 双向              |
| `mergeSameMode` | 118 / 119 / 120       | 用 source / 用左·上邻格 / 只走反向趟 |
| `mergeNull`     | 114 / 115 / 116 / 117 | UP / LEFT / DOWN / RIGHT  |

### 6.6 `ExtCellSet.java:1968` 的证据链（为什么必须逐字照抄）

**问题**：P3 开头 `:1968` 的 `if (max2 >= colSize) continue block19;` 读的 `max2` 是
**方法级局部**（`:1842` 声明），P1（`:1869-1900`）与 P3（`:1969-2000`）共用它 ⇒
此处读的是 **P1 留下的残值**，看起来像反编译噪声。三条独立证据说明它是**真实源码**。

**证据 ① 两个独立 JAR 的同名类给出同一句**（2026-10-13 现场核验）

| 产物 | 位置 | 语句 |
|------|------|------|
| `reportlite` | `re_work/src/reportlite/03-model/com/raqsoft/reportl/model/engine/ExtCellSet.java:1968` | `if (max2 >= colSize) continue block19;` |
| `report` | `re_work/src/report/04-model/com/raqsoft/report/model/engine/ExtCellSet.java:2106` | 同上（仅把 `_$26` 换成 `_$54`） |

**证据 ② CFR 行为探针**（2026-10-13 现场重跑）

把 `continue` 的两种形态分别 `javac` + CFR 反编译：

| 变体 | 源码形态 | CFR 输出 |
|------|----------|----------|
| **D** | `block19: for(...) { while(...) { if (c) continue block19; … } }` —— 在**嵌套 `while` 内**跳向**外层 `for`** | `block0: for(...) { while(...) { if (n >= n2) continue block0; … } }` ← **带 `blockN`** |
| **E** | 同结构，但 `if (c) continue;`（跳向 `while` 自己） | `… if (n >= n2) continue;` ← **无 `blockN`** |

⇒ CFR **只在「`continue` 在嵌套循环里跳向外层循环」时**产出 `continue blockN;`。
而 Java 里要跳到外层循环**必须**写标签 ⇒ `continue block19;` 意味着源码里**确实**有一句
跳向 `:1949` 外层 `for` 的标签 `continue` ⇒ 是**真实源码**，不是噪声。

ⓘ **更正（2026-10-13）**：本节初稿把触发条件写成「显式给外层 `for` 打标签」，**不准确** ——
标签只是手段，真因是「**跳向外层循环**」。初稿探针把 `continue` 直接放在外层 for 体里
（**没有嵌套内层循环**），CFR 会把它简化成普通 `continue;`，因此**复现不出来**；
补上内层 `while` 后才复现（上表 D/E）。

复现命令（JDK8 + `re_work/tools/cfr.jar`；探针放 `/tmp`，不进仓库）：

```bash
J8=<zulu8>/Contents/Home/bin; CFR=<re_work>/tools/cfr.jar
mkdir -p /tmp/probe/out && cd /tmp/probe
# D.java: block19: for(...) { while(...) { if (c) continue block19; … } }
"$J8/javac" -d out D.java && "$J8/java" -jar "$CFR" out/D.class | grep -E 'continue'   # 期望 continue block0;
# E.java: 同结构但 continue;（无标签）
"$J8/javac" -d out E.java && "$J8/java" -jar "$CFR" out/E.class | grep -E 'continue'   # 期望 continue;
```

**证据 ③ 后果：P3 从第一个"触到右边界"的行起整体停摆**

`:1969` 的内层 for 退出条件是 `max2 >= colSize` ⇒ 任何一行只要进入内层 for，退出时
`max2` 必然 `>= colSize`。于是**下一行**的 `:1968` 立即命中 `continue block19` ⇒ 该行被
整个跳过（连 `:1999` 的 `cell.setColMerge(max2 - j)` 都不执行），而 `max2` 保持
`>= colSize` ⇒ **其后每一行都被跳过**。结论：P3 处理完第一个"跨度触到右边界"的行之后
**整体停摆**；若 P1 的残值已 `>= colSize`，P3 **一行都不做**。

**为什么没有夹具能分辨**：全部 12 个夹具只设过 `mergeSame = 98`（纵向，归 P1/P2），
**没有 99/100**（横向，归 P3/P4）⇒ P3/P4 与整个 `mergeNull` 都**无夹具覆盖**
（唯一出处 `tests/fixtures/EngineModelsP0.java`）。所以「把它'修好'」**不会有任何测试失败**；
而将来产品补一个 `mergeSame=99` 的夹具时，**产品会带着这个残值行为** ⇒ 逐字照抄是对的。

**补记（2026-10-14，审计）：Rust 侧的「定值洞」与我们的取止**

上面说的是 Java 侧的残值语义。**Rust 侧另有一个只有移植时才会遇到的问题**：
`max2` 在 Java 是**方法级局部**（`:1842` 声明、`:1869` 首次赋值、`:1968` **先读**），
而 Rust **不允许**读未初始化的局部 ⇒ 必须给初值。本仓取 **`0`**：
`crates/rpx-engine/src/merge/same.rs:22` 的 `let mut max2: i32 = 0;`。

⚠ 这是**推断、不是移植** —— Java 那一处读的是**槽位残值**，其值在语言层面是未定义的。
取止理由：`:1968` 的判据是 `max2 >= colSize`，而 `colSize >= 1` ⇒ **初值 `0` 恒不命中**，
等价于「P1 没写过 `max2` 时 P3 不跳过任何行」；P1 一旦写过，两边读到的就是同一个值。
⇒ **仅当 P1 一行都没处理过时**两边才可能不同，而那种情形下 Java 的残值本就未定义。

**为什么现在无法证伪**：12 个夹具**没有一个** `mergeSame = 99/100`（横向归 P3/P4）
⇒ 见上文「为什么没有夹具能分辨」。将来出现横向 `mergeSame` 的夹具时，
**先拿产品实测确认 `:1968` 的残值到底是几**，再决定是否把初值从 `0` 改成别的；
改初值必须同步更新本节 + `merge/same.rs` 的头注释 + §6.6 上文那段。
（与 `docs/m2-engine-design.md` §5.4.6 的 `same_value` 7 处分歧同属「无夹具可辨」类。）

---

## 7. Rust 侧的引用约定与已建立的对应表

**约定**：Rust 文件头注释用 `ExtCellSet.java:LINE` 标注出处（类名唯一，故省全路径）。  
已建立的对应（2026-10-13）：

| Rust 文件                                 | 对应 Java                            | 覆盖范围                        |
| --------------------------------------- | ---------------------------------- | --------------------------- |
| `rpx-engine/src/stage_a.rs`             | `ExtCellSet.java:139-193`          | 模板 → 两张矩阵（构造器）              |
| `rpx-engine/src/calc.rs`                | `ExtCellSet.java:1448-1543`        | `calculate` → `_$2(ctx)` 编排 |
| `rpx-engine/src/eval/sched.rs`          | `ExtCellSet.java:1297-1446`        | 三条调度通道                      |
| `rpx-engine/src/heads/find.rs`          | `ExtCellSet.java:631-704`          | 找左/上 head                   |
| `rpx-engine/src/heads/normalize.rs`     | `ExtCellSet.java:783-806`          | head 归一                     |
| `rpx-engine/src/ext_algo.rs`            | `ExtCellSet.java:981-1165`         | 扩展算法                        |
| `rpx-engine/src/extend.rs`              | `ExtCellSet.java:148`, `:414-415`  | `_$10` 锚 / 尺寸口径             |
| `rpx-engine/src/stage_b/merge_alias.rs` | `ExtCellSet.java:166-192`          | 合并区两张矩阵分歧                   |
| `rpx-core/src/cell.rs`                  | `NormalCell.java:621/1213/629/637` | 四个 merge 槽解析                |

**尚无对应**（M2-08 / M2-21 待补）：

| Java                        | 行                           | 待补 Rust                |
| --------------------------- | --------------------------- | ---------------------- |
| `mergeSame()`               | `ExtCellSet.java:1832-2049` | 四趟 + 守卫 `_$5/_$6`      |
| `mergeNull()`               | `ExtCellSet.java:2051-2159` | 四趟                     |
| `SimplePager.createPages()` | `SimplePager.java:118-186`  | 合并驱动（单页分支）             |
| 合并区产出 `merged`              | `emit` 侧                    | `emit.rs` 增 `merged` 键 |

**已核实的两处 Rust 侧缺口**（2026-10-13）：

- **`emit.rs` 目前只产 `calculated` 段**：其公开函数只有 `render_calculated`（`:60`）与  
  `render_calculated_error`（`:78`），**没有任何 `pagination` / `merged` 渲染器**  
  （`grep -nE 'pagination|merged' crates/rpx-engine/src/emit.rs` 零命中）。  
  「后续里程碑，本片未实现」这句在 **`tests/engine_grid_compare.rs:14`**（不是 `emit.rs`）。
- **`col_type` 无 Rust 对应**：`mergeSame` P3/P4 与 `mergeNull` P3/P4 都要  
  `getColCell(j).getColType()`（`ExtCellSet.java:1959/1988/2012/2036`、`:2123/2145`），  
  但 Rust 侧只有 `stage_b.rs:289` 的 `row_type_at(rd, row)`，**没有 `col_type_at`** ——  
  实现合并前必须补上列类型读取（否则「同列类型」这条延续判据无从落地）。

---

## 8. 对 M2-08 / M2-21 的意义（落地清单）

1. **两张矩阵都要建模**：`mergeSame`/`mergeNull` 全程只写 `_$26`（计算矩阵），  
   但**读**要靠 `_$26.get` 拿 `ExtCell`、靠 `getRowCell/getColCell` 拿行/列头。  
   Rust 侧 `stage_a` 已有 `calc` 矩阵与 `by_pos` 别名，够用；**确认 `getRowCell/getColCell` 有 Rust 对应**。
2. **合并属性走两级取数**（§4）：先覆盖表（码 89-92）再落回模型 —— 与 `rpx-core::cell` 对接。
3. **顺序不可换**：`mergeSame` 在前、`mergeNull` 在后；各自内部**四趟的扫描方向也不可换**。
4. **单页前提成立**：`engine_samemerge` `page_count=1`，走 `SimplePager.java:127-143` 单页分支，  
   **不必先实现分页**（`docs/m2-engine-design.md:1196`）。
5. **`:1968` 怪癖照抄**，用夹具证明无害，别改「顺手」。
6. **`merged` 产出与 `Dim::Merged` 解豁免**：当前 `engine_grid_compare.rs` 的 `split_exempt`  
   把 `Dim::Merged` 豁免掉了（`:84-90`），并在 `:14` 写明「分页（`pager`）与同值合并（`merge`）  
   属**后续里程碑**，本片未实现」。而 `emit.rs` 目前**只产 `calculated` 段**、无 `merged` 渲染器。  
   ⇒ M2-08 要能被对拍钉住，**必须让 `emit` 真的产出 `merged`**，否则它的唯一可观测面仍被豁免。

---

## 附录：复现命令

```bash
cd re_work

# 语料规模（本文 §1 的数字来源）
find src -name '*.java' | wc -l                    # 5646
find src -name '*.java' -exec cat {} + | wc -l     # 1054227

# 两张矩阵
grep -nE '_\$2[56]\s*=' src/reportlite/03-model/com/raqsoft/reportl/model/engine/ExtCellSet.java

# 合并驱动（全树仅两处调用方）
grep -rnE '\.merge(Same|Null)\s*\(' src/reportlite

# 四个 merge 槽的模型层出处
grep -nE 'public byte get(MergeSame|MergeNull|MergeSameMode|MergeNullMode)\(' \
    src/reportlite/03-model/com/raqsoft/reportl/model/NormalCell.java

# ⚠ BSD grep：多词任选其一必须 grep -E，不能 grep 'a\|b'（会静默零结果）
```

> **方法学提醒**：本文所有「同一逻辑在别处也有」的结论都用 `grep -rnE`（ERE）验证过。  
> 本机是 BSD grep，`grep 'a\|b'` 在 BRE 下把 `\|` 当字面量 → **静默零结果**，  
> 曾多次导致「字段/函数不存在」的误判。**凡 grep 说「没有」，先换 `-E` 或换工具复验。**
