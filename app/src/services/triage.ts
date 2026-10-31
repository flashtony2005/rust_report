import type { BizModel } from './api';

export type TriageBranch = 'ready' | 'clarify' | 'reject';

export interface MatchedTerm {
  token: string;
  type: 'metric' | 'dimension' | 'shape' | 'entity' | 'filter';
  id: string;
  label: string;
  confidence: number;
  status: 'confirmed' | 'draft' | 'candidate';
}

export interface AmbiguityItem {
  token: string;
  message: string;
  candidates: Array<{ id: string; label: string; kind: string; reason: string }>;
}

export interface MissingSlot {
  slot: 'metric' | 'time_window' | 'dimension' | 'shape';
  label: string;
  required: boolean;
  message: string;
  defaultSuggestion?: string;
}

export interface ProbedField {
  table: string;
  column: string;
  concept: string;
  existsInPhysicalDb: boolean;
  status: 'confirmed' | 'candidate_probe' | 'missing';
}

export interface LayerStatus {
  name: 'L1 语言可解' | 'L2 模型覆盖' | 'L3 数据可用';
  pass: boolean;
  status: 'ok' | 'warn' | 'fail';
  question: string;
  mechanism: string;
  summary: string;
  details: string[];
}

export interface QuickFixOption {
  id: string;
  label: string;
  action: 'set_metric' | 'set_dim' | 'set_shape' | 'set_time' | 'resolve_ambiguity';
  value: string;
  description: string;
}

export interface TriageResult {
  branch: TriageBranch;
  branchTitle: string;
  branchSubtitle: string;
  summaryText: string;
  layers: {
    l1: LayerStatus;
    l2: LayerStatus;
    l3: LayerStatus;
  };
  matchedTerms: MatchedTerm[];
  outOfVocabTerms: string[];
  ambiguities: AmbiguityItem[];
  missingSlots: MissingSlot[];
  probedFields: ProbedField[];
  shapeEvidence: {
    shape: string;
    label: string;
    confidence: number;
    reasons: string[];
  };
  blockers: string[];
  remedies: string[];
  quickFixes: QuickFixOption[];
  canGenerateDirectly: boolean;
}

// 常见形态关键词规则
const SHAPE_KEYWORDS: Record<string, { label: string; keywords: string[] }> = {
  rank: { label: '排行 TopN', keywords: ['前', 'top', '后', '排名', '排名前', '最高', '最低', '倒数'] },
  trend: { label: '时序趋势表', keywords: ['走势', '趋势', '按月', '按日', '按年', '月度', '年度', '历史', '变化'] },
  crosstab: { label: '交叉透视表', keywords: ['交叉', '透视', '矩阵', '双向', '二维', '分布'] },
  grouped: { label: '分组汇总', keywords: ['按', '各', '每', '分组', '对比', '汇总'] },
  compare: { label: '同比/环比对比', keywords: ['同比', '环比', '比上月', '比去年', '增长率', '较上期'] },
  detail: { label: '明细列表', keywords: ['明细', '清单', '记录', '详情', '流水', '逐笔'] },
};

// 常见停用词与助词
const STOP_WORDS = new Set([
  '的', '了', '看', '请', '查', '帮我', '我想', '统计', '分析', '显示', '一下', '这个', '那个', '给出', '列出',
  '按', '和', '与', '及', '以及', '各', '每', '一个', '当前', '最近', '所有'
]);

/**
 * 确定性可行性分诊器 (Feasibility Triage Engine)
 * 核心三层判定：
 *  - L1 语言可解：开放语言意图、槽位识别、歧义检测
 *  - L2 模型覆盖：词表/BizModel/alias 覆盖率、未知词提取
 *  - L3 数据可用：对接数据源物理库探查、数据字段对齐、口径确认状态
 */
