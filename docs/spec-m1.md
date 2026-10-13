# Spec — ReportLite Rust 重实现 · M1（rpx-core 解析器 + 模型层）v1.4

> 生成日期：2026-10-12 · 项目总监（大湾区靓仔）  
> 基于：`re_work/reportlite-rust全链路设计.html`（已确认设计文档）第四节/五节 + 源码级格式事实（本轮新验证）  
> 状态：**已确认（用户指令"开始"）**

---

## 1. 产品定义

- **一句话描述**：用 Rust 重实现润乾 ReportLite 报表全链路，M1 交付 `.rpx` 二进制格式的零依赖解析器与模板态内存模型。
- **目标用户**：后续 M3+ 的 axum 服务与 Univer 前端；M1 的直接消费者是 rpx-cli 与对拍测试。
- **核心问题**：`.rpx` 是带混淆属性码的私有紧凑格式，必须先有字节级正确的解析器，整个重实现链才有地基。

## 2. MVP 范围（锁定——不在此列表的一律不做）

| 优先级 | 功能                                                          | 验收标准摘要                                                                                                                                                         |
| --- | ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0  | `rpx_core::open_bytes(&[u8])` / `rpx_core::open<R: Read>()` | 魔数/长度/页脚校验；payload 完整解析不抛错                                                                                                                                     |
| P0  | 三段式矩阵装载（RowCell / ColCell / NormalCell）                     | 维度、行高列宽、单元格值全部与 Java 侧一致                                                                                                                                       |
| P0  | 合并区装载                                                       | 锚点格 + `-1` 非锚点语义，装载后合并区铺满。⚠ **M1 无夹具覆盖**：三个夹具都是"浅合并"，`-1` 空槽分支未被走到（遗留项①）                                                                                       |
| P0  | 7 个 MetaData + 标量尾巴                                         | PrintSetup/ParamMetaData/DataSetMetaData **完整解析**（full.rpx 的 expected.json 含 print_setup 8 字段/2 参数/1 数据集，必须逐字段一致）；Macro/SubReport/BackGraph/Export 按 null 记录跳过 |
| P0  | 属性码字典 `props.rs`                                            | 语义命名常量表，覆盖 RowCell/ColCell/NormalCell 全码位                                                                                                                      |
| P0  | `rpx-cli print <file.rpx>`                                  | 打印维度/行高列宽/合并区/单元格摘要，供人工核对                                                                                                                                      |
| P0  | 真实 .rpx 夹具 ×3                                               | 由 Java 侧真实 writer 产出（非手搓字节）                                                                                                                                    |

## 3. 明确不做（Out-of-Scope — 锁定）

| 不做的功能                    | 原因                         | 何时考虑                        |
| ------------------------ | -------------------------- | --------------------------- |
| 扩展计算 / 求值 / 分页           | 属 rpx-engine（M2）           | M2                          |
| Java OIS 老格式回退           | 设计文档 §4.3 明确只保现代紧凑格式       | 永不（明确报 `UnsupportedFormat`） |
| 数据库 / axum / Univer / 导出 | M3+                        | 各自里程碑                       |
| 异步 / 外部依赖 crate          | M1 纯同步零依赖，构建最快、对拍最稳        | M3 按需引入                     |
| 子报表递归展开                  | 模型先记录 `Cell::Sub` 占位，不递归解析 | M2                          |

## 4. 技术架构（锁定）

| 项           | 值                                                    | 锁定原因              |
| ----------- | ---------------------------------------------------- | ----------------- |
| 语言/工具链      | Rust 1.98.0（rustc/cargo 本机实测可用）                      | 本机实测，无环境风险        |
| edition     | 2021                                                 | 稳定保守              |
| workspace 根 | `F:/project/report/rpx/`                             | 与 re_work 平级，独立工程 |
| rpx-core 依赖 | **零**（纯 std）                                         | 解析器无必要依赖；构建/审计最简  |
| rpx-cli 依赖  | 仅 rpx-core（path）                                     | 同上                |
| 错误处理        | 手写 `RpxError` enum + `Result` 别名（不引 thiserror，保持零依赖） | 同上                |

### 4.1 Crate 布局（M1 只建前两个）

