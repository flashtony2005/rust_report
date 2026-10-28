# M2-12 Text-Text 字典序 —— 独立验证结论（collation-verifier）

范围：复核 `com.raqsoft.commonl.ChineseComparator`（产品 JAR 内 `com/raqsoft/commonl/rule.txt`
→ JDK `java.text.RuleBasedCollator`）在 `Variant2.java` String-String 分支上的**行为模型**，
并产出 Rust 侧金标准。

## 0. 独立性与环境

* 本报告**未读**被验证方的 `tests/fixtures/ProbeCollationModel.java` /
  `ProbeCollationTables.java` / `ProbeCollationReflect.java` / `ProbeCollationQuirk.java`。
  所有结论来自我自己写的探针实跑 + 仅作数据参照的 `tests/fixtures/_collation_dump.tsv`。
* 本人反推出的模型参数来源（均为**独立取证**）：
  1. `rule.txt`（产品数据，从 `reportlite/lib/reportlite.jar` 提取）用自写解析器拆成 **21250 个单字符元素**，
     并测得 **primary(字符) = 该字符在 rule.txt 中的下标**（U+0001→1、U+0041→65、U+00C1→193、
     U+963F→294、U+4E01→3322、U+4E00→18183、U+5F20→19926 全部吻合）。
  2. 用**反射**读取活体 `RuleBasedCollator.tables.contractTable`（JDK 1.8.0_151，产品自带 JRE）
     作为收缩表的地面真值。这是运行时取证，不是读被验证方源码。
  3. 收缩键集合 = 「对 rule.txt 中每个字符取 NFD，长度为 2 的字」——即由 rule.txt 自身推导。
* 环境/命令（照任务给定，实测通过）：
  ```bash
  CP="F:/project/report/reportlite/lib/*;F:/project/report/rpx/tests/fixtures/out"
  D:/Tools/jdk-21.0.2/bin/javac.exe --release 8 -Xlint:none -cp "$CP" -d out ProbeCollationAdversarial.java
  F:/project/report/common/jre/bin/java.exe -cp "$CP" rpx.fixtures.ProbeCollationAdversarial   # 产品 JRE 1.8.0_151
  F:/project/report/common/jre/bin/java.exe -cp "$CP" rpx.fixtures.GenCollationGolden           # 金标准生成
  ```
* 被验证的 collator 配置实测：`strength=2 (TERTIARY)`、`decomposition=1 (CANONICAL_DECOMPOSITION)`。

---

## 一、逐条结论

| # | 断言 | 结论 |
|---|------|------|
| 1 | 先对整个串做 NFD 规范化 | **成立**（等价性成立；机制归属需注意，见注） |
| 2 | 收缩贪心最长匹配 / 截断 / 未映射 `[0x7FFF,cp]` / 增补 `[0x7FFF,high,low]` | **成立，但需修正两处参数**（见 2.2） |
| 3 | 比较 = u16 primary 序列字典序；前缀短者小 | **成立** |
| 4 | 全部 CE 的 secondary/tertiary 恒为 0 | **成立** |
| 5 | 25 组 / 79 键 / 键长恒 2 / 12 个起始字符完全可忽略 | **部分成立**：25 组 ✓、键长恒 2 ✓；**「79 键」不成立**——活体可用键（fwd=true）恰 **54**（108 = 54 fwd + 54 死镜像）；「12 个完全可忽略」对应 12 个空 CE 码点，真正的**截断起始字符为 9** |
| 6 | CJK U+4E00..U+9FA5 共 20902 字，单 CE 且 primary 互异 | **成立** |
| 7 | 参照值 阿294 / 丁3322 / 一18183 / 张19926 | **成立** |

### 1. 断言 1（NFD）—— 成立

实测（`ProbeCollationAdversarial`，全部 PASS）：

```
PASS  CE(0041 0301)      actual=[00C1]      expected=[00C1]
PASS  CE(00C1)           actual=[00C1]      expected=[00C1]
PASS  CMP(00C1 , 0041 0301)  actual=0  expected=0
PASS  CE(0100)           actual=[0041,7FFF,0304]   expected=[0041,7FFF,0304]   # NFD(0100)=0041 0304
PASS  CE(0340)           actual=[]   CE(0300)=[]                                # NFD(0340)=0300
PASS  CE(2E80)           actual=[7FFF,2E80]        # 未列字符
PASS  CE(D87E DD00)      actual=[2CD2]            # D87E DD00=U+2F900; NFD→U+6D3E → primary 2CD2
```

