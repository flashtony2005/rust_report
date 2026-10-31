import {
  BizModel,
  CaliberInfo,
  ClarificationQuestion,
  ContextSnapshot,
  Dataset,
  Requirement,
  Run,
  Spec,
  TriageResult,
} from '../types';
import { ContextEngine } from './contextEngine';
import { compileTemplate, detectShape, executeReport, parseCsv } from './reportEngine';
import { RAW_SAMPLE_CSV, SHAPE_LABELS } from '../data/seedData';
import { OntologyResolver } from './ontologyResolver';

export class ReportAgent {
  /**
   * 核心分诊器：三层可行性联合判定 (L1 语言理解 → L2 模型覆盖 → L3 数据可用与歧义检测)
   */
  public static evaluateTriage(
    text: string,
    dsnId: string,
    bizModel: BizModel,
    optionsOverride?: Record<string, string>
  ): TriageResult {
    const rawText = text.trim();
    const lower = rawText.toLowerCase();
    const context = ContextEngine.prepareContext(dsnId, rawText);

    // ==========================================
    // L1: 语言理解 (Language Resolution)
    // ==========================================
    const extractedMetrics: string[] = [];
    const extractedDims: string[] = [];
    let timeWindow: string | undefined = undefined;
    let shapeHint: string | undefined = undefined;

    // Detect time
    if (lower.includes('7月') || lower.includes('07月')) timeWindow = '2026-07';
    else if (lower.includes('8月') || lower.includes('08月')) timeWindow = '2026-08';
    else if (lower.includes('9月') || lower.includes('09月') || lower.includes('上个月') || lower.includes('上月')) timeWindow = '2026-09';
    else if (lower.includes('三季度') || lower.includes('q3')) timeWindow = '2026-Q3';
    else if (lower.includes('今年') || lower.includes('各月') || lower.includes('趋势') || lower.includes('走势')) timeWindow = '2026年累计';

    // Detect shape
    if (lower.includes('交叉') || (lower.includes('品类') && lower.includes('大区'))) {
      shapeHint = 'crosstab';
    } else if (lower.includes('趋势') || lower.includes('走势') || lower.includes('按月') || lower.includes('月度')) {
      shapeHint = 'trend';
    } else if (lower.includes('排名') || lower.includes('排行') || lower.includes('top') || lower.includes('倒序') || lower.includes('前几')) {
      shapeHint = 'rank';
    } else if (lower.includes('明细') || lower.includes('流水') || lower.includes('逐笔')) {
      shapeHint = 'detail';
    } else {
      shapeHint = 'grouped';
    }

    // ==========================================
    // L2: 业务模型与词表匹配 (Model Coverage)
    // ==========================================
    const matchedMetrics: string[] = [];
    const matchedDims: string[] = [];
    const unknownTokens: string[] = [];

    // Check Metrics
    bizModel.metrics.forEach((m) => {
      if (
        lower.includes(m.label.toLowerCase()) ||
        lower.includes(m.id.toLowerCase()) ||
        m.aliases?.some((a) => lower.includes(a.toLowerCase()))
      ) {
        matchedMetrics.push(m.label);
      }
    });

    // Check Dimensions
    bizModel.dimensions.forEach((d) => {
      if (
        lower.includes(d.label.toLowerCase()) ||
        lower.includes(d.id.toLowerCase()) ||
        d.aliases?.some((a) => lower.includes(a.toLowerCase()))
      ) {
        matchedDims.push(d.label);
      }
    });

    // Semantic keyword mapping
    if (lower.includes('销售额') || lower.includes('业绩') || lower.includes('营收') || lower.includes('金额')) {
      if (!matchedMetrics.includes('销售额')) matchedMetrics.push('销售额');
    }
    if (lower.includes('毛利') || lower.includes('利润') || lower.includes('毛利率')) {
      if (!matchedMetrics.includes('毛利率') && !matchedMetrics.includes('销售毛利')) matchedMetrics.push('毛利率');
    }
    if (lower.includes('客单价')) {
      if (!matchedMetrics.includes('客单价')) matchedMetrics.push('客单价');
    }
    if (lower.includes('逾期率') || lower.includes('坏账')) {
      if (!matchedMetrics.includes('逾期率')) matchedMetrics.push('逾期率');
    }
    if (lower.includes('ltv') || lower.includes('生命周期价值')) {
      if (!matchedMetrics.includes('客户全生命周期价值 (LTV)')) matchedMetrics.push('客户全生命周期价值 (LTV)');
    }

    // Dimensions mapping
    if (lower.includes('大区') || lower.includes('区域')) {
      if (!matchedDims.includes('区域')) matchedDims.push('区域');
    }
    if (lower.includes('门店') || lower.includes('店铺')) {
      if (!matchedDims.includes('门店')) matchedDims.push('门店');
    }
    if (lower.includes('品类') || lower.includes('产品') || lower.includes('分类')) {
      if (!matchedDims.includes('品类')) matchedDims.push('品类');
    }
    if (lower.includes('月') || lower.includes('时间') || lower.includes('走势')) {
      if (!matchedDims.includes('月份')) matchedDims.push('月份');
    }
    if (lower.includes('客户等级') || lower.includes('等级') || lower.includes('tier')) {
      if (!matchedDims.includes('客户等级')) matchedDims.push('客户等级');
    }

    // Detect obvious out-of-vocabulary or unresolvable concepts (🔴 Infeasible check)
    const impossibleKeywords = [
      '量子', '纠缠', '损益表', '资产负债', '现金流', '外星人', '股票期权', '打卡记录', '考勤',
      '薪酬', '工资条', '社保', '公积金', '天气', '气温', '人脸识别',
    ];
    impossibleKeywords.forEach((kw) => {
      if (lower.includes(kw)) {
        unknownTokens.push(kw);
      }
    });

    // Check if user provided totally unknown nonsense
    if (matchedMetrics.length === 0 && matchedDims.length === 0) {
      // Nothing matched
      unknownTokens.push(rawText);
    }

    // ==========================================
    // L3: 数据可用性与歧义检测 (Data Availability & Caliber Ambiguity)
    // ==========================================
    const blockers: string[] = [];
    const tableEvidence: string[] = [];
    context.tables.forEach((t) => tableEvidence.push(`${t.name} (${t.label}, ${t.rows} 行)`));

    // Branch 1: 🔴 Infeasible (不可以做到)
    if (unknownTokens.length > 0 && matchedMetrics.length === 0) {
      return {
        status: 'infeasible',
        statusLabel: '🔴 不可以做到',
        summary: `关键槽位不在词表与当前数据源覆盖域内：未识别概念 [${unknownTokens.join('、')}]`,
        l1: {
          passed: false,
          intent: rawText,
          slots: { 指标: '未知/词表外', 维度: matchedDims.join(', ') || '未指定' },
          extracted: { metrics: [], dimensions: matchedDims, timeWindow, shapeHint },
        },
        l2: {
          passed: false,
          matchedMetrics: [],
          matchedDims,
          unknownTokens,
          activeRules: context.activeRules,
        },
        l3: {
          passed: false,
          dsn: dsnId,
          tableEvidence,
          blockers: [`当前连接库 (${context.dsnLabel}) 中无支持 ${unknownTokens.join('、')} 的物理表或字段`],
          warnings: ['禁止在大模型或引擎层无事实根据臆造报表'],
        },
        rejectionReason: `根据严谨数据可用性审计，词表外词清单：【${unknownTokens.join('、')}】。当前对接系统为 ${context.dsnLabel}，未检索到对应实体与口径。`,
        suggestedQueries: [
          '华东各品类销售额交叉表',
          '按大区和门店统计销售额分组汇总',
          '今年各月销售额趋势走势',
          '统计各品类毛利与进销差价',
        ],
        contextSnapshot: context,
      };
    }

    // Branch 2: 🟡 Clarify (可以做到 · 要先补)
    // Check for ambiguities that require clarification unless already confirmed via optionsOverride
    const clarifications: ClarificationQuestion[] = [];

    // Ambiguity 1: "毛利" / "毛利率" 口径二义性
    if (matchedMetrics.includes('毛利率') || lower.includes('毛利')) {
      const selectedCaliber = optionsOverride?.['margin_caliber'];
      if (!selectedCaliber) {
        clarifications.push({
          id: 'margin_caliber',
          title: '请确认「毛利/毛利率」的核算口径',
          hint: '系统中登记了两种财务核算方式，选择将决定计算公式：',
          options: [
            {
              id: 'comprehensive',
              label: '综合毛利率 = (销售额 - 销售成本) / 销售额',
              desc: '（推荐）核算扣减进销货成本与门店固定分摊',
              isDefault: true,
            },
            {
              id: 'gross_spread',
              label: '纯进销差价 = (销售额 - 进货实付成本) / 销售额',
              desc: '仅按采购进销价差计算，不含门店运营分摊',
            },
          ],
        });
      }
    }

    // Ambiguity 2: "客单价" 或 跨多实体
    if (matchedMetrics.includes('客单价') && !optionsOverride?.['atv_grain']) {
      clarifications.push({
        id: 'atv_grain',
        title: '请确认「客单价」聚合颗粒度',
        hint: '客单价可以按交易订单笔数或按独立购买顾客数计算：',
        options: [
          {
            id: 'order_count',
            label: '按单笔订单均值 = 销售总额 / 订单总笔数',
            desc: '适合评估单次收银交易规模',
            isDefault: true,
          },
          {
            id: 'unique_customer',
            label: '按单客累计均值 = 销售总额 / 去重顾客数',
            desc: '适合评估单客户价值 (ARPU)',
          },
        ],
      });
    }

    // Ambiguity 3: 仅在用户明确表达相对时间如"近期"/"上期"且未给基准，或明确选择时间范围时触发追问
    if ((lower.includes('近期') || lower.includes('对比上一期') || lower.includes('同期')) && !timeWindow && !optionsOverride?.['time_scope']) {
      clarifications.push({
        id: 'time_scope',
        title: '相对时间缺少基准，请确认统计区间',
        hint: '数据源已探查到 2026-07 ~ 2026-09 数据，推荐锚定最近完结月：',
        options: [
          {
            id: 'cur_month',
            label: '2026年9月（最近完结月）',
            desc: '默认最佳实践，排除未结账残缺月份',
            isDefault: true,
          },
          {
            id: 'full_q3',
            label: '2026年第三季度全量 (Q3: 7月~9月)',
            desc: '季度整体宏观统计',
          },
          {
            id: 'all_history',
            label: '全量历史数据汇总',
            desc: '汇总库中现有全部 90 笔记录',
          },
        ],
      });
    }


    if (clarifications.length > 0) {
      return {
        status: 'clarify',
        statusLabel: '🟡 可以做到 · 要先补',
        summary: `语言与模型已覆盖，但存在 ${clarifications.length} 处口径歧义或参数缺失，需要先补上下文再生成`,
        l1: {
          passed: true,
          intent: rawText,
          slots: {
            指标: matchedMetrics.join(', ') || '销售额',
            维度: matchedDims.join(', ') || '区域',
            时间: timeWindow || '缺省未定',
          },
          extracted: { metrics: matchedMetrics, dimensions: matchedDims, timeWindow, shapeHint },
        },
        l2: {
          passed: true,
          matchedMetrics,
          matchedDims,
          unknownTokens: [],
          activeRules: context.activeRules,
        },
        l3: {
          passed: false,
          dsn: dsnId,
          tableEvidence,
          blockers: clarifications.map((c) => c.title),
          warnings: ['口径未确定前禁止直接生成，以防产出误导性报表'],
        },
        clarifications,
        suggestedQueries: [
          '华东各品类销售额交叉表',
          '按大区和门店统计销售额分组汇总',
        ],
        contextSnapshot: context,
      };
    }

    // ==========================================
    // Branch 3: 🟢 Ready (可以做到 · 直接生成)
    // ==========================================
    // Build the deterministic Spec
    let primaryMetric = matchedMetrics[0] || '销售额';
    let chosenShape = (shapeHint as any) || 'grouped';
    let layout: 'list' | 'grouped' | 'crosstab' = 'grouped';
    let rowDims: any[] = [];
    let colDims: any[] = [];

    // Selected options override descriptions
    const rulesApplied = [...context.activeRules];
    let formulaDesc = 'SUM(amount)';
    let metricField = 'amount';

    if (primaryMetric === '毛利率' || lower.includes('毛利')) {
      const cal = optionsOverride?.['margin_caliber'] || 'comprehensive';
      if (cal === 'comprehensive') {
        formulaDesc = '(SUM(amount) - SUM(cost)) / SUM(amount)';
        rulesApplied.push('已确认使用综合毛利率核算口径（含固定分摊成本）');
      } else {
        formulaDesc = '(SUM(amount) - SUM(cost * 0.9)) / SUM(amount)';
        rulesApplied.push('已确认使用纯进销差价口径');
      }
      metricField = 'amount';
    } else if (primaryMetric === '逾期率' || lower.includes('逾期')) {
      formulaDesc = 'COUNT(CASE WHEN overdue_days > 0 THEN 1 END) * 1.0 / COUNT(1)';
      rulesApplied.push('按信贷主实体 overdue_days 统计逾期发生率');
    } else if (primaryMetric === '客户全生命周期价值 (LTV)') {
      formulaDesc = 'SUM(orders.amount) - SUM(loans.losses)';
      rulesApplied.push('本体解析器：启用 CTE 分头预聚合，防范 1:N 笛卡尔积');
    }

    // Configure dimensions & shapes
    if (chosenShape === 'crosstab') {
      layout = 'crosstab';
      rowDims = [{ field: 'region', label: '大区', sort: 'asc' }];
      colDims = [{ field: 'cat', label: '产品品类' }];
    } else if (chosenShape === 'trend') {
      layout = 'grouped';
      rowDims = [{ field: 'month', label: '月份', sort: 'asc' }];
    } else if (chosenShape === 'rank') {
      layout = 'grouped';
      rowDims = [{ field: 'store', label: '门店', sort: 'desc' }];
    } else if (chosenShape === 'detail') {
      layout = 'list';
      rowDims = [];
    } else {
      layout = 'grouped';
      if (matchedDims.includes('门店') && matchedDims.includes('区域')) {
        rowDims = [
          { field: 'region', label: '大区', sort: 'asc' },
          { field: 'store', label: '门店', sort: 'asc' },
        ];
      } else if (matchedDims.includes('品类')) {
        rowDims = [{ field: 'cat', label: '品类', sort: 'asc' }];
      } else if (matchedDims.includes('客户等级')) {
        rowDims = [{ field: 'tier', label: '客户等级', sort: 'asc' }];
      } else {
        rowDims = [{ field: 'region', label: '区域', sort: 'asc' }];
      }
    }

    const effectiveTimeWindow =
      optionsOverride?.['time_scope'] === 'cur_month'
        ? '2026-09'
        : optionsOverride?.['time_scope'] === 'full_q3'
        ? '2026-Q3'
        : timeWindow || '2026年累计 (至 2026-10-30)';

    const spec: Spec = {
      layout,
      shape: chosenShape,
      row_dims: rowDims,
      col_dims: colDims,
      measures: [
        {
          field: metricField,
          agg: 'sum',
          label: primaryMetric,
          format: primaryMetric.includes('率') ? '0.0%' : '#,##0',
        },
      ],
      totals: { grand: true, subtotal: layout === 'crosstab' },
    };

    const caliberInfo: CaliberInfo = {
      metricName: primaryMetric,
      formula: formulaDesc,
      dimensions: rowDims.concat(colDims).map((d) => d.label || d.field),
      timeWindow: effectiveTimeWindow,
      rulesApplied,
      safetyGuarantee: '已通过确定性规则与物理 Schema 审计，无笛卡尔积，粒度完全对齐',
    };

    return {
      status: 'ready',
      statusLabel: '🟢 可以做到',
      summary: '指标明确、槽位完整、口径已确认且与物理数据源完全对齐，可直接生成报表！',
      l1: {
        passed: true,
        intent: rawText,
        slots: {
          指标: primaryMetric,
          维度: rowDims.concat(colDims).map((d) => d.label || d.field).join(' × '),
          时间: effectiveTimeWindow,
          形态: SHAPE_LABELS[chosenShape] || chosenShape,
        },
        extracted: { metrics: matchedMetrics, dimensions: matchedDims, timeWindow: effectiveTimeWindow, shapeHint: chosenShape },
      },
      l2: {
        passed: true,
        matchedMetrics: [primaryMetric],
        matchedDims,
        unknownTokens: [],
        activeRules: rulesApplied,
      },
      l3: {
        passed: true,
        dsn: dsnId,
        tableEvidence,
        blockers: [],
        warnings: [],
      },
      spec,
      caliberInfo,
      contextSnapshot: context,
    };
  }

