import {
  Requirement,
  Datasource,
  Run,
  Schedule,
  Alert,
  SemanticVersion,
  LlmConfig,
  Candidate,
  Spec,
  TemplateCell,
  AnalysisDoc,
  BizModel,
  BizMetric,
  BizDimension,
  BizRule,
  GoldenQuestion,
} from '../types';
import {
  INITIAL_DATASOURCES,
  INITIAL_SCHEDULES,
  INITIAL_ALERTS,
  INITIAL_VERSIONS,
  RAW_SAMPLE_CSV,
  SHAPE_LABELS,
  INITIAL_BIZMODELS,
  INITIAL_GOLDENS,
} from '../data/seedData';
import {
  createSampleDataset,
  compileTemplate,
  executeReport,
  detectShape,
  recommendCandidates,
  parseCsv,
  runAttributionAnalysis,
} from './reportEngine';

type Listener = () => void;

class ReportStudioStore {
  private requirements: Requirement[] = [];
  private datasources: Datasource[] = [];
  private runs: Run[] = [];
  private schedules: Schedule[] = [];
  private alerts: Alert[] = [];
  private versions: Record<string, SemanticVersion[]> = {};
  private bizmodels: Record<string, BizModel> = {};
  private goldens: GoldenQuestion[] = [];
  private llmConfig: LlmConfig = {
    provider: 'deepseek',
    base_url: 'https://api.deepseek.com',
    model: 'deepseek-v4-flash',
    has_key: false,
  };
  private listeners: Set<Listener> = new Set();

  constructor() {
    this.init();
  }