注（说对了结论、理由需澄清）：NFD **不是** `Variant2`/`ChineseComparator` 自己调的，
而是 JDK `RuleBasedCollator` 在 `decomposition=CANONICAL_DECOMPOSITION` 下的内部行为。
把它描述成「先对整串做 NFD」在**行为上等价**，因此模型成立；但实现者不应在 Rust 侧额外
对字符串做一次多余归一化再期望一样，而应把归一化当作模型第一步（二者一致）。

### 2. 断言 2（收缩/截断/未映射）—— 机制成立，参数需修正

实测机制要点（均 PASS）：`A`+U+0301 命中收缩→`[00C1]`；`A`+U+0304（无该键）→`[0041,7FFF,0304]`；
孤立 `U+0301` → `[]`；`U+0301 A` → `[]`（整串截断）；`A U+0327 B` → `[0041]`；
增补平面 → `[7FFF,high,low]`；未映射 BMP → `[7FFF,cp]`（含未配对代理 `U+D800`→`[7FFF,D800]`；
`U+9FA6`→`[7FFF,9FA6]`）。

**2.1 独立测得的收缩表（反射地面真值，25 行）：**

```
rows=25
2-char entryPairs=108  (len!=2: 0)   = 54 fwd=true + 54 fwd=false(dead mirror)
  base-first=55 (54 fwd + 1 dead)   mark-first=53 (all dead)
truncating starters (empty-CE own mapping)=9
  -> 0000, 0001, 0300, 0301, 0302, 0303, 0308, 030A, 0327
```
* 活体表里 2 字条目共 **108** 条，按 `EntryPair.fwd` 分成两半：**fwd=true 54 条 / fwd=false 54 条**。
  真正会被消费的可用收缩键 = **fwd=true 的 54 条**；fwd=false 的 54 条是**死镜像**。
* `base-first`（键形如「字母+组合符」，如 `0041 0301→00C1`）= **55**，但其中 **1 条是死镜像**
  `0001 0000→0001`（fwd=false），其余 **54 条 fwd=true**。其中 53 条可由「rule.txt 中 NFD 长度为 2
  的字符」独立推出，另 **1 条是 JDK 内部键** `0000 0001→0001`（仅反射可得）——54 = 53 + 1。
* `mark-first`（键形如「组合符+字母」，如 `0301 0041→00C1`）= **53**，**全部是 fwd=false 死键**：
  产品对 `U+0301 A` 给 `[]`，而**不会**命中 `0301 0041→00C1`。
* **定案：可用收缩键 = 54**；`collation.bin` 的 `sum(contract_count)=54`、`keys=54` 与之吻合。
  我早期一度报 55，是把死镜像 `0001 0000`（fwd=false）也算了进去，已按 team-lead 更正为 54。

**2.2 必须修正的两处（否则 Rust 侧会算错）：**
1. **收缩键只能收 fwd=true 的 54 条，绝不能把 mark-first 死键或 fwd=false 镜像纳入。**
   反证：若表里含 `0301 0041`，模型会把 `"\u0301A"` 算成 `[00C1]`；产品实际给 `[]`。
   我的探针用「rule.txt 的 NFD-2 推导 + 1 个内部键」的 **54 键表**，对全部边界与 100~200 字符
   随机长串（含 CJK/Hangul/PUA/增补/组合符）**126 例 0 行为不符**。
   + 内部键 `0000 0001→0001` 须显式补上（`U+0000/U+0001` 的「自身映射」在 JDK 里是 UNMAPPED，
   未命中即整串截断：`CE("\u0001")=[]`、`CE("\u0001A")=[]`、`CE("\u0000A")=[]`）。
2. **截断集合 = 「自身映射为 UNMAPPED 的起始字符」= 9 个**：`0000,0001` 与 7 个组合符
   `0300,0301,0302,0303,0308,030A,0327`。（非起始字符的完全可忽略码点不截断：
   `CE(0000 0001)=[0001]`，说明 `U+0000` 未命中时才会截断，命中时不截断。）

### 3. 断言 3（比较）—— 成立

