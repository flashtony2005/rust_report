# Format Contract — .rpx 字节级格式契约（附件 A/B/C）

> 版本：v1.0 · 2026-10-12 · 首席架构师（高见远）
> 上游契约：`docs/spec-m1.md` v1.0（Phase 1 已锁定）
> 证据基线：反编译源码 `F:/project/report/re_work/src/`（CFR 0.152），**只认 `com.raqsoft.reportl.*`（reportlite 产品本体）**
> 证据格式：`文件路径:行号`。凡本文件结论，均可回溯到该行；未核实项显式标注"待后端实现时验证"。

---

## 0. 总则与阅读顺序

```
文件层（ReportUtils.write）
  [RQQR 魔数 4B][int32 BE payload 长度 N][payload N B][页脚 8B]
        ↓ payload 即 ReportDefine.serialize()
载荷层 = ReportDefine.fillRecord(byte[])          ← 附件 C
  内含三类记录，均为 readRecord(IRecord) 切片     ← 附件 A（记录层）
    RowCell / ColCell / NormalCell                ← 附件 B（属性码字典）
```

**三条最高优先级事实**（后文均有证据）：

1. 记录层是"**int32 长度 + 定长载荷**"切片协议，`size <= 0` 即空槽（附件 A.4）。
2. 三类单元格的紧凑序列化是"**固定顺序只写值，没有码位字节**"；所谓"属性码"是 `getPropertyMap/setPropertyMap` 的 byte key，服务于 XML 等外部映射，**不出现在 .rpx 二进制流里**（附件 B.0）。
3. Spec §5.2 的尾巴字段清单与源码有 4 处出入，本文件附件 C.1 给出勘误与正确顺序。

---

## 附件 A：记录层编解码契约（ByteArrayInputRecord）

证据文件：`esproc/06-common/com/scudata/common/ByteArrayInputRecord.java`（读侧）、同目录 `ByteArrayOutputRecord.java`（写侧，用于交叉验证编码规则）。

### A.1 字节序原语

| 方法 | 语义 | 证据（读侧） | 证据（写侧） |
|---|---|---|---|
| `readInt` | 大端 int32：`(ch1<<24)+(ch2<<16)+(ch3<<8)+ch4` | ByteArrayInputRecord.java:80-89 | ByteArrayOutputRecord.java:92-97 |
| `readShort` | 大端 int16：`(ch1<<8)+ch2` | ByteArrayInputRecord.java:62-69 | ByteArrayOutputRecord.java:87-90 |
| `readUnsignedShort` | 大端 uint16（同上，不转 short） | ByteArrayInputRecord.java:71-78 | — |
| `readLong` | 大端 int64：两个大端 int32 拼接 `(hi<<32)+(lo & 0xFFFFFFFF)` | ByteArrayInputRecord.java:95-97 | ByteArrayOutputRecord.java:103-112 |
| `readFloat` | `intBitsToFloat(readInt())` | ByteArrayInputRecord.java:91-93 | ByteArrayOutputRecord.java:99-101 |
| `readDouble` | `longBitsToDouble(readLong())` | ByteArrayInputRecord.java:99-101 | ByteArrayOutputRecord.java:114-116 |
| `readBoolean` | 1 字节，`ch == 1` 为 true；**其余一切值（含 EOF 的 -1）均为 false，不抛异常** | ByteArrayInputRecord.java:25-28 | ByteArrayOutputRecord.java:20-26（true→1，false→0） |
| `readByte` | 1 字节有符号；EOF 抛 `EOFException` | ByteArrayInputRecord.java:30-36 | ByteArrayOutputRecord.java:28-35 |

**Rust 实现要点**

- 全部大端，与 Rust `from_be_bytes` 一一对应；`readLong` 的 `(hi<<32)+(lo & 0xFFFFFFFFL)` 就是标准大端 64 位。
- `readBoolean` 的 EOF 静默 false 是 Java 怪癖。M1 建议：记录内 `read_boolean` 在无剩余字节时返回 `Truncated`（走 AC-02），**若对拍出现差异再回退为静默 false**——标注：待后端实现时验证（正常夹具不会触发）。
- `readByte`/`readShort`/`readInt` 字节不足一律抛 EOF → Rust 统一映射 `Truncated`。

### A.2 字符串：readString / readUTF 与改良 UTF-8

#### A.2.1 长度前缀与 null 语义

| | `readString` | `readUTF` |
|---|---|---|
| 长度前缀 | int32 有符号（`readInt`） | uint16（`readUnsignedShort`） |
| null 表示 | 长度 `-1` → 返回 null | 无 null 概念（长度 0 = 空串） |
| 长度上限 | 2^31-1（实际不会超） | 65535，写侧超限抛 `UTFDataFormatException` |
| 证据 | ByteArrayInputRecord.java:103-111；写侧 ByteArrayOutputRecord.java:118-135 | ByteArrayInputRecord.java:201-206；写侧 ByteArrayOutputRecord.java:155-173 |

#### A.2.2 改良 UTF-8 编解码规则（读写侧逐条对齐）

写侧编码（ByteArrayOutputRecord.java:129-150，`writeString`/`writeUTF` 同一套分支）：

| 字符 c（Java char，UTF-16 码元） | 编码字节数 | 字节序列 |
|---|---|---|
| `0x0001 – 0x007F` | 1 | `c` |
| `0x0000`（`\0`） | **2** | `0xC0, 0x80` |
| `0x0080 – 0x07FF` | 2 | `0xC0\|(c>>6)`, `0x80\|(c&0x3F)` |
| `0x0800 – 0xFFFF`（含代理对一半） | 3 | `0xE0\|(c>>12)`, `0x80\|((c>>6)&0x3F)`, `0x80\|(c&0x3F)` |

长度计算同规则（ByteArrayOutputRecord.java:131、165）。

**对任务书中"`\0` 三字节"说法的更正**：`\0` 是 **2 字节 `0xC0 0x80`**，不是三字节。证据：写侧长度分支 `c >= '\u0001' && c <= '\u007f' ? 1 : (c > '\u07ff' ? 3 : 2)`，`c == 0` 落入 else 得 2（ByteArrayOutputRecord.java:131）；编码分支同理（ByteArrayOutputRecord.java:139-148）。

**补充平面（> U+FFFF）的编码方式**：Java `char` 是 UTF-16 码元，补充平面字符在 Java 字符串里是一对代理（surrogate pair），每个代理值域 `0xD800-0xDFFF > 0x07FF`，各自编 **3 字节**，合计 **6 字节**。即：与 Java `DataOutputStream.writeUTF` 完全相同，**不存在**"补充平面单字符 3 字节"的情况。证据：ByteArrayOutputRecord.java:141-144（>0x07FF → 3 字节）+ 129-133（按码元独立计算，不组成码点）。

读侧解码（ByteArrayInputRecord.java:112-155，`readString`；207-250，`readUTF`，逻辑逐行相同）：

| 首字节 `c & 0xFF` | 动作 |
|---|---|
| `c >> 4 ∈ 0..7`（0x00-0x7F） | 1 字节 → char c。**注意：接受单字节 `0x00`**（写侧不产此形态，属解码器超集） |
| `c >> 4 ∈ 12,13`（0xC0-0xDF） | 2 字节：`(c&0x1F)<<6 \| (b2&0x3F)`；续字节 `(b2&0xC0) != 0x80` 抛 `UTFDataFormatException`。`0xC0 0x80` → `'\0'` |
| `c >> 4 = 14`（0xE0-0xEF） | 3 字节：`(c&0xF)<<12 \| (b2&0x3F)<<6 \| (b3&0x3F)`；两个续字节都须为 `10xxxxxx` |
| 其余（0x80-0xBF、0xF0-0xFF） | 抛 `UTFDataFormatException` |

**与 Java `DataOutputStream.writeUTF` 的异同总结**

- 相同：`\0` → `0xC0 0x80`；1/2/3 字节分界完全一致；代理对 6 字节；`readUTF` 的 uint16 长度前缀与 `writeUTF` 逐字节兼容。
- 不同（仅长度前缀层）：`readString` 用 int32 且以 `-1` 作 null 哨兵（`writeUTF` 无 null）；`readUTF` 即标准 Java 版。
- 解码宽容度：本解码器与 Java `DataInputStream.readUTF` 一样接受单字节 `0x00`，比严格修正 UTF-8 宽松。

#### A.2.3 readUTF2 的潜在缺陷（记录在案，M1 不用）

`readUTF2` 长度用 `readShort`（有符号，ByteArrayInputRecord.java:504），而 `readUTF` 用 `readUnsignedShort`（:202）。若 utflen ∈ [32768, 65535]，`readUTF2` 得负长度 → Java 侧 `NegativeArraySizeException`。M1 只用 `readUTF` 语义，不受影响；列为已知坑。

### A.3 readStrings / readStringArray 家族怪癖（null 占位）

| 方法 | 个数前缀 | null / 空语义 | 证据 |
|---|---|---|---|
| `readStrings` | **int16**（`readShort`） | `size <= 0` → 返回 null（**整个数组** null） | ByteArrayInputRecord.java:159-171 |
| `readStringArray` | int16（`readShort`） | 同上 | ByteArrayInputRecord.java:173-185 |
| 元素读取 | 逐个 `readString()`（int32 + -1=null） | 元素可为 null（占 int32 的 -1，4 字节） | ByteArrayInputRecord.java:167、181 |
| `writeStrings(null)` | 写 `writeShort(0)` | **null 数组只留 2 字节 `0x0000` 占位，不写任何元素** | ByteArrayOutputRecord.java:59-71 |

**关键坑（Spec §8 已预警，此处固化）**：`NormalCell.serialize()` 第 40 槽无条件调用 `out.writeStrings(null)`（NormalCell.java:1519），读侧 `fillRecord` 第 40 槽无条件 `in.readStrings()` 并丢弃结果（NormalCell.java:1595）。即**每个 NormalCell 记录中部都钉了一个 2 字节空洞 `00 00`**。Rust 解析必须原样跳过这 2 字节，否则全表错位。

**对 Spec §5.3 的勘误**：Spec §5.3 表写 "readStrings | int32 个数"——错误，实为 **int16**（`readShort`/`writeShort`，证据如上）。

### A.4 readRecord(IRecord)：空槽与切片协议

```java
// ByteArrayInputRecord.java:254-263
int size = this.readInt();      // int32 大端长度
if (size <= 0) return null;     // 空槽：null 记录
byte[] s = new byte[size];
this.readFully(s);              // 读满 size 字节
r.fillRecord(s);                // 交给记录自己的 fillRecord 解析
return r;
```

| 事实 | 证据 |
|---|---|
| `readRecord` **没有** 1 字节标签；标签机制在 `readObject`（见 A.5） | ByteArrayInputRecord.java:254-263 |
| 空槽表示：`size <= 0`（含 `-1` 与 `0`）→ 返回 null，不调用 fillRecord | ByteArrayInputRecord.java:256-258 |
| 写侧 `writeRecord(null)` → `writeInt(-1)`；`writeRecord(r)` → `writeBytes(r.serialize())` = int32 长度 + 字节 | ByteArrayOutputRecord.java:193-200 |
| 每个记录拿到的 `buf` 是**独立切片**；因此 `fillRecord` 内的 `available()` 是"本记录剩余字节"，不是全文件剩余 | ByteArrayInputRecord.java:259-261 + :623-625 |
| `readRecord2` 用 `in.read(s)` 而非 `readFully`（ByteArrayInputStream 下等价，仅记录在案） | ByteArrayInputRecord.java:558-570 |

**合并区 -1 语义的精确形态**（Spec §5.4）：`ReportDefine.serialize` 对非锚点合并格写 `writeRecord(null)` → 流里就是 **4 字节 `FF FF FF FF`**（int32 -1），不是别的魔术值。证据：ReportDefine.java:1074 + ByteArrayOutputRecord.java:194-195。

### A.5 readObject：1 字节标签字典（VALUE 槽等）

标签=1 字节（`readByte`），读侧 ByteArrayInputRecord.java:265-343，写侧 ByteArrayOutputRecord.java:202-266，两侧逐分支对齐：

| 标签值 | 走向 | 后续字节 | 读侧证据 | 写侧证据 |
|---|---|---|---|---|
| `-1` (0xFF) | null | 无 | :268-270 | :203-204 |
| `0` | IRecord 多态 | `readString()` 类名 → `Class.forName`（`com.raqsoft.` 失败回退 `com.scudata.`）→ `readRecord(rec)` | :271-290 | :205-208 |
| `1` | String（短） | `readUTF()`（uint16 + 改良 UTF-8） | :291-293 | :216-217 |
| `2` | BigDecimal | int32 scale + `readBytes()`（int32 长度 + 大端二进制补码） | :294-298 | :219-223 |
| `3` | java.util.Date | long（毫秒） | :308-310 | :233-235 |
| `4` | Integer | int32 | :311-313 | :236-238 |
| `5` | Long | int64 | :314-316 | :239-241 |
| `6` | Boolean | 1 字节（1=true） | :317-319 | :242-244 |
| `7` | BigInteger | `readBytes()` | :320-322 | :245-247 |
| `8` | byte[] | `readBytes()` | :323-325 | :248-250 |
| `9` | Double | double | :326-328 | :251-253 |
| `10` | Float | float | :329-331 | :254-256 |
| `11` | Byte | 1 字节 | :332-334 | :257-259 |
| `12` | Short | int16 | :335-337 | :260-262 |
| `13` | String（长） | `readString()`（int32 + 改良 UTF-8） | :338-340 | :213-214 |
| `31` (0x1F) | java.sql.Timestamp | long | :299-301 | :224-226 |
| `32` (0x20) | java.sql.Time | long | :302-304 | :227-229 |
| `33` (0x21) | java.sql.Date | long | :305-307 | :230-232 |
| 其他 | 兜底 `readBytes()` | int32 长度 + 字节 | :342 | :263-265 |

注意分支顺序错位（31/32/33 在 3 之前）只是 CFR 反编译的 case 重排，语义无影响。

> **M1 实现状态（v1.3，2026-10-12）**：标签遍历已按本表全部实现，**含末行「其他 → 兜底
> `readBytes()`」**（int32 长度 + 字节，保持流对齐）。独立验证曾指出原实现改为
> `Err(UnknownTag)`，比本源严格、会拒掉 Java 能接受的文件 —— 已按本源修正为兜底，
> 并加 3 个定向单测（未知标签→Bytes、非正长度→空且不越界、声明超长→Truncated）。
> 取舍记录：兜底是宽松策略，未知标签不再显式报错；若后续实测发现 Java 侧对某标签另有
> 语义，应回到本源补具体分支，而不是恢复严格拒绝。

**readObject 与 readRecord 的关系**：`readObject` 是带类型标签的通用值读取；标签 `0` 分支内部转调 `readRecord(rec)` 读 IRecord 子对象——两者通过标签 0 衔接。M1 只用到：标签 `-1`（null）、`1`/`13`（String）、`4`/`5`/`9`/`10`/`6`/`11`/`12`（数值）、`2`（BigDecimal，按 Spec §7 先透传原文）、`31/32/33/3`（时间，透传 long）、`8`（bytes）。

**M1 的 VALUE 槽实际形态**（NormalCell.java:1493 写 / :1569 读）：字符串值 ≤65535 字符走标签 `1`+`writeUTF`；>65535 走标签 `13`+`writeString`。即：**普通字符串单元格 = `01` + uint16 长度 + 改良 UTF-8**。

### A.6 available() 与探测式尾巴

- `available()` = 底层 `ByteArrayInputStream.available()` = **当前记录切片剩余字节数**（ByteArrayInputRecord.java:623-625）。
- reportlite 模型层的探测式守卫共两处（均已核实）：
  1. `NormalCell.fillRecord` 末槽 `rotationAngle`——`if (in.available() > 0) { _$46 = readFloat(); }`（NormalCell.java:1613-1615）。写侧**无条件**写出该 float（NormalCell.java:1537）。即：新文件必有这 4 字节；老文件没有，读侧容忍。
  2. `PrintSetup.fillRecord` 尾部三槽——`fullScreenPreview`、`blankLineFilled`+`columnMode` 各自 `available() > 0` 守卫（PrintSetup.java:1098-1104）。写侧同样无条件写出（PrintSetup.java:1059-1061）。槽位表见附件 C.6。
- `ReportDefine.fillRecord` **没有任何 available() 守卫**（ReportDefine.java:1129-1199 全文无 `available`）——Spec §5.2"尾巴字段 available() 守卫"的说法不成立，详见附件 C.1 勘误。注意其子记录 PrintSetup 自身有尾槽守卫（C.6），不要与载荷层混淆。
- `readXxx2` 全家（ByteArrayInputRecord.java:345-621）是"available()==0 即抛 EOFException"的守卫变体，reportlite 模型层未使用，M1 不实现。

**Rust 实现要点**：`RecordReader` 必须暴露 `remaining()`（当前切片未读字节数），且该计数在进入子记录时重置为子切片长度。AC-06 的"逐字段缺省"只对 `rotationAngle` 一槽成立；其余槽字节不足一律 `Truncated`。

---

## 附件 B：属性码字典（RowCell / ColCell / NormalCell）

### B.0 编码方式裁决（先读此节）

**结论：属性码在 .rpx 序列化流里不以"码位+值"成对出现，而是按固定顺序只写值。**

证据：三个类的 `serialize()`/`fillRecord()` 全是定序裸值序列化，无任何码位字节——
RowCell.java:350-376（写）/ :381-402（读）；ColCell.java:295-319 / :324-343；NormalCell.java:1468-1539 / :1544-1616。

