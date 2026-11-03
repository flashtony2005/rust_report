/**
 * 本体解析器（四步）—— **纯函数，无网络**。
 *
 * 四步对应 `src/views/OntologyView.tsx` 的四个步骤，每一步都要能单独讲清楚：
 *   ① 识别：这句话里哪些词命中了维度/度量，落在哪张表的哪个字段
 *   ② 定主表：从关系图里挑出「中心表」，其余分支都从它出发
 *   ③ 分头聚合：多条 1:N 边必须各自先在**原生粒度**聚合，否则行数相乘、金额重复
 *   ④ 组装输出：拼 Grain-Aligned SQL / Polars，并给出结果预览
 *
 * 第 ③ 步是全部意义所在：不预聚合时，客户 C001（2 订单 × 2 贷款）会被算成 4 行，
 * 订单金额与贷款本金各被计入 2 次。这里的 `compare` 就是把这两个数**算出来摆在一起**，
 * 而不是写一句"会翻倍"。
 */
import {
  CUSTOMERS,
  LOANS,
  ONTOLOGY,
  ORDERS,
  type CustomerRow,
  type LoanRow,
  type OrderRow,
} from '../data/ontology';

/** 把 `resolve()` 的产物适配成后端 `POST /ontology` 的落库请求（字段名对齐 `onto_step` / `onto_hit`）。 */
import type { OntologySaveRequest, OntoStep, OntoHit } from './api';

// ---------------------------------------------------------------------------
// 类型
// ---------------------------------------------------------------------------

export interface Hit {
  token: string;
  kind: 'dim' | 'metric' | 'entity';
  target: string;
  /** 落点：`表.字段` 或口径表达式。 */
  landing: string;
  /** 命中依据：哪个词、哪种匹配规则。 */
  evidence: string;
}

export interface PerCustomer {
  customer: CustomerRow;
  ltv: number;
  orders: number;
  loans: number;
  overdue: number;
  principal: number;
}

export type SumCol = 'ltv' | 'principal';

export interface CompareTable {
  headers: string[];
  rows: string[][];
  /** 这一张表里有哪几列金额（只出现在意图里选中的口径）。 */
  cols: SumCol[];
  /** 行数 + 各金额列的合计。 */
  totals: { rows: number } & Record<SumCol, number>;
}

export interface ResultTable {
  headers: string[];
  rows: Array<Array<string | number>>;
}

export interface Resolution {
  intent: string;
  /** ① 识别 */
  hits: Hit[];
  entities: string[];
  dim: string;
  metrics: string[];
  /** ② 定主表 */
  edges: typeof ONTOLOGY.edges;
  center: { entity: string; why: string; fanOut: number };
  risk: { multiple: boolean; notes: string[] };
  /** ③ 分头聚合 */
  compare: {
    naive: CompareTable;
    grain: CompareTable;
    /** 被放大的是哪个口径（只选了贷款类口径时不该拿 LTV 说事）。 */
    inflationMetric: SumCol;
    inflation: number;
  };
  /** ④ 组装输出 */
  sql: string;
  polars: string;
  preview: ResultTable;
}

// ---------------------------------------------------------------------------
// 意图匹配（规则确定，可逐条核对；不接 LLM 是故意的 —— 这一页讲的是"确定性"）
// ---------------------------------------------------------------------------

const DIM_RULES: Array<{ dim: string; words: string[] }> = [
  { dim: 'level', words: ['等级', '级别', 'level', '会员'] },
  { dim: 'region', words: ['大区', '区域', '地区', 'region'] },
];

const METRIC_RULES: Array<{ metric: string; words: string[] }> = [
  { metric: 'ltv', words: ['ltv', '价值', '销售', '金额', '客单'] },
  { metric: 'overdue_rate', words: ['逾期', '风险', '不良'] },
  { metric: 'principal', words: ['授信', '贷款', '本金', '借款'] },
];

