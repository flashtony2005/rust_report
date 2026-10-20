# Spec — ReportLite Rust 重实现 · M5（多数据集关联）v0.6

> 状态：**M5-01~M5-04 / M5-06 / M5-07 已闭合**（M5-07 与带账⑤的
> `ds[i]` 偏移语法于 2026-10-16 闭合：探针实证 + 8 条单测）。
> M5-01：`ds.select` 全参数矩阵 + 双态求值（`expr/dsselect/` 三文件，单测全绿）；
> M5-02：跨数据集过滤求值 + 合并区双视图（`calc/merged_view.rs`）+
> 跨页合并拆分，`cross_dataset_compare` 10 夹具逐格逐页零差异；
> M5-03：显式 leftHead/topHead 槽位（slot 30/31）—— 机制在 M2 头图工作里
> 已实装（`heads/find.rs` 分支 A + `ext_algo/envelope.rs` 左子格递归 +
> `ext_algo/row.rs` `extend_row` 复制循环），本里程碑以 T4/T5 嵌套主子表
> 夹具逐格对拍正式验收，变异测试证明显式 head 分支承重；
> M5-04：产品夹具闭环 —— `engine_cross`（嵌套主子表，显式左主格 A3 承重）+
> `engine_cross_root`（Root 态跨数据集，字面量过滤 `amount>=2000`）进 20 夹具
> 主对拍全绿，Java 侧 round-trip 门同步闭合；
> M5-05：**探针完成 → 带账**——12 布局实证 + 源码不变量证明 `filtercross`
> 的过滤语义在本版运行时不可驱动（详见 §2.6 末与 §6.3 ⑧），按「无夹具不实现」
> 纪律不实现；
> M5-06 的 Root 态裸列语义（`ds.col` = 首行值）随 M5-01 同批闭合。
> M5-07：**已闭合（2026-10-16）**——探针实证 + 三夹具进主对拍（详见 §6.4）。
> **§6.3 残账 2026-10-20 清零进度**：④（多键全键相等）✅ 闭合·探针 = 输入序 +
> 夹具 `engine_tiesort`；⑥（过滤非布尔文本）✅ 闭合·三形态逐字一致 + 夹具
> `engine_rowtest`；③（`Group.filter@b` 二分）✅ 决策闭合——有序域等价（夹具
> `engine_groupb`），无序域**有意偏差**（产品返回二分命中段，本仓走线性）；
> ⑦（合并段断在分组头行）✅ 已结案：行高单位标定（mm / 每页 297mm）+
> 分组头行三条路径均被产品拒绝（报错/NPE/不合并）⇒ 不可驱动；可驱动形态下本仓
> 矩形求交与产品一致（新夹具 `engine_mergepage` 承重）。
> 上游：M2/M3/M4 已收官（`spec-m2.md` / `spec-m3.md` / `spec-m4.md`），HEAD `9aa6caf`。
> 立项依据：数据蒋堂讲座行动序 #2「多数据集关联语义立项（三分类 + 维度对齐）——
> 报表工具的核心差异化，大（先探针）」（`docs/lecture-notes.md` 行动表）。
> 本文件定义 **M5 做什么、语义是什么、怎么验收**；实现细节以夹具与源码为准。

## 0. 本文件的可信度声明

凡未附证据的断言一律标注「待验证」。本文件的证据分两级：

1. **源码级**（反编译 `re_work/src/reportlite/**`，带 `file:line`）——产品机制定案；
2. **真机级**（探针 `ProbeCross.java` 打金仓 `report` schema，9 模板 round-trip 全绿，
   truth 落 `tests/fixtures/out/cross_*_product.json`）——语义真值。

## 1. 立项依据：为什么是 M5

官方口径（`docs/report-model-and-cell-extend.md` §3.3，手册原文）：

> 「润乾报表的多个数据集**不需要进行整合运算**，而是**直接用于报表的运算**，
> 在报表的运算过程中，通过一定的规则把多个数据集**关联**起来。」
> 「数据集间的整合运算……极大的占用 cpu 资源……很多传统的报表工具号称支持多数据集，
> 其实是把多个数据集通过整合运算，合并成一个数据集……其实质还是单数据集模型。」

即：**关联发生在求值时刻，没有 join、没有临时表**。Rust 侧现状（`crates/rpx-engine/src/expr/dsfn.rs:216-309`）：

- ~~`ds.select` 只有 **1 参数**形态~~ → **M5-01 已闭合**：全参数矩阵
  （`,` 四段 + `:` 次序 + `;` 多排序）+ 双态求值落地于 `expr/dsselect/`
  （`DSSelect.calculate:27-209` 逐行移植，13 单测含 T3 六变体复现）；
- ~~带过滤/限量/去重的 4 参矩阵~~ → **M5-01 已闭合**；
  ~~跨数据集过滤~~ → **M5-02 已闭合**（电流语义 + 双视图对拍）；
  ~~嵌套主子表（显式左主格）~~ → **M5-03 已闭合**：slot 30/31 解析 +
  `heads/find.rs` 分支 A 显式 head 优先 + `ext_algo/envelope.rs` 跨度递归纳入左子格
  + `ext_algo/row.rs` `extend_row` 复制循环，T4/T5 逐格对拍（14 行嵌套网格）；
- ~~Root 态裸列 `ds.col` 现报 `UnsupportedExpr`~~ → **M5-01 同批闭合**
  （`dsfn::root_current_data`：产品 `DataSet.getCurrentData:395-410` 的
  currentRow 光标语义，本仓 Root 态恒 ≡1 ⇒ 首行值；空表 ⇒ null）。

这直接卡住财政工资场景（人员主表 × 工资项明细 × 扣款项），也是「多源分片报表」
（手册 `dyglfpdgnytzzj105`：多源七八个乃至十几个）的入口能力。

## 2. 产品机制定案（源码级）

### 2.1 表达式面：`ds.名(...)` 的分派