```
rpx/
├─ Cargo.toml            # workspace: members = ["crates/rpx-core", "crates/rpx-cli"]
├─ crates/
│  ├─ rpx-core/
│  │  ├─ Cargo.toml      # [dependencies] 与 [dev-dependencies] 均为空（零依赖）
│  │  ├─ src/
│  │  │  ├─ lib.rs        # 公开面：open / open_bytes + 类型再导出
│  │  │  ├─ error.rs      # RpxError（不含死变体，见 §4.2）
│  │  │  ├─ reader.rs     # RecordReader（记录层）+ ObjValue 标签字典
│  │  │  ├─ javautf.rs    # 改良 UTF-8 解码
│  │  │  ├─ props.rs      # 紧凑流槽位常量表 + 位标志 + 默认值（含单测）
│  │  │  ├─ rowcell.rs / colcell.rs / cell.rs   # 行/列/普通格（10/8/49 槽）
│  │  │  ├─ cell/cell_defaults.rs   # Cell 位标志访问器与 is_default（拆出，单文件 ≤300 行门禁）
│  │  │  ├─ print_setup.rs # PrintSetup 33 槽（附件 D）
│  │  │  ├─ param.rs      # Param 10 槽（附件 E.1）
│  │  │  ├─ dataset.rs    # DataSetMetaData / FileDataSetConfig（附件 E.3-E.6）
│  │  │  ├─ report.rs     # ReportDefine（模板态根，附件 C.2 二十四步）
│  │  │  └─ report/report_fill.rs # 合并区铺满 + ParamMetaData 展开（拆出）
│  │  └─ tests/
│  │     ├─ golden.rs     # 3 夹具逐字段对拍 + 错误路径 + 文件层截断扫掠
│  │     ├─ robustness.rs # 维度上界回归 + 记录层截断扫掠（v1.3 新增）
│  │     └─ common/{mod,meta_checks,merge_checks}.rs   # 零依赖 JSON + MetaData/合并区对拍
│  └─ rpx-cli/
│     ├─ Cargo.toml
│     └─ src/main.rs      # print 子命令
├─ tests/fixtures/        # 3 个真实 .rpx + expected.json + 生成它们的 Java 源码 + RUNLOG.md
└─ docs/                  # spec-m1.md（本文件）+ format-contract.md（附件 A-E）
```

> v1.0 布局图里的 `meta.rs` 在实现期按「单文件 ≤300 行」门禁拆成 `print_setup.rs` /  
> `param.rs` / `dataset.rs` 三个模块；`cell.rs` 与 `report.rs` 也各自拆出同名目录下的子模块。  
> 子模块文件必须放在同名目录下（`src/cell.rs` ↔ `src/cell/`），否则 E0583。

### 4.2 公开 API 契约（锁定）

```rust
// rpx-core —— 自由函数入口（不存在 RpxReader 类型；v1.0 文档的 RpxReader::open 是笔误）
pub fn open<R: std::io::Read>(r: R) -> Result<ReportDefine>;   // 读到底后转 open_bytes
pub fn open_bytes(buf: &[u8]) -> Result<ReportDefine>;         // 校验文件层后装载 payload
pub type Result<T> = std::result::Result<T, RpxError>;

pub type CellId = usize;                       // 索引进 ReportDefine::store

pub struct MergedArea {                        // 端点均已归一化（附件 E.9）
    pub begin_row: usize, pub begin_col: usize,
    pub end_row: usize,   pub end_col: usize,
    pub cell: CellId,
}

pub struct ReportDefine {
    pub rows: usize, pub cols: usize,                  // 矩阵维度（含 0 号头行/头列）
    pub row_cells: Vec<RowCell>,                       // 索引 = 行号-1
    pub col_cells: Vec<ColCell>,                       // 索引 = 列号-1
    pub store: Vec<Cell>,                              // 单元格存储；合并区各处共享同一 id
    pub grid: Vec<Vec<Option<CellId>>>,                // (rows)×(cols) 模板态网格，None=空槽
    pub merges: Vec<MergedArea>,                       // 合并区清单（锚点+跨度+共享 CellId）
    pub print_setup: PrintSetup,                       // 读回 null 时兜底默认值（同 Java new）
    pub params: Vec<Param>,                            // ParamMetaData 展开（附件 E.1/E.2）
    pub datasets: Vec<DataSetEntry>,                   // DataSetMetaData 展开（附件 E.3-E.6）
    pub custom_properties: Vec<(String, Option<String>)>, // cpSize；null key 不进表（C.2）
    pub exp_map: Option<Vec<(u8, ObjValue)>>,          // _$5（ByteMap 记录）
    pub scalars: ScalarTail,                           // 步骤 7-16 的 10 个标量 + 19-24 尾部
    pub macro_meta: Option<Vec<u8>>,                   // 四个跳过型 MetaData：只保留原始字节
    pub sub_report_meta: Option<Vec<u8>>,              // （记录存在性，字段展开待 M2）
    pub back_graph_config: Option<Vec<u8>>,
    pub export_config: Option<Vec<u8>>,
}
// RowCell / ColCell / Cell / PrintSetup / Param 的公开字段以 format-contract.md 附件 B/D/E 槽位表为准。
```

`RpxError` 分类（v1.3 起**无死变体**）：

| 变体                                 | 触发条件                                  |
| ---------------------------------- | ------------------------------------- |
| `BadLength { declared, file_len }` | 文件头声明的 payload 长度为负                   |
| `Truncated { ctx }`                | 文件不足 8 字节 / payload 实际字节不足 / 记录内字节不足  |
| `BadFooter`                        | 页脚缺失或不是两遍魔数                           |
| `UnsupportedFormat`                | 魔数不命中（含 Java OIS 老格式；AC-01/AC-09 不回退） |
| `Utf8 { detail }`                  | 改良 UTF-8 解码失败                         |
| `PropertyCode { slot, detail }`    | 槽位越界/类型不符；矩阵维度非法；维度上界越界（AC-10）        |
| `Io`                               | 底层 IO 错误                              |

> **v1.3 清理**：`BadMagic([u8;4])` 与 `UnknownTag { tag, ctx }` 作为死变体移除——前者因 AC-01  
> 要求魔数不命中统一返回 `UnsupportedFormat`，后者因契约附件 A.5 末行要求未知标签兜底  
> `readBytes()`。二者当时均已无构造点。纪律：**不保留死变体**，将来确需区分时连构造点一起加回。

