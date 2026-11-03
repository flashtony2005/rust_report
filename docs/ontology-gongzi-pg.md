# 本体 · 实体与关系（gongzi-pg）

> 生成时间：2026/11/3 11:41:22　|　数据源：`gongzi-pg`　|　口径：主库 `service.*` 全部基表
> 来源：连库现算 —— **真实外键** + **gzxt-main 源码确认**（Java SQL join，证据带 file:line）+ **同名列候选**（代码列恰为另一表主键才推断）
> 关系可信度：真外键 > 源码确认 > 同名列候选；候选关系使用前建议人工确认。

## 整体说明

- **实体 278 个**：主库 service.* 全部基表 212 个 + 视图/物化视图 66 个，名称取库内注释（无注释则显示表名）。
- **关系 20 条**：真外键 2 · 源码确认 9 · 同名列候选 9。
- **维度枢纽**：`dwxxb`（单位信息表）——源码中 join 48 处（41 处经 current_dwxxb CTE 按 dwdm 相连），几乎全部工资业务表靠它取单位信息。
- **五类人员工资主表**：t101（在职在编）/ tcgz1（事业在职）/ tcltgz1（离退）/ t103 / tgygz，各自配有 *lsb 历史留痕表（按 主键+身份证+上月ym 相连做环比）。
- **对齐键约定**：dwdm=单位代码、sfzh=身份证号、ym=工资月份（t101↔jcjxb 按人+单位+月三键对齐）。

## 源码补充说明（gzxt-main 代码分析）

> 分析对象：`E:/project/gzxt-main 源码静态分析（2026-11-02）+ ima「gz」知识库文档（工资统发系统_业务需求文档 20260809-2 / 概要设计说明书 / 停发回退状态流转）`。Spring Boot + JPA 多模块（gongzi-service=工资主模块：44 实体 / 43 service / 38 repository；SQL 多写在 service 层字符串里）。

### 单位与组织（全局维度）

涉及表：`dwxxb`、`dwdzb`、`czzf_rydwdzb`、`czzf_zbdzb`

dwxxb=单位信息表，全系统维度枢纽（源码 join 48 处，按 dwdm 相连；概要设计亦列「单位信息表」）；dwdzb=单位对照主表；czzf_rydwdzb=财政支付侧人员单位对照（CzzfRydwdzb，2024-04 建档）；czzf_zbdzb=指标对照表（概要设计明确提及）。

### 五类人员工资主表（含环比留痕）

涉及表：`t101`、`t103`、`tcgz1`、`tcltgz1`、`tgygz`、`tgygz1`、`salarystatus`

权威定义（ima gz 知识库《业务需求文档 20260809-2》）：t101=行政在职人员工资（含公务员及机关工勤，人社导入接口同名）、t103=行政离休人员工资、tcgz1=事业在职人员工资、tcltgz1=事业离休人员工资、tgygz=雇员工资、tax=税费数据（1.7 节，字段结构与 t101 对齐）。各主表配 *lsb 历史留痕表（t101lsb 等），ConfirmService.java:312-343 按 主键+身份证+上月ym 关联做环比。salarystatus 管工资状态（月份切换/入库/确认流程，见业务流程）。月度快照在 202605 等 schema（先删后插）。

### 基础绩效（标准→考核→申报→明细）

涉及表：`jcjxb`、`gz_jcjxkjb`、`gz_jcjxsqb`、`gz_jcjxsqmxb`、`gz_jcjxbzb`、`gz_jcjxkhb`

链路：jcjxb/gz_jcjxbzb 标准表 → gz_jcjxkjb / GZ_jcjxkhb 考核表 → gz_jcjxsqb 申报表 → gz_jcjxsqmxb 申报明细。源码证据：t101 left join jcjxb on 人(sfzh)+单位(dwdm)+月(ym)（ConfirmService.java:115）；pid=jcjx01 关联申报（202403 快照校验 SQL）。需求文档提到「绩效扣减比例表|管理部门、扣减」。库内另有 GZ_jcjxtjb 统计表。

### 事项申报与审批（附件流）

涉及表：`gz_shxmb`、`gz_shxmsqb`、`gz_shxmsqbspb`、`gz_shxmsqbfile`、`gz_shxm_file`

shxm=事项：gz_shxmb 事项表 → gz_shxmsqb 申报 → gz_shxmsqbspb 审批 → *file 两张附件表。库内注释：休假申报（GZ_holidaysqb 休假表 / GZ_holidaysqmxb 休假明细表）、奖励（GZ_honorsqb / GZ_honorsqmxb 奖励明细表）同属此审批流。

### 停发与扣减

涉及表：`gz_tfsqb`、`GZ_deduction`、`gz_qjglmxb`

gz_tfsqb 更可能是**停发申报表**（tf=停发；GzTfsqbService 按 zt/sszq 查询、join dwxxb 取单位名称；ima 知识库《停发回退状态流转》：待处理→停发申请→单位审核→部门审核→财政审核→停发成功/失败，state=8=财政审核不通过——与 zt 字段用法吻合；原「退费」解释存疑，待核）；GZ_deduction=扣减表（扣减项按 pid/dwdm/sfzh 挂人，doc/ 有《工资扣减工作流程.docx》，需求文档有「规范性津贴补贴扣减」）；gz_qjglmxb=请假管理明细（待核）。

### 财政支付与银行

涉及表：`gz_czzfbank`、`gz_czzfbank_dw`

czzf=财政支付：gz_czzfbank 银行账户，gz_czzfbank_dw 按单位维度的支付银行信息（待核：与 czzf_*dzb 对照表的关系）。

### 人员增减与系统支撑

涉及表：`gz_rybdb`、`gz_ryjcglb`、`gz_ywlxb`、`gz_zwmcb`、`gz_bbs`、`gz_bbscontent`、`openrenshe_log`、`tax`、`field_dictionary`

gz_rybdb=人员变动表（doc/ 有《人员增减变动视图》查询包，对应业务流程「工资数据入库」的前端变动管理）、gz_ryjcglb=人员进出管理类（待核）；gz_ywlxb=业务类型字典、gz_zwmcb=（待核）；gz_bbs/gz_bbscontent=公告板（源码唯一 repository 层 JOIN）；openrenshe_log=人社接口调用日志（人社接口.md：Bearer token 认证、报文含 dwdm）；field_dictionary=字段字典（develop-service，当前仅 enterprise 表 1 条）。

### 资料来源（ima「gz」知识库）