  private init() {
    this.datasources = [...INITIAL_DATASOURCES];
    this.schedules = [...INITIAL_SCHEDULES];
    this.alerts = [...INITIAL_ALERTS];
    this.versions = { ...INITIAL_VERSIONS };
    this.bizmodels = JSON.parse(JSON.stringify(INITIAL_BIZMODELS));
    this.goldens = JSON.parse(JSON.stringify(INITIAL_GOLDENS));

    // Create 3 realistic initial requirements
    const sampleDs = createSampleDataset();

    // Requirement 1: Crosstab
    const spec1: Spec = {
      layout: 'crosstab',
      shape: 'crosstab',
      row_dims: [{ field: 'region', label: '大区', sort: 'asc' }],
      col_dims: [{ field: 'cat', label: '产品品类' }],
      measures: [{ field: 'amount', agg: 'sum', label: '销售额', format: '#,##0' }],
      totals: { grand: true, subtotal: true },
    };
    const tpl1 = compileTemplate(spec1, sampleDs, 1);
    const req1: Requirement = {
      id: 'req-001',
      title: '华东与各区域品类交叉销售报表',
      description: '按大区和产品类别交叉统计销售额，带行末小计与总计行',
      dataset: sampleDs,
      roles: { measures: ['amount'] },
      template_choice: {
        id: 'sales-region-category-crosstab',
        name: '区域品类交叉表',
        shape_label: '交叉表',
        source: 'library',
        score: 95,
        reasons: ['列名完全重合（region、cat、amount）', '业务形态契合度最高'],
        missing: [],
        fields: ['region', 'cat', 'amount'],
      },
      spec: spec1,
      shape_decision: detectShape(spec1, sampleDs),
      analysis: {
        calibers: [
          { label: '销售额', expr: 'amount', agg: 'sum', scope: '按大区 × 品类聚合' },
          { label: '行小计', expr: 'sum(C{})', scope: '品类行维度汇总' },
        ],
        decisions: [
          { what: '选定交叉表版式', source: '双维度（region、cat）加一个度量 amount' },
          { what: '启用行末小计与总计', source: '用户需求明确提及带合计行' },
        ],
        open_questions: [],
        dataset: { rows: sampleDs.rows, columns: sampleDs.columns },
        estimate: { data_rows: sampleDs.rows, how: '按大区分组展开' },
        fields: {
          'row_dims[0].region': { distinct: 3, nulls: 0 },
          'col_dims[0].cat': { distinct: 3, nulls: 0 },
          'measures[0].amount': { numeric: sampleDs.rows, nulls: 0 },
        },
      },
      template: tpl1,
      runs: [],
      status: 'ran',
      status_label: '已验收',
      updated_at: '2026-10-25T14:20:00Z',
    };
    const run1 = executeReport(req1);
    run1.run_id = 'run-1792881021';
    req1.runs.push(run1);

    // Requirement 2: Grouped summary
    const spec2: Spec = {
      layout: 'grouped',
      shape: 'grouped',
      row_dims: [
        { field: 'region', label: '大区', sort: 'asc' },
        { field: 'store', label: '门店', sort: 'asc' },
      ],
      col_dims: [],
      measures: [{ field: 'amount', agg: 'sum', label: '销售额', format: '#,##0' }],
      totals: { grand: true, subtotal: false },
    };
    const tpl2 = compileTemplate(spec2, sampleDs, 1);
    const req2: Requirement = {
      id: 'req-002',
      title: '门店销售业绩分组汇总清单',
      description: '按大区、门店两级汇总销售额，并排序',
      dataset: sampleDs,
      roles: { measures: ['amount'] },
      template_choice: {
        id: 'store-performance-summary',
        name: '门店业绩分组汇总表',
        shape_label: '分组汇总',
        source: 'library',
        score: 90,
        reasons: ['包含门店与区域两级维度', '包含金额度量汇总'],
        missing: [],
        fields: ['region', 'store', 'amount'],
      },
      spec: spec2,
      shape_decision: detectShape(spec2, sampleDs),
      analysis: {
        calibers: [{ label: '销售额', expr: 'amount', agg: 'sum', scope: '按门店汇总' }],
        decisions: [{ what: '分组汇总', source: '双行维度 region -> store' }],
        open_questions: ['是否需要对门店销售额进行降序排序？'],
        dataset: { rows: sampleDs.rows, columns: sampleDs.columns },
        estimate: { data_rows: 7, how: '按门店去重聚合' },
        fields: {
          'row_dims[0].region': { distinct: 3, nulls: 0 },
          'row_dims[1].store': { distinct: 7, nulls: 0 },
        },
      },
      template: tpl2,
      runs: [],
      status: 'ran',
      status_label: '已出稿',
      updated_at: '2026-10-24T11:45:00Z',
    };
    const run2 = executeReport(req2);
    run2.run_id = 'run-1792849920';
    req2.runs.push(run2);

    // Requirement 3: Monthly Trend
    const spec3: Spec = {
      layout: 'grouped',
      shape: 'trend',
      row_dims: [{ field: 'month', label: '月份', sort: 'asc' }],
      col_dims: [],
      measures: [{ field: 'amount', agg: 'sum', label: '月销售总额', format: '#,##0' }],
      totals: { grand: true, subtotal: false },
    };
    const tpl3 = compileTemplate(spec3, sampleDs, 1);
    const req3: Requirement = {
      id: 'req-003',
      title: '2026 年第三季度各月走势',
      description: '统计 7、8、9 三个月全店销售走势',
      dataset: sampleDs,
      roles: { measures: ['amount'] },
      template_choice: {
        id: 'sales-monthly-trend',
        name: '月度销售走势分析表',
        shape_label: '趋势表',
        source: 'library',
        score: 85,
        reasons: ['时间维 month 自动聚合到时间序列'],
        missing: [],
        fields: ['month', 'amount'],
      },
      spec: spec3,
      shape_decision: detectShape(spec3, sampleDs),
      template: tpl3,
      runs: [],
      status: 'compiled',
      status_label: '已出稿',
      updated_at: '2026-10-23T16:10:00Z',
    };

    this.requirements = [req1, req2, req3];
    this.runs = [run1, run2];
  }

  public subscribe(fn: Listener): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  private notify() {
    this.listeners.forEach((fn) => fn());
  }

  // --- Getters ---
  public getRequirements(): Requirement[] {
    return [...this.requirements];
  }

  public getRequirement(id: string): Requirement | undefined {
    return this.requirements.find((r) => r.id === id);
  }

  public getDatasources(): Datasource[] {
    return [...this.datasources];
  }

  public getRuns(reqId?: string): Run[] {
    if (reqId) {
      return this.runs.filter((r) => r.req_id === reqId);
    }
    return [...this.runs];
  }

  public getSchedules(reqId?: string): Schedule[] {
    if (reqId) {
      return this.schedules.filter((s) => s.requirement === reqId);
    }
    return [...this.schedules];
  }

  public getAlerts(): Alert[] {
    return [...this.alerts];
  }

  public getVersions(reqId: string): SemanticVersion[] {
    return this.versions[reqId] || [];
  }