/** 中文里没有词边界，这里靠「包含」判命中；命中词原样回显，便于人核对。 */
function hitWords(intent: string, words: string[]): string[] {
  const text = intent.toLowerCase();
  return words.filter((w) => text.includes(w.toLowerCase()));
}

export function matchIntent(intent: string): {
  dim: string;
  metrics: string[];
  hits: Hit[];
} {
  const hits: Hit[] = [];

  let dim = 'level';
  for (const rule of DIM_RULES) {
    const found = hitWords(intent, rule.words);
    if (found.length) {
      dim = rule.dim;
      hits.push({
        token: found[0],
        kind: 'dim',
        target: ONTOLOGY.dims.find((d) => d.id === rule.dim)!.label,
        landing: `customer.${ONTOLOGY.dims.find((d) => d.id === rule.dim)!.column}`,
        evidence: `意图含「${found.join('/')}」→ 维度命中`,
      });
      break;
    }
  }
  if (!hits.some((h) => h.kind === 'dim')) {
    hits.push({
      token: '（未指明）',
      kind: 'dim',
      target: ONTOLOGY.dims[0].label,
      landing: `customer.${ONTOLOGY.dims[0].column}`,
      evidence: '没有维度词 → 取默认维度（客户等级）',
    });
  }

  const metrics: string[] = [];
  for (const rule of METRIC_RULES) {
    const found = hitWords(intent, rule.words);
    if (!found.length) continue;
    const m = ONTOLOGY.metrics.find((x) => x.id === rule.metric)!;
    metrics.push(m.id);
    hits.push({
      token: found[0],
      kind: 'metric',
      target: m.label,
      landing: `${m.entity}: ${m.expr}`,
      evidence: `意图含「${found.join('/')}」→ 口径命中`,
    });
  }
  if (!metrics.length) {
    // 什么都没命中时给一条最保守的口径，并**明说**这是兜底
    metrics.push('ltv');
    const m = ONTOLOGY.metrics[0];
    hits.push({
      token: '（兜底）',
      kind: 'metric',
      target: m.label,
      landing: `${m.entity}: ${m.expr}`,
      evidence: '没有度量词 → 取默认口径（客户价值 LTV）',
    });
  }

  const entityIds = new Set<string>(['customer']);
  for (const id of metrics) {
    const m = ONTOLOGY.metrics.find((x) => x.id === id);
    if (m) entityIds.add(m.entity);
  }
  for (const id of entityIds) {
    const e = ONTOLOGY.entities.find((x) => x.id === id)!;
    hits.push({
      token: e.name,
      kind: 'entity',
      target: e.name,
      landing: `${e.id}（${e.rows} 行）`,
      evidence: id === 'customer' ? '维度落在客户表 → 必须带上' : `口径 ${id === 'order' ? 'LTV' : '贷款类'} 需要`,
    });
  }

  return { dim, metrics, hits };
}

// ---------------------------------------------------------------------------
// 计算：按客户原生粒度聚合（这是"正确"的那一半）
// ---------------------------------------------------------------------------

function aggregate(): PerCustomer[] {
  return CUSTOMERS.map((customer) => {
    const orders = ORDERS.filter((o) => o.customer_id === customer.id);
    const loans = LOANS.filter((l) => l.customer_id === customer.id);
    return {
      customer,
      ltv: orders.reduce((sum, o) => sum + o.amount - o.discount, 0),
      orders: orders.length,
      loans: loans.length,
      overdue: loans.reduce((sum, l) => sum + l.is_overdue, 0),
      principal: loans.reduce((sum, l) => sum + l.principal, 0),
    };
  });
}

const money = (v: number) => `¥${Math.round(v).toLocaleString('zh-CN')}`;
const pct = (v: number) => `${(v * 100).toFixed(1)}%`;

const COL_LABEL: Record<SumCol, string> = {
  ltv: 'LTV（口径值）',
  principal: '授信（口径值）',
};