`Expression.java:175`：标识符含 `.` 时切出数据集名 → `createDSNode(cs, ds, 剩余, true)`
（`:253-271`）：剩余名是**已登记 DS 函数**才建函数节点（`FunctionLib.isDSFunName`），
否则按列名解析（`getDSColNo`）。已登记 DS 函数全集见 `FunctionLib.java:106-121`：
avg / bfind / fcount / count / dcount / field / fname / **filtercross** / group /
isfield / max / min / plot / **select** / sum。

⛔ **`filter`（一对多）与 `filterone`（一对一）不在登记表里**——IDE 语义面板
（`JPanelDataSet.java:86-101`）有这两个操作，但运行时不认；模板里写
`ds2.filter(...)` 会落到未知符号。**边界带账**（§6.3）。

### 2.2 参数矩阵：`;` → `,` → `:` 三层切分

`ParamParser.java:35-64` + `:66-79`：参数串按**三层**切（`prevLevel` 递进
`\0 → ';' → ',' → ':'`），空段返回 null IParam。`DSSelect.java:36-138` 按此解码：

| 形态 | 结构 | 语义 |
| --- | --- | --- |
| `select(x)` | Leaf | 选列 |
| `select(x:1)` | `:` 两层 | selectExp + order（1 升 / -1 降） |
| `select(x; s1:1, s2:-1)` | `;` 两层 | selectExp + 多表达式排序（`ParamInfo2.java:26-110`） |
| `select(x, f, n, d)` | `,` 四段 | selectExp + **过滤** + **限量 num** + **去重**（`:106-138`） |
| `select(x:1, f, , d)` | 混合 | 官方主子表样例形态（`report-model-and-cell-extend.md` §3.3） |

官方样例（手册 `dpkz101`）：`B2 = ds1.select(订单ID:1,,,订单ID)`（主表去重扩展）、
`B8 = ds2.select(产品ID:1, 订单ID==B2,,产品ID)`（子表用主格过滤）。

### 2.3 双态分派：Row 态 / Root 态

`DSSelect.java:146-205`：

- **Row 态**（`ds.getCurrent() instanceof Row`，`:148`）：只对**当前行**测过滤
  （`Row.test`，`:150-153`），命中 → `setValue` 并返回该行；未命中 → `NullRow`。
  **num/distinct/order 在 Row 态全部不生效**（探针 §3.1 实证）。
- **Root 态**（current 为 Group，`:155`）：`group.filter(...)` → `group.distinct(...)`
  → `evaluateRows` → 排序 → 取行列表；size==0 → `NullRow`（`:175`）；
  size==1 或 num==1 → 首行（`:179`）；否则返回 List（`:185`）⇒ 单元格**扩展**。

数据集 current 的来源：`ExtCellSet.setDSCurrent`（`:582-593`）把**所有 current 为 null
的数据集设为 root group**；扩展格求值时经 `ExtExtendCell.setDSCurrent`
（`ExtExtendCell.java:297-318`）把 current 切成行/组。`DataSet.getCurrentData`
（`DataSet.java:384-401`）：Group 态按 `_$5`（currentRow，`setCurrent` 置 1）取行。

### 2.4 嵌套机制：显式左主格（slot 30）

这是本立项**最关键的机制发现**，回答「主子表为什么能逐实例嵌套」：

1. **模板槽位**：`NormalCell._$56`（leftHead）/ `_$55`（topHead）是**单元格 ID 字符串**
   （`NormalCell.java:229-242`），序列化于 slot 30/31（`:658-659` writeExternal /
   `:1367-1368`）——`.rpx` 里真实存在，不是运行期临时物。
2. **显式优先**：`ExtCellSet._$3(cell)`（左主格，`:781-810`）与 `_$2(cell)`
   （上主格，`:753-780`）**先查显式字符串**（`getSource(head)` 解析单元格 ID），
   查不到才走「左/上近邻 + 拓扑链」默认规则（`:792-793` 装配）。
3. **跨度包含子格**：`_$4/_$3(ExtCell)`（`:944-970`）求扩展格行跨度时，
   **递归纳入左子格**（`getLeftSubSize/ getLeftSubCell`）⇒ 从格带挂在主格下后，
   **主格的行跨度覆盖从格带**。
4. **复制循环**：`extendRow(cell, count)`（`:997-1050`）对跨度内每一行、
   复制 `cell.isLeftHeadOf(sub)` 的格子（`isLeftHeadOf` 沿 leftHead 链走，
   `ExtNormalCell.java:392-399`）⇒ 从格带逐主格实例克隆。
5. **向导同构**：报表向导生成分组/主子表时，对从格带单元格
   `ac.setProperty((byte)30)` + `ac.setValue(CellPosition.toExcelNotation(主格坐标))`
   （`ReportEditor.java:4768-4776`、`:4784`、`:4847`）——即**显式左主格正是产品
   向导的产物**，不是民间偏方。

⇒ **Rust 侧必须建模显式 leftHead/topHead 槽位**（`rpx-core` reader + engine 头图），
否则主子表模板（向导产物）无法复现。

### 2.5 维度对齐：`cross()`（交叉表，同数据集双方向）

`DsValue.cross`（`DsValue.java:36`）抽象 + 三实现：

- `Group.cross(Group)`（`Group.java:112-165`）：同行号归并（`getRowNo`）。
  **两条路径**：`threshold >= 0 && ds.getRowCount() >= threshold` 时走**有序归并**
  （`_$2()` 排序行号数组双指针，O(n+m)）；否则走行号位图成员测试。
  交集空 → `NullGroup`。
- `Group.cross(Row)` / `Row.cross(...)`：成员/同一性判定，不中 → `NullRow`。

调用点：`ExtExtendCell.setDSCurrentTopHead`（`:265-291`）——当同一数据集同时被
**横向扩展格**驱动（`ds.isLeftTop()`，`DataSet.java:336`）时，current 与扩展格
自身值求交。即：**交叉表（同数据集行×列双方向扩展）的维度对齐**，
与多数据集关联是**两件事**，本 spec 只立项前者为带账、后者为实现（§5）。