  public getLlmConfig(): LlmConfig {
    return { ...this.llmConfig };
  }

  public setLlmConfig(cfg: Partial<LlmConfig>) {
    this.llmConfig = {
      ...this.llmConfig,
      ...cfg,
      has_key: !!(cfg.api_key && cfg.api_key.trim().length > 0) || this.llmConfig.has_key,
    };
    this.notify();
  }

  // --- Requirements Actions ---
  public createRequirement(title: string, description: string = '', datasetOverride?: any): Requirement {
    const id = `req-${Date.now().toString(36).slice(-4)}`;
    let dataset: any;

    if (datasetOverride && datasetOverride.kind === 'sql') {
      const parsed = parseCsv(RAW_SAMPLE_CSV);
      dataset = {
        kind: 'sql',
        dsn: datasetOverride.dsn,
        sql: datasetOverride.sql,
        rows: parsed.rows,
        columns: parsed.columns,
        column_meta: parsed.column_meta,
        preview: parsed.preview,
      };
    } else if (datasetOverride && datasetOverride.token) {
      dataset = datasetOverride;
    } else {
      dataset = createSampleDataset();
    }

    const defaultSpec: Spec = {
      layout: 'grouped',
      shape: 'grouped',
      row_dims: [{ field: dataset.columns[0] || 'region', label: dataset.columns[0] || '区域', sort: 'asc' }],
      col_dims: [],
      measures: [{ field: 'amount', agg: 'sum', label: '销售额', format: '#,##0' }],
      totals: { grand: true, subtotal: false },
    };

    const newReq: Requirement = {
      id,
      title: title || '新报表需求',
      description,
      dataset,
      roles: { measures: ['amount'] },
      spec: defaultSpec,
      shape_decision: detectShape(defaultSpec, dataset, title + ' ' + description),
      runs: [],
      status: 'draft',
      status_label: '草稿',
      updated_at: new Date().toISOString(),
    };

    this.requirements.unshift(newReq);
    this.notify();
    return newReq;
  }

  public updateRequirement(id: string, updates: Partial<Requirement>): Requirement {
    const idx = this.requirements.findIndex((r) => r.id === id);
    if (idx === -1) throw new Error(`需求不存在: ${id}`);

    const updated = {
      ...this.requirements[idx],
      ...updates,
      updated_at: new Date().toISOString(),
    };
    this.requirements[idx] = updated;
    this.notify();
    return updated;
  }

  public setRequirementDataset(id: string, dataset: any): Requirement {
    const req = this.getRequirement(id);
    if (!req) throw new Error('需求不存在');
    req.dataset = dataset;
    req.template_choice = undefined;
    req.template = undefined;
    req.status = 'draft';
    req.status_label = '草稿';
    req.updated_at = new Date().toISOString();
    this.notify();
    return req;
  }

  public chooseTemplate(reqId: string, choice: Candidate): Requirement {
    const req = this.getRequirement(reqId);
    if (!req) throw new Error('需求不存在');
    req.template_choice = choice;

    if (choice.source === 'library') {
      if (choice.id.includes('crosstab')) {
        req.spec = {
          layout: 'crosstab',
          shape: 'crosstab',
          row_dims: [{ field: 'region', label: '区域', sort: 'asc' }],
          col_dims: [{ field: 'cat', label: '品类' }],
          measures: [{ field: 'amount', agg: 'sum', label: '销售额', format: '#,##0' }],
          totals: { grand: true, subtotal: true },
        };
      } else if (choice.id.includes('trend')) {
        req.spec = {
          layout: 'grouped',
          shape: 'trend',
          row_dims: [{ field: 'month', label: '月份', sort: 'asc' }],
          col_dims: [],
          measures: [{ field: 'amount', agg: 'sum', label: '月销售总额', format: '#,##0' }],
          totals: { grand: true, subtotal: false },
        };
      } else if (choice.id.includes('rank')) {
        req.spec = {
          layout: 'grouped',
          shape: 'rank',
          row_dims: [{ field: 'store', label: '门店', sort: 'desc' }],
          col_dims: [],
          measures: [{ field: 'amount', agg: 'sum', label: '销售额', format: '#,##0' }],
          totals: { grand: false, subtotal: false },
        };
      }
    }
    const fallbackSpec: Spec = {
      layout: 'grouped',
      shape: 'grouped',
      row_dims: [{ field: 'region', label: '区域', sort: 'asc' }],
      col_dims: [],
      measures: [{ field: 'amount', agg: 'sum', label: '销售额', format: '#,##0' }],
      totals: { grand: true, subtotal: false },
    };
    req.shape_decision = detectShape(req.spec || fallbackSpec, req.dataset, choice.name);
    req.status = 'draft';
    req.updated_at = new Date().toISOString();
    this.notify();
    return req;
  }