/**
 * 一张对照表。
 *
 * `kind = 'naive'`：直接 JOIN —— 行数 = 订单数 × 贷款数，金额按**对方的行数**重复；
 * `kind = 'grain'`：分头聚合 —— 每个客户 1 行，金额就是口径值。
 *
 * 只有被选中的金额口径才出列：意图里只要授信时，表里就不该出现 LTV 那一列
 * （否则读者会以为 LTV 也参与了，还会疑惑为什么它"虚增 0%"）。
 */
function compareTable(rows: PerCustomer[], cols: SumCol[], kind: 'naive' | 'grain'): CompareTable {
  const headers = ['客户', '展开', ...cols.map((c) => COL_LABEL[c])];
  const totals: CompareTable['totals'] = { rows: 0, ltv: 0, principal: 0 };
  const out: string[][] = [];

  for (const r of rows) {
    const n = kind === 'naive' ? Math.max(1, r.orders) * Math.max(1, r.loans) : 1;
    // 直接 JOIN 时：订单金额按「贷款行数」重复，贷款本金按「订单行数」重复
    const mult: Record<SumCol, number> = {
      ltv: kind === 'naive' ? Math.max(1, r.loans) : 1,
      principal: kind === 'naive' ? Math.max(1, r.orders) : 1,
    };
    totals.rows += n;
    const cells = cols.map((c) => {
      const v = (c === 'ltv' ? r.ltv : r.principal) * mult[c];
      totals[c] += v;
      return money(v);
    });
    const interesting = kind === 'naive' ? n > 1 : r.orders > 1 || r.loans > 1;
    if (interesting && out.length < 5) {
      out.push([
        r.customer.name,
        kind === 'naive' ? `${r.orders}×${r.loans} = ${n} 行` : '已收敛 1 行',
        ...cells,
      ]);
    }
  }
  return { headers, rows: out, cols, totals };
}

// ---------------------------------------------------------------------------
// 输出：SQL / Polars / 结果预览
// ---------------------------------------------------------------------------

function buildSql(dimColumn: string, dimLabel: string, metrics: string[]): string {
  const usesOrder = metrics.includes('ltv');
  const usesLoan = metrics.includes('overdue_rate') || metrics.includes('principal');
  const ctes: string[] = [];
  if (usesOrder) {
    ctes.push(
      `order_grain AS (
    -- 订单自己算自己的：先收敛到 customer_id 粒度
    SELECT customer_id,
           SUM(amount) - SUM(discount) AS ltv
    FROM orders
    GROUP BY customer_id
)`,
    );
  }
  if (usesLoan) {
    const cols = ['customer_id'];
    if (metrics.includes('overdue_rate')) {
      cols.push('CAST(SUM(is_overdue) AS REAL) / NULLIF(COUNT(id), 0) AS overdue_rate');
    }
    if (metrics.includes('principal')) cols.push('SUM(principal) AS principal');
    ctes.push(
      `loan_grain AS (
    -- 贷款自己算自己的：同上，避免与订单行相乘
    SELECT ${cols.join(',\n           ')}
    FROM loans
    GROUP BY customer_id
)`,
    );
  }
  const selects = [`c.${dimColumn} AS "${dimLabel}"`, 'COUNT(c.id) AS "客户数"'];
  if (usesOrder) {
    selects.push('ROUND(SUM(COALESCE(o.ltv, 0)), 2) AS "客户价值"');
    selects.push('ROUND(AVG(COALESCE(o.ltv, 0)), 2) AS "平均客户价值"');
  }
  if (metrics.includes('overdue_rate')) {
    selects.push('ROUND(AVG(COALESCE(l.overdue_rate, 0)) * 100, 2) AS "平均逾期率%"');
  }
  if (metrics.includes('principal')) selects.push('ROUND(SUM(COALESCE(l.principal, 0)), 2) AS "授信总额"');

  const joins: string[] = [];
  if (usesOrder) joins.push('LEFT JOIN order_grain o ON c.id = o.customer_id');
  if (usesLoan) joins.push('LEFT JOIN loan_grain l ON c.id = l.customer_id');

  return `-- 本体解析器生成：分头聚合 + 按 ${dimLabel} 汇总
WITH
${ctes.join(',\n')}
SELECT ${selects.join(',\n       ')}
FROM customers c
${joins.join('\n')}
GROUP BY c.${dimColumn}
ORDER BY ${usesOrder ? '"客户价值" DESC' : '"客户数" DESC'};`;
}

