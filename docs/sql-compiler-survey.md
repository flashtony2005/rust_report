# L5 SQL 编译层：GitHub 现成方案调研与借鉴（2026-10-23）

> 目的：L5（rpx-plan → 方言 SQL）动手前先看现成轮子。结论：**生成自研（闭合文法），
> 校验借 sqlglot 的 AST 模式**，护栏照抄生产共识的 allow-list 模型。

## 1. 两个层面分开看

### A. SQL 解析/方言（通用轮子，别自研）

| 项目 | 语言 | 定位 | 对我们的价值 |
|---|---|---|---|
| **sqlglot**（tobymao/sqlglot，MIT，零依赖纯 Python） | Python | 30+ 方言解析/转译/AST 构建/优化器 | **L5 校验层的首选**：`parse_one(sql, read=dialect)` → AST 按**节点类型**拒绝；方言转译（pg/mysql/sqlite 的日期函数、引号、类型差异）全覆盖 |
| **datafusion-sqlparser-rs**（apache，Apache-2.0） | Rust | SQL → 类型化 AST（语法层，不做语义） | 引擎侧（rpx-ds 执行用户 SQL 的前置检查）用；DataFusion 是它上面的规划/执行层 |
| pg_query（libpg_query） | Python/Go | 真 Postgres 解析器 | 仅 PG，过重 |

⚠ sqlglot 自己声明「是 transpiler 不是 validator」—— 解析通过 ≠ 语义正确，
所以语义正确性仍由我们的业务模型 + 粒度检查（L4）负责，它只管**结构安全 + 方言正确**。

### B. 语义层 → SQL（查询形状的参照系）

| 项目 | 查询形状 | 借鉴点 |
|---|---|---|
| **Cube**（cube-js/cube，Apache-2.0） | `{measures, dimensions, timeDimensions[{dimension, granularity, dateRange}], filters[{member, operator, values}]}` | **我们的 rpx-query/v1 形状与它同构**（metrics/dimensions/time_range/filters）——设计得到验证。核心原则：**指标由引擎展开，agent 永不写聚合** |
| **MetricFlow**（dbt，Apache-2.0，2025 开源） | metric + groupBy(grain) + where | 指标编译器定位（query compilation library）与我们 L5 一致 |
| **Malloy**（MIT）+ Publisher | 语义查询语言 → 编译到 SQL 引擎 | 「查询可组合扩展语义源」的思想 |
| **Sidemantic**（2026 新） | 统一 15 种指标格式互转 | 佐证「指标定义应可迁移、可测试」（它带 golden-query 断言） |
| WrenAI | MDL → 语义引擎（Rust/DataFusion） | dry-plan 校验（已在前次调研） |

生产对照数据（dbt 2026 benchmark）：**语义层接地 98–100% vs 裸 text-to-SQL 84–90%** ——
「LLM 只选指标、引擎只编译」这条分工线的收益被再次证实。

## 2. AST 护栏：生产共识的五道检查（多篇文章收敛到同一套）

来源：querystack 的 allow-list 设计、arelion 的生产案例（上线后事故 0）、HuggingFace 官方建议等。
**反面教材：正则/关键字黑名单** —— `drop_date` 误杀（false positive）、
`DE/*comment*/LETE` 绕过（false negative）、嵌套子查询里的 DELETE 看不见（无结构感知）。

```python
tree = sqlglot.parse_one(sql, read=dialect)   # 解析失败即拒
1. 语句类型：tree 必须是 exp.Select（单语句：len(parse(sql)) == 1）
2. 全树禁节点：find_all(exp.Insert|Update|Delete|Drop|Alter|Create|Command|Merge) → 拒
   （子查询/CTE 深处的写操作同样被 find_all 抓到 —— 这是 AST 相对正则的本质优势）
3. 表白名单：find_all(exp.Table) 全部在业务模型实体内；
   ⚠ 战例：只查 t.name 放过了 other_schema.records —— **schema 限定名要一并校验**
4. 禁函数：pg_read_file / dblink / copy / load_file ...（文件/网络/管理面函数）
5. LIMIT 上限：缺失或超阈值即拒
```
纵深防御（应用层，不依赖 SQL 检查）：**只读 DB 角色 + statement_timeout + EXPLAIN dry-run
（非 ANALYZE）+ 执行前给人看 SQL**。

## 3. 对 Report Studio L5 的落地裁剪

我们的特殊性：**SQL 是编译产物，不是生成物** —— 发射器吃 rpx-plan/v1（闭合文法），
自由文本只有一处：业务模型 metric 的 `expression`（人可编辑）。所以：

| 检查 | 我们的做法 | 依据 |
|---|---|---|
| 发射 | **自研闭合发射器保留**（plan → SQL 字符串组装）；方言差异集中在引号/日期粒度/时间参数 | 我们不吃任意 SQL，生成面小；引入 sqlglot 当发射器收益低 |
| 表达式面 | **函数白名单**（SUM/AVG/COUNT/MIN/MAX + 四则运算），其余函数名一律拒 —— 比黑名单强，`pg_read_file` 这类天然进不来 | 白名单 > 黑名单（调研 §2） |
| 出口校验 | 双层：① stdlib 词法扫描（零依赖，永远在）② **若装了 sqlglot 则跑 AST 禁节点/单语句/LIMIT**（防御纵深，优雅降级） | Studio 坚持 stdlib 可跑；sqlglot 是可选增强 |
| 表白名单 | 发射器只引用业务模型实体表（JOIN 走 plan.joins）⇒ 天然闭合；出口再核一遍 + schema 限定名 | 战例教训 |
| 可审计 | 输出带 `definitions[]`（metric id + expression + label）—— 每个数字可回溯到口径 | 「Return the definition used」生产共识 |
| 时间 | 编译期解析预设为日期参数（已做）—— 明文承认「LLM 对『上周』不可靠，代码层解析」 | 调研明文建议 |

**Rust 侧对应物**（后续）：`rpx-ds` 执行用户 `dataset.sql` 时用 datafusion-sqlparser-rs
做同样的单语句/SELECT-only 门禁 —— 现在那条路是「SQL 原样交库执行」，护栏缺位。

## 4. 一句话结论

**生成不用借（闭合文法自研更稳），校验必须借（sqlglot 的 AST 按类型拒绝是行业标准做法），
护栏照抄 allow-list 五道检查 + 只读角色/超时的纵深防御。**
Cube 的查询形状反证了 rpx-query/v1 的设计是对的；「LLM 选指标、引擎编译」的分工
在 2026 的 benchmark 里依然是准确率的分水岭。