  public setShape(reqId: string, shape: any): Requirement {
    const req = this.getRequirement(reqId);
    if (!req) throw new Error('需求不存在');

    const spec: Spec = req.spec || {
      layout: 'grouped',
      shape: 'grouped',
      row_dims: [{ field: 'region', label: '区域', sort: 'asc' }],
      col_dims: [],
      measures: [{ field: 'amount', agg: 'sum', label: '销售额' }],
      totals: { grand: true, subtotal: false },
    };

    spec.shape = shape;
    if (shape === 'crosstab') {
      spec.layout = 'crosstab';
      if (!spec.col_dims || spec.col_dims.length === 0) {
        spec.col_dims = [{ field: 'cat', label: '品类' }];
      }
      spec.totals.subtotal = true;
    } else if (shape === 'detail') {
      spec.layout = 'list';
      spec.totals.grand = false;
    } else {
      spec.layout = 'grouped';
      if (shape === 'trend' && !spec.row_dims.some((d) => d.field === 'month')) {
        spec.row_dims = [{ field: 'month', label: '月份', sort: 'asc' }];
      }
    }

    req.spec = spec;
    req.shape_decision = detectShape(spec, req.dataset);
    req.analysis_stale = true;
    req.updated_at = new Date().toISOString();
    this.notify();
    return req;
  }

  public analyzeRequirement(reqId: string): Requirement {
    const req = this.getRequirement(reqId);
    if (!req) throw new Error('需求不存在');

    const spec: Spec = req.spec || {
      layout: 'grouped',
      shape: 'grouped',
      row_dims: [],
      col_dims: [],
      measures: [],
      totals: { grand: true, subtotal: false },
    };
    const rowDims = (spec.row_dims || []).map((d: any) => d.label || d.field).join('、');
    const msrs = (spec.measures || []).map((m: any) => `${m.label || m.field} (${m.agg})`).join('、');

    req.analysis = {
      calibers: [
        { label: '度量计算', expr: msrs || 'SUM(amount)', scope: `按维度 [${rowDims || '无'}] 聚合` },
        { label: '合计规则', expr: spec.totals?.grand ? 'SUM 全域汇总' : '不计总计', scope: '总计行' },
      ],
      decisions: [
        { what: `采用 ${req.shape_decision?.label || '分组'} 版式`, source: '根据字段角色与形状配置' },
        { what: '自动生成模板网格', source: '编译器根据维度排布推导表头与单元格' },
      ],
      open_questions: req.shape_decision?.blockers?.length
        ? req.shape_decision.blockers.map((b: string) => ({ question: b }))
        : [],
      dataset: { rows: req.dataset.rows, columns: req.dataset.columns },
      estimate: { data_rows: req.dataset.rows, how: '经由 rpx-engine 内存执行' },
      fields: {
        'row_dims[0]': { distinct: 3, nulls: 0 },
        'measures[0]': { numeric: req.dataset.rows, nulls: 0 },
      },
    };
    req.analysis_stale = false;
    req.status = 'analyzed';
    req.status_label = '口径待确认';
    req.updated_at = new Date().toISOString();
    this.notify();
    return req;
  }

  public compileRequirement(reqId: string): Requirement {
    const req = this.getRequirement(reqId);
    if (!req) throw new Error('需求不存在');

    const spec: Spec = req.spec || {
      layout: 'grouped',
      shape: 'grouped',
      row_dims: [{ field: 'region', label: '区域', sort: 'asc' }],
      col_dims: [],
      measures: [{ field: 'amount', agg: 'sum', label: '销售额' }],
      totals: { grand: true, subtotal: false },
    };
    const rev = req.template?.rev || 0;
    req.template = compileTemplate(spec, req.dataset, rev);
    req.status = 'compiled';
    req.status_label = '已出稿';
    req.updated_at = new Date().toISOString();
    this.notify();
    return req;
  }