function buildPolars(metrics: string[]): string {
  const orderAgg = metrics.includes('ltv')
    ? `    // ① 订单在 customer_id 粒度收敛\n    let order_grain = orders\n        .group_by([col("customer_id")])\n        .agg([(col("amount").sum() - col("discount").sum()).alias("ltv")]);\n`
    : '';
  const loanAgg = metrics.some((m) => m === 'overdue_rate' || m === 'principal')
    ? `    // ② 贷款在 customer_id 粒度收敛\n    let loan_grain = loans\n        .group_by([col("customer_id")])\n        .agg([\n            (col("is_overdue").cast(DataType::Float64).sum() / col("id").count())\n                .alias("overdue_rate"),\n            col("principal").sum().alias("principal"),\n        ]);\n`
    : '';
  return `// 与上面的 SQL 等价：先分头聚合，再 1:1 合并（Polars Lazy）
use polars::prelude::*;

pub fn resolve(customers: LazyFrame, orders: LazyFrame, loans: LazyFrame) -> PolarsResult<DataFrame> {
${orderAgg}${loanAgg}
    // ③ 主表左连两个已收敛的子表 —— 不会产生笛卡尔积
    let mut plan = customers;
${metrics.includes('ltv') ? '    plan = plan.join(order_grain, [col("id")], [col("customer_id")], JoinType::Left.into());\n' : ''}${
    metrics.some((m) => m === 'overdue_rate' || m === 'principal')
      ? '    plan = plan.join(loan_grain, [col("id")], [col("customer_id")], JoinType::Left.into());\n'
      : ''
  }
    plan.group_by([col("level")])
        .agg([
            col("id").count().alias("customers"),
            col("ltv").sum().alias("ltv"),
            col("overdue_rate").mean().alias("overdue_rate"),
        ])
        .sort(["ltv"], Default::default())
        .collect()
}`;
}

function preview(dim: string, metrics: string[], rows: PerCustomer[]): ResultTable {
  const groups = new Map<
    string,
    { count: number; ltv: number; rates: number[]; principal: number }
  >();
  for (const r of rows) {
    const key = dim === 'region' ? r.customer.region : r.customer.level;
    const g = groups.get(key) ?? { count: 0, ltv: 0, rates: [], principal: 0 };
    g.count += 1;
    g.ltv += r.ltv;
    g.rates.push(r.loans ? r.overdue / r.loans : 0);
    g.principal += r.principal;
    groups.set(key, g);
  }

  const headers = [ONTOLOGY.dims.find((d) => d.id === dim)!.label, '客户数'];
  if (metrics.includes('ltv')) headers.push('客户价值合计', '平均客户价值');
  if (metrics.includes('overdue_rate')) headers.push('平均逾期率');
  if (metrics.includes('principal')) headers.push('授信总额');

  const body = [...groups.entries()]
    .sort((a, b) => b[1].ltv - a[1].ltv)
    .map(([key, g]) => {
      const cells: Array<string | number> = [key, g.count];
      if (metrics.includes('ltv')) {
        cells.push(money(g.ltv), money(g.ltv / g.count));
      }
      if (metrics.includes('overdue_rate')) {
        cells.push(pct(g.rates.reduce((a, b) => a + b, 0) / (g.rates.length || 1)));
      }
      if (metrics.includes('principal')) cells.push(money(g.principal));
      return cells;
    });

  return { headers, rows: body };
}

// ---------------------------------------------------------------------------
// 主入口
// ---------------------------------------------------------------------------

