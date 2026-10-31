# NL2SQL 开源项目调研与借鉴（2026-10-23）

> 目的：为 Report Studio 的「理解层」（自然语言 → SQL + 报表设计）找可借鉴的架构模式。
> 调研时间 2026-10-23；星数/状态以当时为准。

## 1. 项目盘点

| 项目 | 星数 | 路线 | 状态 | 值得借鉴的点 |
|---|---|---|---|---|
| **Vanna** (vanna-ai/vanna) | 23.8K | RAG：DDL + 文档 + Q/SQL 对 三类知识入向量库，检索后喂 LLM | MIT；⚠ 仓库 2026-03 已归档（架构仍值得读） | 三类训练素材的划分；「纠正即入训」飞轮；准确率对照实验 |
| **WrenAI** (Canner/WrenAI) | 17.9K | **语义层（MDL）** + Rust/DataFusion 语义引擎 + dry-plan 校验 + 记忆 | 活跃（GenBI 方向） | 「上下文层」= 版本化资产（可 PR 评审）；五层上下文；Generate→Deploy→**Know** 闭环 |
| **SuperSonic** (tencentmusic/supersonic) | 5.1K | ChatBI + Headless BI：语义解析流水线 | Java，活跃 | 流水线拆分：Schema Mapper → Semantic Parser → **Corrector** → Translator；规则/LLM 双轨解析器 |
| **DB-GPT** (eosphoros-ai/DB-GPT) | 20.2K | Agentic 数据助手（SQL + 代码 + 图表 + 报告） | 活跃 | 端到端「问数 → 出报告」的形态（与我们最像） |
| **Chat2DB** | 28.3K | SQL 客户端 + AI（微调 Chat2DB-SQL-7B） | 更新放缓 | 多库元数据理解；微调小模型做 SQL 专用任务 |
| **Dataherald** (Dataherald/dataherald) | 3.5K | API-first NL2SQL 引擎（Context Store + 评估 + 反馈） | ⚠ 2024-07 后更新停滞 | 模块化拆分（schema context / generation / evaluation / feedback）；Human-in-the-Loop |
| **PremSQL** (premAI-io/premsql) | — | 本地小模型 text-to-SQL 流水线 | 2025-02 后更新少 | 本地化/数据不出域的取舍 |
| **SemanticSchemaLinker** | 小 | schema 元数据抽取 + 归一 + 语义增强（嵌入）管线 | 2025-02 起 | **schema 元数据工程**：表/列/枚举值/外键/少样例全量入库供检索 |

学术侧：**TS-SQL**（测试用例驱动自纠错，Spider 修正集 EX 88.88%）、**MTSQL-R1**（Propose→Execute→E-Verify→M-Verify→Self-Correct 的 agent 循环）。

## 2. 五种可借鉴模式

### 模式 A：上下文三件套（Vanna）
不微调模型，把三类知识向量化，提问时检索拼进 prompt：
1. **DDL**（表结构）；2. **文档**（业务口径：「活跃客户」= 近 90 天下过单）；3. **问句-SQL 金样例**。
对照实验：只喂 schema ≈ **3%** 准确率；schema + 相关样例 + 文档 ≈ **80%**。20–50 条精选金样例即够起步；
**每次纠错后的 (问句, SQL) 回灌知识库**，准确率随使用爬升（生产数据：50+ 反馈信号后 94% → 97%）。

### 模式 B：语义层是版本化资产（WrenAI MDL）
业务含义（指标口径、join 路径、权限）写成**可 diff、可 PR 评审的文件**，不埋在 prompt 里。
五层上下文：结构（表/列/键）→ 语义（计算字段/规范表）→ 业务（口径定义）→ 操作（join 路径/治理）→ **行为（历史成功查询与纠错记忆）**。
关键流程：问题 → 取相关上下文 → MDL 上规划 → **dry-plan 校验**（先展开成物理 SQL 验合法性，失败给结构化错误）→ 执行 → **答案被确认后回存记忆**（Generate → Deploy → **Know**）。
另一个硬规则：**MDL 里没声明的表列对 agent 物理不可见** —— 幻觉 join 直接没了。