"属性码地址"（RowCell 0-7、ColCell 10-15、NormalCell 20-94）的真实身份是 `getPropertyMap()`/`setPropertyMap()` 的 **ByteMap key**，服务于 IDE/XML 属性映射与属性表：
- RowCell.java:201-212（getPropertyMap）/ :214-285（setPropertyMap）
- ColCell.java:177-186 / :188-245
- NormalCell.java:645-707 / :709-1178

**对 M1 的三条推论**：
1. `props.rs` 的常量表按**紧凑流槽位**组织（B.1/B.2/B.3 的"流序"列），码位列仅作语义索引保留。
2. 解析时**不存在"未知码位"字节**——AC-05 的 `PropertyCode(u16)` 错误实际用途是：紧凑流读越界/类型不符时的定位上下文，以及将来解析 ByteMap expMap 时的键枚举。建议保留该错误 variant，触发条件按槽位边界重定义。
3. 遇到未知/损坏流时，靠 `available()`/长度校验失败报错，而非"码位未命中"。

### B.1 RowCell 字典（属性码 0-7）

getPropertyMap 键（RowCell.java:202-211）：

| 属性码 | 混淆字段 | 语义名 | 类型 | 默认值 | getter 证据 |
|---|---|---|---|---|---|
| 0 | `_$11` | rowType | byte | -91 (0xA5) | RowCell.java:85-87, 203 |
| 1 | `_$10` | rowHeight | float | 8.0f | RowCell.java:93-95, 204 |
| 2 | `_$9` | rowVisible | boolean | true | RowCell.java:115-117, 205 |
| 3 | `_$8` | breakPage | boolean | false | RowCell.java:137-139, 206 |
| 4 | `_$5` | notes | String | null | RowCell.java:302-304, 207 |
| 5 | `_$4` | groupHeaderLevel | byte | 1 | RowCell.java:181-183, 208 |
| 6 | `_$7` | breakColumn | boolean | false | RowCell.java:157-159, 209 |
| 7 | `_$1` | adjustRowH | boolean | false | RowCell.java:59-61, 210 |

字段声明默认值证据：RowCell.java:26-38。`setPropertyMap` 的 null 重置默认值：RowCell.java:220-283（注意 key 5 的 null 重置为 0，与字段默认 1 不同，:281）。

**RowCell 紧凑流槽位表**（serialize RowCell.java:350-376；fillRecord :381-402，逐槽对齐）：

| 流序 | 内容 | 字节 | 证据（写/读） |
|---|---|---|---|
| 1 | rowType | 1 | :356 / :383 |
| 2 | rowHeight | 4（float BE） | :357 / :384 |
| 3 | rowVisible | 1 | :358 / :385 |
| 4 | breakPage | 1 | :359 / :386 |
| 5 | expMap（ByteMap 记录；null → int32 -1） | 4 或 4+N | :360 / :387 |
| 6 | notes（readString） | 4+N 或 4 | :361 / :388 |
| 7 | groupHeaderLevel | 1 | :362 / :389 |
| 8 | **OIS  blob**：`writeBytes(ObjectOutputStream(_$3 customPropsMap, _$2 customPropsExpMap))` | 4+N | :363-372 / :390-399 |
| 9 | breakColumn | 1 | :373 / :400 |
| 10 | adjustRowH | 1 | :374 / :401 |

RowCell expMap（`_$6`）的 ByteMap 键：1=rowHeightExp、2=rowVisibleExp、3=breakPageExp、6=breakColumnExp、7=adjustRowHExp（RowCell.java:101,123,145,167,78）。

### B.2 ColCell 字典（属性码 10-15）

getPropertyMap 键（ColCell.java:179-184）：

| 属性码 | 混淆字段 | 语义名 | 类型 | 默认值 | 证据 |
|---|---|---|---|---|---|
| 10 | `_$9` | colType | byte | -79 (0xB1) | ColCell.java:61-63, 179 |
| 11 | `_$8` | colWidth | float | 25.0f | ColCell.java:67-69, 180 |
| 12 | `_$7` | colVisible | boolean | true | ColCell.java:91-93, 181 |
| 13 | `_$6` | breakPage | boolean | false | ColCell.java:116-118, 182 |
| 14 | `_$3` | notes | String | null | ColCell.java:153-155, 183 |
| 15 | `_$5` | breakColumn | boolean | false | ColCell.java:129-131, 184 |

字段声明默认值：ColCell.java:26-36。

**ColCell 紧凑流槽位表**（ColCell.java:295-319 / :324-343）：

| 流序 | 内容 | 字节 | 证据（写/读） |
|---|---|---|---|
| 1 | colType | 1 | :301 / :326 |
| 2 | colWidth | 4 | :302 / :327 |
| 3 | colVisible | 1 | :303 / :328 |
| 4 | breakPage | 1 | :304 / :329 |
| 5 | expMap（ByteMap 记录） | 4 或 4+N | :305 / :330 |
| 6 | notes | 4+N 或 4 | :306 / :331 |
| 7 | **OIS blob**（`_$2` customPropsMap, `_$1` customPropsExpMap） | 4+N | :307-316 / :332-341 |
| 8 | breakColumn | 1 | :317 / :342 |

ColCell expMap（`_$4`）键：11=colWidthExp、12=colVisibleExp、13=breakPageExp、15=breakColumnExp（ColCell.java:77,99,121,143）。

### B.3 NormalCell 字典（属性码 20-94）

字段默认值声明：NormalCell.java:44-108。`COLOR_BLACK = Color.black.getRGB()`（0xFF000000）、`COLOR_WHITE = 0xFFFFFFFF`、`DEFAULT_FONT_NAME = "宋体"`（NormalCell.java:41-43）。

| 属性码 | 混淆字段 | 语义名 | 类型 | 默认值 | 证据（getter/getPropertyMap） |
|---|---|---|---|---|---|
| 20 | `_$65` | cellType | byte | -64 (0xC0) | NormalCell.java:118-120, 647 |
| 21 | `_$64` | mergedArea | Area | null | NormalCell.java:145-147, 648 |
| 22 | `_$63` | hAlign | byte | -48 (0xD0，=左) | NormalCell.java:157-159, 649 |
| 23 | `_$62` | vAlign | byte | -31 (0xE1，=中) | NormalCell.java:166-168, 650 |
| 24 | `_$61` | diagonalStyle | byte | 0 | NormalCell.java:173-175, 651 |
| 25 | `_$60` | diagonalColor | int | COLOR_BLACK | NormalCell.java:181-183, 653 |
| 26 | `_$59` | diagonalWidth | float | 1.0 | NormalCell.java:189-191, 654 |
| 27 | `_$20` bit0 | textWrap | boolean | false | NormalCell.java:205-207, 655 |
| 28 | `_$58` | extendMode | byte | 16 | NormalCell.java:213-215, 656 |
| 29 | `_$57` | extendArea | Area | null | NormalCell.java:221-223, 657 |
| 30 | `_$56` | leftHead | String | null | NormalCell.java:229-231, 658 |
| 31 | `_$55` | topHead | String | null | NormalCell.java:238-240, 659 |
| 32 | `_$20` bit1 | splitted | boolean | false | NormalCell.java:245-247, 660 |
| 33 | `_$20` bit2 | breakPageAfterRow | boolean | false | NormalCell.java:253-255, 661 |
| 34 | `_$20` bit3 | breakPageAfterCol | boolean | false | NormalCell.java:262-264, 662 |
| 35 | `_$20` bit4 | stretchWhenPaged | boolean | false | NormalCell.java:285-287, 665 |
| 36 | `_$54` | toExcelMode | byte | 34 | NormalCell.java:293-295, 666 |
| 37 | `_$53` | adjustSizeMode | byte | 49（setPropertyMap null 重置为 48，见注） | NormalCell.java:301-303, 667 |
| 38 | `_$52` | cellObj（graph/subreport/barcode 多态） | Object | null | NormalCell.java:309-331, 668 |
| 40 | `_$51` | **value**（VALUE） | Object | null | NormalCell.java:445-447, 669 |
| 41 | `_$50` | **dispValue**（DISPVALUE） | String | null | NormalCell.java:453-455, 670 |
| 42 | `_$49` | indent | float | 0.0 | NormalCell.java:461-463, 671 |
| 43 | `_$48` | fontName | String | "宋体" | NormalCell.java:477-479, 672 |
| 44 | `_$47` | fontSize | short | 12 | NormalCell.java:485-487, 673 |
| 45 | `_$20` bit8 | bold | boolean | false | NormalCell.java:493-495, 674 |
| 46 | `_$20` bit9 | italic | boolean | false | NormalCell.java:501-503, 675 |
| 47 | `_$20` bit10 | underline | boolean | false | NormalCell.java:509-511, 676 |
| 48 | `_$20` bit5 | visible | boolean | **true**（默认 short 32 置位） | NormalCell.java:525-527, 678 |
| 49 | `_$45` | format | String | null | NormalCell.java:533-535, 679 |
| 50 | `_$20` bit6 | rowHidden | boolean | false | NormalCell.java:541-543, 680 |
| 51 | `_$20` bit7 | colHidden | boolean | false | NormalCell.java:549-551, 681 |
| 52 | `_$44` | foreColor | int | COLOR_BLACK | NormalCell.java:557-559, 682 |
| 53 | `_$43` | backColor | int | COLOR_WHITE | NormalCell.java:565-567, 683 |
| 54 | `_$42` | hyperlink | String | null | NormalCell.java:573-575, 684 |
| 55 | `_$41` | hyperlinkWindow | String | null | NormalCell.java:582-584, 685 |
| 60 | `_$40` | lbStyle | byte | 0 | NormalCell.java:333-335, 687 |
| 61 | `_$37` | rbStyle | byte | 0 | NormalCell.java:357-359, 690 |
| 62 | `_$34` | tbStyle | byte | 0 | NormalCell.java:381-383, 693 |
| 63 | `_$31` | bbStyle | byte | 0 | NormalCell.java:405-407, 696 |
| 64 | `_$39` | lbWidth | float | 1.0 | NormalCell.java:341-343, 688 |
| 65 | `_$36` | rbWidth | float | 1.0 | NormalCell.java:365-367, 691 |
| 66 | `_$33` | tbWidth | float | 1.0 | NormalCell.java:389-391, 694 |
| 67 | `_$30` | bbWidth | float | 1.0 | NormalCell.java:423-425, 697 |
| 68 | `_$38` | lbColor | int | COLOR_BLACK | NormalCell.java:349-351, 689 |
| 69 | `_$35` | rbColor | int | COLOR_BLACK | NormalCell.java:373-375, 692 |
| 70 | `_$32` | tbColor | int | COLOR_BLACK | NormalCell.java:397-399, 695 |
| 71 | `_$29` | bbColor | int | COLOR_BLACK | NormalCell.java:429-431, 698 |
| 81 | `_$26` | notes | String | null | NormalCell.java:589-591, 686 |
| 84 | `_$22` | cellGraphConfig | CellGraphConfig | null | NormalCell.java:613-615, 699 |
| 85 | `_$28` | underLineStyle | byte | 0 | NormalCell.java:413-415, 700 |
| 86 | `_$21` | cellStyleName | String | null | NormalCell.java:469-471, 701 |
| 87 | `_$20` bit12 | breakColumnAfterRow | boolean | false | NormalCell.java:269-271, 663 |
| 88 | `_$20` bit13 | breakColumnAfterCol | boolean | false | NormalCell.java:277-279, 664 |
| 89 | `_$4` | mergeSame | byte | 97 | NormalCell.java:1213-1215, 702 |
| 90 | `_$3` | mergeNull | byte | 113 | NormalCell.java:621-623, 703 |
| 91 | `_$2` | mergeSameMode | byte | 118 | NormalCell.java:629-631, 704 |
| 92 | `_$1` | mergeNullMode | byte | 121 | NormalCell.java:637-639, 705 |
| 93 | `_$20` bit14 | backDiagonal | boolean | false | NormalCell.java:197-199, 652 |
| 94 | `_$46` | rotationAngle | float | 0.0 | NormalCell.java:517-519, 677 |

注 1：键 37（adjustSizeMode）字段默认 49（NormalCell.java:56）但 `setPropertyMap` 的 null 重置分支写 48（NormalCell.java:881）——反编译代码自身不一致，**待后端实现时验证**（不影响二进制解析，只影响属性表回填路径）。
注 2：任务书"97-100=MERGESAME"与实际不符：合并族四键是 **89=mergeSame(默认97)、90=mergeNull(默认113)、91=mergeSameMode(默认118)、92=mergeNullMode(默认121)**（NormalCell.java:702-705）。"97"是默认值不是码位，以本表为准。
注 3：hAlign 取值语义（NormalCell.java:1225-1235）：-48=left、-47=center、-46=right、-45=justify、-44=distribute。vAlign（:1237-1244）：-32=top、-31=middle、-30=bottom。extendMode（:656 默认16）：16=默认/自动、17=不扩展、18=扩展、19=扩展（横向判定）——语义证据 ControlUtils.java:1081,1094-1100,1176-1177，**完整枚举语义待后端实现时验证**。

#### B.3.1 位打包 short `_$20`（14 个布尔逐位表）

默认值 `_$20 = 32`（NormalCell.java:89）= 仅 bit5 置位 = visible=true。共 14 个有效位，bit11 与 bit15 无访问器（保留位）：

| 位 | 掩码 | 语义名 | 默认（short=32 时） | getter 证据 |
|---|---|---|---|---|
| 0 | 0x0001 | textWrap | false | NormalCell.java:206 |
| 1 | 0x0002 | splitted | false | NormalCell.java:246 |
| 2 | 0x0004 | breakPageAfterRow | false | NormalCell.java:254 |
| 3 | 0x0008 | breakPageAfterCol | false | NormalCell.java:263 |
| 4 | 0x0010 | stretchWhenPaged | false | NormalCell.java:286 |
| 5 | 0x0020 | visible | **true** | NormalCell.java:526 |
| 6 | 0x0040 | rowHidden | false | NormalCell.java:542 |
| 7 | 0x0080 | colHidden | false | NormalCell.java:550 |
| 8 | 0x0100 | bold | false | NormalCell.java:494 |
| 9 | 0x0200 | italic | false | NormalCell.java:502 |
| 10 | 0x0400 | underline | false | NormalCell.java:510 |
| 12 | 0x1000 | breakColumnAfterRow | false | NormalCell.java:270 |
| 13 | 0x2000 | breakColumnAfterCol | false | NormalCell.java:278 |
| 14 | 0x4000 | backDiagonal | false | NormalCell.java:198 |

#### B.3.2 NormalCell 紧凑流槽位表（49 槽，解析器唯一事实源）

serialize：NormalCell.java:1468-1539；fillRecord：:1544-1616。逐槽对齐：

| 流序 | 内容 | 字节形态 | 证据（写/读） |
|---|---|---|---|
| 1 | mergedArea | Area 记录（4×int32：r1,r2,c1,c2）；null → int32 -1 | :1474 / :1546 |
| 2 | hAlign | 1 | :1475 / :1547 |
| 3 | vAlign | 1 | :1476 / :1548 |
| 4 | diagonalStyle | 1（读侧 80→0 归一化） | :1477 / :1549 |
| 5 | diagonalColor | 4 | :1478 / :1550 |
| 6 | diagonalWidth | 4 | :1479 / :1551 |
| 7 | extendMode | 1 | :1480 / :1552 |
| 8 | extendArea | Area 记录 | :1481 / :1553 |
| 9 | leftHead | readString | :1482 / :1554 |
| 10 | topHead | readString | :1483 / :1555 |
| 11 | toExcelMode | 1 | :1484 / :1556 |
| 12 | adjustSizeMode | 1 | :1485 / :1557 |
| 13 | cellObj 存在标志 | 1（false=0x00）；true 时后接 readString 类名 + 该对象记录 | :1486-1492 / :1558-1568 |
| 14 | **value（VALUE）** | readObject 标签（见 A.5） | :1493 / :1569 |
| 15 | dispValue | readString | :1494 / :1570 |
| 16 | indent | 4 | :1495 / :1571 |
| 17 | fontName | readString | :1496 / :1572 |
| 18 | fontSize | 2 | :1497 / :1573 |
| 19 | format | readString | :1498 / :1574 |
| 20 | foreColor | 4 | :1499 / :1575 |
| 21 | backColor | 4 | :1500 / :1576 |
| 22 | hyperlink | readString | :1501 / :1577 |
| 23 | hyperlinkWindow | readString | :1502 / :1578 |
| 24 | lbStyle | 1（80→0） | :1503 / :1579 |
| 25 | lbWidth | 4 | :1504 / :1580 |
| 26 | lbColor | 4 | :1505 / :1581 |
| 27 | rbStyle | 1（80→0） | :1506 / :1582 |
| 28 | rbWidth | 4 | :1507 / :1583 |
| 29 | rbColor | 4 | :1508 / :1584 |
| 30 | tbStyle | 1（80→0） | :1509 / :1585 |
| 31 | tbWidth | 4 | :1510 / :1586 |
| 32 | tbColor | 4 | :1511 / :1587 |
| 33 | bbStyle | 1（80→0） | :1512 / :1588 |
| 34 | bbWidth | 4 | :1513 / :1589 |
| 35 | bbColor | 4 | :1514 / :1590 |
| 36 | expMap | ByteMap 记录（int16 count + 逐条 byte 键 + readObject 值）；null → -1 | :1515 / :1591 |
| 37 | bitFlags `_$20` | 2（位打包 short） | :1516 / :1592 |
| 38 | cellType | 1 | :1517 / :1593 |
| 39 | notes | readString | :1518 / :1594 |
| 40 | **writeStrings(null) 空洞** | 固定 2 字节 `00 00`，读侧丢弃 | :1519 / :1595 |
| 41 | **OIS blob** | int32 长度 + Java ObjectInputStream 序列化（`_$24` customPropsMap、`_$23` customPropsExpMap 两个对象） | :1520-1529 / :1596-1605 |
| 42 | cellGraphConfig | CellGraphConfig 记录；null → -1 | :1530 / :1606 |
| 43 | underLineStyle | 1（80→0） | :1531 / :1607 |
| 44 | cellStyleName | readString | :1532 / :1608 |
| 45 | mergeSame | 1 | :1533 / :1609 |
| 46 | mergeNull | 1 | :1534 / :1610 |
| 47 | mergeSameMode | 1 | :1535 / :1611 |
| 48 | mergeNullMode | 1 | :1536 / :1612 |
| 49 | rotationAngle | 4（float）；读侧 `available() > 0` 守卫，缺省 0.0 | :1537 / :1613-1615 |