## 5. 格式契约（源码级已验证 — 2026-10-12）

### 5.1 文件层（`ReportUtils.java:70-96`，reportlite 版）

```
[0..4)   = 82,81,81,82                     ("RQQR" 魔数，不命中 → UnsupportedFormat，无 OIS 回退)
[4..8)   = int32 大端 payload 长度 N
[8..8+N) = payload = ReportDefine.serialize() 的字节
[8+N..)  = 82,81,81,82,82,81,81,82          (8 字节页脚 = 两遍魔数，不参与长度)
```

读侧同源：`ReportUtils.read` 校验魔数 → 读长度 → `new ReportDefine2().fillRecord(buf)`。

### 5.2 载荷层 = `ReportDefine.fillRecord(byte[])`（reportlite 版，`ReportDefine.java:1129-1198`）

**唯一事实源是 `docs/format-contract.md` 附件 C.2（24 步字段序，已核实）**，摘要如下：

```
1.  int32 r, int32 c                      ← 含 0 号头行/头列；0 号行/列不落盘
2.  row in 1..r-1      : RowCell   记录（readRecord 切片）
3.  col in 1..c-1      : ColCell   记录
4.  row in 1..r-1, col in 1..c-1（行优先）: NormalCell 记录
       空槽 / 合并非锚点 = int32 -1（即 4 字节 FF FF FF FF）
5.  7 个 MetaData：PrintSetup → MacroMetaData → ParamMetaData → DataSetMetaData
       → SubReportMetaData → BackGraphConfig → ExportConfig（PrintSetup 读回 null 兜底 new）
6.  int32 dispRatio(_$18, 默认133)  byte unit(_$17, 1)  String tip(_$15)  byte submit(_$14, 0)
    String notes(_$13)  String futureCellNumExp(_$11)  int32 imageMaxWidth(_$10, 0)
    int32 imageMaxHeight(_$9, 0)  byte pageBorderMode(_$8, 1)  String reportStyleName(_$7)
7.  int32 cpSize + cpSize×{String key, String value}   ← key 为 null 时跳过 value 不读（读侧怪癖）
8.  记录 expMap(_$5, ByteMap 记录；**不是 String**，v1.1 勘误①)
9.  String chartStyleName(_$12)              ← v1.1 勘误②：v1.0 遗漏此字段
10. String validityScript(_$4)  String bigDataSetName(_$3)  String password(_$2)
11. boolean highPrecision(_$16, 默认 true)   ← v1.1 勘误③：布尔量是 _$16 不是 _$1
12. int32 threshold(_$1, 默认 -1)            ← 流到此结束；v1.1 勘误④：无 md5 字段
```

> **编号口径（v1.4 澄清，避免对错槽位）**：上表是**聚合视图**（同类字段并为一行，共 12 项）。  
> 代码里的 `RpxError::PropertyCode { slot, .. }` 用的是 `docs/format-contract.md` **附件 C.2 的  
> 二十四步编号**，两者不可混用：步骤 1=rows、2=cols、**3=RowCell、4=ColCell、5=NormalCell**、  
> 6=7 个 MetaData、7-16=上面第 6 行的 10 个标量（逐个一步）、17=cpSize、18=expMap、  
> 19=chartStyleName、20-22=三个 String、23=highPrecision、24=threshold。  
> 因此报错文案 `property slot 3 error: rowCell record` 指的是附件 C.2 步骤 3，不是上表第 3 项。

**v1.1 勘误（对 v1.0 的 4 处修正 + 1 处结构性修正，依据 format-contract.md 附件 C.1）**：  
① `_$5` 是 ByteMap 记录（int32 长度 + 载荷），不是 String；② 遗漏 `_$12 chartStyleName`；  
③ 布尔量是 `_$16 highPrecision`（默认 true）而非 `_$1`；④ 尾部是 `boolean + int(threshold)`，不存在 `int + String(md5)`；  
⑤ **`ReportDefine.fillRecord` 没有任何 `available()` 守卫**——"探测式尾巴"仅存在于 NormalCell 槽 49（rotationAngle）。  
v1.0 该节其余结论（矩阵三段式、7 MetaData 顺序、10 标量类型序列、cpSize）经核实**正确**。

### 5.3 记录层（`ByteArrayInputRecord`，`esproc/06-common/com/scudata/common/ByteArrayInputRecord.java`）

**唯一事实源是 `docs/format-contract.md` 附件 A**，M1 必守结论：