export function resolve(intent: string): Resolution {
  const { dim, metrics, hits } = matchIntent(intent);
  const threshold = 0.05;

  const entityIds = new Set<string>(['customer']);
  for (const id of metrics) {
    const m = ONTOLOGY.metrics.find((x) => x.id === id);
    if (m) entityIds.add(m.entity);
  }
  const edges = ONTOLOGY.edges.filter((e) => entityIds.has(e.from) && entityIds.has(e.to));
  const oneToMany = edges.filter((e) => e.cardinality === '1:N');

  const rows = aggregate();
  // 只有金额类口径会被放大；比率类（逾期率）行变多但比率不变，所以不进对照表
  const sumCols: SumCol[] = (['ltv', 'principal'] as SumCol[]).filter((c) => metrics.includes(c));
  const cols: SumCol[] = sumCols.length ? sumCols : ['ltv'];
  const naive = compareTable(rows, cols, 'naive');
  const grain = compareTable(rows, cols, 'grain');
  // 用第一个选中的金额口径讲放大（都没选时退回 ltv，页面会标明是兜底列）
  const inflationMetric: SumCol = cols[0];
  const base = grain.totals[inflationMetric];
  const inflation = base ? naive.totals[inflationMetric] / base - 1 : 0;

  const notes: string[] = [];
  if (oneToMany.length > 1) {
    notes.push(
      `检测到 ${oneToMany.length} 条并列的 1:N 边（${oneToMany
        .map((e) => `${e.from}→${e.to}`)
        .join('、')}）：直接多表 JOIN 会让行数按乘积膨胀。`,
    );
    notes.push(
      '处置：每条放大边各自先用 `GROUP BY customer_id` 收敛成一行，再与主表 1:1 左连 —— 行数与金额都不会重复。',
    );
  } else {
    notes.push('只有一条放大边（或没有）：行数不会被乘积放大，但仍建议先聚合再连接，避免明细撑爆结果。');
  }

  return {
    intent,
    hits,
    entities: [...entityIds],
    dim,
    metrics,
    edges,
    center: {
      entity: 'customer',
      why: '两个 1:N 分支都从它出边，且它是唯一的维度来源 —— 左连它不会改变行数',
      fanOut: oneToMany.length,
    },
    risk: { multiple: oneToMany.length > 1, notes },
    compare: { naive, grain, inflationMetric, inflation },
    sql: buildSql(ONTOLOGY.dims.find((d) => d.id === dim)!.column, ONTOLOGY.dims.find((d) => d.id === dim)!.label, metrics),
    polars: buildPolars(metrics),
    preview: preview(dim, metrics, rows),
  };
}

/** 给界面用的固定口径摘要（rail 里显示"这套本体有什么"）。 */
export const ONTOLOGY_SUMMARY = {
  entities: ONTOLOGY.entities.length,
  edges: ONTOLOGY.edges.length,
  metrics: ONTOLOGY.metrics.length,
  rows: { customer: CUSTOMERS.length, order: ORDERS.length, loan: LOANS.length },
};

/**
 * 把一次四步解析结果适配成后端 `POST /ontology` 的落库请求。
 *
 * 后端只存不重算（见 `crates/rpx-server/src/ontology_http.rs` 的文件头注释），
 * 所以这里把每一步的"结论"装进 `payload_json`、命中逐条列出 —— 下次同样的问句
 * 可以直接从 `/ontology/{id}` 调出来看。
 */
export function toOntologyTrace(
  dsn: string,
  intent: string,
  res: Resolution,
): OntologySaveRequest {
  const hits: OntoHit[] = res.hits.map((h, i) => ({
    ord: i,
    token: h.token,
    kind: h.kind,
    target: h.target,
    evidence: h.evidence,
  }));

  const steps: OntoStep[] = [
    {
      step_no: 1,
      kind: 'identify',
      title: '识别',
      payload_json: JSON.stringify({
        hits: res.hits.map((h) => ({ token: h.token, kind: h.kind, landing: h.landing, evidence: h.evidence })),
      }),
    },
    {
      step_no: 2,
      kind: 'topology',
      title: '定主表',
      payload_json: JSON.stringify({ center: res.center, edges: res.edges }),
    },
    {
      step_no: 3,
      kind: 'grain',
      title: '分头聚合',
      payload_json: JSON.stringify({
        naive: res.compare.naive,
        grain: res.compare.grain,
        inflation: res.compare.inflation,
        inflationMetric: res.compare.inflationMetric,
      }),
    },
    {
      step_no: 4,
      kind: 'compose',
      title: '组装输出',
      sql: res.sql,
      payload_json: JSON.stringify({ preview: res.preview }),
    },
  ];

  return {
    dsn,
    question: intent,
    status: 'confirmed',
    center_entity: res.center.entity,
    fanout_edges: res.center.fanOut,
    sql: res.sql,
    steps,
    hits,
  };
}