### 2.6 多对多：`filtercross`

`DSFilterCross.java`（全文已读）。语义：`ds.filtercross(条件)` 对**当前扩展格的
DSCurrent**（另一数据集的 Group/Row）与自身 current 求交。条件为
`Equals`/`And` 树时可抽等值列对（`_$1(Node, ds, leftList, rightList)`，`:296-330`），
右表 count2 > 6 时**排序 + 二分归并**（`SortDsValue.sort` + `_$1(List, col, key, low, high)`，
`:105-145`）；否则嵌套循环。结果 Group 克隆承载命中行。

⚠ **运行时实证（2026-10-15，M5-05 探针，8 布局 × 三路取证）**：上述「与
DSCurrent 求交」的语义在本版运行时（reportlite 20260106）**不可驱动**——
DSCurrent 读的是 `ExtExtendCell._$4`，而 `_$4` 全源仅在其**自身求值之后**被写
（`ExtExtendCell.java:463` 非 List 结果 / `:440` 扩展写回），filtercross 求值
**期间**必为 `null` ⇒ 只有 `filterExp == null → return cur`（恒等）与
`getNullGroup()`/原样 Group 返回三类出口可达，过滤分支（含 `@r` 的
many-to-many 合并）需要同一格实例**二次求值**才能读到上一次结果，而实证
每实例恰求值一次。证据链见 §6.3 ⑧ 与 `tests/fixtures/RUNLOG.md` §42。

### 2.7 `Group.filter` 的快速路径

`Group.java:201-246`（`_$1(exp, ctx, opt, isClone)`）：

1. 条件整体是常量 → 直接判布尔（`:207-218`）；
2. **option 含 `b`（98）且条件是「本数据集列 == 外部值」的 Equals 树** →
   抽 (col, value) 走**二分**（`_$1(int col, Object value, ...)`，`:248-291`；
   多列走 `_$1(int[] cols, Object[] values, ...)`）；
3. 否则逐行 `row.test(exp, ctx)`（保存/恢复 current，`:232-241`）。

⚠ 二分路径要求行**已按该列有序**（`sort` 置 `_$2`）；无序时行为依赖数据，
实现期须以夹具钉死走哪条路。

## 3. 探针结论（金仓真机级）

探针 `tests/fixtures/ProbeCross.java`（cwd=tests/fixtures，
`common/jre` + `reportlite/lib/*` + `lib/postgresql-42.7.4.jar` 跑；
金仓 `192.168.1.77:54323/test`，`report` schema）。数据：

- `report.emp_info`（5 人，ProbeSqlDemo 已建）：张伟(1,人事部)/李娜(2,财务部)/
  王强(3,技术部)/赵敏(4,技术部)/孙丽(5,市场部)；
- `report.emp_bonus`（6 行，本探针建）：张伟 2 项（全勤奖500/年终奖3000）、
  李娜 1（全勤奖500）、王强 1（餐补300）、赵敏 2（项目奖2000/全勤奖500）、
  **孙丽 0 项**（无匹配用例）。

9 模板全部 round-trip（回读 .rpx 重算逐字节一致）。truth：
`tests/fixtures/out/cross_*_product.json`（含凭据的 `ds_kingbase.json` 不入库，
`out/` 整体 gitignore）。

### 3.1 T1 `cross_emp_bonus`：非嵌套从格带（无显式左主格）

布局：r3 主格带（ds1 四字段）+ r4 从格带（`ds1.*` 跟随 + `ds2.select(item, emp_id==A3)`
四变体）。**实测：从格带不嵌套**——整带只扩展一次，过滤条件取**主格第一个实例**
（张伟 id=1）求值 → 2 行（全勤奖/年终奖）；且同带变体列（num=1/去重/降序）
被主扩展格携带成 **Row 态**（num 不生效——r9c7 仍是年终奖）。

⇒ 教训：**没有显式左主格，就没有主子表**。「从格带写在主格带下面」不等于嵌套。

### 3.2 T2 `cross_dept_band`：去重主格 + 同数据集从格

`A3 = ds1.select(dept:1,,,dept)` → 4 个去重部门（**按中文 collation 升序**：
财务部<技术部<人事部<市场部），extend=18 带 4 个子格。
`B3 = ds1.select(name, dept==A3)` 等同带从格取**每个部门第一人**
（Row 态只测当前行，不展开组内多行——技术部 2 人只显示王强）。
跨数据集从格 `ds2.select(item, emp_id==C3)` 同样只算一次（C3=首实例 2 → 全勤奖/500）。

⇒ 去重主格（distinct）+ 同数据集过滤从格 = **组头带**形态；要组内多行需嵌套（T4）。

### 3.3 T3 Root 态隔离变体（每变体独占模板，规避共带干扰）

| 变体 | 表达式 | 实测真值 |
| --- | --- | --- |
| 过滤 | `ds2.select(item, amount>=2000)` | List [年终奖, 项目奖]，**纵向扩展**（extend=18，2 行） |
| 限量 | `ds2.select(item, emp_id==1, 1)` | 单值 全勤奖（num=1 不扩展） |
| 去重 | `ds2.select(item, , , item)` | List [全勤奖, 年终奖, 餐补, 项目奖]（**首现序**，4 行） |
| 降序 | `ds2.select(item:-1)` | List 全部 6 行按 item 中文降序（项目奖>全勤奖×3>年终奖>餐补） |
| 裸列 | `ds2.item` | **首行值 全勤奖**（extend=17，非扩展格；⛔ 非报错） |
| 计数 | `ds2.count()` | 6（Root 态全量；Row 态为 1，合带 T3 实证） |

⚠ **共带干扰实证**（合带版 T3）：同带多个 DS 函数格互相污染——首格扩展后
其余格求值看到的 ds.current 已被改写，c2-c5 全 null、count=1。**实现期夹具必须
一模板一语义点**（隔离原则），否则对拍结论不可归因。