### 模式 C：中间语义表示，别让 LLM 直写物理 SQL（SuperSonic）
流水线：知识库（schema 词典/索引）→ **Schema Mapper**（问句里的实体对到 schema 元素）→ **Semantic Parser**（规则 + LLM 双轨，产出语义查询 S2SQL）→ **Semantic Corrector**（规则 + LLM 双轨校正）→ **Semantic Translator**（翻成物理 SQL）。
核心思想：**join/公式这类高级语法从 LLM 肩上卸到语义层**，LLM 只做意图理解。规则解析器覆盖常见场景（快、稳、可测），LLM 兜底长尾。

### 模式 D：执行反馈自纠错（TS-SQL / MTSQL-R1 / 生产实践）
生成 → 执行 → 捕获错误/空结果/测试偏差 → 带错误上下文重生成 → 直至通过。
TS-SQL 更进一步：自动生成**测试数据 + 期望结果代码**当外部裁判（不信任 LLM 自我语义判断），语义错误显著下降。
生产侧共识：**执行反馈是最客观的信号**（能跑吗？报错吗？空结果吗？违反约束吗？）。

### 模式 E：护栏 + 金标准评测集（生产共识）
- 护栏：AST 解析拒绝 DDL/DML、缺 LIMIT 自动补、只读账号 + 超时 + 行数上限、**执行前预览 + 人确认**、审计日志。
- 评测：≥30 条真实问句的金标准集（含歧义/多表 join/日期边界/空结果/权限敏感），**每次改 prompt/上下文/模型都重跑**。
- 反馈闭环分层：周（prompt/检索微调）→ 月（检索排序/同义词表）→ 季（金样例固化微调）。

## 3. 对 Report Studio 的映射

| 借鉴模式 | 我们已有的 | 缺口 |
|---|---|---|
| A 三件套 | **列元数据**（类型 + COMMENT 中文备注，2026-10-23 刚落）≈ DDL+文档；**59 份实例库模板 + 运行历史** ≈ 金样例矿 | 把「需求文本 → spec/SQL」的成功对沉淀成可检索的金样例库 |
| B 语义层 | **rpx-spec/v1 本身就是语义契约**（measure/agg/format/label/口径），且 `spec analyze` 可验算 —— 这点比多数项目强 | 口径知识散在模板里，没有「业务口径」独立层；无检索式记忆 |
| C 中间表示 | **spec → 编译 → 模板** 的管线完整；理解层输出 spec 而非裸 SQL 是自然选择 | Schema Mapper（问句实体 → 列/值映射）；Corrector（规则已有 lint/analyze，LLM 纠错未接） |
| D 执行反馈 | `dataset inspect` 能试跑 SQL；`spec analyze` 出 open_questions | 生成 → 执行 → 报错回喂 → 重生成的闭环；测试用例裁判 |
| E 护栏/评测 | SQL 原样交数据库执行（无改写）；运行产物有 diagnostics | 只读账号约束、LIMIT/超时注入、DDL/DML 拒绝、金标准评测集 |

## 4. 落地优先级（建议）

1. **Schema Mapper + 上下文组装**（确定性）：问句分词（已有中文二元滑窗）→ 列/备注/枚举值匹配 → 组装 prompt 上下文。这是模式 A/C 的地基，也是我们元数据层的直接下游。
2. **金样例库**（模式 A）：每次「用户确认过的 需求→spec/SQL」入库；`report list --query` 的中文检索直接复用做 few-shot 召回。
3. **理解层输出 = spec，不是裸 SQL**（模式 C）：SQL 只负责取数（dataset.sql），报表结构（维度/度量/口径/版式）走 spec + 已有编译器 —— join/聚合/分页不用 LLM 操心。
4. **执行反馈纠错环**（模式 D）：生成的 SQL 先 `dataset inspect` 试跑；报错/空结果带上下文重试（最多 N 次）；仍失败转人工。
5. **护栏**（模式 E）：SQL AST 检查（拒绝 DDL/DML）、自动 LIMIT、超时、执行前预览确认。
6. **金标准评测集**（模式 E）：30 条真实需求固化成回归，理解层每次改动必跑。

## 4.5 知识库形态：GraphRAG 值不值（2026-10-23 补）

### 图谱 RAG 三条路线（都不是同一个东西）

