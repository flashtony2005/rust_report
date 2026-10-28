# m2-collation — M2-12 `Text-Text` 字典序（产品 `ChineseComparator` 的移植）

> 作者：后端（collation-rust）· 2026-10-14
> 上游：`docs/m2-engine-design.md` §14（M2-12 函数面）、`docs/spec-m2.md` §6.2
> 证据基线：`F:/project/report/re_work/src/reportlite/**`（CFR 0.152，产品树）+ 产品 JRE 1.8.0_151
> 实测产物 + `tests/fixtures/_collation_*.tsv` + `tests/fixtures/_collation_verdict.md`（独立验证）
> 状态：**已落地并全量对拍**（`crates/rpx-engine/src/collation*`、`tests/collation_golden.rs`）。
>
> **本文的纪律**：每条反直觉结论都给 `文件:行号`、夹具路径或探针名。**没有**读透的一律标 `⚠`。

---

## 0. 契约（Rust 侧对外只认这三条）

| 入口 | 语义 | 出处 |
|------|------|------|
| `collation::compare_text(a, b) -> Ordering` | 两串按产品字典序比较 | `Variant2.java:426`（`compare2` 的 String 分支） |
| `collation::primary_seq(s) -> Vec<u16>` | 一串 → 扁平 primary 序列 | 本文 §4.2 |
| `collation::nfd(cps) -> Vec<u32>` | 码点序列 → NFD（展开 + 重排） | 本文 §4.1 |

`<` `>` `<=` `>=` 经 `expr/num.rs::num_cmp` 的 `Text-Text` 分支走 `compare_text`
（`operator/Smaller.java:40` 等）；`max`/`min`/`DSMax`/`DSMin` 经 `expr/num.rs::aggregate_cmp`
（`function/Max.java:57,106`）。⛔ **`==`（`BinOp::Eq`）不走这里** —— 见 §6。

---

## 1. 产品机制：`rule.txt` → JDK `RuleBasedCollator`

* `ChineseComparator` 把一份规则表读成 JDK 的 `RuleBasedCollator`：
  `ChineseComparator.java:14` 定 `path = "com/raqsoft/commonl/rule.txt"`，
  `:34` `cc = new RuleBasedCollator(rules.toString())`。
* 该规则表就**打在报告引擎的 JAR 里**（`reportlite/lib/reportlite.jar!/com/raqsoft/commonl/rule.txt`），
  源码树里的同一份是 `re_work/resources/reportlite/com/raqsoft/commonl/rule.txt`，**85,231 字节**。
  > ⓘ 旧注释把这条写成"rule.txt 不在仓库、无法复刻" —— **错误**：文件在 JAR 内也确实在源码树里
  > （`find re_work -name rule.txt` 有两处）。本模块按实测模型导出数据表，不再报"缺依赖"。
* 实测的 collator 配置：`strength=2 (TERTIARY)`、`decomposition=1 (CANONICAL_DECOMPOSITION)`
  （`_collation_verdict.md` §0）。**TERTIARY 强度在本题里退化为纯 primary 序**，因为全部 CE 的
  secondary/tertiary 恒为 0（§5.2）。
* `Variant2.compare2:418` 的 String 分支（`:425-427`）直接调 `ChineseComparator`；
  `Variant2.equals:460` 走的是另一条路（精确 `String.equals`，§6）。

---

## 2. 数据模型是怎么被反推出来的

不直接解析 `rule.txt`（那是 JDK 内部格式，且 RBC 还会派生 contract 表），而是**用产品自己的
迭代器把行为 dump 出来，再拟合模型**。参与的探针/生成器（都在 `tests/fixtures/`）：

| 文件 | 作用 |
|------|------|
| `ProbeCollationDump.java` | 用 `CollationElementIterator` 把全部 BMP + 增补抽样 dump 成 `_collation_dump.tsv` |
| `ProbeCollationTables/Reflect/Quirk.java` | 反射读活体 `RuleBasedCollator.tables.contractTable`，取收缩表地面真值 |
| `ProbeCollationModel.java` | **模型本体**（`modelCE` / `modelCompare`）＋ 三重交叉核对 |
| `ProbeCollationAdversarial.java` | **独立验证方**自己写的一套探针（不读被验证方源码） |
| `CollationData.java` / `CollationBin.java` | 从模型导出 13 段并写 `collation.bin`（含不变量断言） |
| `GenCollationData.java` | 编排：先重跑模型核对，再写 bin + golden |
| `GenCollationGolden.java` / `CollationGoldens.java` | 独立种子(314159265)的金标准 |
| `tools_decode_collation_bin.py` | 独立（Python）反解器，直接对 `.bin` 重实现模型再对拍 |