  public runRequirement(reqId: string): { requirement: Requirement; run: Run } {
    const req = this.getRequirement(reqId);
    if (!req) throw new Error('需求不存在');

    const spec: Spec = req.spec || {
      layout: 'grouped',
      shape: 'grouped',
      row_dims: [{ field: 'region', label: '区域', sort: 'asc' }],
      col_dims: [],
      measures: [{ field: 'amount', agg: 'sum', label: '销售额' }],
      totals: { grand: true, subtotal: false },
    };
    if (!req.template) {
      req.template = compileTemplate(spec, req.dataset, 0);
    }

    const run = executeReport(req);
    req.runs.unshift(run);
    this.runs.unshift(run);
    req.status = 'ran';
    req.status_label = '待验收';
    req.updated_at = new Date().toISOString();
    this.notify();
    return { requirement: req, run };
  }

  public chatRequirement(reqId: string, text: string): Requirement {
    const req = this.getRequirement(reqId);
    if (!req) throw new Error('需求不存在');

    if (!req.messages) req.messages = [];
    req.messages.push({ role: 'user', text });

    const spec = req.spec || {
      layout: 'grouped',
      shape: 'grouped',
      row_dims: [],
      col_dims: [],
      measures: [],
      totals: { grand: true, subtotal: false },
    };

    let reply = '已根据您的描述调整报表设置：';
    const lower = text.toLowerCase();

    if (lower.includes('升序')) {
      if (spec.row_dims.length) spec.row_dims[0].sort = 'asc';
      reply += '第一维度排序改为升序；';
    } else if (lower.includes('降序')) {
      if (spec.row_dims.length) spec.row_dims[0].sort = 'desc';
      reply += '第一维度排序改为降序；';
    }

    if (lower.includes('去掉合计') || lower.includes('取消总计') || lower.includes('无合计')) {
      spec.totals.grand = false;
      reply += '已关闭总计行；';
    } else if (lower.includes('加总计') || lower.includes('显示合计')) {
      spec.totals.grand = true;
      reply += '已开启总计行；';
    }

    if (lower.includes('交叉表') || lower.includes('交叉')) {
      spec.layout = 'crosstab';
      spec.shape = 'crosstab';
      if (!spec.col_dims.length) spec.col_dims = [{ field: 'cat', label: '品类' }];
      spec.totals.subtotal = true;
      reply += '已切换为交叉表版式；';
    } else if (lower.includes('明细')) {
      spec.layout = 'list';
      spec.shape = 'detail';
      reply += '已切换为明细清单；';
    }

    req.messages.push({ role: 'agent', text: reply });
    req.spec = spec;
    req.shape_decision = detectShape(spec, req.dataset);
    req.analysis_stale = true;
    req.updated_at = new Date().toISOString();
    this.notify();
    return req;
  }

  public patchTemplateCells(reqId: string, cells: Partial<TemplateCell>[]): Requirement {
    const req = this.getRequirement(reqId);
    if (!req || !req.template) throw new Error('未出稿模板');

    cells.forEach((patch) => {
      const idx = req.template!.cells.findIndex((c) => c.row === patch.row && c.col === patch.col);
      if (idx !== -1) {
        req.template!.cells[idx] = {
          ...req.template!.cells[idx],
          ...patch,
        };
      }
    });

    req.template.rev += 1;
    req.updated_at = new Date().toISOString();
    this.notify();
    return req;
  }

  public publishVersion(reqId: string, note: string = ''): SemanticVersion {
    const req = this.getRequirement(reqId);
    if (!req) throw new Error('需求不存在');

    const curVersions = this.versions[reqId] || [];
    const newVersionNum = curVersions.length + 1;
    const newVer: SemanticVersion = {
      version: newVersionNum,
      saved_at: new Date().toISOString(),
      note: note || `第 ${newVersionNum} 版语义留档`,
      template_rev: req.template?.rev || 1,
      shape: req.spec?.shape || 'grouped',
      semantic: {
        hash: `cal-${Date.now().toString(16).slice(-8)}`,
        calibers: (req.analysis?.calibers || []).map((c: any) => c.expr),
      },
    };

    if (!this.versions[reqId]) this.versions[reqId] = [];
    this.versions[reqId].unshift(newVer);
    this.notify();
    return newVer;
  }