#### B.3.3 读侧归一化与内嵌格式（M1 必守）

1. **style 字节 80→0**：`_$1(byte style)` 将 80（'P'）映射为 0（NormalCell.java:1688-1693），fillRecord 对槽 4/24/27/30/33/43（diagonalStyle/lbStyle/rbStyle/tbStyle/bbStyle/underLineStyle）施加（:1549,1579,1582,1585,1588,1607）。Rust 必须同样归一化。
2. **边框线型字节语义**（NormalCell.java:1316-1336）：0=none、81='Q'=dotted、82='R'=dashed、83='S'=solid、84='T'=double、85='U'=dotted；未知值按 solid。
3. **Area 记录格式**（Area.java:126-142）：4×int32 大端，顺序为 **r1(beginRow), r2(endRow), c1(beginCol), c2(endCol)**——注意不是 r1,c1,r2,c2。writeExternal 另带版本字节（:108-114），但紧凑 serialize 不带。
4. **ByteMap 记录格式**（ByteMap.java:344-372）：int16 count（大端）+ 逐条 `byte key` + `writeObject(value)`（A.5 标签体系）。expMap 为该格式。
5. **OIS blob（槽 41）**：两个 IByteMap 经 Java 对象序列化后整体作为 bytes 内嵌。默认（两者皆 null）时 blob 恒为 **6 字节 `AC ED 00 05 70 70`**（STREAM_MAGIC+STREAM_VERSION+两个 TC_NULL）。M1 策略：读 int32 长度后**按长度整段跳过并原样保留字节**；非 null 的 OIS 解析（含 ByteMap Externalizable 展开）**待后端实现时验证**（M2 需要自定义属性时再实现）。
6. **cellObj 多态（槽 13）**：标志 false 时空态；true 时类名 + 记录，M1 记录类名与原始字节，不实例化（ReportGraphProperty/SubRptProperty/BarcodeProperty 的记录格式**待后端实现时验证**）。
7. **cellGraphConfig（槽 42）**：同上，M1 跳过保留字节，格式待验证。

---

## 附件 C：ReportDefine 载荷字段序核实

证据：`reportlite/03-model/com/raqsoft/reportl/model/ReportDefine.java`（fillRecord :1129-1199，serialize :1047-1126）；`ReportDefine2.java`（全文）；`reportlite/05-core-runtime/com/raqsoft/reportl/util/ReportUtils.java`（read :27-53，write :70-96）。

### C.1 对 Spec §5.2 的核实结论

| Spec §5.2 陈述 | 核实结果 | 说明 |
|---|---|---|
| 矩阵：r、c 两个 int32，行 1..r / 列 1..c 循环，RowCell→ColCell→NormalCell 行优先 | **正确** | ReportDefine.java:1132-1145（读）/ :1050-1076（写） |
| 7 个 MetaData：PrintSetup, MacroMetaData, ParamMetaData, DataSetMetaData, SubReportMetaData, BackGraphConfig, ExportConfig | **正确**（顺序一致） | :1162-1171 / :1077-1083 |
| 10 个标量：int32, byte, String, byte, String, String, int, int, byte, String | **正确**（类型序列一致） | :1172-1181 / :1084-1093 |
| cpSize：int32 + (String,String)×cpSize | **正确**（另有 null key 跳过怪癖，见 C.2） | :1182-1191 / :1094-1113 |
| 尾巴："String(_$5) String(_$4) String(_$3) String(_$2) boolean(_$1) boolean(_$0) int String(md5)" | **错误，4 处出入** | 见下 |
| 尾巴字段"均为 available() > 0 才读（探测式尾巴）" | **错误** | ReportDefine.fillRecord 全文无 available() 守卫；探测式尾巴仅存在于 NormalCell 槽 49（rotationAngle） |

**尾巴 4 处出入明细**（源码 :1192-1198 / :1118-1124）：

1. `_$5` **不是 String**，是 **ByteMap 记录**（`readRecord(new ByteMap())`，即 int32 长度 + ByteMap 载荷；null → int32 -1）——Spec 漏掉了这一整条记录，把它误当成 String。
2. Spec 遗漏了 `_$12`（chartStyleName，String），它位于 `_$5` 记录之后、`_$4` 之前。
3. Spec 的 `boolean(_$1)` 错误：布尔量是 `_$16`（highPrecision，默认 true）；`_$1` 是 int（threshold，默认 -1）。
4. Spec 的 `int String(md5)` 不存在：正确的尾部是 `boolean(_$16)` + `int(_$1)`，**threshold 之后没有任何字段，更没有 md5**。

### C.2 ReportDefine 正确字段序（Rust 唯一事实源）

```
fillRecord 序（ReportDefine.java:1132-1198）：
 1. int32  r                        ← cellMatrix 行数（含 0 号头行）
 2. int32  c                        ← cellMatrix 列数（含 0 号头列）
 3. row = 1..r-1   : RowCell 记录（readRecord）
 4. col = 1..c-1   : ColCell 记录（readRecord）
 5. row = 1..r-1, col = 1..c-1（行优先）: NormalCell 记录（readRecord）
      空槽 / 非锚点合并格 = int32 -1
 6. 7 个 MetaData 记录：
      PrintSetup(_$25) → MacroMetaData(_$24) → ParamMetaData(_$23) →
      DataSetMetaData(_$22) → SubReportMetaData(_$21) → BackGraphConfig(_$20) →
      ExportConfig(_$19)
      注：PrintSetup 读回 null 时兜底 new PrintSetup()（:1163-1165）
 7.  int32  _$18  dispRatio         （默认 133）
 8.  byte   _$17  unit              （默认 1）
 9.  String _$15  tip               （readString）
10.  byte   _$14  submit            （默认 0）
11.  String _$13  notes
12.  String _$11  futureCellNumExp
13.  int32  _$10  imageMaxWidth     （默认 0）
14.  int32  _$9   imageMaxHeight    （默认 0）
15.  byte   _$8   pageBorderMode    （默认 1）
16.  String _$7   reportStyleName
17.  int32  cpSize + cpSize × { String key, String value }
      —— key 为 null 时**跳过 value 不读**（读侧 :1187-1188；
         写侧对应写 writeRecord(null) = int32 -1 占 4 字节，:1101-1112，两侧对齐）
      —— cpSize <= 0 时 _$6 保持 null
18.  记录   _$5   expMap            （readRecord(new ByteMap())）
19.  String _$12  chartStyleName
20.  String _$4   validityScript
21.  String _$3   bigDataSetName
22.  String _$2   password
23.  boolean _$16 highPrecision     （默认 true）
24.  int32  _$1   threshold         （默认 -1；**流到此结束**）
```

serialize 侧逐字段同序（ReportDefine.java:1047-1125），可作对拍 writer 参考。

### C.3 字段语义映射（getter 证据）

| 混淆字段 | 语义名 | 类型 | 默认值 | 证据 |
|---|---|---|---|---|
| `_$25` | printSetup | PrintSetup | new PrintSetup() | ReportDefine.java:129-131, 66 |
| `_$24` | macroMetaData | MacroMetaData | null | :142-144, 67 |
| `_$23` | paramMetaData | ParamMetaData | null | :152-154, 68 |
| `_$22` | dataSetMetaData | DataSetMetaData | null | :222-224, 69 |
| `_$21` | subReportMetaData | SubReportMetaData | null | :232-234, 70 |
| `_$20` | backGraphConfig | BackGraphConfig | null | :242-244, 71 |
| `_$19` | exportConfig | ExportConfig | new ExportConfig() | :252-254, 72 |
| `_$18` | dispRatio | int | 133 | :262-264, 73 |
| `_$17` | unit | byte | 1 | :272-274, 74 |
| `_$15` | tip | String | null | :329-331, 76 |
| `_$14` | submit | byte | 0 | :120-122, 77 |
| `_$13` | notes | String | null | :339-341, 78 |
| `_$12` | chartStyleName | String | null | :349-351, 79 |
| `_$11` | futureCellNumExp | String | null | :506-508, 80 |
| `_$10` | imageMaxWidth | int | 0 | :515-517, 81 |
| `_$9` | imageMaxHeight | int | 0 | :523-525, 82 |
| `_$8` | pageBorderMode | byte | 1 | :101-103, 83 |
| `_$7` | reportStyleName | String | null | :111-113, 84 |
| `_$6` | customProperties | Map<String,String> | null | :884-886, 85 |
| `_$5` | expMap | IByteMap | null | :893-895, 86 |
| `_$4` | validityScript | String | null | :921-923, 87 |
| `_$3` | bigDataSetName | String | null | :931-933, 88 |
| `_$2` | password | String | null | :950-952, 89 |
| `_$16` | highPrecision | boolean | true | :964-967, 75 |
| `_$1` | threshold | int | -1 | :973-975, 90 |

### C.4 ReportDefine2 核实结论

| 问题 | 结论 | 证据 |
|---|---|---|
| 是否覆写 fillRecord/serialize？ | **否**。全文只有构造器与属性表辅助方法，二进制读写完全继承 ReportDefine | ReportDefine2.java:38-39（extends ReportDefine）；对 ReportDefine2.java 全文 grep `fillRecord|serialize` 零命中 |
| 构造器 `(nRow, nColumn)` 语义 | `nRow<1 \|\| nColumn<1` 直接返回（不建矩阵）；否则**双双先 +1** 再建 `Matrix(nRow, nColumn)`；`(i,0)=new RowCell()`（i=1..nRow-1）、`(0,j)=new ColCell()`（j=1..nColumn-1）、内部区 `new NormalCell()` 填满 | ReportDefine2.java:47-64 |
| 与"0 号虚格边"的关系 | 文件内 r/c = 逻辑行列 + 1；RowCell/ColCell/NormalCell 循环均从 1 开始，0 号行/列**不落盘** | ReportDefine.java:1054-1063（写循环）、1135-1143（读循环）；ReportDefine2.java:52-58 |
| 读入口的构造细节 | `ReportUtils.read` 命中魔数后 `new ReportDefine2().fillRecord(buf)`；无参构造经 `ReportDefine()`→`(10,10)` 先建 11×11 矩阵，随后 fillRecord 用 `new Matrix(r, c)` **整体替换**——初值矩阵被丢弃 | ReportUtils.java:45-46；ReportDefine.java:92-94；ReportDefine.java:1134 |
| Rust 对应策略 | 不必模拟 11×11 占位：读 r/c 后直接按 (r×c) 建网格，逻辑等价 | 由上三条推导 |

### C.5 文件层补充核实（对 Spec §5.1 的两点补强）

| 事实 | 证据 |
|---|---|
| 写侧：`82,81,81,82` + int32 BE 长度 + payload + `82,81,81,82,82,81,81,82`（页脚 8 字节，即两遍魔数） | ReportUtils.java:70-96 |
| 读侧：只校验 4 字节魔数 + 读长度 + 读 payload，**不校验页脚**；魔数不命中时回退 Java OIS（`in.reset()` + ObjectInputStream） | ReportUtils.java:27-53 |
| M1 决策：按 Spec §3/AC-09，魔数不命中必须 `UnsupportedFormat`，**不回退 OIS**；页脚按 AC（P0 验收行）要求校验——真实 writer 必写页脚，夹具必然带，校验不会误杀 | spec-m1.md:29（不做项）、:154（AC-01/09）、:19（P0 行） |
| 长度校验（AC-02）：Java 侧 `readBytes(in, buf)` 读不足会抛 EOF；Rust 在进入 fillRecord 前检查剩余字节 ≥ N，不足报 `Truncated` | ReportUtils.java:42-44；spec-m1.md:155 |

---

## 附件 D：PrintSetup 记录格式（ReportDefine 首个 MetaData，M1 完整解析）

证据：`reportlite/05-core-runtime/com/raqsoft/reportl/usermodel/PrintSetup.java`（serialize :1027-1062，fillRecord :1066-1105，字段默认值 :134-166）。

**为什么必须完整解析**：`ReportDefine._$25` 构造时即 `new PrintSetup()`（ReportDefine.java:66），`serialize()` 无条件写出该记录，**载荷里永远存在且长度恒 >0**（30+ 槽），不存在 null 分支（写侧 `writeRecord` 只在整体为 null 时写 -1，而此处永远非 null）。M1 若把 PrintSetup 当"占位跳过"处理会丢失维度换算基准（unit 转换率与纸张参数相关），建议按定序裸值完整解析。

**槽位表**（serialize :1029-1061 与 fillRecord :1068-1104 逐槽对齐）：

| 流序 | 字段 | 类型 | 默认值 | 证据 |
|---|---|---|---|---|
| 1 | paper | short | 9 | PrintSetup.java:1029/1068, 134 |
| 2 | tableColumnNum | short | 1 | :1030/1069, 135 |
| 3 | layout_rows | short | 1 | :1031/1070, 137 |
| 4 | layout_cols | short | 1 | :1032/1071, 138 |
| 5 | rowNumPerPage | int | 20 | :1033/1072, 139 |
| 6 | zoomPageWidth | short | 1 | :1034/1073, 140 |
| 7 | zoomPageHeight | short | 1 | :1035/1074, 141 |
| 8 | zoomScale | short | 100 | :1036/1075, 142 |
| 9 | paperWidth | float | 210.0 | :1037/1076, 143 |
| 10 | paperHeight | float | 297.0 | :1038/1077, 144 |
| 11 | leftMargin | float | 19.0 | :1039/1078, 145 |
| 12 | rightMargin | float | 19.0 | :1040/1079, 146 |
| 13 | topMargin | float | 25.0 | :1041/1080, 147 |
| 14 | bottomMargin | float | 25.0 | :1042/1081, 148 |
| 15 | orient | byte | 1 | :1043/1082, 150 |
| 16 | layout | byte | 0 | :1044/1083, 151 |
| 17 | pageHeaderAndFooterYMode | byte | 0 | :1045/1084, 155 |
| 18 | pageHeaderAndFooterXMode | byte | 2 | :1046/1085, 154 |
| 19 | rowTableHeaderAndFooterMode | byte | 0 | :1047/1086, 156 |
| 20 | titleYMode | byte | 0 | :1048/1087, 153 |
| 21 | titleXMode | byte | 2 | :1049/1088, 152 |
| 22 | colTableHeaderAndFooterMode | byte | 0 | :1050/1089, 157 |
| 23 | pagerStyle | byte | 2 | :1051/1090, 158 |
| 24 | zoomMode | byte | 1 | :1052/1091, 159 |
| 25 | hAlign | byte | 1 | :1053/1092, 160 |
| 26 | vAlign | byte | 0 | :1054/1093, 161 |
| 27 | backGraphPrinted | boolean | false | :1055/1094, 162 |
| 28 | virtualPrinter | String | null | :1056/1095, 149 |
| 29 | columnRepeatedGroupHeader | boolean | false | :1057/1096, 136 |
| 30 | copyCount | int | 1 | :1058/1097, 163 |
| 31 | fullScreenPreview | boolean | false | :1059/1099-1100, 164 |
| 32 | blankLineFilled | boolean | false | :1060/1102, 165 |
| 33 | columnMode | byte | 0 | :1061/1103, 166 |

注：槽 31-33 为 available() 守卫尾槽（写侧无条件、读侧按剩余字节），语义见附件 A.6；老文件可能缺这三个槽，按默认值缺省。

---

## 附录：M1 实现清单（从本契约直接导出的硬约束）

1. `javautf.rs`：实现 A.2.2 编码规则 + A.2.1 两种长度前缀；`\0`=0xC0 0x80（2 字节）；代理对 6 字节；解码接受单字节 0x00。
2. `reader.rs`：大端原语（A.1）+ `read_record` 切片协议（A.4，`size<=0`→None）+ `read_object` 标签字典（A.5，M1 子集）+ `remaining()`（A.6）。
3. `props.rs`：按 B.1/B.2/B.3.2 的**流序槽位**组织常量；位标志按 B.3.1 逐位定义（默认 0x0020）。
4. `rowcell.rs/colcell.rs/cell.rs`：槽位数分别为 10/8/49；每类第 8/7/41 槽是 OIS blob（int32 长度 + 跳过）；NormalCell 第 40 槽固定跳过 2 字节；第 49 槽 available 守卫；6 个 style 槽 80→0。
5. `report.rs`：按 C.2 的 24 步顺序装载；cpSize 的 null-key 跳过分支；`_$5` 是 ByteMap 记录而非 String；**无 available 尾巴**。
6. 合并区装载：非锚点 = int32 -1 空槽；锚点格的 Area（r1,r2,c1,c2 四 int32）铺满网格（ReportDefine.java:1151-1159）。
7. `meta.rs`：PrintSetup 按附件 D 的 33 槽完整解析（非跳过）；MacroMetaData/ParamMetaData/DataSetMetaData/SubReportMetaData/BackGraphConfig/ExportConfig 六个记录 M1 先按 readRecord 长度切片跳过、保留原始字节，格式展开待 M2（各记录的 fillRecord 格式待后端实现时验证）。
8. 夹具对拍：expected.json 只收录非默认属性单元格，缺失字段=附件 B/D 的默认值清单；合并区仅锚点有记录；浮点按 f32 位比较或 1e-5 容差。