- 工资统发系统_业务需求文档（20260809-2）.docx —— 五类人员↔表权威定义：t101=行政在职、t103=行政离休、tcgz1=事业在职、tcltgz1=事业离休、tgygz=雇员工资、tax=税费数据；业务流程：工资月份切换→数据入库→税费录入确认→单位/主管单位确认→数据导出→停发/回退
- 3.2.2软件概要设计说明书.doc —— 人社对接导入接口（T101=公务员及机关工勤、T103=离休公务员、事业…）；表名|功能说明清单；单位信息表/指标对照表
- 停发回退状态流转（知识库图片）—— 状态机：待处理→停发申请→单位审核→部门审核→财政审核→停发成功/失败（state=8 财政审核不通过）
- 人社接口.md —— 接口报文含 dwdm 等字段；/gongzi/api/v1/ren-sh* 认证走 Bearer token
- 工资系统数据校验和提示规则doc(1).doc（D:/Backup/Documents）—— 业务闸门规则清单：工资数据下载 / 银行数据下载(MDB) / 基础绩效导入校验 / 税费导入校验 / 工资数据确认
- gzxt-main DwxxbService.listMain（源码，用户 2026-11-03 指路）—— 主页单位上报表的真实口径：zt 状态机 × lx 财政类型，状态列 dwxxb.gzzt/jcjxzt/sjzt

### 数据校验规则（业务闸门，出自《工资系统数据校验和提示规则doc(1).doc》）

**工资数据下载**

- 下载工资单不会改变主页的已下载单位数，但数据库的单位信息表（dwxxb）会记录哪个单位有下载过 —— dwxxb 兼作『单位是否已下载』的痕迹表

**银行数据下载（下载 MDB）**

- 有单位没确认工资或有在途停发（gz_tfsqb）→ 不能下载 MDB
- 可以下载时，所属镇区的全部单位变成下载状态，主页的已下载单位数才会变化 —— 银行下载是后续导入动作的闸门

**基础绩效导入 Excel 校验（全部/部分更新）**

- 组织部还没导入行政在职（t101）和行政离休（t103）数据 → 提示「请先导入工资数据」（绩效导入依赖工资主表先入库）
- 月份未填写或错误 → 提示「只能导入当前工资月份的数据」（ym 闸门）
- 金额为空默认 0；非数字 → 「第几行的[内容]不是数字格式」；填公式 → 「第几行的[内容]不能使用公式」
- 单位代码+身份证号对应不上 → 提示「单位名称[单位代码]的姓名[身份证号]」不存在（jcjxb↔t101 按人+单位双键校验，与源码 join 三键一致）
- 所属镇区的单位已下载银行数据 → 提示「银行已下载数据，不能导入」
- 有基础绩效数据未导入 → 提示「有1人未导入基础绩效数据，无基础绩效的人员需登记为0，保证基础绩效数据和工资数据对应」（人数对齐闸门）

**税费数据导入 Excel 校验（tax 表）**

- 还没有行政在职（t101）和事业在职（tcgz1）数据 → 提示「请先导入工资数据」
- 单位代码未填或错误 → 提示「只能导入所选单位的数据」（按 dwdm 限定单位）
- 金额为空默认 0；非数字/公式 → 同基础绩效的格式校验
- 身份证号对应不上 → 提示「姓名[身份证号]」不存在
- 所属镇区的单位已下载银行数据 → 提示「银行已下载数据，不能导入」
- 有税费数据未导入 → 提示「有1人未导入税费数据，无税费的人员需登记为0，保证税费数据和工资数据对应，工资数据才能进行确认」（人数对齐闸门）

**工资数据确认（最终闸门）**

- 基础绩效和工资数据没对应 → 单位经办确认时提示「有人员没有基础绩效数据，请补齐数据再确认」
- 税费数据和工资数据没对应 → 确认时提示「有人员没有税费数据，请补齐数据再确认」

### 单位上报情况（dashboard 口径，出自 gzxt-main DwxxbService.listMain(zt,lx) + 用户提供的 dashboard 统计原始 SQL（2026-11-03）+ 统计视图 service.v_tj）

- **状态机**：1、已下载 2、应上报 3、未确认 4、已确认 5、未下载（用户可见的状态机）
- **财政类型**：1、市级财政工资 2、市级财政绩效 3、市级财政税金 4、镇区财政工资 5、镇区财政绩效 6、镇区财政税金
- **状态真列**：上报状态真列：dwxxb.gzzt=工资确认状态、dwxxb.jcjxzt=绩效确认状态、dwxxb.sjzt=税金确认状态（值如『已确认』，COALESCE 判空）；dwxxb.gzbz=下载标志（值『下载』=已下载）；镇区/市级的分界 = sszq='市级财政' 与否
- **判定规则**：应上报=当月单位集全量；已确认=gzzt/jcjxzt/sjzt='已确认'（税金另有『无 t101/tcgz1 数据视同已确认』）；未确认=<>'已确认' 且税金要求 EXISTS 有工资数据；已下载/未下载=gzbz 是否='下载'；当月单位集 = service.current_dwxxb join dwxxb on dwdm

**单位上报统计 · 汇总口径（dashboard 原始实现，5 行 × 6 列）**