| 方案 | 本质 | 索引成本 | 增量更新 | 适合 |
|---|---|---|---|---|
| **MS GraphRAG** (35K★) | LLM 从文本抽三元组 + Leiden 社区 + 逐社区摘要 | **极高**（1M token 文档 ≈ $1k–3k） | 不支持，全量重建 | 静态大语料的**全局性问题**（"这些事故报告的共同主题"） |
| **LightRAG** (39K★) | 双层图（实体+主题）+ 向量融合，增量并入 | GraphRAG 的 1/10–1/5 | ✅ 增量 | 动态增长的知识库、事实/多跳问答 |
| **Graphiti** (30K★) | **双时态**图（事实何时为真），实时写入 | 按更新摊 | ✅ 实时 + 版本 | Agent 长期记忆、点时间回溯 |
| Youtu-GraphRAG（腾讯优图，2025-09 开源） | **Schema 引导**构图（属性/关系/关键词/社区四层）+ 子查询分解检索 | 比同类省 30%+（最高省 90% token） | — | 跨域迁移、成本敏感的落地 |
| SAG（2026 新路线） | 不建三元组：chunk 挂「索引卡」（event + entities 超边） | 低 | ✅ | 规避三元组谓词不稳（"使用/基于/支持"分裂） |

### 关键判断：我们的知识库**根本不需要「抽取」**

MS GraphRAG 贵在「LLM 从非结构化文本抽实体关系」。但我们的知识**本来就是结构化的：
表/列/外键/COMMENT/枚举值/金样例**，图是**现成的**，直接建属性图即可 —— 零抽取成本。
而且 2026 年的神经符号 GraphRAG 对比给了硬数据：**本体（Schema）引导的图谱正确性 79% vs 纯神经抽取 21%**，完整性 83% vs 17% ——
Schema 引导的图 > 自由抽取图，正好与 Youtu-GraphRAG 的结论互证。我们连「本体」都是现成的（库 Schema + rpx-spec 语义）。

### 我们的图谱形状（确定性工程，不用引图数据库）

```
(表 t_sales) --FK--> (表 t_region)          ← information_schema 的外键
   |N
(列 amount) --COMMENT--> "销售额"           ← 列元数据（已有 column_meta）
   |                    --取值--> 华北/华东   ← 枚举值采样（<50 distinct 时取）
   |                    --口径--> sum/标签    ← rpx-spec 的 measure 定义
(金样例) --覆盖--> [列集]                     ← 需求文本→spec/SQL 成功对
```

检索 = 问句分词（已有中文二元滑窗）→ 命中节点 → **沿 FK/join 边扩展一跳**（"销售额"→amount→t_sales→可 join 的 t_region）→ 拼上下文。
这正是 SuperSonic 的「Knowledge Base + Schema Mapper」，也避开了向量 RAG 的两个死穴：
① 向量只会"看脸"不会"顺藤摸瓜"（多跳 join 链检索率极低）；② chunk 切分把实体切碎。

### 选型结论

1. **不上 MS GraphRAG**（全局主题摘要对我们无用；抽取成本白花）。
2. **第一步手写轻量属性图**（SQLite/JSON + 现有分词器就够，几十张表规模用不上 Neo4j）：
   节点=表/列/值/术语，边=FK/COMMENT/口径/样例覆盖。增量维护 = 每次拉元数据时同步更新，天然是增量的。
3. **金样例用「超边」记**（一个样例覆盖多列），学 SAG 不硬拆三元组；样例间相似检索用已有 `report list --query`。
4. **运行历史/纠错记忆**若要「何时为真」的版本语义，再考虑 Graphiti 式双时态 —— 当前 revision 只 +1 的规模，先用简单 append 日志。

## 5. 一句话结论

行业已收敛成同一形状：**语义层/上下文层（版本化、可检索）→ 中间语义表示 → 校验器 → 执行反馈回灌记忆**。
我们比多数项目多走了一步（rpx-spec 就是可编译的中间表示 + spec analyze 事实核对），
缺的是前面的「Schema Mapper + 金样例记忆」和后面的「执行反馈回灌」—— 而这两块都是确定性工程 + 一个可插拔的 LLM 适配器，不需要重造引擎。
