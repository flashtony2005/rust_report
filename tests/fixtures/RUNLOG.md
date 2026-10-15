# RUNLOG — M1 真实 .rpx 测试夹具（Java 侧产出）

> 生成者：mvp-dev-expert-team-backend（贝洛奇）· 2026-10-12
> 产物目录：`F:/project/report/rpx/tests/fixtures/`
> 契约依据：`docs/spec-m1.md` §5（格式契约）· `references/01-standards/test-discipline.md`

---

## 1. 构建与运行命令（已在本机实测通过）

### 编译（JDK 21 → Java 8 字节码）

```bash
cd F:/project/report/rpx/tests/fixtures
"D:/Tools/jdk-21.0.2/bin/javac.exe" --release 8 -Xlint:none \
  -cp "F:/project/report/reportlite/lib/*" \
  *.java -d out
```

- 源码按职责拆 4 个文件，单文件均 < 300 行（门禁 `find . -maxdepth 1 -name '*.java' | xargs wc -l`）：
  | 文件 | 行数 | 职责 |
  |------|------|------|
  | `GenFixtures.java` | 126 | 入口：装配、写出、回读校验（零业务逻辑） |
  | `Fixtures.java` | 297 | 三个夹具的模型构造 |
  | `ExpectedJson.java` | 276 | 内存模型 → expected.json 映射 |
  | `Json.java` | 144 | 零依赖 JSON 写入器 |
- `javap -v` 实测 `major version: 52`（Java 8），符合产品 JRE 1.8.0_151。
- 包名 `rpx.fixtures`；运行主类为 `rpx.fixtures.GenFixtures`。

### 运行（产品自带 JRE，与真实运行环境一致）

```bash
cd F:/project/report/rpx/tests/fixtures
"F:/project/report/common/jre/bin/java.exe" \
  -cp "F:/project/report/reportlite/lib/*;out" \
  rpx.fixtures.GenFixtures .
```

> Git Bash 中 classpath 分隔符 `;` 必须整体置于双引号内，否则被 shell 解释为命令分隔符。

### 回读自校验（强烈建议 Rust 侧跑测试前先跑这个）

```bash
"F:/project/report/common/jre/bin/java.exe" \
  -cp "F:/project/report/reportlite/lib/*;out" \
  rpx.fixtures.GenFixtures --verify .
```

`--verify` 用产品自带 reader（`ReportUtils.read`）把每个 `.rpx` 读回内存模型，
重新渲染 expected.json 并与磁盘上的文件逐字符比对。**当前三夹具全部 PASS**，
即 `.rpx` 字节与 `expected.json` 严格等价 —— 这是 Rust 解析器能对拍的硬证据。

---

## 2. 产物清单

| 文件 | 字节 | 说明 |
|------|------|------|
| `simple.rpx` | 3626 | 5×4 基本矩阵 |
| `simple.expected.json` | — | 对应黄金预期 |
| `merged.rpx` | 5196 | 三种合并区 |
| `merged.expected.json` | — | 对应黄金预期 |
| `full.rpx` | 7491 | 全属性覆盖 |
| `full.expected.json` | — | 对应黄金预期 |
| `GenFixtures.java` / `Fixtures.java` / `ExpectedJson.java` / `Json.java` | — | 生成器源码（留档） |

### 文件层字节实测（`od -An -tu1`）

| 夹具 | 头 4 字节 | payload 长度 (BE int32) | 总字节 | 页脚 8 字节 |
|------|-----------|------------------------|--------|-------------|
| simple | 82 81 81 82 | 3610 | 3626 | 82 81 81 82 82 81 81 82 |
| merged | 82 81 81 82 | 5180 | 5196 | 82 81 81 82 82 81 81 82 |
| full | 82 81 81 82 | 7475 | 7491 | 82 81 81 82 82 81 81 82 |

校验式：`总字节 == 8 + payload长度 + 8`，三夹具均成立。

---

## 3. 夹具构造参数（Rust 侧写测试的直接输入）

### 3.1 `simple.rpx` — `new ReportDefine2(5, 4)`

矩阵维度：`rows=6, cols=5`（含 0 号虚格边）；逻辑区 `row_count=5, col_count=4`。

**行高**（`setRowHeight`，索引 1..5，默认 8.0）：

| 行 | 1 | 2 | 3 | 4 | 5 |
|----|---|---|---|---|---|
| 高 | 24.0 | 16.0 | *8.0* | 20.0 | *8.0* |

**列宽**（`setColWidth`，索引 1..4，默认 25.0）：

| 列 | 1 | 2 | 3 | 4 |
|----|---|---|---|---|
| 宽 | 80.0 | 60.0 | 60.0 | *25.0* |

**单元格**（16 个非空格；斜体 = 未显式设置、保持模型默认因而不出现在 JSON）：

| (r,c) | value | 类型 | 显式设置的样式 |
|-------|-------|------|----------------|
| (1,1) | `"ReportLite"` | String | fontName=Arial, fontSize=14, bold, hAlign=-47(CENTER) |
| (1,2) | `"Sales"` | String | fontName=Arial, bold, hAlign=-47 |
| (1,3) | `"Region"` | String | bold, hAlign=-47 |
| (1,4) | `"Qty"` | String | bold, hAlign=-46(RIGHT) |
| (2,1) | `"North"` | String | — |
| (2,2) | `100` | Integer | — |
| (2,3) | `"East"` | String | — |
| (2,4) | `200` | Integer | — |
| (3,1) | `"South"` | String | — |
| (3,2) | `150` | Integer | — |
| (3,3) | `"West"` | String | — |
| (3,4) | `120` | Integer | — |
| (4,1) | `"Total"` | String | bold |
| (4,2) | `370` | Integer | bold |
| (5,1) | `"Avg"` | String | — |
| (5,2) | `92.5` | Double | hAlign=-46 |

params=[]、datasets=[]。print_setup / scalars 为**产品默认值**（见 §5）。

### 3.2 `merged.rpx` — `new ReportDefine2(6, 5)`

矩阵维度：`rows=7, cols=6`；逻辑区 `row_count=6, col_count=5`。

**合并区清单**（3 个；锚点设值，非锚点在 .rpx 中落为空记录）：

| 形状 | 锚点 (begin_row, begin_col) | (end_row, end_col) | 锚点 value |
|------|------------------------------|--------------------|-----------|
| 1×3 横向 | (1, 1) | (1, 3) | `"Title"` |
| 2×2 方块 | (2, 2) | (3, 3) | `"Merged2x2"` |
| 3×1 纵向 | (4, 4) | (6, 4) | `"Side"` |

锚点附带样式：(1,1) fontName=Arial/fontSize=14/bold/hAlign=-47；(2,2) hAlign=-47/vAlign=-31。

**行高**：`[22.0, 18.0, 8.0, 8.0, 8.0, 8.0]`（索引 1..6）
**列宽**：`[70.0, 25.0, 25.0, 40.0, 50.0]`（索引 1..5）

**合并区外的普通单元格**（4 个）：

| (r,c) | value | 类型 |
|-------|-------|------|
| (2,5) | `99` | Integer |
| (5,1) | `"A"` | String |
| (5,2) | `"B"` | String |
| (5,3) | `"C"` | String |

合计 7 个非空格。

### 3.3 `full.rpx` — `new ReportDefine2(7, 6)`

矩阵维度：`rows=8, cols=7`；逻辑区 `row_count=7, col_count=6`。

**extendMode 全覆盖**（4 个常量各一格）：

| (r,c) | value | extendMode | 常量 |
|-------|-------|-----------|------|
| (1,1) | `"ExtendDefault"` | 16 | EXTEND_DEFAULT |
| (2,1) | `"ExtendNone"` | 17 | EXTEND_NONE |
| (3,1) | `"ExtendVertical"` | 18 | EXTEND_VERTICAL |
| (4,1) | `"ExtendHoriz"` | 19 | EXTEND_HORIZONTAL |

> (1,1) 的 16 等于模型默认，故 JSON 中不出现 `extendMode` 字段（见 §4 的"缺省即默认"规则）。

**value ≠ dispValue**（2 格）：

| (r,c) | value | dispValue |
|-------|-------|-----------|
| (1,2) | `"1000"` (String) | `"1,000"` |
| (2,2) | `2000` (Integer) | `"2,000"` |

**样式全覆盖**：

| (r,c) | value | 显式样式 |
|-------|-------|----------|
| (1,3) | `"Styled"` | fontName=Arial, fontSize=16, bold, italic, underline, textWrap, foreColor=16711680(0xFF0000), backColor=16776960(0xFFFF00), format="#,##0" |
| (2,3) | `"Rotated"` | rotationAngle=45.0, indent=2.0 |
| (3,3) | `"Hidden"` | visible=false |

**合并区**（1 个）：2×2，锚点 (5,2) → (6,3)，value `"MergedFull"`，hAlign=-47。

**值类型覆盖**：(5,4)=`3.14159` (Double)、(6,4)=`true` (Boolean)、(7,1)=`"Footer"` (String)。

**行高**（索引 1..7，全部显式设置）：`[28.0, 18.0, 18.0, 14.0, 16.0, 14.0, 22.0]`
**列宽**（索引 1..6，全部显式设置）：`[90.0, 70.0, 70.0, 50.0, 50.0, 60.0]`

**ParamMetaData**（2 个参数）：

| name | description | param_type | data_type | value |
|------|-------------|-----------|-----------|-------|
| `startDate` | `Start date` | 0 | 11 (DT_STRING) | `"2024-01-01"` |
| `topN` | `Top N rows` | 0 | 1 (DT_INT) | `"10"` |

**DataSetMetaData**（1 个数据集，FileDataSetConfig）：

| name | data_source_name | factory_class | file_name |
|------|------------------|---------------|-----------|
| `ds1` | `ds1` | `com.raqsoft.reportl.dataset.FileDataSetFactory` | `data.csv` |

**PrintSetup**（显式设置）：paper=9 (A4)、paper_width=595.0、paper_height=842.0、
orientation=1 (portrait)、left_margin=20.0、right_margin=20.0、top_margin=25.0、
bottom_margin=25.0、copy_count=1。

**标量尾巴**（全部显式设置，均非默认）：

| 字段 | 值 | 模型默认 |
|------|-----|----------|
| disp_ratio | 100 | 133 |
| unit | 0 (inch) | 1 (mm) |
| tip | `"full fixture tip"` | null |
| notes | `"full fixture notes"` | null |
| chart_style_name | `"chartStyle1"` | null |
| future_cell_num_exp | `"=1"` | null |
| image_max_width | 800 | 0 |
| image_max_height | 600 | 0 |
| page_border_mode | 1 | 1 |
| report_style_name | `"reportStyle1"` | null |
| validity_script | `"validityScript1"` | null |
| big_data_set_name | `"bigDataSet1"` | null |
| password | `"fixture-secret"` | null |
| submit | 1 | 0 |
| high_precision | false | true |
| threshold | 5 | -1 |

---

## 4. expected.json 契约（Rust 侧必读）

### 4.1 维度语义

- `rows` / `cols` = **矩阵维度，含 0 号虚格边**（= `getRowCount()+1` / `getColCount()+1`）。
  - simple: rows=6, cols=5；merged: rows=7, cols=6；full: rows=8, cols=7
- `row_count` / `col_count` = 逻辑行列数（= rows-1 / cols-1），为方便额外提供。
- `row_heights` 长度 = `rows - 1`，索引对应行 1..row_count。
- `col_widths` 长度 = `cols - 1`，索引对应列 1..col_count。
- **验收式：`len(row_heights) == rows-1` 且 `len(col_widths) == cols-1`**（已实测成立）。

### 4.2 单元格收录规则（"非空格"）

一个单元格出现在 `cells` 中，当且仅当它的**任一可读属性不等于模型默认值**。
判定与渲染共用同一套常量（`ExpectedJson.isBlank` 与 `emitCell` 严格镜像），
因此二者不会漂移。默认值清单：

| 属性 | 默认值 |
|------|--------|
| value / dispValue | null |
| extendMode | 16 (EXTEND_DEFAULT) |
| hAlign | -48 (HALIGN_LEFT) |
| vAlign | -31 (VALIGN_MIDDLE) |
| fontName | `"宋体"` |
| fontSize | 12 |
| bold / italic / underline | false |
| visible | true |
| textWrap | false |
| foreColor | -16777216 (0xFF000000, black) |
| backColor | -1 (0xFFFFFFFF, white) |
| rotationAngle / indent | 0.0 |
| format | null |

> **Rust 侧含义：JSON 中**缺失**的字段 = 模型默认值，不要报错、不要当成 0。**

### 4.3 合并区：仅锚点记录

`fillRecord` 会把锚点单元格对象**铺满整个合并区**，因此回读后的模型里合并区每个
槽位持的都是锚点对象。expected.json 只收录锚点槽位（`(row,col) == (begin_row,begin_col)`），
非锚点槽位一律跳过 —— 这也是 expected.json 能从 `.rpx` 稳定重建的原因。

Rust 侧应对应实现 AC-04：合并区各处 `CellId` 相同，且 `MergedArea` 清单与
expected.json 的锚点记录一致。

### 4.4 值类型编码（记录层 `writeObject(o, true)` 标签）

| Java 类型 | 标签字节 | expected.json 形态 |
|-----------|---------|--------------------|
| null | -1 | `null` |
| String | 1 | JSON string |
| Integer | 4 | JSON number（整数） |
| Long | 5 | JSON number（整数） |
| Double | 9 | JSON number（浮点） |
| Float | 10 | JSON number（浮点） |
| Boolean | 6 | JSON `true` / `false` |
| BigDecimal | 2 | M1 先按字符串透传（本批夹具未使用） |

### 4.5 字段命名

snake_case（`disp_ratio` / `image_max_width` / `high_precision` / `begin_row` …）；
单元格级样式沿用接口风格的驼峰（`dispValue` / `extendMode` / `hAlign` / `fontName` /
`fontSize` / `foreColor` / `backColor` / `rotationAngle` / `textWrap`），
与 `INormalCell` getter 名一一对应，便于对照。数值为 JSON number，字符串原样。

---

## 5. 已知事实与坑（给 Rust 侧）

1. **PrintSetup 永远非空且带产品默认值。** `ReportDefine` 构造时
   `_$25 = new PrintSetup()`，所以即使不设置，序列化也会写出 PrintSetup 记录。
   simple / merged 的 `print_setup` 因此是产品默认值：
   paper=9、paper_width=210.0、paper_height=297.0、orientation=1、
   left/right_margin=19.0、top/bottom_margin=25.0、copy_count=1。
   **解析侧必须能吃掉这条记录，不能假设它为 null。**
2. **`setUnit` 会重算所有行高列宽。** 费率：unit=1 → 25.4，unit=0 → 72.0，其他 → 1.0。
   full.rpx 因此先 `setUnit(0)` 再显式设置全部行列尺寸，最终 JSON 中无派生浮点
   （全部为 28.0/18.0 这类整齐值）。Rust 侧只按字节读，无需重算。
3. **每个 NormalCell 槽位都是一条完整记录**（非空记录），不是长度 ≤0 的空槽；
   只有**合并区的非锚点槽**才是空记录（`writeRecord(null)` → 长度 -1）。
   simple/merged/full 中未设置任何属性的普通格，序列化出的是一条全默认值记录。
4. **`Area(int r1,int c1,int r2,int c2)` 构造器参数名有误导性**：实际赋值是
   `beginRow=r1, endRow=c1, beginCol=r2, endCol=c2`。生成器因此统一走
   `new Area()` + `setArea(br, bc, er, ec)`（后者参数顺序符合直觉）。
   Rust 侧按字节顺序读，与此无关，但若日后加 Java 侧工具需注意。
5. **RowCell / ColCell 行高列宽始终写出**（`writeFloat`，无条件），默认 8.0 / 25.0。
6. **`reportlite` 版矩阵后直接是 PrintSetup**，无 PrintSetup 前的 `byte(_$31)`
   （全功能 report 产品版才多这一个字节）。本批夹具由 reportlite.jar 产出，
   按 spec §5.2 字节对齐。
7. **`ReportDefine2` 未覆写 `serialize()` / `fillRecord()`**（只覆写 OIS 的
   writeExternal/readExternal），所以 `.rpx` 字节格式就是 `ReportDefine` 的，
   与 spec §5.2 完全一致。

---

## 6. 未设置项及原因

| 未设置项 | 原因 |
|----------|------|
| `_$6` 属性 map（cpSize 区） | `ReportDefine` 无公开 setter 直接灌入任意 map；空 map 序列化为 `cpSize=0`，解析侧已覆盖空路径 |
| `_$5` 记录（serialize 中的 `writeRecord(this._$5)`） | 无公开 setter，默认为 null → 落为空记录 |
| 时间类单元格值（Timestamp/Time/sql.Date） | M1 范围是模板态标量与文本；时间值属计算态，且 `java.sql.Date` 标签 33 与 `java.util.Date` 标签 3 易混，留待 M2 |
| BigDecimal 单元格值 | spec §7 明确"记录层 BigDecimal 先以字符串透传，精度转换 M2 再议" |
| SubReport / BackGraph / Export / Macro 四类 MetaData | spec §4.2 明确 M1"记录存在性，字段后置"；留空记录即可覆盖"存在性"解析路径 |
| 边框样式（LB/RB/TB/BB style/width/color） | `full.rpx` 已覆盖对齐/字体/颜色/换行/旋转/缩进/可见性；边框属性结构与之同构（byte+float+int），M1 属性码字典已由架构师统一，避免夹具与字典双份维护 |

---

## 7. 自检结果（本轮回传依据）

| 检查项 | 结果 |
|--------|------|
| `javac --release 8` 编译 | PASS，0 error；`major version 52` |
| 单文件 ≤ 300 行门禁 | PASS（126 / 297 / 276 / 144） |
| 产品 JRE 运行生成 | PASS，退出码 0，无 stderr |
| 3 个 `.rpx` 头 4 字节 = 82,81,81,82 | PASS |
| 页脚 8 字节 = 82,81,81,82,82,81,81,82 | PASS |
| `总字节 == 8 + payload + 8` | PASS |
| expected.json 合法 JSON | PASS（Python `json.load`） |
| `len(row_heights)==rows-1`、`len(col_widths)==cols-1` | PASS |
| 合并区锚点自洽（begin == 单元格坐标） | PASS |
| 单元格坐标在逻辑区内 | PASS |
| **回读 round-trip：`.rpx` → reader → expected.json 逐字符一致** | **PASS ×3** |

> 生成式代码失效模式自检：① happy-path 偏差 —— 已覆盖非默认样式、4 种 extendMode、
> 3 种值类型、合并区、空 params/datasets；② 沉默逻辑错误 —— round-trip 门禁直接
> 证明二进制与黄金文档一致，非"看起来对"；③ 幻觉依赖 —— 所有 API 签名均从反编译
> 源码逐条核实（含 `Area` 构造器陷阱、`setUnit` 副作用、`_$20` 位打包），并用
> `jar tf` 确认类在 reportlite.jar 中真实存在；④ 缺失系统上下文 —— Java 8 目标 /
> 产品 JRE / classpath 分隔符均已实测；⑤ 性能盲区 —— 一次性生成，无 N+1；⑥ 静默缺失
> —— 编译零 warning（`-Xlint:none` 仅屏蔽 `--release 8` 过时告警），文件句柄全部
> try/finally 关闭。

---

# M2 增补：realmerge.rpx —— 真合并夹具（M1 遗留项①）

> 生成者：mvp-dev-expert-team-backend（贝洛奇）· 2026-10-12
> 动机：M1 三个夹具的合并区全是「浅合并」——非锚点槽在流里是完整默认记录，
> 而非 `int32 -1`，导致 rpx-core 的 `-1` 空槽分支与合并区网格铺满路径从未被执行。

## 8.1 产品 API 考证（为什么必须深克隆 + 全矩形 setCell）

`ReportDefine2` 本身**没有任何合并 API**（全文 grep `merge` 零命中）。
真正的合并在 IDE 动作层 `AtomicReport` case 23
（`02-ide-core/com/raqsoft/reportl/ide/AtomicReport.java:509-556`）：

```java
INormalCell firstCell = (INormalCell) this.report.getCell(firstPos).deepClone();
firstCell.setMergedArea(this.rect.getArea());
int r2 = this.rect.getBeginRow();
while (r2 <= this.rect.getEndRow()) {
    c = this.rect.getBeginCol();
    while (c <= this.rect.getEndCol()) {
        this.report.setCell(r2, c, firstCell);   // 同一个对象塞进每一格
        ++c;
    }
    ++r2;
}
```

`ReportEditor.mergeCells()`（`ReportEditor.java:2719-2756`）只是选取矩形后走
`undoManager.doing(new AtomicReport(...))`，且夹带 `JOptionPane`/`GV.appFrame`，
**无法 headless 调用**——所以本夹具直接复刻 case 23 的模型级调用序列
（`GenRealMerge.mergeRect`），只用公开 API，不手搓字节。

**为什么这能产出 `-1`**：`ReportDefine.serialize()` 对普通格的判定是
（`ReportDefine.java:1064-1075`）：

```java
if (nc == null || !nc.isMerged()) { out.writeRecord(nc); continue; }
Area ar = nc.getMergedArea();
if (ar.getBeginCol() == col && ar.getBeginRow() == row) { out.writeRecord(nc); continue; }
out.writeRecord(null);        // ← 非锚点：int32 -1
```

只有**该槽自己的 cell** 报 `isMerged()==true` 且不是锚点时才写 null。只给锚点设
Area（M1 的做法）时，其余槽仍是普通默认 NormalCell，`isMerged()` 为假 → 写完整
默认记录。把同一个带 Area 的对象铺满整个矩形后，每一格都 `isMerged()==true`，
非锚点全部落入 `writeRecord(null)`。

## 8.2 构造参数 —— `new ReportDefine2(7, 5)`

矩阵维度：`rows=8, cols=6`（含 0 号虚格边）；逻辑区 `row_count=7, col_count=5`。

**合并区**（3 个，均按产品序列生成）：

| 形状 | 锚点 | 区域 (begin..end) | 锚点 value | 锚点样式 |
|------|------|-------------------|-----------|----------|
| 1×3 横向 | (1,1) | (1,1)..(1,3) | `"WideTitle"` | fontName=Arial, fontSize=14, bold, hAlign=-47 |
| 2×2 方块 | (3,2) | (3,2)..(4,3) | `"Block"` | hAlign=-47, vAlign=-31 |
| 3×1 纵向 | (2,4) | (2,4)..(4,4) | `"Side"` | vAlign=-31 |

**行高**（索引 1..7）：`[24.0, 8.0, 20.0, 8.0, 18.0, 8.0, 22.0]`（行 2/4/6 保持默认 8.0）
**列宽**（索引 1..5）：`[80.0, 60.0, 60.0, 40.0, 50.0]`（全部显式设置）

**合并区外的普通格**（10 个）：
(2,1)`"r2c1"`、(2,2)`"r2c2"`、(2,3)`"r2c3"`、
(5,1)`"r5c1"`、(5,2)`"r5c2"`、(5,3)`"r5c3"`、(5,4)`"r5c4"`、
(6,1)`"r6c1"`、(6,2)`42`(Integer)、(7,1)`"r7c1"`。

expected.json 共收录 13 个非默认格（3 锚点 + 10 普通格）。

## 8.3 字节级证据（int32 -1 空槽）

`realmerge.rpx`：4997 字节；payload 4981；魔数 `82 81 81 82`；页脚
`82 81 81 82 82 81 81 82`；`总字节 == 8 + payload + 8` 成立。

矩阵段走查结果：35 个 NormalCell 槽中**恰有 7 个是 `int32 -1`**，
且与合并非锚点位置**逐一吻合**：

| 槽位 | 文件偏移 | 原始字节 |
|------|---------|----------|
| (1,2) | 568 | `FF FF FF FF` |
| (1,3) | 572 | `FF FF FF FF` |
| (3,3) | 2007 | `FF FF FF FF` |
| (3,4) | 2011 | `FF FF FF FF` |
| (4,2) | 2319 | `FF FF FF FF` |
| (4,3) | 2323 | `FF FF FF FF` |
| (4,4) | 2327 | `FF FF FF FF` |

**对照组（证明 M1 缺口真实存在）**：同一走查脚本跑 `merged.rpx`，
35 个槽中 `int32 -1` 的数量为 **0** —— M1 浅合并夹具确实从未触发该分支。

## 8.4 Rust 侧验证（rpx-core 现已覆盖该路径）

`rpx-cli print realmerge.rpx` 实跑输出（节选）：

```
matrix: 8x6 (logical 7x5)
row_heights: [24, 8, 20, 8, 18, 8, 22]
col_widths:  [80, 60, 60, 40, 50]
merges:
  (1,1)..(1,3)
  (2,4)..(4,4)
  (3,2)..(4,3)
  (1,1): value="WideTitle" merged=(1,1)..(1,3)
  (1,2): value="WideTitle" merged=(1,1)..(1,3)     ← 合并区铺满
  (1,3): value="WideTitle" merged=(1,1)..(1,3)     ← 合并区铺满
  ...
```

- 3 个 MergedArea 与 expected.json 完全一致；
- 合并区内 10 个槽全部持锚点 value 与 merged 标记（AC-04 "各处 CellId 相同" 达成）；
- 用 `tests/fixtures/cli_crosscheck.py` 对 4 个夹具逐一交叉校验
  （维度/行高列宽/合并区清单/网格铺满/非默认格齐全）：
  `simple` OK、`realmerge` OK、`merged` **FAIL(网格未铺满)**、`full` **FAIL(网格未铺满)**。
  后两者的失败正是 M1 缺口的直接体现——它们的非锚点槽是默认记录，
  解析器无从得知应与锚点同格。**这印证了 realmerge.rpx 的必要性。**

## 8.5 复现命令

```bash
cd F:/project/report/rpx/tests/fixtures
"D:/Tools/jdk-21.0.2/bin/javac.exe" --release 8 -Xlint:none \
  -cp "F:/project/report/reportlite/lib/*" *.java -d out
"F:/project/report/common/jre/bin/java.exe" \
  -cp "F:/project/report/reportlite/lib/*;out" rpx.fixtures.GenRealMerge .
# → realmerge.rpx + realmerge.expected.json，并自带 round-trip PASS 断言

# Rust 侧交叉校验（需先设 LIB，见 §9）
cd F:/project/report/rpx
python tests/fixtures/cli_crosscheck.py realmerge
```

## 8.6 待办（不属本轮范围）

- `crates/rpx-core/tests/` 尚无 realmerge 的黄金测试（本轮不修改 crates/）。
  建议下一位在 `golden.rs` 增加 `golden_realmerge`，断言：
  ① 7 个空槽被正确跳过；② `merges` 含上述 3 项；③ 合并区 10 槽 CellId 与锚点相同；
  ④ 行高列宽与 expected.json 一致。`cli_crosscheck.py` 已可作为该测试的参照实现。

---

# 9. 环境注记（M2 增补）

本机 Git Bash 默认 `LIB` 为空，`cargo build`/`cargo run` 链接期会报
`LNK1181: cannot open input file 'kernel32.lib'`。Windows SDK 与 MSVC lib
实际都在，只需导出：

```bash
export LIB="C:/Program Files (x86)/Windows Kits/10/Lib/10.0.26100.0/um/x64;C:/Program Files (x86)/Windows Kits/10/Lib/10.0.26100.0/ucrt/x64;C:/Program Files/Microsoft Visual Studio/2022/Community/VC/Tools/MSVC/14.43.34808/Lib/x64"
```

另：`target/debug/incremental` 有权限问题，cargo 会刷
`error deleting lock file ... (os error 5)` 警告。设 `CARGO_INCREMENTAL=0` 后
`cargo clippy --workspace --all-targets -- -D warnings` 实测 exit 0（零代码告警）。

---

# 10. 消歧实验（M2 增补）

> 产出者：mvp-dev-expert-team-backend（贝洛奇）
> 探针：`ProbeDisambig.java`（309 行）· 共享构件 `CalcProbe.java`
> 原始输出：`probe_disambig_out.txt` / `probe_disambig_err.txt`

上轮 Phase F 的 D 列数字（100/200/150）在两种解释下完全一致，无法判定
「函数真的求值」还是「失败后回退显示本行 B 值」。本节用受控输入把两种解释
拆开。

## 10.1 消歧 1：`sum(range)` 到底算没算？

**受控输入（无数据集、无扩展，故无歧义）**：B2=100、B3=200、B4=150 三个
字面量格，D2 放公式。真正求和必须是 450。

| 公式 | 实测值 | 结论 |
|------|--------|------|
| `sum(B2:B4)` | **100** | **不聚合** = 操作数 1 的值 |
| `sum(B2)` | 100 | 单操作数原样透传 |
| `sum(B3:B4)` | 200 | = B3，同样只取操作数 1 |
| `sum(B2:B4, value())` | ReportError | 两操作数形式非法 |
| `sum(value(), B2:B4)` | ReportError | 两操作数形式非法 |
| `sum(B2, B2:B4)` | ReportError | 两操作数形式非法 |
| `sum(B2:B4, B2)` | ReportError | 两操作数形式非法 |

**450 没出现，但函数确实执行了** —— 证据是 0 值场景返回 `Double(0.0)` 而非
`null`（失败回退会是 null）。源码级机制（`Sum.java:104-106`）：

```java
if (!this.param.isLeaf()) {   // ':' 让参数变成复合参数 -> 非 leaf
    return this._$1(ctx);      // 走 cellSet 分支，不是聚合分支
}
```

`_$1(ecs, cellSet, param2, ctx)` 把**操作数 1 当作逐格表达式、操作数 2 当作
cellSet**；cellSet 是单个 ExtCell（非 List）时只求值一次
（`Sum.java:174-190`）。所以 `sum(A:B)` ≡ 「把 B 设为 current 后求值 A 一次」。

**引擎必须实现的正确聚合形式**（List 操作数才聚合，`Sum.java:112-121`）：

| 公式 | 实测值 |
|------|--------|
| `ds1.sum(qty)`（数据集聚合，DS 函数） | **450** |
| `sum(ds1.select(qty))`（List 操作数） | **450** |
| `ds1.avg(qty)` | 150.0 |
| `ds1.max(qty)` | 200 |
| `ds1.count()` | 3 |

> **给 Rust 引擎的硬约束**：不要实现 Excel 式区域求和。`sum(X)` 当 X 求值为
> List 时才是聚合；当 X 是区域时退化为「操作数 1 求值一次」。

**数据集在扩展中的行为差异**（`engine_sum` 夹具已钉死）：
`ds1` 被扩展格引用 → 计算时 current 前进 → `ds1.sum(qty)` 退化为当前行
（100/200/150）；未被扩展格引用的 `ds2` → current 停在根组 → 450。