```sql
-- bz=指标行（应上报/已确认/未确认/已下载/未下载），zt=1..5
-- c1..c3 = 市级财政 工资/绩效/税金；c4..c6 = 镇区财政 工资/绩效/税金
SELECT '应上报单位数' AS bz,
  sum(CASE WHEN dwxxb.sszq='市级财政' THEN 1 ELSE 0 END) AS c1,
  sum(CASE WHEN dwxxb.sszq='市级财政' THEN 1 ELSE 0 END) AS c2,
  sum(CASE WHEN dwxxb.sszq='市级财政' THEN 1 ELSE 0 END) AS c3,
  sum(CASE WHEN dwxxb.sszq<>'市级财政' THEN 1 ELSE 0 END) AS c4,
  sum(CASE WHEN dwxxb.sszq<>'市级财政' THEN 1 ELSE 0 END) AS c5,
  sum(CASE WHEN dwxxb.sszq<>'市级财政' THEN 1 ELSE 0 END) AS c6,
  1 AS zt
FROM service.current_dwxxb JOIN service.dwxxb ON current_dwxxb.dwdm::text = dwxxb.dwdm::text
UNION ALL
SELECT '已确认单位数' AS bz,
  sum(CASE WHEN COALESCE(dwxxb.gzzt,'')='已确认' AND dwxxb.sszq='市级财政' THEN 1 ELSE 0 END) AS c1,
  sum(CASE WHEN COALESCE(dwxxb.jcjxzt,'')='已确认' AND dwxxb.sszq='市级财政' THEN 1 ELSE 0 END) AS c2,
  sum(CASE WHEN (COALESCE(dwxxb.sjzt,'')='已确认' OR NOT EXISTS (SELECT t101.xh FROM service.t101 WHERE t101.t10100::text=dwxxb.dwdm::text) AND NOT EXISTS (SELECT tcgz1.xh FROM service.tcgz1 WHERE tcgz1.tcgz101::text=dwxxb.dwdm::text)) AND dwxxb.sszq='市级财政' THEN 1 ELSE 0 END) AS c3,
  sum(CASE WHEN COALESCE(dwxxb.gzzt,'')='已确认' AND dwxxb.sszq<>'市级财政' THEN 1 ELSE 0 END) AS c4,
  sum(CASE WHEN COALESCE(dwxxb.jcjxzt,'')='已确认' AND dwxxb.sszq<>'市级财政' THEN 1 ELSE 0 END) AS c5,
  sum(CASE WHEN (COALESCE(dwxxb.sjzt,'')='已确认' OR NOT EXISTS (SELECT t101.xh FROM service.t101 WHERE t101.t10100::text=dwxxb.dwdm::text) AND NOT EXISTS (SELECT tcgz1.xh FROM service.tcgz1 WHERE tcgz1.tcgz101::text=dwxxb.dwdm::text)) AND dwxxb.sszq<>'市级财政' THEN 1 ELSE 0 END) AS c6,
  2 AS zt
FROM service.dwxxb JOIN service.current_dwxxb ON current_dwxxb.dwdm::text = dwxxb.dwdm::text
UNION ALL
SELECT '未确认单位数' AS bz,
  sum(CASE WHEN COALESCE(dwxxb.gzzt,'')<>'已确认' AND dwxxb.sszq='市级财政' THEN 1 ELSE 0 END) AS c1,
  sum(CASE WHEN COALESCE(dwxxb.jcjxzt,'')<>'已确认' AND dwxxb.sszq='市级财政' THEN 1 ELSE 0 END) AS c2,
  sum(CASE WHEN COALESCE(dwxxb.sjzt,'')<>'已确认' AND (EXISTS (SELECT t101.xh FROM service.t101 WHERE t101.t10100::text=dwxxb.dwdm::text) OR EXISTS (SELECT tcgz1.xh FROM service.tcgz1 WHERE tcgz1.tcgz101::text=dwxxb.dwdm::text)) AND dwxxb.sszq='市级财政' THEN 1 ELSE 0 END) AS c3,
  sum(CASE WHEN COALESCE(dwxxb.gzzt,'')<>'已确认' AND dwxxb.sszq<>'市级财政' THEN 1 ELSE 0 END) AS c4,
  sum(CASE WHEN COALESCE(dwxxb.jcjxzt,'')<>'已确认' AND dwxxb.sszq<>'市级财政' THEN 1 ELSE 0 END) AS c5,
  sum(CASE WHEN COALESCE(dwxxb.sjzt,'')<>'已确认' AND (EXISTS (SELECT t101.xh FROM service.t101 WHERE t101.t10100::text=dwxxb.dwdm::text) OR EXISTS (SELECT tcgz1.xh FROM service.tcgz1 WHERE tcgz1.tcgz101::text=dwxxb.dwdm::text)) AND dwxxb.sszq<>'市级财政' THEN 1 ELSE 0 END) AS c6,
  3 AS zt
FROM service.dwxxb JOIN service.current_dwxxb ON current_dwxxb.dwdm::text = dwxxb.dwdm::text
UNION ALL
SELECT '已下载单位数' AS bz,
  sum(CASE WHEN COALESCE(dwxxb.gzbz,'')='下载' AND dwxxb.sszq='市级财政' THEN 1 ELSE 0 END) AS c1,
  NULL::numeric AS c2, NULL::numeric AS c3,
  sum(CASE WHEN COALESCE(dwxxb.gzbz,'')='下载' AND dwxxb.sszq<>'市级财政' THEN 1 ELSE 0 END) AS c4,
  NULL::numeric AS c5, NULL::numeric AS c6,
  4 AS zt
FROM service.dwxxb JOIN service.current_dwxxb ON current_dwxxb.dwdm::text = dwxxb.dwdm::text
UNION ALL
SELECT '未下载单位数' AS bz,
  sum(CASE WHEN COALESCE(dwxxb.gzbz,'')<>'下载' AND dwxxb.sszq='市级财政' THEN 1 ELSE 0 END) AS c1,
  NULL::numeric AS c2, NULL::numeric AS c3,
  sum(CASE WHEN COALESCE(dwxxb.gzbz,'')<>'下载' AND dwxxb.sszq<>'市级财政' THEN 1 ELSE 0 END) AS c4,
  NULL::numeric AS c5, NULL::numeric AS c6,
  5 AS zt
FROM service.dwxxb JOIN service.current_dwxxb ON current_dwxxb.dwdm::text = dwxxb.dwdm::text;
```

**经典问题：按所属镇街统计单位上报情况 —— 分镇区 SQL（统计视图 v_tj）**

```sql
SELECT fl AS "分类", sszq AS "所属镇区", "已确认", "待处理", "下载", "工资总数"
FROM service.v_tj
ORDER BY fl, sszq;

（fs 汇总口径（全市 6 个数）见上面 summary_sql；手工等价按 sszq 分组写法：SELECT dwxxb.sszq, COUNT(*) AS 单位数, SUM(CASE WHEN COALESCE(dwxxb.gzzt,'')='已确认' THEN 1 ELSE 0 END) AS 工资已确认, SUM(CASE WHEN COALESCE(dwxxb.jcjxzt,'')='已确认' THEN 1 ELSE 0 END) AS 绩效已确认, SUM(CASE WHEN COALESCE(dwxxb.sjzt,'')='已确认' THEN 1 ELSE 0 END) AS 税金已确认 FROM dwxxb GROUP BY dwxxb.sszq ORDER BY dwxxb.sszq）
```

## 实体清单