```
PASS  CMP(0301 , )        actual=0   # 双方 primary 序列皆空
PASS  CMP(0301 0041 , 0041)  actual=-1   # [] < [0041]
PASS  CMP( , 0061)        actual=-1   # 前缀：短者小
PASS  CMP(0061 0062 , 0061 0062 0063) actual=-1
```
`主要素序列字典序 + 前缀短者小` 的 `modelCompare` 与产品 `compare` 的**符号**在 126 例上全一致。
（注意产品是 TERTIARY 强度，但所有 CE 的 sec/ter 为 0，故退化为纯 primary 序，见 4。）

### 4. 断言 4（sec/ter 恒 0）—— 成立

对 `_collation_dump.tsv` 全部 **63872** 行（BMP 0x0000–0xF7FF 共 63488 + 384 个增补抽样）
做正则检查，**非 `primary/0000/0000` 的行数 = 0**：
```
$ grep -vcP '\t([0-9A-F]{4}/0000/0000(,[0-9A-F]{4}/0000/0000)*)?$' _collation_dump.tsv
0
```

### 5. 断言 5 —— 部分成立（「25 组 / 键长 2」成立；「79 键」不成立；「12」措辞不准）

实测（我自己反射取活体 contractTable，并读 `EntryPair.fwd`）：
```
rows=25                                        ← 「25 组」✓
len!=2 keys=0                                  ← 「键长恒为 2」✓
2-char entryPairs=108 = 54 fwd=true + 54 dead  ← 与「79」不符
   base-first=55 (54 fwd + 1 dead)  mark-first=53 (all dead)
truncating starters=9
```
* **「25 组」成立**：我的独立推导（rule.txt 的 NFD-2 分解）也恰得 25 个起始字符
  = 16 个拉丁字母 `A C E I N O U Y a c e i n o u y` ∪ 7 个组合符 ∪ {0000,0001}。
* **「键长恒为 2」成立**（len!=2 计数为 0）。
* **「79 键」不成立**：活体表 2 字条目共 108 条，其中 **fwd=true 恰 54 条**（可用键）、
  fwd=false 54 条（死镜像/死键）。任何自然口径（108 / 55 / 54 / 53）都给不出 79。
  **定案值 = 54**，与 `collation.bin` 的 `sum(contract_count)=54`、`keys=54` 完全一致。
  （我早期报 55，是把死镜像 `0001 0000`（fwd=false）计入，已按 team-lead 更正。）
  **行为上**：只要最终表 = 这 54 条 fwd=true 键（剔除全部 fwd=false），即与产品一致。
* **「12 个起始字符完全可忽略」**：数字 **12** 指「BMP 中 CE 完全为空的码点」，
  实测 `0000 0001 0300 0301 0302 0303 0308 030A 0327 0340 0341 0344`（12 个）；
  其中只有 **9 个**是收缩起始字符（`0000,0001` + 7 个组合符），`0340/0341/0344` 要**经 NFD**
  折叠（`0340→0300`、`0341→0301`、`0344→(0308 0301)`）后才完全可忽略。故措辞「起始字符」不准确；
  真正的**截断起始字符数 = 9**，与 `collation.bin` 的 `sec[12]`（9 个 1）一致。

### 6. 断言 6（CJK）—— 成立

```
PASS  CLAIM CJK 4E00..9FA5 all single CE   measured=single=20902
PASS  CLAIM CJK primaries distinct         measured=distinct=20902
```
补充：range 之外如 `U+9FA6 → [7FFF,9FA6]`（未列，未映射）；
兼容汉字 `U+FA1F → [0149]`（在 rule.txt 里，是 listed 单 CE）；
`U+2F800 → [2348]`（NFD=U+4E3D 后取 primary）、`U+2F900 → [2CD2]`（NFD=U+6D3E）。

### 7. 断言 7（参照值）—— 成立

```
PASS  CLAIM ref U+963F primary==294    PASS  CLAIM ref U+4E01 primary==3322
PASS  CLAIM ref U+4E00 primary==18183  PASS  CLAIM ref U+5F20 primary==19926
```

---

## 二、探针 SUMMARY（原文）