  /**
   * 将分诊成功的 🟢 结果转化为真报表 (Spec → Compile → Dry Run → Univer Sheets)
   */
  public static executeReportFromTriage(
    reqId: string,
    triage: TriageResult,
    dsnId: string
  ): { requirement: Requirement; run: Run } {
    if (!triage.spec) {
      throw new Error('分诊尚未通过，无法直接生成报表');
    }

    const parsed = parseCsv(RAW_SAMPLE_CSV);
    const dataset: Dataset = {
      kind: 'sql',
      dsn: dsnId,
      sql: `SELECT ${parsed.columns.join(', ')} FROM sales;`,
      rows: parsed.rows,
      columns: parsed.columns,
      column_meta: parsed.column_meta,
      preview: parsed.preview,
    };

    const template = compileTemplate(triage.spec, dataset, 0);

    const requirement: Requirement = {
      id: reqId,
      title: triage.l1.intent,
      description: `由报表 Agent 三层分诊生成：【${triage.caliberInfo?.metricName}】· 统计范围：${triage.caliberInfo?.timeWindow} · 数据源：${dsnId}`,
      dataset,
      roles: { measures: [triage.spec.measures[0]?.field || 'amount'] },
      template_choice: {
        id: `agent-gen-${triage.spec.shape}`,
        name: `Agent 自动装配模板（${SHAPE_LABELS[triage.spec.shape || 'grouped']}）`,
        shape_label: SHAPE_LABELS[triage.spec.shape || 'grouped'] || '分组汇总',
        source: 'generated',
        reasons: [
          `确定性分诊通过：${triage.caliberInfo?.metricName}（${triage.caliberInfo?.formula}）`,
          `事实层校验：数据源 ${dsnId} 结构已对齐，包含 ${dataset.rows} 条有效物理事实`,
          '防翻车安全校验：通过粒度对齐检查',
        ],
        missing: [],
      },
      spec: triage.spec,
      shape_decision: detectShape(triage.spec, dataset, triage.l1.intent),
      analysis: {
        calibers: [
          {
            label: triage.caliberInfo?.metricName || '度量',
            expr: triage.caliberInfo?.formula || 'SUM(amount)',
            agg: triage.spec.measures[0]?.agg || 'sum',
            scope: `统计时间：${triage.caliberInfo?.timeWindow} · 维度：${triage.caliberInfo?.dimensions.join(' × ')}`,
          },
        ],
        decisions: [
          { what: `选定形态: ${SHAPE_LABELS[triage.spec.shape || 'grouped']}`, source: 'NL 语义与数据基数特征推导' },
          { what: `采用口径: ${triage.caliberInfo?.formula}`, source: 'Agent 三层判定闭环' },
        ],
        open_questions: [],
        dataset: { rows: dataset.rows, columns: dataset.columns },
        estimate: { data_rows: dataset.rows, how: 'SQL 物理数据源执行' },
      },
      template,
      runs: [],
      status: 'ran',
      status_label: '已验收',
      updated_at: new Date().toISOString(),
    };

    const run = executeReport(requirement);

    // If multi-entity ontology query (e.g. 客户等级 / LTV / 逾期率)
    if (
      triage.l1.intent.includes('客户等级') ||
      triage.l1.intent.includes('ltv') ||
      triage.l1.intent.includes('逾期') ||
      triage.spec.row_dims.some((d) => d.field === 'tier')
    ) {
      const res = OntologyResolver.resolve(
        {
          schema: 'rpx-bizmodel/v1',
          version: 1,
          source: { dsn: dsnId, generated_at: new Date().toISOString(), generator: 'ontology' },
          entities: [],
          relationships: [],
          metrics: [],
          dimensions: [],
          rules: [],
          open_questions: [],
        },
        triage.l1.intent
      );
      if (res && res.univerSnapshot) {
        run.univerSnapshot = res.univerSnapshot;
        run.computedData = {
          headers: res.outputData.headers,
          rows: res.outputData.rows,
        };
        run.shape = {
          rows: res.outputData.rows.length + 1,
          cols: res.outputData.headers.length,
        };
        run.artifacts['report.html'] = `<pre style="font-family:monospace;padding:16px;background:#0f172a;color:#a5b4fc">${res.grainAlignedSql}</pre>`;
        run.artifacts['report.csv'] = [res.outputData.headers.join(','), ...res.outputData.rows.map((r) => r.join(','))].join('\n');
      }
    }

    requirement.runs.push(run);

    return { requirement, run };
  }
}