  // --- Schedules ---
  public createSchedule(reqId: string, kind: any, at: string = '08:00', weekday?: number, interval_hours?: number): Schedule {
    const sch: Schedule = {
      id: `sch-${Date.now().toString(36).slice(-4)}`,
      requirement: reqId,
      kind,
      at,
      weekday,
      interval_hours,
      enabled: true,
      next_run: kind === 'daily' ? `明天 ${at}` : kind === 'weekly' ? `下周一 ${at}` : `每隔 ${interval_hours || 6} 小时`,
      last_status: 'succeeded',
      runs: 0,
    };
    this.schedules.unshift(sch);
    this.notify();
    return sch;
  }

  public toggleSchedule(id: string): Schedule {
    const sch = this.schedules.find((s) => s.id === id);
    if (!sch) throw new Error('排期不存在');
    sch.enabled = !sch.enabled;
    this.notify();
    return sch;
  }

  public runScheduleNow(id: string): { ok: boolean; run: Run } {
    const sch = this.schedules.find((s) => s.id === id);
    if (!sch) throw new Error('排期不存在');
    const { run } = this.runRequirement(sch.requirement);
    sch.runs += 1;
    sch.last_status = 'succeeded';
    this.notify();
    return { ok: true, run };
  }

  public deleteSchedule(id: string) {
    this.schedules = this.schedules.filter((s) => s.id !== id);
    this.notify();
  }

  // --- Alerts ---
  public checkAlerts(reqId: string, pct: number = 0.2): { ok: boolean; observation: any; alerts: Alert[]; notes: string[] } {
    const req = this.getRequirement(reqId);
    if (!req) throw new Error('需求不存在');

    const cur = 605200;
    const prev = 653700;
    const actualDrop = (prev - cur) / prev; // ~7.4%
    const isHit = actualDrop >= pct;

    const observation = {
      cur,
      prev,
      pct: -actualDrop,
    };

    const notes = [
      `本期窗口: 2026-09-01 ~ 2026-09-30, 上期窗口: 2026-08-01 ~ 2026-08-31`,
      `设定阈值: ${(pct * 100).toFixed(0)}%, 实际变动率: -${(actualDrop * 100).toFixed(1)}%`,
    ];

    const emittedAlerts: Alert[] = [];
    if (isHit) {
      const alt: Alert = {
        id: `alt-${Date.now().toString(36)}`,
        req_id: reqId,
        dsn: req.dataset.dsn || req.dataset.file,
        metric: 'amount',
        metric_label: '销售额',
        window: { cur_start: '2026-09-01', cur_end: '2026-09-30' },
        at: new Date().toISOString(),
        level: actualDrop > 0.3 ? 'critical' : 'warn',
        message: `${req.title} 销售额出现异动，降幅达到 ${(actualDrop * 100).toFixed(1)}%，已超过 ${(pct * 100).toFixed(0)}% 警戒线`,
      };
      this.alerts.unshift(alt);
      emittedAlerts.push(alt);
    }

    this.notify();
    return { ok: true, observation, alerts: emittedAlerts, notes };
  }

  // --- Datasources ---
  public saveDatasource(ds: Partial<Datasource>): Datasource {
    if (ds.id) {
      const idx = this.datasources.findIndex((x) => x.id === ds.id);
      if (idx !== -1) {
        this.datasources[idx] = {
          ...this.datasources[idx],
          ...ds,
          driver_label: ds.driver === 'sqlite' ? 'SQLite 文件' : ds.driver === 'postgres' ? 'PostgreSQL / Kingbase' : 'MySQL',
          has_password: !!ds.password || this.datasources[idx].has_password,
        } as Datasource;
        this.notify();
        return this.datasources[idx];
      }
    }

    const newDs: Datasource = {
      id: ds.id || `ds-${Date.now().toString(36).slice(-4)}`,
      label: ds.label || '新建数据源',
      driver: ds.driver || 'postgres',
      driver_label: ds.driver === 'sqlite' ? 'SQLite 文件' : ds.driver === 'postgres' ? 'PostgreSQL / Kingbase' : 'MySQL',
      host: ds.host || '127.0.0.1',
      port: ds.port || 5432,
      database: ds.database || 'demo',
      user: ds.user || 'postgres',
      password: ds.password,
      has_password: !!ds.password,
      options: ds.options || '',
    };
    this.datasources.push(newDs);
    this.notify();
    return newDs;
  }

  public deleteDatasource(id: string) {
    this.datasources = this.datasources.filter((x) => x.id !== id);
    this.notify();
  }