---

## 附件 E：ParamMetaData / DataSetMetaData / FileDataSetConfig 记录格式（M1 实现期逆向，2026-10-12）

> 背景：full.rpx 的 expected.json 要求 params/datasets 逐字段对拍（spec-m1.md §2 P0 行），
> 附录第 7 条"六个 MetaData 先跳过"对该两类不适用——Spec 是契约记录方，以 Spec 为准。
> 本节由实现期从源码逆向产出，证据均为 `reportlite/05-core-runtime/com/raqsoft/reportl/usermodel/` 下文件。

### E.1 Param 记录（10 槽，Param.java:113-126 写 / :129-141 读）

| 流序 | 内容 | 字节 | 证据 |
|---|---|---|---|
| 1 | paramType | 1 | :115 / :131 |
| 2 | dataType | 1 | :116 / :132 |
| 3 | pad（读侧丢弃） | 1 | :117 / :133 |
| 4 | name | readString | :118 / :134 |
| 5 | description | readString | :119 / :135 |
| 6 | null 占位（读侧丢弃） | 4（-1） | :120 / :136 |
| 7 | value | readString | :121 / :137 |
| 8 | short（丢弃） | 2 | :122 / :138 |
| 9 | short（丢弃） | 2 | :123 / :139 |
| 10 | null 占位（丢弃） | 4（-1） | :124 / :140 |

### E.2 ParamMetaData 外层（ParamMetaData.java:138-153 写 / :156-169 读）

`int16 个数 + 逐条 readRecord(Param)`；个数 ≤0 → 空表（只占 2 字节）。

### E.3 DataSetConfig 基类（6 槽，DataSetConfig.java:108-117 写 / :120-128 读）

`name(readString), dataSourceName(readString), releaseMode(byte), cacheData(bool), colTitles(readStrings，int16 个数), noDataPrompt(bool)`

### E.4 FileDataSetConfig（FileDataSetConfig.java:244-337 写 / :340-418 读）

**关键结构：基类 6 槽整体包在 int32 长度 blob 里**（写侧 :248 `writeBytes(super.serialize())`，读侧 :343 `super.fillRecord(in.readBytes())`），其后才是文件槽：

| 流序 | 内容 | 备注 |
|---|---|---|
| 0 | 基类 blob | int32 长度 + E.3 的 6 槽 |
| 7 | fileName / sheetName / encoding | 各 readString |
| 10 | colList | int16 个数 + readString |
| 11-12 | title / quoteRemoved | bool |
| 13 | seperator | short（char） |
| 14 | colTypeList | `available()>0` 守卫；int16 个数 + int32 |
| 15 | colFormatList | 同上守卫；int16 个数 + readString |
| 16-20 | filter + gatherCol/gatherFun/groupCol/selectCol 四组 | `available()>0` 整段守卫；各组 int16 个数 + readString |
| 21 | changeFile | `available()>0` 守卫 |
| 22-23 | beginRow / endRow | `available()>0` 成对守卫 |

### E.5 DataSetMetaData 外层（DataSetMetaData.java:127-149 写 / :152-174 读）

`int16 个数 + 逐条 { boolean flag; flag 为 true 时 String 类名 + readRecord(配置) }`；
**flag=false 的条目读侧直接跳过且不进列表**（:160-169）。

### E.6 factory_class 不在二进制流里（重要）

`getFactoryClass()` 是 DataSetConfig 的抽象方法（DataSetConfig.java:49），每个配置类返回常量：
`FileDataSetConfig.getFactoryClass() = "com.raqsoft.reportl.dataset.FileDataSetFactory"`（FileDataSetConfig.java:42-44）。
解析侧需按类名映射（rpx-core `dataset.rs::factory_class_for`），不可从字节恢复。

### E.7 实现期对契约的两处补充（原契约未覆盖，已实现验证）

1. **RowCell 槽 7 是 groupHeaderLevel（byte，默认 1）**，位于 notes 之后、OIS blob 之前（RowCell.java:362/:389）。
   附件 B.1 槽位表已含此槽，但附录第 4 条"每类第 8/7/41 槽是 OIS blob"易被误读为 RowCell 无此槽——实现曾因此错位，特此固化。
2. **FileDataSetConfig 基类是长度前缀 blob 包裹**（E.4），NormalCell/RowCell/ColCell 则全是内联裸值——三类单元格无此包裹，勿混淆。

### E.8 合并区装载语义澄清（黄金测试发现，Spec AC-04 据此修订）

Java fillRecord（ReportDefine.java:1143-1161）语义：**锚点记录读入时铺满整个合并区；行优先循环继续，
后续槽位若是完整记录则按自身内容 setCell 覆盖**。因此：
- 设计师正常合并（所有槽共享锚点对象）→ 流里非锚点为 -1 空记录 → 装载后全区持同一 CellId；
- 仅锚点设 Area 的"浅合并"（本批夹具即此形态，生成器未向矩阵传播）→ 流里非锚点是完整默认记录 →
  装载后非锚点槽持有自己的默认格。

二者解析行为都必须是"锚点铺满 + 后续覆盖"，Rust 实现与该语义逐行一致；
AC-04 的验收断言相应改为"锚点槽持合并格；区域内其他槽要么持锚点 id 要么是默认格"。

### E.9 退化合并区与维度上界（独立验证发现，2026-10-12）

1. **退化/越界 Area 的归一化**：Area 四个端点是流内 int32，可能是负值或指向矩阵外。
   `fillRecord` 原实现直接 `area.end_row as usize` 落到 `MergedArea`，负值会产生 1.8e19
   级索引 —— 解析本身不 panic（`fill_merge` 有夹取），但任何按网格消费 `merges` 的下游
   必越界。M1 口径：**锚点分支先归一化**（clamp 进 `[row..rows-1]` / `[col..cols-1]`，
   保证 end ≥ begin），`fill_merge` 只接受已归一化元组 —— grid 铺满与 `MergedArea` 记录
   因此恒为同一组数。本条为 M1 决策（Java 侧对该畸形的行为未验证，不冒充）。
2. **矩阵维度上界（抗 DoS）**：矩阵每个槽（**含空槽**）在流里至少占 4 字节 int32 记录
   长度前缀，RowCell/ColCell 亦然，故合法文件恒满足 `rows*cols ≤ payload.len()/4`。
   M1 据此设界并返回 `PropertyCode(1)`：否则 i32 维度会诱导 `Vec::with_capacity` /
   `vec![vec![None; cols]; rows]` 做 10^11 字节级分配，**直接 abort 进程**（非 Err）。
   该界不会拒绝任何真实模板（已由三夹具回归验证）。
   **补充（复验后）**：设界后仍有 `Vec::with_capacity(rows-1)` 的 **22×payload** 线性尖峰
   （RowCell 88B / 每 4B payload，实测）。M1 已去掉该预分配（`Vec::new()` 按真实记录数增长），
   使「顶格 rows + 无记录」的输入不产生任何预分配；实测峰值降为与真实记录数同阶。

---

## 附件 F：引擎（计算态/分页态）源码级契约

> **可信度声明**：本附件由项目总监在架构师因 API 429 失败后接管整理。**只收录已核实项**（附
> `文件:行号` 或实测证据）；未逐行读的大类与未跑通的链路一律列在 F.6「待补清单」，**不得当结论用**。
> 对应规格：`docs/spec-m2.md`。

### F.1 主格判定（MainCell.java）

主格（MainCell）是扩展分组的键：一个主格持有 `(leftHead, topHead)` 两个引用与 `changed` 标志，
用于判断"新遇到的格是否仍属同一扩展区域"（`MainCell.java:9-11,21-39`）。

`setCell(cell, cell00)` 判定表（`cell00` = 传入的基准格引用）：

| `changed` 入 | 条件 | 动作 |
|---|---|---|
| false | `(left==cell00 && top==cell00) \|\| left != leftHead` | `leftHead=left; topHead=top; changed=true` |
| false | 仅 `top != topHead` | `topHead=top; changed=true` |
| false | 都不变 | `changed=false`（**复用同一主格对象**） |
| true | — | `leftHead=left; topHead=top`（不显式改 `changed`，保持 true） |

`reset()` 清空两头并置 `changed=false`（`:49-53`）。
**移植要点**：Rust 侧不需要对象同一性，用 `CellId` 整数比较即可；但 `changed` 的三段判定必须逐字对齐。

### F.2 PropQueue —— **死代码，不属于 M2 结构**（v0.1 勘误）

分块双向链表队列，元素 `(ExtCell, byte key)`，BLOCKSIZE 默认 256（`PropQueue.java:9,19-27`）。

**结论：reportlite 版无任何使用者 → 扩展算法不依赖它。** 证据：对 `F:/project/report/re_work/src` 全量
grep `PropQueue`，命中仅 `reportlite/03-model/.../engine/PropQueue.java`（自身定义）与
`report/04-model/.../engine/PropQueue.java`（**另一个产品 report 的同名类**，不在本工程范围）。
无任何 `new PropQueue()` / `.push(` / `.search(` 调用点。

顺带解掉一个原「待验证」项：`index1` 为何不推进。`PropQueue` 是 `com/raqsoft/commonl/Queue.java`
（同 DataBlock 结构）的复制，**后者有 `poll()` 会 `this.index1++` 推进**；而 `PropQueue` 只
`push`/`search`/`reset`，**从不消费** → `index1` 是继承自模板的未使用字段，恒为 0，
故 `search()` 从 0 起扫 = 全队列扫描（`:47-67`）。**此项已确认，非 CFR 丢失。**

**M2 含义**：不要照搬这个队列。若扩展算法需要待处理集合，直接用 `Vec<(CellId, u8)>` + 线性查找即可。

### F.3 行列引擎（RowCellEngine.java / ColCellEngine.java）

两个类结构同构，是**引擎态的行列载体**（模板态的 RowCell/ColCell 见附件 B.1/B.2）：

| 字段 | 行引擎 | 列引擎 | 含义 |
|------|--------|--------|------|
| `cs` | ✔ | ✔ | 所属 `ExtCellSet` |
| `srcPrepare` | `IRowCell` | `IColCell` | 模板态源对象 |
| `srcCurrent` | `ExtCell` | `ExtCell` | 当前扩展实例 |
| `sourceRow` / `sourceCol` | ✔ | ✔ | 源行号/列号（int） |
| 表达式槽 | `rowVisibleExp`、`rowHeightExp`、`breakPageExp`、`breakColumnExp`、`adjustRowHeightExp` | `colVisibleExp`、`colWidthExp`、`breakPageExp`、`breakColumnExp` | 字段类型是 `Object`，运行时可能是 `Expression` 也可能是常量 |
| `customPropEngMap` | ✔ | ✔ | `IByteMap`，自定义属性码 → 表达式 |

证据：`RowCellEngine.java:23-32`（字段声明）、`:47-61`（表达式落盘）、`ColCellEngine.java:24-31`、`:46-60`。

**关键推论（可迁移）**：两引擎的 `writeExternal` 对每个表达式槽都写成 `Expression.toString()` 的字符串
（`RowCellEngine.java:47-61`），`readExternal` 读回 `Object`（`:90-106`）——**说明表达式在引擎态落盘为字符串，
运行时再解析**。这与 `.rpx` 模板态把值写成对象并不矛盾，但 M2 的 lexer/parser 必须接受字符串形态
（形如 `=B2*2`）。

### F.4 headless 计算链路（对拍基准 — 已实测）

**这是 M2 正确性的基石**：Java 侧可以不启动 GUI、不连数据库跑出计算态。

```java
IReport rd  = ReportUtils.read(path);                    // 读模板
IReport rpt = new Engine(rd, new Context()).calc();     // 计算
```

- 证据：`tests/fixtures/ProbeCalc.java:76-77, 91-92, 127, 146`；栈帧 `probe_err.txt`：
  `Engine.calc(Engine.java:85)` → `Engine.calcDataSet(:131)` → `calcDataSet(:147)` → `FileDataSetFactory.createDataSetM`。
- **可取字段（实测）**：每个计算态格的 `value`（含 Java 类型 `Integer`/`Double`/`String`）、`src`（源格坐标）、
  `rowMerge`、`colMerge`、`extend`（extendMode）、`subCells`（附属格集合）——见 `probe_out.txt` [A]/[C]/[D]。
- **数据集两条路**：① 内存注入 `new Context()` + `rd.setDataSetMetaData(dsmd)`（Phase D 可行）；
  ② 落 CSV 后走真实 `FileDataSetFactory`（Phase C 可行，需先写 `data.csv`）。
- **缺数据集是致命错误**：`ReportError` + 「ServletContext 为空，系统未初始化」（Phase B 实测）。
  → Rust 侧同样必须**先解析数据集再扩展**，否则会重蹈"空上下文算出错值"。
- **表达式实测**：`B2*2` 形态**可算且结果正确**（Phase F：100→200、200→400、150→300）；
  `sum($B$2:$B$4)` 报「未知的单元格 $B$2」（**绝对引用语法在该路径不可用**）；
  `sum($B2:B4)` 结果与"失败回退显示本行值"**不可区分** → 见 F.6 待验证。

### F.5 extendMode 行为（实测）

`extendMode` 常量：`16=ExtendDefault`、`17=ExtendNone`、`18=ExtendVertical`、`19=ExtendHorizontal`
（`crates/rpx-core/src/props.rs::def`，源见附件 B.3.2 槽 7）。

实测（`probe_out.txt` Phase C，`full.rpx` + 3 行 CSV 数据）：

| 模板格 | extend | 计算态行为 |
|---|---|---|
| `A2`（区域列） | 18 | 主格带 **3 个 subCells**：`{(2,1) "North"} {(3,1) "East"} {(4,1) "South"}` → 数据行数决定扩展长度 |
| `B2`（数量列） | 18 | 与 A 列同区扩展 |
| `C2`（Doubled） | 17 | 不扩展 |
| `D2`（Running） | 17 | 不扩展 |
| 其余行 | 16 | 默认 |

**待验证**：extend=16（默认）在有数据集时到底扩不扩展——实测里 16 的格子 `subCells=1`，但 Phase F 显示
"Running" 列在换公式后仍逐行不同（说明它随数据行变化），**默认模式的扩展语义需专门消歧**（F.6）。

### F.6 待补清单（禁止当结论用）

| 项 | 状态 | 消歧/补齐方式 |
|----|------|---------------|
| `ExtCellSet` 3781 行内部算法 | **部分关闭**（v0.4 勘误，见 K.5/L.1） | 扩展见附件 I、调度见 K.1、合并见 K.2/K.3、收尾/页壳/序列化见 K.4、**属性表达式见附件 L**；**仅 `perfected` 未覆盖（P2）**。P0 按"输入输出等价"实现，结构等价性不追求 |
| `ExtNormalCell` 2719 行 | **部分关闭**（v0.3 勘误，见 K.5） | `checkExtend/calculate/testValue/nonCalc/expandLeftHead/serialize` 已逐行核实（清单见 K.5）；`prepProperties` 与富文本/图形分支未读 |
| `SimplePager` 3403 行 | **部分关闭**（v0.3 勘误，见 K.5） | `createPages`、固定区探测、`copyRowTo`/`copyCellTo` 已逐行核实；主循环 `_$5:274+` 的分组/分栏分支按 P1 补 |
| `sum(range)` 求值语义 | 不可区分 | 构造 `sum(B2:B4)` 置于行 2：真值 450 vs 回退值 100，一测即分（已派发） |
| extendMode=16 默认模式的扩展行为 | 不可区分 | 同 F.5，需专用夹具（已派发） |
| 分页基准链路 | 未跑通 | `ProbeCalc.java:211,255` 已写 `ReportUtils2.getPagerInfo(rpt, 595, 842, 1)`，但最后一次运行（12:44）早于代码定稿（12:52） |
| 函数注册时机 | 待查 | Phase B 报「ServletContext 为空」；确认函数是否在 web 初始化注册（决定 P1 函数面能否对拍） |
| ~~`$` 绝对引用语法~~ | ✅ **已解** | 见 F.7：ReportLite 的 `$` 是**整格前缀**（`$B2` 可用），**不支持 Excel 的 `$B$2`** |
| ~~`PropQueue.index1`~~ | ✅ **已解** | 见 F.2：`PropQueue` 是死代码，`index1` 是继承自 `commonl.Queue` 的未使用字段 |
| 表达式 → 外部化 | 已确认 | `RowCellEngine.java:47-61` 写 `toString()` |

---

## 附件 G：地址引用解析契约（v0.2 新增）

> 用途：M2 的表达式引用解析、探针 `src` 字段、循环引用检测都依赖这几条。

### G.1 `$` 前缀 = **isSaved**（改用保存的 current 解析）——**不是 Excel 语法，也不是「绝对格」**

`Expression.java:193-203`：

```
c = id.charAt(0);
if (c == '@') { ...参数引用... }                      // @name / @@name
if (c == '$') {
    id = id.substring(1);                             // 只剥掉开头的 $
    if (cs instanceof ExtCellSet && (cell = ((ExtCellSet)cs).getSource(id)) != null)
        return new CSVariable(cell, true);            // true = isSaved（不是「绝对」）
    throw new ReportError(EngineMessage.get().getMessage("Expression.unknownCell") + "$" + id);
}
if (cs instanceof ExtCellSet && (cell = ((ExtCellSet)cs).getSource(id)) != null)
    return new CSVariable(cell, false);               // isSaved = false
```