```
model: ruleChars=21250 contractKeys=54 starters=25 selfUnmapped=9
PASS  CLAIM contract rows == 25  measured=25
PASS  CLAIM all contract keys length 2  measured=len!=2 count=0
PASS  CLAIM 2-char entryPairs == 108 (54 fwd + 54 dead)  measured=multi=108 fwd=true=54 fwd=false=54
PASS  CLAIM live fwd=true keys == 54  measured=measured=54
PASS  CLAIM model contractKeys == fwd=true keys  measured=model=54 fwd=true=54 | base-first=55 mark-first=53
PASS  CLAIM truncating starters (empty-CE own mapping) == 9  measured=measured=9
PASS  CLAIM ref U+963F/U+4E01/U+4E00/U+5F20 primary==294/3322/18183/19926
PASS  CLAIM CJK 4E00..9FA5 all single CE  measured=single=20902
PASS  CLAIM CJK primaries distinct  measured=distinct=20902
SUMMARY cases=126 mismatched=0
```
**模型 vs 产品的行为用例 0 不符**（51 条边界单字符 + 27 条边界对 + 12 组 100~200 字符随机长串的
CE 与 compare），结构断言亦全 PASS —— 断言 5 的两条厂商数字已按独立实测真值（**54 键 / 9 截断起始**）改写。

打过的边界清单（全部 PASS）：收缩命中/未命中；可忽略截断（孤立、串首、串中、连续多个、
整串皆可忽略）；ccc 重排（`A 0327 0301` vs `A 0301 0327`；`A 0301 0300` vs `A 0300 0301`）；
未映射（希腊/西里尔/PUA/BMP 边界 FFFD-FFFF/兼容汉字/未列 CJK）；增补平面
（`10000/20000/1F600/2F800`，含 NFD 到 BMP 的 `2F800`）；未配对代理
（`D800`/`DC00`/`D800 D800`）；前缀（`""`、`ab`/`abc`、`A`/`A+0301`）；`U+0000/U+0001`
与组合符起始截断（`0001A`、`0340A`）；长随机串互比。

### 附：反射测得的完整 contractTable（25 行，原始证据；`=>` 右侧为 CE 的 primary）

```
[0]  0000=>FFFF(UNMAPPED),        0000 0001=>0001
[1]  0001=>FFFF(UNMAPPED),        0001 0000=>0001
[2]  0041=>0041,  0041 0301=>00C1, 0041 0302=>00C2, 0041 0303=>00C3, 0041 0308=>00C4, 0041 030A=>00C5, 0041 0300=>00C0
[3]  0300=>UNMAPPED, 0300 0045=>00C8, 0300 0049=>00CC, 0300 004F=>00D2, 0300 0055=>00D9, 0300 0061=>00E0, 0300 0065=>00E8, 0300 0069=>00EC, 0300 006F=>00F2, 0300 0075=>00F9, 0300 0041=>00C0
[4]  0301=>UNMAPPED, 0301 0045=>00C9, 0049→00CD, 004F→00D3, 0055→00DA, 0059→00DD, 0061→00E1, 0065→00E9, 0069→00ED, 006F→00F3, 0075→00FA, 0079→00FD, 0041→00C1
[5]  0302=>UNMAPPED, 0302 0045=>00CA, ... 0302 0041=>00C2            (共 10 键)
[6]  0303=>UNMAPPED, 0303 004E=>00D1, ... 0303 0041=>00C3            (共 6 键)
[7]  0308=>UNMAPPED, 0308 0045=>00CB, ... 0308 0041=>00C4            (共 11 键)
[8]  030A=>UNMAPPED, 030A 0061=>00E5, 030A 0041=>00C5               (共 2 键)
[9]  0043=>0043,  0043 0327=>00C7
[10] 0327=>UNMAPPED, 0327 0063=>00E7, 0327 0043=>00C7               (共 2 键)
[11] 0045=>0045,  0045 0301=>00C9, 0302=>00CA, 0308=>00CB, 0300=>00C8   (共 4 键)
[12] 0049=>0049,  0049 0301=>00CD, 0302=>00CE, 0308=>00CF, 0300=>00CC   (共 4 键)
[13] 004E=>004E,  004E 0303=>00D1
[14] 004F=>004F,  004F 0301=>00D3, 0302=>00D4, 0303=>00D5, 0308=>00D6, 0300=>00D2 (共 5)
[15] 0055=>0055,  0055 0301=>00DA, 0302=>00DB, 0308=>00DC, 0300=>00D9   (共 4 键)
[16] 0059=>0059,  0059 0301=>00DD
[17] 0061=>0061,  0061 0301=>00E1, 0302=>00E2, 0303=>00E3, 0308=>00E4, 030A=>00E5, 0300=>00E0 (共 6)
[18] 0063=>0063,  0063 0327=>00E7
[19] 0065=>0065,  0065 0301=>00E9, 0302=>00EA, 0308=>00EB, 0300=>00E8   (共 4 键)
[20] 0069=>0069,  0069 0301=>00ED, 0302=>00EE, 0308=>00EF, 0300=>00EC   (共 4 键)
[21] 006E=>006E,  006E 0303=>00F1
[22] 006F=>006F,  006F 0301=>00F3, 0302=>00F4, 0303=>00F5, 0308=>00F6, 0300=>00F2 (共 5)
[23] 0075=>0075,  0075 0301=>00FA, 0302=>00FB, 0308=>00FC, 0300=>00F9   (共 4 键)
[24] 0079=>0079,  0079 0308=>00FF, 0301=>00FD
```
（表中 `[2]`–`[24]` 的行首「自身项」为 `starter=>starter`，即该起始字符自己的 primary；
`FFFF(UNMAPPED)` 表示自身项为 UNMAPPED —— 这类起始字符（9 个）未命中收缩时**整串截断**。）