| 实体 | 中文名（表注释） | 类型 | 代码实体类 | 关系数 |
|---|---|---|---|---|
| service.GZ_bkfb | — | 表 | — | 0 |
| service.GZ_deduction | 扣减表 | 表 | — | 0 |
| service.GZ_holidaysqb | 休假表 | 表 | — | 0 |
| service.GZ_holidaysqmxb | 休假明细表 | 表 | — | 0 |
| service.GZ_honorsqb | — | 表 | — | 0 |
| service.GZ_honorsqmxb | 奖励明细表 | 表 | — | 0 |
| service.GZ_jcjxbzb | 基础绩效标准表 | 表 | GzJcjxbzb（gongzi.domain） | 0 |
| service.GZ_jcjxkhb | 基础绩效考核表 | 表 | GzJcjxkhb（gongzi.domain） | 0 |
| service.GZ_jcjxtjb | 基础绩效统计表 | 表 | — | 0 |
| service.GZ_khb | — | 表 | — | 0 |
| service.GZ_khjgcfcfdzb | — | 表 | — | 0 |
| service.GZ_khjgkhlbdzb | — | 表 | — | 0 |
| service.GZ_khjgsqb | — | 表 | — | 0 |
| service.GZ_nzjsqb | — | 表 | — | 0 |
| service.GZ_nzjsqmxb | — | 表 | — | 0 |
| service.GZ_patchpost | 变动表 | 表 | — | 0 |
| service.GZ_patchsqb | — | 表 | — | 0 |
| service.GZ_patchtfgz | — | 表 | — | 0 |
| service.GZ_qhdmb | 区号 | 表 | — | 0 |
| service.GZ_qtsjbsb | — | 表 | — | 0 |
| service.GZ_rybdb_bak | — | 表 | — | 0 |
| service.GZ_rybdbdjyb | — | 表 | — | 0 |
| service.GZ_rybdtsjyb | — | 表 | — | 0 |
| service.GZ_rybdyyb | — | 表 | — | 0 |
| service.GZ_rybtdcdzb | — | 表 | — | 0 |
| service.GZ_rygzzwdzb | 职务对照表 | 表 | — | 0 |
| service.GZ_ryjbgzb | 人员级别表 | 表 | — | 0 |
| service.GZ_ryjryyb | — | 表 | — | 0 |
| service.GZ_rysfbdb | — | 表 | — | 0 |
| service.GZ_rytcyyb | — | 表 | — | 0 |
| service.GZ_ryxxb | — | 表 | — | 0 |
| service.GZ_shxm_file | 审核项目文件 | 表 | GzShxmFile（gongzi.domain） | 0 |
| service.GZ_shxmb | 审核项目表 | 表 | GzShxmb（gongzi.domain） | 0 |
| service.GZ_shxmsqb | 审核项目申请表 | 表 | GzShxmsqb（gongzi.domain） | 0 |
| service.GZ_shxmsqb_bak | 审核项目申请表 | 表 | — | 0 |
| service.GZ_shxmsqbfile | 审核项目申请文件表 | 表 | GzShxmsqbfile（gongzi.domain） | 0 |
| service.GZ_shxmsqbspb | 审核项目审批表 | 表 | GzShxmsqbspb（gongzi.domain） | 0 |
| service.GZ_shxmsqbspb_bak | 审核项目审批表 | 表 | — | 0 |
| service.GZ_sjbbsb | — | 表 | — | 0 |
| service.GZ_sjbspb | — | 表 | — | 0 |
| service.GZ_zcb | 增册表 | 表 | — | 0 |
| service.GZ_zfbtsqb | — | 表 | — | 0 |
| service.GZ_zfbtsqmxerrb | — | 表 | — | 0 |
| service.GZ_zfbtzwdzb | — | 表 | — | 0 |
| service.GZ_zwb | — | 表 | — | 0 |
| service.application | 应用管理 | 表 | Application（common.domain） | 0 |
| service.authority | 角色信息 | 表 | — | 1 |
| service.authority_menu | 角色菜单 | 表 | — | 0 |
| service.authority_permission | 角色权限 | 表 | — | 0 |
| service.bfaa | 补发补扣申请表 | 表 | Bfaa（gongzi.domain） | 0 |
| service.bfab | 补发补扣审批表 | 表 | Bfab（gongzi.domain） | 0 |
| service.bsaa | 数据包报送 | 表 | Bsaa（gongzi.domain） | 0 |
| service.bsab | 数据包审批表 | 表 | Bsab（gongzi.domain） | 0 |
| service.bsab_bak | 数据包审批表 | 表 | — | 0 |
| service.czdqqhb | 财政地区区划表 | 表 | — | 0 |
| service.czzf_rydwdzb | 人员单位对照表 | 表 | CzzfRydwdzb（gongzi.domain） | 0 |
| service.czzf_rydwdzb_bak | 人员单位对照表 | 表 | — | 0 |
| service.czzf_rydwdzb_bak1 | 人员单位对照表 | 表 | — | 0 |
| service.czzf_rydwdzb_bak2 | 人员单位对照表 | 表 | — | 0 |
| service.czzf_rydwdzb_bak3 | 人员单位对照表 | 表 | — | 0 |
| service.czzf_rydwdzb_bak4 | 人员单位对照表 | 表 | — | 0 |
| service.czzf_zbdzb | 指标对照表 | 表 | CzzfZbdzb（gongzi.domain） | 0 |
| service.czzf_zbdzb_bak | 指标对照表 | 表 | — | 0 |
| service.czzf_zbdzb_bak1 | 指标对照表 | 表 | — | 0 |
| service.czzf_zbdzb_bak2 | 指标对照表 | 表 | — | 0 |
| service.czzf_zbdzb_bak3 | 指标对照表 | 表 | — | 0 |
| service.czzf_zfb | — | 表 | — | 0 |
| service.del_gz_bbs_bak | — | 表 | — | 0 |
| service.del_gz_bbscontent_bak | — | 表 | — | 0 |
| service.department | 部门信息 | 表 | — | 0 |
| service.dictionary | 数据字典 | 表 | — | 0 |
| service.dwdzb | — | 表 | Dwdzb（gongzi.domain） | 0 |
| service.dwryb | — | 表 | — | 0 |
| service.dwxxb | 单位信息表 | 表 | Dwxxb（gongzi.domain） | 4 |
| service.dwxxb0 | — | 表 | — | 0 |
| service.dwxxb2 | — | 表 | — | 0 |
| service.dwxxb_202605 | 单位信息表 | 表 | — | 0 |
| service.dwxxb_bak | 单位信息表 | 表 | — | 0 |
| service.dwxxb_bak1 | — | 表 | — | 0 |
| service.dwxxb_bak2 | 单位信息表 | 表 | — | 0 |
| service.dwxxblsb | 单位信息历史表 | 表 | — | 0 |
| service.encc | — | 表 | Encc（common.domain） | 0 |
| service.enterprise | 企业信息 | 表 | — | 0 |
| service.error_log | 错误日志 | 表 | ErrorLog（common.domain） | 0 |
| service.field_dictionary | 表字段数据字典配置 | 表 | FieldDictionary（develop.domain） | 0 |
| service.file | 附件信息 | 表 | — | 0 |
| service.gygzdwb | — | 表 | — | 0 |
| service.gz_bbs | — | 表 | GzBbs（gongzi.domain） | 0 |
| service.gz_bbscontent | — | 表 | GzBbscontent（gongzi.domain） | 0 |
| service.gz_czzfbank | 单位支付账号信息维护 | 表 | GzCzzfbank（gongzi.domain） | 0 |
| service.gz_czzfbank_dw | 单位银行申请 | 表 | GzCzzfbankDw（gongzi.domain） | 0 |
| service.gz_jcjxkjb | 基础绩效扣减表 | 表 | GzJcjxkjb（gongzi.domain） | 0 |
| service.gz_jcjxsqb | 基础绩效申请表 | 表 | GzJcjxsqb（gongzi.domain） | 0 |
| service.gz_jcjxsqblsb | — | 表 | — | 0 |
| service.gz_jcjxsqmxb | 基础绩效明细表 | 表 | GzJcjxsqmxb（gongzi.domain） | 0 |
| service.gz_jcjxsqmxb_lsb | — | 表 | — | 0 |
| service.gz_khjgdcb | — | 表 | — | 0 |
| service.gz_qjglmxb | 请假管理明细表 | 表 | GzQjglmxb（gongzi.domain） | 0 |
| service.gz_rybdb | 人员变动表 | 表 | GzRybdb（gongzi.domain） | 0 |
| service.gz_rygzxjtb | 工作性津贴表 | 表 | — | 0 |
| service.gz_ryjcglb | 人员进出管理表 | 表 | GzRyjcglb（gongzi.domain） | 0 |
| service.gz_tfspb | — | 表 | — | 0 |
| service.gz_tfspb_bak | — | 表 | — | 0 |
| service.gz_tfsqb | 停发回退申请表 | 表 | GzTfsqb（gongzi.domain） | 1 |
| service.gz_tfsqb_bak | 停发回退申请表 | 表 | — | 0 |
| service.gz_ywlxb | 业务类型表 | 表 | GzYwlxb（gongzi.domain） | 0 |
| service.gz_zwmcb | — | 表 | GzZwmcb（gongzi.domain） | 0 |
| service.gzhzd | — | 表 | — | 0 |
| service.hmac | — | 表 | Hmac（common.domain） | 0 |
| service.hmac_log | — | 表 | HmacLog（common.domain） | 0 |
| service.jcjxb | 基础绩效表 | 表 | Jcjxb（gongzi.domain） | 2 |
| service.jcjxb_bak | 基础绩效表 | 表 | — | 0 |
| service.jcjxb_bak1 | 基础绩效表 | 表 | — | 0 |
| service.jcjxb_bak2 | 基础绩效表 | 表 | — | 0 |
| service.jcjxb_bak3 | 基础绩效表 | 表 | — | 0 |
| service.jcjxb_bak4 | 基础绩效表 | 表 | — | 0 |
| service.jcjxblsb | — | 表 | — | 0 |
| service.menu | 系统菜单 | 表 | — | 0 |
| service.oauth2_authorization | — | 表 | — | 1 |
| service.oauth2_authorization_consent | — | 表 | — | 1 |
| service.oauth2_registered_client | — | 表 | — | 0 |
| service.openrenshe_log | — | 表 | OpenrensheLog（gongzi.domain） | 0 |
| service.operation_log | 操作日志 | 表 | OperationLog（common.domain） | 0 |
| service.operation_log_lsb | — | 表 | — | 0 |
| service.permission | 权限设置 | 表 | — | 0 |
| service.private_word | 工作流常用词 | 表 | — | 1 |
| service.salarystatus | — | 表 | Salarystatus（gongzi.domain） | 0 |
| service.subsystem | 子系统 | 表 | — | 0 |
| service.system_config | — | 表 | — | 0 |
| service.t101 | 行政在职人员工资表 | 表 | T101（gongzi.domain） | 3 |
| service.t101_202605_bak | 行政在职人员工资表 | 表 | — | 0 |
| service.t101_202606 | 行政在职人员工资表 | 表 | — | 0 |
| service.t101_202606_bak | 行政在职人员工资表 | 表 | — | 0 |
| service.t101_202607 | 行政在职人员工资表 | 表 | — | 0 |
| service.t101_202607_bak | 行政在职人员工资表 | 表 | — | 0 |
| service.t101_202608 | 行政在职人员工资表 | 表 | — | 0 |
| service.t101_202608_all | 行政在职人员工资表 | 表 | — | 0 |
| service.t101_202608_bak | 行政在职人员工资表 | 表 | — | 0 |
| service.t101_202609 | 行政在职人员工资表 | 表 | — | 0 |
| service.t101_202609_bak | 行政在职人员工资表 | 表 | — | 0 |
| service.t101_202611 | 行政在职人员工资表 | 表 | — | 0 |
| service.t101_bak | — | 表 | — | 0 |
| service.t101_test | — | 表 | — | 0 |
| service.t101lsb | — | 表 | — | 1 |
| service.t101tf | — | 表 | — | 0 |
| service.t103 | 行政离休人员工资表 | 表 | T103（gongzi.domain） | 1 |
| service.t103_202605_bak | 行政离休人员工资表 | 表 | — | 0 |
| service.t103_202606 | 行政离休人员工资表 | 表 | — | 0 |
| service.t103_202606_bak | 行政离休人员工资表 | 表 | — | 0 |
| service.t103_202607 | 行政离休人员工资表 | 表 | — | 0 |
| service.t103_202607_bak | 行政离休人员工资表 | 表 | — | 0 |
| service.t103_202608 | 行政离休人员工资表 | 表 | — | 0 |
| service.t103_202608_all | 行政离休人员工资表 | 表 | — | 0 |
| service.t103_202608_bak | 行政离休人员工资表 | 表 | — | 0 |
| service.t103_202609 | 行政离休人员工资表 | 表 | — | 0 |
| service.t103_202609_bak | 行政离休人员工资表 | 表 | — | 0 |
| service.t103_202611 | 行政离休人员工资表 | 表 | — | 0 |
| service.t103_bak | — | 表 | — | 0 |
| service.t103hb | — | 表 | — | 0 |
| service.t103lsb | — | 表 | — | 1 |
| service.t103tf | — | 表 | — | 0 |
| service.tax | 税费数据录入表 | 表 | Tax（gongzi.domain） | 1 |
| service.tax_1 | — | 表 | — | 0 |
| service.tcgz1 | 事业在职人员工资表 | 表 | Tcgz1（gongzi.domain） | 1 |
| service.tcgz1_202605_bak | 事业在职人员工资表 | 表 | — | 0 |
| service.tcgz1_202606 | 事业在职人员工资表 | 表 | — | 0 |
| service.tcgz1_202606_bak | 事业在职人员工资表 | 表 | — | 0 |
| service.tcgz1_202607 | 事业在职人员工资表 | 表 | — | 0 |
| service.tcgz1_202607_bak | 事业在职人员工资表 | 表 | — | 0 |
| service.tcgz1_202608 | 事业在职人员工资表 | 表 | — | 0 |
| service.tcgz1_202608_all | 事业在职人员工资表 | 表 | — | 0 |
| service.tcgz1_202608_bak | 事业在职人员工资表 | 表 | — | 0 |
| service.tcgz1_202609 | 事业在职人员工资表 | 表 | — | 0 |
| service.tcgz1_202609_bak | 事业在职人员工资表 | 表 | — | 0 |
| service.tcgz1_202611 | 事业在职人员工资表 | 表 | — | 0 |
| service.tcgz1lsb | — | 表 | — | 1 |
| service.tcgz1tf | — | 表 | — | 0 |
| service.tcltgz1 | 事业离休人员工资表 | 表 | Tcltgz1（gongzi.domain） | 1 |
| service.tcltgz1_202605_bak | 事业离休人员工资表 | 表 | — | 0 |
| service.tcltgz1_202606 | 事业离休人员工资表 | 表 | — | 0 |
| service.tcltgz1_202606_bak | 事业离休人员工资表 | 表 | — | 0 |
| service.tcltgz1_202607 | 事业离休人员工资表 | 表 | — | 0 |
| service.tcltgz1_202607_bak | 事业离休人员工资表 | 表 | — | 0 |
| service.tcltgz1_202608 | 事业离休人员工资表 | 表 | — | 0 |
| service.tcltgz1_202608_all | 事业离休人员工资表 | 表 | — | 0 |
| service.tcltgz1_202608_bak | 事业离休人员工资表 | 表 | — | 0 |
| service.tcltgz1_202609 | 事业离休人员工资表 | 表 | — | 0 |
| service.tcltgz1_202609_bak | 事业离休人员工资表 | 表 | — | 0 |
| service.tcltgz1_202611 | 事业离休人员工资表 | 表 | — | 0 |
| service.tcltgz1hb | — | 表 | — | 0 |
| service.tcltgz1lsb | — | 表 | — | 1 |
| service.tcltgz1tf | — | 表 | — | 0 |
| service.temp | — | 表 | — | 0 |
| service.tfgz_file | — | 表 | — | 0 |
| service.tfspb | — | 表 | — | 0 |
| service.tfsqb | — | 表 | — | 0 |
| service.tgygz | 雇员工资 | 表 | Tgygz（gongzi.domain） | 0 |
| service.tgygz1 | 雇员工资 | 表 | Tgygz1（gongzi.domain） | 0 |
| service.user | 用户信息 | 表 | — | 1 |
| service.user_authority | 用户角色关系 | 表 | — | 10 |
| service.user_key | — | 表 | — | 0 |
| service.usermas | 单位用户信息表 | 表 | — | 1 |
| service.usermas_20251119 | 单位用户信息表 | 表 | — | 1 |
| service.usermas_20260120 | 单位用户信息表 | 表 | — | 1 |
| service.usermas_backup | 单位用户信息表 | 表 | — | 1 |
| service.usermas_bak | — | 表 | — | 1 |
| service.usermas_copy1 | 单位用户信息表 | 表 | — | 1 |
| service.usermas_sq | 单位用户信息表 | 表 | — | 1 |
| service.work_secret | 工作秘密设置信息表 | 表 | WorkSecret（common.domain） | 0 |
| service.workflow_permission | 工作流权限设置 | 表 | — | 0 |
| service.zbdzb_sj | — | 表 | — | 0 |
| service.ztcount | — | 表 | — | 0 |
| service.current_dwbm | — | 视图 | — | 0 |
| service.current_dwxxb | — | 视图 | — | 0 |
| service.dw_alldm | — | 视图 | — | 0 |
| service.dw_czall | — | 视图 | — | 0 |
| service.dw_newcz | — | 视图 | — | 0 |
| service.dw_ry | — | 视图 | — | 0 |
| service.dw_sjcz | — | 视图 | — | 0 |
| service.dw_sum_rylx | — | 视图 | — | 0 |
| service.dw_sum_rylx_bak | — | 视图 | — | 0 |
| service.dw_sum_rylx_bak1 | — | 视图 | — | 0 |
| service.dw_sum_rylx_bak2 | — | 视图 | — | 0 |
| service.dw_sum_rylx_t30 | — | 视图 | — | 0 |
| service.dw_tfy | — | 视图 | — | 0 |
| service.dw_tfymc | — | 视图 | — | 0 |
| service.mv_export_zy | — | 视图 | — | 0 |
| service.v_bank_dw | — | 视图 | — | 0 |
| service.v_bank_gydw | — | 视图 | — | 0 |
| service.v_cfry | — | 视图 | — | 0 |
| service.v_dqym | — | 视图 | — | 0 |
| service.v_dwsszq | — | 视图 | — | 0 |
| service.v_export_czdetail | — | 视图 | — | 0 |
| service.v_export_czdetail_202607 | — | 视图 | — | 0 |
| service.v_export_czdetail_202608 | — | 视图 | — | 0 |
| service.v_export_czdetail_202609 | — | 视图 | — | 0 |
| service.v_export_czdetail_bak | — | 视图 | — | 0 |
| service.v_export_zy | 五表工资统发归口代码(zy)核对视图：统发易编码/名称 + 归口代码/名称，含补全来源，可分析 zy IS NULL | 视图 | — | 0 |
| service.v_exportcz | — | 视图 | — | 0 |
| service.v_exportcz_t30 | — | 视图 | — | 0 |
| service.v_exportgygz | — | 视图 | — | 0 |
| service.v_exportgygz_bak | — | 视图 | — | 0 |
| service.v_exportgygzdetail | — | 视图 | — | 0 |
| service.v_exportgygzdetail1 | — | 视图 | — | 0 |
| service.v_exportgygzdetail_bak | — | 视图 | — | 0 |
| service.v_gzbdqk | — | 视图 | — | 0 |
| service.v_gzbdqk_bak | — | 视图 | — | 0 |
| service.v_gzdw | — | 视图 | — | 0 |
| service.v_gzready | — | 视图 | — | 0 |
| service.v_gzry | — | 视图 | — | 0 |
| service.v_jcjxfgdw | — | 视图 | — | 0 |
| service.v_jcjxfgry | — | 视图 | — | 0 |
| service.v_jcjxrtj | — | 视图 | — | 0 |
| service.v_jcjxry | — | 视图 | — | 0 |
| service.v_report_tj2 | — | 视图 | — | 0 |
| service.v_rybdjk | — | 视图 | — | 0 |
| service.v_rybdmx | — | 视图 | — | 0 |
| service.v_rydwmap | — | 视图 | — | 0 |
| service.v_sfry | — | 视图 | — | 0 |
| service.v_sjql | — | 视图 | — | 0 |
| service.v_t101 | — | 视图 | — | 0 |
| service.v_t10100 | — | 视图 | — | 0 |
| service.v_t101_23_30_bak | — | 视图 | — | 0 |
| service.v_t101_t30 | — | 视图 | — | 0 |
| service.v_t103 | — | 视图 | — | 0 |
| service.v_tcgz1 | — | 视图 | — | 0 |
| service.v_tcltgz1 | — | 视图 | — | 0 |
| service.v_tfqk | — | 视图 | — | 0 |
| service.v_tfry | — | 视图 | — | 0 |
| service.v_tj | — | 视图 | — | 0 |
| service.v_ywhj | — | 视图 | — | 0 |
| service.v_ywjc | — | 视图 | — | 0 |
| service.v_zbdzb | — | 视图 | — | 0 |
| service.v_zynull | — | 视图 | — | 0 |
| service.view_current_ry | — | 视图 | — | 0 |
| service.view_gygz_t101 | — | 视图 | — | 0 |
| service.view_gygz_tcgz1 | — | 视图 | — | 0 |
| service.view_lsb_ry | — | 视图 | — | 0 |