### 3.4 T4 `cross_nest`：嵌套主子表·经典布局（显式左主格）

`E4.setLeftHead("A3")`（其余默认）。实测网格 **14 行**：

```
r3  张伟(1,人事部,8500)          ← 主格实例
r4  全勤奖 500                   ┐
r5  年终奖 3000                  ┘ 张伟 2 项（多匹配 → 2 行）
r6  李娜(2,财务部,9200)
r7  全勤奖 500                   ← 李娜 1 项
r8  王强(3,技术部,12000)
r9  餐补 300                     ← 王强 1 项
r10 赵敏(4,技术部,11000)
r11 项目奖 2000                  ┐
r12 全勤奖 500                   ┘ 赵敏 2 项
r13 孙丽(5,市场部,7800)          ← 无匹配：**零从格行**
```

- 主格实例行 3,6,8,10,13；从格行紧随其后，**多匹配展开、无匹配不产行**；
- 从格两列（item/amount）同步扩展（F6 被 E4 的扩展携带，值逐行正确）；
- 合并段（`merged`）：主格列空单元格在从格行上被合并（r4/r5 c1-c4 各 rm=2）。
- **这就是官方主子表的产品行为**，可直接作为 M5-04 夹具基准。

### 3.5 T5 `cross_nest_vals`：嵌套 + 从格带跟值

从格带 A-D 列也挂显式左主格并写 `ds1.id/name/dept/salary`（Row 态）。
实测：**每个从格行都重复主格值**（r4/r5 都是 1/张伟/人事部/8500）；
且**无匹配的主格仍产 1 个从格行**（r14：5/孙丽/市场部/7800，奖金两列空）。

⇒ 与 T4 的差异全部来自「从格带是否存在非扩展的显式头跟随格」——
**无匹配是否产行取决于从格带构成**，实现必须两条路都建模（T4/T5 双夹具）。

## 4. JOIN 三分类 → ReportLite 表达形式映射

数据蒋堂分类（`docs/lecture-notes.md` 启发 7/8）与产品形态的对应：

| 蒋堂分类 | ReportLite 形态 | 产品机制 | 本 spec 处置 |
| --- | --- | --- | --- |
| 外键关联（多对一，事实→维表） | `维表.select(维列, 事实键==主格)` | select 带过滤（Row/Root 双态） | **M5-01/02 实现** |
| 主子表（一对多，主→从） | 显式左主格 + `从表.select(从列, 主键==主格)` | slot 30 + 跨度 + 复制循环 | **M5-03 已闭合**（机制 M2 实装，T4/T5 验收）；M5-04 补产品夹具闭环 |
| 同维表（一对一） | `ds.filterone(...)`（IDE 有、运行时不认） | 未登记 | **带账**（§6.3）；等价写法 = num=1 的 select |
| 多对多 | `ds.filtercross(条件)` | `DSFilterCross` | **M5-05 实现**（先探针） |
| 维度对齐（交叉表） | 同数据集行×列双扩展 | 19×18 交叉展开（**无 `cross()` 函数**——立项时的预估机制名，源码核实产品 `FunctionLib` 只有 `filtercross`） | **M5-07 已闭合**（2026-10-16）：M2 复制循环 + M5-03 heads 链天然覆盖，三夹具进主对拍 |

## 5. 任务表（M5）

| 编号 | 任务 | 落点 | 验收 |
| --- | --- | --- | --- |
| M5-01 | `ds.select` 参数矩阵：`,` 四段（过滤/限量/去重）+ `:` 次序 + `;` 多排序 | `rpx-engine`（`expr/call.rs` 参数树 + `expr/dsselect/` 双态求值） | ✅ **已闭合**（2026-10-15）：单测覆盖四形态 + 6 种产品报错；T3 六变体复现；门禁全绿 |
| M5-02 | 跨数据集过滤求值：过滤式里的 CSVariable/DSVariable 解析与 current 管理 | `rpx-engine`（`calc/ds_current.rs` 扩展） | ✅ **已闭合**（2026-10-15）：T1/T2/T3 八夹具逐格逐页零差异；电流语义与页内投影双变异实测；`calc/merged_view.rs` 双视图落地（跨页合并拆分同批闭合） |
| M5-03 | 显式 leftHead/topHead 槽位：reader 解析 + 头图 + 跨度计算 + 嵌套复制 | `rpx-core`（slot 30/31）+ `rpx-engine`（`heads/` + `ext_algo/`） | ✅ **已闭合**（2026-10-15）：机制在 M2 头图工作实装，本里程碑以 T4/T5 嵌套主子表夹具固化验收（`cross_dataset_compare` 现 10 夹具）；变异测试摘除 `heads/find.rs` 分支 A ⇒ T4/T5 均从 14 行塌陷为 9 行（证明显式 head 承重，非默认近邻误判）；`merge_data` 跨页克隆/服务面双视图不变式不受影响 |
| M5-04 | 产品夹具 `engine_cross`（嵌套主子表）+ `engine_cross_root`（Root 变体，一模板一语义点） | `tests/fixtures`（`EngineModelsM5.java` + `GenEngineFixtures` + `engine_cross.csv`/`engine_cross.csv2`） | ✅ **已闭合**（2026-10-15）：`EngineModelsM5.crossFixture()`（r3 主格 `ds1.select(id/name/dept/salary)` + r4 从格 `ds2.select(item/amount, emp_id==A3)` 带 `setLeftHead("A3")`，title 跨 6 列合并）+ `crossRootFixture()`（3×2，Root 态 `ds2.select(item/amount, amount>=2000)` 无主格引用）；离线用 CSV 文件数据集复刻，进 `engine_grid_compare` 20 夹具主对拍全绿，Java 侧 `--verify` round-trip 同步闭合 |
| M5-05 | `filtercross` 多对多 | `rpx-engine` | **探针完成 → 带账**（2026-10-15）：8 布局实证（从格带/主带/从从带 × 裸列/限定名 × 无选项/`@r`）+ 跟踪子类 + 解析树三路取证 ⇒ 过滤语义依赖的 `ExtExtendCell._$4` 在 filtercross 求值期间恒为 null（全源唯一写入点在其自身求值**之后**）⇒ 本版运行时不可驱动。按「无夹具不实现」纪律不实现，理由与复现路径见 §2.6 末、§6.3 ⑧ |
| M5-06 | Root 态裸列 `ds.col` = 首行值（修 `UnsupportedExpr`） | `rpx-engine`（`expr/dsfn.rs` `root_current_data`） | ✅ **已闭合**（随 M5-01）：`dsfn::root_current_data` 实现产品 currentRow 语义；`select(item:item)` → paramTypeError 单测 + 裸列首行值单测双钉 |
| M5-07 | 维度对齐（交叉表同数据集双方向） | `rpx-engine` | ✅ **已闭合**（2026-10-16）：探针 `ProbeCrossAlign`（4 布局）钉死产品语义——**交叉点格被复制为（横实例×纵实例）矩阵，相对引用分别跟随两方向实例**（B1 横跟列、A2 纵跟行）；三夹具 `engine_crossalign{,_lit,_ds2}` 进 `engine_grid_compare` 主对拍全绿（23 夹具）；Rust 侧 M2 复制循环 + M5-03 heads 链天然对齐，无需新实现；细节见 §6.4 |