## 10.2 消歧 2：extendMode=16 在有数据集时是否扩展？

**受控输入**：3 行 × 3 列模板（1 表头行 + 1 模板数据行），数据集 5 行
（R1..R5），数据格公式 `ds1.select(region)`，逐个改 extendMode。

| extendMode | 计算态矩阵 | 锚点 subCells | 锚点生效 extend | 逐行值 |
|-----------|-----------|--------------|----------------|--------|
| 16 DEFAULT | **6 行** | **5** | **18**（自动升为竖向） | R1..R5 每行一个 |
| 18 VERTICAL | **6 行** | 5 | 18 | R1..R5 每行一个 |
| 17 NONE | 6 行（由 B 列驱动） | **1** | 17 | **ArrayList([R1..R5]) 整体塞进一格** |
| 19 HORIZONTAL | 6 行 × **7 列** | 5 | 19 | 横向铺开 |

**结论**：extendMode=16 **会扩展**。产品并非「按声明值扩展」，而是
`ExtExtendCell.prepareCalculate`（:314-328）在运行时按 leftHead/topHead
拓扑把 16 **改写**为 18/19/17：

```java
if (16 == extend) {
    if (leftHead == cell00 && topHead != cell00) setExtensible(19);
    else if (leftHead != cell00 && topHead != cell00) setExtensible(17);
    else setExtensible(18);
}
```

**行数公式**：`计算态行数 = 模板行数 + (数据集行数 - 1)`。锚点占第 1 个数据行，
其余 N-1 行由 `extendRow` 插入，故 3 + 4 = 7（`engine_ext` 实测 7x4）。

**横向扩展边界**（D2b）：单独一行里用 19 时，列数恒增 N-1：
- 3 列模板、19 在第 1 列 → 7 列，值 R1..R5 落在第 1..5 列
- 4 列模板、19 在第 4 列 → 8 列，值 R1..R5 落在第 4..8 列
- 但 19 与 16/18（竖向扩展）**同行**时（`engine_ext`），列数不增（7x4）——
  竖向扩展已把该行登记为可扩展行，横向扩展被吞掉。

## 10.3 消歧 3：分页基准（此前从未跑通，现已跑通）

链路：`ReportUtils2.getPagerInfo(rpt, 595, 842, 1)` → `new PageBuilder(rpt, pi)`
→ `createPages()` → `getAllPages()`。

- 40 数据行、行高 20、unit=1（**毫米**）、A4 595x842pt
  → `pageCount=3, xPages=1, yPages=3, rowNumPerPage=20`
  → 每页 **14 行**（297mm / 20mm ≈ 14，`rowNumPerPage=20` 只是 propMap
  槽 33 的提示值，不参与实际切页）

| 页 | 行数 | 首值 | 末值 |
|----|------|------|------|
| 1 | 14 | region | R13 |
| 2 | 14 | R14 | R27 |
| 3 | 14 | R28 | R40 |

## 10.4 消歧 4：分组头（groupHeaderLevel）

两种构造方式结果相反：

- **变体 1（-90 打在会扩展的锚点行上）→ 分页 NPE**。扩展把 -90 复制到每一个
  插入行（实测行类型 `1:-91 2:-90 3:-90 4:-90 ...`），整块都成了分组头，
  `SimplePager.createPages` 在 `SimplePager.java:261` 取到越界的
  `getRowCell(minRow)` → `NullPointerException`。
- **变体 2（独立 -90 分组行 + 普通数据行）→ 跑通，且分组头逐页重复**：
  `pageCount=4`，页 1 首值 `region`，页 2/3/4 首值均为 `group: R*`。

行类型常量（`IRowCell`）：TITLE_HEADER=-96 / TABLE_HEADER=-95 /
TABLE_FOOTER=-94 / PAGE_HEADER=-93 / PAGE_FOOTER=-92 / NORMAL=-91 /
**GROUP_HEADER=-90**。分组头级别取值 1..20。

---

# 11. M2 引擎夹具（新增）

> 产出者：mvp-dev-expert-team-backend（贝洛奇）· 均为产品 writer 产出，零手搓字节

## 11.1 产物清单

> 8 个夹具。P0 工作流追加了 `engine_merge` / `engine_cycle` /
> `engine_samemerge` 并顺带重生成了 `engine_ext`（2527→2494 字节），
> `engine_disp` 为本轮追加（§12.3）。下标字节数均已按盘上实测更正。

| 文件 | .rpx 字节 | .expected.json 字节 | 说明 |
|------|-----------|---------------------|------|
| `engine_sum.rpx` | 9413 | 6275 | sum/聚合各形态；双数据集 ds1（在扩展中）/ds2（不在） |
| `engine_sum.csv` / `engine_sum.csv2` | 44 / 44 | — | 两个数据集的输入（制表符分隔、GBK、首行列名） |
| `engine_ext.rpx` | 2494 | 6929 | extendMode 17/18/19 各占一行，各拥有自己的扩展包络 |
| `engine_expr.rpx` | 5363 | 3929 | 相对引用 / 绝对列引用 / 混合算术 |
| `engine_pager.rpx` | 2483 | 9822 | 40 数据行 + 独立 -90 分组头行，触发 4 页 |
| `engine_merge.rpx` | 3142 | 3862 | M2-04 跨扩展区的合并区 |
| `engine_cycle.rpx` | 1930 | 994 | M2-07 循环引用 → `ReportError`（无计算态网格） |
| `engine_samemerge.rpx` | 1969 | 2438 | M2-08 `mergeSame=98` 合并相邻等值 |
| `engine_disp.rpx` | 3115 | 2561 | M2-14 `dispValue`（本轮新增，§12.3） |
| `engine_types.rpx` | 3020 | 3045 | 类型保真：int/long/double/float/decimal/boolean（types 工作流，§12.6） |
| `engine_rowheight.rpx` | 3510 | 5561 | M2-22 行高属性表达式 + 因果（§14） |

## 11.2 expected.json 契约（与 M1 结构态 expected.json 区分）

M1 的 expected.json 是**结构态**（rpx-core 的断言对象）；M2 的
engine_*.expected.json 是**计算态 + 分页态**：

| 字段 | 含义 |
|------|------|
| `unit` / `unit_note` | 1=mm 0=inch 2=point，行高列宽以此为单位 |
| `dataset_file` / `dataset` | 数据集文件名 + 列/行字面量（供 Rust 侧重建） |
| `template_rows` / `template_cols` | 模板矩阵尺寸（扩展前） |
| `formulas[]` | 每个表达式格的 `{r,c,exp,extend_mode}` |
| `row_cells[]` | 每行的 `{r,row_type,group_header_level,row_height}` |
| `calculated` | 计算态：`{rows,cols,cells[]}`，每格 `{r,c,value,type,disp_value,source{src_row,src_col,row_merge,col_merge,extend,sub_cells,sub_values[]}}` |
| `pagination` | `{paper_w,paper_h,columns,page_count,x_pages,y_pages,row_num_per_page,pages[]}`，每页 `{page,rows,first_value,last_value,col1_values[]}` |

`value` **带类型**（`type` 字段：int/double/string/boolean/list/null）——
产品返回 `Integer(100)`，写成 `100.0` 就会与黄金不符。

## 11.3 字节级与回读证据

```
$ java -cp "F:/project/report/reportlite/lib/*;out" rpx.fixtures.GenEngineFixtures --verify .
PASS engine_sum: 9413 bytes round-trip to expected.json
PASS engine_ext: 2494 bytes round-trip to expected.json
PASS engine_expr: 5363 bytes round-trip to expected.json
PASS engine_pager: 2483 bytes round-trip to expected.json
PASS engine_merge: 3142 bytes round-trip to expected.json
PASS engine_cycle: 1930 bytes round-trip to expected.json
PASS engine_samemerge: 1969 bytes round-trip to expected.json
PASS engine_disp: 3115 bytes round-trip to expected.json
VERIFY OK: 8 engine fixtures generated
```

`--verify` 用产品 reader（`ReportUtils.read`）读回二进制 → 重新
`Engine.calc()` → 重新渲染 expected.json → 与磁盘逐字符比对。
**八个夹具全部 PASS**，即「.rpx 字节 + CSV → 计算态 → expected.json」链路自洽。

Rust 侧结构态交叉校验（未改动 `crates/`）：

```
engine_sum   matrix: 7x10 (logical 6x9)
engine_ext   matrix: 5x4  (logical 4x3)
engine_expr  matrix: 7x6  (logical 6x5)
engine_pager matrix: 5x4  (logical 4x3)  row_heights: [20, 20, 20, 20]
```

> 更正（本轮）：`engine_ext` 一行此前写作 `4x5 (logical 3x4)`，行列写反了。
> 模型 `EngineModels.extFixture()` 是 `new ReportDefine2(4, 3)`（4 行 3 列），
> `common/loader.rs::template_dims_match_product_output` 也断言 `(4, 3)`，
> 故盘上实测应为 `5x4 (logical 4x3)`（物理矩阵 = 逻辑 + 1 圈脚手架）。

注意 logical（模板）与 matrix（计算态）尺寸不同 —— 这正是 M1 结构态与
M2 计算态的分界。

## 11.4 engine_sum 逐格真值（消歧 1 的落盘证据）

数据集 ds1 = ds2 = North 100 / East 200 / South 150：

| 行 | region | qty | twice<br>`B2*2` | sumRange<br>`sum(B2:B4)` | ds1Sum<br>`ds1.sum(qty)` | sumList1<br>`sum(ds1.select(qty))` | **ds2Sum<br>`ds2.sum(qty)`** | **sumList2<br>`sum(ds2.select(qty))`** | **ds2Count<br>`ds2.count()`** |
|----|--------|-----|------|------|------|------|------|------|------|
| 2 | North | 100 | 200 | **100** | 100 | 100 | **450** | **450** | **3** |
| 3 | East | 200 | 400 | **100** | 200 | 200 | **450** | **450** | **3** |
| 4 | South | 150 | 300 | **100** | 150 | 150 | **450** | **450** | **3** |

即：450 只在「数据集不在扩展中」或「List 操作数」两条路径上出现，
`sum(B2:B4)` 恒为 100。

## 11.5 engine_pager 分页真值

`pageCount=4, xPages=1, yPages=4, rowNumPerPage=20`，A4 595x842pt，行高 20mm：

| 页 | 行数 | 首值 | 末值 |
|----|------|------|------|
| 1 | 14 | region | R12 |
| 2 | 14 | **group: R\*** | R25 |
| 3 | 14 | **group: R\*** | R38 |
| 4 | 4 | **group: R\*** | R40 |

## 11.6 源码文件行数门禁

门禁命令（任一文件 >300 行即退回）：

```bash
cd tests/fixtures && find . -name '*.java' | xargs wc -l | awk '$1>300 && $2!="total"'
```

| 文件 | 行数 | 职责 |
|------|------|------|
| `Fixtures.java` | 297 | M1 三个夹具的模型构造（未改动） |
| `CalcProbe.java` | 285 | 探针共享构件：模型构建器 + 计算态/分页 dumper |
| `ProbeExtend.java` | 278 | 消歧 D2/D2b/D3/D4：extendMode、分页、分组头 |
| `ProbeP0.java` | 277 | P0 夹具探针（merge / cycle / samemerge） |
| `ExpectedJson.java` | 276 | M1 结构态 expected.json 渲染（未改动） |
| `CalcJson.java` | 253 | 计算态/分页态 → JSON（带类型） |
| `ProbeTypes.java` | 246 | 类型保真探针（字面类型是否被 `.rpx` 往返降级） |
| `EngineModels.java` | 243 | 引擎夹具的模型 + 数据生成（本轮 +`dispFixture`） |
| `ProbeCalc.java` | 240 | P0-2 八阶段驱动（原 485 行，已拆分） |
| `GenEngineFixtures.java` | 236 | 生成 + `--verify` 回读门禁 |
| `ProbeDisp.java` | 204 | `dispValue` 缺口探针（本轮新增，§12.4） |
| `ProbeFunPriority.java` | 166 | 函数优先级真机取证（M6 孤儿表，§13.1） |
| `EngineModelsM2.java` | 80 | `engine_rowheight` 的模型（§14.2） |
| `ProbeDisambig.java` | 199 | 消歧 D1a..D1d：sum/区域求值 |
| `GenRealMerge.java` | 154 | M2 真合并夹具（未改动） |
| `Json.java` | 144 | 零依赖 JSON 写入器（未改动） |
| `GenFixtures.java` | 126 | M1 入口（未改动） |
| `EngineModelsP0.java` | 109 | P0 夹具的模型（merge / cycle / samemerge） |
| `EngineModelsTypes.java` | 81 | `engine_types` 的模型 |
| `CalcMergeJson.java` | 72 | 合并格 → JSON |
| `VerifyDollar.java` | 70 | `$` 引用语义验证 |
| `FileIO.java` | 45 | UTF-8 文本读写 |

全部 ≤300 行（20 个文件，3835 行）。`ProbeCalc.java`（485→240）与
`GenEngineFixtures.java`（317→163→236）已按单一职责拆分；
`ProbeDisambig.java`（426→199）按「sum 消歧 / extend+分页+分组头」切两个探针。

Python 侧（非 Java 门禁，不计入上表）：`loader_crosscheck.py` 326 行
（本轮新增，§12.1）、`cli_crosscheck.py` 83 行、`_shape_audit.py` 67 行、
`_source_audit.py` 57 行（后两个是 backend-2 的打印型勘察脚本，与 §12.1 分工见 §12.6）。

## 11.7 复现命令

```bash
cd F:/project/report/rpx/tests/fixtures
"D:/Tools/jdk-21.0.2/bin/javac.exe" --release 8 -Xlint:none \
  -cp "F:/project/report/reportlite/lib/*" *.java -d out

# P0-2 八阶段链路
"F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
  rpx.fixtures.ProbeCalc . > probe_out.txt 2> probe_err.txt

# M2 消歧实验（sum / 区域求值）
"F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
  rpx.fixtures.ProbeDisambig . > probe_disambig_out.txt 2> probe_disambig_err.txt

# M2 消歧实验（extendMode / 分页 / 分组头）
"F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
  rpx.fixtures.ProbeExtend . > probe_extend_out.txt 2> probe_extend_err.txt

# 引擎夹具生成 / 回读门禁
"F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
  rpx.fixtures.GenEngineFixtures .
"F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
  rpx.fixtures.GenEngineFixtures --verify .
```

---

# 12. 地面真值加载器独立复核 + dispValue 覆盖缺口（本轮）

> 产出者：mvp-dev-expert-team-backend（贝洛奇）
> 起因：backend-2 交来 `crates/rpx-engine/tests/common`（74 passed / clippy 0）。
> 这一层是**所有 M2 断言的基座** —— 基座读错，上面全是幻觉。故用**第二套独立
> 实现**（Python，与 Rust 零共享代码）反向复核，并顺手查出一处覆盖缺口。

## 12.1 `loader_crosscheck.py`：四类独立复核（PASS）

`tests/fixtures/loader_crosscheck.py`（303 行、零依赖、非 Java 门禁对象）。
退出码即判据：`0` = 逐条一致，`1` = 有漂移（并逐条打印）。

```bash
cd F:/project/report/rpx && python tests/fixtures/loader_crosscheck.py
```

四步：

1. 盘上 `engine_*.expected.json` 全集 vs `loader.rs::all_names()`（防"新增夹具静默漏测"）
2. 从 JSON 重推夹具表 —— 模板尺寸 / 计算态尺寸 / 非空格数 / source 计数 —— vs
   `fixture_loader.rs` 里**写死的数字**
3. 重推分页逐页、`engine_samemerge` 合并格、`engine_ext` / `engine_merge` 锚点抽查 vs 写死断言
4. 用 JSON 里**真实出现的键集合**反向校验 `loader.rs` 的 `expect_keys` 白名单
   （产品侧一旦新增键，加载器会 panic；这里提前发现）

复核结果（终态，全部 OK）：

```
盘上 engine_* 夹具 9 个：engine_cycle, engine_disp, engine_expr, engine_ext, engine_merge,
                        engine_pager, engine_samemerge, engine_sum, engine_types

[1] all_names 声明长度 9 == 实际列出 9；无幽灵名；盘上 9 个全部被收录
    PENDING 登记表为空（两条临时登记在 backend-2 同步后已删除 —— 见 §12.6）
[2] SHAPE 声明长度 8 == 实际条目 8
    SHAPE 恰好覆盖「全部非错误型夹具」（engine_cycle 是错误型，不入表）
    8 个夹具的 模板尺寸 / 计算态尺寸 / 非空格数 / source 计数 逐条一致
    source 双向不变量「有 source ⟺ 落在模板尺寸内」8/8 夹具成立
    source 计数覆盖全部非空格（有 + 无 == 非空格）8/8
[3] 分页 page_count=4 / x_pages=1 / y_pages=4 / row_num_per_page=20、逐页 [14,14,14,4] 一致
    末页 rows(4) > 第 1 列非空值数(3) —— 空值被跳过
    engine_samemerge：1 页 4 格、row_merge=2、col_merge=1、值原文 ["A","A","B","B"]（含引号）
    engine_ext：(2,1) extend=17 type=list / (3,1) extend=18 subCells=5 /
                (4,1) extend=19 附属格落 (8,1)..(8,5)
    engine_merge：(2,1) row_merge=2 extend=18
    engine_cycle：error.class / chain=B2-C2-B2-C2 / cause_class 后缀 / \uXXXX 已解码 全对
[4] 12 层白名单全部覆盖真实键集合 → 加载器对当前产物不会 panic
    （补 engine_disp 后，`cell` 层不再有"未出现的键 disp_value"这条 note）
结论：PASS —— 加载器写死的数字与产品产物逐条一致，白名单未漏键
```

同一批数字的独立复算（本轮复核时）与 `SHAPE` 逐条相等：

| 夹具 | SHAPE = (模板 r,c, 计算 r,c, 非空格, 有src, 无src) |
|------|----------------------------------------------------|
| engine_sum | (6, 9, 8, 9, 36, 36, 0) |
| engine_ext | (4, 3, 8, 7, 42, 12, 30) |
| engine_expr | (6, 5, 8, 5, 20, 20, 0) |
| engine_pager | (4, 3, 43, 3, 124, 10, 114) |
| engine_merge | (4, 4, 8, 4, 19, 12, 7) |
| engine_samemerge | (3, 3, 7, 3, 12, 6, 6) |
| engine_disp | (4, 4, 6, 4, 11, 10, 1) |
| engine_types | (4, 4, 4, 4, 16, 16, 0) |

两条**口径提醒**（不是缺陷，但很容易写错断言）：

- `Page.first_value` / `last_value` 走 `Json::as_string()` —— **已解码、无引号**；
  同一结构里的 `col1_values` 走 `v.raw` —— **原文 token、带引号**。一个结构两种口径。
- `expect_keys` 是**白名单**（拒绝未识别键），**不是**"必填键"检查：
  `disp_value` / `source` / `first_value` / `last_value` 都可以缺。故 `Option` 字段
  不会因为字段不存在而误报。

## 12.2 dispValue 覆盖缺口（补夹具前的证据）

`disp_value` 是本轮之前**唯一**被三处代码建模、却在语料里 0 次出现的字段：

```
$ grep -c disp_value tests/fixtures/*.expected.json      # 补 engine_disp 之前
engine_cycle/expr/ext/merge/pager/samemerge/sum.expected.json  → 0
full/merged/realmerge/simple.expected.json                     → 0     （共 11 个文件）
```

于是三处断言全是**永真式**：

| 位置 | 形态 | 后果 |
|------|------|------|
| `crates/rpx-core/tests/golden.rs:99` | `if let Some(dv) = jc.get("dispValue")` | 恒不进入 |
| `crates/rpx-engine/tests/common/loader.rs:159` | `cell.get("disp_value").map(..)` | 恒为 `None` |
| `crates/rpx-cli/src/main.rs:129` | `if let Some(dv) = &cell.disp_value` | 打印分支恒不触发 |

而 `crates/rpx-core/src/cell.rs:137` 确实从 **DISPVALUE 槽** `read_string`，且
`cell/cell_defaults.rs:41` 用 `self.disp_value.is_none()` 参与"是否空单元格"判定 ——
也就是说：**一个静默丢掉 DISPVALUE 槽的解析器，可以让全部 74 个测试保持绿色**，
并且空单元格判定会静默出错。这正是失效模式清单里的第 2 类（沉默逻辑错误）
与第 6 类（静默缺失）。

## 12.3 `engine_disp` 夹具（本轮新增）

先证明"能产出"（§12.4 探针），再落盘。生成路径与既有一致：模型 → 产品
`ReportUtils.write` → 产品 `Engine.calc()` → `CalcJson` 渲染 → `--verify` 回读门禁。
**零手搓字节。**

- 模板 `ReportDefine2(4, 4)`；数据集 `engine_disp.csv` = North 100 / East 200 / South 150
- 表达式只有两格：`(2,4)=6*1`、`(3,1)=ds1.select(qty)`（后者为纵向扩展锚点）
- 产物：`engine_disp.rpx` 3115 字节、`engine_disp.csv` 44 字节、`engine_disp.expected.json` 2561 字节

逐格真值（`unit=1` mm；grid 6x4；11 个非空格，其中 7 格带 `disp_value`）：

| (r,c) | value | type | disp_value | source |
|-------|-------|------|-----------|--------|
| (1,1..4) | `"A"` `"B"` `"C"` `"D"` | string | — | src=自身 extend=16 subCells=1 |
| (2,1) | `"North"` | string | `"北区"` | extend=16 |
| (2,2) | `100` | int | `"100.00"` | extend=16 |
| (2,3) | `null` | **null** | `"仅显示值"` | extend=16 |
| (2,4) | `6` | int | `"6 件"` | extend=**17** |
| (3,1) | `100` | int | `"Q"` | extend=18 **subCells=3** |
| (4,1) | `200` | int | `"Q"` | extend=16 |
| (5,1) | `150` | int | `"Q"` | **无 source（超出模板 4x4）** |

`(3,1)` 的 `sub_values` = `(3,1) 100 / (4,1) 200 / (5,1) 150`。

一次钉住 5 条契约：

1. **显示值与值分离**：`value=100` 而 `disp_value="100.00"` —— 不得用显示值反推值，也不得
   因为 `"100.00"` 把类型判成 decimal（`type` 仍是 `int`）。
2. **`value=null` 但非空格**：`(2,3)`。`CalcJson:51` 的跳过条件是 `v == null && dv == null`，
   所以"value 为空就跳过"的实现会**少一格**。
3. **计算值与显示值并存**：`(2,4)` 计算得 `6`，显示 `"6 件"`；且该格 `extend` 被改写成 17。
4. **扩展下发显示值**：锚点 3 个附属格**逐个**带 `"Q"`，含超出模板的 `(5,1)`。
   注意 `sub_values` 里**没有** `disp_value` 字段（`CalcJson:86-95` 只写 r/c/value/type），
   要断言附属格显示值必须走顶层 `cells[5,1].disp_value`。
5. **非 ASCII 显示值**：`"北区"` 走 modified-UTF-8 往返，顺带压编码路径。

改动面（三处，全部在 Java 侧、不动 `crates/`）：
`GenEngineFixtures.NAMES` +1、`EngineModels.build` +2 行分支、`EngineModels.dispFixture()` 新增。

**回归证据 —— 原有夹具字节不变**：

```
$ md5sum engine_* > before ; GenEngineFixtures . ; md5sum engine_* > after
diff before after | grep '^>'          # 只有 engine_disp.{rpx,csv,expected.json} 三行
diff before after | grep '^<'          # 空
原有文件字节不变数 = 21 / 21
```

即重生成**没有**带来任何静默漂移：7 个既有夹具的 `.rpx` / `.csv` / `.expected.json`
共 21 个文件字节完全相同（这也解释了为什么 `fixture_loader.rs` 的写死数字不需要随之改动）。

`--verify` 结果见 §11.3（8/8 PASS）。

**同步结果**：落盘时 `all_names()` 还是 7 项，我临时在
`loader_crosscheck.py::PENDING_IN_LOADER` 登记了两条（`engine_disp` + 同批出现的
`engine_types`，见 §12.6）。backend-2 随后把 `all_names()` 提到 9 项、并把原来
两张数字表合并成 `SHAPE`，两处登记随即删除 —— 登记表的三条自清理规则
（过期登记 / 已同步残留 / 未登记的漏测）现在都为 OK。我独立复算的
`(4,4,6,4,11,10,1)` 与 `SHAPE` 里那行**逐字段相同**。

## 12.4 `ProbeDisp`：四个判决（补夹具前的可行性证明）

`tests/fixtures/ProbeDisp.java`（204 行）—— 独立探针，不依赖生成器。
复现：

```bash
cd F:/project/report/rpx/tests/fixtures
"D:/Tools/jdk-21.0.2/bin/javac.exe" --release 8 -Xlint:none \
  -cp "F:/project/report/reportlite/lib/*" *.java -d out
"F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
  rpx.fixtures.ProbeDisp . > probe_disp_out.txt 2> probe_disp_err.txt   # exit 0
```

| 待证点 | 判决 | 数字 |
|--------|------|------|
| DISP-0 模板态写入 | OK | `(2,1) value="North" disp="北区"`、`(2,2) Integer(100) disp="100.00"`、`(2,3) value=null disp="仅显示值"` |
| DISP-1 计算态是否存活 | **存活** | 6x4 网格，非平凡 `dispValue` **7 格**（(2,1)(2,2)(2,3)(2,4)(3,1)(4,1)(5,1)） |
| DISP-2 `value=null` 的格 | **保留** | 计算态 `(2,3) value=null disp="仅显示值"`（未被当空格丢弃） |
| DISP-3 `.rpx` 写→读 | **5/5 OK** | `out/disp_probe.rpx` 3114 字节，5 格 `value`+`dispValue` 逐一相同 |
| DISP-4 扩展格下发 | **下发** | 锚点 `(3,1) extend=18 subCells=3`，`sub[0..2] value=100/200/150 dispValue="Q"` |

结论：`disp_value` 是**可产出的真实契约**，缺口可以补（§12.3 已补）。

> **编码注记**：产品 JRE 的 `System.out` 默认是 **GBK**（Windows 平台默认），
> 所以 `probe_*_out.txt` / `probe_*_err.txt` 是 GBK 文本，直接以 UTF-8 打开会乱码。
> 本轮已为每个探针输出追加了 UTF-8 伴生件 `*.utf8.txt`（原文未改动，仍是原始证据）。
> 换算命令：
>
> ```bash
> python -c "import glob; \
>   [open(p[:-4]+'.utf8.txt','w',encoding='utf-8',newline='').write( \
>       open(p,'rb').read().decode('gbk',errors='replace')) \
>    for p in glob.glob('probe_*_out.txt')+glob.glob('probe_*_err.txt')]"
> ```
>
> 夹具 `.expected.json` 是 UTF-8（`FileIO.write` 显式指定），不受此影响。

## 12.5 本轮自检链

```
python tests/fixtures/loader_crosscheck.py                      → PASS（exit 0）
GenEngineFixtures --verify .                                    → VERIFY OK: 9 engine fixtures
cargo test --workspace                                          → 94 passed / 0 failed
cargo clippy --workspace --all-targets -- -D warnings           → exit 0
find . -name '*.java' | xargs wc -l | awk '$1>300'               → 无输出（20 文件 / 3835 行）
```

> 环境注记：`cargo` 需先导出 `LIB`（见 §9），否则链接期报
> `LNK1181: cannot open input file 'kernel32.lib'`。
>
> 时序注记：本轮复核期间 `crates/rpx-engine` 被并行编辑，曾出现两类**瞬时**红灯
> （`arena/tests.rs` 缺 `ExtendMode` 导入、`src/` 一处 clippy），均在数分钟内被
> 对应 owner 修好；终态见上表。发现的问题与处置见 §12.6。

## 12.6 语料新增 `engine_types` + `SHAPE` 的 clippy 取舍（发现，非本轮改动）

**一、`engine_types` 是另一条工作流并入的。** 我做本轮复核期间（15:36），types
工作流把 `engine_types` 加进了**同一个** `GenEngineFixtures.NAMES`（现 9 项）与
`EngineModels.build`，与我加的 `engine_disp` 合并干净、无冲突。它补的正是
backend-2 指出的那条："真夹具里没有 double/decimal/boolean 数值格"：

| (r,c) | value | type |
|-------|-------|------|
| (2,1) | `100` | int |
| (2,2) | `1234567890123` | long |
| (2,3) | `92.5` | double |
| (2,4) | `100` | **double**（整数值的 double，`value_raw` 必须是裸 `100` 而非 `100.0`） |
| (4,1) | `100.0` | decimal（`BigDecimal.toString()`） |
| (4,2)(4,3) | `true`/`false` | boolean |
| (4,4) | `1.5` | float |

与 `engine_disp` 互补（后者 0 个类型变体、7 格 `disp_value`），无重复。
**当前 9 个夹具由同一个生成器 + 同一个 `--verify` 门禁覆盖**（`VERIFY OK: 9`），
没有出现"第二个生成器写进同一语料却没人门禁"的洞。

**二、`SHAPE` 曾把 clippy 门禁打红（已修）。** 复核时实测：

```
error: very complex type used. Consider factoring parts into `type` definitions
  --> crates/rpx-engine/tests/fixture_loader.rs:32:14
error: could not compile `rpx-engine` (test "fixture_loader")
```

诱因是把两张数字表合并成 `SHAPE` 后的 8 元组
`[(&str, i32, i32, i32, i32, usize, usize, usize); 8]` —— 在 `-D warnings` 下
`clippy::type_complexity` 直接变成编译错误。**行数预算**是关键约束：
`fixture_loader.rs` 当时是 296/300 行，只剩 4 行余量，所以"展开成具名结构体"
会立刻撞 300 行门禁（8 行 × 8 字段）。

backend-2 采用了类型别名方案（也是 clippy 报错文本自己的建议），**实测 296 → 298 行**，
与预算推算一致：

```rust
type ShapeRow = (&'static str, i32, i32, i32, i32, usize, usize, usize);
const SHAPE: [ShapeRow; 8] = [ ... ];
```

`loader_crosscheck.py` 现在**两种写法都接受**，并额外钉住别名的**字段顺序**
（`ShapeRow` 若被调换字段顺序，脚本的解析会静默错位 —— 这条断言专防那个）。

**三、与 backend-2 勘察脚本的分工**：`_shape_audit.py`（67 行）与
`_source_audit.py`（57 行）是**打印型**工具（dump 形状 / 混淆矩阵，给人看，无判据）；
`loader_crosscheck.py`（326 行）是**断言型**门禁（OK/FAIL + 退出码，可进复现链）。
两者互补，保留各自定位即可，但建议在各自文件头互相点名，避免后来者误以为重复实现。