export type { OrderRow, LoanRow };

// ---------------------------------------------------------------------------
// 真模型词表解析（postgres 数据源）：命中库注释里的真实维度/度量词，生成单表聚合 SQL
// ---------------------------------------------------------------------------

import type { Lexicon, LexiconWord } from './api';

export interface RealResult {
  hits: { token: string; kind: 'dim' | 'metric' | 'time'; landing: string; evidence: string; table: string }[];
  entities: string[];
  dimTokens: LexiconWord[];
  metricHits: LexiconWord[];
  center: string | null;
  why: string;
  notes: string[];
  sql: string;
}

/** 把一句话在真词表里匹配：token 是列的中文注释，命中即给出去向（表.列）。
 *  排序：匹配长度优先（所属镇>单位），备份/临时表（_bak/del_/快照月表）降权；某类词零命中时用"去尾字"模糊兜底（所属镇街↔所属镇区）。 */
export function resolveFromLexicon(question: string, lex: Lexicon): RealResult {
  const q = question.trim();
  const JUNK = /(^del_|(_bak|_backup|_copy|_tmp)\d*$|_\d{6})/i;
  const HUB = 'service.dwxxb'; // 维度枢纽表（源码分析结论）：同分时优先
  const scan = (words: LexiconWord[], kind: 'dim' | 'metric' | 'time') => {
    const out: { token: string; kind: 'dim' | 'metric' | 'time'; landing: string; evidence: string; table: string; _score: number }[] = [];
    for (const w of words) {
      if (!w.token || w.token.length < 2) continue;
      const junk = JUNK.test(w.table);
      let weak = false;
      let hitLen = 0;
      if (q.includes(w.token)) hitLen = w.token.length;
      else if (w.token.length >= 3 && q.includes(w.token.slice(0, -1))) {
        hitLen = w.token.length - 1;
        weak = true;
      }
      if (!hitLen) continue;
      out.push({
        token: w.token,
        kind,
        landing: w.landing,
        table: w.table,
        evidence:
          (weak ? '模糊命中（词尾不同）：' : '') +
          w.evidence +
          (junk ? '（备份/临时表，建议换正表）' : ''),
        _score: hitLen - (junk ? 10 : 0) + (w.table === HUB ? 2 : 0),
      });
    }
    return out.sort((a, b) => b._score - a._score).slice(0, 8);
  };
  const dimHits = scan(lex.dims, 'dim');
  const metricHitsRaw = scan(lex.metrics, 'metric');
  const timeHits = scan(lex.times, 'time');
  const hits = [...dimHits, ...metricHitsRaw, ...timeHits];

  // 去重：dimTokens 按词去重（同名维只留排名最高的一条落点），度量按落点去重
  const seenToken = new Set<string>();
  const dims = dimHits.filter((h) => !seenToken.has(h.token) && seenToken.add(h.token));
  const seenLanding2 = new Set<string>();
  const mets = metricHitsRaw.filter((h) => !seenLanding2.has(h.landing) && seenLanding2.add(h.landing));
  const entities = [...new Set(hits.map((h) => h.landing.split('.').slice(0, 2).join('.')))];
  const dimTokens = dims.slice(0, 3);
  const center = dimTokens[0]?.table ?? mets[0]?.table ?? null;
  // 度量只保留中心表自己的列 —— 别表的度量列拼进单表 SQL 会直接报错
  const metricHits = mets.filter((m) => m.table === center).slice(0, 6);

  const notes: string[] = [];
  if (!center) {
    notes.push('没有命中任何维度/度量词 —— 请换说法，或先给相关表/列补中文注释');
  } else {
    if (entities.length > 1) {
      notes.push(`命中 ${entities.length} 张表（${entities.join('、')}），SQL 只用中心表 —— 跨表聚合需人工确认关联键后再 JOIN，防扇出`);
    }
    const dropped = mets.filter((m) => m.table !== center).length;
    if (dropped) {
      notes.push(`有 ${dropped} 个度量落在其它表，未纳入本 SQL（避免跨表列名错误）`);
    }
  }
  if (timeHits.length) {
    notes.push(`命中时间词（${timeHits.map((h) => h.token).join('、')}）但未生成过滤条件 —— 时间窗口请在执行前手工加 WHERE`);
  }

  // 单表聚合 SQL：维度 GROUP BY，度量 SUM，恒带 COUNT
  let sql = '';
  if (center) {
    sql = buildGroupSql(
      center,
      dimTokens.map((d) => ({ col: d.landing.split('.')[2], alias: d.token })),
      metricHits.map((m) => ({ col: m.landing.split('.')[2], alias: `${m.token}合计` })),
    );
  }

  return {
    hits,
    entities,
    dimTokens,
    metricHits,
    center,
    why: center
      ? `维度落在 ${dimTokens[0]?.table ?? center}，单表聚合即可满足；无需跨表 JOIN（本次词表命中未发现并列放大边）`
      : '未命中词表',
    notes,
    sql,
  };
}