依赖序：M5-01 → M5-02 → M5-03（机制 M2 实装，本里程碑验收）→ M5-04（夹具闭环）→ M5-05（探针→带账）/M5-06 并行 → M5-07 后置。

## 6. 边界与带账

### 6.1 明确不做

- **不做笛卡尔积/整合运算**：产品语义是求值时关联（§1），Rust 不建中间连接表。
- **不做 SQL 下推关联**：`ds.select` 的过滤在内存行上求值（产品同构）；
  「关联尽量下推到 SQL」是模板设计建议（讲座行动序 #1 注），不是引擎任务。
- **不做 `esproc`/`spl` 表达式函数**（`FunctionLib.java:131-132`）：SPL 引擎另立里程碑。

### 6.2 继承带账（spec-m3 §5 仍然有效）

分页高级 / 跨页合并 / 文本换行 / M2-22 残余 / 样式宏 / 两扩展格同行 11×5 等
沿用 `spec-m3.md` §5 与 `spec-m2.md` §6.2 的结论，本 spec 不重复。

### 6.3 新带账（本立项核出）

1. **`ds.filter` / `ds.filterone` 运行时不认**：IDE 语义面板有这两个操作
   （`JPanelDataSet.java:94-97`），但 `FunctionLib` 未登记（`:106-121`），
   模板写了一定落空。等同维表关联的可用写法是 `select(..., num=1)`。
2. **共带干扰**（§3.3）：同带多 DS 函数格的 current 互相污染有确定性但未完全
   建模的顺序依赖；夹具按「一模板一语义点」隔离（已定为 M5-04 纪律）。
3. **`Group.filter` 二分快速路径的有序性前提**（§2.7）：option `b` + 等值条件时
   产品假设行已按该列有序。**已探针（2026-10-20，`ProbeM5Rest` P4）**：
   - **有序域等价**：按该列升序时二分与线性同值（`sum@b(id,g=="B")` =
     `sum(id,g=="B")` = 21；`A` 组 = 4）⇒ 夹具 `engine_groupb` 钉这条等价域。
   - **无序域产品是坏的**：数据 `B A B A B` 时 `sum@b` = **9**（二分命中点所在的
     连续段），线性 = 21。**本仓一律走线性，故在该域与产品有意偏差** ——
     理由：`b` 是「数据已排序」前提下的优化，复刻它等于复刻一个依赖数据布局的
     错误结果，且需要对齐产品 `_$3` 的相邻性模型。
   - 顺带核出：**降序数据会查不到**（`B B B A A` 下 `sum@b(id,g=="A")` = 0）。
4. ~~**`;` 多键排序「全键相等」时的次序**~~ ✅ **已闭合（2026-10-20）**：
   探针 `ProbeM5Rest` P1 实测 —— 全键相等（k1=K,k2=Z）时产出 = **输入序**
   7/3/9/1/5（对照 `;id` = 1/3/5/7/9），与手写归并（`SortDsValue.sort:384-401`，
   首键 `Collections.sort` + 逐段次键重排，两趟都稳定）的推导一致 ⇒
   **本仓稳定多键比较等价，不需要手写归并**。夹具 `engine_tiesort` 已入主对拍。
5. ~~**Root 态 currentRow 光标只建模了恒 1**~~ ✅ **已闭合（2026-10-16）**：
   探针 `ProbeDsOffset`（`re_work/tmp_probe/dsoffset/`）9 布局实证 `ds[i].列`
   （`DSOffset.java:33-44` 改写光标 + `DataSet.java:395-411` 的 Group 分支
   按 `_$5` 取行）：Root 态读第 i 行、越界（`<1`/`>rowCount`）⇒ `Null` **不报错**；
   Row 态偏移被**忽略**（Row 分支不看 `_$5`，纵带实测每行 = 当前行值）；
   非整下标报 paramTypeError（产品消息键）；`ds[1+1]` 非叶子运算接受；
   裸 `ds[i]` 产品返回 DataSet 对象（无实用对拍形态）⇒ eval 显式报错不猜。
   落地：`Expr::DsOffsetColumn`（parse_bp 后缀按 `.列` 形态分流）+ `dsfn::ds_offset_column`；
   单测 `expr/tests_offset.rs` 8 条。⚠ 顺手修 `split_ds` 数字守卫：`1.5` 曾被
   错切 `DsColumn{"1","5"}`（产品 `Expression.java:174` 是「存在才切」，
   本仓取「点号前纯数字 ⇒ 不切」的形态近似）。⚠ 与产品**有意偏差**：产品按
   left 求值结果分派，本仓按后缀形态分流（Value 无 DataSet 变体）。