---

# 13. 类型面夹具 engine_types（M2-14 覆盖补齐）

> 产出者：backend-2 · 探针 `ProbeTypes.java`（246 行）· 模型 `EngineModelsTypes.java`（81 行）
> 原始输出：`probe_types_out.txt` / `probe_types_err.txt`
> 动机：§12 查出的覆盖缺口 —— 7 个既有 engine_* 夹具的类型总计只有
> `{'string':124,'int':124,'list':5}`，`CalcJson.typeName`（`CalcJson.java:224-253`）
> 的 9 个分支只走到 3 个，M2-14 的 `Integer(100)` vs `Double(100.0)` 无产品夹具背书。

## 13.1 探针结论（先探后建，不猜）

### 线路 A：字面量 `setValue` → `.rpx` write→read→calc

| 字面量 | 实测 typeName |
|---|---|
| `Integer.valueOf(100)` | `int` |
| `Long.valueOf(1234567890123L)` | `long` |
| `Double.valueOf(100.0)` | `double` |
| `Double.valueOf(92.5)` | `double` |
| `Float.valueOf(1.5f)` | `float` |
| `new BigDecimal("100.00")` | `decimal` |
| `new BigDecimal("3.14159")` | `decimal` |
| `Boolean.TRUE` / `Boolean.FALSE` | `boolean` |
| `"txt"` | `string` |
| `Short.valueOf((short)7)` | `Short`（typeName 兜底分支） |
| `Byte.valueOf((byte)9)` | `Byte`（同上） |

`[lit] ROUND-TRIP OK: every literal type survived .rpx` —— 12 格在**内存态与读回态**的
`type`/`value` 完全一致。依据：`ByteArrayOutputRecord.writeObject:202-266` 每类型一个 tag
（1 String / 2 BigDecimal / 4 Integer / 5 Long / 6 Boolean / 9 Double / 10 Float / 11 Byte / 12 Short），
`ByteArrayInputRecord.readObject:265-343` 按 tag 还原出**同一个 boxed 类型**。

### 线路 B：表达式（无数据集，操作数 A2=Integer 100 / B2=200 / C2=Double 2.0 / D2=BigDecimal 3）

| 表达式 | value | 实测 typeName |
|---|---|---|
| `A2/3` | 33.333333333333336 | double |
| `A2*1.0` | 100 | **double** |
| `A2/2` | 50 | double |
| `A2+B2` | 300 | int |
| `A2*C2` | 200 | double |
| `A2-B2` | -100 | int |
| `A2/C2` | 50 | double |
| `A2*100000000000` | 10000000000000 | long |
| `A2>B2` / `A2==B2` | false | boolean |
| `D2*2` | 6 | decimal |
| `A2/D2` | 33.33333333333333333333 | decimal |

注意 `A2*1.0` 的 `value` 是裸 `100` 而 `type=double` —— 正是
`CalcJson.value():209-211` 把整数值 Double 折叠成 `Long.toString` 的效果，
**判类型只能看 `type` 字段**。

### 线路 C：数据集聚合（无 `select`，故无扩展；3 行 100/200/150）

| 表达式 | value | typeName |
|---|---|---|
| `ds1.sum(qty)` | 450 | int |
| `sum(ds1.select(qty))` | 450 | int |
| `ds1.count()` | 3 | int |
| `ds1.avg(qty)` | 150 | double |
| `ds1.max(qty)` | 200 | int |
| `ds1.min(qty)` | 100 | int |

## 13.2 产物：`engine_types`（第 9 个夹具）

4×4、**纯字面量**（`formulas: []`）、无数据集、无扩展 ⇒ 计算态 == 模板态（4×4），
16 格全部带 `source`。

| 文件 | 字节 |
|---|---|
| `engine_types.rpx` | 3020 |
| `engine_types.csv` | 44 |
| `engine_types.expected.json` | 3045 |

布局（标签行即 string 对照格）：

| | c1 | c2 | c3 | c4 |
|---|---|---|---|---|
| r1 | `"int"` | `"long"` | `"double-frac"` | `"double-integral"` |
| r2 | `100` (int) | `1234567890123` (long) | `92.5` (double) | `100` (**double**) |
| r3 | `"decimal"` | `"boolean-true"` | `"boolean-false"` | `"float"` |
| r4 | `100.00` (decimal) | `true` | `false` | `1.5` (float) |

类型直方图：`{string:8, int:1, long:1, double:2, decimal:1, boolean:2, float:1}`。
两个关键格：**(2,4)** `Double(100.0)` 的 `value` 是裸 `100` 但 `type=double`；
**(4,1)** `BigDecimal("100.00")` 的 `value` 原样 `100.00`、`type=decimal`。

全夹具类型总计：`{'string':137,'int':130,'null':1,'list':5,'long':1,'double':2,'decimal':1,'boolean':2,'float':1}`
（`null` 来自同事的 `engine_disp`）—— 从 3 种变成 **8 种**。

## 13.3 浮点 / BigDecimal 能否过 `.rpx` 往返：**能**

同 13.1 线路 A。`Float(1.5)`、`BigDecimal("100.00")`、`BigDecimal("3.14159")`
在 write→read→calc 后 `type` 与 `value` 逐字符不变，故本夹具全部走字面量线路，
不需要用表达式硬凑。

## 13.4 复现命令

```bash
cd F:/project/report/rpx/tests/fixtures
"D:/Tools/jdk-21.0.2/bin/javac.exe" --release 8 -Xlint:none \
  -cp "F:/project/report/reportlite/lib/*" *.java -d out
"F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
  rpx.fixtures.ProbeTypes out > probe_types_out.txt 2> probe_types_err.txt
"F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
  rpx.fixtures.GenEngineFixtures .            # OK: 9 engine fixtures generated
"F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
  rpx.fixtures.GenEngineFixtures --verify .   # VERIFY OK: 9 engine fixtures generated
python _shape_audit.py
```

## 13.5 可复现性（本轮回传依据）

- `javac --release 8` exit 0；新增/改动 Java 文件均 ≤300 行。
- `--verify .` → `VERIFY OK: 9 engine fixtures generated`（9/9 PASS）。
- 生成前后对 24 个既有产物做 sha256 diff：**仅新增 engine_types 的 3 个文件，
  其余 24 个（含 engine_disp）逐字节相同**。
- Rust 侧：`cargo test --workspace --no-fail-fast` **93 passed / 0 failed**；
  `cargo clippy --workspace --all-targets -- -D warnings` exit 0。

## 13.6 同步更新的硬编码清单

| 文件 | 改动 |
|---|---|
| `GenEngineFixtures.java` | `NAMES` 加 `engine_types`（236 行） |
| `EngineModels.java` | `build()` 加 1 行分派（243 行） |
| `crates/rpx-engine/tests/common/loader.rs` | `all_names()` → 9 项（含 `engine_disp` + `engine_types`，276 行） |
| `crates/rpx-engine/tests/fixture_loader.rs` | `len()==9`；模板/计算态两张表合并为 `SHAPE`（8 夹具，298 行） |
| `crates/rpx-engine/tests/fixture_types.rs` | **新增**，`engine_types` 类型面断言（3 测试，110 行） |
| `crates/rpx-engine/tests/units_contract.rs` | `FIXTURES` → 7（加 `engine_disp`；**排除** `engine_types`：无扩展格 ⇒ block_height=0，行数公式不适用） |

> **并发提示**：本轮 `EngineModels.java` / `GenEngineFixtures.java` 一度被两人同时编辑
> （先落 `engine_disp`，后落 `engine_types`），出现过一次中间态编译失败
> （`找不到符号 dispFixture()`）。共享分发文件需串行化。

---

## §14 分组头动态生成验证（总监亲做，2026-10-12）

**背景**：spec §8 挂着一条 ⚠「分组头动态生成未验证」，此前表述为
「按分组键动态生成（`ds1.group(...)` + `DSGroup`）未测」，并推测
「`-90` 打在会扩展的锚点行上 → 扩展把 `-90` 复制到每个插入行 → `SimplePager.java:261` NPE」。

**探针**：`ProbeGroup.java`（S1–S8 八个场景）。方法不是读源码推断，而是
**「模板 rowTypes ↔ 计算态 rowTypes」逐行对拍** —— 这是判决「引擎是复制 `-90` 还是合成 `-90」的唯一硬证据。

```
编译  "D:/Tools/jdk-21.0.2/bin/javac.exe" --release 8 -Xlint:none \
      -cp "F:/project/report/reportlite/lib/*" *.java -d out        # exit 0
运行  cd F:/project/report/rpx/tests/fixtures        # cwd 必须在此
      "F:/project/report/common/jre/bin/java.exe" -Dfile.encoding=UTF-8 \
        -cp "F:/project/report/reportlite/lib/*;out" rpx.fixtures.ProbeGroup \
        > probe_group_out.txt 2>&1
```

**结论 1（推翻旧表述）**：引擎**从不合成** `-90`。它是纯模板期静态属性
（`IRowCell.java:21`），由 `ExtCellSet` 构造器 latch（`:151-159` → `_$12 |= 2`）。
`-90` 的复制来自 `extendRow` 的行带克隆，**与 `group()` 无关**。

**结论 2（NPE 触发条件精确化）**：不是「行数多」，而是「`-90` 被复制成大量 `-90`」。

| 场景 | 构造 | 计算态 | 分页 |
|------|------|--------|------|
| S5 | 40 行 + `-90` 在锚点行 | 43 行，**40 个 -90** | ❌ NPE `SimplePager:261` |
| S7 | 40 行 + `-90` 在**非锚点行** | 44 行，**1 个 -90** | ✅ 11 页 |
| S8 | 3 行 + `-90` 在锚点行 | 6 行，3 个 -90 | ✅ 1 页 |

**结论 3（反直觉）**：`DSGroup.calculate` 在**单个分组键**时返回**标量** `list.get(0)`
（`DSGroup.java:139-141`），只有 `>= 2` 键才返回 `List` 触发扩展。
`group()` 的真实作用只是 `isExtended() == true`（`:23-25`）⇒ 成为扩展源 ⇒ 决定**行数**。

**落点**：`docs/format-contract.md` **附件 J**（J.1–J.6，含 M2 硬约束）；
`docs/spec-m2.md` §8 两条 ⚠ → ✅。

**可复现性**：两次运行关键行 `diff` 完全一致。

---

# 13. 函数节点优先级的真机取证（M6 孤儿表处置）

> 产出者：mvp-dev-expert-team-backend（贝洛奇）
> 起因：独立验证方变异 **M6**（把 `lefthead` 优先级 17 改成 19）**存活** —— 133 全绿。
> 根因已核实：`fun_priority` 只有它自己的单测读，`parser` 根本不查它 ⇒ 永真式断言。

## 13.1 探针 `ProbeFunPriority.java`（166 行）

直接读产品自己建出来的树（`Expression.getHome()` 是 public，`Expression.java:66`），
逐节点打 `getPriority / isOperator / isExtended / getLeft / getRight`。

```bash
cd F:/project/report/rpx/tests/fixtures
"D:/Tools/jdk-21.0.2/bin/javac.exe" --release 8 -Xlint:none \
  -cp "F:/project/report/reportlite/lib/*;out" ProbeFunPriority.java -d out
"F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*;out" \
  rpx.fixtures.ProbeFunPriority 2>/dev/null > probe_fun_priority_out.txt
```

证据文件：`probe_fun_priority_out.txt`（GBK 原文）+ `probe_fun_priority_out.utf8.txt`（UTF-8 版）。

**踩坑记录**：`lefthead`/`tophead` 传 `cs=null` 必 NPE ——
`LeftHead.setParameter:26` 做 `(ExtCellSet) cs`、`:37` 调 `getSource`。
所以判 17 优先级必须先造一个真实计算矩阵（`CalcProbe.calc(rd, null)`）。

## 13.2 真机树形（结论：**优先级确实改变树形**）

| 表达式 | 产品真实树 | 判定 |
|--------|-----------|------|
| `lefthead(A1)` | root=`LeftHead` **pri=17** | ✓ 表对（`LeftHead.java:22`） |
| `mcorder(1)` | root=`McOrder` **pri=16** | ✓ 表对（`McOrder.java:110`） |
| `sum(1)` | root=`Sum` **pri=19** | ✓ 默认（`Node.java:54`） |
| `-sum(A1)` | root=`Negative(18)` right=`Sum` | 对照：19 时前缀正常挂上 |
| **`-lefthead(A1)`** | root=`LeftHead(17)`，left/right 均 null | ⛔ **`Negative` 被孤立丢弃** |
| **`-mcorder(1)`** | root=`McOrder(16)` | ⛔ **同上** |
| **`-lefthead(A1)*2`** | root=`Multiply(14)` left=`LeftHead` | ⛔ `Negative` 被丢弃，乘号正常 |
| `not lefthead(A1)` | root=`Not(16)` right=`LeftHead` | ✓ `16<17` 下探成功 ⇒ 无差异 |
| `lefthead(A1)+2` | root=`Add(13)` left=`LeftHead` | ✓ `13<17` 与 `13<19` 都成立 ⇒ 无差异 |

所有形状的产品 `isExtended` 都是 **false** ⇒ **对 M2 扩展算法零影响**。

## 13.3 机制：产品会把前驱子树**静默丢弃**

`Expression.java:442-450` 的右脊下探 + `Node.setLeft` 是**空实现**（`Node.java:64-65`）：
当 `parent == null` 且 `node` 是 `Function`（`Function` 不覆写 `setLeft`）时，
`node.setLeft(right)` 落空、`home` 被覆盖 ⇒ **前驱子树丢失**。
`-lefthead(A1)` 的减号就这样没了 —— 这是**产品的缺陷**，但它是必须对齐的行为。

**Pratt 在结构上没有"丢弃已建子树"这个操作**（递归下降只会往上挂）。
所以"把 `fun_priority` 接进 parser"只会造出**第三种**树形
（我们的 `-(lefthead(A1))` vs 产品的 `lefthead(A1)`），既不忠实，
又把一个 P1 级语义差异混进 P0 路径。**要忠实复刻只能移植线性右脊算法。**

## 13.4 处置（选 B，且不留永真式单测）

| 动作 | 说明 |
|------|------|
| **删掉永真式单测** | `function_priority_table_matches_the_java_assignments` 已移除 |
| 三个数字提为**具名常量** | `FUN_PRIORITY_LEFTHEAD/TOPHEAD/MCORDER/DEFAULT`，出处逐条挂在 `names.rs` |
| 加 ⛔ 横幅 | `names.rs` 明写「本表当前无人消费，且是**刻意**的」+ 何时接线 + 接线时必须同时做的三件事 |
| **加接线绊线** | `expr/tests/deviation/wiring_tripwire.rs` 两条测试，钉住 parser **当前真实树形** |
| 摘掉 crate 根 `pub use` | 不让它看起来像"已被消费的公开 API" |

`grep -rn "fun_priority" crates/ --include=*.rs` 现在只有：**定义 + 文档引用**，零调用方。

### 变异验证：绊线有牙

| 变异 | 结果 |
|------|------|
| **M6-wire**：在 `parse_prefix` 里按 `fun_priority` 丢弃低优先级调用的前缀（= 接线 + 复刻 orphaning） | `prefix_operator_before_a_low_priority_call_is_kept_by_our_parser ... FAILED`<br>`the_known_divergence_from_the_products_orphan_behaviour_is_pinned ... FAILED`<br>89 passed / 2 failed |
| 还原 | 91 passed / 0 failed |

⇒ **真正的"接线"会被杀死**；只有"改表里的数字"这个不改变任何行为的变异仍存活 ——
这是**正确的**，因为该表已被显式声明为无人消费，不再是悬空的承诺。

## 13.5 何时接线（§12.1 第 8 步）

接线时必须**同时**做三件事，缺一不可：

1. 决定是否连 orphaning 一起复刻（复刻 ⇒ 改用线性算法；不复刻 ⇒ 文档里显式记差异）；
2. 补**能观测接线**的回归（断言树形随优先级变化，而不是断言"查表返回表里的值"）；
3. 删掉 `names.rs` 的 ⛔ 横幅 —— 它存在的唯一目的就是提醒"这里有未消费的能力"。

---

# 14. M2-22：`engine_rowheight` —— 行高属性表达式基准（新增夹具）

> 产出者：mvp-dev-expert-team-backend（贝洛奇）· 产品 writer 产出，`--verify` PASS
> 起因：architect 勘误 —— 属性表达式求值（`calcProperties`）是 **P0 而非 P2**，
> 行高可以由表达式算出（`ExtRowCell.java:370-380`，key=1 → `rowHeightExp`）。

## 14.1 为什么这个夹具不可省（零覆盖的失效模式）

在此之前 **9 个夹具的行高全是模板里的定值**（`engine_pager` 是 `setRowHeight(20f)`），
所以「**行高算错 ⇒ 页数错**」这条失效模式在基准里**根本测不出来**。
而分页按「页高 / 行高累加」切页 —— 没有这条基准，M2-11 写出来也无从对拍。

## 14.2 模型（`EngineModelsM2.java`，80 行）

模板 `ReportDefine2(6, 3)`，数据集 12 行（`engine_rowheight.csv`）：

| 模板行 | 模板行高 | 行高表达式 | 说明 |
|--------|----------|-----------|------|
| 1 | 10 | — | 表头 |
| 2 | 10 | `40` | **纵向扩展锚点**（`ds1.select(region/qty)`） |
| 3 | 10 | `25*2` | **不扩展**的行，照样挂行高表达式 |
| 4 | 10 | `20+5+2` | 纯字面量算术 |
| 5、6 | 10 | — | — |

**模板行高故意一律 10** ⇒ 计算值必然不同于模板值，差异可观测。

## 14.3 地面真值（`engine_rowheight.expected.json`）

`row_cells[].computed_row_heights` = **全部 17 个计算态行高（按行序）**：

```text
[10, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 50, 27, 10, 10]
 └头 └──────── 12 行继承锚点的 40 ────────┘ └25*2┘ └20+5+2┘ └─ 模板 5、6 ─┘
```

⇒ 同时钉住三件事：
1. **表达式真的被求值**（40 / 50 / 27 三个不同结果）；
2. **插入行继承锚点的计算行高**（12 个插入行全是 40）；
3. **扩展行与属性表达式是两条独立通道**（模板行 3 不扩展，但行高照样算成 50）。

### 因果：行高 ⇒ 页数（这正是 lead 要的「时序」证据）

```text
页 1 = 8 行 = 10 + 7×40 = 290mm ≤ 297mm（可用高）  → 第 9 行会是 330mm > 297，故切页
页 2 = 9 行 = 4×40 + 50 + 27 + 10 + 10 = 257mm
合计 587mm ⇒ page_count = 2
```

**反事实核算**：若 17 行全用**模板**行高 10，则合计 170mm ⇒ **1 页**。
⇒ 夹具同时给出「行高算出来是 2 页」与「行高错了会是 1 页」两个数，
**「行高错 ⇒ 页数错」这条因果被直接钉死**，也就钉死了「属性表达式在分页之前求值」
（时序依据：`ExtCellSet.java:1504` 调 `calcProperties`，`reflush()` 在 `:2258`）。

## 14.4 覆盖了哪条属性通道（lead 要求留标记）

`row_cells[]` 里带表达式的行会多出这几个键（**机器可读**）：

```json
{"r":2, "row_height":10.0, "height_exp":"40", "exp_channel":"row-builtin-key1",
 "computed_anchor_row":2, "computed_row_heights":[...],
 "not_covered": ["customPropEngMap"]}