**先把「剥 `$`」这半边说清（成立）**：代码只剥**一个开头 `$`**，剩下的字符串交给 `getSource` → `CellLocation.parse`。
因此 `$B2` 可用；`$B$2` 剥完得 `B$2`，解析失败 → 报 `未知的单元格$B$2`
（本轮用 `VerifyDollar.java` 复测确认：`setExp(D2,"$B$2")` → `ReportError: 单元格D2中错误: 表达式（$B$2）: 未知的单元格$B$2`）。

**但 `$` 的作用不止「剥掉」——它还传了 `isSaved=true`**（v0.4 修正）：

```
CSVariable.java:31-44
    public CSVariable(ExtCell source, boolean isSaved) {
        ...
        this._$5 = isSaved ? (byte)1 : 0;
    }
CSVariable.java:116-127   // calculate()
    current = this._$5 == 1 ? cs.getSavedCurrent() : cs.getCurrent();
```

即：**`$` 切换的是「用哪个 current 去解析引用」，不是「引用是否绝对」**。
`getSavedCurrent()` 返回 `saveCurrent()` 拍下的快照（`ExtCellSet.java:586-592`），
而 `saveCurrent()/resumeCurrent()` 由 `Sum`/`Select`/`Average`/`Max`/`Min` 在求值操作数前后调用
（`Sum.java:150,190` 等）——所以 `$` **只在聚合/多操作数上下文里才产生差异**。

**本轮实测（`tests/fixtures/VerifyDollar.java`，同一次运行内并排，数据集 North/100 East/200 South/150）**：

| 表达式 | 行2 | 行3 | 行4 | 结论 |
|--------|-----|-----|-----|------|
| `B2`   | 100 | 200 | 150 | 标量相对 |
| `$B2`  | 100 | 200 | 150 | **与 `B2` 完全一致** |
| `B2*2` | 200 | 400 | 300 | — |
| `$B2*3`| 300 | 600 | 450 | 标量下 `$` 无差别 |
| `sum(B2:B4)`  | 100 | 100 | 100 | **常量** |
| `sum($B2:B4)` | 100 | 200 | 150 | **逐行** |
| `sum(B2:B2)`  | 100 | 200 | 150 | 同格，`$` 无差别 |
| `sum($B2:B2)` | 100 | 200 | 150 | 同格，`$` 无差别 |

> ⚠ **M2 硬约束（两条）**：① 不得按 Excel 习惯实现 `$`（`$B$2` 必须报「未知的单元格」）；
> ② **不得把 `$` 简化为「剥掉后当普通引用」**——那样 `sum($B2:B4)` 会算成 100/100/100，
> 而基准要求 100/200/150，golden 必挂。必须实现 isSaved ≡「用保存的 current」。

### G.2 源格查找 `ExtCellSet.getSource(String cellId)`（`:610-621`）

```
CellLocation pos = CellLocation.parse(cellId);
if (pos != null) {
    row = pos.getRow(); col = pos.getCol();
    if ((row > 0 && row < rowSize && col > 0 && col < colSize || (row == 0 && col == 0))
        && (eng = (NormalCellEngine)matrix.get(row, col)) != null)
        return eng.srcCurrent;                        // ← 探针输出的 src 字段
}
return null;
```

| 契约点 | 规则 |
|--------|------|
| 地址解析 | `CellLocation.parse(String)`（`B2` → row/col），解析失败返回 `null`（不抛异常） |
| 边界 | `0 < row < rowSize && 0 < col < colSize`，**或** `row==0 && col==0`（A1 特例） |
| 返回 | `NormalCellEngine.srcCurrent`（`ExtCell`）——即该模板格的**当前扩展实例** |
| 失败 | 一律返回 `null`（由调用方决定抛错） |

> 探针 dump 里的 `src=(2,1)` 就是这个 `srcCurrent` 的源坐标；M2 对拍时它是对齐扩展正确性的关键字段。

### G.3 左邻/上邻主格追溯与循环引用检测（`:631-646`）

`ExtCellSet._$3(ExtNormalCell cell)` 的前段：

1. 取 `cell.getLeftHead()`（左邻的**源格 id 字符串**），trim 后 `getSource(head)` 得 `head1`；
2. 若 `!cell.isLeftHeadOf(head1)` → 返回 `head1`（找到真正的头）；
3. 否则沿 `getCellLeftHead()` 链用 `Stack` 走一遍，若**走回自己**（`cell == head2`）→
   构造 `IIllIIIIIIIIIlll(cell, 3)` 并抛 `ReportError`——**这就是循环引用的检测点之一**
   （配套 `CircularRefException.java`）；
4. 未命中左邻则从 `col-1` 向左线性扫，跳过条件：
   `head12.isMerged() && 18 != extend && head12.getRow() != xrow`。

> 步骤 3 的「沿链走回自己」是 M2-07（循环引用必须返回错误、不得死循环）的**原型语义**：
> Java 靠对象同一性，Rust 侧用 `CellId` 相等即可等价实现。

---

## 附件 H：求值与分页的实测契约（2026-10-12，消歧实验产出）

> 证据：`tests/fixtures/probe_disambig_out.txt` / `probe_extend_out.txt` / `RUNLOG.md` §10-§11，
> 基准夹具 `engine_sum.rpx` / `engine_ext.rpx` / `engine_expr.rpx` / `engine_pager.rpx`（均 `--verify` PASS）。
> **本附件推翻了「sum 是区域聚合」的直觉，也推翻了「extendMode=16 不扩展」的直觉。**

### H.1 求值语义三条规则（**不是 Excel 语义**）

受控输入（无数据集、无扩展；字面量格 B2=100 / B3=200 / B4=150）实测：

| 公式 | 实测结果 | 结论 |
|------|----------|------|
| `sum(B2:B4)` | **100**（= B2，即操作数 1） | **区域不聚合** |
| `sum(B3:B4)` | **200**（= B3） | 同上，只取操作数 1 |
| `sum(B2)` | 100 | 单操作数透传 |
| `sum(B2:B4, value())` / `sum(value(), B2:B4)` / `sum(B2, B2:B4)` / `sum(B2:B4, B2)` | 全部 `ReportError` | **两操作数形式非法** |
| `sum(...)` 于全 0 输入 | `Double(0.0)` | **函数确实执行**（回退路径给 `null`，可据此区分） |
| `ds1.sum(qty)` | **450** | 操作数求值为 **List** 时才聚合 |
| `sum(ds1.select(qty))` | **450** | 同上 |
| `ds1.avg(qty)` / `ds1.max(qty)` / `ds1.count()` | 150.0 / 200 / 3 | List 路径正常 |

**机制**：`Sum.java:104-106` —— `:` 让参数成为复合参数 → `isLeaf()` 为 false → 走 cellSet 分支而非聚合分支；
`_$1(ecs, cellSet, param2, ctx)`（`Sum.java:174-190`）把**操作数 1 当逐格表达式、操作数 2 当 cellSet**，
cellSet 是单个 `ExtCell` 时只求值一次。即 **`sum(A:B)` ≡「把 B 设为 current 后求值 A 一次」**。
聚合路径要求操作数求值为 `List`（`Sum.java:112-121`）。

> ⛔ **M2 硬约束**：**禁止实现 Excel 式区域求和**。判据即上表：`sum(B2:B4)` 必须等于 100 而不是 450。

> ⚠ 本表的"450"是「无扩展格引用该数据集」这一受控输入下的值。工程夹具 `engine_sum`
> 里同名语义由 `ds2.sum(qty)`=450（未被引用）与 `ds1.sum(qty)`=100/200/150（被引用）承载；
> 机制见 `docs/m2-engine-design.md` **§5.2**。⛔ 别把 450 当成 `ds1.sum(qty)` 的固定值。

### H.2 数据集是否被扩展格引用会改变聚合语义

`engine_sum` 用双数据集把两种行为同时钉死（逐行数值均为实测）：

| 列 | 是否在扩展区引用 | 逐行结果 |
|----|------------------|----------|
| `ds1.sum(qty)` | **是**（被扩展格引用） | 100 / 200 / 150（随行变化） |
| `sum(ds1.select(qty))` | 是 | 100 / 200 / 150 |
| `ds2.sum(qty)` | **否**（不在扩展中） | **450 / 450 / 450**（全量聚合） |
| `ds2.count()` | 否 | 3 / 3 / 3 |

→ 聚合结果取决于**数据集与扩展格的引用关系**，不是单纯按表达式求值。实现时必须保留这一上下文。

> ⚠ 机制根源见 `docs/m2-engine-design.md` **§5.2**（`ds.current` 生命周期）：被扩展格引用的数据集，
> 其 `current` 被**扩展机灌成 `Row`**（沿 head 链行走，§5.2.3）⇒ `ds1.sum(qty)` 逐行退化；未被引用的停在 RootGroup ⇒ 全量 450。
> ⛔ 推进 `ds.current` 的是**扩展格的 head 链行走**，不得写成「MainCell（`_$19`）是 `ds.current` 的推进器」之类旧说法；若见即删。

### H.3 类型即契约

产品返回 `Integer(100)` 时 expected.json 写 `100`，写 `100.0` 即不符。
⛔ **M2 硬约束**：求值结果**不得统一提升为 f64**；`Integer` / `Long` / `Double` / `String` / `List`
必须原样区分（M1 的 `ObjValue` 已有对应变体，M2 沿用）。`engine_*.expected.json` 的 `type` 字段
（`int`/`double`/`string`/`list`）就是断言依据。

### H.4 extendMode 是**运行时改写**，不是按声明值扩展

`ExtExtendCell.prepareCalculate:314-328` 按 `leftHead`/`topHead` 拓扑把声明值**改写**为实际生效值：

| 声明 | 计算态实际生效 | 计算态矩阵 | 锚点 subCells | 值分布 |
|------|----------------|------------|---------------|--------|
| 16 DEFAULT | **18（自动升为竖向）** | 6 行 | **5** | R1..R5 每行一个 |
| 18 VERTICAL | 18 | 6 行 | 5 | R1..R5 每行一个 |
| 17 NONE | 17 | 6 行（由其他列驱动） | **1** | **`ArrayList([R1..R5])` 整塞一格** |
| 19 HORIZONTAL | 19 | 6 行 × **7 列** | 5 | 横向铺开 |

- **extend=18 行数公式**：`计算态行数 = 模板行数 + (数据集行数 − 1)`。锚点占第 1 个数据行，
  其余 N−1 行由 `extendRow` 插入（`engine_ext` 实测 3 行模板 + 4 行数据 → 7×4）。
- **横向边界**：单独一行用 19 时列数恒增 N−1（3 列模板第 1 列 → 7 列）；但 19 与 16/18 **同行**时
  列数不增——竖向扩展已把该行登记为可扩展行，横向扩展被吞。
- ⛔ **M2 硬约束**：必须复刻 `leftHead`/`topHead` 拓扑判定，否则「声明 16」与「声明 18」的报表行为分叉。
- 旁证：17 的值是 `ArrayList` → 数据集列在非扩展格里的形态是**整列 List**，与 H.1/H.2 的 List 判定同源。

#### H.4 补遗（v0.5）：17/18/19 必须在**不同行**才可区分

上表来自旧 `engine_ext`（四个 extendMode 声明挤在**同一行**）。那个布局**无法区分 17/18/19**——
竖向扩展把整条行带登记为可扩展行，右侧三列全退化成 `sub_cells=1`，实测 extend 分布
`{16:8, 17:1, 18:2, 19:1}` 但**行为完全一致**。重排成「一行一个模式」（`engine_ext` v0.5，模板 4×3、数据 5 行）后：

| 位置 | 声明 | 计算态 `extend` | `sub_cells` | 值 / 子格位置 | 结论 |
|------|------|-----------------|-------------|---------------|------|
| `(2,1)` | 17 NONE | 17 | **1** | 单格 `ArrayList(["R1".."R5"])`，`type=list` | **不展开，整列收成一个 List** |
| `(3,1)` | 18 VERTICAL | 18 | **5** | 子格 `(3,1)(4,1)(5,1)(6,1)(7,1)` = R1..R5 | 纵向铺开 |
| `(4,1)` | 19 HORIZONTAL | 19 | **5** | 子格 `(8,1)(8,2)(8,3)(8,4)(8,5)` = R1..R5 | 横向铺开 |

- **19 的副作用（重要）**：横向扩展会把该**列结构本身放大 N 倍**（本夹具 1 列 → 5 列），
  放大出的兄弟列记 `col_merge=5`；计算态网格因此是 **8×7**（7 = 5 + 2），不是 8×3。
  「19 只影响单元格、不影响列结构」是错的。
- ⛔ **M2 判定硬约束**：M2-15 只能用重排后的夹具。拿旧布局做 golden，会放过
  「把 17/18/19 当成同一种处理」的实现。

### H.5 分页基准（**已跑通**）

链路：`ReportUtils2.getPagerInfo(rpt, 595, 842, 1)` → `PageBuilder.createPages()` → `getAllPages()`。

`engine_pager`：40 数据行、行高 20、`unit=1`（毫米）、A4 595×842pt、独立 `-90` 分组行 →
`page_count=4`、`x_pages=1`、`y_pages=4`，**每页 14 行**（= 297mm / 20mm）：

| 页 | 行数 | 首值 | 末值 |
|----|------|------|------|
| P1 | 14 | `region`（表头） | R12 |
| P2 | 14 | `group: R*` | R25 |
| P3 | 14 | `group: R*` | R38 |
| P4 | 4 | `group: R*` | R40 |

| 契约点 | 规则 |
|--------|------|
| 每页行数 | **按页高 / 行高累加得出**，不用 `row_num_per_page` |
| `row_num_per_page` | expected.json 落盘值 **20**，但真值是 14 → 它只是 **propMap 槽 33 的提示值，不参与切页** |
| 分组头 | 独立 `-90` 行**每页重复**且**计入页高**（P2/P3/P4 首值均为 `group: R*`） |
| ⚠ `-90` 的坑 | 把 `-90` 打在**会扩展的锚点行**上 → 扩展把 `-90` 复制到每个插入行 → 整块成分组头 → `SimplePager.java:261` 取到越界 `getRowCell` → **NPE**。分组头必须用独立行 |
| 横向分页 | 已入基准：`engine_sum` 9 列超页宽 → `page_count=2 / x_pages=2 / y_pages=1`（单栏 `columns=1`）|
| 行数口径 | `pages[].rows` **计入重复的分组头行**：14+14+14+4 = **46 = 计算态 43 行 + 分组头重复 3 行**（P2/P3/P4 各 1）。P4 的 `col1_values` 只有 3 项（`group: R*`/R39/R40），因末行为空不计数 —— 这是 `rows` 与 `col1_values` 长度不等的原因，**不是缺陷** |

### H.6 行类型常量（`rowType`，级别 1..20）

`TITLE_HEADER=-96` / `TABLE_HEADER=-95` / `TABLE_FOOTER=-94` / `PAGE_HEADER=-93` /
`PAGE_FOOTER=-92` / `NORMAL=-91` / **`GROUP_HEADER=-90`**

> 旁证一致：`NORMAL=-91` 与 M1 `props.rs::def::ROW_TYPE = -91` 相符（附件 B.1 槽 1）。
> 级别 1..20 对应 M1 RowCell 槽 7 的 `groupHeaderLevel`（附件 E.7）。

### H.7 合并的两种来源（M2-04 模板合并 / M2-08 mergeSame）—— 2026-10-12 实测

**结论一句话**：模板合并区在扩展中**跨度保留在锚点上**；而同值合并（`mergeSame`）**只在分页阶段发生**，
`Engine.calc()` 的输出里永远看不到。

#### H.7.1 模板合并 × 扩展（M2-04，`engine_merge`）

模板 4×4，`A2` 合并 `A2:A3` 且 `extend=18`，`C2` 合并 `C2:D2`：

| 观测 | 值 |
|------|-----|
| 计算态网格 | **8×4**（模板 4 行 + 3 行数据 → 每个实例占 **2 行**） |
| 锚点 `(2,1)` | `row_merge=2`、`col_merge=1`、`extend=18`、`sub_cells=3` |
| 纵向跨度在扩展中的表现 | 行 2/3 = North、4/5 = East、6/7 = South —— **每个实例各占 2 行**，`merged` 段逐对记录 `row_merge=2` |
| 横向合并 | `(2,3)`/`(2,4)` 均 `col_merge=2`，**复制到每一个扩展实例** |

> 契约：合并跨度**不因扩展而收缩或分摊**；扩展插入的是**整个合并块**（块高 = `row_merge`），
> 所以 `计算态行数 = 模板行数 + (数据集行数 − 1) × row_merge`（本例 4 + 2×2 = 8 ✅）。

#### H.7.2 同值合并 `mergeSame`（M2-08，`engine_samemerge`）

⚠ **最关键的一条**：`ExtCellSet.mergeSame()` **不在计算链路里**。全量 grep 的调用方只有两处：

```
SimplePager.java:133   ecs.mergeSame();     <- 单页分支
SimplePager.java:179   ecs.mergeSame();     <- 多页分支，对每页克隆调用
CalcSubRpt.java:138    ecs.mergeSame();     <- 子报表
```

因此**必须在 `PageBuilder.createPages()` 之后再读网格**。实测（列 A 值 `A,A,B,B,C`，`extend=18`，槽 89 = 98）：

| 阶段 | `(2,1)` | `(4,1)` | `(6,1)` |
|------|---------|---------|---------|
| `Engine.calc()` 原始网格 | rm=1 cm=1 | rm=1 cm=1 | rm=1 cm=1 |
| `createPages()` 之后 | **rm=2** cm=1 | **rm=2** cm=1 | rm=1 cm=1 |