## 三、给 Rust 侧的金标准（交付物 2）

`tests/fixtures/_collation_golden.tsv`（由 `GenCollationGolden.java` 生成，**种子 314159265**，
非被验证方的 20261014）：

```
# 元信息行（以 '#' 开头，解析时应跳过）
SORT\t<0基序号>\t<码点 hex 空格分隔>\t<UTF-8 原文，\t\r\n 转义>
PAIR\t<hexA>\t<hexB>\t<-1|0|1>
```
* **3200 条 SORT**（长度 0–8 码点；池覆盖 ASCII 可打印、Latin-1、Latin Ext-A、
  希腊、西里尔、CJK、Hangul、PUA、组合符、增补平面 U+10000–U+10FFF 与 U+20000–U+2A6DF 抽样）+
  **58 条 PAIR**（收缩命中/未命中、截断、ccc 重排、未映射、增补、代理、前缀、空串、case）。
* 排序用 `Arrays.sort(arr, ChineseComparator.getInstance())`；**同序（compare==0）相对次序的可复现性**：
  排序前先把数组按**码点序**排好，再用 Java 的**稳定**排序（`Arrays.sort(Object[])` 为 TimSort）
  按 collator 排序 —— 因此 compare==0 的元素保持码点升序。已在文件头 `#` 注释里写明。
  生成两次结果逐字节相同（`diff` 通过）。
* 例：`PAIR 00C1 0041 0301 0`、`PAIR 0301 0041 0041 -1`、`PAIR 1F600 0301 1F600 0301 0041 0`、
  `PAIR 0041 0327 0301 0041 0301 0327 0`。
* 生成后**回读自检**（生成器内置）：
  ```
  VERIFY sorts=3200 tiePairs=249 badSortOrder=0 badTieOrder=0 badPair=0
  ```
  （`badSortOrder` = 违反单调不减的相邻对数；`badTieOrder` = 同序组内违反码点升序的对数；
  `badPair` = PAIR 期望符号与产品不符的条数。三者均 0。）
* Rust 侧验收建议：SORT 段要求「在该实现下单调不减，且代价相等组按码点升序」；
  PAIR 段要求符号逐一相等。

## 四、交付物 4（`tools_decode_collation_bin.py`）—— 已执行，RESULT PASS

`crates/rpx-engine/data/collation.bin` 已生成。用**我自己的**独立 Python 反解器
（不读任何被验证方 Java 源码，仅按格式契约解析、重实现模型、再与
`_collation_dump.tsv` / `_collation_golden.tsv` / `_collation_nfd_golden.tsv` 交叉对拍）
对最终 `.bin`（`sha256=9b2d81c2…b3cb`）实跑，命令与**原文输出**如下：