```

- ✅ **覆盖**：**内置行属性通道**（`ExtRowCell` 的 prop expMap，key 1/2/7/13/15），
  本夹具只用 **key 1（行高）** → `exp_channel: "row-builtin-key1"`（正向标记）。
- ⛔ **未覆盖**：`customPropEngMap` 那条通道（`ExtRowCell.java:93-95`、`:406-437`，
  求值走 `getSingleValue` 拆 List，与内置通道规则不同）→ `not_covered: ["customPropEngMap"]`。

**为什么两个键都要**：只有正向标记的话，读的人无法从"覆盖了哪条"推出"产品还有哪条没测"。
边界只活在文档里等于没有边界 —— team-lead 的一次质询 precisely 抓到了这个缺口
（我曾口头声称"已写进夹具"，而当时产物里只有正向标记，**报告与产物不符**）。

**为什么放在 `row_cells[]` 而不是顶层**：顶层是 loader 的 12 键白名单
（`common/loader.rs:54-69` 的 `expect_keys`），多写一个键会让**所有**夹具的 `load()`
直接 panic —— 属于"必须先改 loader 才能发产物"的顺序危险。`row_cells[]` 的未建模键会被
`parse_row_cells` 收进 `RowCellInfo.extra`，**既安全又可断言**：

```rust
fx.row_cells.iter().flat_map(|rc| rc.extra.iter()).any(|(k, _)| k == "not_covered")
```

若将来 backend-2 愿意把顶层白名单扩到 13 键，可升级成顶层 `coverage` 字段 —— 那才是
语义上更正的落点，届时 `not_covered` 可搬上去。

## 14.5 两个我踩过并修掉的坑（留给后来者）

1. ⛔ **不能拿模板行号索引计算态网格** —— 扩展在锚点下方插入 N-1 行，两套行号不再一一对应。
2. ⛔ **不能靠 `source.src_row` 反查模板行** —— 扩展插入的格，`src_row` 报的是**它自己的**
   行号（`engine_sum` 里 `(3,1)` 的 `src_row` 就是 3）。第一版就是这么错的，
   三行的计算行高全读成 40（锚点的值），**靠肉眼看数字才发现**。
   ⇒ 最终改成「按计算态行序倒出全部行高数组」，不需要任何映射，对分页实现更直接。

## 14.6 回归证据与同步项

```
原有 9 个夹具的 27 个文件（.rpx/.csv/.expected.json）字节不变 = 27 / 27
新增 3 个：engine_rowheight.{rpx 3510B, csv, expected.json}
GenEngineFixtures --verify . → VERIFY OK: 10 engine fixtures（10/10 PASS）
```

`loader_crosscheck.py::PENDING_IN_LOADER` 已登记一条（附原因与接手人），
脚本对该条目**容忍 SHAPE 暂缺**（否则会长期红灯），但**未登记**的新夹具仍会把它打红。

**待 backend-2 同步**（他们的文件，我不越界改）：
- `common/loader.rs::all_names()` 加 `"engine_rowheight"`；
- `fixture_loader.rs::SHAPE` 补一行：**`(6, 3, 17, 3, 42, 18, 24)`**
  （模板 6×3、计算态 17×3、42 非空格、18 带 source、24 不带）；
- `GRID_FIXTURES` 常量同步加一项。

---

# 15. #17 消歧 1：多选参数（`@名` 绑 List）—— 真机已验证

> 产出者：mvp-dev-expert-team-backend（贝洛奇）
> 结论：**architect 的源码推断成立**，且真机证据比推断更细（两个边界都是 true）。

## 15.1 探针 `ProbeParamList.java`（163 行）

走产品**唯一**的 `isExtend()` 调用点那条路径（`ExtNormalCell.java:510`，
构造表达式时 `ds` 恒为 null，见 `:498-504`）：
`new Expression(cs, null, ctx, expStr, false).isExtend()`；再补一层全量 `Engine.calc()`。

前置事实核过：`Context.setParamValue` 写 `params` map（`Context.java:163-168`），
`isParamName` 查 `containsKey`（`:133-135`）⇒ 登记即可用。

证据文件：`probe_param_list_out.txt`（GBK）+ `probe_param_list_out.utf8.txt`。

## 15.2 A/B 结果（`@p` 与 `@@p` 行为完全一致）

| 参数值 | `@p` | `@@p` | 全量 calc 的后果 |
|--------|------|-------|-----------------|
| `Integer(7)` | false | false | 格不变（3×2） |
| `String("x")` | false | false | 格不变 |
| **`List[1,2,3]`** | **true** | **true** | **矩阵 3×2 → 5×2，锚点 `extend=18 subCells=3`** |
| **`List[9]`（单元素）** | **true** | **true** | `extend` 被改写成 **18**，但 `subCells=1` |
| **`List[]`（空）** | **true** | **true** | ⚠ **成为扩展格却没有子格** |

树根一律是 `Constant(pri=19)` ⇒ 证实产品建的就是带值 `Constant`，
`isExtend()` 读的正是它的值（`Constant.java:22-24` 的 `value instanceof List`）。

## 15.3 三条比推断更细的发现

1. **判据是 `instanceof List`，不看内容** ⇒ **单元素 List 与空 List 都是 true**。
   源码推断只说"多选参数（值是 List）"，真机补上了这两个边界。
2. ⚠ **空 List ⇒ 是扩展格但没有子格**（`isExtend=true` 而 `subCells` 无从产生）。
   这是 M2 扩展实现必须显式处理的边界，不能默认"是扩展格就一定有 sub_cells"。
3. **单元素 List 也会把 `extend` 改写成 18**，即使 `subCells=1`。
   ⇒ 拓扑改写与"是否真多值"**无关**，只由 `isExtend()` 决定。
   这条对 M2-15 的改写逻辑是硬约束。

## 15.4 对 Rust 侧的影响：规则不变，但**默认值从假设变成有据**

我们已有的规则 `Expr::Param { value } => matches!(value, Some(Value::List(_)))`
**与真机一致，无需改动**。本轮做的是：

- `ast.rs` 的 `Expr::Param` 文档：⛔「未验证」→ ✅「**真机已验证**」，
  附上 15.2 的表与 15.3 的三条边界；
- 回归 `param_bound_to_a_list_value_is_extendable` 扩成 **3 个 List 形态（含空）+ 2 个标量 + 未知值**；
- 变异 **M17**（把 `Expr::Param` 分支改成恒 `false`）⇒ 该测试 FAILED（90 passed / 1 failed），
  还原后 91 passed ⇒ **有牙**。

**「未知时取 false」现在是证据支撑的**：真机上标量参数确实是 `false`，
所以无 `Context` 时保守取 false **不会误判标量**（此前只是假设）。
接线仍属 M2-13 求值切片（那里才有 `Context`）。

## 15.5 一处**我自己的对照组设计错误**（诚实记录）

探针 B 组的对照项 `ds1.select(qty)` 我打出 `isExtend=false`、树根 `Offset(pri=17)`，
这与夹具里"`ds1.select(qty)` ⇒ true"**冲突**。原因**不是**产品行为变了，
而是**我的对照组造错了**：`cs` 传了 null、`ds` 也是 null ⇒
`ctx.getDataSet("ds1")` 返回 null ⇒ `Expression.java:175` 的条件不满足 ⇒
走不到 `createDSNode`，最后落到 `(` 后缀上建了个 `Offset`。

⇒ 这恰好复现了 `names.rs` 文档里那条"`ds` 恒 null ⇒ DS 回退不可达"的同款机制，
**是我该记得的约束却忘了套用**。正确对照组必须带真实数据集（像
`ProbeFunPriority` 里那样先 `CalcProbe.calc` 造出 `ExtCellSet` 当 `cs`）。
不影响 §15.2 的结论（那几项都不依赖数据集），但这条对照**不作为证据采信**。

## 15.6 ⛔ 构造纪律：造 `DSSelect` 必须**同时**给 `cs` 与 `ds`

（team-lead 要求写进 RUNLOG，防止下一个人重踩）

我在 `ProbeParamList` 的对照组里犯的错：`new Expression(null, null, ctx, "ds1.select(qty)", false)`
⇒ 打出 `isExtend=false`、树根 `Offset(pri=17)`，与夹具里"`ds1.select(qty)` ⇒ true"**冲突**。

**根因**（`Expression.java:175`）：

```java
int index2 = id.lastIndexOf(46);
if (index2 > 0
    && (tempDS = ctx.getDataSet(dsName = id.substring(0, index2))) != null   // ← cs/ctx 里没有 ds1 就到此为止
    && (node2 = this.createDSNode(cs, tempDS, id.substring(index2 + 1), ctx, true)) != null) {
    return node2;
}
```

⇒ **只给 `ctx` 里的 DataSet 不够**：这条路径要求 `ctx.getDataSet("ds1")` 返回非空，
而 `ds1.select(...)` 的识别**根本走不到** `(` 后缀那个分支之前。
结果 `id` 落到后面 → 因为下一个 token 是 `(` → 建了个 `Offset(pri=17)`。

**正确做法**（`ProbeFunPriority` 用的就是这个）：
1. 先 `CalcProbe.calc(rd, null)` 造出真实的 `ExtCellSet` 当 `cs`；
2. 数据集要挂在**同一个 ctx** 上（`CalcProbe.calc` 自建 Context，所以要走
   `EngineModels` + `attachFileDataSet` + 让产品 `FileDataSetFactory` 从 CSV 读）；
3. 造 DS 表达式时 **`cs` 与 `ds` 都要给**。

**教训**：受控组不成立就**不算证据**。发现受控组与已知结论冲突时，
第一反应应是"我是不是把受控组造错了"，而不是"产品行为变了"。

---

# 16. head 图真机基线（§12.1 第 5/6 步的前置）

> 产出者：mvp-dev-expert-team-backend（贝洛奇）· **只 dump，不碰 `src/**`**
> 目的：head 图是当前唯一「写错了但测不出」的部分 —— 方向写反、或把两处
> `sameRowTypeAs` 的不对称写漏，结果**依然是一张合法的 head 图**，没有任何断言会失败。

产物：`ProbeHeadMap.java`（逐夹具逐格）、`ProbeHeadMap2.java`（判别性样本），
输出 `probe_head_map_out.utf8.txt` / `probe_head_map2_out.utf8.txt`。

## 16.1 源码侧基线（先读懂再对照数字）

| 位置 | 内容 |
|------|------|
| `ExtCellSet.java:776-777` | `cell.setCellLeftHead(_$3(cell)); cell.setCellTopHead(_$2(cell));` |
| `_$3`（左）`:648-665` | `for(col=cell.col-1; col>=1; --col)` **递减、不含 0**；命中条件 `containRow(cell) && 18==extend`；**无 `sameRowTypeAs`**；兜底 `return _$10`（cell00） |
| `_$2`（上）`:685-704` | `for(row=cell.row-1; row>=1; --row)` **递减、不含 0**；首分支 `sameRowTypeAs && containCol && 19==extend`（`:691`）；`:695` **再判一次** `sameRowTypeAs`；兜底 `return _$10` |
| `ExtExtendCell.java:69-76` | `getLeftSubSize()= _$6.size()`（leftSubs）、`getTopSubSize()= _$5.size()`（topSubs）；**基类 `ExtCell` 两个 size 都 `return 0`** |

## 16.2 逐夹具 head 基线（requirement 1）

`leftHead != cell00` 的反例（`topHead` 一栏**全部为 0**，见 16.4）：

| 夹具 | grid | leftHead 反例数 | 反例明细 |
|------|------|---------------|---------|
| engine_sum | 8×9 | **8** | (2,2)→L(2,1)；(2,3)…(2,9)→L(2,2) |
| engine_ext | 8×7 | **2** | (3,2)→L(3,1)、(3,3)→L(3,1)　← **与已知一致** |
| engine_expr | 8×5 | **4** | (2,2)→L(2,1)；(2,3)…(2,5)→L(2,2) |
| engine_pager | 43×3 | **2** | (3,2)→L(3,1)、(3,3)→L(3,2) |
| engine_merge | 8×4 | **3** | (2,2)→L(2,1)；(2,3)(2,4)→L(2,2) |
| engine_samemerge | 7×3 | **2** | (2,2)→L(2,1)、(2,3)→L(2,2) |
| engine_disp | 6×4 | **3** | (3,2)(3,3)(3,4)→L(3,1) |
| engine_types | 4×4 | **0** | — |
| engine_rowheight | 17×3 | **2** | (2,2)→L(2,1)、(2,3)→L(2,2) |
| engine_cycle | — | 错误型夹具，跳过（`ReportError`） |

**形态要点（反例不是"都是最近"，而是链式）**：`(2,3)…(2,9)` 的 head 是 **(2,2)** 而不是 (2,1)，
因为 `_$3` 递减扫描**命中第一个就返回** —— (2,2) 本身也是 18 扩展格，先被命中。
⇒ 实现若写成"一路走到最左"，这些格会全错。

## 16.3 方向判别样本（requirement 2）—— **两个都是"递减/最近"**

### 上 head（S2）：两个 19 扩展格上下相邻

```text
模板(1,1) → 计算格(1,1) extend=19 leftHead=cell00 topHead=cell00
模板(3,1) → 计算格(3,1) extend=19 leftHead=cell00 topHead=(1,1)
模板(6,1) → 计算格(6,1) extend=16 leftHead=cell00 topHead=(3,1)   ★ 最近，不是 (1,1)
```

⇒ **(6,1) 的 topHead = (3,1)**。递增扫描会返回 (1,1)，**结果不同 ⇒ 判别样本成立**。

### 左 head（S3，受控）：同行两个 18 扩展格

```text
模板(2,1) extend=18 leftHead=cell00
模板(2,3) extend=18 leftHead=(2,1)
模板(2,5) extend=16 leftHead=(2,3)   ★ 最近，不是 (2,1)
```

⇒ 同为**递减（最近）**。

## 16.4 `_$2`（上 head）的两条分支在现有语料里**零覆盖** —— 已用专门样本补上

10 个夹具里 `topHead != cell00` 的格数**全部为 0**：因为所有扩展格都是 **18**（纵向），
而 `_$2` 的首分支只认 **19**（横向）。`ProbeHeadMap2` 的 S1 首次命中：

```text
S1 grid=3×6
  模板(1,1) → 计算格(1,1) extend=19  leftHead=cell00 topHead=cell00
  模板(2,1) → 计算格(2,1) extend=16  leftHead=cell00 topHead=(1,1)   ← 19 分支命中
  模板(3,1) → 计算格(3,1) extend=16  leftHead=cell00 topHead=(1,1)   ← head-of-head 分支命中
```

⇒ `_$2` 的**两条返回路径都被真机证实存在**（19 分支 + 「头的头」分支）。
`_$3` 的第二条路径同样被 S4 证实：连续三个 18 扩展格时 `(2,5).leftHead = (2,3)`
（跳过 `(2,4)`，因为它自己已是 head）。

## 16.5 `sameRowTypeAs` 的两处不对称：源码侧确认，**真机侧尚无法判别**

`_$3`（左）**不调** `sameRowTypeAs`、`_$2`（上）**调两次**（`:691`/`:695`）——
已在源码侧逐行确认（全仓 `sameRowTypeAs` 的调用点只有 `ExtCellSet.java:691` 与 `:695`，都在 `_$2` 里）。
⚠ **真机侧无法判别**：要让这两处产生不同结果，需要"行类型不同"的相邻格样本
（产品 `IRowCell` 的 rowType 差异，如普通行 vs 分组头 `-90` vs 表头 `-95`），
现有 10 个夹具没有这种相邻组合。**标为源码推断，未经真机验证。**

## 16.6 `cell00` 的 `_$5` / `_$6` 成员数（requirement 4）

**先说一条实测事实**：`cell00` 的运行时类型是 **`ExtExtendCell`**（不是基类 `ExtCell`），
所以它的两个 sub 列表**非空**（基类两个 size 都 `return 0`）。

| 夹具 | leftSubSize(`_$6`) | topSubSize(`_$5`) |
|------|--------------------|-------------------|
| engine_sum | 46 | 54 |
| engine_ext | 14 | 12 |
| engine_expr | 26 | 30 |
| engine_pager | 10 | 12 |
| engine_merge | 13 | 15 |
| engine_samemerge | 7 | 9 |
| engine_disp | 13 | 16 |
| engine_types | 16 | 16 |
| engine_rowheight | 16 | 18 |

⚠ 两列**没有恒等关系**（差值在 −2 到 +8 之间摆动），**不要用"topSubs = leftSubs + k"去推**。

## 16.7 源码推断 vs 真机实测（requirement 5）

| 结论 | 性质 |
|------|------|
| 递减扫描、命中即返回、兜底 cell00 | **真机**（S2/S3 判别）+ 源码一致 |
| `_$3` 判 18、`_$2` 判 19 | **真机**（engine_ext/S3 的 18 命中；S1 的 19 命中） |
| `_$2` 存在 head-of-head 第二条路径 | **真机**（S1 的 (3,1)） |
| `_$3` 不调 / `_$2` 调两次 `sameRowTypeAs` | ⚠ **仅源码**（真机判别需"行类型不同"样本） |
| `isMerged() && 18!=extend && getRow()!=xrow` 这类守卫 | ⚠ **仅源码**（未构造出触发样本） |

---

# 17. head 图判别夹具（S1–S4 + `-96` 正反对照）

> 产出者：mvp-dev-expert-team-backend（贝洛奇）· 产品 writer 产出，`--verify` PASS
> 起因：§16 的发现 —— 10 个夹具里 `topHead` **100% 是 cell00**，
> ⇒ `_$2` 无论怎么写都"看起来对"。**没有夹具的路径等于没测。**

## 17.1 产物：2 个夹具（按形状合并，不牺牲可断言性）

| 夹具 | 模板 | 计算态 | 承载样本 |
|------|------|--------|---------|
| `engine_head_top` | 6×4 | 6×6 | **S1**（19 分支 + head-of-head）、**S2**（上 head 递减方向）、行 2 = `-96` 正对照 |
| `engine_head_left` | 5×6 | 11×6 | **S3**（左 head 递减方向）、**S4**（`_$3` 第二条路径）、行 4 = `-96` **反证** |

模型：`EngineModelsHead.java`（133 行）。

## 17.2 `engine_head_top` 实测（grid 6×6，topHead 反例 **4** 个）

```text
(3,1)→T(1,1){src=1,1} extend=19    ← head-of-head 分支（(2,1) 自己不是 head，取它的 topHead）
(4,1)→T(3,1){src=3,1} extend=19    ← 19 分支
(5,1)→T(3,1){src=3,1} extend=19    ← 19 分支
(6,1)→T(3,1){src=3,1} extend=19    ← ★ S2：最近 = (3,1)，不是 (1,1) ⇒ 递减
```

## 17.3 `engine_head_left` 实测（grid 11×6，leftHead 反例 **15** 个）

```text
行 2（-91）：(2,2)(2,3)→L(2,1)；(2,4)(2,5)(2,6)→L(2,3)      ← ★ S3：最近 = (2,3)，不是 (2,1)
行 3（-91）：(3,2)→L{3,1} (3,3)→L{3,2} (3,4)(3,5)(3,6)→L{3,3} ← ★ S4：跳过已是 head 的 (3,4)
行 4（-96）：(4,2)(4,3)→L{4,1}；(4,4)(4,5)(4,6)→L{4,3}      ← 与行 2 形态**逐字相同**（反证）
```

## 17.4 ⛔ 一个必须知道的实现细节：head 的坐标有两个口径

实测形如 `(3,2)→L(5,1){src=3,1}`：

- `getRow()/getCol()` = **head 对象自身**在计算态里的位置（这里是 (5,1)，因为扩展把行下移了 2 行）；
- `getSourceRow()/getSourceCol()` = 它的**模板出处**（这里是 (3,1)）。

**⚠ 精确表述（team-lead 纠正后的定稿，别简化成"会比错"）**：

- 全 12 个夹具、237 个带 `source` 的格中，**calc 坐标 ≠ src 的有 127 个**
  （主要来自 `engine_rowheight` / `engine_pager` 这类大量插入行的夹具）；
- 在 `engine_head_top`（18 个带 src 格）与 `engine_head_left`（30 个）里，
  **格自身的 calc 坐标与 src 坐标恰好全部重合**；
- 但**被引用的 head 目标**上，`engine_head_left` 有 **3 个**自身 ≠ src：
  `(11,1){src=(5,1)}`、`(11,2){src=(5,2)}`、`(11,3){src=(5,3)}`。

⇒ **head 用 `{src_row, src_col}`（模板出处），不是它在计算态里的位置。**

**三层口径，别混（我实测过，数字各不相同）**：

| 层 | 数量 | 其中"自身坐标 ≠ src" |
|----|------|---------------------|
| ① `expected.json` 的 `cells[].source`（= `ecs.getSource(r,c)`，按**模板坐标**查） | 188 个带 src 的格 | **1 个**（在 `engine_merge`）⇒ 这一层两者几乎重合 |
| ② 格自带 source 对象（`cell.getSource()`）vs 格自身坐标 | 535 | 413 |
| ③ **被引用的 head 目标**：`head.getRow()/getCol()` vs `head.getSourceRow()/getSourceCol()` | 167 | **149** |

⚠ 我此前把 ②③ 的数字误当成 ① 的结论（说"head 夹具里两者重合、该断言暂不能区分两种写法"）——
**那是口径用错**（`cell.getSourceRow()` 与 fixture 的 `source` 字段不是同一个东西）。
**准确结论**：

- **对 `cells[].source` 这一层**（①）：两者几乎重合 ⇒ 用它**区分不出** src / CellId 两种写法；
- **对 head 本身**（③）：**149/167 两者不同** ⇒ `left_head`/`top_head` 落进夹具后，
  「用 src」与「用计算态坐标」会**大面积不同** ⇒ 这个选择**有判别力**、必须钉住；
- ⇒ 所以"head 用 src"既是产品语义，也**恰好是唯一能被夹具证伪的写法**。

⚠ 另一条硬约束（team-lead 2026-10 从 `engine_head_left` 坐标表读出，我复核成立）：
**计算态里存在「没有 `source` 的格」，它们同样可以被 head 指向**
（`engine_head_left` 的 `(11,1)(11,2)(11,3)` 的 `source` 为 null，却各被 4 个格当作 left head）。
⇒ **实现时不能把「有没有 `source`」当作「是不是合法 head 目标」的判据。**
（这正是我探针早期版本的 bug：用 `getSource` 枚举格 ⇒ 整段漏掉这些格；已改用 `getCell`。）

## 17.5 `-96` 正反对照：把 `sameRowTypeAs` 的不对称**真机夹住**了

第一版用 `-95` 做，**判别失败**：`ExtNormalCell.sameRowTypeAs:266-269` 把
`-91/-95/-94/-90` 当**同一族**，`-95` vs `-91` 返回 **true** ⇒ 行为不变、判别不出来。

改用 **`-96`（`IRowCell.TYPE_TITLE_HEADER`）**（不在同族集合 ⇒ 走 `return type1 == type2` ⇒ **false**），
构成**正证 + 反证**双向：

| 面 | 夹具 | 行类型 | 实测 | 证明 |
|----|------|--------|------|------|
| `_$2`（上） | `engine_head_top` 行 2 = **-96** | 与上方 19 扩展格不同族 | `(2,1).topHead` 从 `(1,1)` **变成 `cell00`**（反例数 5 → 4） | ✅ **`_$2` 确实查行类型** |
| `_$3`（左） | `engine_head_left` 行 4 = **-96** | 与左侧 18 扩展格不同族 | `(4,4).leftHead = {4,3}`，**与 -91 行形态逐字相同** | ✅ **`_$3` 不查行类型**（反证） |

⚠ 行类型已**落进 `expected.json` 的 `row_cells[].row_type`**，所以判别依据是机器可读的：

```text
engine_head_top  row_types: [(1,-91), (2,-96), (3,-91), (4,-91), (5,-91), (6,-91)]
engine_head_left row_types: [(1,-95), (2,-91), (3,-91), (4,-96), (5,-91)]
```

## 17.6 回归与同步

```
本轮只新增 6 个文件（2 夹具 × .rpx/.csv/.expected.json），无既有文件改动
GenEngineFixtures --verify . → VERIFY OK: 12 engine fixtures（12/12 PASS）
```

**待 backend-2 同步**（数字已算好，见 `loader_crosscheck.py::PENDING_IN_LOADER`）：

```text
all_names()      += "engine_head_top", "engine_head_left"
SHAPE            += ("engine_head_top", 6, 4,  6, 6, 22, 18,  4)
                  += ("engine_head_left", 5, 6, 11, 6, 66, 30, 36)
GRID_FIXTURES    += 两个名字
```

✅ 勘误（2026-10-12 team-lead）：上一条**归属记错了**。`tests/fixtures/ProbeMergePages.java`
是 **backend-2 的产物**（多页合并取证探针），不是 team-lead 的；当时的编译错误是 backend-2 漏写
`throws Throwable`，**现已修复**，全量 `javac --release 8 *.java` **exit 0**。
⚠ 当时为不卡住验证而用的 `ls *.java | grep -v ProbeMergePages` **workaround 已撤销** ——
它是临时排查手段，若留在复现命令里会让后续读者静默跳过这个文件。

---

# 19. 多页合并：坐标口径取证（`ProbeMergePages.java`）

> ⚠ 编号说明：本节写于 §18 已被架构师占用之后，故物理位置在 §18 **之前**；
> 为免两个 `# 18.` 并存，本节改为 §19。内容与 §18 无关。

> 产出者：backend-2 · 探针 `ProbeMergePages.java`（197 行）· 原始输出 `probe_mergepages_out.txt`
> 目的：现有 3 个有 `merged.cells` 内容的夹具**都是单页**，所以「页内坐标」与「全局坐标」
> 恰好重合、从未被证伪。`CalcMergeJson.renderMerged`（`:37-59`）在 `pg` 上循环，
> 反编译读法是**页内坐标**，但必须有真机样本佐证 —— Rust 侧的交叉校验依赖它。

## 19.1 两个变体的实测

各 40 数据行、行高 20mm、595x842、`unit = 1`：

| 变体 | 模型 | 页数 | 每页行数 | 每页合并格 | `r` 范围 | 跨度 |
|---|---|---|---|---|---|---|
| A | `mergeSame=98` 纵向，region 成对重复 | 4 | 14/14/14/**1** | 12/12/12/**0** | [2,13] / [2,13] / [2,13] / — | 全部 `row_merge=2 / col_merge=1` |
| B | 模板纵向合并 `A2:A3` | 6 | 14×5 + 12 | 12×5 + 10 | [2,13]×5 / [2,11] | 全部 `row_merge=2 / col_merge=1` |

## 19.2 结论

* ✅ **`r` 确实是页内坐标**（从 1 起、**每页重新计数**）：两个变体共 10 页，
  `r` 始终落在 `1..=该页行数` 内。**与 `CalcMergeJson.java:37-59` 的读法一致，契约无修正。**
* ⛔ **`page` 不保证覆盖每一页**：变体 A 末页只有 1 行、无可合并 ⇒ 该页 **0** 个合并格。
  所以「每页都应出现在 `merged.cells` 里」是**错的**判据。
* ⛔ **模板纵向合并会传播到每一页**，不只停在模板行所在的那一页（变体 B **6/6** 页都有合并格）。
  所以「合并只出现在模板行所在页」同样是**错的**判据。

## 19.3 顺带被推翻的一条规则（head 图夹具的功劳）

原 `subcell_assert.rs` 归纳的是「纵向附属格行 = `src_row + k * row_merge`」，
在**当时 8 个纵向锚点**上 100% 吻合。加入 `engine_head_left` 后**被证伪** ——
该夹具同一列有**三个**纵向锚点，实测附属格行为：

```text
anchor(2,1) src_row=2 → [2,3,4]     ← 首行 == src_row（巧合：它是本列第一个）
anchor(3,1) src_row=3 → [5,6,7]     ← 首行 5 ≠ 3
anchor(4,1) src_row=4 → [8,9,10]    ← 首行 8 ≠ 4
```

⇒ **`src_row` 是模板态坐标，扩展带在同列是共享的**：后面的锚点被前面锚点占掉的行数**依次下推**。
与「不能用模板行号索引计算态网格」「19 锚点行号由版面决定」属**同一类**陷阱。

已把断言修正为在**全部 11 个纵向锚点**上实测零违反的性质：
同列 / 严格升序且不重复 / 步长 = `row_merge` / 首行 `>= src_row` / 同列锚点区间不重叠。
**没有**直接放宽成"不断言" —— 那样会丢掉坐标判别力。

## 19.4 留给后端的登记材料（我按约定**不碰** `NAMES`）

模型已备好在探针里，公开方法 `ProbeMergePages.multiPageMergeModel()`，接线三行：

```java
// GenEngineFixtures.NAMES：追加 "engine_mergepages"
// EngineModels.build(name)：加分支 return ProbeMergePages.multiPageMergeModel();
// EngineModels.rows(name) ：该夹具用探针内 rows()（region 成对重复 A,A,B,B,C,C,D,D…）
```

变体 A 的形状（4 页 14/14/14/1、每满页 12 个合并格、**末页 0 个**）正好把 18.2 的两条
「错的判据」钉成 ground truth。

---

# 18. #17 消歧 3：`Escape.remove` 完整转义规则（真机已验证，含一处新差异）

> 产出者：mvp-dev-expert-team-backend（贝洛奇）
> 结论：真值表**基本逐条对齐**，修掉了 1 处实现差异；并**意外发现一条我们与产品不一致**的行为。

## 18.1 源码规则（`Escape.java:173-214`，逐行读出）

| 输入 | 输出 | 依据 |
|------|------|------|
| `\t` / `\r` / `\n` | 9 / 13 / 10 | `:191-202` |
| 其它 `\x` | **只留 `x`（反斜杠被丢掉）** | `:203-206` 的 `default` |
| 非 `\` 字符 | 原样 | `:208-210` |
| **结尾的孤立 `\`** | **整段丢弃**（不留反斜杠） | `:188` 的 `if (++i == len) break;` |

## 18.2 真机实测（`ProbeEscape.java`，输出 `probe_escape_out.utf8.txt`）

### A. 字符串字面量入口（`Expression.java:121-123` 直调 `Escape.remove`）

| 输入 | 产品计算出的值 | 性质 |
|------|--------------|------|
| `'a\tb'` | `a<TAB>b` | ✅ 真机 |
| `'a\nb'` | `a<LF>b` | ✅ 真机 |
| `'a\xb'` | `axb` | ✅ 真机（**未知转义丢反斜杠**） |
| `'a\\b'` | `a\b` | ✅ 真机 |
| `'a\"b'` | `a"b` | ✅ 真机 |

### B. `$[...]` 入口（`Expression.java:169-171`，先 `trim()` 再 `Escape.remove`）

| 输入 | 产品计算出的值 | 性质 |
|------|--------------|------|
| `$[abc]` / `$[ a b ]` / `$[a\tb]` / `$[a\xb]` | `abc` / `a b` / tab / `axb` | ✅ 真机 |

⇒ **实现改动**：`lexer.rs::unescape` 的 `None => out.push('\\')` 改成 **`None => {}`（丢弃）**，
与 `Escape.java:188` 对齐；并把真值表连同行号写进函数文档。
新增回归 `string_escapes_follow_escape_remove_exactly`（9 条断言覆盖全表）。

## 18.3 ⛔⛔ 真机**证伪了我原本的假设**，并暴露一条真实差异

我原以为 `$[a\]` 的内容是 `a\`、经 `Escape.remove` 后应为 `a`。**真机不是这样**：

```text
表达式 $[a\]   → 抛异常 ReportError "单元格A2中有错误: 表达式：$[a\]中: 未知的单元格：$"
```

⇒ 产品的 `Sentence.scanBracket` **不把 `\]` 当转义** ⇒ 找不到配对的 `]` ⇒
`$[...]` 分支根本不成立 ⇒ 落到 `$` 的格引用路径 ⇒ 报"未知的单元格：$"。

⇒ **我们与产品在这里不一致**：我们的 `take_group` 会把 `]` 配成对 ⇒ 返回常量 `a`。
**这已登记为缺口**（`ast.rs` / `deviation.rs` 的断言里明写"这是已知差异，不是正确行为"）。

**连带结论**：`:188`「尾部孤立反斜杠被丢弃」这条规则虽然源码明确，但**真机两条入口都到不了**
（字面量入口引号不闭合；`$[...]` 入口如上报错）⇒ 该断言标为 **源码依据、真机不可达**，
实现按源码逐行对齐（比猜一个行为保守），但不宣称已验证。

## 18.4 `{...}` 相邻原子：**登记为缺口（不是"待造夹具"）**

已确认**当前造不出判别正例**（无 `ValueList(19)` 前驱的构造），按约定写法登记：

> **`{...}` 相邻原子缺口**：`_$3`/`_$2` 走通用 `Expression.java:445` 的 `node.setLeft(right)`
> 后，`Select` 的 `left` 可能是**前驱的高优先级可扩展节点**（如 `ValueList` 的 `isExtended()` 为真），
> 此时产品的 `Select.isExtended()` 为 **true**，而本切片恒判 **false**。
> **触发条件**：存在形如 `[1,2]{x}` 的相邻原子（左侧是优先级 19 且可扩展的节点）。
> **当前无法构造判别正例** ⇒ 该差异**未被任何夹具覆盖**，实现按 false 处理并在
> `ast.rs` 的 `Expr::Select` 分支注明。
> ⛔ **等能构造出「高优先级可扩展前驱」时必须回来补正例**（例如能生成 19 扩展且左邻可扩展的夹具时）。
> 这**不是"还没做"，是"做不了"**——不要按"待造夹具"理解。

## 18.5 自检

```
cargo test -p rpx-engine --lib → 94 passed / 0 failed（较上轮 +2 条转义回归）
cargo test --workspace → 见回报
cargo clippy -D warnings → 见回报
ProbeEscape.java 编译通过（`javac --release 8 *.java` **全量**、exit 0；早期「排除 team-lead 在途的 ProbeMergePages.java」是**误判**——该文件是 backend-2 的产物且已修好，排除会静默跳过别人的探针，理由见 §17 末勘误）
```

## 19.5 更一般的结论：`src_row` → 计算态行号，纵向与横向是**两种不同的映射**

> 本节由 team-lead 复核两个样本后要求补记；它**取代**了我原先的「19 锚点行号 = 上方纵向
> 扩展块占满后的下一行（3+5=8）」那条单点拟合（该拟合来自 `engine_ext` 一个样本，
> 已被 `engine_head_top` 证伪，线索已撤回）。

| 方向 | 样本 | `src_row` | sub_rows | 关系 |
|---|---|---|---|---|
| 纵向 18 | `engine_head_left` 锚点 1/2/3 | 2 / 3 / 4 | `[2,3,4]` / `[5,6,7]` / `[8,9,10]` | **模板行 + 同列前面锚点累计占用的行数** |
| 横向 19 | `engine_ext` | 4 | `[8,8,8,8,8]` | **无规律** |
| 横向 19 | `engine_head_top` | 1 | `[1,1,1,1,1]`…（行=1） | **与上一条相反** |

⇒ **横向锚点的行号归属仍是未知**。两个样本已互相矛盾（一个被下推到第 8 行、一个留在模板行），
所以**不要试图拟合**。当前 Rust 侧只钉「同属一行 + 列走向」，这是正确的保守边界。

⇒ 反过来，纵向那条**也不是** `src_row + k*row_merge`（已被 `engine_head_left` 证伪），
而是「**同列共享扩展带、按锚点顺序依次下推**」。

**给实现方的硬约束**：`source.src_row` 是**模板态**坐标，
**不可**直接当作计算态行号使用；同一列多个纵向扩展格时，后面的锚点会被前面占用的行数顶下去。

---

# 19. head 字段落进 `expected.json`（M2-09 / §12.1 第 5/6 步的前置）

> 产出者：mvp-dev-expert-team-backend（贝洛奇）· team-lead 批准
> 之前 head 真相**只在探针文本里**，Rust 侧无法断言；现在它进了夹具。

## 19.1 格式（team-lead 定，我照做）

```json
"left_head": {"src_row": 1, "src_col": 1}
"top_head":  {"src_row": 3, "src_col": 1}
```

- 用 **`{src_row, src_col}` 对象**：`(0,0)` 唯一表示 `cell00`，裸数字没有这个约定；
- **可选键**：`getCellLeftHead()` 为 null 时**省略该键**，**不写 `null`**
  （与 `source` 一致，避免 loader 的 `expect_keys` 与 `Option` 语义打架）；
- 从**格对象自身**取 head（`cell instanceof ExtCell`），**不从 `source` 推** ——
  因为 head 目标**可以没有 source**（见 §17.4 的硬约束）。

## 19.2 实测（`engine_head_top` 第 1 列）

```text
(1,1) left_head={0,0}  top_head={0,0}     ← cell00 用 {0,0} 表示
(2,1) left_head={0,0}  top_head={0,0}     ← 行 2 是 -96 行 ⇒ sameRowTypeAs=false ⇒ 落 cell00
(3,1) left_head={0,0}  top_head={1,1}     ← head-of-head 分支
(4,1) left_head={0,0}  top_head={3,1}     ← 19 分支
(5,1) left_head={0,0}  top_head={3,1}
(6,1) left_head={0,0}  top_head={3,1}     ← ★ S2：最近 (3,1)，不是 (1,1)
```

## 19.3 回归与同步状态

```
GenEngineFixtures --verify . → VERIFY OK: 12 engine fixtures（12/12 PASS）
.rpx / .csv 共 24 个文件字节不变 = 24 / 24（只有 expected.json 变）
```

⛔ **当前 `cargo test` 是红的**：`tests/common/loader.rs:172` 的 `expect_keys("cell", [...])`
白名单尚未加 `left_head` / `top_head` ⇒ `load()` 见到新键就 panic
（`fixture_types.rs` 的三个测试因此 FAILED）。已私信 backend-2 给出三处改法
（白名单 + `Cell` 字段 + `parse_cells` 解析）。**他同步后即全绿，无需我这边再动。**

## 19.4 合并格分页（`ProbeMergePages`，证据 `probe_mergepages_out.utf8.txt`）

**⚠ 先纠正一个流传中的说法**：此前"多页**横向**合并逐页各 12 格；多页**纵向**合并 0 格"**方向说反了**。
实测两组都是 `rm/cm = 2/1` ⇒ **全部是纵向合并**（`col_merge=1` 即非横向）。

| 组 | 页数 | 合并格总数 | 每页格数 |
|----|------|-----------|---------|
| A：`mergeSame=98` 打在扩展列，40 行 | 4 | 36 | page1/2/3 各 **12**，page4 **0**（该页只有 1 行） |
| B：模板合并 `A2:A3`，40 行 | 6 | 70 | page1..5 各 **12**，page6 **10** |

⇒ 正确措辞（两种解释**未区分**）：
**已观测：多页下纵向合并逐页都有格（A 组 page1-3 各 12、B 组 6 页全有）；
「page4 为 0」可由「该页只有 1 行、没有可合并的行」解释，也可由「末页跳过合并」解释，
单页/末页夹具上这两种解释无法区分。**

⚠ 另：12 个**夹具**里**没有任何「多页 + 合并格」的组合**
（`engine_pager` 4 页、`engine_sum` 2 页、`engine_rowheight` 2 页的 `merged.cells` 全为 0；
有合并格的 `engine_ext`(1 页/35 格)、`engine_merge`(1 页/12 格)、`engine_samemerge`(1 页/4 格) 都是单页）
⇒ **该观察全部来自探针，不来自夹具**。要把它变成可回归的基准，需要落一个多页+合并的夹具。

## 19.5 ⭐ 回归基线文件

**`tests/fixtures/head_matrix.txt`**（166 行）—— 12 个夹具的**逐格 head 矩阵**，
由 `engine_*.expected.json` 直接导出，每格形如 `left/top`：

```text
## engine_head_top  grid=6x6  非空格=22
       c1         c2         c3   ...
  r1   (0,0)/(0,0) (0,0)/(0,0) ...
  r3   (0,0)/(1,1) ...        ← head-of-head 分支
  r6   (0,0)/(3,1) ...        ← ★ S2：最近 (3,1)
```

`(0,0)` = cell00；`None` = 产品侧为 null（键会被省略）。

实测（12 个夹具逐格统计）：**非空格 410 个，缺 head 键的 = 0**。
⇒ **「head 为 null 时省略键」这条分支在当前语料里从未被走到** ——
backend-2 的 `Option` 处理**没有夹具支撑**，属源码约定而非实测行为。
（`_$3`/`_$2` 的兜底 `return _$10`（cell00）几乎总是成功，这解释了为什么全是 `(0,0)` 或某个格。）

## 19.6 枚举纪律（我自己踩的坑，写下来）

⛔ **枚举计算态的格必须用 `ecs.getCell(r,c)`（按计算态坐标），不能用 `ecs.getSource(r,c)`
（那是按模板坐标查出处）**。用 `getSource` 枚举会**整段漏掉**插入到模板尺寸之外的格 ——
`engine_head_left` 的第 6..11 行、`engine_pager` 的第 5..43 行当时全被漏掉，
而那正是"没有 source 的格也能被 head 指向"的样本所在。
我的 §16/§17 head 基线因此重算过一次（**逐格反例数不变**，变的是坐标审计那两行）。

## ProbeDsCurrent（2026-10-13）

产品侧探针 ProbeDsCurrent 的**原始 stdout 全文**（真机私有状态实录，常驻证据）。
源码：`F:/project/report/re_work/tmp_probe/dscur/ProbeDsCurrent.java`（位于 rpx 仓库之外）。
运行：cwd=`tests/fixtures`，`java -Dfile.encoding=UTF-8 -cp "<reportlite>/lib/*;<dscur>/out" rpx.fixtures.ProbeDsCurrent`，exit 0。
sha256(probe_out.txt)=b29f1e24c1c99aafc51f31e026bfe7b6d37f79e177dccc8a00b20f026ceeb597（行尾为 Windows CRLF，原样保留）。

```text
=== 0. 本探针的口径（先读这里再读数字）===
  要查的机制：DataSet 的 current 指针在求值中如何推进？
  current      = DataSet.getCurrent()，静态类型 DsValue，实为 Row 或 Group
  currentRow   = DataSet.getCurrentRow()，Group 时表示组内第几行（1 基）
  isLeftTop    = DataSet.isLeftTop()，true 表示 current 是'行/左'型、false 为'组/上'型
  ExtExtendCell 私有 _$7 = 扩展实例数组；_$4 = 本格求值时暂存的 DsValue（反射直读）
  三个夹具跑完都直接读 **计算后** 的 current，不插桩、不改产品。

########## engine_sum ##########
  [1] 计算后 数据集 current 指针
    ds=ds2 dataRows=3 currentRow=1 isLeftTop=false current=Group(root=true rows=3 value=null)
         rootGroup=Group(root=true rows=3 value=null)
    ds=ds1 dataRows=3 currentRow=1 isLeftTop=false current=Group(root=true rows=3 value=null)
         rootGroup=Group(root=true rows=3 value=null)
  [2] 逐格：自身坐标 / src / extend / ExtExtendCell 私有态（只列非空格）
    (1,1) class=ExtNormalCell value="region" src=(1,1) extend=16
    (1,2) class=ExtNormalCell value="qty" src=(1,2) extend=16
    (1,3) class=ExtNormalCell value="twice" src=(1,3) extend=16
    (1,4) class=ExtNormalCell value="sumRange" src=(1,4) extend=16
    (1,5) class=ExtNormalCell value="ds1Sum" src=(1,5) extend=16
    (1,6) class=ExtNormalCell value="sumList1" src=(1,6) extend=16
    (1,7) class=ExtNormalCell value="ds2Sum" src=(1,7) extend=16
    (1,8) class=ExtNormalCell value="sumList2" src=(1,8) extend=16
    (1,9) class=ExtNormalCell value="ds2Count" src=(1,9) extend=16
    (2,1) class=ExtExtendCell value="North" src=(2,1) extend=18 | EEC _$7=len=3 _$4=Row(rowNo=1)
    (2,2) class=ExtExtendCell value=Integer(100) src=(2,2) extend=18 | EEC _$7=null _$4=Row(rowNo=1)
    (2,3) class=ExtNormalCell value=Integer(200) src=(2,3) extend=17
    (2,4) class=ExtNormalCell value=Integer(100) src=(2,4) extend=17
    (2,5) class=ExtNormalCell value=Integer(100) src=(2,5) extend=17
    (2,6) class=ExtNormalCell value=Integer(100) src=(2,6) extend=17
    (2,7) class=ExtNormalCell value=Integer(450) src=(2,7) extend=17
    (2,8) class=ExtNormalCell value=Integer(450) src=(2,8) extend=17
    (2,9) class=ExtNormalCell value=Integer(3) src=(2,9) extend=17
    (3,1) class=ExtExtendCell value="East" src=(2,1) extend=18 | EEC _$7=len=3 _$4=Row(rowNo=2)
    (3,2) class=ExtExtendCell value=Integer(200) src=(2,2) extend=18 | EEC _$7=null _$4=Row(rowNo=2)
    (3,3) class=ExtNormalCell value=Integer(400) src=(2,3) extend=17
    (3,4) class=ExtNormalCell value=Integer(100) src=(2,4) extend=17
    (3,5) class=ExtNormalCell value=Integer(200) src=(2,5) extend=17
    (3,6) class=ExtNormalCell value=Integer(200) src=(2,6) extend=17
    (3,7) class=ExtNormalCell value=Integer(450) src=(2,7) extend=17
    (3,8) class=ExtNormalCell value=Integer(450) src=(2,8) extend=17
    (3,9) class=ExtNormalCell value=Integer(3) src=(2,9) extend=17
    (4,1) class=ExtExtendCell value="South" src=(2,1) extend=18 | EEC _$7=len=3 _$4=Row(rowNo=3)
    (4,2) class=ExtExtendCell value=Integer(150) src=(2,2) extend=18 | EEC _$7=null _$4=Row(rowNo=3)
    (4,3) class=ExtNormalCell value=Integer(300) src=(2,3) extend=17
    (4,4) class=ExtNormalCell value=Integer(100) src=(2,4) extend=17
    (4,5) class=ExtNormalCell value=Integer(150) src=(2,5) extend=17
    (4,6) class=ExtNormalCell value=Integer(150) src=(2,6) extend=17
    (4,7) class=ExtNormalCell value=Integer(450) src=(2,7) extend=17
    (4,8) class=ExtNormalCell value=Integer(450) src=(2,8) extend=17
    (4,9) class=ExtNormalCell value=Integer(3) src=(2,9) extend=17
    非空格合计 = 36

########## engine_expr ##########
  [1] 计算后 数据集 current 指针
    ds=ds1 dataRows=3 currentRow=1 isLeftTop=false current=Group(root=true rows=3 value=null)
         rootGroup=Group(root=true rows=3 value=null)
  [2] 逐格：自身坐标 / src / extend / ExtExtendCell 私有态（只列非空格）
    (1,1) class=ExtNormalCell value="region" src=(1,1) extend=16
    (1,2) class=ExtNormalCell value="qty" src=(1,2) extend=16
    (1,3) class=ExtNormalCell value="ref" src=(1,3) extend=16
    (1,4) class=ExtNormalCell value="absCol" src=(1,4) extend=16
    (1,5) class=ExtNormalCell value="mixed" src=(1,5) extend=16
    (2,1) class=ExtExtendCell value="North" src=(2,1) extend=18 | EEC _$7=len=3 _$4=Row(rowNo=1)
    (2,2) class=ExtExtendCell value=Integer(100) src=(2,2) extend=18 | EEC _$7=null _$4=Row(rowNo=1)
    (2,3) class=ExtNormalCell value=Integer(100) src=(2,3) extend=17
    (2,4) class=ExtNormalCell value=Integer(300) src=(2,4) extend=17
    (2,5) class=ExtNormalCell value=Integer(200) src=(2,5) extend=17
    (3,1) class=ExtExtendCell value="East" src=(2,1) extend=18 | EEC _$7=len=3 _$4=Row(rowNo=2)
    (3,2) class=ExtExtendCell value=Integer(200) src=(2,2) extend=18 | EEC _$7=null _$4=Row(rowNo=2)
    (3,3) class=ExtNormalCell value=Integer(200) src=(2,3) extend=17
    (3,4) class=ExtNormalCell value=Integer(600) src=(2,4) extend=17
    (3,5) class=ExtNormalCell value=Integer(400) src=(2,5) extend=17
    (4,1) class=ExtExtendCell value="South" src=(2,1) extend=18 | EEC _$7=len=3 _$4=Row(rowNo=3)
    (4,2) class=ExtExtendCell value=Integer(150) src=(2,2) extend=18 | EEC _$7=null _$4=Row(rowNo=3)
    (4,3) class=ExtNormalCell value=Integer(150) src=(2,3) extend=17
    (4,4) class=ExtNormalCell value=Integer(450) src=(2,4) extend=17
    (4,5) class=ExtNormalCell value=Integer(300) src=(2,5) extend=17
    非空格合计 = 20

########## engine_ext ##########
  [1] 计算后 数据集 current 指针
    ds=ds1 dataRows=5 currentRow=1 isLeftTop=false current=Group(root=true rows=5 value=null)
         rootGroup=Group(root=true rows=5 value=null)
  [2] 逐格：自身坐标 / src / extend / ExtExtendCell 私有态（只列非空格）
    (1,1) class=ExtNormalCell value="none17" src=(1,1) extend=16
    (1,2) class=ExtNormalCell value="none17" src=(1,1) extend=16
    (1,3) class=ExtNormalCell value="none17" src=(1,1) extend=16
    (1,4) class=ExtNormalCell value="none17" src=(1,1) extend=16
    (1,5) class=ExtNormalCell value="none17" src=(1,1) extend=16
    (1,6) class=ExtNormalCell value="vertical18" src=(1,2) extend=16
    (1,7) class=ExtNormalCell value="horiz19" src=(1,3) extend=16
    (2,1) class=ExtNormalCell value=ArrayList([R1, R2, R3, R4, R5]) src=(2,1) extend=17
    (2,2) class=ExtNormalCell value=ArrayList([R1, R2, R3, R4, R5]) src=(2,1) extend=17
    (2,3) class=ExtNormalCell value=ArrayList([R1, R2, R3, R4, R5]) src=(2,1) extend=17
    (2,4) class=ExtNormalCell value=ArrayList([R1, R2, R3, R4, R5]) src=(2,1) extend=17
    (2,5) class=ExtNormalCell value=ArrayList([R1, R2, R3, R4, R5]) src=(2,1) extend=17
    (3,1) class=ExtExtendCell value="R1" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=1)
    (3,2) class=ExtExtendCell value="R1" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=1)
    (3,3) class=ExtExtendCell value="R1" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=1)
    (3,4) class=ExtExtendCell value="R1" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=1)
    (3,5) class=ExtExtendCell value="R1" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=1)
    (4,1) class=ExtExtendCell value="R2" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=2)
    (4,2) class=ExtExtendCell value="R2" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=2)
    (4,3) class=ExtExtendCell value="R2" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=2)
    (4,4) class=ExtExtendCell value="R2" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=2)
    (4,5) class=ExtExtendCell value="R2" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=2)
    (5,1) class=ExtExtendCell value="R3" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=3)
    (5,2) class=ExtExtendCell value="R3" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=3)
    (5,3) class=ExtExtendCell value="R3" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=3)
    (5,4) class=ExtExtendCell value="R3" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=3)
    (5,5) class=ExtExtendCell value="R3" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=3)
    (6,1) class=ExtExtendCell value="R4" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=4)
    (6,2) class=ExtExtendCell value="R4" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=4)
    (6,3) class=ExtExtendCell value="R4" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=4)
    (6,4) class=ExtExtendCell value="R4" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=4)
    (6,5) class=ExtExtendCell value="R4" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=4)
    (7,1) class=ExtExtendCell value="R5" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=5)
    (7,2) class=ExtExtendCell value="R5" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=5)
    (7,3) class=ExtExtendCell value="R5" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=5)
    (7,4) class=ExtExtendCell value="R5" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=5)
    (7,5) class=ExtExtendCell value="R5" src=(3,1) extend=18 | EEC _$7=len=5 _$4=Row(rowNo=5)
    (8,1) class=ExtExtendCell value="R1" src=(4,1) extend=19 | EEC _$7=len=5 _$4=Row(rowNo=1)
    (8,2) class=ExtExtendCell value="R2" src=(4,1) extend=19 | EEC _$7=len=5 _$4=Row(rowNo=2)
    (8,3) class=ExtExtendCell value="R3" src=(4,1) extend=19 | EEC _$7=len=5 _$4=Row(rowNo=3)
    (8,4) class=ExtExtendCell value="R4" src=(4,1) extend=19 | EEC _$7=len=5 _$4=Row(rowNo=4)
    (8,5) class=ExtExtendCell value="R5" src=(4,1) extend=19 | EEC _$7=len=5 _$4=Row(rowNo=5)
    非空格合计 = 42

=== 4. engine_sum 消融实验：删/换 (2,5) 后看 (2,6) 是否变脸 ===
--- 基线(原样)：(2,5)=ds1.sum(qty)，(2,6)=sum(ds1.select(qty))
    计算行数=8，逐行 C1 / C2 / C5 / C6：
      r1 C1="region"  C2="qty"  C5="ds1Sum"  C6="sumList1"
      r2 C1="North"  C2=Integer(100)  C5=Integer(100)  C6=Integer(100)
      r3 C1="East"  C2=Integer(200)  C5=Integer(200)  C6=Integer(200)
      r4 C1="South"  C2=Integer(150)  C5=Integer(150)  C6=Integer(150)
      r5 C1=null  C2=null  C5=null  C6=null
      r6 C1=null  C2=null  C5=null  C6=null
      r7 C1=null  C2=null  C5=null  C6=null
      r8 C1=null  C2=null  C5=null  C6=null
    ds=ds2 dataRows=3 currentRow=1 isLeftTop=false current=Group(root=true rows=3 value=null)
         rootGroup=Group(root=true rows=3 value=null)
    ds=ds1 dataRows=3 currentRow=1 isLeftTop=false current=Group(root=true rows=3 value=null)
         rootGroup=Group(root=true rows=3 value=null)
--- (2,5) 已移除 exp slot40（旧值=ds1.sum(qty)）
    计算行数=8，逐行 C1 / C2 / C5 / C6：
      r1 C1="region"  C2="qty"  C5="ds1Sum"  C6="sumList1"
      r2 C1="North"  C2=Integer(100)  C5=null  C6=Integer(100)
      r3 C1="East"  C2=Integer(200)  C5=null  C6=Integer(200)
      r4 C1="South"  C2=Integer(150)  C5=null  C6=Integer(150)
      r5 C1=null  C2=null  C5=null  C6=null
      r6 C1=null  C2=null  C5=null  C6=null
      r7 C1=null  C2=null  C5=null  C6=null
      r8 C1=null  C2=null  C5=null  C6=null
    ds=ds2 dataRows=3 currentRow=1 isLeftTop=false current=Group(root=true rows=3 value=null)
         rootGroup=Group(root=true rows=3 value=null)
    ds=ds1 dataRows=3 currentRow=1 isLeftTop=false current=Group(root=true rows=3 value=null)
         rootGroup=Group(root=true rows=3 value=null)
--- 变体B(交换 2,5<->2,6)：(2,5)<->(2,6) 表达式已交换
    计算行数=8，逐行 C1 / C2 / C5 / C6：
      r1 C1="region"  C2="qty"  C5="ds1Sum"  C6="sumList1"
      r2 C1="North"  C2=Integer(100)  C5=Integer(100)  C6=Integer(100)
      r3 C1="East"  C2=Integer(200)  C5=Integer(200)  C6=Integer(200)
      r4 C1="South"  C2=Integer(150)  C5=Integer(150)  C6=Integer(150)
      r5 C1=null  C2=null  C5=null  C6=null
      r6 C1=null  C2=null  C5=null  C6=null
      r7 C1=null  C2=null  C5=null  C6=null
      r8 C1=null  C2=null  C5=null  C6=null
    ds=ds2 dataRows=3 currentRow=1 isLeftTop=false current=Group(root=true rows=3 value=null)
         rootGroup=Group(root=true rows=3 value=null)
    ds=ds1 dataRows=3 currentRow=1 isLeftTop=false current=Group(root=true rows=3 value=null)
         rootGroup=Group(root=true rows=3 value=null)
--- (2,1) 已移除 exp slot40（旧值=ds1.select(region)）
--- (2,2) 已移除 exp slot40（旧值=ds1.select(qty)）
    计算行数=6，逐行 C1 / C2 / C5 / C6：
      r1 C1="region"  C2="qty"  C5="ds1Sum"  C6="sumList1"
      r2 C1=null  C2=null  C5=Integer(450)  C6=Integer(450)
      r3 C1=null  C2=null  C5=null  C6=null
      r4 C1=null  C2=null  C5=null  C6=null
      r5 C1=null  C2=null  C5=null  C6=null
      r6 C1=null  C2=null  C5=null  C6=null
    ds=ds2 dataRows=3 currentRow=1 isLeftTop=false current=Group(root=true rows=3 value=null)
         rootGroup=Group(root=true rows=3 value=null)
    ds=ds1 dataRows=3 currentRow=1 isLeftTop=false current=Group(root=true rows=3 value=null)
         rootGroup=Group(root=true rows=3 value=null)
```

---

# 20. M2-12 Text-Text 字典序：Rust 数据表 + 引擎夹具（`collation-data`）

> 边界：**不修改** `crates/rpx-engine/src/`（Rust collation 模块属他人）。本轮只产数据、golden、
> 第 14 个夹具，以及 `crates/rpx-engine/tests/` 里被夹具数影响的**计数**。
> 产物：`GenCollationData.java`（+ 拆出的 `CollationData.java` / `CollationGoldens.java` / `CollationCcc.java`）、
> `crates/rpx-engine/data/collation.bin`、三个 golden、`engine_collate` 夹具。

## 20.1 产物与 SHA-256

```
bin.path = crates/rpx-engine/data/collation.bin
bytes    = 81444
sha256   = 9b2d81c25b7d6bfcc512bed5f51e2830fdd12855b07ee4a6d7e08118cd11b3cb
```

13 段（固定序，全小端，段表 4 字节对齐）：

```
sec[0]  cjk_primary     2 uc 20902     sec[7]  ccc_val         1 uc 653
sec[1]  singles         4 uc 933       sec[8]  contract_starter4 uc 25
sec[2]  nfd_cp          4 uc 2053      sec[9]  contract_count  1 uc 25
sec[3]  nfd_off         4 uc 2053      sec[10] contract_key    4 uc 54
sec[4]  nfd_len         1 uc 2053      sec[11] contract_key_ce 2 uc 54
sec[5]  nfd_pool        4 uc 3390      sec[12] ignorable       1 uc 25
sec[6]  ccc_cp          4 uc 653
```

## 20.2 复现命令（Git Bash，classpath 段必须整体双引号）

```bash
cd F:/project/report/rpx/tests/fixtures
CP="F:/project/report/reportlite/lib/*;F:/project/report/rpx/tests/fixtures/out"
D:/Tools/jdk-21.0.2/bin/javac.exe --release 8 -Xlint:none -cp "$CP" -d out *.java
F:/project/report/common/jre/bin/java.exe -cp "$CP" rpx.fixtures.GenCollationData \
  tests/fixtures/_collation_dump.tsv tests/fixtures crates/rpx-engine/data/collation.bin
```

原始输出（节选，逐字）：

```
GEN OK sections[0=20902 1=933 2=2053 3=2053 4=2053 5=3390 6=653 7=653 8=25 9=25 10=54 11=54 12=25]
GEN OK cross-check: SUMMARY bmp=0/63488 pairs=0/595984 fuzzCE=0/30000 fuzzCMP=0/8000 singles=21837 contracts=25
GEN OK supplementary: checked=384 mismatched=0
GEN OK CE dump: rows=63872 ce=151485 supplementary=384 (all secondary/tertiary == 0)
GEN OK ccc: sun.text.normalizer.NormalizerImpl.getCombiningClass(int) via reflection
GEN OK nfd golden rows=2186 | contract keys=54 ignorable=12
GEN OK bin bytes=81444 sha256=9b2d81c25b7d6bfcc512bed5f51e2830fdd12855b07ee4a6d7e08118cd11b3cb
```

golden（`tests/fixtures/`）：`_collation_nfd_golden.tsv` 2186 行（输入列已确认无重复）、
`_collation_contract_golden.tsv` 66 行 = 54 键行 + 12 `IGNORABLE` 行、`_collation_summary.txt`。

## 20.3 ccc 的取得方式

**反射优先**：`sun.text.normalizer.NormalizerImpl.getCombiningClass(int)`（候选还包括 JDK 9+ 的
`getCanonicalClass`、`sun.text.Normalizer.*`），逐码点取值，过滤 v==0。反射**全部**失败时退到黑盒推导：
对不自我分解的 mark，按 `NFD(a+b)` 是否交换 a/b 定序，得到同序重编码（`rank = 1 + #{q: ccc(p)>ccc(q)}`）。
本次真机走的是反射路径（见 20.2 `ccc:` 行）。

## 20.4 独立复核（不经过 Rust 加载器）

`python tests/fixtures/tools_decode_collation_bin.py` → **`RESULT PASS`**：

```
cjk.count=20902 singles.count=933 nfd.count=2053 nfd_pool.count=3390 ccc.count=653
starters=25 sum(contract_count)=54 keys=54 sum(ignorable)=9
singles_sorted/nfd_cp_sorted/ccc_cp_sorted=True  keys_in_range=True  nfd off/len in pool=True
structure bad=0
SUMMARY bmp=0/63488 supp=0/384 sort=0/3200 pair=0/58 nfd=0/2186
```

`singles` 比 dump 少两个（`0x2F800`/`0x2FA00`）：二者是**补充平面**码点（cp>0xFFFF），不进 BMP singles 段。

## 20.5 引擎夹具 `engine_collate`（8×6，全部无扩展）

```bash
F:/project/report/common/jre/bin/java.exe -cp "$CP" rpx.fixtures.GenEngineFixtures .
F:/project/report/common/jre/bin/java.exe -cp "$CP" rpx.fixtures.GenEngineFixtures --verify .
# → PASS engine_collate: 8560 bytes round-trip to expected.json
# → VERIFY OK: 14 engine fixtures generated   (exit 0)
```

8×6 = 48 格**全非空且全部带 source**（`SHAPE` 行 = `("engine_collate", 8, 6, 8, 6, 48, 48, 0)`）。
覆盖：文本 `>`/`<`/`>=`/`<=`（张/阿、b/a、一/丁 —— 后者码点序与产品序**相反**）、
全可忽略 U+0301 的截断（6 条）、收缩键 + ccc 重排（6 条）、`["…"]` 值列表的文本 `max`/`min`（6 条）。

夹具**不带数据集**：产品 `CalcProbe.writeCsv` 用 **GBK** 写 CSV，而 Rust 加载器要求 ASCII/UTF-8，
故 R8 的 `max`/`min` 改用**值列表** `["张","阿"]`（产品的真聚合形态），避免非 ASCII CSV。

## 20.6 与 brief 的 4 处口径差异（以真机实测为准）

1. **收缩键 = 54，不是 79**。79 是 `_collation_tables/contract.tsv` 的总行数 = 25 个 starter 头行 + 54 键行。
2. **bin 段 12 的 `ignorable` = 9**（25 个 starter 中 9 个 value=-1）；brief 的「12」指
   **golden 里的 `IGNORABLE` 行数** = 9 个 starter + 3 个经 NFD 归零的码点（U+0340/U+0341/U+0344）。
3. **`nfd_len` 最小值 = 1，不是恒 ≥2**：存在 1017 个「单码点分解」（如 U+0340→U+0300）；
   `sec[2]=2053`（1493 BMP + 560 补充平面）。
4. 段 8 高 16 位读作**段 10 的偏移**（`[10][off .. off+count)`），这是使 (starter,count) 自洽的唯一读法。

## 20.7 ⛔ 当前集成状态：未绿（根因在引擎侧，不在本轮）

`cargo test --workspace --no-fail-fast`：**20 个 target 全跑，唯一红灯 1 条**：

```
engine_grid_compare::engine_output_matches_every_fixture_cell_by_cell ... FAILED
  panicked: 未建模的引擎错误，无法映射 class：
            unsupported expression: Text("张") <=> Text("阿")
```

同一轮里被夹具数影响的**计数类**测试全绿（证明账本已对齐）：

```
test every_fixture_formula_is_state_four ... ok            (TOTAL_FORMULAS == 78)
test all_fixtures_load_and_self_identify ... ok            (all_names().len() == 14)
test template_dims_match_product_output ... ok
test calculated_dims_and_source_counts_match_product_output ... ok   (SHAPE 48/48/0)
```

**根因**：`crates/rpx-engine/src/` 内**没有任何** collation 代码
（`grep -ri "collation\|collate\|314C4352" crates/rpx-engine/src/` 为空；无 `*collat*` 文件；
`collation.bin` 未被任何 src 引用）。Text-Text 比较的 **Rust 实现尚未落地**，
故新夹具 `engine_collate` 必然红。该实现**不在本任务边界内**，需由 Rust 侧落地后转绿。
`engine_grid_compare` 明确写了「不再设夹具级豁免」，所以不应对该夹具开白名单 —— 红是**正确信号**。

`cargo clippy --all-targets -- -D warnings` 实测 **exit 0**（零告警）。

## 20.8 口径钉死（下轮 spec 更新的直接依据）

### (a) `engine_collate` 的覆盖范围 / 未覆盖范围 —— 中文**只走公式字面量路径**

本夹具的中文（张/阿/一/丁）**只**出现在**公式字面量**里（`"张">"阿"`、`max(["张","阿"])`），
**不经过数据集路径**。原因是**既有**缺口（非本轮引入）：

- 产品 `CalcProbe.java:260-262` 的 `writeCsv` 用 **GBK** 写夹具 CSV（产品 `FileDataSetFactory` 按 GBK 读）；
- `crates/rpx-engine/src/dataset.rs:16-17` 明确声明「非 ASCII 的 GBK 内容本 crate 不覆盖」，
  Rust 加载器对夹具只接受 ASCII/UTF-8。

⇒ **不得**据此认为「数据集里带中文」已被验证 —— 本夹具**未覆盖**该形态。
实证：全部引擎夹具 CSV **非 ASCII 字节数 = 0**（`engine_collate.csv` / `engine_types.csv` /
`engine_disp.csv` 均 44 字节）。故 R8 的 `max`/`min` 用**值列表** `["张","阿"]`（产品的真聚合形态），
而非 `ds1.select(region)` —— 这是**正确收敛，不是绕路**。

### (b) 三处"看起来不一致"的计数 —— 语义不同，不可互推

| 数字 | 出处 | 含义 |
|------|------|------|
| `sec[10] = sec[11] = 54` | `collation.bin` 段 10/11 | 收缩表**前向键**总数（25 个 starter 的键之和） |
| `sec[12]` 中 `1` 的个数 = **9** | `collation.bin` 段 12（25 个 u8） | 25 个 starter 中 value=-1（**起始字符级**可忽略）的个数 |
| `IGNORABLE` 行 = **12** | `_collation_contract_golden.tsv` | CE 流为空的 BMP 码点总数 = 9 个起始字符 **+** U+0340/U+0341/U+0344 |

后 3 个（U+0340/U+0341/U+0344）**不是**收缩表起始字符，它们**经 NFD** 才归零
（→ U+0300/U+0301/U+0308）。`_collation_summary.txt` 的 `contract.golden` 行现已写成
`keys=54 ignorable_rows=12(9 starters + 3 via NFD)`，避免 12 与 9 混淆。
brief 里的「12」指**第三行**（golden 的 IGNORABLE 行数），**不是**段 12 里 1 的个数（9）。

### (c) 三处台账改动的文件与行号（夹具数 12→14 的连带修正）

| 文件 | 行 | 改动 |
|------|----|------|
| `crates/rpx-engine/tests/common/loader.rs` | 14–15 | `all_names()` 13→**14** 个名 + 追加 `engine_collate` |
| `crates/rpx-engine/tests/fixture_loader.rs` | 18 | `GRID_FIXTURES: [&str; 12]` → `[&str; 13]` + 追加 `engine_collate` |
| 同上 | 40 / 53 | `SHAPE: [ShapeRow; 12]` → `[13]`；新增 `("engine_collate", 8, 6, 8, 6, 48, 48, 0)` |
| 同上 | 58 | `assert_eq!(all_names().len(), 13)` → `14` |
| `crates/rpx-engine/tests/expr_state_tripwire.rs` | 47 | `TOTAL_FORMULAS` 44 → **78**（+34 条 engine_collate 公式，全 `get_state()==4`） |

**注释类同步**（只把「12 个夹具」改「14 个」，无逻辑）：
`engine_grid_compare.rs:5,87`、`m09_compare.rs:85`、`schema_sentinel.rs:17`。
凡**未被夹具数触及**的数字**一律未改** —— 例如 `engine_rowheight_pager.rs` 的「克隆 12 次」
是**行克隆数**不是夹具数，保持 12（首版盲目 12→14 曾误伤，已回退）。

---

## 21. M2-12 `(A:B)` List 逐格分支（2026-10-14，第 15 个引擎夹具 `engine_listrange`）

> 承接 §14.5 的结论「普通 `B2:B4` 恒走 else 单格分支」。本节问的是：
> **什么形态能让 `cellSet` 求值成 `List<ExtCell>`、触达逐格循环，产品在那里算什么？**

### 21.1 定性（源码链，`reportlite/03-model/.../expression/`）

| 写法 | 上游链 | cellSet 求值成 |
|------|--------|------|
| `B2:B4` | `ParamParser.java:66-79`：参数分隔符按 `;`→`,`→`:` 三级试；`:` 是**分隔符** ⇒ 拆 sub0/sub1 | `CSVariable` 单格 ⇒ else 分支 |
| `[B2:B4]` | `ValueList.java:161-162` 的 `:` 形态 → `_$1:68-120` 逐格收集（`Link.java:85` 同款 `testValue()`） | `List<ExtCell>` ⇒ 逐格 |
| `(B2:B4)` | `ParamParser._$1:110-117` 扫描时**跳过 `()` 内** ⇒ 不被拆 ⇒ `Expression.createNode` 走表达式分支 ⇒ `Expression.java:412-413` 的 `:` → `operator.Link`；`Link._$1:78-89` 收集 | 同左 |

⇒ **分水岭在参数解析器，不在聚合语义**。要让逐格循环可观测，value 操作数必须是
`@`（`CSCurrent.java:23-28` 返回 `getCurrent().getValue(false)`）；若 value 是常量，
每轮 `setCurrent` 后求值仍得同一值。

### 21.2 探针 14 形态（`ProbeListRange.java`）

```bash
CP="F:/project/report/reportlite/lib/*;F:/project/report/rpx/tests/fixtures/out"
F:/project/report/common/jre/bin/java.exe -cp "$CP" rpx.fixtures.ProbeListRange > _probe_listrange_raw.txt
python _mk_listrange_summary.py   # → _probe_listrange_summary.txt（GBK 解码后的可读摘要）
```

| 公式 | (2,3) 值 | 对照（else 形态） |
|------|---------|------------------|
| `sum(@:[B2:B4])` / `sum(@:(B2:B4))` | `Integer(420)` | `sum(B2:B4)` = `100` |
| `max(@:[B2:B4])` / `max(@:(B2:B4))` | `Integer(200)` | `max(B2:B4)` = `100` |
| `min(@:[B2:B4])` / `min(@:(B2:B4))` | `Integer(100)` | `min(B2:B4)` = `100` |
| `avg([B2:B4]:@)` / `avg((B2:B4):@)` | `Double(140.0)` | `avg(B2:B4)` = `Double(120.0)` |

**2 项反序对照报错**（List 落到 value 位置 ⇒ cellSet 退化为单格、`@` 求值触发自引用）：
`sum([B2:B4]:@)`、`avg(@:[B2:B4])` →
`ReportError: 单元格：C2中有错误，C2-C2-C2存在循环引用`。

### 21.3 第 15 个产品夹具

```bash
F:/project/report/common/jre/bin/java.exe -cp "$CP" rpx.fixtures.GenEngineFixtures --verify .
# → PASS engine_listrange: 5518 bytes round-trip to expected.json
# → VERIFY OK: 15 engine fixtures generated   (exit 0)
```

形状 `("engine_listrange", 5, 6, 5, 6, 19, 19, 0)`：模板 5×6、计算态 5×6、19 格**全带 source**
（无扩展、无合并 ⇒ 双矩阵重合）。12 条公式 = 三形态 × 四函数，C 列 `[A:B]`、D 列 `(A:B)`、
E 列 else 对照。CSV 复用 `EngineModelsRange.rows()`（B 列 100/200/120，与 `engine_avgrange` 同源）。

### 21.4 Rust 落地

| 文件 | 内容 |
|------|------|
| `crates/rpx-engine/src/expr/cellset.rs`（新，125 行） | `CellSet::Single/Many` + `eval_cellset()` + `rect_cells()`（`Link._$1:78-89` ≡ `ValueList._$1:112-119` 的遍历：左端格**自身**坐标取 top/left、右端 `+rowMerge-1` 取 bottom/right、命中判据 `cell_pos == (i,j)` 排除别名槽、每格 `testValue()` 门） |
| `crates/rpx-engine/src/expr/eval.rs` | `sum_link` / `region_aggregate` → 统一的 `region_loop` + `region_accumulate`；avg 的操作数反转保留在 `region_aggregate`（换 index，不复制循环） |
| `crates/rpx-engine/src/calc/ctx/eval_ctx.rs` | 新增 5 个方法：`cell_at`（`by_pos`，计算矩阵）/ `cell_pos` / `row_merge` / `col_merge` / `test_value_gate`（转发 `cell_eval::test_value`，与 `cell_value_checked` 同出口） |
| 测试台账 | `loader.rs` 15 名 / `fixture_loader.rs` GRID 14 + SHAPE 14 行 / `expr_state_tripwire.rs` `TOTAL_FORMULAS` 78→**90** |

**⚠ 两处忠实性修正（同一轮内发现并修掉，均由合成钉子钉住）**：

1. **累积槽位的 null 语义**：`Max/Min` 的 List 分支里产品写
   `result = Variant2.getSingleValue(obj)`（`Max.java:163` / `Min.java:163`），取到 `null` 时
   `result` 变 **Java null** ⇒ 下一轮 `result != null` 为假、走「无条件重新初始化」分支。
   首版 Rust 写成 `Some(Value::Null)`（下轮会走比较分支）——**语义分叉**，已改为 `Option`
   的 `None`（`if take_b { b } else { Some(a) }`，`b` 为 `None` 即"槽位为空"）。
2. **`aggregate_cmp` 缺 null 分支**（由 ① 的合成钉子暴露）：产品 `compare2:419-424` 的
   null 判定在**最前面**（`(null,null)=0`、`(null,x)=-1`、`(x,null)=1`），而 `num.rs::aggregate_cmp`
   原先遇 null 报 `UnsupportedExpr` —— 空 List 元素路径必踩。已补三支。

**合成钉子**：`tests_aggregate.rs` ⑫ `region_min_reinitializes_after_a_null_slot_instead_of_comparing`
（序列 `[100, 空List, 50]`，产品 min = **50**；旧写法会得 null）。为此给 `Fake` 补了区域几何
（`FCell.row/col` + `Fake::set_pos` + `impl EvalCtx` 的 `cell_at`/`cell_pos`）。
**变异验证（承重证明）**：把 `eval.rs` 改回旧写法 `Some(if take_b { b.unwrap_or(Value::Null) } else { a })`
⇒ **仅该条单独变红**（245 passed / 1 failed），改回后 246 全绿、md5 复原。

可达条件（元素是「空 List 或首元素为 null 的 List」）**产品夹具无此形态** ⇒ 属已声明边界
（`docs/m2-engine-design.md` §14.6 ⑤.4/⑦）。
**同族未改**：`num_cmp`（`<`/`>`）的 null 分支仍缺 —— 无夹具、跨 AC 线，已在 §14.6 ⑦ 声明。

---

# 22. pager 支持性守卫落地 + `pager.rs` 模块拆分（2026-10-14 晚，team-lead）

> 背景：总监批准「M3 准入务实路」；M2 三条余留边界的处置定案见 `docs/spec-m2.md` §6.2 末条，
> M3 范围见 `docs/spec-m3.md`。本节只记**代码侧**的守卫与拆分。

## 22.1 为什么必须守（本轮核出的真问题）

`rpx-core` 早已解析 `PrintSetup.pager_style`（槽 23，`print_setup.rs:35/123`，默认 2），
但**引擎完全不读它** —— `pager::paginate` 连 style 形参都没有（分页走**探针口径**三常量
`PAPER_W_PT` / `PAPER_H_PT` / `COLUMNS`）。⇒ `pagerStyle 0/1`、多栏
（`columnMode` / `isColumnRepeatedGroupHeader`）此前是**静默按 2**：模板命中时产出错误分页
而调用方无从知晓。这正是「诚实失败优于静默错」要消灭的形态。

## 22.2 守卫（2026-10-14 落地）

| 件 | 内容 |
|----|------|
| `EngineError::UnsupportedPaging { detail }`（`error.rs`） | **已声明边界**（非产品行为；产品支持这些配置）；Display + 单测各一 |
| `pager::ensure_supported(&PrintSetup)`（`pager/guard.rs`） | 三字段非默认即报错：`pager_style != 2` / `column_mode != 0` / `column_repeated_group_header` |
| 调用点 | `calc::calculate()` **入口**（输入校验失败不跑计算；`calc/run.rs:59-63`） |
| 单测 | `pager/tests.rs`：默认放行 / `pagerStyle ∈ {0,1,3}` 拒绝 / 多栏两开关各自拒绝 |
| 集成 | `tests/pager_guard.rs`：① **15 夹具模板全在支持范围内**（不误伤）② 三字段逐一拒绝且**先于计算**（空数据集也先报 `UnsupportedPaging`） |
| 回归 | `engine_grid_compare` 全量对拍 7 passed（`engine_output_matches_every_fixture_cell_by_cell` ok） |

## 22.3 顺带的模块拆分（G4：`pager.rs` 一度 535 行 > 500）

| 文件 | 行数 | 内容 |
|------|-----|------|
| `pager/guard.rs`（新） | 39 | `ensure_supported`（从 pager.rs 拆出） |
| `pager/arith.rs`（新） | 47 | 三条算术：`paper_size_pt` / `unit_to_pt` / `breaks` + `fits`（含 `TOLERANCE`） |
| `pager.rs` | 465 | `pub use arith::{paper_size_pt, unit_to_pt}` / `pub use guard::ensure_supported` 保持原外部路径不变 |

`fits` / `breaks` 用 `pub(super)`：`pager.rs` 与 `pager/tests.rs` 均可见，外部不可见。

## 23. M3/M4 并行推进：快照映射 v1 落地 + M4 前端脚手架（2026-10-14）

### 23.1 决策：M3 与 M4 并行（总监令）

> 「M3 M4 一起推进，这样可以看到报表显示」

- **排期变更**：M3-08（快照协议）从 P1 后置**提为关键路径**——它是 M4 的唯一输入接口
  （M4 立档 `docs/spec-m4.md`；M3 spec 同步改 v0.2）。
- **两条数据管道**：开发期 `rpx-server snapshot`（本地文件）→ 生产 `POST /render`
  （同一条 `snapshot::build` 代码）；前端开发不被 axum/libSQL 阻塞。

### 23.2 快照契约 v1（字段名以 Univer 官方类型定义为准）

2026-10-14 逐条核对 docs.univer.ai 六页：`IWorkbookData` / `IWorksheetData` /
`ICellData` / `IStyleData` / `HorizontalAlign` / `VerticalAlign`。要点：

| 项 | 结论（易错点） |
|----|---------------|
| `mergeData` / `cellData` / `rowData` / `columnData` | 官方字段名如此（**不是** `merges`/`colData`——第三方教程有误） |
| `ht` / `vt` | **数字枚举**：h `-48→1(L)` `-47→2(C)` `-46→3(R)` `-45→4(JUST)` `-44→6(DIST)`；v `-32→1` `-31→2` `-30→3` |
| `t`（CellValueType） | 1/2/3/4；文本一律 4=FORCE_STRING（防 "00123" 被推断成数字） |
| 坐标 | Univer 0 起；引擎 `CalcCell`/`MergedCell` 1 起 ⇒ `-1` 转换 |

全套映射表与声明近似（mm→px 96dpi、Decimal 丢尾零、NaN→null）见 `docs/spec-m4.md` §4/§7。

### 23.3 `rpx-server` 落地（M3-08 v1）

- 新 crate（`crates/rpx-server`）：`snapshot.rs` + `snapshot/{value,style,geom,tests}.rs` + `main.rs`（dev bin）。
- 命令行：`rpx-server snapshot <file.rpx> [-o out.json]`（与 `spec-m3.md` §3.2 主链路 ①-⑩ 同构，
  ①从文件而非 libSQL）。
- **首个第三方依赖**：`serde_json`——G5 受审层登记（`dep_allowlist.txt` + `code-review.md` §3.2 表首行）。
- 位置依据：快照的最终归属是 server（M3-05 只加 axum 路由，不动映射代码）。

### 23.4 `mergeData` 锚点恢复：初版算法被实证证伪（重要教训）

- 初版思路「过滤被其他跨度覆盖的槽」→ 端到端测试实测 **4 条（应 6 条）**：
  延续槽 `(3,1,2,1)` 携带整块跨度 2，把下一块的真锚点 `(4,1)`/`(6,1)` "覆盖"掉了
  ⇒ **单槽无法推断锚点**（相邻块接续时本质歧义）。
- 现算法（`geom.rs`）：**矩形槽集不变式 → 贪心消费 + 逐格校验**（游标槽必为新块最小槽；
  网格缺槽/重叠 ⇒ `Err`，不静默）。
- 三个测试钉死：`engine_merge` **12 槽 → 6 锚点**（逐块坐标）/
  `engine_samemerge` **4 槽 → 2 锚点**（该夹具跨度只在 `merged` 段可见——误用 `source` 段必红）/
  负路径（缺槽报错）。
- 教训入库：**映射算法必须端到端跑真实夹具**——7 个单测全绿也没拦住这个 bug。

### 23.5 M4 前端脚手架（web/）

- 栈：Vite 8.3.4 + React 19 + TS；Univer **1.0.4**（`@univerjs/presets` + `@univerjs/preset-sheets-core`；
  官方包结构核对：`lib/index.css` / `locales/zh-CN`（`export default`）/
  `createUniver → { univer, univerAPI: FUniver }`）。
- `src/report.ts`（装载）+ `src/App.tsx`（UI）+ `vite.config.ts`（端口 5288；**不用 StrictMode**——
  effect 双执行会二次 `createUniver` 到同一 canvas 容器）。
- 校验：`tsc --noEmit` **0 错误**；`vite build` **exit 0**（2.57s；主包含 Univer 6.6MB 属正常）。
- 待验证：真实浏览器截图（playwright 需 `--browser=chromium` 且下载 chrome-for-testing-1247；
  本机 `npx` 不可用、系统无 Chrome）。

### 23.6 快照实测（`engine_merge`，`rpx-server snapshot`）

- `mergeData` 6 条（0 起坐标）：`(1,0)-(2,0)` / `(1,2)-(1,3)` / `(3,0)-(4,0)` / `(3,2)-(3,3)` / `(5,0)-(6,0)` / `(5,2)-(5,3)`。
- `rowData` 行高 30.24px（8mm）、`columnData` 列宽 94.49px（25mm）。
- `cellData`：`"region"` → `t:4`；`100` → `t:2`；样式含 `ff:"宋体"` `fs:12` `ht:1` `vt:2` `cl/bg` 色。
- 产物：`tests/fixtures/out/engine_merge.snapshot.json`（2.4KB）。

## §24 M3-01 参数通道：建树期内联 + `engine_param` 夹具闭环（2026-10-14 晚）

> 总监令「M3 M4 一起推进」后 M3 的第一号任务。验收：`engine_param` 进 16 夹具
> 主对拍（`engine_grid_compare`）逐格一致。参数解析期的四条真机结论来自
> `ProbeParamFull.java`（本轮新探针，输出 `probe_param_full_out.utf8.txt`）。

### 24.1 参数全景探针：四个缺口的真机答案

`ProbeParamList`（§15）只钉了「值是不是 List 决定 isExtend」+ 三个 calc 案例：

| 缺口 | 真机答案 | 对实现的含义 |
|---|---|---|
| 空 List `List[]` | 矩阵 **3×2 不变**；(2,1) 锚点 `extend=18 subCells=1` 但值 `null` | §15「成为扩展格却没有子格」的说法要修正：subCells 仍是 1 |
| `@@q` 未登记 | **不报错**：`Constant(null)`；计算态 `extend=17`（非扩展格无条件置 17） | `@@` 路径不查 `isParamName` |
| `@p` 未登记 | `ReportError`：`单元格A2中有错误: 表达式：@p中: 未知的参数：@p` | 未登记 ⇒ **显式失败**，不是"保守 false" |
| Types 转换 | 见 §24.5 | Servlet 层的字符串→类型，`/render` 的参数解析对应它 |

### 24.2 M3-01 实现定案：建树期内联（`parse_with`）

三条建树路径（产品 `Expression.java`）与 Rust 实现一一对应：

| 写法 | 产品行为 | 本 crate |
|---|---|---|
| `@p` 已登记 | `Constant(getParamValue(p))`（`:187-188`） | 内联 `Expr::Const(值)` |
| `@p` 未登记 | `ReportError`（`:189-191`） | `EngineError::UnknownParam` |
| `@@p` 无论登记 | `Constant(getParamValue(p))`（`:183-186`，**不查** `isParamName`） | 内联；未登记 ⇒ `Const(Null)` |
| 裸名已登记 | `ParamNode(id, 值)`（`:233`；`ParamNode extends Constant`） | 内联 `Const(值)`，**排在函数判定之后**（`:210` 早于 `:232`） |

⛔ **被证伪的早期设计**：`Expr::Param { value: Option<Value> }` + 求值期
`ctx.param()` 兜底。`is_extend` 是**准备期**判定（`ExtNormalCell.checkExtend`），
求值期才拿到值**救不了扩展**（扩展已经错在同一条链上）；留着兜底只会掩盖
"建树期没绑定"的 bug。M3-01 落地时删除该变体与该 trait 方法（`params.rs`
模块文档有完整论证）。

已知限制（provisional 策略的推论）：参数名形如 `[A-Za-z]+\d+`（`p1`、`t1`）时
被 `is_cell_id` 形状判定截胡成 `CellRef`（与 `qty1` 列名误判同类同源），等
M2-06 的矩阵重解析一并修正；坏后果只落在"参数名形如格号且值为 List"。

### 24.3 `engine_param` 夹具（第 16 个，主对拍闭环）

模板 3×4（`EngineModelsM3.java`），注入 `p=ArrayList[1,2,3]`、`s=Integer(7)`：

| 格 | 表达式 | 计算态 |
|---|---|---|
| A2 | `@p` | 18 扩展 3 子格，矩阵 3→5 行；sub_values `(2,1)=1 (3,1)=2 (4,1)=3` |
| C2 | `sum(@p)` | `6`，`extend=17`（聚合**不透传** is_extend） |
| D2 | `max(@p)` | `3`，同上 |
| B3 | `@s` | `7` —— 模板第 3 行被 A2 的扩展挤到**计算态第 5 行** |
| B2 | 字面量旁注 | 延续槽在铺开行复制模板值（(3,2)(4,2) 同值） |

对拍路径：`engine_text()` 读夹具的 `params` 段 → `calculate_with(rd, ds, params)`
→ `emit::render_document` → 逐格比（值/类型/source/sub_values/heads/尺寸）。
**`engine_grid_compare` 7/7 全绿**（含本夹具）。⚠ 注入值必须是 `java.util.List`
（产品判据 `instanceof List`，`Constant.java:22-24`）—— `int[]` 不扩展。

### 24.4 两扩展格同行：产品实证（**显式带账，不进夹具**）

第一版模板 E2=`@@p` 与 A2=`@p` 同行（两个 18 扩展格），产品产出 **11×5**：

- `@@p`（E2）：sub_values **连续** `(2,5)(3,5)(4,5)`；
- `@p`（A2）：sub_values 锚点落在**行带起点** `(2,1)(5,1)(8,1)`，间隔 = 行带高 3；
  每个子格值复制到整条行带（A 列 2-4 行=1、5-7 行=2、8-10 行=3）；
- 即行带高由先处理的扩展格决定，后处理扩展格的子格锚点被对齐到行带边界。

本 crate 扩展算法（M2-15）不建模该交互 ⇒ 夹具去掉 E2（单扩展格形态，Rust 可
对拍），本条作为**显式带账**：产品行为已实证（数据如上），Rust 未覆盖。
`@@p` 的解析期语义由 `expr/tests/deviation.rs` 与探针 B 组分别钉死。

### 24.5 `Types.getProperData(dataType, 输入)` 转换矩阵（真机）

| dataType | 输入 | 结果 |
|---|---|---|
| 1 | `10` / ` 10 ` | `Integer(10)` |
| 1 | `abc` | `NumberFormatException` |
| 11 | `abc` / ` pad ` | `"abc"` / `" pad "`（**不 trim**） |
| 11 / 1 | 空串 | `null` |
| 12 | `true` / `TRUE` | `Boolean(true)`；`yes` ⇒ `null` |
| 51 | `1,2,3` | `ArrayList([1, 2, 3])` |
| 61 | `a,b,c` | `ArrayList([a, b, c])`；`a, ,c` ⇒ `[a, null, c]` |

注：这是 **Servlet 层**的字符串→类型转换（`ReportUtils2.java:236-238`，
`dataType==61` 逗号分隔），与 `/render` 的参数解析对应；引擎侧
`setParamValue` 收到的是已定型对象（loader 的 JSON→`Value` 只需 Int/Double
两档，`tests/common/loader.rs` 的 `parse_param_value`）。

## §25 M3-02 GBK 数据集解码：JDK 映射冻结 + engine_gbk 夹具（2026-10-14 深夜）

> 总监令「M3-02 GBK/GB18030 解码」。验收：中文 CSV（GBK）端到端进计算态，
> 列名/值逐格对拍。产品口径与实现证据全部来自真机（探针/夹具），非标准表。

### 25.1 产品口径：默认 GBK，且是调用方传进去的

| 环节 | 源码位置 | 行为 |
|---|---|---|
| 配置默认值 | `FileDataSetConfig.java:25` | `private String encoding = "GBK"` —— 模板不写就是 GBK |
| factory 取用 | `FileDataSetFactory.java:93` | `cs = getEncoding()` |
| 传给文件对象 | 同上 `:112` | `new FileObject(fileName, cs, "s", dmCtx)` |
| 真正解码 | 同上 `:414` | `new FileCursor(fObj, 1, 1, columns, types, s, t+q, dmCtx)` |

⇒ 产品 CSV 解码 = **JDK charset 按 `cs` 解**；夹具生成器
`CalcProbe.writeCsv:277` 也一直用 `OutputStreamWriter(..., "GBK")` 写 ——
M3 之前 16 个夹具能过纯属 **ASCII 巧合**（ASCII 是 GBK 子集，逐字节等价）。

### 25.2 实现：JDK 映射原样冻结（零依赖，collation.bin 先例）

数据链（每一步都有机械证据，禁止发明映射）：

```text
ProbeEncoding.java   遍历 GBK 全定义域（lead 81-FE × trail 40-FE 除 7F），
                     new String(bs, "GBK")，过滤 U+FFFD ⇒ gbk_jdk.bin
                     【23,940 对】——比常见标准表的 21,86 项多：JDK 映射了
                     GB18030 双字节区与私有区，"以产品为准"与"以标准表为准"
                     的差别正在此
scripts/gen_gbk_table.py   只做五项校验（升序/范围/非 FFFD/非代理区/对齐）
                     + 加 12 字节头（magic RPXGBK1 + u32 BE count）⇒ gbk.bin
encoding.rs         include_bytes!("../data/gbk.bin")，二分查找，零运行时文件依赖
```

- `decode(bytes, encoding: Option<&str>)`：None/`""`/`GBK`/`CP936`/`MS936`/
  `WINDOWS-936`（大小写不敏感，`Charset.forName` 语义）→ GBK 路径；
  `UTF-8`/`UTF8` → `from_utf8`（兼容显式声明 UTF-8 的模板）；其余名 →
  `EngineError::UnsupportedEncoding`。
- **全表对拍单测**：`gbk.bin` 与 `gbk_jdk.bin` 逐对全等（23,940 对，
  不是抽查）——"表 == JDK 真值"的机械证据。

### 25.3 `engine_gbk` 夹具（第 17 个，中文全链路）

模板 3×3（`EngineModelsM3.gbkFixture()`），CSV 中文列名/中文值（GBK）：

| 格 | 表达式 | 计算态 |
|---|---|---|
| A2 | `ds1.select(区域)` | 18 扩展 3 子格，中文子格值 华北/华南/华东 |
| B2 | `ds1.select(数量)` | 18 扩展 3 子格（100/200/150） |
| C2 | `ds1.区域 + "站"` | 17（裸 DSVariable 不扩展），值 `华北站`（首行） |
| A3 | `华东`（字面量） | 被扩展挤到计算态第 5 行 |

**顺带的实证（解答了一个隐含假设）**：裸 `ds1.列名`（`DSVariable`）产品判
**不扩展**（`Node.isExtended()` 默认 false，`Expression.createDSNode` 构造的
就是 `DSVariable`）——只有 `select()` 等 DS 函数扩展。第一版夹具用裸
`ds1.区域` 得到 3×3（不扩展），与 Rust 侧 `Expr::DsColumn → false` **一致**
（双方共识，不是分歧）；改为 `select` 版后 5×3，中文子格值驱动矩阵增长。
`engine_sum` 的 `ds1.select(region)` 18 扩展与此同源。

### 25.4 两处偏离/边界的诚实记录

1. **非法字节：显式报错而非 U+FFFD**。JDK charset 默认 `CodingErrorAction`
   是 REPLACE（非法字节静默变 U+FFFD）。本实现报 `MalformedEncoding`
   （带字节偏移）—— M3-02 要消灭的失败模式正是"静默乱码"；夹具内容恒为
   合法 GBK，真实链路不触发。
2. **GB18030 四字节序列：带账不实现**。模板显式写 `GB18030` 且文件含四字节
   序列的场景无夹具（"禁止无基准对拍"）；且产品默认 `GBK` 时 JDK 的 GBK
   charset 同样不接受四字节。命中报 `UnsupportedEncoding`（`spec-m3.md` §8）。

### 25.5 验证矩阵

| 证据 | 结果 |
|---|---|
| `ProbeEncoding` 真机 dump | 23,940 对；锚点 0xD6D0→U+4E2D 等 6 个 codepoint 与日志一致 |
| `gen_gbk_table.py` 五项校验 | 通过（拒绝坏表比没有表重要） |
| Rust 单测（`encoding` 模块） | 5 条：锚点查找 / **全表逐对对拍** / ASCII+GBK 混合 / 四类非法字节 / charset 名派发 |
| `engine_gbk` 主对拍 | `engine_grid_compare` 7/7 全绿（中文列名/子格值/拼接/行挤占逐格一致） |
| workspace 测试 | 全绿（260 单测 + 全部集成；`all_names` 15→17、tripwire 94→97、SHAPE 表 +1 行） |
| Java round-trip | `VERIFY OK: 17 engine fixtures generated` |
| 既有 16 夹具回归 | 全绿（ASCII 按 GBK 解逐字节等价，行为不变） |

## §26 M3-05 `POST /render` 主链路：axum 落地 + 浏览器全链路实测（2026-10-14 深夜）

### 26.1 形态与契约

`rpx-server` 从「快照映射 + dev bin」扩为 **HTTP 服务端**（`spec-m3.md` §3.2 十步
主链路的 HTTP 版）。库/bin 分层（`src/lib.rs` 导出 `form`/`http`/`snapshot`），
集成测试可 import 主链路纯函数 `http::render_request`，与真 socket 响应体双轨对拍。

| 请求字段 | 次数 | 内容 |
|---|---|---|
| `report` | 1 | `.rpx` 字节（魔数 `rpx_core::open_bytes` 校验） |
| `dataset` | 0..n | CSV，multipart `filename` = 模板 `FileDataSetConfig.file_name` |
| `params` | 0..1 | 参数 JSON（口径与夹具 loader 互逆：`1`→Int、`1.5`/`1e2`→Double、数组→List） |

错误分级：表单/模板/引擎问题 → 400 + `{"error": …}`；快照不变式违反 → 500。
上传完整性**预检**：模板引用的 CSV 必须都在表单里，按**文件名**报缺
（引擎的 `MissingDataset` 只带 `ds1` 这种数据集名，上传用户只认文件名）。

### 26.2 引擎侧唯一改动：闭包字节源

`load_datasets` 重构为委托 `load_datasets_with(rd, read: Fn(&str) -> Option<Vec<u8>>)`
——磁盘路径与 HTTP 上传**共用同一份** parse/物化代码（否则两份口径必漂移）。
单测：内存字节 ≡ 磁盘字节（`engine_sum` 双数据集逐格相等），缺文件显式
`MissingDataset`。

### 26.3 验证矩阵

| 证据 | 结果 |
|---|---|
| `tests/render_http.rs`（真 socket + 手写 HTTP/1.1 客户端，零新测试依赖） | 6/6：响应体与库函数直调**逐字节一致** / GBK 中文过网 / params 过网（List 扩到第 5 行）/ 缺文件 400 点名文件名 / 坏魔数 400 / 未知字段 400 |
| 服务端 curl 冒烟 | `POST /render`（engine_gbk）200，快照含「区域/华北/华南/华东」；`/api` 404 坑见 §26.4 |
| 浏览器端到端（playwright 库直驱 + `setInputFiles`） | **E2E_OK**：选 engine_gbk.rpx + csv → `POST /api/render` 200 → Univer canvas 装载；截图 `tests/fixtures/out/m4-render-http.png`（表头 区域/数量/备注 + 华北/华南/华东 + 扩展行 华东站） |
| workspace 测试 | 394 全绿（含引擎新单测 + form 4 条 + http 集成 6 条） |
| 前端 | `tsc --noEmit` 0 错；`vite build` exit 0 |

### 26.4 排查实录：vite 代理「不生效」的根因（不是代理坏）

现象：浏览器 `POST /api/render` → 404；`GET /api/health` 经 vite 空响应/404，
而直连 `127.0.0.1:8090` 全好。逐跳定位：

1. 调试插件证明请求到达 proxy 中间件但**没到**后置中间件 ⇒ proxy 接了请求；
2. 换 node 嗅探服务器当代理目标 → 200 ⇒ vite 代理本身没坏；
3. 原始 TCP dump：转发字节完全正常（`GET /api/health HTTP/1.1`，Host 已改写）；
4. 中间人转发器插在 vite 与 rpx-server 之间 ⇒ **实锤：路径是 `/api/health`
   （`/api` 前缀没剥）**，服务端只有 `/render` 路由 → axum 默认 404 空体。

**根因**：vite 的 `server.proxy` 默认**原样转发全路径**（与经典 http-proxy 一致），
`'/api': 'http://127.0.0.1:8090'` 不会剥前缀。修法 =
`rewrite: (path) => path.replace(/^\/api/, '')`（`web/vite.config.ts`）。
教训：proxy 类的 404 要先打路径再打代理组件——「代理没生效」的直觉这次是错的。
附带记录：vite 不带 `--host 127.0.0.1` 绑 IPv6 `::1`（§7 已记）。

## §27 M3-06 分页真实口径：纸码表 + 双口径分道 + 守卫补齐（2026-10-14 深夜）

### 27.1 两条口径的 Java 事实源（都是 faithful port，不是漂移）

| 口径 | Java 路径 | 页尺寸 |
|---|---|---|
| 探针（M2-11 夹具基准） | `CalcJson.renderPages` → `ReportUtils2.getPagerInfo(rpt,595,842,1)`：`setPaperSize`（**置纸码 256=自定义**，`PagerInfo.java:118-121`）+ 清零四边边距（`ReportUtils2.java:107-115`） | 595×842pt 零边距（`paper_size_pt` 复刻 float32 往返） |
| 模板真值（M3-06，`/render`） | `new PagerInfo(PrintSetup)` **不覆盖**（`PagerInfo.java:43-68`） | 纸码表(80 码) 或自定义字段 → (纸mm − 边距) × 72/25.4（hp=true，`ReportDefine._$16=true`） |

关键实锤：**`setPaperSize` 会把纸码改成 256**——所以探针的 209.9×297.0mm 走 switch
默认分支（自定义字段），而模板路径按纸码表（9=A4=210×297）。两套算术不同源：
探针是 pt→mm(f32)→pt；真值的值**本来就以 mm 为单位**（模板槽 9/10），无那次往返。
orient **不换宽高**（`PagerInfo.getPageFormat` 只写 `PageFormat.setOrientation`，
SimplePager 走它；`PrintSetup.java:920-935` 那个轮换边距的方法不在链路上）。

实现：`pager/paper.rs`（304 行）—— 纸码表由 `scripts/gen_paper_table.py` 从
`PrintSetup.java` 两个 switch **生成**（80+80 项，禁手抄；`python3` 的文件访问
必须走 argv，内联 `-c`/stdin 会被沙箱拦——本轮新坑）。

### 27.2 双口径分道与快照接线

* `calculate()`（夹具路径）保持探针口径 —— 17 夹具的 `pagination` 段对拍**不动**。
* `pager::paginate_with_setup(grid, ps)`（新）：`usable_page_pt` + `tableColumnNum`
  + `row_num_per_page` 取模板值，自带 `ensure_supported` 复核与可用尺寸非正检查。
* `/render`（`http.rs`）与 dev bin（`main.rs snapshot_cmd`）计算后**覆盖** pagination
  ⇒ 快照 workbook 顶层 `pagination` 块（契约 `spec-m4.md` §4.5，rpx 扩展键）。
* `PageInfo` 加 `row_range`/`col_range`（emit 逐键序列化不读 ⇒ 对拍输出逐字节不变，
  `emit/tests.rs` 断言钉死）。
* 实测：默认 A4 减边距 = 487.559×700.157pt；`engine_sum` 真值下 2 页变 **横向 x=2**
  （宽 487.6 < 595）；`engine_pager` 行切点不变仍 4 页；`row_range` 页间重叠如实
  （区带/组头副本每页重复）。

### 27.3 守卫：补了什么、为什么撤了一个

* 补 `tableColumnNum != 1`（`PageBuilder.init1:207` → `pager.setColumns(pi.getTableColumnNum())`
  —— 多栏的另一个入口，此前只拦 `columnMode`）。
* 补**列区带**守卫 `ensure_data_columns`：`colType != -79`（`IColCell.java:15-17`，
  -80 左列头/-78 右列头）显式报错——`split_cols` 固定按 1..=cols，产品 `_$2(r)`
  会排除区带列。
* ⛔ **撤掉换行静态守卫**（写完即被 17 夹具全红证伪）：`-64` 是
  `INormalCell.TYPE_NORMAL`（`INormalCell.java:21`，普通文本格默认类型，任何模板
  都有大量）——测量分支（`SimplePager._$1:1557-1573`）的条件「值 instanceof String
  且页内放不下」**都是断页期才知道的事**，静态拦截 = 拒绝所有真实模板。核对结论：
  运行时风险带账（`guard.rs` 尾注），精确守卫需把模板 cell_type 接进计算网格——记后续带账。

### 27.4 验证矩阵

| 证据 | 结果 |
|---|---|
| `paper.rs` 单测 | 4 条：纸码表抽查（A4/Letter/A3/9999）/ 自定义码兜底（含 f32→f64 提升精度）/ 默认 setup 可用尺寸 / 自定义纸+边距 |
| `pager_guard.rs` | 5 条（原 2 + 新 3）：17 夹具 `ensure_data_columns` 不误伤 / `tableColumnNum=2` 拒绝 / 列区带变异（-80）拒绝 / `paginate_with_setup` 自带守卫复核 |
| `engine_grid_compare` | 17 夹具全绿——探针口径与对拍**逐字节未变**（`PageInfo` 新字段不进 emit） |
| `render_http.rs` | 8/8（+2）：`pagination` 块真值断言（487.559/700.157，且非 595×842）+ 0 起坐标 / 引擎错误→400 映射（UnknownParam） |
| dev bin 冒烟 | `engine_sum` 真值 2 页(横向 x=2)、`engine_pager` 4 页、`engine_gbk` 1 页 |
| workspace | 全绿（engine 266 lib + 全部集成） |

## §28 M3-03：类型归属定案 + rpx-ds 骨架 + sqlite 真连冒烟（2026-10-15 凌晨）

### 28.1 定案（spec-m3 §3.4）

物化契约**下沉 `rpx-core`**：`Value`（值模型，11 变体对 `CalcJson.typeName` 封闭）
+ `RowSet { name, cols, rows }`；`rpx-ds` 只依赖 `rpx-core`，**不依赖 `rpx-engine`**；
引擎 `DataSet` 保留运行时态（`DsCur`/`selected`）经 `DataSet::from_rowset` 唯一入口。

裁决理由（对照 §8 旧问句）：① 供数方不依赖消费方（rpx-ds→rpx-engine 会把
arena/expr/pager 全拖进取数 crate）；② `DataSet` 整体下沉不可行——它带求值期
指针态，物化契约与运行时态必须分离；③ `Value` 零依赖纯数据可干净下沉，engine
`value.rs` 改 re-export 保两条既有路径零改动（server/cli 引用不动）；④ `RowSet`
与 `ReportDefine`/`PrintSetup` 同层（引擎输入契约归 core 是 M1 起的分层纪律）。

### 28.2 rpx-ds 骨架

* `DataSource` trait（`fetch(query)` → `RowSet`，Rust 原生 async fn in trait）
* `SqlSource`：`AnyPool` 按 URL 前缀选驱动；sqlite 系池上限强制 1（蓝图坑③）；
  `install_default_drivers()` 先行（坑①）
* `dialect.rs`：四库方言表（占位符/分页/布尔字面量）——**只立表不实现改写**，
  模板 SQL 原样透传（§8 SQL 注入面同口径：用户资产、单实例可信）
* 值映射 `any_cell`：级联 try_get（i64→i32→i16→f64→f32→bool→String），
  全落空按**列类型名**显式报 `UnsupportedColumnType`——不塞占位值
* G5 登记：sqlx 0.9（any/postgres/mysql/sqlite/runtime-tokio）+ tokio（理由表 code-review §3.2）

### 28.3 sqlx 0.9 实测坑（三个，全有单测钉着）

1. **`SqlSafeStr` lint**：0.9 起动态 SQL 被拒（只认 `&'static str`）。模板 SQL 是
   用户资产 ⇒ 按官方口径 `AssertSqlSafe(sql.to_owned())` 显式豁免并留档。
2. **`AnyPool` 挪进私有 `reexports`**：facade 只公开 `Any`/`AnyPoolOptions`/`AnyRow` ⇒
   字段类型用 `sqlx::pool::Pool<sqlx::any::Any>`（AnyPool = Pool<Any> 的别名）。
   `Row`/`Column`/`ValueRef`/`TypeInfo` 四个 trait 都要显式 use（方法全在 trait 上）。
3. **sqlite 声明 `BOOLEAN` 的列 Any 层取不出来**：fetch 期即报
   `AnyDriverError("Any driver does not support the SQLite type SqliteTypeInfo(Bool)")`
   ——eager 报错，级联 try_get 根本没机会。布尔列在 sqlite 上用 INTEGER 0/1
   （`Long`）承载；PG 的 `BOOL` 走 `Bool` 分支待真库冒烟验证（spec-m3 §8 带账）。

### 28.4 验证矩阵

| 证据 | 结果 |
|---|---|
| `rpx-ds` 单测 | 5/5：sqlite 内存库真连真取（建表/插入/SELECT 物化，类型逐格断言 + NULL 落 Null）/ 未知 scheme 显式报错 / 坏 SQL 透驱动错误 / 方言分派 2 条 |
| core/engine 桥 | `RowSet` 单测 + `from_rowset` 对拍（name/cols/rows 字段级一致 + 与 `new` 直构等价） |
| 既有面 | `Value` 下沉后 engine 内部与 server/cli 引用**零改动**（re-export）；17 夹具对拍不变 |
| workspace | **410 测试全绿** + **G1–G7 全绿** |

## §29 M3-04 报表资产库：AssetStore + 三路由（2026-10-15 凌晨）

### 29.1 选型偏离定案（spec-m3 §3.5）

libSQL（蓝图 §7.1）→ **sqlx/sqlite 承载**：libsql 不在离线缓存（沙箱无法验证），
而蓝图自注本地模式下其增量仅 async API（文件格式同 SQLite）；sqlx 已入仓零新增
依赖面；升级路径保留（只动 `asset.rs`）。schema 按蓝图 §8.2 落地并加存
`param_type`/`data_type`/`description` 原样（`is_static` = `param_type==0` 派生，
`Param.java:22-24`：NORMAL=0/DYNAMIC=1/ATTRIBUTE=2）。

### 29.2 行为要点

* **参数清单上传时落库**：`put` = `rpx_core::open_bytes` 魔数预检 → 解析
  `rd.params` → 事务写 BLOB + 清单 ⇒ `GET /meta` 读库即答（前端生成参数表单
  不依赖解析器）；空清单与不存在**可区分**（`[]` vs 404——先证存在性）。
* 三路由（`http.rs`）：`POST /reports`（multipart `file`，文件名去扩展名 =
  报表名，与 `/render` 同口径）/ `GET /reports` / `GET /reports/{id}/meta`；
  `serve --db rpx.db`（默认 rpx.db，蓝图 §7.1 单文件部署）。
* 外来类型不许 impl 本仓 trait（E0117）：`AssetError` 的 IntoResponse 改为
  本地辅助函数 `asset_error_response`。
* `AssetStore` 住 `rpx-ds`（蓝图 §7 同章逻辑）；`rpx-server` 加 path 依赖
  `rpx-ds`（G5 不涉及 path 依赖）。

### 29.3 本轮新坑（各值一条纪律）

1. **Rust 字符串续行 `\`+换行会剥掉下一行全部前导空白**——
   `"…, description\` + `\n FROM …"` 拼成 `descriptionFROM`，SQL 被静默破坏
   （报错还是误导性的 `no such column: name`）。逐级二分（schema dump →
   逐条语句试跑 → 按名取值）才定位到字符串本体。**排查实录**：schema 正确、
   INSERT 成功、裸 SELECT 全过，唯 meta() 挂——最后对比发现唯一差异是续行。
   修法 = SQL 字符串一律单行；全仓扫描确认其余续行均安全（`(` 前不需空格 /
   行尾自带空格 / 中文拼接无损）。
2. **axum 0.8 路由捕获语法是 `{id}`**，`:id` 在运行时 panic
   （`Path segments must not start with ':'`）——0.7 的写法到 0.8 是**运行时炸**
   而非编译期。
3. 测试辅助 `post()` 写死 `/render` 路径——加 `post_to(addr, path, …)`，
   旧签名转发保持既有用例不动。

### 29.4 验证矩阵

| 证据 | 结果 |
|---|---|
| `rpx-ds` asset 单测 | 4 条：坏魔数拒收且不落库 / `full.rpx` 参数清单往返（startDate DT_STRING=11 默认 2024-01-01、topN DT_INT=1 默认 10，`Types2.java:22/:32`）/ 列表+缺 id 404 / 三类 param_type 分组纯映射（无名参数跳过） |
| `render_http.rs` | 11/11（+3）：上传→列表→meta 全链路 / 坏魔数 400 且 `GET /reports` 仍 `[]` / 缺 id 404 |
| workspace | **417 测试全绿** + **G1–G7 全绿** |

## §30 M3-07 收官：G8 分层矩阵 + e2e 冒烟（2026-10-15 晨）

### 30.1 G8 crate 分层矩阵（gate.sh）

path 依赖方向 = 架构：`core <- {engine, ds} <- {cli, server}`（M3-03 定案）。
越界 / 未登记 crate 均 FAIL。**变异测试闭环**：rpx-cli 临时加 `rpx-ds` → G8 红
（`分层违规 rpx-cli -> rpx-ds`）；撤 → 绿。

⚠ 变异测试还抓出 G8 初版自身 bug：awk 多写了 `-v RS=`（段落模式），
逐行匹配全失效——变异注入「该红却绿」，正中本仓「改夹具必红」纪律的靶心。
G5 一直没有 RS= 是对的（同一套 FNR 逻辑）。

### 30.2 e2e 冒烟（scripts/e2e_smoke.sh）

真二进制全链路（与 cargo test 集成测试的区别：覆盖 main.rs 参数解析 +
AssetStore 建库 + 编译产物可执行这一层）：
health → `POST /reports`（full.rpx）→ `GET /meta`（startDate/topN 静态参数 +
默认值）→ `POST /render`（engine_gbk）断言快照键集（id/sheetOrder/mergeData/
cellData/rowData/columnData/pagination/page_w_pt）+ GBK 中文数据 → 坏魔数 400
且 `GET /reports` 仍 `[]`。trap 清理服务与临时库。

### 30.3 本轮三坑

1. **sqlite URL 两形态都炸**（code 14 unable to open database file）：
   `sqlite://target/x.db` 把 `target` 当 URL **host**；`sqlite:///target/x.db`
   又把相对路径**拔高成绝对**（Windows 下落 `C:/target`）。根治 =
   `AssetStore::open` 改收**纯文件路径**，内部
   `SqliteConnectOptions::filename()` + `create_if_missing(true)`，绕开 URL
   解析层（`:memory:` 照常特殊文件名）。
2. **`cd X && cmd &` 老坑三连**（§26.4 记过又踩）：cd 被吞进后台子 shell ⇒
   主 shell cwd 漂移，后续相对路径全落空（本轮 python/cargo/mkdir 全中招）。
   e2e 冒烟脚本因此把临时库放**仓库内相对路径**并依赖脚本自身的 `cd "$ROOT"`。
3. Bash 工具的 cwd **每次调用重置**到会话初始目录——上一条里 cd 过不代表
   下一条还在；所有多步操作要么单条内闭环、要么绝对路径。

### 30.4 验证矩阵

| 证据 | 结果 |
|---|---|
| G8 变异 | `cli->ds` 注入必红 / 撤销必绿 |
| e2e 冒烟 | 全绿（6 步断言） |
| workspace | 417 测试全绿 |
| G1–G8 | 全绿 |

## §31 M4-04/M4-05 收官：多 sheet 翻页 + 参数表单（2026-10-15 晨）

### 31.1 多 sheet 切片（snapshot/paged.rs）

`pagination.page_count > 1` ⇒ 每页一个 sheet（`rpx-sheet-{n}` /「第 N 页」），
`sheetOrder` 顺序即页序（Univer tab = 翻页）。坐标平移（页内 = 全局 − 页偏移）；
页间重叠的区带/分组头副本**自然每页重现**（切片法与产品独立页矩阵同观感）；
跨页合并区**裁剪到页界**（锚点在页内才保留）——与产品「拆分」的显示差异记
spec-m4 §4.6 声明近似（产品拆分语义无夹具基准）。单页/无 pagination 走原单
sheet 路径 ⇒ 夹具对拍（走 rpx-engine emit）不受影响。

真实夹具 `engine_pager`（真值 4 页）实证：4 tab、页 1-4 row_range
`[0,11]/[1,22]/[1,33]/[1,42]`（页首重叠如实）、各页 cellData 0 起平移、
merged 全空（M2 已声明缺口）。

### 31.2 参数表单（web）

App.tsx：渲染改**显式按钮**；参数动态行（名称/值 + 内联 SVG 加/删图标，无 emoji；
配色 Token 化 `:root` 变量）；值推断与 `form.rs::json_to_value` 同规
（整数→Int / 小数→Double / 其余字符串）。tsc 0 错 + vite build exit 0。

### 31.3 浏览器实测（playwright 直驱）

`engine_pager` 渲染 → 4 个页 tab 全渲染 → 切「第 4 页」→ 页首区带副本
`group: R*` 在页 4 重现、数据从 R1 起（坐标平移正确）。截图
`tests/fixtures/out/m4-paged-tabs.png` / `m4-page4.png`。
（选择器教训：Univer 1.0.4 的 tab 类名不是 `.univer-sheet-tab`——断言用**文本**
`getByText('第 4 页', { exact: true })` 而非内部类名。）

### 31.4 验证矩阵

| 证据 | 结果 |
|---|---|
| paged 单测（真实夹具） | 4 sheet / 名/ 键 0 起且不越页界 / merged 空（已声明缺口） |
| render_http / snapshot 全测 | 11/11 + 15/15 |
| 浏览器 | canvas + 4 tab + 切页 + 参数表单交互全通 |
| workspace | 418 测试全绿；G1–G8 全绿；e2e 冒烟全绿 |

## §32 带账清偿轮：金仓真连冒烟 + 跨页合并拆分 + M2-22 语义定案（2026-10-15 晨）

**总监令「③ 带账清偿」。九项账目盘点（spec-m3 §5/§8 + spec-m2 §6.2），本轮清偿/升级四项。**

### §32.1 PG 真连冒烟清偿（金仓 KingbaseES 实锤）

- 总监提供公司服务器 192.168.1.77:54323（库 `test`，用户 `system`，密码走会话即清）；
  TCP 探通后 `SELECT version()` 自报 **KingbaseES V008R006C008B0014**（人大金仓，
  PG 线协议兼容）——sqlx postgres 驱动**直连成功**，`system`/`test`/非标端口
  组合与金仓出厂默认吻合。
- 新增 `crates/rpx-ds/tests/smoke_pg.rs`（env 门控 `RPX_PG_URL`，未设时 SKIP 保
  G3 全绿；本机门禁挂上该变量后常驻真跑）：建 `rpx_smoke_t`（9 类型列 + NUMERIC）
  → 插 2 行（含 NULL/负数/中文）→ 物化逐格断言 → `DROP` 清理。
- **三条真机实锤**：
  1. **整数族统一 `Long`**——sqlx Any 层 `i64::compatible = ty.kind().is_integer()`
     （sqlx-core-0.9 `any/types/int.rs:15/43/71`），级联先试 i64，int2/int4/int8
     全兼容；金仓真机同此。`any_cell` 的 i32/i16 分支只是非常规驱动兜底。
  2. **BOOL → `Bool` 分支实锤**（spec-m3 §8 带账半边清偿）；REAL→`Float`、
     DOUBLE→`Double`、TEXT/VARCHAR→`Text`、NULL→`Null` 同轮验证。
  3. **NUMERIC fetch 期响亮失败**——`DsError::Sql(ColumnDecode("...Any driver
     does not support the Postgres type ... Numeric"))`，**到不了 `any_cell`
     级联**（Any 层 eager 拒绝，与 sqlite BOOLEAN 同构）；`any_cell` 尾注已改写
     （`UnsupportedColumnType` 分支只对「Any 认识但九连 try_get 落空」可达）。
- 过程坑两枚：① 首轮断言按「INT4→Int」写期望被真机打脸（见实锤 1）；② 测试里
  页 1 期望值算术错 + 漏两个横向 span 块，均被对拍当场抓出（实现无错，测试错）。

### §32.2 sqlite BOOLEAN 钉子 + CLI 断言清偿

- `sql_source.rs` 新增钉子 `sqlite_declared_boolean_column_fails_loudly`：
  sqlite 声明 `BOOLEAN` 的列 fetch 期报错且文本含 "does not support"——
  响亮失败契约钉死，业务承载 = INTEGER 0/1（`Long`）。
- `e2e_smoke.sh` 步骤 ⑤ CLI 冒烟（spec-m2 §9「CLI 无自动化断言」清偿）：
  真二进制 `rpx-cli print engine_sum.rpx` 断 `^matrix:`/`^cells: [1-9]\d* non-default`/
  `^print_setup: paper=`（ASCII 锚避控制台编码），负路径缺文件非零退出。

### §32.3 跨页合并拆分清偿（声明近似 → 产品语义）

- 源码实锤：`SimplePager.java:1624-1638`（行向）——断行处 `deepClone` 锚点、
  `newArrea = Area(row, startCol, pageStartRow+endRow, endCol)`（延续段独立成块）、
  `ar.setEndRow(_$28-1)`（上页块裁到页末）；`:2316-2335`（列向）同构；非文本格
  走同一路径（`cellHeight==0` 不触发整块搬移条件）。退化分支（文本高于整页 /
  `rt==-90` 标题行整块搬移，`:1574-1622`）仍带账（需字体测量，无夹具）。
- 实现 `snapshot/paged.rs`：`merge_data_paged` 改**全局块 × 页矩形求交**；锚点
  在页外 ⇒ 交叠区左上角**注入克隆锚点**（值/样式取原锚点格 = deepClone 快照
  等价物；`setCell` 覆盖语义同产品 `:1632-1634`）；`cell_json` 抽公共出口。
- 专项测试 `paged_snapshot_splits_cross_page_merge_with_cloned_anchor`：
  engine_merge 真网格 + 自造分页（页界切 North 块行 1/2 之间）——上页裁剪、
  延续块、克隆锚点值=原锚点值、其余块按交叠平移，全部逐键断言。

### §32.4 M2-22 残余语义定案（无基准带账 → 有据带账）

三要素源码定案（详见 spec-m2 §6.2 2026-10-15 条目）：`customPropEngMap` = 模板
自定义属性表达式表（格/行/列三类，`NormalCell.java:605` 等）；state 8 队列 =
挂起表达式推 `third` 队列（与值通道共用 trampoline，无新调度器）；cell-graph
属性值对象 = `customPropSetMap`，消费端 `getCustomPropertiesMap():135-144` =
表达式值覆盖模板静态自定义属性——**纯渲染元数据，对计算网格零影响**。探针
`CalcJson` 不导出自定义属性 ⇒ 无产品基准，维持不实现；复现路径已写明（扩探针
→ 18 号夹具 → 三步实现）。

### §32.5 账本余额（清偿后）

| 账目 | 状态 |
|---|---|
| PG 真连冒烟 | ✅ 清偿（金仓实锤；门禁常驻） |
| sqlite BOOLEAN | ✅ 钉子已装 |
| CLI 自动化断言 | ✅ e2e 步骤 ⑤ |
| 跨页合并拆分 | ✅ 产品语义实现 + 专项测试 |
| M2-22 残余 | ⏳ 有据带账（语义定案 + 复现路径） |
| MySQL 真连 | ⏳ 带账（无服务器） |
| 换行回填 / GB18030 / 两扩展格同行 / 样式宏 | ⏳ 沿用前账 |

| 证据 | 结果 |
|---|---|
| rpx-ds（含 smoke_pg + BOOLEAN 钉子） | 10 lib + 1 集成全绿 |
| rpx-server（含跨页拆分专项） | 16/16 全绿 |
| e2e 冒烟 | 上传→meta→render→负路径→CLI print 全绿 |

## §33 第 18 号夹具 engine_emp：员工信息查询——select 主格 + 裸列跟随（2026-10-15 晨）

**总监拿一张设计器截图问「能实现吗」：`=ds1.select(编号)` 一列 + 同行四格裸
`=ds1.部门/姓名/性别/工资`（员工信息查询，5 列）。答案：能，且语义早有夹具
背书——engine_gbk 的裸引用 `ds1.区域+"站"` 已逐行跟随（华北站/华南站/华东站）。
本轮把截图**原样**做成夹具，产品真机 + 引擎对拍 + 浏览器渲染三重实证。**

### §33.1 夹具（engine_emp，3 行 × 5 列）

- `EngineModelsM3.empFixture()`：r1 标题字面量「员工信息查询」、r2 表头
  编号/部门/姓名/性别/工资、r3 = `ds1.select(编号)` + 四条**裸** `ds1.<列>`
  （无任何算子装饰——把跟随语义钉到最素形态）。
- 数据 5 名员工（GBK CSV，中文列头）：张伟/李娜/王强/赵敏/孙丽，工资 8500..7800。
- 产品真机：计算网格 **7×5**（标题+表头+5 行带）；五条裸引用**逐行跟随**
  （人事部/财务部/技术部/技术部/市场部 …），标题/表头原位不动。
  `VERIFY OK: 18`（回读自校验），既有 17 夹具重生成零字节扰动。

### §33.2 Rust 侧接入

- `loader.rs::all_names` 17 → 18；`engine_grid_compare` 逐格对拍通过。
- **钉子按设计先红**：`expr_state_tripwire` 的 `TOTAL_FORMULAS` 97 → 102
  （5 条全 state 4：Select/DSVariable 均未覆写 getState）——先核对再改常量，
  留档于常量注释。

### §33.3 端到端演示（浏览器）

- `out/emp_demo.cjs`：服务端 `POST /render` 内容断言（员工信息查询/张伟/孙丽/
  12000/7800 全命中，cellData 7 行）+ 网页选文件渲染 → Univer canvas 截图
  `out/m4-emp-demo.png`（标题+表头+5 员工行，与总监截图同形态）。
- 坑三枚：① Univer 内容画在 **canvas**，`getByText('张伟')` 永远等不到
  （DOM 无此文本）——内容断言改走服务端快照，浏览器只截图；② `cd && … &`
  把整条 `&&` 链吞进后台子 shell（主 shell cwd 不变）——后台命令一律子 shell
  形式 + 绝对路径；③ playwright 库三处版本漂移（mcp 1.58 要 1205 内核未装）
  —— 用 managed node 自带的 `@playwright/cli/node_modules/playwright` 1.64
  （chromium-1247 已装）。

| 证据 | 结果 |
|---|---|
| 产品真机 GenEngineFixtures --verify | VERIFY OK: 18 |
| engine_grid_compare（18 夹具逐格） | 全绿 |
| expr_state_tripwire | 102 条全 state 4 |
| workspace | 全绿（无 FAILED） |
| 浏览器 | 快照内容断言 + 截图 m4-emp-demo.png |

## §34 SQL 数据集链路闭环：模板自带 SQL 直连金仓（2026-10-15 晨）

**M3-03 的最后一块：`.rpx` 里声明 `SQLDataSetConfig` → rpx-ds 真连数据库 →
引擎计算 → 与产品真机逐格对拍。全程金仓 KingbaseES V8R6（PG 驱动直连）。**

### §34.1 反逆向定案（re_work/usermodel）

- `SQLDataSetConfig`（version=5）`.rpx` 记录体（**serialize()/fillRecord 通道**，
  与 FileDataSetConfig 同构）：基类 6 槽 blob + sql + beginRow + endRow（表达式
  行窗口）+ schema + params(i16 × [exp,type]) + isGuarded + threshold。
- ⛔ **凭据与模板分离**：连接三要素在 `NormalConnectionFactory`（调用方注册
  `ctx.setConnectionFactory(dsn, factory)`），配置文件 `raqsoftConfig.xml`
  `<DBList>`；模板只存 `data_source_name` 引用。rpx-server 照抄此架构。
- `SQLDataSetFactory.createDataSetM`：宏 `${...}` 替换 → `?` 占位参数求值
  （List 展开）→ `isGuarded` 先 `count(*)` 过阈值守护 → `noDataPrompt`
  空结果显式抛 `NoDataException`。
- GitHub 调研（总监令）：无 rpx 开源实现；官方开源 **esProc SPL**
  （SPLWare/esProc）同源可对照（SQLTool/Sentence/RQException 干净源码）。

### §34.2 探针 ProbeSql.java → 第 19 份夹具 engine_sql

- PG JDBC 驱动（postgresql-42.7.4，Maven Central）连金仓——线协议兼容直连。
- 探针自建 `rpx_sql_smoke` 表（5 员工，英文列名——第一探针降不确定度）；
  模板 2 行 4 列：表头 + `ds1.select(id)` / `ds1.dept` / `ds1.salary` /
  `sum(ds1.salary)`（SQL 数据集上的裸引用跟随与聚合退化同轮实证）。
- 产品真机：**`VERIFY OK: engine_sql round-trip`**；计算态 r=1..6；
  `sum(ds1.salary)` 逐行退化（8500/9200/12000/11000/7800）——与
  engine_sum 的 current-row 语义一致。

### §34.3 Rust 侧

- `rpx-core::dataset`：`SqlDataSetConfig` 解析（真实探针 .rpx 单测钉死；
  `params` size==0 **保持 null**——产品 readExternal 不建列表，Some([]) 是错的）
  + `factory_class_for` 加 SQL 映射。
- `rpx-engine::dataset`：`load_datasets_full(rd, read, sql_fetch)`——**SQL 执行
  由调用方闭包注入**（G8：引擎不依赖 rpx-ds；cli/server 用 SqlSource 实现）；
  数据集名来自配置（`ds.name = name`，SQL 结果不携带名字）；新错误
  `SqlDatasetNoConnection`（通道未注册）/`SqlFetch`（取数失败文本原样上抛）。
- `rpx-server::tests/sql_fixture.rs`（env 门控 `RPX_KB_URL`）：金仓真机
  逐格对拍（值+类型）**全绿**；async 取数桥进同步委托闭包 =
  多线程 runtime + `block_in_place(block_on)`。

### §34.4 带账与坑

- 带账：SQL 数据集的 `?` 占位参数 / begin_row/end_row 行窗口 / 宏替换
  未建模（探针未覆盖，解析槽已保留）；MySQL 真连（无服务器）。
- 坑：javac `--release 8` 无 `FileWriter(String,Charset)`；bash 通配符
  `*.java` 依赖 cwd（连续漏 cd 三连——写成 run_kb_tests.sh 脚本终结）；
  `cargo` 输出 grep "error" 会抓到 incremental lock 噪音（过滤 "lock file"）。

| 证据 | 结果 |
|---|---|
| 探针（产品真机连金仓） | VERIFY OK: engine_sql round-trip |
| rpx-core 解析单测（真实 .rpx） | 全绿（含凭据不入模板断言） |
| engine 委托管线单测 | 全绿 |
| 金仓真机逐格对拍 sql_fixture | 全绿（env 门控） |
| 三 crate 全量 | core 19 / engine 263 / server 全绿 |

## §35 服务面数据源注册：`/render` 吃上数据库真数（2026-10-15 午）

**M3-05 的 SQL 数据集延伸：`serve --datasources <path.json>` 建 `dsn → SqlSource`
注册表，`POST /render` 对含 `SQLDataSetConfig` 的模板预取真数后进主链路——
浏览器端到端从此能直连金仓。**

### §35.1 架构（产品同构）

- `crates/rpx-server/src/datasource.rs`（新）：`DataSourceRegistry`——JSON 配置
  （字符串形态 = 完整连接串 / 对象形态 = `{url,user,pass}` 分写）→ 启动期逐条
  建池 **fail fast**；`Debug` 手写（连接池不进输出，凭据不入日志——与
  「凭据不入模板」同纪律）。产品对照：`raqsoftConfig.xml` 的 `<DBList>`。
- `http.rs` 改造：应用状态升级 `AppState { store, dsr }`（三资产 handler 同步
  迁移——Router 内 State 类型必须统一，混用直接 E0308）；主链路拆三层：
  - `render_request(req)`（同步纯函数，测试对拍基准）：无注册表路径；
  - `render_request_ds(req, dsr)`（异步 handler 入口）：**先预取**——
    `prefetch_sql` 把全部 SQL 数据集按 `(dsn, sql_text)` 物化成
    `HashMap → RowSet`（同键只取一次），再进同步 `render_core`；
  - `render_core`：文件预检 + `load_datasets_full`（SQL 委托闭包查预取表）
    + 计算 + 分页 + 快照——G8 分层不动（async 取数只在 server 编排层）。
- `main.rs`：`serve --datasources <path.json>`；不配置 = 只服务文件数据集模板。
- 错误面（全 400，消息给前端上屏）：`NoDataSource`（模板含 SQL 数据集但没配
  注册表，点名 dsn + 补救路径）/ `UnknownDataSource`（列出已登记数据源名）/
  `SqlFetch`（sqlx 错误文本原样透传）。

### §35.2 测试（sql_fixture.rs 3 → 6）

| 测试 | 环境 | 断言 |
|---|---|---|
| `sql_dataset_without_registry_is_400` | 零环境 | 400 + 点名 kingbase + `--datasources` |
| `sql_dataset_unknown_dsn_is_400` | 零环境（sqlite 内存库当注册项） | 400 + 「数据源未登记：kingbase（已登记：other）」 |
| `render_http_sql_dataset_matches_library` | `RPX_KB_URL` 门控 | **HTTP 响应体 ≡ 库函数直调逐字节一致** + 内容抽验（HR/MKT/8500/7800 流到快照） |

- 新坑一枚（真机实锤）：两条金仓测试并行 DROP/CREATE 同一张表，在金仓
  **目录索引上撞 23505**（`_typ_typname_nsp_index`）——共享
  `tokio::sync::Mutex` 全程互斥后收敛（模板 SQL 表名写死，串行是唯一解）。
- 教训再钉：run_kb_tests.sh 的过滤 grep 把编译错误也吞了（两次实锤）——
  脚本改为全量日志落盘 `target/kb_tests.log` + 失败时回显 error 行。
- 噪音识别：每 target 「generated 1 warning」= 沙箱 incremental 锁噪音
  （os error 5，前账），clippy `--all-targets` 强制重编译后零真告警。

| 证据 | 结果 |
|---|---|
| rpx-core / rpx-engine / rpx-server | 全绿（263 + 全部集成） |
| 金仓 HTTP 端到端 | 响应体 ≡ 库直调逐字节 |
| 门禁 G1–G8 | 全绿 |

## §36 浏览器实拍金仓真数演示：report schema（2026-10-15 午）

**总监令「① 浏览器实拍金仓真数演示 + test 库创建 report schema」。零 Rust
代码改动——上一轮的数据源注册架构直接吃上真数。**

### §36.1 金仓侧（探针 ProbeSqlDemo.java 幂等）

- `CREATE SCHEMA IF NOT EXISTS report`（schema 常驻，不随演示删除）；
  `report.emp_info(id, dept, name, salary)` 先删后建，插 5 名中文员工
  （engine_emp 同款数据：张伟/人事部/8500 … 孙丽/市场部/7800）。
- 模板 `report_demo.rpx`（3 行 4 列）：r1 标题格 `setMergedArea(Area(1,1,1,4))`
  跨 4 列「员工信息查询（金仓 report schema 真数）」、r2 中文表头、
  r3 = `ds1.select(id)` + 三条裸引用（dept/name/salary）；
  `SQLDataSetConfig` 只存 `data_source_name=kingbase` 引用 + SQL
  `SELECT … FROM report.emp_info ORDER BY id`（凭据不入模板）。
- 产品真机：round-trip `VERIFY OK: report_demo`；产品真值
  `out/report_demo_product.json` 28 格（标题合并 4 槽 + 5 行中文员工带）。

### §36.2 端到端实拍（`out/kb_demo.cjs`，绝对路径单链闭环）

- `serve --db kb-demo.db --datasources out/ds_kingbase.json`（注册表日志：
  「数据源注册表已就绪：kingbase」）+ vite → playwright。
- 服务端断言：快照 7 行、mergeData 1 块（标题合并进快照）、内容抽验全命中
  （张伟/孙丽/人事部/市场部/12000/7800）→ Univer canvas 截图
  `out/m4-kingbase-demo.png`。
- ⛔ `out/ds_kingbase.json` 含真实密码——`out/` 整体 gitignore，**不入库**；
  仓内只有不含凭据的 `report_demo.rpx`（SQL 数据集语义的实证：凭据与模板分离）。

| 证据 | 结果 |
|---|---|
| 探针 round-trip | VERIFY OK: report_demo |
| /render 快照 | 7 行 + 标题合并 + 中文内容全命中 |
| 浏览器实拍 | m4-kingbase-demo.png（金仓真数 → Univer） |

## §37 多数据集关联立项：显式左主格 + DSSelect 参数矩阵（2026-10-15 午后）

**总监令「下一步建议 开工」。行动序 #2（`docs/lecture-notes.md`）落位：多数据集关联
语义立项——JOIN 三分类 + 维度对齐，报表工具核心差异化。按「先探针后实现」纪律：
本 session = 反逆向定案 + 金仓真机探针 + 立项文档，零 Rust 代码改动。**

### §37.1 反逆向定案（源码级，全带 file:line）

- **官方口径实锤**：润乾多数据集「不需要整合运算，直接用于报表运算，运算过程中通过
  规则关联」——无 join、无临时表，关联发生在求值时刻（手册 §3.3）。
- **`ds.select` 参数矩阵三层切分**（`ParamParser.java:35-64`）：`\0` 分段 →
  `;` 分排序段 → `,`/`:` 分参数；四参形态 `select(列, 过滤, 限量n, 去重d)` +
  次序 `x:1` + 多排序 `x; s1:1,s2:-1`。Rust 现状只有 1 参数（`dsfn.rs:216-309`）。
- **双态分派**（`DSSelect.java:146-205`）：Row 态（current 是 Row）只测当前行，
  **num/distinct/order 全部不生效**；Root 态（current 是 Group）走
  filter→distinct→evaluateRows→排序，返回 List ⇒ 单元格扩展。
- **嵌套机制 = 显式左主格（slot 30）**（本 session 最大机制发现，五步闭环）：
  ① `NormalCell._$56/_$55` = 单元格 ID 字符串，序列化于 slot 30/31；
  ② `ExtCellSet._$3/_$2`（`:753-810`）**先查显式字符串**再走默认近邻拓扑；
  ③ 跨度计算 `_$4/_$3`（`:944-970`）**递归纳入左子格** ⇒ 主格跨度覆盖从格带；
  ④ `extendRow`（`:997-1050`）复制 `cell.isLeftHeadOf(sub)` 的格子 ⇒ 从格逐实例克隆；
  ⑤ `ReportEditor.java:4768-4776` 报表向导对从格带 `setProperty((byte)30)` +
  `CellPosition.toExcelNotation(主格坐标)`——**显式左主格正是产品向导产物**。
  实证推论：默认头规则只认「左近邻纵向扩展/上近邻横向扩展」，**跨行向下的从格带
  不会自动挂头**（T1/T2 初版不嵌套的直接原因）。
- **`ds.current` 管理**：`ExtCellSet.setDSCurrent`（`:582-593`）把 null current 设为
  root group；`DataSet.getCurrentData`（`:384-401`）Group 态按 currentRow（默认 1）
  取行 ⇒ **Root 态裸列 = 首行值**（Rust 现报 UnsupportedExpr，是修错项 M5-06）。
- **`cross()` 维度对齐**（`Group.java:112-165` + `ExtExtendCell.setDSCurrentTopHead`
  `:265-291`）：同行号归并（threshold 达标有序归并，否则位图成员测试）⇒ 交叉表
  同数据集双方向扩展，与多数据集关联是两件事（M5-07 带账）。
- **filtercross 多对多**（`DSFilterCross.java`）：等值/And 树抽列对，右表 count2>6
  走排序 + 二分归并。`Group.filter` 快速路径（`:201-291`）：常量条件 / option `b`
  + 等值树走二分——**前提是行已按该列有序**（新带账）。
- **边界**：`filter`/`filterone` IDE 语义面板有（`JPanelDataSet.java:94-97`）但
  `FunctionLib` 未登记（`:106-121`）⇒ 模板写了落空（新带账）。

### §37.2 金仓真机探针（ProbeCross.java，9 模板 round-trip 全绿）

`report` schema 两张真表：`emp_info` 5 人 + `emp_bonus` 6 行（张伟/赵敏各 2 项 =
多匹配；孙丽 0 项 = 无匹配）。truth 落 `tests/fixtures/out/cross_*_product.json`。

| 模板 | 语义点 | 真值结论 |
|---|---|---|
| T1 `cross_emp_bonus` | 非嵌套从格带（无显式左主格）+ num/去重/降序 | 整带单次求值；Row 态 num/去重/序不生效 |
| T2 `cross_dept_band` | 去重主格 `select(dept:1,,,dept)` + 同/跨数据集从格 | 去重按 collation 序出部门 |
| T3 ×6 隔离模板 | Root 态每变体独占模板 | filter→List 纵扩 / num→单值 / distinct→List 首现序 / desc→全量排序 / bare→首行值 / count→6 |
| T4 `cross_nest` | **显式左主格** `rd.getCell(4,5).setLeftHead("A3")` | 嵌套主子表 14 行网格：多匹配展开（张伟 2 行）/ 无匹配零行（孙丽）/ 从格双列同步扩展 / 主格列空单元格合并 |
| T5 `cross_nest_vals` | 嵌套 + 从格带跟值 | 无匹配仍产 1 行（与 T4 的差异点） |

### §37.3 踩坑记录（本 session 实证）

1. **裸列名在 `ds.select` 过滤里绑定本数据集列**：`ds2.select(item, id==A3)` 的裸
   `id` 绑到 ds2 自身主键（奖金表 id 1..6），症状 = 每主格实例只匹配 1 行且数值
   错位（张伟的年终奖挂到李娜行下）。**关联列必须写 `emp_id==A3`**（11 处全局修正）。
2. **合带共带干扰**：同带多 DS 函数格 current 互相污染——6 个 Root 变体挤一模板，
   首格扩展后其余格看到被改写的 ds.current（c2-c5 全 null、count=1 退化成 Row 态）。
   **修复 = 每变体独占模板**；夹具纪律再钉「一模板一语义点」。
3. **从格带写在主格带下方 ≠ 嵌套**：T1/T2 初版从格带整体只算一次（取主格首实例值）。
   定位 = `ExtCellSet._$2/_$3` 默认头规则不认跨行向下；机制 = 显式 leftHead 字符串
   （§37.1 第④步），`ReportEditor.java:4768` 向导代码实锤。
4. javac 跑 `ProbeCross` 漏 `lib/*`（postgresql-42.7.4.jar 在 fixtures/lib）——
   ClassNotFoundException；按纪律 `common/jre/bin/java.exe` 跑（JDK21 编译 --release 8）。

### §37.4 立项文档 spec-m5 v0.1（docs/spec-m5.md，285 行）

- §2 产品机制定案（2.1 表达式分派 / 2.2 参数矩阵表 / 2.3 双态 / 2.4 显式左主格
  五步 / 2.5 cross() / 2.6 filtercross / 2.7 filter 快速路径）；
- §3 探针结论（9 模板真值，含 T4 网格图）；
- §4 JOIN 三分类 → ReportLite 形态映射（等值连接→嵌套主子表 / 多对多→filtercross /
  维度对齐→cross()）；
- §5 任务表 **M5-01..07**：01 参数矩阵 → 02 跨数据集过滤求值 → 03 显式左主格
  reader+引擎 → 04 夹具闭环（`engine_cross`/`engine_cross_root`）→ 05 filtercross →
  06 Root 裸列修错 → 07 cross() 带账；
- §6 新带账 3 项：filter/filterone 不认、共带干扰、二分有序性前提。

| 证据 | 结果 |
|---|---|
| 反编译源码 | DSSelect/ParamParser/ExtCellSet/Group/DSFilterCross 全读，file:line 齐 |
| 金仓探针 | 9 模板 round-trip 全绿 + truth JSON |
| 立项文档 | spec-m5 v0.1（M5-01..07 + 验收口径） |
| Rust 代码 | 零改动（立项纪律） |

## §38 M5-01 闭合：ds.select 参数矩阵 + 双态求值（2026-10-15 深夜）

**总监令「下一步」。按 spec-m5 §5 任务表从 M5-01 开工（M5-02/03 的前置）。
`DSSelect.calculate`（`DSSelect.java:27-209`，211 行）逐行移植 + 三个协作者
（`Group.filter/distinct`、`SortDsValue.sort`、`ParamInfo2.parse`）落地；
M5-06（Root 态裸列 = 首行值）作为实现前置同批闭合。**

### §38.1 落地清单

| 文件 | 内容 | 产品对应 |
|---|---|---|
| `expr/ast.rs` | `Param` 枚举（Leaf/Node 完整 IParam 树）+ `Call.param_tree: Option<Box<Param>>`（Box 断环，E0072） | `IParam` |
| `expr/call.rs` | `build_param`：`ParamParser._$1:35-64` 层级递进复刻（`\0→;→,→:`，每档问顶层分隔符存在性） | `ParamParser.java` |
| `expr/dsselect/mod.rs` | 共享辅助（`bind_ds`/`eval_bound`/`missing`）+ `ds_select` 主入口（双态分派） | `DSSelect.calculate:146-208` |
| `expr/dsselect/decode.rs` | `SelectArgs` 全字段解码 + `param_info2` + `leaf_or_err` + `as_param_int` | `DSSelect.calculate:36-145` |
| `expr/dsselect/rows.rs` | 定序 `eval_rows_idx` + `;` 多键稳定排序 | `Group.evaluateRows` / `SortDsValue.sort:343-414` |
| `expr/dsfn.rs` | `row_path` 抽公共尾段（select/sum/avg/max/min 的 Row 态共用，含哨兵闸）+ `root_current_data`（M5-06） | `DSSelect.java:148-153` / `DataSet.getCurrentData:395-410` |
| `expr/tests_select.rs` | 14 单测（新建，约 460 行） | 探针 T3 六变体内存复刻 |

### §38.2 三个实现期实证（代码注释均已带 file:line）

1. **`getLeafExpression` 对非叶子节点是抛 `invalidParam`**（`lIllIlIlIIIlIlIl.java:56-60`）
   不是返 null ⇒ `select(item, a:b)` 的过滤段走 `leaf_or_err` 报错（测试第 5 种产品报错）。
2. **Root 态 `getCurrentData` 走 currentRow 光标**（`DataSet.java:395-410`）：`_$5` 初值
   **1**（`:31`）且每次 `setCurrent` 重置为 1（`:362-364`）⇒ Root 态裸列 = **首行值**；
   `DSVariable.calculate` 与 `UnknownSymbol.setDS` 同走此路径（两侧不分叉）⇒
   `select(item:item)` 次序表达式求值为首行 Text ⇒ **paramTypeError**
   （`DSSelect.java:83-87`）。**M5-06 由此闭合**（原报 `UnsupportedExpr` 是错的）。
3. **`;` 多排序先 sort 后 evaluate、`:` 次序先 evaluate 后 sort**（`:161-171`）——
   两条路径刻意保留，合并会改掉 `:` 形态语义（模块文档警告 1）。

### §38.3 踩坑：lexer `;` 被粘进单词（本 session 最大坑，三段排查）

**症状**：`ds2.select(item; emp_id:1, amount:-1)` 报「表达式在 `emp_id` 处无法继续
解析」；三个含 `;` 的单测全败，且报错 token 全是 `;` 后第一个词。

**推演与实证的张力**：按 `infix()` 表（Colon=Link bp4、Comma=bp1）手推
`emp_id:1, amount:-1` 整段**应该**能解析通——怀疑过 `build_param` 切分、`parse_params`
旧路径、`is_cell_id` 截胡，全部排除。**Instrument 一招定案**：临时测试直接打
`lex()` 产物 ⇒ `Word("item;")`、`Word("1;")`——**`;` 被单词扫描粘进了前一个词**。

**根因**：`token.rs::is_delimiter` 不含 `;`。lexer 的 `';'` match arm 只在 `;`
处于 token 起始位置时生效；默认臂「吃到分隔符为止」会把词中间的 `;` 一起吞掉。
产品侧 `;` 从不出现在单个 Expression 内部——`ParamParser` 在**字符串级**先切分
（`ParamParser.java:66-79`），各段才交给 `Expression` 扫描；本仓把参数切分与词法
放在同一趟（lexer 模块文档偏差 1），`;` 必须硬断。

**修法**：`is_delimiter` 加 `;`，登记为第 4 处刻意偏差（token.rs 文档注明：
产品分隔集逐字对齐 + 本偏差理由 + 实证日期）。既有夹具零含 `;` ⇒ 无回归面。

**教训**：报错信息「在 X 处无法继续解析」的 X 是**解析停止处的 token**，不是病因；
当手推全部通过时，直接 instrument 词法产物，别继续在语法层打转。

### §38.4 验证

| 证据 | 结果 |
|---|---|
| `expr::tests_select` | **14/14**（四形态解码 + 6 种产品报错 + T3 六变体真值 + num/z 限量 + 空单果 + `;` 多键 + Row 态四钉子 + 哨符闸 + 指针恢复 + Root 裸列首行值） |
| workspace 全量 | 450+ 测试全绿（rpx-engine lib 276，含新增 14） |
| 门禁 | G1–G8 全绿（G4 触发的 dsselect 拆分：583 → 282/253/93 三文件） |

### §38.5 提交

- `dd126e0` `feat(rpx-engine): M5-01+M5-06 闭合——ds.select 全参数矩阵与双态求值 + Root 态裸列首行值`
  （30 文件：dsselect 三文件 + Param 树 + build_param + 14 单测 + spec-m5 v0.2 + 本 RUNLOG
  + 11 份探针 truth JSON/probe 日志；本提交由宿主环境对暂存区自动落成，内容与本轮
  最终状态逐字一致）。

## §39 M5-02 闭合：跨数据集过滤求值 + 合并区双视图（2026-10-15 深夜）

**总监令「推进m5 m6」第二步。M5-01 闭合后按 spec-m5 §5 依赖序推进 M5-02。
交付两件事：① **电流语义**——过滤式里的 `A3` 解析到 current 同 row 的主格实例
（T1 从格带逐主格实例滤出各自奖金的充要条件）；② **合并区双视图 + 跨页合并
拆分**——M2 遗留带账「跨页合并格拆分」同批清偿（`m2-engine-design.md` §15.5
由 ❌ 改 ✅）。端到端载体 `tests/cross_dataset_compare.rs`：8 夹具（T1 ×1 +
T2 ×1 + T3 Root 变体 ×6）与探针真值逐格逐页零差异。**

### §39.1 落地清单

| 文件 | 内容 | 产品对应 |
|---|---|---|
| `calc/merged_view.rs` | 新（181 行）：合并区双视图——`areas` 扫描去重 / `global_slots` 全局逐槽 / `page_slots` 页内逐槽 / `attach` 双挂 | `MergedArea` + `CalcMergeJson.renderMerged` |
| `calc/merged_view/tests.rs` | 新（234 行）：4 单测（全槽整段跨度 / 标题跨列页裁剪 / 纵向跨行页拆分 / 空页跳过） | `cross_emp_bonus` 16 条真值复刻 |
| `calc/run.rs` | `calculate_with` 接线双视图；`export_grid` 改返 `(CalcGrid, Vec<MergedArea>)`；`export_merged` 删除（内核迁 `merged_view::areas`） | — |
| `arena/calc_grid.rs` | 新增 `merged_pages` 字段；`MergedCell` 文档重写（坐标口径：`CalcGrid` 里全局、emit 产出才页内；删「page 恒 1 且仍然正确」的过期豁免账本） | — |
| `emit.rs` | `merged_section` 退化为纯序列化：有 `pagination` 取 `merged_pages`，否则取 `merged` | `CalcMergeJson` 序列化侧 |
| `emit/tests.rs` | 新钉子 `render_merged_serializes_the_page_local_view`（页内 12 条逐字 + 回落全局 13 条含第 9 槽整段跨度 9） | 真值 `cross_emp_bonus` 16 条同形 |
| `expr/tests_select/` | **目录化拆分**（G4，见 §39.5）：`mod.rs` 493 行 + `cross_current.rs` 65 行（电流语义钉子） | — |
| `expr/tests_eval.rs` | `Fake` 新增 `set_left_head` / `add_ext` 两个 `pub(crate)` 辅助（搭主/从格实例链） | — |
| `tests/cross_dataset_compare.rs` | 新（265 行）：8 夹具端到端对拍 + 内存复刻与 spec 事实互证 | truth `out/cross_*_product.json` |
| `tests/common/compare` | 对拍框架扩展（merged 段逐条比对） | — |
| `rpx-server/src/snapshot/geom.rs` | `merge_data` 文档更新：删脆弱的行号引用，改为「M5-02 起 `merged` 定型为全局逐槽视图」 | — |

### §39.2 核心设计决策：双视图（产品「一份数据两个读者」）

`CalcGrid` 同时挂两份合并区投影，两条消费链各取所需：

| 视图 | 坐标口径 | 消费者 | 为什么必须是这个口径 |
|---|---|---|---|
| `merged`（全局逐槽） | 全局坐标 + **整段**跨度 | `rpx-server` 快照 `geom::merge_data` 的矩形槽集不变式 + `paged::merge_data_paged` 的跨页 deepClone 拆分 | 页 2 的局部坐标若混进来会与页 1 碰撞（页内坐标全从 1 起） |
| `merged_pages`（页内逐槽） | 页局部坐标 + **该页裁剪后**跨度 | emit 对拍 | `CalcMergeJson.renderMerged` 读的是**页格**（探针先 `PageBuilder.createPages()` 再逐页读 `pg.getCell(r,c)` 的 `getRowMerge/getColMerge`） |

**跨页合并拆分的产品机制 → 本仓等价建模**：`SimplePager._$1` 对跨页合并做
拆分——锚点区 `setEndRow`/`setEndCol` 收窄到页末，并 `deepClone` 出延续页新区
（行向 `:1624-1638`、列向 `:2316-2335`）⇒ 后续页格拿到锚点值克隆与**自己的**
裁剪跨度。等价于「合并区矩形 × 页矩形求交」：**交矩形维度**即该页所有相关槽
报的跨度（不是「本槽还剩多宽」——产品页格读的是合并区宽高）。`1×1` 交矩形
被探针跳过（`CalcMergeJson.java:48-50`）⇒ 同跳（`cross_emp_bonus` 第 2 页
标题第 9 槽即此形态：真值 16 条全部落在页 1）。

`/render` 会用模板真值口径覆盖 pagination 但不重算 merged ⇒ 两份都必须在引擎侧
算好挂上，emit 只做选择。

### §39.3 两次失败尝试（记档防复踩）

1. **emit 侧投影**：第一版把投影逻辑写在 `emit.rs`——拿每个全局槽的 `(r,c)+跨度`
   当矩形与页求交，延续槽算出「本槽剩余宽度」8,7,6,5,4,3,2（真值要求全部 8）。
   **病因**：emit 侧没有锚点知识（同一 `CellId` 的槽彼此不可区分），无法知道
   「本槽是锚点还是延续槽」。**修法**：投影移到引擎侧，`areas()` 按 `CellId`
   去重拿锚点，`page_slots` 对锚点矩形求交后**逐槽**发射。
2. **锚点去重单视图**：第二版把 `export_merged` 改成按 `CellId` 去重（每区一条）
   喂给两个读者 ⇒ 单页夹具只剩 1 条/区，对拍报 **11 missing**（真值是逐槽）。
   **修法**：承认「一份扫描、两种投影」才是产品结构，确立双视图（§39.2）。

### §39.4 电流语义钉子（M5-02 的语义核心）

`resolve.rs::resolve_cell` 走 left/top head 链，把过滤式里的 `A3` 解析到
**current 同 row 的主格实例**（`CSVariable.java` 逐行移植）。钉子
`cross_filter_tracks_master_current_row` 的搭建要点（也是本次踩的坑）：

- **宿主实例的左 head 必须指向主格同 row 实例，宿主引擎格的左 head 必须是主格
  引擎格**——`Fake::put` 默认 `lh=ROOT`，首跑缺 `f.set_left_head(host, a3)`
  这条链，`common_left_head` 一路走到 ROOT、`via_left_subs` 返回第一个实例，
  钉子以 `["全勤奖","年终奖"] != ["全勤奖"]` 失败（**搭建错，不是实现错**，
  按实现改代码就会把对的实现改坏）——与真实模板结构一致后才绿。
- 三段断言：张伟行 2 项 → 李娜行 1 项 → **回张伟行又 2 项**（反向判据，排除
  「只看第二次调用」的假绿）。

### §39.5 踩坑：G1/G4 双红（本轮门禁两次才过）

- **G4**：`expr/tests_select.rs` 加钉子后 **540 行 > 500**（M5-01 时 488 行）。
  按仓规拆成目录模块 `tests_select/{mod.rs, cross_current.rs}`（493 + 65），
  第 6 节整节搬迁，共享夹具走 `use super::{root_fake, texts}`。
- **G1**：新钉子写完**忘跑 `cargo fmt --all`**（两处 `assert_eq!` 超宽 + 一处
  续行注释对齐）——与 §38.3 同源的「最后一步省掉」型失误，第二次门禁即绿。
- **G2 clippy**（首轮提交前）：模块文档续行行首 `+ ` 被 rustdoc 当列表项
  （doc list item without indentation ×4）→ 改措辞拆段；`|m| merged_cell_json(m)`
  redundant closure → `map(merged_cell_json)`。

### §39.6 验证

| 证据 | 结果 |
|---|---|
| `merged_view` 4 单测 | 全绿（17 槽整段跨度 / 16 条页内裁剪 / 跨行页 3+2 / 空页跳过） |
| `emit` 15 测试 | 全绿（含新钉子：页内 12 条逐字 + 回落全局 13 条） |
| `expr::tests_select` 15 测试 | 全绿（14 M5-01 + 1 电流语义钉子） |
| `cross_dataset_compare` | 8 夹具逐格逐页零差异（T1 含 16 条 merged 真值） |
| rpx-engine lib | **283** passed（M5-01 时 276：+4 merged_view +1 电流钉子 +2 emit） |
| workspace 全量 | **453** passed，34 个测试二进制零失败 |
| 门禁 | G1–G8 全绿（二轮：首轮 G1/G4 红，修复后复跑） |
| 变异 ① 电流语义 | `resolve_cell` 改 `Some(src)` 静态解析 ⇒ **仅** `cross_filter_tracks_master_current_row` 红，其余 14 个 tests_select 全绿；已还原 |
| 变异 ② 页内投影 | `page_slots` 跨度改整段（不裁剪）⇒ `merged_view` 2 钉子红（17≠16、5≠3）+ 端到端 `extra=1 mismatch=8`；已还原 |

### §39.7 新带账

合并段断在**分组头行**（`rowType == -90`）时产品会把上一页合并区**伸长**盖住
（`SimplePager.java:1550-1596` 的 `rt == -90` 分支）——本仓只做矩形求交，
现有夹具无此形态（跨页合并首例 `cross_emp_bonus` 是标题带）。登记于
`spec-m5.md` §6.3 第 ⑦ 项。

### §39.8 提交

- 见文末 git log（`feat(rpx-engine): M5-02 闭合——跨数据集过滤电流语义 + 合并区
  双视图与跨页拆分`）；含 M4 §36 演示产物三件套补提（`out/kb_demo.cjs`、
  `out/m4-kingbase-demo.png`、`out/report_demo_product.json`——RUNLOG §36.2
  引用但漏提交的遗留）。

## §40 M5-03 闭合：显式 leftHead/topHead 槽位（2026-10-15 深夜）

**总监令「推进m5 m6」第三步。M5-02 提交 `6a7e18a` 后，按 spec-m5 §5 依赖序推进
M5-03。反逆向结论：**机制已在 M2 头图工作实装**，本里程碑只对既有机制做**验收固化**
——把 T4/T5 嵌套主子表夹具并入主对拍载体，并用变异测试证明显式 head 路径承重。
**无独立实现提交**（机制来自 M2），随 spec 升级 v0.3→v0.4 一并进账。**

### §40.1 反逆向结论（slot 30/31 链路全通）

| 环节 | 现状 | 产品对应 |
|---|---|---|
| reader 解析 | `rpx-core` `cell.rs` 已有 `left_head`/`top_head` 字段（slot 30/31 = `_$56/_$55` 单元格 ID 字符串） | `NormalCell._$56/_$55` |
| 头图装配 | `stage_b.rs` 读 `declared_left_head`/`declared_top_head` → `heads/find.rs` 分支 A 显式优先 | `ExtCellSet._$2/_$3:753-810` |
| 跨度递归 | `ext_algo/envelope.rs` `expand_left_head` 把左主格跨度递归纳入左子格 | `ExtCellSet._$4/_$3:944-970` |
| 复制循环 | `ext_algo/row.rs` `extend_row` 按主格实例数克隆从格带 | `extendRow:997-1050` |

→ M5-03 的「reader 解析 + 头图 + 跨度 + 嵌套复制」四件事在 M2 已闭环，无新增代码。

### §40.2 验收夹具固化（T4/T5 并入主对拍）

`cross_dataset_compare.rs` 的 `names` 数组新增两条（现 **10 夹具**：T1×1 + T2×1 +
T3 Root 六变体×6 + T4×1 + T5×1）：

- **T4 `cross_nest`**：多匹配展开 —— 主格 5 人 × 各自奖金行 ⇒ **14 行嵌套网格**；
  从格双列（item/amount）同步扩展；主格列空单元格合并段。
- **T5 `cross_nest_vals`**：无匹配仍产 **1 行**（从格带构成决定无匹配也留壳）；
  与 T4 同为 14 行网格但末段空行语义不同。

结果：`cross_datasets_match_product_truth_cell_by_cell` 与
`memory_replica_matches_spec_facts` 均 **ok**（2 passed）；逐格零差异含 merged 段。

### §40.3 变异测试：证明显式 head 路径承重（防假绿）

**判据**：T4/T5 可能「恰好」被默认近邻规则误判通过（产品默认头规则会就近找同列
主格），必须证明它们走的是**显式** slot 30/31 路径。

**操作**：临时摘除 `heads/find.rs` 分支 A（`if let Some(id) = non_blank(g.declared_*)`
改为 `non_blank(None)`），模拟「slot 30/31 未实现」：

| 夹具 | 正常 | 变异后 | 差异 |
|---|---|---|---|
| T4 `cross_nest` | 14 行 | 9 行 | missing=33 / extra=25 / mismatch=11 |
| T5 `cross_nest_vals` | 14 行 | 9 行 | missing=49 / mismatch=59 |

**结论**：摘除显式 head 后 T4/T5 均塌成 9 行（退化为默认近邻单例），证明它们
确实承重于 slot 30/31 显式左主格路径，不是默认规则的巧合。**还原后全绿。**

对照：`engine_head_left`/`engine_head_top` 只覆盖默认近邻（全 fallback 到 cell00），
引擎级显式 head 的唯一承载就是 T4/T5 —— 这也正是 M5-03 选 T4/T5 作验收夹具的理由。

### §40.4 验证与门禁

| 证据 | 结果 |
|---|---|
| `cross_dataset_compare` | 10 夹具逐格零差异（含 T4/T5 16 条 merged） |
| workspace 全量 | 453 passed（与 M5-02 持平；M5-03 无新代码，仅固化夹具） |
| 门禁 | G1–G8 全绿（`cargo fmt --all` 已跑；无新 .rs 文件故 G4 不触发） |
| 变异 | 摘除分支 A ⇒ T4/T5 14→9 行（已还原） |

### §40.5 新带账 / 开放项

- 无新增带账（M5-03 机制复用 M2，M5-02 的带账⑦仍唯一未复刻项：分组头行合并区
  伸长 `SimplePager.java:1550-1596`）。
- 开放决策：M5-03 未单独成提交（机制来自 M2，仅夹具+文档增量）。若需审计锚点，
  可补一枚 `test` 型提交把 T4/T5 入网钉死；当前已随 spec v0.4 进账，留待 M5-04
  夹具闭环时一并提交更省 commits。

### §40.6 下一步

- **M5-04**：产品夹具闭环 —— 用 `EngineModelsM5.java` + `GenEngineFixtures` 生成
  `engine_cross`（嵌套主子表）+ `engine_cross_root`（Root 变体，一模板一语义点），
  进 19+ 夹具主对拍全绿 + Java 侧 round-trip。这是 M5-05（filtercross）/
  M5-07（cross()）的前置。