  // --- BizModel Management ---
  public getBizModel(dsnId: string): BizModel {
    if (!this.bizmodels[dsnId]) {
      // Create empty model scaffold if not present
      this.bizmodels[dsnId] = {
        schema: 'rpx-bizmodel/v1',
        version: 1,
        source: {
          dsn: dsnId,
          generated_at: new Date().toISOString(),
          generator: 'manual/created',
        },
        entities: [],
        relationships: [],
        metrics: [],
        dimensions: [],
        rules: [],
        open_questions: [],
      };
    }
    return this.bizmodels[dsnId];
  }

  public confirmMetric(dsnId: string, metricId: string) {
    const bm = this.getBizModel(dsnId);
    const m = bm.metrics.find((x) => x.id === metricId);
    if (m) {
      m.status = 'confirmed';
      m.evidence.push(`human_confirmed:${new Date().toISOString().slice(0, 10)}`);
      this.notify();
    }
  }

  public confirmDimension(dsnId: string, dimId: string) {
    const bm = this.getBizModel(dsnId);
    const d = bm.dimensions.find((x) => x.id === dimId);
    if (d) {
      d.status = 'confirmed';
      d.evidence.push(`human_confirmed:${new Date().toISOString().slice(0, 10)}`);
      this.notify();
    }
  }

  public addMetric(dsnId: string, metric: BizMetric) {
    const bm = this.getBizModel(dsnId);
    bm.metrics.unshift(metric);
    this.notify();
  }

  public addDimension(dsnId: string, dim: BizDimension) {
    const bm = this.getBizModel(dsnId);
    bm.dimensions.unshift(dim);
    this.notify();
  }

  public addRule(dsnId: string, rule: BizRule) {
    const bm = this.getBizModel(dsnId);
    bm.rules.unshift(rule);
    this.notify();
  }

  public deleteRule(dsnId: string, ruleId: string) {
    const bm = this.getBizModel(dsnId);
    bm.rules = bm.rules.filter((r) => r.id !== ruleId);
    this.notify();
  }

  public addAlias(dsnId: string, itemType: 'metric' | 'dimension', itemId: string, alias: string) {
    const bm = this.getBizModel(dsnId);
    if (itemType === 'metric') {
      const m = bm.metrics.find((x) => x.id === itemId);
      if (m && !m.aliases.includes(alias)) {
        m.aliases.push(alias);
        this.notify();
      }
    } else {
      const d = bm.dimensions.find((x) => x.id === itemId);
      if (d) {
        if (!d.aliases) d.aliases = [];
        if (!d.aliases.includes(alias)) {
          d.aliases.push(alias);
          this.notify();
        }
      }
    }
  }

  public getGoldenQuestions(): GoldenQuestion[] {
    return [...this.goldens];
  }

  public runGoldenEvaluation(): { total: number; passed: number; results: any[] } {
    const results = this.goldens.map((q) => {
      // Simulate evaluation through the query IR compiler
      return {
        id: q.id,
        intent: q.intent,
        matchedMetric: q.query.metrics[0]?.ref,
        matchedDims: q.query.dimensions.map((d) => d.ref),
        shape: q.expect.shape,
        passed: true,
        note: q.expect.note,
      };
    });
    return {
      total: this.goldens.length,
      passed: this.goldens.length,
      results,
    };
  }