| 方法                                              | 语义（附件 A 有逐条证据）                                                                                                                                                                                                                                                        |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| readInt/readShort/readLong/readFloat/readDouble | 全大端；readLong = 两个大端 int32 拼接                                                                                                                                                                                                                                          |
| readBoolean                                     | 1 字节，仅 `==1` 为 true；EOF 静默 false（M1 映射 `Truncated`，对拍有差异再回退）                                                                                                                                                                                                          |
| readString                                      | int32 长度（**-1=null**）+ 改良 UTF-8；`\0` = 2 字节 `0xC0 0x80`；补充平面 = 代理对各 3 字节（共 6 字节）；与 Java `DataOutputStream.writeUTF` 编码规则一致                                                                                                                                            |
| readUTF                                         | uint16 长度 + 同体改良 UTF-8；无 null 概念                                                                                                                                                                                                                                      |
| **readRecord(IRecord)**                         | **无 1 字节标签**：int32 长度 + 定长切片；`size<=0` → 空槽返回 null；切片交给记录自己的 fillRecord；**切片内 available() 只算本记录剩余**                                                                                                                                                                   |
| **readObject**（VALUE 槽等）                        | 1 字节标签字典：-1=null / 0=IRecord多态(类名+readRecord) / 1=String短(readUTF) / 13=String长(readString) / 2=BigDecimal(int32 scale+unscaled bytes) / 3,31,32,33=时间(long 毫秒) / 4=Integer / 5=Long / 6=Boolean / 7=BigInteger / 8=byte[] / 9=Double / 10=Float / 11=Byte / 12=Short |
| readStrings/readStringArray                     | 个数前缀是 **int16**（v1.1 勘误：v1.0 误作 int32）；`size<=0` → 整个数组 null，只留 2 字节占位                                                                                                                                                                                                |
| available()                                     | 当前切片剩余字节数；全模型层仅 NormalCell 槽 49 rotationAngle 用它做守卫                                                                                                                                                                                                                   |

**普通字符串单元格 VALUE 形态**（NormalCell.java:1493 写/:1569 读）：字符数 ≤65535 → 标签 `0x01` + uint16 长度 + 改良 UTF-8；>65535 → 标签 `0x0D` + int32 长度 + 改良 UTF-8。

### 5.4 合并区语义（`ReportDefine.java:1145-1160`）

- 非锚点合并格落盘 = `writeRecord(null)` = **4 字节 `FF FF FF FF`**（不是别的魔术值）；
- 锚点格的 `mergedArea` 是 Area 记录：**4×int32 大端，顺序 r1(beginRow), r2(endRow), c1(beginCol), c2(endCol)**（不是 r1,c1,r2,c2，Area.java:126-142）；
- 装载时 Java 把锚点格对象 `setCell` 铺满整个合并区——Rust 装载后合并区网格各处持同一 `CellId`（arena 语义等价）。

**两条后续裁决（实现期补，必读）**：

- **附件 E.8「浅合并」**：若流里非锚点槽是**完整默认记录**（而非 `-1`），装载后该槽持有自己的默认格——这是"只给锚点设 Area、未向矩阵传播"的形态，本批三个夹具即此形态。因此不变式是「锚点槽持合并格；区内其他槽要么持锚点 id，要么是默认格」，二者都与 Java 读侧一致。**真实 `-1` 编码路径至今无夹具覆盖**（遗留项①）。
- **附件 E.9「退化/越界 Area 归一化」**：Area 四端点是流内 int32，可能为负或指向矩阵外。M1 口径：锚点分支先归一化（clamp 进矩阵、保证 end ≥ begin），`fill_merge` 只接受已归一化元组——grid 铺满与 `MergedArea` 记录恒为同一组数，下游按网格消费不会越界。这是 M1 决策（Java 侧对畸形的行为未验证，不冒充）。


### 5.5 模板态模型（M1 范围）

M1 只需**模板态**（未扩展、未计算）：矩阵 + 行高列宽 + 单元格属性（值/对齐/字体/边框/扩展方式/合并/位标志）。  
设计文档 §5 的 arena + `main_of` 主格图属引擎态，M2 实现；M1 用朴素的 `grid: Vec<Vec<Option<CellId>>>` + `store: Vec<Cell>` 即可（字段名与 §4.2 一致，M2 可直接换成 arena 而不动调用方），但**字段命名与 §5 对齐**，避免 M2 推倒重来。

### 5.6 属性码不在流里（v1.1 结构性修正 — format-contract.md 附件 B.0）

**裁决：三类单元格的紧凑序列化是"固定顺序只写值，没有码位字节"**（RowCell.java:350-376/381-402、ColCell.java:295-319/324-343、NormalCell.java:1468-1539/1544-1616 逐槽对齐为证）。所谓"属性码 0-7/10-15/20-94"只是 `getPropertyMap/setPropertyMap` 的 ByteMap key，服务于 IDE/XML 属性映射，**不出现在 .rpx 二进制流里**。

对实现的三条硬约束：

1. `props.rs` 按**紧凑流槽位**组织常量（RowCell 10 槽 / ColCell 8 槽 / NormalCell 49 槽），码位列仅作语义索引；
2. 解析流时不存在"未知码位"字节——AC-05 的 `PropertyCode` variant 实际用途是槽位越界/类型不符的定位上下文与将来 ByteMap expMap 的键枚举；
3. 未知/损坏流靠长度校验与 `available()` 报错，不靠"码位未命中"。

**NormalCell 49 槽的五个必守陷阱**（附件 B.3.2/B.3.3）：