## 视图清单（66 个，含物化视图）

> 依赖关系来自 pg_rewrite/pg_depend（视图定义真实引用），每视图列出其派生来源。

| 视图 | 中文名（注释） | 依赖的基表/视图 |
|---|---|---|
| service.current_dwbm | — | `service.current_dwbm`、`service.dwxxb`、`service.salarystatus`、`service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1` |
| service.current_dwxxb | — | `service.current_dwxxb`、`service.dwxxb`、`service.salarystatus`、`service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1` |
| service.dw_alldm | — | `service.czzf_rydwdzb`、`service.czzf_zbdzb`、`service.dw_alldm` |
| service.dw_czall | — | `service.czzf_zbdzb`、`service.dw_czall` |
| service.dw_newcz | — | `service.czzf_zbdzb`、`service.dw_newcz` |
| service.dw_ry | — | `service.dw_czall`、`service.dw_ry`、`service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1` |
| service.dw_sjcz | — | `service.czzf_zbdzb`、`service.dw_sjcz` |
| service.dw_sum_rylx | — | `service.dw_sum_rylx`、`service.salarystatus`、`service.v_t101`、`service.v_t103`、`service.v_tcgz1`、`service.v_tcltgz1` |
| service.dw_sum_rylx_bak | — | `service.dw_czall`、`service.dw_sum_rylx_bak`、`service.salarystatus`、`service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1` |
| service.dw_sum_rylx_bak1 | — | `service.dw_czall`、`service.dw_sum_rylx_bak1`、`service.salarystatus`、`service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1` |
| service.dw_sum_rylx_bak2 | — | `service.dw_czall`、`service.dw_sum_rylx_bak2`、`service.salarystatus`、`service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1` |
| service.dw_sum_rylx_t30 | — | `service.dw_sum_rylx_t30`、`service.salarystatus`、`service.v_t101_t30`、`service.v_t103`、`service.v_tcgz1`、`service.v_tcltgz1` |
| service.dw_tfy | — | `service.czzf_rydwdzb`、`service.czzf_zbdzb`、`service.dw_tfy` |
| service.dw_tfymc | — | `service.czzf_zbdzb`、`service.dw_tfymc` |
| service.mv_export_zy | — | `service.v_export_zy` |
| service.v_bank_dw | — | `service.dw_czall`、`service.gz_czzfbank`、`service.gz_czzfbank_dw`、`service.v_bank_dw` |
| service.v_bank_gydw | — | `service.dw_czall`、`service.gz_czzfbank`、`service.gz_czzfbank_dw`、`service.v_bank_gydw` |
| service.v_cfry | — | `service.salarystatus`、`service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1`、`service.v_cfry` |
| service.v_dqym | — | `service.salarystatus`、`service.v_dqym` |
| service.v_dwsszq | — | `service.dwxxb`、`service.v_dwsszq` |
| service.v_export_czdetail | — | `service.czdqqhb`、`service.dw_newcz`、`service.salarystatus`、`service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1`、`service.v_export_czdetail` |
| service.v_export_czdetail_202607 | — | `service.czdqqhb`、`service.salarystatus`、`service.t101_202607`、`service.t103_202607`、`service.tcgz1_202607`、`service.tcltgz1_202607`、`service.v_export_czdetail_202607` |
| service.v_export_czdetail_202608 | — | `service.czdqqhb`、`service.salarystatus`、`service.t101_202608`、`service.t103_202608`、`service.tcgz1_202608`、`service.tcltgz1_202608`、`service.v_export_czdetail_202608` |
| service.v_export_czdetail_202609 | — | `service.czdqqhb`、`service.salarystatus`、`service.t101_202609`、`service.t103_202609`、`service.tcgz1_202609`、`service.tcltgz1_202609`、`service.v_export_czdetail_202609` |
| service.v_export_czdetail_bak | — | `service.czdqqhb`、`service.dw_czall`、`service.salarystatus`、`service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1`、`service.v_export_czdetail_bak` |
| service.v_export_zy | 五表工资统发归口代码(zy)核对视图：统发易编码/名称 + 归口代码/名称，含补全来源，可分析 zy IS NULL | `service.czzf_rydwdzb`、`service.czzf_zbdzb`、`service.dw_newcz`、`service.salarystatus`、`service.t101`、`service.t101lsb`、`service.t103`、`service.t103lsb`、`service.tcgz1`、`service.tcgz1lsb`、`service.tcltgz1`、`service.tcltgz1lsb` …（共 14 个） |
| service.v_exportcz | — | `service.dw_sum_rylx`、`service.v_bank_dw`、`service.v_exportcz`、`service.v_zbdzb` |
| service.v_exportcz_t30 | — | `service.dw_czall`、`service.dw_sum_rylx_t30`、`service.gz_czzfbank`、`service.gz_czzfbank_dw`、`service.v_exportcz_t30`、`service.v_zbdzb` |
| service.v_exportgygz | — | `service.dw_czall`、`service.salarystatus`、`service.tgygz`、`service.v_bank_gydw`、`service.v_exportgygz`、`service.v_zbdzb` |
| service.v_exportgygz_bak | — | `service.dw_czall`、`service.dw_newcz`、`service.gz_czzfbank`、`service.salarystatus`、`service.tgygz`、`service.v_exportgygz_bak`、`service.v_zbdzb` |
| service.v_exportgygzdetail | — | `service.czdqqhb`、`service.dw_czall`、`service.salarystatus`、`service.tgygz`、`service.v_exportgygzdetail` |
| service.v_exportgygzdetail1 | — | `service.dw_czall`、`service.salarystatus`、`service.tgygz`、`service.v_exportgygzdetail1` |
| service.v_exportgygzdetail_bak | — | `service.dw_czall`、`service.salarystatus`、`service.tgygz`、`service.v_exportgygzdetail_bak` |
| service.v_gzbdqk | — | `service.jcjxb`、`service.salarystatus`、`service.t101`、`service.t101lsb`、`service.t103`、`service.t103lsb`、`service.tcgz1`、`service.tcgz1lsb`、`service.tcltgz1`、`service.tcltgz1lsb`、`service.v_gzbdqk` |
| service.v_gzbdqk_bak | — | `service.jcjxb`、`service.salarystatus`、`service.t101`、`service.t101lsb`、`service.t103`、`service.t103lsb`、`service.tcgz1`、`service.tcgz1lsb`、`service.tcltgz1`、`service.tcltgz1lsb`、`service.v_gzbdqk_bak` |
| service.v_gzdw | — | `service.v_gzdw`、`service.v_gzry` |
| service.v_gzready | — | `service.t101`、`service.t101lsb`、`service.t103`、`service.t103lsb`、`service.tcgz1`、`service.tcgz1lsb`、`service.tcltgz1`、`service.tcltgz1lsb`、`service.v_dqym`、`service.v_gzready` |
| service.v_gzry | — | `service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1`、`service.v_gzry` |
| service.v_jcjxfgdw | — | `service.v_jcjxfgdw`、`service.v_jcjxfgry` |
| service.v_jcjxfgry | — | `service.jcjxb`、`service.t101lsb`、`service.v_dqym`、`service.v_jcjxfgry` |
| service.v_jcjxrtj | — | `service.jcjxb`、`service.t101lsb`、`service.t103lsb`、`service.tcgz1lsb`、`service.tcltgz1lsb`、`service.v_dqym`、`service.v_jcjxrtj` |
| service.v_jcjxry | — | `service.salarystatus`、`service.t101`、`service.t103`、`service.v_jcjxry` |
| service.v_report_tj2 | — | `service.current_dwxxb`、`service.dwxxb`、`service.t101`、`service.tcgz1`、`service.v_report_tj2` |
| service.v_rybdjk | — | `service.v_rybdjk`、`service.v_rybdmx` |
| service.v_rybdmx | — | `service.salarystatus`、`service.t101`、`service.t101lsb`、`service.t103`、`service.t103lsb`、`service.tcgz1`、`service.tcgz1lsb`、`service.tcltgz1`、`service.tcltgz1lsb`、`service.v_rybdmx` |
| service.v_rydwmap | — | `service.czzf_rydwdzb`、`service.v_rydwmap` |
| service.v_sfry | — | `service.jcjxb`、`service.salarystatus`、`service.t101`、`service.tcgz1`、`service.v_sfry` |
| service.v_sjql | — | `service.dwxxb`、`service.salarystatus`、`service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1`、`service.v_sjql` |
| service.v_t101 | — | `service.t101`、`service.v_t101` |
| service.v_t10100 | — | `service.t101`、`service.v_t10100` |
| service.v_t101_23_30_bak | — | `service.t101`、`service.v_t101_23_30_bak` |
| service.v_t101_t30 | — | `service.t101`、`service.v_t101_t30` |
| service.v_t103 | — | `service.t103`、`service.v_t103` |
| service.v_tcgz1 | — | `service.tcgz1`、`service.v_tcgz1` |
| service.v_tcltgz1 | — | `service.tcltgz1`、`service.v_tcltgz1` |
| service.v_tfqk | — | `service.v_dqym`、`service.v_tfqk`、`service.v_tfry` |
| service.v_tfry | — | `service.t101lsb`、`service.t103lsb`、`service.tcgz1lsb`、`service.tcltgz1lsb`、`service.v_tfry` |
| service.v_tj | — | `service.dwxxb`、`service.v_tj` |
| service.v_ywhj | — | `service.v_ywhj`、`service.v_ywjc` |
| service.v_ywjc | — | `service.GZ_jcjxbzb`、`service.czzf_rydwdzb`、`service.czzf_zbdzb`、`service.dwxxb`、`service.jcjxb`、`service.t101`、`service.t101lsb`、`service.t103`、`service.t103lsb`、`service.tcgz1`、`service.tcgz1lsb`、`service.tcltgz1` …（共 19 个） |
| service.v_zbdzb | — | `service.czzf_zbdzb`、`service.v_zbdzb` |
| service.v_zynull | — | `service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1`、`service.v_zynull` |
| service.view_current_ry | — | `service.t101`、`service.t103`、`service.tcgz1`、`service.tcltgz1`、`service.view_current_ry` |
| service.view_gygz_t101 | — | `service.dw_czall`、`service.salarystatus`、`service.tgygz`、`service.view_gygz_t101` |
| service.view_gygz_tcgz1 | — | `service.salarystatus`、`service.tgygz`、`service.view_gygz_tcgz1` |
| service.view_lsb_ry | — | `service.t101lsb`、`service.t103lsb`、`service.tcgz1lsb`、`service.tcltgz1lsb`、`service.view_lsb_ry` |