6. ~~**Row 态过滤非布尔**~~ ✅ **已闭合（2026-10-20）**：产品抛 `error.Row.test`
   （`Row.java:88-94`），探针 `ProbeM5Rest` P2 三形态（常量 `5` / `id+1` / 字符串）
   的**内层文本逐字一致**：「记录过滤条件应该返回布尔值」（外层
   `单元格A2中有错误: 表达式：…中: ` 是格级外壳，本仓不建模）。本仓
   `expr/num.rs::as_bool` 已按同一措辞出口；夹具 `engine_rowtest`（错误型）+
   `tests/m5_residual.rs` 钉子（钉措辞 + 错误类型）双保险。
7. ~~**合并段断在分组头行时的「上一页伸长」未复刻**~~ ✅ **已结案（2026-10-20，
   探针 `ProbeRowHeight` + `ProbeMergePage` + 夹具 `engine_mergepage`）**：
   分两步把这条账从「未探针」变成「有据结案」。

   **第一步 · 行高与分页口径标定**（`ProbeRowHeight`，8 组行高）：
   - `IRowCell.setRowHeight(x)` 的单位是 **mm**（不是 pt）；
   - `ReportUtils2.getPagerInfo(rpt, 595, 842, 1)` 的 595/842 是 **pt**（A4 纵向），
     842pt ≈ 297mm ⇒ **每页可用高度 297mm**；
   - 每页行数 = 累计高度不超过 297mm 的最大行数（不扣页眉边距）；
     实测：20mm→12 行全一页、30mm→9+3、50mm→5+5+2、100mm→2×6、150mm→1×12；
   - `rowNumPerPage` 是常量 20，**与真实切页无关**，别看它。

   **第二步 · 分组头行的三条可达路径全部被产品拒绝**：
   | 布局 | 产品行为 |
   | --- | --- |
   | 显式合并区（`setMergedArea`）盖住 `rowType=-90` 的行 | 报错「表头合并设置不合法」 |
   | **带内行本身**设 `rowType=-90`（会随实例复制到每一行） | `SimplePager.createPages` **NPE** |
   | 两行带（数据行 + 分组头行 `left_head=A2`）+ mergeSame | mergeSame **不合并**（相邻同值才合并，分组头行插在中间断了相邻性） |

   ⇒ **原描述的形态在产品侧不可驱动**，按「无夹具不实现」纪律不实现
   （与 ⑧ `filtercross` 同类的有据结案）。

   **第三步 · 可驱动形态下产品与本仓一致**：`engine_mergepage`（100mm 行高 ⇒
   每页 2 行，A 段 3 行跨第 1/2 页、B 段跨第 2/3 页）实测产品把每页的跨度
   **裁成交矩形维度**：A 段在第 1 页报 `row_merge=2`、第 2 页只剩 1 行 ⇒ 不上报；
   B 段在末页报 2。这与本仓 `calc/merged_view.rs::page_slots`（矩形求交）
   **逐格一致**，夹具已进 39 夹具主对拍全绿 ⇒ 现有实现无需改动。
8. **`filtercross` 在本版运行时不可驱动**（M5-05 探针核出，证据链）：
   - **依赖字段**：过滤语义读 `cell.getDSCurrent()` = `ExtExtendCell._$4`；
   - **写入点全集**（全源 grep 确认无第三方写入）：`ExtExtendCell.java:463`
     （格自身非 List 结果写回）与 `:440`（扩展写回收尾循环）——**都在该格
     求值之后**；`_$3`（另一个候选机制，`setDSCurrentLeftHead` 消费）在本
     jar 中**无任何写入点**（dead 字段，`clone()` 只置 null）；
   - **实证**：探针 `ProbeFilterCross`（跟踪子类注册替换 + 反射读私有态）
     8 布局（主带/从格带/从从带 × 裸列名/`ds2.` 限定名 × 无选项/`@r`）：
     `filtercross` 每实例恰被调用一次，`cell._$4` **恒 null** ⇒ 恒走
     `getNullGroup()`；`@r` 变体证明函数可达（返回根组原样），但过滤分支
     需要同一格实例**二次求值**（第一次落 `_$4`、第二次才能读到）——本版
     调度器不产生二次求值；
   - **处置**：M5-05 转带账，Rust 侧不实现（「无夹具不实现」纪律——
     没有能区分实现的语义出口，实现无对拍基准）；
   - **复现**：探针源码 `re_work/tmp_probe/filtercross/`（rpx 仓外），
     运行 `run.ps1`（cwd=tests/fixtures）；完整记录见 RUNLOG §42。
9. **交叉点裸列不对齐**（M5-07 探针 B 布局核出）：19×18 交叉展开下，交叉点
   写裸列（`ds1.amount`）产品给出 **null/首行值的混合态**（锚点 null、横向第二
   实例 10），不是按 (region, product) 组合查行——与带账 ② 共带干扰同族；
   本仓 Row/Root 态裸列语义未复刻此混合态，夹具不落该形态（对齐写法 =
   相对引用双头，C 布局）。

### 6.4 M5-07 闭环记录（2026-10-16）

**第一步核掉一个立项假设**：产品运行时**没有 `cross()` 函数**——`FunctionLib.java:114`
登记的 DS 函数只有 `filtercross`；「维度对齐」的产品形态就是 19（横向）× 18（纵向）
同带交叉展开，靠 M2 扩展机制承载。

**探针 `ProbeCrossAlign`**（scratch，`re_work/tmp_probe/crossalign/`；CWD=tests/fixtures）
4 布局实证（数据集：region 2 值 × product 3 值真交叉，`select(x:1,,,x)` 去重）：