- 槽 4/24/27/30/33/43（6 个 style 字节）：读侧 **80→0 归一化**（NormalCell.java:1688-1693）；
- 槽 13：cellObj 存在标志字节，true 时后接类名 readString + 对象记录（M1 只记录名与原始字节）；
- 槽 36：expMap 是 ByteMap 记录（int16 count + byte key + readObject 值）；
- **槽 40：`writeStrings(null)` 钉死的 2 字节空洞 `00 00`，读侧丢弃**——不错过这 2 字节则全表错位；
- 槽 41：OIS blob（int32 长度 + Java 对象序列化字节），默认（customProps 全 null）恒为 **6 字节 `AC ED 00 05 70 70`**——M1 按长度整段跳过并原样保留；
- 槽 49：rotationAngle float，读侧 `available() > 0` 守卫（新文件必有，老文件容忍）。

## 6. 验收标准（EARS）

| 编号    | 功能          | 验收标准                                                                                                                                                                                                                                                | 优先级 |
| ----- | ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- |
| AC-01 | 魔数校验        | While 文件头 4 字节 ≠ RQQR，`open_bytes` **必须**返回 `UnsupportedFormat` 且不得读取后续字节                                                                                                                                                                           | P0  |
| AC-02 | 长度校验        | While 声明长度超出实际数据，**必须**返回 `Truncated`                                                                                                                                                                                                               | P0  |
| AC-03 | 矩阵装载        | While 提供真实 .rpx 夹具，解析出的 rows/cols/行高/列宽/单元格值 **必须**与 Java 侧 expected.json 完全一致                                                                                                                                                                      | P0  |
| AC-04 | 合并区         | While 夹具含合并区，解析出的 MergedArea 清单与锚点属性 **必须**与 expected.json 一致；按 Java fillRecord 忠实语义（附件 E.8）：锚点槽持合并格，区域内其他槽要么持锚点 id（流里为 -1）要么是默认格（流里为完整记录）。**双向**校验：expected.json 声明的合并区数量/坐标与解析出的 `r.merges` 必须互相覆盖（v1.3 修正——旧版覆盖集合由 `r.merges` 自证，删掉 JSON 声明测试仍绿） | P0  |
| AC-05 | 槽位契约        | While 解析 RowCell/ColCell/NormalCell，每个流序槽 **必须**按 format-contract.md 附件 B 槽位表对位；长度越界/类型不符返回 `PropertyCode(槽位)` 而非静默跳过                                                                                                                               | P0  |
| AC-06 | 标量尾巴        | While 载荷在标量区后提前结束，**必须**返回 `Truncated`（附件 C.2 的二十四步在 `fillRecord` 里**没有任何 `available()` 守卫**——见 v1.1 勘误⑤），不错位、不 panic、不得静默补默认值                                                                                                                      | P0  |
| AC-07 | CLI         | While 执行 `rpx-cli print fixture.rpx`，输出 **必须**含维度/行高列宽/合并区/非空单元格摘要，退出码 0                                                                                                                                                                            | P0  |
| AC-08 | 构建          | cargo build / cargo test / cargo clippy 在 workspace 根 **必须**零 error、零 warning                                                                                                                                                                       | P0  |
| AC-09 | OIS 回退      | While 遇到 Java 序列化老格式（魔数不命中），**必须**明确报错，**不得**尝试 OIS 回退                                                                                                                                                                                              | P0  |
| AC-10 | 维度上界（抗 DoS） | While rows/cols 声明超过 payload 容量（矩阵每槽含空槽至少占 4 字节 int32 长度前缀），**必须**返回 `PropertyCode(1)`；**不得**因未校验维度诱导的**任何**分配（容量预分配、网格、行/列 Vec）而 abort 进程（v1.3 新增：独立验证发现 24 字节文件可将进程 abort，exit 127。实现口径：不按未校验维度预分配，网格按 `max_slots` 有界；实测峰值 ≤ 22×payload 且严格线性）    | P0  |

## 7. 边界与约束

- 只读 `.rpx`，不写（writer 由 Java 夹具程序承担，Rust 侧不实现 serialize）。
- M1 不连任何数据库、不起任何服务。
- 所有测试数据必须来自**真实 Java writer**（reportlite.jar + ReportDefine2 API），禁止手搓字节——手搓数据验证不了格式正确性。
- 数值精度：记录层 BigDecimal（标签 2）先以字符串透传，精度转换 M2 再议（`rust_decimal`）。

## 8. 已知坑（内嵌 — 从 format-contract.md 与源码级验证搬运）