**对拍数字（全为"不符数/总数"，均为 0）**：

```
SUMMARY bmp=0/63488 pairs=0/595984 fuzzCE=0/30000 fuzzCMP=0/8000 singles=21837 contracts=25
model.supplementary=checked=384 mismatched=0
model.ce=CE dump: rows=63872 ce=151485 supplementary=384 (all secondary/tertiary == 0)
ccc.method=sun.text.normalizer.NormalizerImpl.getCombiningClass(int) via reflection
Python 独立反解: SUMMARY bmp=0/63488 supp=0/384 sort=0/3200 pair=0/58 nfd=0/2186  RESULT PASS
```

**数据表就绪**：`crates/rpx-engine/data/collation.bin`，81,444 字节，
`sha256=9b2d81c25b7d6bfcc512bed5f51e2830fdd12855b07ee4a6d7e08118cd11b3cb`，
FNV-1a64=`0x39985A59A7D26227`（Rust 侧指纹钉子，`src/collation.rs`）。
本表由产品实测导出，Rust 侧**只读、不重算、零第三方依赖**。

---

## 3. `collation.bin` 的 13 段布局（全部 little-endian）

```
Header 16B : u32 magic=0x314C4352 | u32 version=1 | u32 sec_cnt=13 | u32 reserved=0
段表       : 13 × ( u32 byte_offset , u32 count )   // offset 自文件起点；各段起始 4 字节对齐
段数据     : 段宽（字节）依次 2,4,4,4,1,4,4,1,4,1,4,2,1
```

| # | 名称 | 宽 | count | 含义 / 不变量 |
|---|------|----|-------|----------------|
| 0 | cjk_primary | u16 | 20902 | `index = cp − 0x4E00`（U+4E00..U+9FA5），**primary 互异**（`CollationData.assertInvariants`） |
| 1 | singles | u32 | 933 | `cp \| (primary<<16)`，**按 cp 升序**；**不含** `[0x4E00,0x9FA5]` |
| 2 | nfd_cp | u32 | 2053 | 有规范分解的码点，升序；**不含** Hangul 音节 `[0xAC00,0xD7A3]` |
| 3 | nfd_off | u32 | 2053 | 分解结果在 pool 中的起始下标 |
| 4 | nfd_len | u8 | 2053 | 分解长度；**min=1**（如 U+0340→U+0300），max=4（`_collation_summary.txt`） |
| 5 | nfd_pool | u32 | 3390 | 分解结果码点序列（**可能 > 0xFFFF**） |
| 6 | ccc_cp | u32 | 653 | 规范组合类非 0 的码点，升序 |
| 7 | ccc_val | u8 | 653 | 对应 ccc（1..240） |
| 8 | contract_starter | u32 | 25 | `cp \| (first_key_index<<16)`；**高 16 位是"键在段 10 里的起始下标"**，按 cp 升序 |
| 9 | contract_count | u8 | 25 | 与 [8] 平行：该起始字符的键数，**sum=54** |
| 10 | contract_key | u32 | 54 | `cp0 \| (cp1<<16)`，**键长恒为 2** |
| 11 | contract_key_ce | u16 | 54 | 每键对应的 primary |
| 12 | ignorable | u8 | 25 | 与 [8] 平行：`1`=未命中键时**整串截断**，**sum=9** |

> 关于 [8]：任务书把高 16 位写作 `first_key_index`。`CollationData.java:29-32` 明确它是
> **offset into section 10**（`[10][kidx .. kidx+count)` 即该组的键），本实现按 offset 读。
> 组内键是**预过滤后的活键（`fwd=true`）**，Rust 侧不再过滤（§5.3）。

---

## 4. 算法（三步）

### 4.1 `nfd(cps: &[u32]) -> Vec<u32>`

1. **逐码点展开**：
   * `cp ∈ [0xAC00,0xD7A3]`（Hangul 音节）→ 标准 L/V/T 算法：
     `S=cp−0xAC00`；`L=0x1100+S/588`；`V=0x1161+(S%588)/28`；`T=S%28`，仅当 `T>0` 再追加 `0x11A7+T`。
   * 否则若 `cp ∈ nfd_cp`（段 2 二分）→ 追加 `pool[off .. off+len]`（**len 可为 1，不可跳过**）。
   * 否则原样。
2. **规范重排**：对每一段**连续** `ccc != 0` 的码点稳定升序。ccc 从段 6/7 查，**查不到视为 0**
   （即起始字符，会切断重排段）。