| 属性 | 槽位 | 默认 | 码值 |
|------|------|------|------|
| `mergeSame` | 89 | **97 = 关** | **98 = 纵向合并 / 99 = 横向合并 / 100 = 双向** |
| `mergeNull` | 90 | 113 | 空值合并（同结构） |
| `mergeSameMode` | 91 | 118 | 119 / 120 改变基准格的取法；`getMergeSameMode()==120` 时纵向分支被跳过 |

- 判定式（`ExtCellSet.java:1856,1955`）：`cell.getMergeSame() != 98 && != 100` → 跳过（纵向分支）；
  横向分支用 `!= 99 && != 100`。
- 被合并的格**都报整块跨度**（回看 `merged` 段：r=2 与 r=3 都写 `row_merge=2`），
  与 M1「合并区非锚点槽持锚点 id」的编码一致。
- ⛔ **M2 硬约束**：不得把 `mergeSame` 实现在计算阶段；也不得只实现 98 而忽略 99/100 与 mode 的两个基准分支。

### H.8 循环引用是**引擎自身**检出（M2-07，`engine_cycle`）

早先只看到 `leftHead` 链上的栈检测（附件 G.3）。实测表明**表达式成环同样被检出**，且不止一种形态：

| 构造 | 结果 |
|------|------|
| `B2 = C2`、`C2 = B2`（互引用） | `ReportError`，cause `IIllIIIIIIIIIlll`，链 `B2-C2-B2-C2` |
| `B2 = B2`（自引用） | `ReportError`，链 `B2-B2-B2` |
| `leftHead(B2)=C2`、`leftHead(C2)=B2`（主格链成环） | `ReportError`，链 `C2-B2-C2` |

- 异常形状：`com.raqsoft.commonl.ReportError`，`getCause()` 是
  `com.raqsoft.reportl.model.engine.IIllIIIIIIIIIlll`（诊断器），其 message 里带**引用链字符串**。
- message 全文（UTF-8，`engine_cycle.expected.json` 落盘值）：
  `: 单元格：C2中有错误，B2-C2-B2-C2存在循环引用\n错误来源：B2-C2-B2-C2存在循环引用`
- ⛔ **M2 硬约束**：成环必须**返回引擎错误**，不得 panic、不得死循环；
  对拍断言应取 `chain`（`B2-C2-B2-C2`）而不是整个本地化消息。

---

## 附件 I：扩展算法逐行契约（`extendRow` / `extendCol`）

> **本节是算法正文，不是摘要。** `crates/rpx-engine` 的实现按本节逐条落，代码里只留指向本节的引用。
> 出处：反编译源码 `re_work/src/reportlite/03-model/com/raqsoft/reportl/model/engine/`
> 下 `ExtCellSet.java` / `ExtExtendCell.java` / `ExtNormalCell.java` / `ExtCell.java`，**行号为该文件实际行号**。
> 状态：【已核实】 = 已读源码逐行确认；【待验证】 = 有源码依据但未用夹具逐格验证。

### I.1 唯一调用点与 `count` 的来源 【已核实】

`extendRow` / `extendCol` 在整个产品里**只有一处调用**：`ExtExtendCell.calculate`（`ExtExtendCell.java:426-430`）。

```java
if ((result = ((Expression)this.engine.expression).calculate(ctx)) instanceof List) {  // :376
    List list = (List)result;
    int len = list.size();                                                             // :379-380
    if (len == 0) { this.setValue(null); return; }                                     // :381-384
    ...逐元素 setValue + 记 valList / dispValList / formatList...                        // :385-424
    byte extend = this.getExtensible();                                                // :425
    if (18 == extend) this.engine.cs.extendRow(this, len);                             // :426-427
    else              this.engine.cs.extendCol(this, len);                             // :428-429
    for (i = 0; i < len; ++i) { cell = (ExtExtendCell)this._$7[i]; cell.setValue(valList[i]); ... }  // :431+
}
```

四条**硬约束**：

| # | 约束 | 依据 |
|---|------|------|
| I.1a | `count` = **表达式结果 List 的 `size()`**，**不是**「数据集行数」 | `:379-380` |
| I.1b | 结果**不是 `List`** ⇒ 完全不扩展 | `:376` 的 `instanceof List` |
| I.1c | `len == 0` ⇒ **不调用** `extendRow/Col`，锚点 `setValue(null)` 后 `return` | `:381-384` |
| I.1d | `18 → extendRow`；**其余（19，若出现 17 也走这里）→ extendCol** | `:425-429` |

- I.1c 同时解释了为什么 `extendRow` 里可以放心写 `extCells[0] = cell`：`count ≥ 1` 由上游保证。
  **直接调 `extendRow(cell, 0)` 会 `ArrayIndexOutOfBoundsException`**（`new ExtCell[0]` 后给下标 0 赋值，
  `ExtCellSet.java:994-995` / `:1094-1095`）。Rust 侧**不得**继承这个假设 —— 上游是我们自己的代码，
  应对 `count == 0` 显式返回错误或空结果，而不是跟着 panic。
- ⛔ 行数公式里的「数据集行数」严格来说应读作**「锚点表达式返回的 List 长度」**。
  `ds1.select(region)` 场景下二者恰好相等，带条件/多列的 `select` 则不等。
- `i` 的循环体还负责把 `valList[i]` 逐个写回 `this._$7[i]`（即 `extCells[]` 的每个实例），
  并在写回前 `cs.setCurrent(cell)` —— **值的填充发生在扩展之后**，扩展本身只负责"造格与连杆"。

### I.2 包络（`rows` / `cols` 从哪来）【已核实】

| 量 | 方法 | 定义 | 递归方向 |
|----|------|------|----------|
| `beginRow` | `_$4(cell)` `:928-939` | `min(cell.getRow(), min over leftSubCell 递归)` | **左**子格链 |
| `endRow` | `_$3(cell)` `:940-951` | `max(cell.getRow()+getRowMerge(), max over leftSubCell 递归)` | **左**子格链 |
| `beginCol` | `_$2(cell)` `:1059-1070` | `min(cell.getCol(), min over topSubCell 递归)` | **上**子格链 |
| `endCol` | `_$1(cell)` `:1071-1082` | `max(cell.getCol()+getColMerge(), max over topSubCell 递归)` | **上**子格链 |

```
rows = endRow - beginRow      // 半开区间 [beginRow, endRow)
cols = endCol - beginCol      // 半开区间 [beginCol, endCol)
```

⛔ **行包络沿 `leftSubCell` 递归、列包络沿 `topSubCell` 递归** —— 方向搞反会得到错的带高。
（直觉：纵向扩展的作用域由"左 head 链"决定，横向由"上 head 链"决定。）

⚠ 四个方法都**先取自己的坐标，再取子格坐标求 min/max**，所以单格无子格时包络 = 自身（`rows = rowMerge`、`cols = colMerge`）。

### I.3 插行 / 插列 【已核实】

**插行 `_$3(int row, int count)` `:809-830`**
```
_$26.insertRows(row, count);            // 计算矩阵行数 += count
_$16 += count;                          // 行计数（含 0 号脚手架）
for j = 0 .. icol-1:
  for i = row+count .. irow-1:
     cell = _$26.get(i, j)
     if cell == null || cell.getRow() < row: continue
     if i == cell.getRow() + count && j == cell.getCol():   // 恰好是"主格自己的新位置"
         cell.addRow(count)                                  // ← 只平移主格自身
     if cell.getRow() + cell.getRowMerge() - 1 <= i: continue // ← 合并块内非锚点槽：跳过
     i = cell.getRow() + cell.getRowMerge() - 1
```
⇒ **只有主格自身的 `row` 被平移**；合并块内其余槽位靠"跳块"跳过（它们不是主格）。

**插列 `_$2(int col, int count)` `:876-878`，转调 `_$1(col, count, _$7(), _$6(), true)` `:902-926`**
```
_$26.insertCols(col, count, begin, end, alignColHead)
_$15 += count;
if alignColHead: for j = col+count .. icol-1: get(0,j).addCol(count)   // 第 0 行列头脚手架
for i = begin .. end:                                                  // 只在"行头行"区间内
   for j = col+count .. icol-1: 主格自身 → addCol(count)
```
配套三个判定：
- `_$7()` `:858-865` = **第一个**"行头行"（从 `i=1` 起向后扫）
- `_$6()` `:867-874` = **最后一个**"行头行"（从末尾向前扫）
- `_$2(int row)` `:846-857` = 行头判定：`row == 0` → `true`；`get(row,0) == null` → `true`；
  否则 `rowType ∈ {-91, -95, -94, -90}` → `true`
  ⇒ **「行头行」集合 = {0} ∪ {rowType ∈ {-91, -95, -94, -90}}**

⛔ **`_$n` 存在多重载，且语义互不相干 —— 这是最容易读错源码的地方**：

| 方法 | 重载 | 含义 | 出处 |
|---|---|---|---|
| `_$2` | `(int row)` | `boolean` **行头判定** | `:846-857` |
| `_$2` | `(int col, int count)` | **插列** | `:876-878` |
| `_$2` | `(ExtNormalCell)` | `ExtCell` **找上 head**（= 设计文档 §4.2 的 `find_top_head`） | `:668-705` |
| `_$2` | `(ExtCell)` | `int` **包络起始列** | `:1059-1070` |
| `_$3` | `(int row, int count)` | **插行** | `:809-830` |
| `_$3` | `(int row)` | **删行** | `:443-469` |
| `_$3` | `(ExtNormalCell)` | `ExtCell` **找左 head**（= §4.2 的 `find_left_head`） | `:631-667` |
| `_$3` | `(ExtCell)` | `int` **包络结束行** | `:940-951` |
| `_$1` | `(int col,int count)` / `(int,int,int,int,boolean)` | **插列**（后者带行范围与列头对齐） | `:880-901` / `:902-926` |
| `_$1` | `(int col)` | **删列** | `:952-980` |
| `_$1` | `(ExtCell)` | `int` **包络结束列** | `:1071-1082` |

⚠ **`ExtNormalCell` 是 `ExtCell` 的子类**，所以 `_$3(x)` 的实参静态类型决定调哪一支：
`_$3(ExtNormalCell)` 命中"找左 head"，`_$3(ExtCell)` 命中"包络结束行"。**两者只差一个形参类型，
行为完全不同**（一个返回 `ExtCell`，一个返回 `int`）。在 `extendRow`/`extendCol` 内部，形参声明为
`ExtCell cell`，所以 `_$4(cell)`/`_$3(cell)`/`_$2(cell)`/`_$1(cell)` 走的都是**包络**那一支 —— 别被同名的
head 查找方法带跑。

⚠ 还需注意：`_$4(cell)` / `_$3(cell)` / `_$2(cell)` / `_$1(cell)` 内部递归时传的是
`getLeftSubCell(i)` / `getTopSubCell(i)` 的返回值（静态类型 `ExtCell`），所以递归也走包络支。**一致。**

### I.4 `_$8()` 是**中断检查**，不是矩阵操作 【已核实】

`:233-237`：`if (this._$11) throw new ReportError("Calculation is interrupted!");`
纯 cancel 钩子，`extendRow`/`extendCol` 里每轮循环都调它。
Rust 侧对应「每轮检查一次取消标志」的等价物，**不要**给它编造矩阵语义。

### I.5 `extendRow(cell, count)` 两趟 【已核实】

```
beginRow = _$4(cell); endRow = _$3(cell); rows = endRow - beginRow;             // :991-993
extCells = new ExtCell[count]; extCells[0] = cell; cell.setExtCells(extCells);   // :994-996
_$3(endRow, rows * (count - 1));                                               // :997 ① 在 endRow 处插 rows*(count-1) 行
cell.expandLeftHead(rows * (count - 1));                                       // :998 ② 沿左 head 链撑大 rowMerge
icol = _$26.getColSize();                                                      // :999

// 第 1 趟（:1000-1029）：克隆
for m = 1 .. count-1:
  _$8();
  for i = beginRow .. endRow-1:
     k = m*rows; x = i + k;
     cells1 = _$26.getRow(i); cells2 = _$26.getRow(x);
     cells2[0] = clone(cells1[0]); cells2[0].addRow(k);      // ← 只克隆第 0 列脚手架（每带行一格）
     for j = 1 .. icol-1:
        sub = cells1[j]; if sub == null: continue
        if sub.getRow()==i && sub.getCol()==j && cell.isLeftHeadOf(sub):
            newSub = clone(sub as ExtNormalCell); newSub.addRow(k);
            铺满 [x, x+getRowMerge()) × [j, j+getColMerge()) ← newSub     // ← 合并块整体铺
            if sub == cell: extCells[m] = newSub; newSub.setExtCells(extCells)
        j += sub.getColMerge() - 1                                        // ← 跳块

// 第 2 趟（:1030-1056）：修 head 指针
for m = 1 .. count-1:
  _$8();
  for i,j 同上（条件同）:
     newSub = cells2[j]; head = sub.getCellLeftHead();
     if sub != cell: newSub.setCellLeftHead(_$26.get(head.getRow()+k, head.getCol()))  // ← 平移
     if (top = sub.getCellTopHead()) != _$10: newSub.setCellTopHead(top)              // ← 原样共享
```

三条易错点：

1. **克隆判据 `cell.isLeftHeadOf(sub)` 是自含的（reflexive）**：`isLeftHeadOf` 从 `sub` **自身**起步沿
   `leftHead` 链上溯（`ExtNormalCell.java:391-398`）：`for (head = sub; head != null; head = head.getCellLeftHead())`。
   故 `cell.isLeftHeadOf(cell) == true` —— 锚点自己天然被复制，这正是 `if (sub == cell)` 分支能命中的原因。
2. **只克隆"左 head 链覆盖到"的格，不是整条行带**。带内其它格（以及 `sub.getRow() != i` 的合并槽位）不复制，
   合并区靠第 1 趟的双循环 `set(r,c,newSub)` **整块铺满**补回。
3. **第 2 趟只平移 `leftHead`；`topHead` 原样共享**（除非它等于 root `_$10` 则不动）。
   纵向复制时"本格上方那格"不变 —— 与"行扩展沿列方向堆叠"的直觉一致。

### I.6 `extendCol(cell, count)` 两趟 【已核实】

```
if (!_$2(cell.getRow()))
    throw new ReportError(EngineMessage.get().getMessage("ExtCellSet.entendCol"));  // :1087-1090 前置校验
beginCol = _$2(cell); endCol = _$1(cell); cols = endCol - beginCol;                 // :1091-1093
extCells = new ExtCell[count]; extCells[0] = cell; cell.setExtCells(extCells);      // :1094-1096
_$2(endCol, cols * (count - 1));                                                    // :1097 ① 插列（走 _$7()/_$6() 界定行范围）
for j2 = beginCol .. endCol-1:                                                      // :1098-1104 ② 显式克隆第 0 行（列头）
   sub2 = _$26.get(0, j2);
   for m = 1 .. count-1: cl = clone(sub2); cl.addCol(m*cols); _$26.set(0, j2+m*cols, cl)
cell.expandTopHead(cols * (count - 1));                                             // :1105
irow = _$26.getRowSize();                                                           // :1106 循环外取一次（插列不改行数）

// 第 1 趟（:1107-1140）：克隆
for i = 1 .. irow-1:
  _$8();
  for j = beginCol .. endCol-1:
     sub = _$26.get(i, j); if sub == null: continue
     if sub.getRow()==i && sub.getCol()==j && cell.isTopHeadOf(sub):
        for m = 1 .. count-1:
           newSub = clone(sub); newCol = m*cols + j; newSub.addCol(m*cols);
           铺满 [i, i+getRowMerge()) × [newCol, newCol+getColMerge()) ← newSub
           if sub == cell: extCells[m] = newSub; newSub.setExtCells(extCells)
           if sub.getCellLeftHead() == _$10: _$10._$6.add(newSub)   // ← 登记到 root 的字段 _$6
     j += sub.getColMerge() - 1

// 第 2 趟（:1141-1164）：修 head
for i = 1 .. irow-1:
  _$8();
  for j 同上条件:
     left = sub.getCellLeftHead(); top = sub.getCellTopHead();
     for m = 1 .. count-1:
        newSub = _$26.get(i, j + cols*m);
        if sub != cell: newSub.setCellTopHead(_$26.get(top.getRow(), top.getCol()+cols*m))  // ← 平移
        if left == _$10: continue
        lcol = left.getCol();
        if beginCol <= lcol && lcol < endCol: newSub.setCellLeftHead(_$26.get(left.getRow(), lcol+cols*m))
        else:                                 newSub.setCellLeftHead(left)                   // ← 带外原样
```

四条易错点：

1. **前置校验**：只有"行头行"（见 I.3）上的格才允许列扩展，否则抛 `ReportError`，消息键
   **`ExtCellSet.entendCol`** —— ⚠ 产品把 `extend` 拼错成了 `entend`，**逐字照抄，别"纠正"**。
   这是继循环引用（附件 H.8）之后**第二条可断言的错误路径**，且目前没有夹具覆盖（⚠ 建议补）。
2. **第 0 行列头是显式逐个克隆的**，与 `extendRow` 里 `cells2[0]` 那一行等价（行/列脚手架必须同步长出去）。
3. 第 1 趟里 `sub.getCellLeftHead() == root` 的格会 `root._$6.add(newSub)` ——
   ⚠ `_$6` 在这里是 **root 的私有字段 `leftSubs`**（左 head 为其自身的格列表），
   与同名方法 `_$6()`（找最后一行头行）**不是一回事**。root 的另一个私有字段 `_$5` = `topSubs`。
   `engine_ext` 实测 `cell00._$5` 有 **12** 个成员（`ProbeHeads`，见设计文档 §12.2），
   而 `extend_col` 的**施力点正是 `cell00._$5` 中 `containCol(anchor)` 的成员** ——
   ⛔ 写成"沿锚点的 head 链给每个 head `addColMerge`"会得到与夹具**完全相反**的 `col_merge`
   （设计文档 §12.1 易错点 2）。这两个字段**没有公开 API**，探针靠反射读取。