| 坑                                | 根因                                                                         | 修法                                          |
| -------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------- |
| NormalCell 槽 40 钉 2 字节空洞         | serialize 无条件 `writeStrings(null)`（仅写 int16 占位 0x0000）                     | 解析时原样跳过（附件 B.3.2 槽 40）                      |
| OIS blob 默认 6 字节                 | 两个 IByteMap 经 ObjectOutputStream，全 null 时 = STREAM_MAGIC+VERSION+2×TC_NULL | 按 int32 长度整段跳过，保留原字节；非 null 展开待 M2          |
| 6 个 style 字节 80→0                | 读侧 `_$1(byte)` 归一化（NormalCell.java:1688-1693）                              | Rust 同样归一化（槽 4/24/27/30/33/43）              |
| cpSize 的 null key 分支             | 读侧遇 null key 跳过 value 不读；写侧写 4 字节 -1 占位                                    | 读侧同逻辑：null key → 不读 value                   |
| 位打包 short `_$20`                 | 14 个布尔挤在一个 short（默认 32=仅 visible 置位）；bit11/15 保留                           | props.rs 逐位常量（附件 B.3.1）                     |
| readStrings 个数是 int16            | `readShort/writeShort`，不是 int32                                            | 按 int16 解析（附件 A.3）                          |
| `\0` 编 2 字节                      | 改良 UTF-8：0xC0 0x80；补充平面代理对 6 字节                                            | javautf.rs 按附件 A.2.2 实现                     |
| **属性码不在流里**                      | 紧凑流固定顺序只写值；码位仅是 ByteMap key                                                | props.rs 按流序槽位组织（§5.6）                      |
| readRecord 无标签                   | 切片协议：int32 长度 + 定长载荷，size<=0 空槽                                            | reader.rs 按切片实现（§5.3）                       |
| rotationAngle 条件存在               | 仅当记录内 `available() > 0` 时写出/读入                                             | 唯一可缺省槽，缺省 0.0                               |
| 紧凑路径无版本字节                        | 现代格式不带版本号                                                                  | 不做版本协商；老文件直接 `UnsupportedFormat`            |
| **reportlite vs report 产品字段序差异** | reportlite 无 PrintSetup 前的 `byte(_$31)`                                    | 只按 reportlite 序实现（§5.2），引用源码认准 `reportl` 包名 |
| CFR 混淆名 `_$N`                    | 属性码字段名全混淆                                                                  | 语义命名重建，字典以 format-contract.md 为准            |
| readBoolean EOF 静默 false         | Java 怪癖：EOF 返回 false 不抛                                                    | M1 映射 `Truncated`；对拍有差异再回退（待验证项）            |

**工程坑（M1 实测，与格式无关，但都绊过）**：

| 坑                                      | 现象                                                                                                                                           | 处置                                                                                                |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| 本机编译必须设 `LIB`                          | `LNK1181: cannot open input file 'kernel32.lib'`                                                                                             | `export LIB=<MSVC 14.43.34808>/lib/x64;<WinSDK 10.0.26100.0>/um/x64;<同 SDK>/ucrt/x64`（§9 已内联完整命令） |
| cargo 说 `generated 1 warning` 却不是 lint | `error copying object file ... metadata.rmeta ... 拒绝访问 (os error 5)`、`error deleting lock file for incremental compilation ... (os error 5)` | 沙箱文件系统限制。**判据是 `clippy` 的 exit code**：exit 0 即零告警，别当成代码告警去"修"                                     |
| 单文件 ≤300 行门禁                           | 超限必须拆                                                                                                                                        | 子模块文件要放同名目录（`src/cell.rs` ↔ `src/cell/`），否则 E0583                                                 |
| 死变体悄悄堆积                                | pub enum 变体无构造点时 clippy 不报                                                                                                                   | 纪律：无构造点的错误变体一律删除（v1.3 删了 `BadMagic` / `UnknownTag`）                                               |
| 断言容易写成自证                               | `assert_eq!(Some(X), want.get("k").map(\|_\| X))` 恒等于 `X == X`                                                                               | 断言必须让 JSON 侧的值真正参与比较，否则变异测试测不出问题（v1.3 的 `meta_checks` 与 merge 双向都栽在这）                             |
| 页脚后多余字节不报错                             | 文件尾追加垃圾仍解析成功                                                                                                                                 | M1 有意宽容（真实 writer 不会追加；严格化有误杀风险）。M2 若要做完整性校验再收紧，并先评估老文件                                           |

## 9. 端到端验证步骤（锁定）

```bash
# 1. 夹具（Java 侧，本机 JRE）
cd F:/project/report/rpx/tests/fixtures
"F:/project/report/common/jre/bin/java.exe" -cp "F:/project/report/reportlite/lib/*" GenFixtures .
#    → simple.rpx / merged.rpx / full.rpx + expected.json（Java 同时输出预期值）

# 2. 构建与测试（本机须显式设 LIB，否则 MSVC 链接报 LNK1181: cannot open kernel32.lib）
cd F:/project/report/rpx
export LIB='C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.43.34808\lib\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\ucrt\x64'
cargo build
cargo test          # 22 测试：3 黄金夹具逐字段 + 3 错误路径 + 16313 点文件层截断扫掠
                    #          + 16265 点记录层截断扫掠 + 3 健壮性(DoS) + 11 lib 单测 + 1 CLI
cargo clippy --all-targets -- -D warnings

# 3. CLI 实跑（人工核对；⚠ 尚无自动化断言——遗留项③）
cargo run -p rpx-cli -- print tests/fixtures/simple.rpx
cargo run -p rpx-cli -- print tests/fixtures/merged.rpx
cargo run -p rpx-cli -- print tests/fixtures/full.rpx
#    人工比对：维度/行高列宽/合并区/非空单元格与 expected.json 一致，退出码 0

# 4. 健壮性抽验（v1.3 起，可选）
printf '\x52\x51\x51\x52\x00\x00\x00\x08\x7f\xff\xff\xff\x00\x00\x00\x01\x52\x51\x51\x52\x52\x51\x51\x52' > /tmp/dos.rpx
./target/debug/rpx-cli.exe print /tmp/dos.rpx; echo "exit=$?"
#    期望：error: property slot 1 error: matrix dims exceed payload capacity，exit 1（不是 abort/127）
```