> 为什么按**码点**而不是 UTF-16 `char`：见 §5.1（`U+FA6C` 反例）。
> 产品侧 NFD 由 JDK RBC 在 `CANONICAL_DECOMPOSITION` 下内部完成（`_collation_verdict.md` §1 注）；
> 我们把它显式做成模型第一步，**只做一次**。

### 4.2 `primary_seq(s) -> Vec<u16>`

```
n = nfd(s.chars())            # 按码点
i = 0
while i < n.len():
    cp = n[i]
    if cp 是收缩起始字符（段 8 二分，得 kidx/kcnt/ignorable）:
        if i+1 < n.len() and (cp, n[i+1]) ∈ 段10[kidx .. kidx+kcnt):
            push 段11[命中键];  i += 2;  continue
        if ignorable:  return 已发出的序列      # ⛔ 整串截断，不是"跳过"
        push 单映射(cp);  i += 1;  continue
    else:
        push 单映射(cp);  i += 1
```

`单映射(cp)`：`0x4E00<=cp<=0x9FA5` → 段 0；否则查段 1（非 CJK 单 CE）；否则**未映射** →
发 `[0x7FFF, cp]`；**`cp > 0xFFFF` 时改发 `[0x7FFF, 高代理, 低代理]`**
（`hi=0xD800+((cp−0x10000)>>10)`，`lo=0xDC00+((cp−0x10000)&0x3FF)`）。

### 4.3 `compare_text(a, b)`

两条 primary(`u16`) 序列的**字典序**；一方是另一方前缀时**短者小**。
在 Rust 里就是 `Vec<u16>::cmp`（逐元素比较后比长度），无需多级权重（§5.2）。

---

## 5. 三条已踩过的坑（照抄，别再踩）

### 5.1 遍历必须按**码点**，不是 UTF-16 `char`

反例 `U+FA6C`（CJK 兼容汉字）：其规范分解是 `U+242EE`（**增补平面**）。按 `char` 遍历会把它看成
两个代理 `char` 而产出 4 个元素；按码点遍历才对。实测 `_collation_dump.tsv` 的行：

```
FA6C	7FFF/0000/0000,D850/0000/0000,DEEE/0000/0000
```

同族证据：`_collation_golden.tsv` 的 `PAIR D87E DD00 4E3D 1`（`D87E DD00`=U+2F900，NFD→U+6D3E），
以及 `GenCollationGolden.java:142-143` 的增补对。Rust 侧因此以 `&[u32]` 为内部口径
（`collation::nfd(cps)` / `primary_seq_cps`），`&str` 只是便捷入口。

### 5.2 全部 CE 的 secondary/tertiary **恒为 0**，比较只需 primary

`GenCollationData.checkCeDump` 对 `_collation_dump.tsv` 全部 **151,485** 个 CE 断言
`secondary==0 && tertiary==0`，不均即 `IllegalStateException`；独立验证同样给出 0 例外
（`_collation_verdict.md` §4）。故产品虽配 `strength=TERTIARY`，行为退化为纯 primary 序 ⇒
Rust 侧不需要多级权重，`Vec<u16>` 字典序足矣。

### 5.3 可忽略字符是"截断"而不是"跳过"

命中**可忽略起始字符**（段 12 = 1）且未命中任何键时，**丢弃整个后缀**，而不是忽略该字符后继续。实测：

| 输入 | 产品 CE | 出处 |
|------|---------|------|
| `"\u{0301}A"` | `[]`（空） | `_collation_verdict.md` §2；`_collation_golden.tsv` `PAIR 0301 0041 / 0041 = -1` |
| `"\u{0301}"` vs `""` | `0`（相等） | `_collation_golden.tsv` `PAIR 0301 / (empty) = 0` |
| `"A\u{0327}B"` vs `"AB"` | `-1` | `_collation_golden.tsv` `PAIR 0041 0327 0042 / 0041 0042 = -1` |
| `"\u{0001}A"` | `[]` | `_collation_verdict.md` §5.2③；`_collation_contract_golden.tsv` `IGNORABLE 0001` |

**且这些"可忽略起始字符"恰 9 个**：`0000,0001` + 7 个组合符 `0300,0301,0302,0303,0308,030A,0327`
（`collation.bin` 段 12 的 9 个 `1`）。注意 `0340/0341/0344` 三者**不是**起始字符 —— 它们靠 NFD
折叠成 `0300` / `0301` / `(0308 0301)` 才变得完全可忽略（§4.1；`_collation_verdict.md` §5）。