```bash
$ cd tests/fixtures && python tools_decode_collation_bin.py ../../crates/rpx-engine/data/collation.bin
bin      = ..\..\crates\rpx-engine\data\collation.bin
bytes    = 81444  sha256=9b2d81c25b7d6bfcc512bed5f51e2830fdd12855b07ee4a6d7e08118cd11b3cb
  cjk.count              OK (got=20902 expect=20902)
  singles.count          OK (got=933 expect=933)
  nfd.count              OK (got=2053 expect=2053)
  nfd_pool.count         OK (got=3390 expect=3390)
  ccc.count              OK (got=653 expect=653)
  starters               OK (got=25 expect=25)
  sum(contract_count)    OK (got=54 expect=54)
  keys                   OK (got=54 expect=54)
  sum(ignorable)         OK (got=9 expect=9)
  singles_sorted         OK (got=True expect=True)
  nfd_cp_sorted          OK (got=True expect=True)
  ccc_cp_sorted          OK (got=True expect=True)
  keys_in_range          OK (got=True expect=True)
  nfd off/len in pool    OK (got=True expect=True)
singles investigation: dump non-CJK single-CE=935 bin singles=933 missing=['0x2f800', '0x2fa00']
   0x2f800 nfd_len=1 nfd->['0x4e3d'] ; model CE=['0x2348']
   0x2fa00 nfd_len=1 nfd->['0x9829'] ; model CE=['0x2e82']
structure bad=0
SUMMARY bmp=0/63488 supp=0/384 sort=0/3200 pair=0/58 nfd=0/2186
RESULT PASS
```
（本段为**我自己重跑**的结果，非转引他人；`bmp/supp` = 全 BMP 63488 + 384 增补码点的
「.bin 重实现 CE」对「`_collation_dump.tsv` 地面真值 CE」的逐点不符数，均 0；`sort` = 金标准
3200 条，`pair` = 58 条边界对，`nfd` = 2186 条 NFD 单测，不符数均 0。）

## 五、复核定案与保留意见（不省略）

### 5.1 本次复核定案（team-lead 会签）

1. **收缩键 = 54 条**（不是最初口径的 79、也不是我一度报的 55）。
   依据 `_collation_internals.txt`：多字符 EntryPair 共 **108** 条，`fwd=true` 恰 **54**、
   `fwd=false` 恰 **54**，镜像成对。
   **规律「mark-first 死键 ⟺ `fwd=false`」成立**，保留为正式结论：我报的 **55** 是漏滤了
   `0001 0000`（首字符非组合符，但 `fwd=false`），已更正为 54。
   `collation.bin` 的 `sum(contract_count)=keys=54` 与此吻合。
   **Rust 侧须只装 fwd=true 的 54 条**；纳入任何 `fwd=false` 会在 `"\u0301A"` / `"\u0300E"`
   这类输入上偏离产品（详见 2.2）。
2. **`singles` 933 条成立**：`_collation_dump.tsv` 分类下「非 CJK 单 CE」码点为 935，
   `.bin` 存 933，缺的 2 个是 **`U+2F800`**（NFD→`U+4E3D`，CE `2348`）与
   **`U+2FA00`**（NFD→`U+9829`，CE `2E82`）。两者 `nfd_len==1`，且 **NFD 先行**，
   故永不以「自身」查 singles 表 —— **剔除正确**。
   **我此前第 4 条保留意见（singles 覆盖范围）到此由我亲手证伪并确认**：
   singles 只在「NFD 后仍为单码点且非 CJK」时被查到，`U+2F800/U+2FA00` 由 NFD 表覆盖；
   全 BMP(63488)+384 增补实测 `bmp=0 supp=0`，**无任何 `got=7FFF,cp` 回退泄漏**，
   故不存在因漏存而走入未映射分支。

### 5.2 其余保留意见

3. **`U+0000/U+0001` 的 UNMAPPED 自身映射**是 JDK 内部怪癖，不在 rule.txt 分解里，
   易漏；漏了会把 `"\u0001"` 算成 `[0001]`（产品是 `[]`）。**Rust 侧须把 0000/0001
   显式纳入「截断起始字符」（9 个）而不是普通 singles。**
4. **归一化归属**：模型说「先 NFD」行为正确，但若 Rust 侧再额外做一次归一化或再做 NFC，
   会改变结果（如 `U+0340`）。建议只做一次 NFD。