| # | 布局 | 产品真值 |
| --- | --- | --- |
| A | 19h(region) × 18v(product)，交叉点字面量 | 网格 5×4；交叉点复制为 **2×3 矩阵**（每格同字面量）——纯拓扑成立 |
| B | 同 A，交叉点 = `ds1.amount`（裸列） | **不对齐**：锚点 null、(2,3)=10（首行值）——共带污染近亲，登记带账 ⑨ |
| C | 同 A，交叉点 = `B1+"/"+A2`（相对引用双头） | **完美对齐**：6 格 = 华北/A…华东/C；6 格的 `left_head`/`top_head` 均指向**源头格** (2,1)/(1,2)（dump 实测），实例化靠复制时 current 沿链传递——对齐必须靠相对引用 |
| D | 19h(ds2.x) × 18v(ds1.region)，交叉点字面量 | 网格 4×5，结构与 A 同构（跨数据集无分歧） |

**夹具三件套**（一模板一语义点，`EngineModelsM5.crossAlignFixture(name)`）：
`engine_crossalign`（C 核心语义）/ `_lit`（A 拓扑对照）/ `_ds2`（D 跨数据集对照），
`engine_crossalign{,2}.csv` 检入；进 `engine_grid_compare` 主对拍（23 夹具）。

**Rust 侧结论**：**零新实现**——`engine_output_matches_every_fixture_cell_by_cell`
23 夹具全绿。M2 的扩展复制循环（横扩复制右格、纵扩复制下格，交叉点被两方向
先后复制成矩阵）+ M5-03 的 heads 链（交叉点的 left_head/top_head 自动挂接、
相对引用沿链解析）的组合**天然覆盖**了交叉点双实例化。此前「带账待实现」的
预估（归并/位图机制）被证伪——产品用的是结构复制，不是值对齐算法。

## 7. 变更记录

- **v0.1（2026-10-15）**：立项。反逆向定案 §2（参数矩阵/双态/显式左主格/
  cross/filtercross/filter 快速路径）；金仓探针 9 模板 §3；三分类映射 §4；
  任务表 M5-01..07 §5；新带账 3 项 §6.3。探针产物：`ProbeCross.java` +
  `out/cross_*_product.json`（9 份，round-trip 全绿）。
- **v0.2（2026-10-15）**：**M5-01 + M5-06 闭合**。
  - 落地：`expr/ast.rs` `Param` 树 + `Call.param_tree`（Box 断环）；
    `expr/call.rs` `build_param`（`ParamParser._$1:35-64` 递进复刻）；
    `expr/dsselect/{mod,decode,rows}.rs`（`DSSelect.calculate:27-209` 逐行移植
    + `ParamInfo2.parse` + `SortDsValue.sort` + `Group.filter/distinct`）；
    `expr/dsfn.rs` `row_path` 抽公共尾段 + `root_current_data`。
  - 实证新出口（本版核出，均已写进代码注释）：产品 `DataSet.getCurrentData:395-410`
    的 Root 态走 currentRow 光标（初值 1、`setCurrent` 重置，`DataSet.java:31/362-364`）
    ⇒ `select(item:item)` 次序表达式求值为首行 Text ⇒ paramTypeError；
    `DSVariable.calculate` 与 `UnknownSymbol.setDS` 同走 `getCurrentData`（两侧不分叉）。
  - 坑（本版踩并修复）：lexer `is_delimiter` 原不含 `;` ⇒ `item;emp_id` 被粘成一个
    `Word`（产品的 `;` 切分发生在字符串级、扫描之前，本仓同趟处理必须硬断）——
    已修并登记为 `token.rs` 第 4 处刻意偏差。
  - 测试：`expr/tests_select.rs` 13 个（四形态/6 种产品报错/T3 六变体/num-z/空单果/
    `;` 多键/Row 四钉/哨符闸/指针恢复）；workspace 450+ 全绿；G1–G8 全绿。
  - 带账新增 ④⑤⑥（§6.3）。
- **v0.3（2026-10-15）**：**M5-02 闭合**。
  - 落地：`calc/merged_view.rs`（新；双视图——全局逐槽 `global_slots` 供服务面
    快照、页内逐槽 `page_slots` 供 emit 对拍）+ `calc/run.rs` 接线
    （`export_grid` 改返 `(CalcGrid, Vec<MergedArea>)`）+ `arena/calc_grid.rs`
    新增 `merged_pages` 字段 + `emit.rs` `merged_section` 退化为纯序列化。
  - **跨页合并拆分**（`SimplePager._$1:1624-1638` 行向 / `:2316-2335` 列向的
    deepClone 锚点 + `setEndRow`/`setEndCol` 收窄）等价建模为
    「合并区矩形 × 页矩形求交」：交矩形维度即该页所有相关槽报的跨度；
    `1×1` 交矩形跳过（`CalcMergeJson.java:48-50` 同口径）。
  - 关键设计决策：**双视图**而非单侧投影——`rpx-server` 快照（`merge_data`）
    与 `build_paged` 的跨页拆分需要全局坐标；emit 对拍需要页局部坐标
    （`CalcMergeJson.renderMerged` 读页格）。两次失败尝试已记入 RUNLOG §39.3
    （emit 侧投影算错延续槽宽度；锚点去重单视图破坏单页逐槽形态）。
  - 测试：`merged_view` 4 单测 + `emit` 2 钉子（页内 12 条真值复刻 + null 值格）
    + `expr/tests_select.rs` 电流语义钉子（主格实例行切换，15 测试全绿）；
    `cross_dataset_compare` 8 夹具（T1 ×1 + T2 ×1 + T3 Root 变体 ×6）逐格逐页
    零差异；workspace 全绿；G1–G8 全绿。
  - 变异测试证据（两条均实测后还原）：① `resolve_cell` 静态化（丢 current 链）
    ⇒ **仅**电流语义钉子红；② `page_slots` 跨度不裁剪 ⇒ `merged_view` 2 钉子
    + 端到端 `extra=1 mismatch=8`。
  - 带账新增 ⑦（§6.3：分组头行合并区伸长）；`m2-engine-design.md` §15.5
    「跨页合并格拆分」由 ❌ 改 ✅。