**收缩键 = 54 条（不是 79、也不是 55）**：活体 `contractTable` 里 2 字条目共 108 = 54 `fwd=true`
（可用）+ 54 `fwd=false`（死镜像）。任何 `fwd=false` 键（如 `0301 0041`）若被装进表，会把
`"\u{0301}A"` 算成 `[00C1]`，与产品 `[]` 不符。另有 1 条 JDK 内部键 `0000 0001→0001` 必须显式保留
（`U+0000/U+0001` 自身映射在 JDK 里是 UNMAPPED，未命中即截断）。
出处：`_collation_verdict.md` §2、§5.1；`collation.bin` 段 9/10 的 `sum=54`。

**`singles` 为什么是 933 而不是 935**：`U+2F800`（NFD→U+4E3D）与 `U+2FA00`（NFD→U+9829）在 dump 里是
"非 CJK 单 CE"，但它们的 `nfd_len==1` 且 **NFD 先行**（§4.2 第 1 步先做 NFD），永远不以"自身"查
`singles` ⇒ 剔除正确。全 BMP(63,488)+384 增补实测 `bmp=0 supp=0`，无 `7FFF,cp` 回退泄漏
（`_collation_verdict.md` §5.1②；`tools_decode_collation_bin.py` 的 `singles investigation`）。

### 5.4（补充）孤立代理也能被码点路径复现

`&str` 承载不了 `D800..DFFF`。金标准 `_collation_golden.tsv` 含 3 条此类 `PAIR`；本实现用
`compare_cps(&[u32])` 复现：孤立代理 `cp<=0xFFFF` 且未映射 ⇒ 走 `[0x7FFF, cp]` 兜底
（实测 `CE(U+D800)=[7FFF,D800]`，`_collation_verdict.md` §2），与产品一致。

**金标准 hex 的口径 = 码点，不是 UTF-16 单元**：`GenCollationGolden.java:175-185` 的 `hex()` 用
`codePointAt` + `charCount` 逐**码点**输出，故一个**合法代理对被折叠成单个码点** —— PAIRS 里的
`{"D800 DC00", "E000"}`（`GenCollationGolden.java:142`）落盘字段是 `PAIR / 10000 / E000 / -1`
（TAB 分隔，第 3230 行，`10000` 即 **U+10000**）；而第 3236 行 `PAIR / D800 / DC00 / -1` 是
**两个字段**各含一个孤立代理码点。⛔ 二者 CE 不同：`U+10000 → [7FFF, D800, DC00]`（**3** 元素，
`_collation_dump.tsv` 行 `010000`），若误当两个孤立代理则得 `[7FFF, D800, 7FFF, DC00]`（**4** 元素）；
两种读法在现有夹具上**比较符号恰好一致**，故"测试全绿"不能证明没踩坑，必须按序列断言。
钉子：`tests/collation_golden.rs::surrogate_pair_and_lone_surrogates_are_distinct_paths`；
解析口径本身的说明在 `collation_golden.rs::decode_field`。

---

## 6. 与 `==` 的分工（别改错地方）

`==`（`BinOp::Eq`）走 `expr/num.rs::val_eq` → `Variant2.equals:460`：同类型两对象先
`getSingleValue`，再 `o1.equals(o2)`（`:470`）——**精确 `String.equals`，不经过 collator**。
故 `"\u{0301}" == ""` 为 **false**，尽管 collation 下二者相等（§5.3）。
Rust 的 `expr/num.rs::val_eq` 保持原样并在注释里写明这一点。

---

## 7. 落点与自证

| 层 | 文件 |
|----|------|
| 数据表 | `crates/rpx-engine/data/collation.bin`（只读，生成物） |
| 装载/入口/指纹钉子 | `crates/rpx-engine/src/collation.rs` |
| 段访问 | `crates/rpx-engine/src/collation/tables.rs` |
| NFD | `crates/rpx-engine/src/collation/nfd.rs` |
| primary 序列 | `crates/rpx-engine/src/collation/seq.rs` |
| 接线 | `src/expr/num.rs`（`num_cmp` / `aggregate_cmp`）；回归单测 `src/expr/tests_aggregate.rs` 第 ⑧ 条 |
| 金标准对拍 | `crates/rpx-engine/tests/collation_golden.rs`（5 条，含全 BMP+增补逐点） |
| 端到端 | `tests/fixtures/engine_collate.{rpx,csv,expected.json}` 经 `tests/engine_grid_compare.rs` 逐格对拍 |

`collation_golden.rs` 的实测数字：dump `checked=63872 mismatched=0`（其中增补 `sup=384`）；
SORT `3200` 条复现次序；PAIR `58` 条符号全等；NFD `2186` 行全等；收缩键 `54` + 可忽略 `12`；
参照值 阿294 / 白555 / 陈1854 / 丁3322 / 一18183 / 张19926 / 赵20001。