4. 第 2 趟 `leftHead` 分三支：`root` → 不动；**带内** → 平移；**带外** → 原样保留。
   而 `extendRow` 的第 2 趟没有"带内/带外"之分，直接平移。**这是两个方向最容易写错的不对称点。**

### I.7 对称性总表（实现时对照）【已核实】

| | `extendRow`（纵向，18） | `extendCol`（横向，19） |
|---|---|---|
| 前置校验 | 无 | `_$2(cell.getRow())`，否则 `ReportError("ExtCellSet.entendCol")` |
| 包络 | `_$4(cell)` / `_$3(cell)`，沿 **leftSub** 递归 | `_$2(cell)` / `_$1(cell)`，沿 **topSub** 递归 |
| 插入 | `_$3(endRow, rows*(count-1))` | `_$2(endCol, cols*(count-1))` |
| 脚手架克隆 | 行内 `cells2[0]`（**单格**，每带行一格） | **显式 for 循环**克隆第 0 行的每个带列 |
| `expand*Head` | `expandLeftHead(rows*(count-1))` | `expandTopHead(cols*(count-1))` |
| 复制判据 | `cell.isLeftHeadOf(sub)`（自含） | `cell.isTopHeadOf(sub)`（自含） |
| 复制坐标 | `i+k, j`（`k = m*rows`） | `i, j+cols*m` |
| 第 2 趟 leftHead | **直接平移**（`sub == cell` 除外） | **带内平移 / 带外原样 / root 不动** |
| 第 2 趟 topHead | **原样共享**（root 除外） | **平移** |
| 额外登记 | 无 | `leftHead == root` ⇒ `root._$6.add(newSub)` |

### I.8 `expandLeftHead` / `expandTopHead` —— M2-04 块高的来源 【已核实】

`ExtNormalCell.java:463-490`：
```java
public void expandLeftHead(int mergeInc) {
    ExtCell sub = this;
    for (ExtCell head = this.leftHead; head != null; head = head.getCellLeftHead()) {
        head.expandLeftHead(sub, this, mergeInc);            // 多态钩子，ExtExtendCell.java:214-226 覆写
        if (head.containRow(this)) {
            head.addRowMerge(mergeInc);
            this.engine.cs._$1(head.getRow(), head.getCol(),
                               head.getRow()+head.getRowMerge(),
                               head.getCol()+head.getColMerge(), head);   // 重铺合并块
        }
        sub = head;
    }
}
// expandTopHead 完全对称：沿 topHead 链，用 containCol / addColMerge
```
⇒ 这是 **M2-04「模板合并区跨进扩展行」里锚点 `row_merge` 变大的机制**：
纵向扩展会让**沿左 head 链、且 `containRow(this)` 成立**的每个 head 的 `row_merge += rows*(count-1)`，
并立即把该合并块在新坐标上重铺一遍。

⚠ **尚未验证**：`ExtExtendCell` 覆写的 `expandLeftHead`（`:214-226`）是**空实现**还是另做处理，本附件未读完；
且 `rows*(count-1)` 与夹具观测的 `row_merge=2` 之间的确切对应关系需要 dump `leftHead` 链才能定案。
在 `engine_merge` 上逐格验证之前，**本条只作实现指引，不得当结论引用**。

### I.9 源坐标语义：`source.src_row` 是**模板态坐标** ⛔ 不可当计算态行号

| 事实 | 证据 |
| ---- | ---- |
| `engine.sourceRow` / `sourceCol` **只在构造时写一次**；此后扩展**只改 `cell.row`，从不改 `engine.sourceRow`** | `ExtNormalCell.java:127`、`ExtRowCell.java:58`（`this.engine.sourceRow = row`）；`extendRow/extendCol` 调的是 `addRow(k)` / `addCol(m*cols)`（附件 I.5/I.6） |
| 扩展克隆出的实例**共享同一 `sourceRow`** ⇒ 同一模板格的 N 个实例 `src_row` 相同 | 实测 `engine_ext` 横向 19：5 个实例 `src_row` 全 = 4 |
| 子报表传播时 `sourceRow` 是**从源引擎拷贝**，仍是模板坐标 | `SubRpt.java:144`、`lIIlIIIIIIllIlIl.java:126` |
| `source != null` ⟺ `r ≤ template_rows 且 c ≤ template_cols`（两张矩阵的分工） | `ExtCellSet.java:623-629`（与 `docs/spec-m2.md` §7 一致） |

**实测补充**（`ProbeMergePages` / `EngineModelsHead`，12 夹具全覆盖；`tests/fixtures/RUNLOG.md` §19.5，backend-2 + team-lead）：

| 方向 | 样本 | `src_row` | 计算态行带 |
| ---- | ---- | --------- | ---------- |
| 纵向 18 | `engine_head_left` 锚点 1/2/3 | 2 / 3 / 4（**模板行**） | `[2,3,4]` / `[5,6,7]` / `[8,9,10]`（依次下推） |
| 横向 19 | `engine_ext` | 4 | `[8,8,8,8,8]` |
| 横向 19 | `engine_head_top` | 1 | 行 = 1（留在模板行，**与上一条矛盾**） |

⇒ **横向（19）的计算态行号归属未验证**，两样本互相矛盾 ⇒ **不要试图拟合**，只钉「同属一行 + 列走向」。

**三条硬约束**

1. ⛔ **不得**用 `src_row` 索引计算态网格；
2. ⛔ **不得**用 `src_row` 反查模板行——同一模板行的多个实例 `src_row` 相同，反查**不唯一**（这是「⛔ 不能靠 `source.src_row` 反查模板行」的同源陷阱）；
3. 断言 `calculated` 段时，`src_row` / `src_col` 的期望值是**模板态坐标**，必须与**模板网格**比对，不能与计算态网格比对。

### I.10 本节未覆盖 / 需在实现时补验证

| 项 | 说明 |
|---|---|
| `insertRows` / `insertCols` 的矩阵层语义 | 属 `Matrix`/`ExtMatrix`（M1 范围外）；本节只依赖"行/列数 +count、坐标平移"的外部行为 |
| `_$16` / `_$15` | 行/列计数累加器，语义 = **含 0 号脚手架**的总行/列数；与 `getRowCount()`（= `getRowSize()-1`）的口径差 1 |
| `ExtExtendCell` 覆写的 `expandLeftHead`/`expandTopHead` 行为 | 未读（见 I.8） |
| `containRow` / `containCol` | 未读；实现前须确认是"行区间相交"还是"含锚点" |
| `extendCol` 的前置校验错误路径 | **无夹具**。建议补一个夹具：在非行头行上放 `extend=19` 的格 ⇒ 断言 `ReportError` + 消息键 |
| `count == 0` 的 Rust 侧行为 | 产品靠上游 `return` 回避；Rust 侧须自行定义（建议返回错误或空结果），不可 panic |

---

## 附件 J：分组头（`-90`）与动态分组的真实机制（2026-10-12，探针实测）

> 探针 `tests/fixtures/ProbeGroup.java`（S1–S8 场景），输出 `probe_group_out.txt`。
> 本节**推翻了**此前 spec §8 里「引擎按分组键动态生成 `-90` 行」的表述。

### J.1 判决：`-90` 是**纯静态模板属性**，引擎从不合成它

`-90` = `IRowCell.TYPE_GROUP_HEADER`（`IRowCell.java:21`）。它只在**构造期**被读一次：

```java
// ExtCellSet 构造器，:151-159
if (cell.getRowType() == -90) { this._$12 = (short)(this._$12 | 2); }
// 结果：hasGroupHeader()  (_$12 & 2)   :3689-3691
```

全量 grep 证据：`ExtCellSet` 里**没有任何**「按分组键插入行并置 `-90`」的代码；
唯一的行插入是 `extendRow` 走的 `_$3(endRow, count)` → `_$26.insertRows`（`:809-830`）。

### J.2 模板 rowTypes ↔ 计算态 rowTypes 对拍（决定性证据）

| 场景 | 构造 | 模板 rowTypes | 计算态 | `minus90` | 分页 |
|------|------|--------------|--------|-----------|------|
| **S1** | `-90` 行挂 `ds1.group(region)` | `{-91,-90,-91}` | `rows=6` | **3** | — |
| **S3** | `-90` 行挂**字面量**（无 group） | `{-91,-90,-91}` | `rows=6` | **3** | — |
| **S2** | 普通行挂 `ds1.group(region)`（对照） | `{-91,-91,-91}` | `rows=6` | **0** | — |
| **S6** | `group()` 放第 3 行 | `{-91,-91,-91}` | `rows=8` | **0** | — |

⇒ **S1 ≡ S3**：有没有 `group()`，`-90` 的复制行为**完全一样**。
复制来自行 2 上 `ds1.select(region)` 的**扩展**（3 行数据 → 插 2 行），
`-90` 只是跟着 `extendRow` 的行带克隆一起被复制。
⇒ **`group()` 对 `-90` 零影响；它只影响行数。**

### J.3 `group()` 的真实作用：`isExtended() == true`

```java
// DSGroup.java:23-25
public boolean isExtended() { return true; }
```

这正是 V25 里 `DSGroup` 入选 `is_extend` 白名单的原因：它让该格被包成 `ExtExtendCell`
（扩展源）。**但它决定的是「扩展几行」，不是「这行是不是分组头」。**

⚠ **反直觉分支**（实现必踩）：`DSGroup.calculate` 在**单个分组键时不扩展**：

```java
if (list.size() == 1) { return list.get(0); }        // :139-141  返回标量，不是 List
if (list.size() == 0) { return this.ds.getNullGroup(); }  // :142-144
return list;                                             // :145  只有 >=2 键才返回 List
```

⇒ 只有 `>= 2` 个不同分组键才会返回 `List`，从而触发 `instanceof List` 分支（附件 I.1b）进入扩展。

### J.4 分组头每页重复的机制（正常路径）

`SimplePager` 读 `-90` 行并在每页重复。S7 实测（40 行、11 页）：

```
page 1: rows=5 | r1="region" r3="R1"  r4="R2"  r5="R3"
page 2: rows=5 | r2="R4"    r3="R5"  r4="R6"  r5="R7"
page 3: rows=5 | r2="R8"    r3="R9"  r4="R10" r5="R11"
```

`r2`（那个 `-90` 行）在**每页**都出现且带上该页首个分组键的值 —— 这才是「分组头」的正常表现。

### J.5 NPE 的精确触发条件（`-90` 打在扩展锚点行上）

| 场景 | 构造 | 计算态 | 结果 |
|------|------|--------|------|
| **S5** | 40 行 + `-90` 在锚点行 | `rows=43`，**42 行里 40 个 `-90`** | ❌ **NPE** |
| **S7** | 40 行 + `-90` 在**非锚点行** | `rows=44`，**只有 1 个 `-90`** | ✅ 11 页正常 |
| **S8** | 3 行 + `-90` 在锚点行 | `rows=6`，3 个 `-90` | ✅ 1 页正常 |

⇒ **成因不是「行数多」，也不是「有 `-90`」，而是「`-90` 被 `extendRow` 复制成大量 `-90`」。**
（S8 只有 3 个就正常，S7 只有 1 个也正常。）

栈帧与机理：

```
java.lang.NullPointerException
  at com.raqsoft.reportl.model.engine.SimplePager.createPages(SimplePager.java:261)
  at com.raqsoft.reportl.usermodel.PageBuilder.init1(PageBuilder.java:406)
  at com.raqsoft.reportl.usermodel.PageBuilder.init(PageBuilder.java:378)
```

`SimplePager.java:251-270` 要求分组头的 `groupHeaderLevel` **严格递增**：

```java
byte prevGroupLevel = 0;
IRowCell rowCell = r.getRowCell(minRow);
++minRow;
if (rowCell.getRowType() == -90) { prevGroupLevel = rowCell.getGroupHeaderLevel(); }
else { throw new RuntimeException(...SimplePager.groupMergeCell); }
while (minRow <= maxRow) {
    rowCell = r.getRowCell(minRow);
    ++minRow;
    if (rowCell.getRowType() == -90) {
        byte groupLevel = rowCell.getGroupHeaderLevel();
        if (groupLevel <= prevGroupLevel) continue;   // 同 level 全被跳过
        prevGroupLevel = groupLevel;
        continue;
    }
    ...
}
```

几十个 level=1 的 `-90` 让层级**永不递增**，遍历走到越界行，`getRowCell` 返回 `null` ⇒ **NPE**。

### J.6 ⛔ M2 硬约束

1. **`-90` 绝不能与扩展锚点同行。** 夹具构造时分组头行必须**无表达式**，锚点在另一行。
2. **不要实现「按分组键合成 `-90` 行」** —— 产品没有这个行为。Rust 侧只要照抄
   `rowType` 随行带克隆即可。
3. **`group()` 只算行数不算行类型**：实现 `is_extend` 时 `DSGroup → true`，但不要让它去改 `rowType`。
4. 分页遇 NPE 时先查「`-90` 是否被复制」；Rust 侧应把它报成**结构化错误**
   （`EngineError`），不要 panic（对齐附件 I.1c 的同类纪律）。

---

## 附件 K：求值调度与同值/空值合并的源码级契约（v0.3 新增，2026-10-12）

> 本附件回答 M2 任务书第 4 问（异常协程 → Rust 显式状态机）与第 5 问（`mergeSame`/`mergeNull`）。
> 附件 H.7 给的是**实测口径**，本附件给的是**逐行规则**；两者不冲突，H.7 证明「合并只发生在分页阶段」，
> K.2/K.3 规定「一旦发生，四趟扫描怎么走」。证据均为 reportlite 本体重编译源码，架构师逐行复核。

### K.1 求值调度：Java 异常协程 → Rust 显式状态机

Java 用**抛异常当 switch**实现协程切换：需求值方抛 `(cell, state)`，调度方 catch 后换格继续。

**抛出点（4 处，2 处是调度、2 处是诊断）**

| 位置 | 触发条件 | 抛出物 | 证据 |
| ---- | -------- | ------ | ---- |
| `ExtNormalCell.testValue` | 本格 `nonCalc()` 且有表达式：`rowMerge > 0` 时先 `rowMerge \|= Integer.MIN_VALUE`（bit31 = "正在算"标记）再抛；`rowMerge <= 0` 直接抛 CIRCLE | `(this, engine.state)` / `(this, 1)` | ExtNormalCell.java:301-319 |
| `ExtNormalCell.calculate` | `engine.state > cs.getState()`（本格还没轮到） | `(this, engine.state)` | ExtNormalCell.java:634-639 |
| `ExtExtendCell.calculate` | 同上 | `(this, engine.state)` | ExtExtendCell.java:359-364 |
| `ExtCellSet._$3` / `_$2` | 左邻/上邻主格链走回自己 | `IIllIIIIIIIIIlll(cell, 3)` → 转 `ReportError`（**不是调度**） | ExtCellSet.java:641-646、:678-683 |

**状态常量**（Node.java:45-53）：`NONE=0 / CIRCLE=1 / PARSE=2 / PROPERTY=3 / FIRST=4 / SECOND=5 /
SPECIAL=6 / FINISHED=7 / THIRD=8`。其中 1/3 是**错误态**，4/5/6/8 是**调度态**。
引擎级轮次 `_$14`（`getState():1755-1761`）：`2`=prepare（:744）、`4/5/6`=三个调度通道、`7`=完成（:1526）。

**四条通道**（`ExtCellSet`）

| 通道 | 方法 | 筛选 | 处理的 state | 证据 |
| ---- | ---- | ---- | ------------ | ---- |
| 4（横向扩展格） | `_$1(ctx, st)` 第一趟 | `extensible == 19` | **仅 4**（换格），其余原样重抛 | :1303-1333 |
| 4（纵向扩展格） | 同上第二趟 | `extensible == 18` | 同上 | :1334-1363 |
| 5（表达式格） | `_$2(ctx, special, st)` | `extensible == 17` | 4/5 换格；**6** → 压入 `special` 队列；**8** → 级联失效 | :1366-1426 |
| 6（special 排空） | `_$1(ctx, special, st)` | 队列逐个 | 6 → 重新入队；其余重抛 | :1428-1446 |

- `extensible` 的取值来源：`prepareCalculate` 里有表达式的一律置 **17**（ExtNormalCell.java:539-540）；
  无表达式但 `extendMode ∈ {18,19}` 的保持 18/19（`checkExtend:513`）；`checkExtend` 命中则原格被
  **换成 `ExtExtendCell`** 落回矩阵（ExtCellSet.java:751-753、构造 :1167-1186）。
- **state 8 是级联失效**：`setCalState(8)` 后把依赖栈 `st` 里所有格也标 8 并压入 `_$24` 队列（:1403-1418），
  由 `reflush` 收尾。**不是**简单跳过。
- **state 6 会活锁**：special 队列里的格再抛 6 就重新入队（:1438-1441），产品侧无重试上限；
  Rust 侧**必须**加显式上限并报 `EngineError`，不得静默死循环。

**顶层调度序列**（`calculate():1448-1458` → `_$2(ctx):1483-1543`，入口有 `if (_$14 <= 4)` 守卫）