## 10. 变更记录

| 日期         | 变更内容                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | 原因                                                  | 影响范围                                                                                   |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------- | -------------------------------------------------------------------------------------- |
| 2026-10-12 | v1.0 创建                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | 用户指令"开始"，进入 M1                                      | 全部                                                                                     |
| 2026-10-12 | v1.1 勘误（架构师 Phase 2 产出 format-contract.md 后合入）：§5.2 尾巴 4 处错误（\_$5=ByteMap 记录/漏 _$12/布尔是 \_$16/尾部无 md5）+ 删除"available() 守卫尾巴"错误陈述；§5.3 readStrings 个数 int32→int16、readRecord 无标签（切片协议）、补 readObject 19 标签字典；§5.4 合并 -1 精确形态与 Area 四 int32 序；新增 §5.6 属性码不在流里的结构性裁决；§8 坑表扩充至 13 条                                                                                                                                                                                           | 源码级核实发现 v1.0 与字节事实不符                                | §5.2-5.6、§8、AC-05                                                                      |
| 2026-10-12 | v1.2（M1 实现完成）：AC-04 按 Java fillRecord 忠实语义修订（附件 E.8）；format-contract.md 增补附件 D（PrintSetup 33 槽）与附件 E（Param/DataSet/FileDataSetConfig 记录格式、factory_class 类名映射、RowCell groupHeaderLevel 槽位固化、FileDataSetConfig 基类 blob 包裹）；实现交付记录见下                                                                                                                                                                                                                                        | 实现期黄金测试发现 AC-04 原断言与 Java 语义不符；三类 MetaData 格式原契约未覆盖 | AC-04、docs/format-contract.md                                                          |
| 2026-10-12 | v1.3（独立验证与修复）：新增 AC-10 维度上界（抗 DoS）；AC-04 补「双向校验」要求；§9 补 `LIB` 环境变量；按 QA 报告修复 1 个进程中止缺陷 + 6 处测试/契约缺口（详见下节）                                                                                                                                                                                                                                                                                                                                                                | 用户要求「分析验证有什么问题」，派出独立验证方复验                           | report.rs / reader.rs / report_fill.rs / props.rs / golden.rs / tests(+, 3 个文件) / docs |
| 2026-10-12 | v1.4（文档自查修正，用户指令「根据这个检测反思及修正」）：修正 §2 入口名（`RpxReader::open` → `open`/`open_bytes`）；§4.1 布局图补齐实现期拆分（`meta.rs`→`print_setup.rs`/`param.rs`/`dataset.rs` + 两个子模块 + 3 个测试文件）；§4.2 API 契约按代码重写（`cells`→`grid`、`params: Vec<Param>`、`datasets: Vec<DataSetEntry>`、补 `store`/`custom_properties`/`exp_map`/4 个 blob）；**AC-06 与 v1.1 勘误⑤的矛盾改为"返回 Truncated"**；§5.2 补「聚合视图 vs 附件 C.2 二十四步」编号口径；§5.4 补 E.8/E.9 两条后续裁决；§8 新增「工程坑」表；§9 标注 CLI 无自动化断言 + 补健壮性抽验命令；代码侧同步删除死变体 `BadMagic` | 文档自查发现 8 处与代码/自洽性冲突，其中 AC-06 与本文档自己的勘误直接矛盾，会误导 M2   | §2/§4.1/§4.2/§5.2/§5.4/§6/§8/§9/§10、error.rs                                           |

### M1 交付记录（2026-10-12，v1.2）

- 实现：`crates/rpx-core`（13 个模块，零依赖纯 std）+ `crates/rpx-cli`；单文件均 ≤300 行。
- 验收：`cargo test` 13 测试全绿（3 黄金夹具逐字段对拍 + 3 错误路径 + 1 全前缀截断扫掠  
  （三夹具共约 16000 个截断点，无一 panic）+ 5 个改良 UTF-8 单测 + 1 CLI 格式单测）；  
  `cargo clippy --all-targets -- -D warnings` 零告警；CLI 对 merged/full 实跑输出与  
  expected.json 逐字段一致。
- 实现期发现并固化（附件 E.7/E.8）：RowCell 槽 7 groupHeaderLevel、FileDataSetConfig  
  基类 blob 包裹、合并区"锚点铺满+后续覆盖"语义、factory_class 类名常量映射。
- 说明：原定后端工程师实现，因其连续两次遇 API 429 限流（瞬时基础设施故障），  
  按专家团异常处理条款由项目总监接管实现；契约/夹具/测试三要素均为团队成员前置产出。

### M1 独立验证与修复记录（2026-10-12，v1.3）

用户要求「分析验证有什么问题」后，按专家团规程派出**未参与实现**的独立验证方（QA）复验；  
其在 `target/verify-scratch/` 内做变异测试与对抗构造，未改动 `crates/` 与 `tests/fixtures/`。  
共发现 1 个进程中止缺陷 + 6 处测试/契约缺口，全部已修复并留下回归测试。