## 关系清单

| 1 端 | N 端 | 类型 | 依据 |
|---|---|---|---|
| service.authority | service.user_authority | 1:N | 外键约束 |
| service.user | service.user_authority | 1:N | 外键约束 |
| service.dwxxb | service.gz_tfsqb | 源码 | gzxt-main 源码 join：gz_tfsqb left join dwxxb on gz_tfsqb.dwdm=dwxxb.dwdm（GzTfsqbService.java:89） |
| service.dwxxb | service.jcjxb | 源码 | gzxt-main 源码 join：jcjxb left join dwxxb on jcjxb.dwdm=dwxxb.dwdm（DwCzallService.java:616） |
| service.dwxxb | service.t101 | 源码 | gzxt-main 源码 join：from t101 left join dwxxb on t10100=dwdm（ConfirmService.java:105） |
| service.dwxxb | service.tax | 源码 | gzxt-main 源码 join：tax left join dwxxb on tax01=dwdm（TaxService.java:87） |
| service.t101 | service.jcjxb | 源码 | gzxt-main 源码 join：t101 left join jcjxb on lower(t10105)=lower(sfzh) 且 t10100=dwdm 且 t101.ym=jcjxb.ym（人+单位+月对齐，ConfirmService.java:115） |
| service.t101 | service.t101lsb | 源码 | gzxt-main 源码 join：t101 left join t101lsb on t10100+t10105 且 lsb.ym=上月（环比留痕，ConfirmService.java:312） |
| service.t103 | service.t103lsb | 源码 | gzxt-main 源码 join：t103 left join t103lsb on t10300+t10304 且 lsb.ym=上月（ConfirmService.java:336） |
| service.tcgz1 | service.tcgz1lsb | 源码 | gzxt-main 源码 join：tcgz1 left join tcgz1lsb on tcgz101+tcgz106 且 lsb.ym=上月（ConfirmService.java:328） |
| service.tcltgz1 | service.tcltgz1lsb | 源码 | gzxt-main 源码 join：tcltgz1 left join tcltgz1lsb on tcltgz100+tcltgz104 且 lsb.ym=上月（ConfirmService.java:343） |
| service.oauth2_authorization_consent | service.oauth2_authorization | 候选 | 同名列 registered_client_id：service.oauth2_authorization_consent 的主键，出现在 service.oauth2_authorization |
| service.user_authority | service.private_word | 候选 | 同名列 user_id：service.user_authority 的主键，出现在 service.private_word |
| service.user_authority | service.usermas | 候选 | 同名列 user_id：service.user_authority 的主键，出现在 service.usermas |
| service.user_authority | service.usermas_20251119 | 候选 | 同名列 user_id：service.user_authority 的主键，出现在 service.usermas_20251119 |
| service.user_authority | service.usermas_20260120 | 候选 | 同名列 user_id：service.user_authority 的主键，出现在 service.usermas_20260120 |
| service.user_authority | service.usermas_backup | 候选 | 同名列 user_id：service.user_authority 的主键，出现在 service.usermas_backup |
| service.user_authority | service.usermas_bak | 候选 | 同名列 user_id：service.user_authority 的主键，出现在 service.usermas_bak |
| service.user_authority | service.usermas_copy1 | 候选 | 同名列 user_id：service.user_authority 的主键，出现在 service.usermas_copy1 |
| service.user_authority | service.usermas_sq | 候选 | 同名列 user_id：service.user_authority 的主键，出现在 service.usermas_sq |

## 源码证据索引（gzxt-main）

- `ConfirmService.java:105/312/328/336/343` —— t101↔dwxxb、t101↔jcjxb、四对主表→历史表
- `DwCzallService.java:616` —— jcjxb↔dwxxb
- `TaxService.java:87` —— tax↔dwxxb
- `GzTfsqbService.java:89` —— gz_tfsqb↔dwxxb