/** 由中心表+维度/度量（列名+别名）拼单表聚合 SQL —— 自动解析与人工修订后的重新生成共用。 */
export function buildGroupSql(
  center: string,
  dims: { col: string; alias: string }[],
  metrics: { col: string; alias: string }[],
): string {
  const sel: string[] = [];
  const groupCols: string[] = [];
  for (const d of dims) {
    sel.push(`t."${d.col}" AS "${d.alias}"`);
    groupCols.push(`t."${d.col}"`);
  }
  sel.push(`COUNT(*) AS "行数"`);
  for (const m of metrics) {
    sel.push(`SUM(t."${m.col}") AS "${m.alias}"`);
  }
  const lines = ['SELECT', sel.map((s) => `  ${s}`).join(',\n')];
  lines.push(`FROM ${center} t`);
  if (groupCols.length) {
    lines.push('GROUP BY');
    lines.push(`  ${groupCols.join(',\n  ')}`);
    lines.push('ORDER BY');
    lines.push(`  ${groupCols[0]}`);
  }
  return lines.join('\n') + ';';
}

/** 真模式痕迹落库结构（对齐 onto_step/onto_hit 字段）。 */
export function toRealTrace(
  dsn: string,
  question: string,
  rr: RealResult,
): {
  dsn: string;
  question: string;
  status: string;
  center_entity: string | null;
  fanout_edges: number;
  sql: string;
  steps: { step_no: number; kind: string; title: string; detail?: string; sql?: string }[];
  hits: { ord: number; token?: string; kind: string; target?: string; evidence?: string }[];
} {
  return {
    dsn,
    question,
    status: 'auto',
    center_entity: rr.center,
    fanout_edges: 0,
    sql: rr.sql,
    steps: [
      {
        step_no: 1,
        kind: 'identify',
        title: '识别（真词表）',
        detail: `命中 ${rr.hits.length} 条：${rr.hits.map((h) => h.token).join('、') || '无'}`,
      },
      {
        step_no: 2,
        kind: 'center',
        title: '定主表',
        detail: rr.why,
      },
      {
        step_no: 4,
        kind: 'compose',
        title: '组装输出',
        sql: rr.sql,
        detail: '单表聚合 SQL（执行要等 /query/execute）',
      },
    ],
    hits: rr.hits.map((h, i) => ({
      ord: i,
      token: h.token,
      kind: h.kind,
      target: h.landing,
      evidence: h.evidence,
    })),
  };
}