| # | 严重度    | 问题（复验证据）                                                                                           | 修复                                                                                                                                                                        |           |                                                                                                           |                                                                                                                                    |
| - | ------ | -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- | --------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| 1 | **严重** | 24 字节文件 \`RQQR                                                                                     | len=8                                                                                                                                                                     | INT_MAX,1 | footer`让`open_bytes`/CLI 以 `memory allocation of 188978560848 bytes failed\` **abort（exit 127）**，而非返回 Err | `report.rs` 加 `max_slots = payload.len()/4 + 1` 上界返回 `PropertyCode(1)`；新增 `tests/robustness.rs::rejects_absurd_matrix_dims`（6 组维度） |
| 2 | **严重** | AC-04 单向自证：`covered` 由 `r.merges` 构建、锚点 `merged` 是可选断言 → 删光 JSON 三个合并声明后 7 个 golden 仍全绿（变异 M3 存活）  | 新增 `tests/common/merge_checks.rs` 双向对拍（数量 + 坐标 + 锚点 CellId），`covered` 改由 JSON 声明构建；已用变异实验证实 M3 现在被杀（`merge count mismatch — expected.json declares 0, parser produced 3`） |           |                                                                                                           |                                                                                                                                    |
| 3 | 一般     | 截断扫掠 16313 点全部止步文件层长度/页脚守卫，记录层零覆盖                                                                  | 新增 `robustness.rs::record_layer_truncation_never_panics`：直接对 payload 每个前缀调 `ReportDefine::fill`（`catch_unwind` 捕获 panic），共 16265 点                                        |           |                                                                                                           |                                                                                                                                    |
| 4 | 一般     | `normalize_style` 的 80→0 归一化无任何测试（改成恒等后 golden 全绿）                                                 | `props.rs` 加 3 个单元测试（归一化 / 默认位标志 / 槽位数）                                                                                                                                   |           |                                                                                                           |                                                                                                                                    |
| 5 | 一般     | 契约偏差：readObject 未知标签返回 `Err(UnknownTag)`，而契约附件 A.5 末行要求兜底 `readBytes()`（比 Java 严格，会拒掉 Java 能接受的文件） | `reader.rs` 补兜底分支（int32 长度 + 字节，保持流对齐）+ 3 个定向单测                                                                                                                           |           |                                                                                                           |                                                                                                                                    |
| 6 | 一般     | `meta_checks.rs:25` 的 ds name 断言是 `X == X` 同义反复，未校验取值                                              | 改为 `class_name` 非空断言 + `factory_class_for(class_name) == factory_class` 映射一致断言                                                                                            |           |                                                                                                           |                                                                                                                                    |
| 7 | 建议     | `MergedArea` 端点用 `as usize` 未校验：负 `end_row` 会产生 1.8e19 级索引，下游按网格消费必越界 panic                        | 锚点分支归一化端点（clamp 进矩阵且 end ≥ begin），`fill_merge` 改收已归一化元组                                                                                                                   |           |                                                                                                           |                                                                                                                                    |

**仍存缺口（未修，建议 M2 前处理）**：真实「非锚点 = int32 -1」编码路径无夹具覆盖（三夹具  
均为"浅合并"，非锚点槽是完整默认记录，`report.rs` 的空槽 `else continue` 分支未被走到）；  
NormalCell 大量槽位仅以默认值出现（槽位互换类缺陷不可检出）；AC-07 CLI 输出无自动化断言  
（仅手动实跑比对）；readBoolean EOF 与 Java 静默 false 的一致性未验证。

**验证边界（不冒充已验证）**：未做产品设计师真实合并文件的端到端对拍；夹具可重现性已由  
验证方在 scratch 用产品自带 writer 重新生成并 `cmp` 逐字节一致（六个文件），产品侧  
`GenFixtures --verify` PASS ×3。

**复验结论（同日，同一独立验证方复跑）**：上轮 4 项「已确认」问题**全部确认修复**；夹具变异  
由 23/24 提升到 **24/24 全杀**（含 M3）；8 组维度实跑 CLI 全部 Err + exit 1，**无一 abort**；  
新 `robustness.rs` 补齐记录层截断覆盖（16265 前缀无一 panic）；22 测试冷跑全绿、clippy exit 0。

独立内存上界推导（验证方自建计数全局分配器实测）：修复后峰值单次分配 ≈ **22 × payload 长度**  
且**严格线性**——攻击者要耗尽 M 字节内存需提供约 M/22 大小的文件（旧版为 24 字节 → 176 GB）。  
据此追加两项收尾：① `row_cells`/`col_cells` 去掉 `with_capacity` 预分配，消掉那 22×L 尖峰  
（"顶格 rows + 无记录"的输入不再触发任何预分配，首条记录读失败即 Err）；② 移除因契约兜底  
而失去全部构造点的 `RpxError::UnknownTag` 死变体。

复验方放行结论：**M1 可放行 M2**；遗留项均为「测试/夹具覆盖」而非功能缺陷，纳入 M2 回归清单：  
① 真实「-1 非锚点」合并夹具；② NormalCell 大量槽位仅以默认值出现；③ AC-07 CLI 无自动化断言；  
④ readObject 多个标签分支（5/10/11/12/2/7/8/3/31/32/33/0/13）无夹具覆盖；  
⑤ readBoolean EOF 与 Java 静默 false 的一致性未验证。