- **v0.4（2026-10-15）**：**M5-03 闭合**。
  - 机制已在 M2 头图工作实装（`heads/find.rs` 分支 A 读 `declared_left_head`/
    `declared_top_head` 字符串、`ext_algo/envelope.rs` `expand_left_head` 跨度递归
    纳入左子格、`ext_algo/row.rs` `extend_row` 复制循环）。本里程碑只做**验收固化**：
    把 T4（`cross_nest` 多匹配展开 14 行）+ T5（`cross_nest_vals` 无匹配仍 1 行）
    并入 `cross_dataset_compare` 主对拍载体（现 10 夹具）。
  - **变异测试证明显式 head 承重**：临时摘除 `heads/find.rs` 分支 A（模拟
    slot 30/31 未实现）⇒ T4 从 14 行塌成 9 行（missing=33 / extra=25 / mismatch=11）、
    T5 同塌（missing=49 / mismatch=59）；还原后全绿。证明 T4/T5 走的是**显式**
    左主格路径，不是默认近邻规则误判（`engine_head_left/top` 只覆盖 fallback
    到 cell00 的默认路径，不承重）。
  - 测试：`cross_dataset_compare` 10 夹具逐格零差异；`memory_replica_matches_
    spec_facts` 同绿；workspace 全绿；G1–G8 全绿（无新代码，仅固化夹具 + 文档）。
  - 说明：M5-03 无独立实现提交（机制来自 M2），随本里程碑的夹具固化 + spec 升级
    一并进 `spec-m5.md` v0.4；若需要可补一枚 `test` 型提交锚定 T4/T5 入网。

- **v0.5（2026-10-15）**：**M5-04 闭合（产品夹具闭环）**。
  - 离线（无金仓）用 **CSV 文件数据集**复刻嵌套主子表，作为 M5-03 显式左主格机制的
    「一等夹具」闭环：`EngineModelsM5.crossFixture()` 建 4×6 模板，r3 主格
    `ds1.select(id/name/dept/salary)`，r4 从格 `ds2.select(item/amount, emp_id==A3)`
    并 `setLeftHead("A3")`，title 跨 6 列合并；`crossRootFixture()` 建 3×2 模板，
    Root 态 `ds2.select(item/amount, amount>=2000)`（无主格引用，走 Root 态求值路径）。
    数据集 `ds1=engine_cross.csv`（5 员工）/ `ds2=engine_cross.csv2`（6 奖金：张伟·赵敏各 2 项、
    孙丽 0 项），`attachFileDataSet` 由 `GenEngineFixtures` 主管线写入。
  - **关键修复**：`engine_cross.csv2`（ds2）初版是 UTF-8 写出，而 `.rpx` 按产品默认 GBK
    解码 ⇒ Rust 侧严格 GBK 解码抛「GBK 尾字节越界」。转码为 GBK+CRLF 后，Rust 与
    Java 同口径解码；并**重跑 `GenEngineFixtures` 生成侧**把 `.expected.json` 真值从
    「UTF-8 误读乱码」纠回正确中文（例如 `\u5168\u52e4\u5956` 全勤奖，旧真值为
    `\u934f\u3125\u5adf\u6fc2\ufffd` 乱码）。
  - 验收：`loader.rs::all_names` 由 18 → 20（加 `engine_cross`/`engine_cross_root`），
    `engine_grid_compare` 主对拍链自动覆盖；`engine_output_matches_every_fixture_
    cell_by_cell` 对 20 夹具逐格（值/类型/source 六子字段/left_head/top_head/网格尺寸/
    `merged.cells` 合并跨度/分页）零差异全绿；Java 侧 `--verify .` 全 20 夹具 round-trip 闭合。
  - 网格真值快照：`engine_cross` = 18 行 × 6 列；主格带 `sub_cells:5`，从格带按员工
    匹配展开（张伟 2 / 王强 1 / 赵敏 2 / 孙丽 0），`left_head` 全指向主格 A3（src_row:3,
    src_col:1），`merged` 段含 (4,6)/(6,6) 等金额带 row_merge=2 跨度。
  - 测试：`engine_grid_compare` 20 夹具全绿；`cross_dataset_compare` 10 夹具不变；
    G1–G8 全绿（M5-04 无引擎新源码，仅夹具 + 数据集 + 文档增量，故 G4 不触发新超限）。

- **v0.6（2026-10-15）**：**M5-05 探针完成 → 带账（不可驱动）**。
  - 反逆向先行：`FunctionLib.java:113` 确认 `filtercross` 已登记（与
    `filter`/`filterone` 不同）；旧版转换层 `transfer/ExpParse.dsFilter` 证实
    表达式形态 `filtercross(exp)` / `filtercross@r(exp)`（旧语法第二参
    `true` → `@r`），与源码 option `'r'` 一致。
  - 探针（`re_work/tmp_probe/filtercross/`，rpx 仓外）：`ProbeFilterCross`
    （8 布局）+ `TracingFilterCross`（注册替换 `FunctionLib._$3` 的
    `filtercross` 项为跟踪子类，记录 `this.ds.current`/`cs.current`/
    `cs.current._$4`/option/结果）。
  - 实证结论：`cell._$4` 在该函数求值期间**恒 null**（每实例恰求值一次；
    `_$4` 全源唯一写入点 `ExtExtendCell.java:463/:440` 均在其自身求值
    **之后**）⇒ 过滤分支（Row 匹配 / Group many-to-many 合并）不可达，
    仅有恒等（`filterExp == null`）、`getNullGroup()`、原样 Group 返回
    三类出口。`_$3`（另一候选）在本 jar 无写入点（dead）。
  - 处置：不实现（无夹具不实现）；新带账 §6.3 ⑧（含证据链与复现路径）；
    完整探针记录见 RUNLOG §42。M5 剩余：M5-07（cross()）维持带账后置。