```
prepareCalculate(ctx, true)        // 建 leftHead/topHead 图 + 换 ExtExtendCell
_$1(ctx, dependCell)              // 通道 4：先 19 后 18
resetDSCurrent / setDSCurrent
_$2(ctx, special, dependCell)     // 通道 5
calcProperties（属性表达式，:1504）
reportStyleName 宏求值 → _$6；chartStyleName → _$4（:1460-1468、:1507-1519）
_$2()                             // 裁掉尾部全空列 :2175-2198
llllllIIlIIIlllI.perfect(...)     // P2 延后
_$1(ctx, special, dependCell)     // 通道 6
_$14 = 7                          // FINISHED
```

**Rust 等价要求（硬约束）**

1. 异常 → `Result<T, Suspend { cell: CellId, state: u8 }>` 的 **trampoline**；每条通道 = 一个显式 `loop`，
   `state` 分派 = `match`。**禁止**用 Rust 递归求值（栈溢出，且无法复现 state 8 的级联失效）。
2. `dependCell` 栈 = `Vec<CellId>`（Java `Stack(256)` 是** soft 上限，不是硬界**）。
3. `nonCalc` 位语义照搬：**bit30 = 已计算**（`setCalc` 置 / `rowMerge &= 0x3FFFFFFF` 清，:287-298、:322-324），
   **bit31 = 正在算**（`testValue` 置）。序列化时 `rowMerge & Integer.MAX_VALUE` 把两个标志位抹掉
   （:2579），故**对拍永远看不到这两位**——不要把它们算进期望值。
4. state 1（CIRCLE）与 3（PROPERTY）必须映射为 `EngineError`，等价产品的 `CircularRefException`（M2-07）。

### K.2 `mergeSame` 四趟扫描（ExtCellSet.java:1832-2049）

**锚点谓词**（四趟共用前四项）：`cell != null && value != null && cell.getRow() == i && cell.getCol() == j`
—— 只处理"自身即左上角"的格，被别人覆盖的槽直接跳过。

| 趟 | 遍历序 / 扫描方向 | `mergeSame` 值域 | mode 条件 | 行号 |
| -- | ----------------- | ---------------- | --------- | ---- |
| 1 纵正 | 列升序、行升序；向下扫 | `∈ {98, 100}` | `mode != 120` | :1851-1902 |
| 2 纵反 | 列降序、行降序；向上扫 | `∈ {98, 100}` | `mode == 120` | :1903-1948 |
| 3 横正 | 行升序、列升序；向右扫 | `∈ {99, 100}` | `mode != 120` | :1949-2002 |
| 4 横反 | 行降序、列降序；向左扫 | `∈ {99, 100}` | `mode == 120` | :2003-2048 |

**连续条件**（纵向为例，`other` 为待并入格）：值 `Variant2.equals` → `other.colMerge == colMerge` →
`other.mergeSame` 仍在值域 → **`getRowCell(max2).rowType == rt`**（行类型变了即断）→ 外加 **mode 基准格 `src` 相同**。

**mode 三分支的语义**（这是最容易按直觉写错的一处）

| `mergeSameMode` | `src` 取法 | 判定 | 含义 | 证据 |
| --------------- | ---------- | ---- | ---- | ---- |
| 118 `MM_DEFAULT` | `cell.getSource()` | 逐格比 `other.getSource()` **对象同一** | 只有**同一模板格复制出的克隆**才合并 | :1864-1865、:1881-1882 |
| 119 `MM_NORMAL` | **左邻格** `_$26.get(i3, j3-1)`（`j3 > 1` 时） | 逐格比左邻 | "同列数据且左邻相同"才合并 | :1866-1867、:1883-1884 |
| 120 `MM_REVERSE` | 无 `src` 判定 | 由第 2/4 趟反向处理 | 反向合并 | :1856、:1908 |

**互斥守卫（`mode == 118` 才生效，Rust 必须保留）**

- `_$6(start, end, col)`（:1790-1809）：遍历 `col` 左侧所有列，若 `[start, end)` 行段上存在**跨边界**的
  合并格（`row1 < start && row2 < end`）则返回 false。
- `_$5(start, end, row)`（:1811-1830）：横向对偶。
- 命中后回退：`max2 -= other.getRowMerge()`（:1890-1892）/ `max2 -= other.getColMerge()`（:1990-1992）。
  ⇒ 缺这两个守卫会把两个本应独立的合并区并成一块，**且不报错**。

**落格与游标**：整块矩形 `[i3, max2) × [j3, j3+colMerge)` 全部 `set(..., cell)`（:1894-1898），
再 `cell.setRowMerge(max2 - i3)`（:1899），游标 `i3 += max2 - i3` 跳过已合并段（:1900）。
第 2/4 趟的对偶写法是 `setRowMerge(max - k - 1)` + `i3 -= max - k - 1`（:1945-1946）。

⚠ `Variant2.equals` 的异常被 `catch` 后只记日志、`same` 保持 `false`（:1872-1877，4 处同构）
⇒ **比较异常 = 视为不等**，不中断、不回滚。

**⛔ 第 3 趟（横正）独有的残值守卫（`:1968`，2026-10-13 证真）**

```java
int rowMerge = cell.getRowMerge();        // :1967
if (max2 >= colSize) continue block19;    // :1968  ← max2 是**第 1 趟留下的残值**
for (max2 = j + cell.getColMerge(); max2 < colSize; max2 += other2.getColMerge()) { … }  // :1969
```

`max2` 是**方法级局部**（`:1842`），第 1 趟与第 3 趟共用；`:1968` 读它时 `:1969` 尚未赋值。
`block19` 标签打在**外层 `for (i…)`** 上（`:1949`），故 `continue block19` = **跳到下一行**。

- **不是反编译噪声**：双 JAR 的 CFR 产物同形；且 CFR 行为探针证明只有「显式标签 + `continue 标签`」
  才会产出 `continue blockN;`（证据链见 `docs/java-reference-map.md` §6.6a）。
- **后果**：第 3 趟在「某行横向跨度触到右边界」后，其后每行的首个候选格都命中并整行跳过
  ⇒ 从那一行起整体停摆；残值本身 ≥ `colSize` 时整趟空转（§6.6b 的控制流模拟）。
- **无夹具可辨**：全部夹具只设过 `mergeSame = 98`，没有 99/100、没有 mode 119/120、没有 `mergeNull`。
- ⇒ **实现必须逐字保留**（本仓 `crates/rpx-engine/src/merge/same.rs` 的 `'block19`）。
  将来补 `mergeSame = 99` 夹具时，产品会带着这个残值行为，改了反而对不上。

### K.3 `mergeNull` 四趟扫描（ExtCellSet.java:2051-2159）

**方向语义（反直觉，必须记牢）**：属性码标在**空值格**上，含义是"我這個空格往哪个方向去找有值的格"。

| 趟 | 遍历序 / 扫描方向 | 被吞格的 `mergeNull` | 锚点动作 | 行号 |
| -- | ----------------- | -------------------- | -------- | ---- |
| 1 下锚 | 列升序、行升序；向下扫 | **114 `MN_UP`**（空格向上找值） | `addRowMerge(rm)` 向下吞 | :2069-2090 |
| 2 上锚 | 列升序、行降序；向上扫 | **116 `MN_DOWN`** | **`addRow(-rm)` 上移锚点**后再吞 | :2091-2113 |
| 3 右锚 | 行升序、列升序；向右扫 | **115 `MN_LEFT`** | `addColMerge(cm)` | :2114-2135 |
| 4 左锚 | 行升序、列降序；向左扫 | **117 `MN_RIGHT`** | **`addCol(-cm)` 左移锚点** | :2136-2158 |

- **锚点谓词不含 `value != null`**（与 `mergeSame` 的关键差异）：只要求 `cell.getRow() == i && cell.getCol() == j`（:2073）。
- 连续条件：被吞格 `value == null` 且 `mergeNull` 等于该趟的值、`colMerge`（纵向）/`rowMerge`（横向）相同、
  `rowType`/`colType` 与锚点一致（:2079、:2101、:2124、:2146）。
- ⚠ 第 2/4 趟用 **负增量移动锚点自身坐标**（`addRow(-rm)` / `addCol(-cm)`），随后按**新坐标**回填矩形
  （:2103-2108、:2148-2153）。实现顺序若写成"先回填再移动"会整块错位。
- 第 1/3 趟的游标是 `i3 += cell.getRowMerge()`（:2088、:2133），第 2/4 趟是 `--i3` / `--j`（:2111、:2156）——
  与 `mergeSame` 的"跳过已合并段"写法不同，别互相套用。

### K.4 `reflush` / 页壳 / 计算态序列化（对拍桥）

- **`reflush()`（:2258-2271）** 固定 5 步：`_$4()` → `_$3()`（子报表传播 :2161-2173）→ `_$2()`（裁尾全空列
  :2175-2198）→ 重建 `Context`（只回填 `paramMap`）→ 若 `_$24`/`_$23` 非空则 `_$12 |= 4` → 清队列 →
  `_$5()`（属性回写）→ `_$1()`（相邻边框互推 :2200-2256）。
- **页壳 `_$1(rows, cols, needClone)`（:2273-2325）**：新建 `Matrix(rows, cols)`；第 0 行/列放根格 `_$10`
  的克隆；随后按行带复制，**锚点按跨度铺满**（:2311-2319）；`needClone` 时分别对 `ExtRowCell` /
  `ExtColCell` / `ExtNormalCell` 调 `deepClone`。`SimplePager` 每页都以 `_$1(1, colCount+1, true)` 建壳
  （SimplePager.java:324）。
- **计算态序列化 = 对拍桥**：`serialize():3432-3503` / `fillRecord():3507-3558`，槽位序为

```
[int32 rows][int32 cols]
[(rows-1) 条 RowCell 记录][(cols-1) 条 ColCell 记录]
[int32 srcList 长度][srcList 条 NormalCell 记录]              // srcPrepare 去重表
[(rows-1)(cols-1) 个槽：writeRecord(锚点记录 或 null) + int32 src 索引]
[_$10 记录][_$3 ByteMap 记录（available 守卫）][_$2 BackGraphConfig（available 守卫）]
```

  **非锚点/空槽只写 int32 `-1` 记录、不带 src 索引**（:3491）；读侧对称地"记录为 null 即 `continue`、
  不读索引"（:3531-3535）。合并格只在锚点写记录（:3485-3490）。
  单条 `ExtNormalCell.serialize():2574-2612` = `byte 是否合并`（合并时 `row / rowMerge & 0x7FFFFFFF /
  col / colMerge`；非合并时 `byte calc 位`）+ `writeObject(cellValue)` + `int16 条数` +
  `{byte key, writeObject value}`（`propSetMap`）+ OIS blob（`customPropSetMap`）+ `writeRecord(cgc)`。
  ⇒ 与 M1 记录层同协议（附件 A）：**Rust 只需实现 `fillRecord` 方向的解析**即可逐格比对，
  无需写 writer（沿用 M1 纪律：writer 由 Java 夹具程序承担）。

### K.5 对附件 F.6 待补清单的勘误（避免两份记录互相矛盾）

| F.6 原条目 | 现状 | 已补位置 |
| --------- | ---- | -------- |
| `ExtCellSet` 3781 行内部算法「未读」 | **部分关闭**（v0.4 勘误） | 扩展见附件 I；调度见 K.1；合并见 K.2/K.3；收尾/页壳/序列化见 K.4；**属性表达式见附件 L**（原判「P2」**已撤销**——行高可由表达式算出，是 M2-11 的前提）。**仍未覆盖**：仅 `perfected`（P2 延后） |
| `ExtNormalCell` 2719 行「未读」 | **部分关闭** | `checkExtend:492-514`、`calculate:634-676`、`testValue:301-319`、`nonCalc/setCalc/reset:287-324`、`expandLeftHead/expandTopHead:463-490`、`serialize:2574-2612` 已逐行核实；**仍未覆盖**：`prepProperties` 与富文本/图形分支 |
| `SimplePager` 3403 行「未读」 | **部分关闭** | `createPages:118-186`、固定区探测 `_$4/_$3:1391-1435`、`copyRowTo:2621-2641`、`copyCellTo:2662-2747` 已逐行核实（含深拷贝特例与合并格恢复）；主循环 `_$5:274+` 的分组/分栏分支仍按 P1 逐行补 |

**勘误（2026-10-13，架构师）：「U4 空数据集」与「分组头动态生成」两项已关闭，不要再当"待验证"。**

- **U4 空数据集** —— `ExtExtendCell.calculate:376-384` 在求出 `List` 之后先判
  `if (len == 0) { this.setValue(null); return; }` ⇒ **空数据集不发生扩展**：行数 = 模板行数，
  锚点置**空值**、**无子格**；`count` 的真语义 = **表达式返回 `List` 的长度**（⛔ 不是「数据集行数」，
  带条件的 `select` 下二者不等）。⚠ 且 `extendRow`/`extendCol` 里 `new ExtCell[count]` 之后**无条件**写
  `extCells[0] = cell`（`ExtCellSet.java:994-995` / `:1094-1095`）⇒ **`count=0` 本身会 AIOOBE**，
  只是被上游那句 `return` 拦住 ⇒ Rust 侧必须自己也兜（不扩展 + 值置空，**绝不 panic**）。
- **分组头动态生成** —— 引擎**从不合成 `-90`**；`-90` 是**纯模板期静态属性**（`IRowCell.TYPE_GROUP_HEADER`，
  `IRowCell.java:21`），由 `ExtCellSet` 构造器一次性 latch（`:151-155` → `_$12 |= 2` → `hasGroupHeader()`）。
  `-90` 的复制只来自 `extendRow` 的行带克隆，**与 `group()` 无关**；`group()` 的真实作用只有
  `DSGroup.isExtended() == true`（`DSGroup.java:23-25`）⇒ 成为扩展源 ⇒ **只决定行数**。
  ⚠ 反直觉分支：`DSGroup.calculate` 在**单个分组键**时返回**标量** `list.get(0)`（`:139-141`），
  只有 `>= 2` 个键才返回 `List` 从而触发扩展。完整论证见**附件 J**（J.1–J.6）。

---

## 附件 L：属性表达式契约与行高来源（v0.4 新增，2026-10-12）

> 本附件**修正 K.5 的一处判断错误**：K.5 把 `calcProperties` 归入「未覆盖 / P2」，实际它是 **M2-11 分页正确性的前提**。
> 起因是模板态 `RowCell` 的 height 槽是**裸 float**（`RowCell.java:317` 写、`:335` 读），看起来「行高不可能是表达式」——
> 但行高**另有**一份 `expMap` 承载表达式，两条路径并存。

### L.1 模板态确实携带行/列属性表达式（且 M1 已解析，引擎侧**不要改 M1**）

- 模板 `RowCell.expMap` 是 `IByteMap`（流内第 5 槽，附件 B.1），`ExtRowCell` 构造器按键取出表达式（`:66-91`）：

| key | 落到 | 语义 |
| --- | ---- | ---- |
| 2 | `rowVisibleExp` | 行可见性 |
| **1** | **`rowHeightExp`** | **行高** |
| 13 | `breakPageExp` | 强制分页 |
| 15 | `breakColumnExp` | 强制分栏 |
| 7 | `adjustRowHeightExp` | 调整行高 |

- `ExtColCell` 构造器同构（`:70-90`）：**12=列可见、11=列宽、13=分页符、15=分栏符**。
- **求值时机**：在 `calculate()` 内部、`reflush()` 与分页**之前**（`ExtCellSet._$1(ctx, false)` → `calcProperties`，`ExtCellSet.java:1504`）。
  ⇒ **行高可以是表达式算出来的**；⚠ `SimplePager` 按「页高 / 行高累加」切页（附件 H.5），行高错 ⇒ 分页全错。
- ✅ **M1→M2 无衔接缺口**：`rowcell.rs:18,35`、`colcell.rs:30`、`cell.rs:88,159` 已把三类 expMap 解析为
  `Option<Vec<(u8, ObjValue)>>`。rpx-engine **按键直接取**即可，**不得为此改 M1**。

### L.2 `calcProperties` 的三条差异（照抄容易错）

| 面 | 取值方式 | 落地 | 证据 |
| -- | -------- | ---- | ---- |
| `ExtRowCell` | `Variant2.getValue`（**不拆 List**） | `setRowHeight(((Number)result).floatValue())` ⇒ 返回 List 会 **ClassCastException** | ExtRowCell.calcProperties |
| `ExtColCell` | 同上 | 列宽同理 | ExtColCell.calcProperties |
| `ExtNormalCell` | **`Variant2.getSingleValue`**（List 取**首元素**，空 List → null） | 写入 `propSetMap` | ExtNormalCell.calcProperties、Variant2.getSingleValue |

- **NormalCell 必须跳过 key 49（FORMAT）与 41（DISPVALUE）**：`if (obj == null || key == 49 || key == 41) continue;`
  ⇒ 格式与显示值**不走属性通道**（41 另走 `calcDispValue`）。这是附件 H.6「`disp_value` 在 7 夹具出现 0 次」的一半成因。
- **属性表达式与值表达式共用同一套异常协程**：同一个 `IIllIIIIIIIIIlll` + `third`（`IllIllllllllllll` 延后栈）
  + `three`（`_$24` 队列）；延后判据是 `pState > cs.getState() && pState == 8`（`ExtNormalCell`/`ExtRowCell` 同构）。
  ⇒ Rust 的 `Suspend { cell, state }` trampoline **同时服务值通道与属性通道**，⛔ 不要写两套调度。
- 收尾：`reflush()` 的 `_$5()` 把属性结果回写行/列/格；`_$23`/`_$24` 非空时置 `_$12 |= 4`（`:2264-2266`），
  该位是「有属性未算完」的标志（不要当成错误）。