  // --- Natural Language "Ask" bar generation (Semantic-Layer Grounded) ---
  public askAndGenerate(text: string, dsnId: string): Requirement {
    const ds = this.datasources.find((d) => d.id === dsnId);
    const parsed = parseCsv(RAW_SAMPLE_CSV);
    const bm = this.getBizModel(dsnId);

    // 1. Semantic Entity Linking via BizModel Aliases
    let matchedMetric = bm.metrics.find((m) =>
      m.aliases?.some((a) => text.includes(a)) || text.includes(m.label) || text.includes(m.id)
    );
    if (!matchedMetric) {
      matchedMetric = bm.metrics[0] || {
        id: 'sales_amount',
        label: '销售额',
        entity: 'sales',
        column: 'amount',
        expression: 'SUM(amount)',
        filters: [],
        evidence: [],
        aliases: ['销售额'],
        status: 'confirmed' as const,
      };
    }

    // 2. Identify dimensions
    const matchedDims: any[] = [];
    bm.dimensions.forEach((d) => {
      if (d.aliases?.some((a) => text.includes(a)) || text.includes(d.label)) {
        matchedDims.push(d);
      }
    });

    // Fallbacks if no specific dim mentioned
    let chosenShape: any = 'grouped';
    let layout: any = 'grouped';
    let rowDims: any[] = [];
    let colDims: any[] = [];

    if (text.includes('交叉') || text.includes('品类') || (matchedDims.length >= 2 && !text.includes('明细'))) {
      chosenShape = 'crosstab';
      layout = 'crosstab';
      const rDim = matchedDims[0]?.id === 'sales_category' ? 'sales_region' : (matchedDims[0]?.id || 'sales_region');
      const cDim = 'sales_category';
      rowDims = [{ field: 'region', label: '大区', sort: 'asc' }];
      colDims = [{ field: 'cat', label: '产品品类' }];
    } else if (text.includes('走势') || text.includes('月') || text.includes('趋势') || matchedDims.some((d) => d.id === 'sales_month')) {
      chosenShape = 'trend';
      rowDims = [{ field: 'month', label: '月份', sort: 'asc' }];
    } else if (text.includes('排行') || text.includes('top') || text.includes('名') || text.includes('降序')) {
      chosenShape = 'rank';
      rowDims = [{ field: 'store', label: '门店', sort: 'desc' }];
    } else if (text.includes('明细') || text.includes('流水')) {
      chosenShape = 'detail';
      layout = 'list';
      rowDims = [];
    } else {
      rowDims = [{ field: 'region', label: '区域', sort: 'asc' }];
    }

    const metricField = matchedMetric.column || 'amount';
    const spec: Spec = {
      layout,
      shape: chosenShape,
      row_dims: rowDims,
      col_dims: colDims,
      measures: [
        {
          field: metricField,
          agg: 'sum',
          label: matchedMetric.label,
          format: matchedMetric.format || '#,##0',
        },
      ],
      totals: { grand: true, subtotal: layout === 'crosstab' },
    };

    const dataset = {
      kind: 'sql' as const,
      dsn: dsnId,
      sql: `SELECT ${parsed.columns.join(', ')} FROM sales;`,
      rows: parsed.rows,
      columns: parsed.columns,
      column_meta: parsed.column_meta,
      preview: parsed.preview,
    };

    const reqId = `req-${Date.now().toString(36).slice(-4)}`;
    const tpl = compileTemplate(spec, dataset, 0);

    const activeRules = (bm.rules || []).filter((r) => r.status === 'confirmed').map((r) => r.label);

    const newReq: Requirement = {
      id: reqId,
      title: text,
      description: `来自自然语言解析：「${text}」，数据源 ${ds?.label || dsnId}`,
      dataset,
      roles: { measures: [metricField] },
      template_choice: {
        id: 'generated-nl',
        name: `基于业务模型 [${matchedMetric.label}] 编译模板`,
        shape_label: SHAPE_LABELS[chosenShape] || '分组汇总',
        source: 'generated',
        reasons: [
          `命中业务模型口径：${matchedMetric.label}（${matchedMetric.expression}）`,
          activeRules.length > 0 ? `自动装配业务规则：${activeRules.join('；')}` : '无排他规则',
          '通过 rpx-query/v1 契约验证，无未决歧义',
        ],
        missing: [],
      },
      spec,
      shape_decision: detectShape(spec, dataset, text),
      analysis: {
        calibers: [
          {
            label: matchedMetric.label,
            expr: matchedMetric.expression,
            agg: 'sum',
            scope: `语义层指标 [${matchedMetric.id}] · 状态: ${matchedMetric.status}`,
          },
        ],
        decisions: [
          { what: `选定 ${SHAPE_LABELS[chosenShape]} 展现形态`, source: '语义特征推导' },
          { what: `采用业务口径 ${matchedMetric.label}`, source: `同义词库命中及事实层状态: ${matchedMetric.status}` },
        ],
        open_questions: [],
        dataset: { rows: dataset.rows, columns: dataset.columns },
        estimate: { data_rows: dataset.rows, how: 'SQL 确定性执行' },
      },
      template: tpl,
      runs: [],
      status: 'compiled',
      status_label: '已出稿',
      updated_at: new Date().toISOString(),
    };

    this.requirements.unshift(newReq);
    this.notify();
    return newReq;
  }
}

export const store = new ReportStudioStore();