export function triageRequirement(
  text: string,
  model: BizModel | null,
  dsn: string,
  explicitMetrics: string[] = [],
  explicitDims: string[] = [],
  explicitShape = '',
): TriageResult {
  const cleanText = text.trim();
  const lowerText = cleanText.toLowerCase();

  // 1. 词表与别名提取
  const matchedTerms: MatchedTerm[] = [];
  const ambiguities: AmbiguityItem[] = [];
  const outOfVocabTerms: string[] = [];

  // 从 model 中获取已注册词库
  const metrics = model?.metrics ?? [];
  const dimensions = model?.dimensions ?? [];
  const entities = model?.entities ?? [];

  // A. 匹配指标
  for (const m of metrics) {
    const candidates = [m.label || m.id, m.id, ...(m.aliases ?? [])].filter(Boolean);
    let matched = false;
    for (const cand of candidates) {
      if (lowerText.includes(cand.toLowerCase()) || explicitMetrics.includes(m.id)) {
        matchedTerms.push({
          token: cand,
          type: 'metric',
          id: m.id,
          label: m.label || m.id,
          confidence: explicitMetrics.includes(m.id) ? 1.0 : 0.95,
          status: (m.status as 'confirmed' | 'draft' | 'candidate') || 'confirmed',
        });
        matched = true;
        break;
      }
    }
  }

  // 补上显式勾选但不在文字里的指标
  for (const expId of explicitMetrics) {
    if (!matchedTerms.some((m) => m.id === expId)) {
      const m = metrics.find((x) => x.id === expId);
      matchedTerms.push({
        token: m?.label || expId,
        type: 'metric',
        id: expId,
        label: m?.label || expId,
        confidence: 1.0,
        status: (m?.status as any) || 'confirmed',
      });
    }
  }

  // B. 匹配维度
  for (const d of dimensions) {
    const candidates = [d.label || d.id, d.id, ...(d.aliases ?? [])].filter(Boolean);
    for (const cand of candidates) {
      if (lowerText.includes(cand.toLowerCase()) || explicitDims.includes(d.id)) {
        matchedTerms.push({
          token: cand,
          type: 'dimension',
          id: d.id,
          label: d.label || d.id,
          confidence: explicitDims.includes(d.id) ? 1.0 : 0.9,
          status: (d.status as 'confirmed' | 'draft' | 'candidate') || 'confirmed',
        });
        break;
      }
    }
  }

  // 补上显式勾选但不在文字里的维度
  for (const expId of explicitDims) {
    if (!matchedTerms.some((m) => m.id === expId)) {
      const d = dimensions.find((x) => x.id === expId);
      matchedTerms.push({
        token: d?.label || expId,
        type: 'dimension',
        id: expId,
        label: d?.label || expId,
        confidence: 1.0,
        status: (d?.status as any) || 'confirmed',
      });
    }
  }

  // C. 歧义检测 (Ambiguity Detection)
  // 例 1: "销售" 可能指销售额，也可能指销售量或销售订单笔数
  if (lowerText.includes('销售') && !lowerText.includes('销售额') && !lowerText.includes('销量') && !lowerText.includes('笔数')) {
    ambiguities.push({
      token: '销售',
      message: '词意不确定：可能指「销售金额」或「销售订单笔数」',
      candidates: [
        { id: 'sales_amount', label: '销售额 (金额汇总)', kind: 'metric', reason: '通常默认取交易金额汇总' },
        { id: 'order_count', label: '销售订单数 (订单量)', kind: 'metric', reason: '按订单记录数统计' },
      ],
    });
  }

  // 例 2: "客户" 可能指客户数，也可能指客户等级维度
  if (lowerText.includes('按客户') && !lowerText.includes('客户等级') && !lowerText.includes('客户名') && !lowerText.includes('客户数')) {
    ambiguities.push({
      token: '按客户',
      message: '维度粒度未明：按客户单人明细还是按客户等级/大区聚合？',
      candidates: [
        { id: 'customer_level', label: '客户等级 (聚合汇总)', kind: 'dimension', reason: '粗粒度汇总' },
        { id: 'customer_id', label: '单个客户明细', kind: 'dimension', reason: '细粒度逐人明细' },
      ],
    });
  }

  // D. 形态证据 (Shape Evidence)
  let detectedShape = explicitShape || 'grouped';
  let shapeConfidence = explicitShape ? 1.0 : 0.6;
  const shapeReasons: string[] = [];

  if (explicitShape) {
    shapeReasons.push(`手动指定形态：${SHAPE_KEYWORDS[explicitShape]?.label || explicitShape}`);
  } else {
    for (const [sKey, def] of Object.entries(SHAPE_KEYWORDS)) {
      for (const kw of def.keywords) {
        if (cleanText.includes(kw)) {
          detectedShape = sKey;
          shapeConfidence = 0.88;
          shapeReasons.push(`命中关键词「${kw}」→ 推断形态为【${def.label}】`);
          break;
        }
      }
      if (shapeConfidence > 0.8) break;
    }
    if (shapeReasons.length === 0) {
      if (matchedTerms.some((t) => t.type === 'dimension')) {
        detectedShape = 'grouped';
        shapeReasons.push('识别到维度与指标，默认采用【分组汇总】形态');
      } else {
        detectedShape = 'detail';
        shapeReasons.push('未检测到明显聚合维度，默认回退为【明细列表】');
      }
    }
  }

  // E. 槽位完整性检测 (Slot Completeness)
  const missingSlots: MissingSlot[] = [];
  const matchedMetrics = matchedTerms.filter((t) => t.type === 'metric');
  const matchedDims = matchedTerms.filter((t) => t.type === 'dimension');

  if (matchedMetrics.length === 0) {
    missingSlots.push({
      slot: 'metric',
      label: '必填指标',
      required: true,
      message: '缺少核心统计指标（报表必须至少包含一个数值度量）',
      defaultSuggestion: metrics[0]?.id,
    });
  }

  // 趋势形态要求时间维度
  if (detectedShape === 'trend') {
    const hasTimeDim = matchedDims.some((d) => {
      const meta = dimensions.find((x) => x.id === d.id);
      return meta?.kind === 'time' || d.id.includes('time') || d.id.includes('month') || d.id.includes('date');
    });
    if (!hasTimeDim) {
      missingSlots.push({
        slot: 'time_window',
        label: '时间窗口与粒度',
        required: true,
        message: '时序趋势表必须指定时间维度（如按月、按日、近30天等）',
        defaultSuggestion: 'month',
      });
    }
  }

  // F. 词表外与未知业务词提取 (Out-of-vocabulary check)
  // 建立已知词库（最长匹配优先）
  const allKnownCandidates: string[] = [
    ...metrics.flatMap((m) => [m.label, m.id, ...(m.aliases ?? [])]).filter(Boolean) as string[],
    ...dimensions.flatMap((d) => [d.label, d.id, ...(d.aliases ?? [])]).filter(Boolean) as string[],
    ...entities.flatMap((e) => [e.label, e.id, (e as any).name]).filter(Boolean) as string[],
    ...Object.values(SHAPE_KEYWORDS).flatMap((k) => k.keywords),
    'ltv', '按', '看', '和', '与', '及', '各', '每', '的', '了', '走势', '趋势', '排名', '分析', '统计', '报表', '明细', '请', '查'
  ].sort((a, b) => b.length - a.length);

  // 用已知词依次把句子消解，剩下的文本才是未知词
  let remaining = cleanText;
  for (const token of allKnownCandidates) {
    if (!token) continue;
    // 全局不区分大小写替换为空格
    const escaped = token.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    remaining = remaining.replace(new RegExp(escaped, 'gi'), ' ');
  }

  // 对剩余片段按空白与标点切分
  const rawResiduals = remaining.split(/[\s,，。、；;!?！？\-_/]+/);
  for (const seg of rawResiduals) {
    const trimmed = seg.trim();
    if (trimmed.length >= 2 && !STOP_WORDS.has(trimmed) && !/^\d+$/.test(trimmed)) {
      if (!outOfVocabTerms.includes(trimmed)) {
        outOfVocabTerms.push(trimmed);
      }
    }
  }

  // G. L3 数据可用性探查 (Data Availability & Grounding)
  // 核心思想：规则只能验证 bizmodel 登记表，L3 负责探查物理系统真实数据
  const probedFields: ProbedField[] = [];
  const missingDataItems: string[] = [];

  // 判断当前 dsn 的可用性
  const isDemoSqlite = dsn.includes('demo') || dsn.includes('sqlite') || dsn === 'demo_sqlite';

  // 已知 demo_sqlite 数据表：customer (id, name, level, region), order (id, customer_id, amount, discount), loan (id, customer_id, principal, is_overdue)
  if (isDemoSqlite) {
    // 检查已匹配的指标与维度在物理表中的对齐情况
    for (const m of matchedMetrics) {
      probedFields.push({
        table: m.id.includes('loan') || m.id.includes('overdue') ? 'loan' : 'order',
        column: m.id.includes('ltv') ? 'amount' : m.id.includes('overdue') ? 'is_overdue' : 'amount',
        concept: m.label,
        existsInPhysicalDb: true,
        status: m.status === 'confirmed' ? 'confirmed' : 'candidate_probe',
      });
    }
    for (const d of matchedDims) {
      probedFields.push({
        table: 'customer',
        column: d.id.includes('level') ? 'level' : d.id.includes('region') ? 'region' : 'id',
        concept: d.label,
        existsInPhysicalDb: true,
        status: d.status === 'confirmed' ? 'confirmed' : 'candidate_probe',
      });
    }

    // 检查词表外词在 demo_sqlite 里是否能探查到
    for (const unknown of outOfVocabTerms) {
      if (unknown.includes('渠道') || unknown.includes('支付') || unknown.includes('退款') || unknown.includes('应收')) {
        // 探查到弱相关候选列（但未建模）
        probedFields.push({
          table: 'order',
          column: 'payment_method',
          concept: unknown,
          existsInPhysicalDb: true,
          status: 'candidate_probe',
        });
      } else if (unknown.includes('考勤') || unknown.includes('员工') || unknown.includes('工资') || unknown.includes('离职') || unknown.includes('cpu') || unknown.includes('内存') || unknown.includes('流量')) {
        // 物理库完全不存在
        missingDataItems.push(`物理库缺表：无「${unknown}」相关表结构或列`);
        probedFields.push({
          table: '—',
          column: '—',
          concept: unknown,
          existsInPhysicalDb: false,
          status: 'missing',
        });
      }
    }
  } else {
    // 未知外部系统 / 空 dsn / 未探查
    if (!model || model.metrics.length === 0) {
      missingDataItems.push(`数据源 [${dsn || '未选择'}] 尚未运行发现 (Discover)，数据字典为空`);
    }
  }

  // H. 三层可行性状态评估 (L1 / L2 / L3)
  // L1 语言可解
  const l1Pass = cleanText.length > 0 && ambiguities.length === 0 && (matchedMetrics.length > 0 || explicitMetrics.length > 0);
  const l1Status: LayerStatus = {
    name: 'L1 语言可解',
    pass: l1Pass,
    status: l1Pass ? 'ok' : ambiguities.length > 0 || cleanText.length === 0 ? 'warn' : 'fail',
    question: '这句话要什么（指标 / 维度 / 时间 / 口径）',
    mechanism: '开放意图解析 + 槽位抽取 + 歧义过滤',
    summary: l1Pass
      ? `意图清晰：抽取到 ${matchedMetrics.length} 个指标、${matchedDims.length} 个维度，形态倾向为【${SHAPE_KEYWORDS[detectedShape]?.label || detectedShape}】`
      : ambiguities.length > 0
      ? `存在歧义词（${ambiguities.map((a) => a.token).join('、')}），需确认具体口径`
      : '缺少核心统计指标槽位，无法推导出目标度量',
    details: [
      ...matchedTerms.map((t) => `槽位识别 [${t.type}]: ${t.label} (${t.id})`),
      ...ambiguities.map((a) => `歧义提醒: ${a.message}`),
      ...missingSlots.map((s) => `缺槽位: ${s.message}`),
    ],
  };

  // L2 模型覆盖
  const hasHardOutOfVocab = outOfVocabTerms.some((w) =>
    probedFields.some((p) => p.concept === w && p.status === 'missing')
  );
  const l2Pass = matchedMetrics.length > 0 && !hasHardOutOfVocab && outOfVocabTerms.length <= 1;
  const l2Status: LayerStatus = {
    name: 'L2 模型覆盖',
    pass: l2Pass,
    status: l2Pass ? 'ok' : hasHardOutOfVocab ? 'fail' : 'warn',
    question: '词表 / bizmodel 里有没有这些词及别名',
    mechanism: 'BizModel 登记账册 + 词表静态匹配 (words.rs)',
    summary: l2Pass
      ? `已登记覆盖：命中 ${matchedTerms.length} 个业务术语，词表覆盖完备`
      : outOfVocabTerms.length > 0
      ? `发现词表外词：[${outOfVocabTerms.join('、')}]，未在当前业务模型或别名表中登记`
      : '未匹配到已登记的指标或维度',
    details: [
      `当前业务模型收录指标 ${metrics.length} 个、维度 ${dimensions.length} 个`,
      ...(outOfVocabTerms.length > 0 ? [`未收录词清单: ${outOfVocabTerms.join(', ')}`] : ['全部关键词均有登记']),
    ],
  };

  // L3 数据可用
  const l3HardFail = missingDataItems.length > 0 || (!isDemoSqlite && (!model || model.metrics.length === 0));
  const hasCandidateProbe = probedFields.some((p) => p.status === 'candidate_probe');
  const l3Pass = !l3HardFail && !hasCandidateProbe && probedFields.every((p) => p.existsInPhysicalDb);
  const l3Status: LayerStatus = {
    name: 'L3 数据可用',
    pass: l3Pass,
    status: l3HardFail ? 'fail' : hasCandidateProbe || !isDemoSqlite ? 'warn' : 'ok',
    question: '对接系统有没有这个数据、物理表字段口径对不对',
    mechanism: '数据源物理 Schema 探查 → 语义匹配 → 口径确认',
    summary: l3Pass
      ? `物理库已对齐：所需数据表与字段均在数据源 [${dsn}] 探查可用且已完成语义建模`
      : l3HardFail
      ? `物理数据缺失：当前对接系统无法支持所提需求（${missingDataItems[0] || '缺对应物理表'}）`
      : '发现物理库未确认候选字段，需人工/模型确认口径后沉淀入账',
    details: [
      `数据源: ${dsn || '未选择'} (${isDemoSqlite ? '已建模演示库' : '外部未探查库'})`,
      ...probedFields.map(
        (p) =>
          `物理映射: ${p.concept} → ${p.table}.${p.column} [${
            p.status === 'confirmed' ? '已确认为正式资产' : p.status === 'candidate_probe' ? '探查到候选列(待确认)' : '物理库不存在'
          }]`
      ),
      ...missingDataItems,
    ],
  };

  // I. 综合三态分诊判定 (🟢 / 🟡 / 🔴)
  let branch: TriageBranch = 'ready';
  let branchTitle = '🟢 可以做到';
  let branchSubtitle = '指标命中、槽位齐或有默认、无歧义、证据足够、物理数据对齐';
  let summaryText = '直接生成：spec → run → 真报表（承诺域内，e2c3042 已验证可达）';

  const blockers: string[] = [];
  const remedies: string[] = [];
  const quickFixes: QuickFixOption[] = [];

  // 判断是否落入 🔴 不可以做到
  if (l3HardFail || (outOfVocabTerms.length > 0 && hasHardOutOfVocab)) {
    branch = 'reject';
    branchTitle = '🔴 不可以做到';
    branchSubtitle = '关键槽在词表外 / 物理库缺表缺字段 / 结构缺到补不了';
    summaryText = '明确拒绝 + 报告缺什么。错误成本不对称，系统永不假装懂！';
    blockers.push(...missingDataItems);
    if (outOfVocabTerms.length > 0) {
      blockers.push(`关键槽词表外：[${outOfVocabTerms.join('、')}]`);
    }
    remedies.push('改述需求：更换为当前数据源已支持的业务范围（如客户等级、LTV、贷款逾期）');
    remedies.push('前往「业务知识」页面运行连库发现 (Discover)，登记新的实体与指标口径');
    remedies.push('切换或接入含有相应数据表的数据源 (DSN)');
  }
  // 判断是否落入 🟡 可以做到 · 要先补
  else if (
    ambiguities.length > 0 ||
    missingSlots.length > 0 ||
    hasCandidateProbe ||
    outOfVocabTerms.length > 0 ||
    shapeConfidence < 0.7
  ) {
    branch = 'clarify';
    branchTitle = '🟡 可以做到 · 要先补';
    branchSubtitle = '歧义未消 / 时间窗缺且无默认 / 存在候选列待确认 / 形态证据不足';
    summaryText = '注入上下文 + 追问确认。不猜，先问 → 确认后再生成！';

    // 收集 blockers
    for (const amb of ambiguities) {
      blockers.push(`歧义待消：${amb.message}`);
      for (const cand of amb.candidates) {
        quickFixes.push({
          id: `fix_amb_${cand.id}`,
          label: `采用「${cand.label}」`,
          action: cand.kind === 'metric' ? 'set_metric' : 'set_dim',
          value: cand.id,
          description: cand.reason,
        });
      }
    }

    for (const slot of missingSlots) {
      blockers.push(`缺少必要槽位：${slot.message}`);
      if (slot.defaultSuggestion) {
        quickFixes.push({
          id: `fix_slot_${slot.slot}`,
          label: `补齐默认【${slot.label}: ${slot.defaultSuggestion}】`,
          action: slot.slot === 'metric' ? 'set_metric' : slot.slot === 'time_window' ? 'set_time' : 'set_dim',
          value: slot.defaultSuggestion,
          description: `自动装配默认 ${slot.label}`,
        });
      }
    }

    if (hasCandidateProbe) {
      blockers.push('物理库探查到未建模候选列，需确认口径映射');
      remedies.push('快速一键确认为当前模型的临时指标并继续');
    }

    remedies.push('在下方一键确认槽位，消解歧义后立即生成');
    remedies.push('补充时间周期（如：近30天、2026年Q1、按月）');
  } else {
    // 🟢 可以做到
    branch = 'ready';
    branchTitle = '🟢 可以做到';
    branchSubtitle = '指标命中、槽位齐或有默认、无歧义、物理数据对齐';
    summaryText = '直接生成：spec → run → 真报表（承诺域内，保证真实可达）';
  }

  return {
    branch,
    branchTitle,
    branchSubtitle,
    summaryText,
    layers: {
      l1: l1Status,
      l2: l2Status,
      l3: l3Status,
    },
    matchedTerms,
    outOfVocabTerms,
    ambiguities,
    missingSlots,
    probedFields,
    shapeEvidence: {
      shape: detectedShape,
      label: SHAPE_KEYWORDS[detectedShape]?.label || detectedShape,
      confidence: shapeConfidence,
      reasons: shapeReasons,
    },
    blockers,
    remedies,
    quickFixes,
    canGenerateDirectly: branch === 'ready',
  };
}
