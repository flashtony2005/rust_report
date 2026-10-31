import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ApiError,
  askQuestion,
  compileQuery,
  deleteDimension,
  deleteGolden,
  deleteMetric,
  deleteRule,
  discoverBizModel,
  editAlias,
  getBizModel,
  getSemanticDb,
  listDatasources,
  listGoldens,
  patchDimension,
  patchEntity,
  patchRelationship,
  promoteBizItem,
  runGoldens,
  runQuery,
  upsertGolden,
  upsertMetric,
  upsertRule,
  type BizModel,
  type Datasource,
  type Golden,
  type GoldenExpect,
  type GoldenOutcome,
  type SemanticDbStatus,
} from '../services/api';
import { fmtTime } from '../services/reports';

/**
 * 业务知识（BizModel）—— 语义层的资产台账。
 *
 * 页面口径（与后端一致，改一处要改两处）：
 * - **事实与推断分离**：每条都带 `status` 与 `evidence`，`confirmed` 只能人工产生；
 * - **指标口径一律 draft 起步**，人改过才 `confirmed`；
 * - **规则只有 confirmed 才装配进 SQL**（草稿规则不许改数字）；
 * - **金样例没有期望值不算通过**（「没断言」≠「通过」）。
 *
 * 所有数据来自 `GET /bizmodels/{dsn}` 与 `GET /bizmodels/{dsn}/goldens`，页面里没有写死的示例。
 */

type Tab = 'ontology' | 'metrics' | 'dimensions' | 'rules' | 'aliases' | 'goldens';

interface MetricForm {
  id: string;
  label: string;
  expression: string;
  entity: string;
  column: string;
  format: string;
  time_field: string;
}

interface RuleForm {
  id?: string;
  label: string;
  expression: string;
  applies_to: string[];
}

interface GoldenForm {
  id?: string;
  question: string;
  metrics: string[];
  dims: string[];
  timeField: string;
  timeGrain: string;
  preset: string;
  start: string;
  end: string;
  rows: string;
  firstCell: string;
  columns: string;
  note: string;
}

interface DimForm {
  id: string;
  label: string;
  kind: string;
  column: string;
  time_grain: string;
  values: string;
  aliases: string;
}

interface EntityForm {
  id: string;
  table: string;
  label: string;
  grain_description: string;
  grain_key: string;
}

interface RelForm {
  fromEntity: string;
  fromKey: string;
  toEntity: string;
  toKey: string;
  cardinality: string;
}

const PRESETS: Array<[string, string]> = [
  ['', '不限定时间'],
  ['last_month', '上一个整月'],
  ['last_7_days', '最近 7 天'],
  ['last_30_days', '最近 30 天'],
  ['this_month', '本月'],
  ['last_3_months', '最近 3 个月'],
  ['this_year', '本年'],
  ['last_year', '去年'],
];

const GRAINS: Array<[string, string]> = [
  ['', '不截断'],
  ['day', '按日'],
  ['week', '按周'],
  ['month', '按月'],
  ['quarter', '按季'],
  ['year', '按年'],
];

const STATUS_LABEL: Record<string, string> = {
  confirmed: '已核准',
  candidate: '待确认',
  draft: '草稿',
};

function statusClass(status?: string): string {
  if (status === 'confirmed') return 'ran';
  if (status === 'candidate') return 'analyzed';
  return 'draft';
}

function errText(e: unknown): string {
  if (e instanceof ApiError) return `${e.code}：${e.message}`;
  return e instanceof Error ? e.message : String(e);
}

/** 编辑态 → IR。与后端的 `rpx-query/v1` 字段一一对应，不加私有字段。 */
function buildQuery(f: GoldenForm): Record<string, unknown> {
  const query: Record<string, unknown> = {
    schema: 'rpx-query/v1',
    metrics: f.metrics.map((ref) => ({ ref })),
  };
  if (f.dims.length) {
    query.dimensions = f.dims.map((ref) => {
      const dim: Record<string, unknown> = { ref };
      if (f.timeGrain && ref === f.timeField) dim.time_grain = f.timeGrain;
      return dim;
    });
  }
  if (f.timeField) {
    if (f.preset) query.time_range = { field: f.timeField, preset: f.preset };
    else if (f.start && f.end) query.time_range = { field: f.timeField, start: f.start, end: f.end };
  }
  return query;
}

function expectOf(f: GoldenForm): GoldenExpect {
  const expect: GoldenExpect = {};
  if (f.rows.trim() !== '') expect.rows = Number(f.rows);
  if (f.firstCell.trim() !== '') expect.first_cell = f.firstCell.trim();
  const columns = splitList(f.columns);
  if (columns.length) expect.columns = columns;
  if (f.note.trim() !== '') expect.note = f.note.trim();
  return expect;
}

/** 逗号/顿号/换行分隔的列表 → 数组（去空、去重，保持输入顺序）。 */
function splitList(text: string): string[] {
  const out: string[] = [];
  for (const part of text.split(/[,，、\n]/)) {
    const one = part.trim();
    if (one && !out.includes(one)) out.push(one);
  }
  return out;
}

/** 把一条已有金样例还原成编辑态（**编辑**而不是重新建一条）。 */
function formOfGolden(g: Golden, timeField: string): GoldenForm {
  const q = g.query as {
    metrics?: Array<{ ref: string }>;
    dimensions?: Array<{ ref: string; time_grain?: string }>;
    time_range?: { field?: string; preset?: string; start?: string; end?: string };
  };
  const tr = q.time_range ?? {};
  const dimRefs = (q.dimensions ?? []).map((d) => d.ref);
  const grainDim = (q.dimensions ?? []).find((d) => d.time_grain);
  return {
    id: g.id,
    question: g.question,
    metrics: (q.metrics ?? []).map((m) => m.ref),
    dims: dimRefs,
    timeField: tr.field ?? timeField,
    timeGrain: grainDim?.time_grain ?? '',
    preset: tr.preset ?? '',
    start: tr.start ?? '',
    end: tr.end ?? '',
    rows: g.expect.rows != null ? String(g.expect.rows) : '',
    firstCell: g.expect.first_cell ?? '',
    columns: (g.expect.columns ?? []).join('、'),
    note: g.expect.note ?? '',
  };
}

const emptyGolden = (timeField = ''): GoldenForm => ({
  question: '',
  metrics: [],
  dims: [],
  timeField,
  timeGrain: '',
  preset: '',
  start: '',
  end: '',
  rows: '',
  firstCell: '',
  columns: '',
  note: '',
});

/** 关系基数白名单（与后端一致；写错直接改数，所以只给四个选项）。 */
const CARDINALITIES: Array<[string, string]> = [
  ['many_to_one', '多对一（上溯，不放大）'],
  ['one_to_one', '一对一'],
  ['one_to_many', '一对多（下钻，会放大）'],
  ['many_to_many', '多对多（必须预聚合）'],
];

const DIM_KINDS: Array<[string, string]> = [
  ['enum', '枚举（有值域）'],
  ['text', '文本'],
  ['number', '数值'],
  ['date', '日期'],
];

export const BizModelView: React.FC = () => {
  const [sources, setSources] = useState<Datasource[]>([]);
  const [dsn, setDsn] = useState('');
  const [model, setModel] = useState<BizModel | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>('metrics');

  const [goldens, setGoldens] = useState<Golden[]>([]);
  const [outcomes, setOutcomes] = useState<GoldenOutcome[] | null>(null);
  const [summary, setSummary] = useState<{
    total?: number;
    passed?: number;
    failed?: number;
    avg_ms?: number;
  } | null>(null);

  const [metricForm, setMetricForm] = useState<MetricForm | null>(null);
  const [ruleForm, setRuleForm] = useState<RuleForm | null>(null);
  const [goldenForm, setGoldenForm] = useState<GoldenForm | null>(null);
  const [aliasFor, setAliasFor] = useState<{ section: 'metrics' | 'dimensions'; id: string } | null>(null);
  const [aliasText, setAliasText] = useState('');
  const [trial, setTrial] = useState<{ sql: string; columns: string[]; rows: unknown[][]; count: number } | null>(null);
  const [sample, setSample] = useState(true);

  // 管理台新增的编辑/过滤状态
  const [db, setDb] = useState<SemanticDbStatus | null>(null);
  const [query, setQuery] = useState('');
  const [dimForm, setDimForm] = useState<DimForm | null>(null);
  const [entityForm, setEntityForm] = useState<EntityForm | null>(null);
  const [relForm, setRelForm] = useState<RelForm | null>(null);
  const [questionText, setQuestionText] = useState('');
  const [metricTrial, setMetricTrial] = useState<{
    id: string;
    sql: string;
    columns: string[];
    rows: unknown[][];
    count: number;
  } | null>(null);

  /** 当前页签的过滤词（名称 / 标识 / 公式 / 别名都算命中）。 */
  const q = query.trim().toLowerCase();
  const hit = (...fields: Array<string | null | undefined>) =>
    !q || fields.some((f) => (f ?? '').toLowerCase().includes(q));

  /** 过滤生效时，当前页签命中多少条（给过滤框右侧的说明用）。 */
  const countsVisible = (() => {
    if (!q || !model) return 0;
    switch (tab) {
      case 'ontology':
        return (
          model.entities.filter((e) => hit(e.id, e.label, e.table as string)).length +
          model.relationships.length +
          model.open_questions.filter((x) => hit(x.question, x.source)).length
        );
      case 'metrics':
        return model.metrics.filter((m) => hit(m.id, m.label, m.expression, (m.aliases ?? []).join(' ')))
          .length;
      case 'dimensions':
        return model.dimensions.filter((d) =>
          hit(d.id, d.label, d.kind, (d.values ?? []).join(' '), (d.aliases ?? []).join(' ')),
        ).length;
      case 'rules':
        return model.rules.filter((r) => hit(r.id, r.label, r.expression)).length;
      case 'aliases':
        return [...model.metrics, ...model.dimensions].filter((x) =>
          hit(x.id, x.label, (x.aliases ?? []).join(' ')),
        ).length;
      default:
        return goldens.filter((g) => hit(g.id, g.question, g.expect.note)).length;
    }
  })();

  const timeFields = useMemo(
    () => (model?.dimensions ?? []).filter((d) => (d.kind as string) === 'date'),
    [model],
  );

  const counts = useMemo(
    () => ({
      entities: model?.entities.length ?? 0,
      relationships: model?.relationships.length ?? 0,
      metrics: model?.metrics.length ?? 0,
      dimensions: model?.dimensions.length ?? 0,
      rules: model?.rules.length ?? 0,
      aliases:
        (model?.metrics ?? []).reduce((n, m) => n + (m.aliases?.length ?? 0), 0) +
        (model?.dimensions ?? []).reduce((n, d) => n + (d.aliases?.length ?? 0), 0),
      goldens: goldens.length,
    }),
    [model, goldens],
  );

  const loadModel = useCallback(async (target: string) => {
    if (!target) return;
    setLoading(true);
    setError(null);
    try {
      const res = await getBizModel(target);
      setModel(res.model);
      const g = await listGoldens(target);
      setGoldens(g.items ?? []);
      // 语义库状态是"附加信息"：拿不到就不显示，不让它挡页面
      try {
        setDb(await getSemanticDb());
      } catch {
        setDb(null);
      }
    } catch (e) {
      setError(errText(e));
      setModel(null);
      setGoldens([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const page = await listDatasources();
        setSources(page.items ?? []);
        const first = page.items?.[0]?.id ?? '';
        setDsn(first);
        await loadModel(first);
      } catch (e) {
        setError(errText(e));
        setLoading(false);
      }
    })();
  }, [loadModel]);

  const switchDsn = async (next: string) => {
    setDsn(next);
    setOutcomes(null);
    setSummary(null);
    setMetricForm(null);
    setRuleForm(null);
    setGoldenForm(null);
    setDimForm(null);
    setEntityForm(null);
    setRelForm(null);
    setQuestionText('');
    setQuery('');
    setTrial(null);
    setMetricTrial(null);
    await loadModel(next);
  };

  const run = async (label: string, fn: () => Promise<void>) => {
    setBusy(true);
    setNotice(null);
    setError(null);
    try {
      await fn();
      setNotice(label);
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  };

  const refreshGoldens = async () => {
    const g = await listGoldens(dsn);
    setGoldens(g.items ?? []);
  };

  // ---------------------------------------------------------------- 指标

  const renderMetrics = () => {
    const all = model?.metrics ?? [];
    const list = all.filter((m) => hit(m.id, m.label, m.expression, (m.aliases ?? []).join(' ')));
    return (
      <section className="card pad">
        <div className="row" style={{ alignItems: 'flex-start' }}>
          <div className="grow">
            <h3 style={{ margin: 0, fontSize: 15 }}>指标口径库</h3>
            <div className="hint" style={{ marginTop: 4 }}>
              指标 = 带口径的聚合表达式。人工改过的表达式自动成为事实层（<b>confirmed</b>），
              自动推断出来的只能算草稿。
            </div>
          </div>
          <button
            className="btn small primary"
            onClick={() =>
              setMetricForm({
                id: '',
                label: '',
                expression: '',
                entity: model?.entities[0]?.id ?? '',
                column: '',
                format: '#,##0',
                time_field: timeFields[0]?.id ?? '',
              })
            }
          >
            ＋ 新建指标
          </button>
        </div>

        {metricForm ? (
          <div className="card pad" style={{ marginTop: 14, background: 'var(--paper-2)' }}>
            <div className="eyebrow">Metric</div>
            <div className="form-grid" style={{ marginTop: 10 }}>
              <div>
                <label className="field">标识</label>
                <input
                  type="text"
                  value={metricForm.id}
                  placeholder="net_amount"
                  onChange={(e) => setMetricForm({ ...metricForm, id: e.target.value })}
                />
              </div>
              <div>
                <label className="field">显示名</label>
                <input
                  type="text"
                  value={metricForm.label}
                  placeholder="净额"
                  onChange={(e) => setMetricForm({ ...metricForm, label: e.target.value })}
                />
              </div>
              <div>
                <label className="field">实体</label>
                <select
                  value={metricForm.entity}
                  onChange={(e) => setMetricForm({ ...metricForm, entity: e.target.value })}
                >
                  {(model?.entities ?? []).map((en) => (
                    <option key={en.id} value={en.id}>
                      {en.label || en.id}
                    </option>
                  ))}
                </select>
              </div>
              <div style={{ gridColumn: 'span 2' }}>
                <label className="field">
                  聚合表达式（白名单：sum/avg/count/min/max/abs/round/coalesce/nullif）
                </label>
                <input
                  type="text"
                  value={metricForm.expression}
                  placeholder="SUM(amount) - SUM(discount)"
                  onChange={(e) => setMetricForm({ ...metricForm, expression: e.target.value })}
                />
              </div>
              <div>
                <label className="field">格式</label>
                <input
                  type="text"
                  value={metricForm.format}
                  placeholder="#,##0.00"
                  onChange={(e) => setMetricForm({ ...metricForm, format: e.target.value })}
                />
              </div>
            </div>
            <div className="form-actions">
              <button
                className="btn primary"
                disabled={busy}
                onClick={() =>
                  run('指标已保存（人工口径即事实层）', async () => {
                    const res = await upsertMetric(dsn, {
                      id: metricForm.id.trim(),
                      label: metricForm.label.trim() || undefined,
                      expression: metricForm.expression.trim(),
                      entity: metricForm.entity || undefined,
                      column: metricForm.column.trim() || undefined,
                      format: metricForm.format.trim() || undefined,
                      time_field: metricForm.time_field || undefined,
                    });
                    setModel(res.model);
                    setMetricForm(null);
                  })
                }
              >
                保存
              </button>
              <button className="btn" onClick={() => setMetricForm(null)}>
                取消
              </button>
              <span className="hint">表达式先过编译器白名单：存进去却编不出来的口径不算资产。</span>
            </div>
          </div>
        ) : null}

        <table className="tbl" style={{ marginTop: 14 }}>
          <thead>
            <tr>
              <th style={{ width: '17%' }}>指标 / 标识</th>
              <th style={{ width: '24%' }}>公式</th>
              <th style={{ width: '18%' }}>别名</th>
              <th style={{ width: '9%' }}>状态</th>
              <th style={{ width: '32%' }}>操作</th>
            </tr>
          </thead>
          <tbody>
            {list.map((m) => (
              <tr key={m.id}>
                <td>
                  <b>{m.label || m.id}</b>
                  <div className="tiny">{m.id}</div>
                </td>
                <td>
                  <code className="sql-inline">{m.expression || '—'}</code>
                  <div className="tiny">{m.format || '无格式'}</div>
                </td>
                <td>
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                    {(m.aliases ?? []).map((a) => (
                      <span key={a} className="pill">
                        {a}
                      </span>
                    ))}
                  </div>
                </td>
                <td>
                  <span className={`badge ${statusClass(m.status)}`}>
                    {STATUS_LABEL[m.status] ?? m.status}
                  </span>
                </td>
                <td>
                  <div className="row" style={{ gap: 6, flexWrap: 'wrap' }}>
                    <button
                      className="btn small"
                      onClick={() =>
                        setMetricForm({
                          id: m.id,
                          label: m.label ?? '',
                          expression: (m.expression as string) ?? '',
                          entity: (m.entity as string) ?? '',
                          column: (m.column as string) ?? '',
                          format: (m.format as string) ?? '',
                          time_field: (m.time_field as string) ?? '',
                        })
                      }
                    >
                      编辑
                    </button>
                    <button
                      className="btn small"
                      disabled={busy}
                      title="只跑这一条指标，看 SQL 与结果"
                      onClick={() =>
                        run(`试跑 ${m.id} 完成`, async () => {
                          const res = await runQuery(dsn, {
                            schema: 'rpx-query/v1',
                            metrics: [{ ref: m.id }],
                          });
                          setMetricTrial({
                            id: m.id,
                            sql: res.compiled.sql,
                            columns: res.columns,
                            rows: res.rows,
                            count: res.row_count,
                          });
                        })
                      }
                    >
                      试跑
                    </button>
                    {m.status !== 'confirmed' ? (
                      <button
                        className="btn small"
                        disabled={busy}
                        onClick={() =>
                          run(`已核准 ${m.id}`, async () => {
                            const res = await promoteBizItem(dsn, 'metrics', m.id);
                            setModel(res.model);
                          })
                        }
                      >
                        核准
                      </button>
                    ) : null}
                    <button
                      className="btn small"
                      disabled={busy}
                      onClick={() =>
                        run(`已删除指标 ${m.id}`, async () => {
                          const res = await deleteMetric(dsn, m.id);
                          setModel(res.model);
                          if (metricTrial?.id === m.id) setMetricTrial(null);
                        })
                      }
                    >
                      删除
                    </button>
                  </div>
                </td>
              </tr>
            ))}
            {list.length === 0 ? (
              <tr>
                <td colSpan={5} className="empty">
                  {all.length ? '没有匹配的指标' : '还没有指标 —— 先「重新发现」，或直接新建一条'}
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>

        {metricTrial ? (
          <div style={{ marginTop: 14 }}>
            <div className="row" style={{ gap: 8 }}>
              <b className="tiny">试跑 {metricTrial.id}</b>
              <span className="tiny">共 {metricTrial.count} 行</span>
              <div className="grow" />
              <button className="btn small" onClick={() => setMetricTrial(null)}>
                收起
              </button>
            </div>
            <pre className="sqlbox">{metricTrial.sql}</pre>
            {metricTrial.columns.length ? (
              <div className="preview-wrap">
                <table className="tbl dense compact">
                  <thead>
                    <tr>
                      {metricTrial.columns.map((c) => (
                        <th key={c}>{c}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {metricTrial.rows.slice(0, 5).map((row, i) => (
                      <tr key={String(i)}>
                        {row.map((cell, j) => (
                          <td key={String(j)} className={typeof cell === 'number' ? 'num' : undefined}>
                            {cell == null ? '' : String(cell)}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : null}
          </div>
        ) : null}
      </section>
    );
  };

  // ---------------------------------------------------------------- 维度

  const renderDimensions = () => {
    const all = model?.dimensions ?? [];
    const list = all.filter((d) =>
      hit(d.id, d.label, d.kind, (d.aliases ?? []).join(' '), (d.values ?? []).join(' ')),
    );
    return (
      <section className="card pad">
        <h3 style={{ margin: 0, fontSize: 15 }}>维度与值域字典</h3>
        <div className="hint" style={{ marginTop: 4 }}>
          低基数列的枚举值域是实体链接的白名单：有值域才能把「大区」对到 <code>region</code>。
          维度不新造 —— 它的物理列来自库；能改的是<b>怎么读它</b>（类型、粒度、值域、别名）。
        </div>

        {dimForm ? (
          <div className="card pad" style={{ marginTop: 14, background: 'var(--paper-2)' }}>
            <div className="eyebrow">Dimension</div>
            <div className="form-grid" style={{ marginTop: 10 }}>
              <div>
                <label className="field">标识</label>
                <input type="text" value={dimForm.id} readOnly style={{ opacity: 0.7 }} />
              </div>
              <div>
                <label className="field">显示名</label>
                <input
                  type="text"
                  value={dimForm.label}
                  onChange={(e) => setDimForm({ ...dimForm, label: e.target.value })}
                />
              </div>
              <div>
                <label className="field">类型</label>
                <select
                  value={dimForm.kind}
                  onChange={(e) => setDimForm({ ...dimForm, kind: e.target.value })}
                >
                  {DIM_KINDS.map(([v, label]) => (
                    <option key={v} value={v}>
                      {label}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="field">物理列</label>
                <input
                  type="text"
                  value={dimForm.column}
                  placeholder="region"
                  onChange={(e) => setDimForm({ ...dimForm, column: e.target.value })}
                />
              </div>
              <div>
                <label className="field">时间粒度（日期维度）</label>
                <select
                  value={dimForm.time_grain}
                  onChange={(e) => setDimForm({ ...dimForm, time_grain: e.target.value })}
                >
                  {GRAINS.map(([v, label]) => (
                    <option key={v} value={v}>
                      {label}
                    </option>
                  ))}
                </select>
              </div>
              <div style={{ gridColumn: 'span 3' }}>
                <label className="field">值域（逗号 / 顿号分隔；留空 = 不限制）</label>
                <input
                  type="text"
                  value={dimForm.values}
                  placeholder="华东、华南、华北"
                  onChange={(e) => setDimForm({ ...dimForm, values: e.target.value })}
                />
              </div>
              <div style={{ gridColumn: 'span 3' }}>
                <label className="field">别名（逗号 / 顿号分隔）</label>
                <input
                  type="text"
                  value={dimForm.aliases}
                  placeholder="大区、区域"
                  onChange={(e) => setDimForm({ ...dimForm, aliases: e.target.value })}
                />
              </div>
            </div>
            <div className="form-actions">
              <button
                className="btn primary"
                disabled={busy}
                onClick={() =>
                  run(`维度 ${dimForm.id} 已保存`, async () => {
                    const res = await patchDimension(dsn, {
                      id: dimForm.id,
                      label: dimForm.label.trim(),
                      kind: dimForm.kind || undefined,
                      column: dimForm.column.trim() || undefined,
                      time_grain: dimForm.time_grain,
                      values: splitList(dimForm.values),
                      aliases: splitList(dimForm.aliases),
                    });
                    setModel(res.model);
                    setDimForm(null);
                  })
                }
              >
                保存
              </button>
              <button className="btn" onClick={() => setDimForm(null)}>
                取消
              </button>
              <span className="hint">改这些不需要动库 —— 它只改「怎么读这一列」，不改数据本身。</span>
            </div>
          </div>
        ) : null}

        <table className="tbl" style={{ marginTop: 14 }}>
          <thead>
            <tr>
              <th style={{ width: '16%' }}>维度 / 标识</th>
              <th style={{ width: '10%' }}>类型</th>
              <th style={{ width: '24%' }}>值域样本</th>
              <th style={{ width: '16%' }}>别名</th>
              <th style={{ width: '9%' }}>状态</th>
              <th style={{ width: '25%' }}>操作</th>
            </tr>
          </thead>
          <tbody>
            {list.map((d) => (
              <tr key={d.id}>
                <td>
                  <b>{d.label || d.id}</b>
                  <div className="tiny">
                    {d.id}
                    {d.column ? ` · ${d.column}` : ''}
                  </div>
                </td>
                <td>
                  <span className="pill">{d.kind ?? 'text'}</span>
                  {d.time_grain ? <div className="tiny">粒度 {d.time_grain}</div> : null}
                </td>
                <td className="tiny">
                  {(d.values ?? []).length ? (d.values ?? []).slice(0, 10).join('、') : '无枚举限制'}
                  {(d.values ?? []).length > 10 ? ' …' : ''}
                </td>
                <td>
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                    {(d.aliases ?? []).map((a) => (
                      <span key={a} className="pill">
                        {a}
                      </span>
                    ))}
                  </div>
                </td>
                <td>
                  <span className={`badge ${statusClass(d.status)}`}>
                    {STATUS_LABEL[d.status] ?? d.status}
                  </span>
                </td>
                <td>
                  <div className="row" style={{ gap: 6, flexWrap: 'wrap' }}>
                    <button
                      className="btn small"
                      onClick={() =>
                        setDimForm({
                          id: d.id,
                          label: d.label ?? '',
                          kind: d.kind ?? 'text',
                          column: (d.column as string) ?? '',
                          time_grain: (d.time_grain as string) ?? '',
                          values: (d.values ?? []).join('、'),
                          aliases: (d.aliases ?? []).join('、'),
                        })
                      }
                    >
                      编辑
                    </button>
                    {d.status !== 'confirmed' ? (
                      <button
                        className="btn small"
                        disabled={busy}
                        onClick={() =>
                          run(`已核准 ${d.id}`, async () => {
                            const res = await promoteBizItem(dsn, 'dimensions', d.id);
                            setModel(res.model);
                          })
                        }
                      >
                        核准
                      </button>
                    ) : null}
                    <button
                      className="btn small"
                      disabled={busy}
                      onClick={() =>
                        run(`已删除维度 ${d.id}`, async () => {
                          const res = await deleteDimension(dsn, d.id);
                          setModel(res.model);
                          if (dimForm?.id === d.id) setDimForm(null);
                        })
                      }
                    >
                      删除
                    </button>
                  </div>
                </td>
              </tr>
            ))}
            {list.length === 0 ? (
              <tr>
                <td colSpan={6} className="empty">
                  {all.length ? '没有匹配的维度' : '还没有维度 —— 先「重新发现」'}
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </section>
    );
  };

  // ---------------------------------------------------------------- 规则

  const renderRules = () => {
    const all = model?.rules ?? [];
    const list = all.filter((r) => hit(r.id, r.label, r.expression, (r.applies_to ?? []).join(' ')));
    return (
      <section className="card pad">
        <div className="row" style={{ alignItems: 'flex-start' }}>
          <div className="grow">
            <h3 style={{ margin: 0, fontSize: 15 }}>业务规则与过滤</h3>
            <div className="hint" style={{ marginTop: 4 }}>
              规则是布尔表达式（如 <code>amount &gt; 0</code>），编译时自动并进 WHERE。
              只有<b>已核准</b>的规则会生效 —— 草稿规则不许改数字。<code>applies_to</code> 为空 = 全局。
            </div>
          </div>
          <button
            className="btn small primary"
            onClick={() => setRuleForm({ label: '', expression: '', applies_to: [] })}
          >
            ＋ 新建规则
          </button>
        </div>

        {ruleForm ? (
          <div className="card pad" style={{ marginTop: 14, background: 'var(--paper-2)' }}>
            <div className="eyebrow">Rule</div>
            <div className="form-grid" style={{ marginTop: 10 }}>
              <div>
                <label className="field">名称</label>
                <input
                  type="text"
                  value={ruleForm.label}
                  placeholder="有效订单"
                  onChange={(e) => setRuleForm({ ...ruleForm, label: e.target.value })}
                />
              </div>
              <div style={{ gridColumn: 'span 2' }}>
                <label className="field">布尔表达式</label>
                <input
                  type="text"
                  value={ruleForm.expression}
                  placeholder="amount > 0"
                  onChange={(e) => setRuleForm({ ...ruleForm, expression: e.target.value })}
                />
              </div>
              <div style={{ gridColumn: 'span 3' }}>
                <label className="field">生效实体（不选 = 全局）</label>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                  {(model?.entities ?? []).map((en) => {
                    const on = ruleForm.applies_to.includes(en.id);
                    return (
                      <button
                        key={en.id}
                        className={`tog ${on ? 'on' : ''}`}
                        onClick={() =>
                          setRuleForm({
                            ...ruleForm,
                            applies_to: on
                              ? ruleForm.applies_to.filter((x) => x !== en.id)
                              : [...ruleForm.applies_to, en.id],
                          })
                        }
                      >
                        {en.label || en.id}
                      </button>
                    );
                  })}
                </div>
              </div>
            </div>
            <div className="form-actions">
              <button
                className="btn primary"
                disabled={busy}
                onClick={() =>
                  run('规则已保存（已核准规则即刻生效）', async () => {
                    const res = await upsertRule(dsn, {
                      id: ruleForm.id,
                      label: ruleForm.label.trim(),
                      expression: ruleForm.expression.trim(),
                      applies_to: ruleForm.applies_to,
                    });
                    setModel(res.model);
                    setRuleForm(null);
                  })
                }
              >
                保存
              </button>
              <button className="btn" onClick={() => setRuleForm(null)}>
                取消
              </button>
            </div>
          </div>
        ) : null}

        <div className="list" style={{ marginTop: 14 }}>
          {list.map((r) => (
            <div key={r.id} className="list-row" style={{ alignItems: 'flex-start' }}>
              <div className="grow">
                <div className="t">
                  {r.label} <span className="tiny">{r.id}</span>
                </div>
                <pre className="sqlbox">{r.expression}</pre>
                <div className="s">
                  生效实体：{(r.applies_to ?? []).join('、') || '全局'} · 依据：
                  {(r.evidence ?? []).join('、') || '—'}
                </div>
              </div>
              <span className={`badge ${statusClass(r.status)}`}>
                {STATUS_LABEL[r.status] ?? r.status}
              </span>
              <div className="row" style={{ gap: 6 }}>
                <button
                  className="btn small"
                  onClick={() =>
                    setRuleForm({
                      id: r.id,
                      label: r.label ?? '',
                      expression: (r.expression as string) ?? '',
                      applies_to: (r.applies_to as string[]) ?? [],
                    })
                  }
                >
                  编辑
                </button>
                <button
                  className="btn small"
                  disabled={busy}
                  onClick={() =>
                    run(`已删除规则 ${r.id}`, async () => {
                      const res = await deleteRule(dsn, r.id);
                      setModel(res.model);
                      if (ruleForm?.id === r.id) setRuleForm(null);
                    })
                  }
                >
                  删除
                </button>
              </div>
            </div>
          ))}
          {list.length === 0 ? (
            <div className="empty">
              {all.length
                ? '没有匹配的规则'
                : '还没有业务规则。规则挂在模型上而不是模板上 —— 换报表也照样生效。'}
            </div>
          ) : null}
        </div>
      </section>
    );
  };

  // ---------------------------------------------------------------- 别名

  const renderAliases = () => {
    const rows = [
      ...(model?.metrics ?? []).map((m) => ({ section: 'metrics' as const, item: m })),
      ...(model?.dimensions ?? []).map((d) => ({ section: 'dimensions' as const, item: d })),
    ].filter(({ item }) => hit(item.id, item.label, (item.aliases ?? []).join(' ')));
    return (
      <section className="card pad">
        <h3 style={{ margin: 0, fontSize: 15 }}>同义词映射</h3>
        <div className="hint" style={{ marginTop: 4 }}>
          业务人员说的是行话（「流水」「营收」「大区」）。别名只影响检索与提示，<b>不改口径</b> ——
          所以改别名不会让历史结果变得不可比。
        </div>
        <div className="list" style={{ marginTop: 14 }}>
          {rows.map(({ section, item }) => (
            <div key={`${section}-${item.id}`} className="list-row">
              <div style={{ width: 200 }}>
                <b>{item.label || item.id}</b>
                <div className="tiny">
                  {section === 'metrics' ? '指标' : '维度'} · {item.id}
                </div>
              </div>
              <div className="grow" style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
                {(item.aliases ?? []).map((a) => (
                  <span key={a} className="pill">
                    {a}
                    <button
                      title="删除这个同义词"
                      style={{
                        border: 0,
                        background: 'none',
                        cursor: 'pointer',
                        padding: '0 0 0 4px',
                        color: 'inherit',
                      }}
                      onClick={() =>
                        run(`已移除「${a}」`, async () => {
                          const res = await editAlias(dsn, { section, id: item.id, remove: a });
                          setModel(res.model);
                        })
                      }
                    >
                      ×
                    </button>
                  </span>
                ))}
                {(item.aliases ?? []).length === 0 ? <span className="tiny">没有别名</span> : null}
              </div>
              {aliasFor && aliasFor.section === section && aliasFor.id === item.id ? (
                <div className="row" style={{ gap: 6 }}>
                  <input
                    type="text"
                    autoFocus
                    value={aliasText}
                    placeholder="新同义词"
                    style={{ width: 130 }}
                    onChange={(e) => setAliasText(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && aliasText.trim()) {
                        run(`已为 ${item.id} 添加「${aliasText.trim()}」`, async () => {
                          const res = await editAlias(dsn, {
                            section,
                            id: item.id,
                            add: aliasText.trim(),
                          });
                          setModel(res.model);
                          setAliasFor(null);
                          setAliasText('');
                        });
                      }
                    }}
                  />
                  <button
                    className="btn small"
                    onClick={() => {
                      setAliasFor(null);
                      setAliasText('');
                    }}
                  >
                    取消
                  </button>
                </div>
              ) : (
                <button
                  className="btn small"
                  onClick={() => {
                    setAliasFor({ section, id: item.id });
                    setAliasText('');
                  }}
                >
                  ＋ 加同义词
                </button>
              )}
            </div>
          ))}
          {rows.length === 0 ? <div className="empty">还没有可加别名的资产</div> : null}
        </div>
      </section>
    );
  };

  // ---------------------------------------------------------------- 本体

  const renderOntology = () => {
    const entities = model?.entities ?? [];
    const rels = model?.relationships ?? [];
    const questions = model?.open_questions ?? [];
    return (
      <div className="stack">
        <section className="card pad">
          <div className="row" style={{ alignItems: 'flex-start' }}>
            <div className="grow">
              <h3 style={{ margin: 0, fontSize: 15 }}>实体与粒度</h3>
              <div className="hint" style={{ marginTop: 4 }}>
                「一行代表什么」是所有口径的地基：粒度不对，JOIN 之后金额会被明细行数放大。
                <b>粒度键改了就是重新定义分母</b>，所以只能人来改（改完即事实层）。
              </div>
            </div>
            <a className="btn small" href="#/ontology">
              打开四步解析器 →
            </a>
          </div>

          {entityForm ? (
            <div className="card pad" style={{ marginTop: 14, background: 'var(--paper-2)' }}>
              <div className="eyebrow">Entity</div>
              <div className="form-grid" style={{ marginTop: 10 }}>
                <div>
                  <label className="field">标识</label>
                  <input type="text" value={entityForm.id} readOnly style={{ opacity: 0.7 }} />
                </div>
                <div>
                  <label className="field">物理表</label>
                  <input type="text" value={entityForm.table} readOnly style={{ opacity: 0.7 }} />
                </div>
                <div>
                  <label className="field">显示名</label>
                  <input
                    type="text"
                    value={entityForm.label}
                    placeholder="订单"
                    onChange={(e) => setEntityForm({ ...entityForm, label: e.target.value })}
                  />
                </div>
                <div style={{ gridColumn: 'span 3' }}>
                  <label className="field">粒度描述（一句话说清一行是什么）</label>
                  <input
                    type="text"
                    value={entityForm.grain_description}
                    placeholder="一行 = 一个订单"
                    onChange={(e) =>
                      setEntityForm({ ...entityForm, grain_description: e.target.value })
                    }
                  />
                </div>
                <div style={{ gridColumn: 'span 3' }}>
                  <label className="field">粒度键（逗号分隔；改了等于换分母）</label>
                  <input
                    type="text"
                    value={entityForm.grain_key}
                    placeholder="id"
                    onChange={(e) => setEntityForm({ ...entityForm, grain_key: e.target.value })}
                  />
                </div>
              </div>
              <div className="form-actions">
                <button
                  className="btn primary"
                  disabled={busy}
                  onClick={() =>
                    run(`实体 ${entityForm.id} 已保存`, async () => {
                      const keys = splitList(entityForm.grain_key);
                      const res = await patchEntity(dsn, {
                        id: entityForm.id,
                        label: entityForm.label.trim(),
                        grain_description: entityForm.grain_description.trim(),
                        grain_key: keys.length ? keys : undefined,
                      });
                      setModel(res.model);
                      setEntityForm(null);
                    })
                  }
                >
                  保存
                </button>
                <button className="btn" onClick={() => setEntityForm(null)}>
                  取消
                </button>
                <span className="hint">粒度键必须是这一行的唯一标识（主键，或多列联合键）。</span>
              </div>
            </div>
          ) : null}

          <table className="tbl" style={{ marginTop: 14 }}>
            <thead>
              <tr>
                <th style={{ width: '24%' }}>实体 / 表</th>
                <th style={{ width: '34%' }}>粒度</th>
                <th style={{ width: '16%' }}>主键</th>
                <th style={{ width: '10%' }}>状态</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {entities.map((e) => (
                <tr key={e.id}>
                  <td>
                    <b>{e.label || e.id}</b>
                    <div className="tiny">
                      {e.id} · {e.table}
                    </div>
                  </td>
                  <td className="tiny">{(e.grain?.description as string) || '未记录'}</td>
                  <td className="mono tiny">{((e.grain?.key as string[]) ?? []).join(', ') || '—'}</td>
                  <td>
                    <span className={`badge ${statusClass(e.status)}`}>
                      {STATUS_LABEL[e.status] ?? e.status}
                    </span>
                  </td>
                  <td>
                    <div className="row" style={{ gap: 6 }}>
                      <button
                        className="btn small"
                        onClick={() =>
                          setEntityForm({
                            id: e.id,
                            table: (e.table as string) ?? e.id,
                            label: e.label ?? '',
                            grain_description: (e.grain?.description as string) ?? '',
                            grain_key: ((e.grain?.key as string[]) ?? []).join('、'),
                          })
                        }
                      >
                        编辑
                      </button>
                      {e.status !== 'confirmed' ? (
                        <button
                          className="btn small"
                          disabled={busy}
                          onClick={() =>
                            run(`已核准 ${e.id}`, async () => {
                              const res = await promoteBizItem(dsn, 'entities', e.id);
                              setModel(res.model);
                            })
                          }
                        >
                          核准
                        </button>
                      ) : null}
                    </div>
                  </td>
                </tr>
              ))}
              {entities.length === 0 ? (
                <tr>
                  <td colSpan={5} className="empty">
                    还没有实体
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </section>

        <section className="card pad">
          <h3 style={{ margin: 0, fontSize: 15 }}>关系（决定 JOIN 怎么走）</h3>
          <div className="hint" style={{ marginTop: 4 }}>
            只有 <b>many_to_one</b> / <b>one_to_one</b> 方向不放大行数；沿 <b>one_to_many</b> 走会重复计数，
            规划器会提前给警告。基数写错直接改数，所以只能人来定。
          </div>

          {relForm ? (
            <div className="card pad" style={{ marginTop: 14, background: 'var(--paper-2)' }}>
              <div className="eyebrow">Relationship</div>
              <div className="form-grid" style={{ marginTop: 10 }}>
                <div style={{ gridColumn: 'span 2' }}>
                  <label className="field">从</label>
                  <input
                    type="text"
                    readOnly
                    style={{ opacity: 0.7 }}
                    value={`${relForm.fromEntity}.${relForm.fromKey}`}
                  />
                </div>
                <div style={{ gridColumn: 'span 2' }}>
                  <label className="field">到</label>
                  <input
                    type="text"
                    readOnly
                    style={{ opacity: 0.7 }}
                    value={`${relForm.toEntity}.${relForm.toKey}`}
                  />
                </div>
                <div style={{ gridColumn: 'span 2' }}>
                  <label className="field">基数</label>
                  <select
                    value={relForm.cardinality}
                    onChange={(e) => setRelForm({ ...relForm, cardinality: e.target.value })}
                  >
                    {CARDINALITIES.map(([v, label]) => (
                      <option key={v} value={v}>
                        {label}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
              <div className="form-actions">
                <button
                  className="btn primary"
                  disabled={busy}
                  onClick={() =>
                    run('关系已保存（已核准）', async () => {
                      const res = await patchRelationship(dsn, {
                        from: { entity: relForm.fromEntity, key: splitList(relForm.fromKey) },
                        to: { entity: relForm.toEntity, key: splitList(relForm.toKey) },
                        cardinality: relForm.cardinality,
                      });
                      setModel(res.model);
                      setRelForm(null);
                    })
                  }
                >
                  保存
                </button>
                <button className="btn" onClick={() => setRelForm(null)}>
                  取消
                </button>
              </div>
            </div>
          ) : null}

          <table className="tbl" style={{ marginTop: 14 }}>
            <thead>
              <tr>
                <th style={{ width: '38%' }}>从</th>
                <th style={{ width: '14%' }}>基数</th>
                <th style={{ width: '28%' }}>到</th>
                <th style={{ width: '10%' }}>状态</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {rels.map((r, i) => (
                <tr key={r.id ?? String(i)}>
                  <td className="mono tiny">
                    {r.from?.entity}.{((r.from?.key as string[]) ?? []).join(',')}
                  </td>
                  <td>
                    <span className={`pill ${r.cardinality === 'many_to_one' ? 'ok' : 'warn'}`}>
                      {r.cardinality ?? '—'}
                    </span>
                  </td>
                  <td className="mono tiny">
                    {r.to?.entity}.{((r.to?.key as string[]) ?? []).join(',')}
                  </td>
                  <td>
                    <span className={`badge ${statusClass(r.status)}`}>
                      {STATUS_LABEL[r.status] ?? r.status}
                    </span>
                  </td>
                  <td>
                    <button
                      className="btn small"
                      onClick={() =>
                        setRelForm({
                          fromEntity: r.from?.entity ?? '',
                          fromKey: ((r.from?.key as string[]) ?? []).join('、'),
                          toEntity: r.to?.entity ?? '',
                          toKey: ((r.to?.key as string[]) ?? []).join('、'),
                          cardinality: r.cardinality ?? 'many_to_one',
                        })
                      }
                    >
                      编辑
                    </button>
                  </td>
                </tr>
              ))}
              {rels.length === 0 ? (
                <tr>
                  <td colSpan={5} className="empty">
                    没有声明外键，也没有可推断的 <code>*_id</code> 命中 —— 单表查询不受影响
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </section>

        <section className="card pad">
          <h3 style={{ margin: 0, fontSize: 15 }}>待确认问题</h3>
          <div className="hint" style={{ marginTop: 4 }}>
            发现问题比给出答案值钱：这些是发现器没把握的地方，逐条确认后会变成资产。
          </div>

          <div className="row" style={{ gap: 8, marginTop: 12 }}>
            <input
              type="text"
              className="grow"
              value={questionText}
              placeholder="人发现的问题，例如：退款要不要冲减销售额？"
              onChange={(e) => setQuestionText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && questionText.trim()) {
                  run('已记下这个问题', async () => {
                    const res = await askQuestion(dsn, {
                      action: 'add',
                      question: questionText.trim(),
                      source: '人',
                    });
                    setModel(res.model);
                    setQuestionText('');
                  });
                }
              }}
            />
            <button
              className="btn"
              disabled={busy || !questionText.trim()}
              onClick={() =>
                run('已记下这个问题', async () => {
                  const res = await askQuestion(dsn, {
                    action: 'add',
                    question: questionText.trim(),
                    source: '人',
                  });
                  setModel(res.model);
                  setQuestionText('');
                })
              }
            >
              ＋ 记一条
            </button>
          </div>

          <div className="list" style={{ marginTop: 12 }}>
            {questions.map((item, i) => (
              <div key={`${i}-${item.question}`} className="list-row">
                <span className="pill warn">{i + 1}</span>
                <div className="grow">
                  <div className="t">{item.question}</div>
                  <div className="s">
                    {item.source} {item.evidence?.length ? `· ${item.evidence.join('、')}` : ''}
                  </div>
                </div>
                <button
                  className="btn small"
                  disabled={busy}
                  onClick={() =>
                    run('已标记为处理过', async () => {
                      const res = await askQuestion(dsn, { action: 'dismiss', index: i });
                      setModel(res.model);
                    })
                  }
                >
                  已处理
                </button>
              </div>
            ))}
            {questions.length === 0 ? <div className="empty">没有待确认问题</div> : null}
          </div>
        </section>
      </div>
    );
  };

  // ---------------------------------------------------------------- 金样例

  const renderGoldens = () => {
    const byId = new Map((outcomes ?? []).map((o) => [o.id, o]));
    const list = goldens.filter((g) => hit(g.id, g.question, g.expect.note, JSON.stringify(g.query)));
    return (
      <div className="stack">
        <section className="card pad">
          <div className="row" style={{ alignItems: 'flex-start' }}>
            <div className="grow">
              <h3 style={{ margin: 0, fontSize: 15 }}>问答回归测试集</h3>
              <div className="hint" style={{ marginTop: 4 }}>
                一条 = 问句 + 期望的 IR + 期望值；评测时真跑 SQL 再逐条比。
                模型每次重发现都会改动资产，靠它抓口径漂移。
              </div>
            </div>
            <div className="row" style={{ gap: 8 }}>
              <button
                className="btn small"
                disabled={busy}
                onClick={() => {
                  setGoldenForm(emptyGolden(timeFields[0]?.id ?? ''));
                  setTrial(null);
                }}
              >
                ＋ 新建样例
              </button>
              <button
                className="btn small primary"
                disabled={busy || goldens.length === 0}
                onClick={() =>
                  run('全量评测完成', async () => {
                    const res = await runGoldens(dsn);
                    setOutcomes(res.outcomes);
                    setSummary(res.summary);
                    await refreshGoldens();
                  })
                }
              >
                ▶ 运行全量评测
              </button>
            </div>
          </div>

          {summary ? (
            <div className="kpis four" style={{ marginTop: 16, marginBottom: 4 }}>
              <div className="kpi">
                <div className="lab">用例</div>
                <div className="num">{summary.total ?? 0}</div>
              </div>
              <div className="kpi">
                <div className="lab">通过</div>
                <div className="num" style={{ color: 'var(--ok)' }}>
                  {summary.passed ?? 0}
                </div>
              </div>
              <div className="kpi">
                <div className="lab">不通过</div>
                <div
                  className="num"
                  style={{ color: (summary.failed ?? 0) > 0 ? 'var(--err)' : 'var(--ink-3)' }}
                >
                  {summary.failed ?? 0}
                </div>
              </div>
              <div className="kpi">
                <div className="lab">平均耗时</div>
                <div className="num">{summary.avg_ms ?? 0} ms</div>
              </div>
            </div>
          ) : null}

          <div className="list" style={{ marginTop: 14 }}>
            {list.map((g) => {
              const outcome = byId.get(g.id);
              const last = g.last_run;
              const passed = outcome ? outcome.passed : last ? last.passed : null;
              return (
                <div key={g.id} className="list-row" style={{ alignItems: 'flex-start' }}>
                  <span className="pill">{g.id}</span>
                  <div className="grow">
                    <div className="t">「{g.question}」</div>
                    <div className="s">
                      指标{' '}
                      {(g.query.metrics as Array<{ ref: string }> | undefined)?.map((m) => m.ref).join('、') ||
                        '—'}
                      {' · '}维度{' '}
                      {(g.query.dimensions as Array<{ ref: string }> | undefined)
                        ?.map((d) => d.ref)
                        .join('、') || '无'}
                      {' · '}期望{' '}
                      {[
                        g.expect.rows != null ? `${g.expect.rows} 行` : '',
                        g.expect.first_cell ? `首格 ${g.expect.first_cell}` : '',
                        g.expect.columns?.length ? `${g.expect.columns.length} 列` : '',
                      ]
                        .filter(Boolean)
                        .join(' / ') || '（未设）'}
                    </div>
                    {outcome ? (
                      <div
                        className="s"
                        style={{ color: outcome.passed ? 'var(--ok)' : 'var(--err)' }}
                      >
                        {outcome.passed ? '✓ ' : '✗ '}
                        {outcome.detail}
                        {outcome.sql ? (
                          <details>
                            <summary className="tiny">看这条的 SQL</summary>
                            <pre className="sqlbox">{outcome.sql}</pre>
                          </details>
                        ) : null}
                      </div>
                    ) : last ? (
                      <div className="s">
                        上次 {fmtTime(last.at)} · {last.passed ? '通过' : '不通过'} · {last.detail}
                      </div>
                    ) : (
                      <div className="s">还没评测过</div>
                    )}
                  </div>
                  <span
                    className={`badge ${passed === null ? 'draft' : passed ? 'ran' : 'failed'}`}
                  >
                    {passed === null ? '未评测' : passed ? '通过' : '不通过'}
                  </span>
                  <button
                    className="btn small"
                    onClick={() => {
                      setGoldenForm(formOfGolden(g, timeFields[0]?.id ?? ''));
                      setTrial(null);
                    }}
                  >
                    编辑
                  </button>
                  <button
                    className="btn small"
                    disabled={busy}
                    onClick={() =>
                      run(`评测 ${g.id} 完成`, async () => {
                        const res = await runGoldens(dsn, g.id);
                        setOutcomes((prev) => [
                          ...(prev ?? []).filter((o) => o.id !== g.id),
                          ...res.outcomes,
                        ]);
                        setSummary(res.summary);
                        await refreshGoldens();
                      })
                    }
                  >
                    评测
                  </button>
                  <button
                    className="btn small"
                    disabled={busy}
                    onClick={() =>
                      run(`已删除 ${g.id}`, async () => {
                        await deleteGolden(dsn, g.id);
                        await refreshGoldens();
                      })
                    }
                  >
                    删除
                  </button>
                </div>
              );
            })}
            {list.length === 0 ? (
              <div className="empty">
                {goldens.length
                  ? '没有匹配的样例'
                  : '还没有样例。新建一条：一句话 + 指标 + 维度 + 期望行数'}
              </div>
            ) : null}
          </div>
        </section>

        {goldenForm ? (
          <section className="card pad">
            <div className="eyebrow">
              Golden sample{goldenForm.id ? ` · 编辑 ${goldenForm.id}` : ' · 新建'}
            </div>
            <div className="form-grid" style={{ marginTop: 10 }}>
              <div style={{ gridColumn: 'span 3' }}>
                <label className="field">业务问句（写清口径）</label>
                <input
                  type="text"
                  value={goldenForm.question}
                  placeholder="各大区 8-9 月的销售额"
                  onChange={(e) => setGoldenForm({ ...goldenForm, question: e.target.value })}
                />
              </div>
              <div>
                <label className="field">指标（单选）</label>
                <select
                  value={goldenForm.metrics[0] ?? ''}
                  onChange={(e) =>
                    setGoldenForm({ ...goldenForm, metrics: e.target.value ? [e.target.value] : [] })
                  }
                >
                  <option value="">选择指标</option>
                  {(model?.metrics ?? []).map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.label || m.id}
                    </option>
                  ))}
                </select>
              </div>
              <div style={{ gridColumn: 'span 2' }}>
                <label className="field">维度（可多选；日期维度可加粒度）</label>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
                  {(model?.dimensions ?? []).map((d) => {
                    const on = goldenForm.dims.includes(d.id);
                    return (
                      <button
                        key={d.id}
                        className={`tog ${on ? 'on' : ''}`}
                        onClick={() =>
                          setGoldenForm({
                            ...goldenForm,
                            dims: on
                              ? goldenForm.dims.filter((x) => x !== d.id)
                              : [...goldenForm.dims, d.id],
                          })
                        }
                      >
                        {d.label || d.id}
                      </button>
                    );
                  })}
                </div>
              </div>
              <div>
                <label className="field">时间字段</label>
                <select
                  value={goldenForm.timeField}
                  onChange={(e) => setGoldenForm({ ...goldenForm, timeField: e.target.value })}
                >
                  <option value="">不限定</option>
                  {timeFields.map((d) => (
                    <option key={d.id} value={d.id}>
                      {d.label || d.id}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="field">时间粒度</label>
                <select
                  value={goldenForm.timeGrain}
                  onChange={(e) => setGoldenForm({ ...goldenForm, timeGrain: e.target.value })}
                >
                  {GRAINS.map(([v, label]) => (
                    <option key={v} value={v}>
                      {label}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="field">时间窗</label>
                <select
                  value={goldenForm.preset}
                  onChange={(e) => setGoldenForm({ ...goldenForm, preset: e.target.value })}
                >
                  {PRESETS.map(([v, label]) => (
                    <option key={v} value={v}>
                      {label}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="field">起始（不选预设时）</label>
                <input
                  type="date"
                  value={goldenForm.start}
                  onChange={(e) => setGoldenForm({ ...goldenForm, start: e.target.value, preset: '' })}
                />
              </div>
              <div>
                <label className="field">结束（不含）</label>
                <input
                  type="date"
                  value={goldenForm.end}
                  onChange={(e) => setGoldenForm({ ...goldenForm, end: e.target.value, preset: '' })}
                />
              </div>
              <div />
              <div>
                <label className="field">期望行数</label>
                <input
                  type="number"
                  value={goldenForm.rows}
                  placeholder="2"
                  onChange={(e) => setGoldenForm({ ...goldenForm, rows: e.target.value })}
                />
              </div>
              <div>
                <label className="field">期望首行首列</label>
                <input
                  type="text"
                  value={goldenForm.firstCell}
                  placeholder="华东"
                  onChange={(e) => setGoldenForm({ ...goldenForm, firstCell: e.target.value })}
                />
              </div>
              <div>
                <label className="field">期望列名（逗号分隔）</label>
                <input
                  type="text"
                  value={goldenForm.columns}
                  placeholder="region、amount"
                  onChange={(e) => setGoldenForm({ ...goldenForm, columns: e.target.value })}
                />
              </div>
              <div>
                <label className="field">备注</label>
                <input
                  type="text"
                  value={goldenForm.note}
                  placeholder="口径来源 / 谁确认的"
                  onChange={(e) => setGoldenForm({ ...goldenForm, note: e.target.value })}
                />
              </div>
            </div>

            <div className="form-actions">
              <button
                className="btn primary"
                disabled={busy}
                onClick={() =>
                  run('已保存金样例', async () => {
                    await upsertGolden(dsn, {
                      id: goldenForm.id,
                      question: goldenForm.question.trim(),
                      query: buildQuery(goldenForm),
                      expect: expectOf(goldenForm),
                    });
                    await refreshGoldens();
                    setGoldenForm(null);
                    setTrial(null);
                  })
                }
              >
                保存这条
              </button>
              <button
                className="btn"
                disabled={busy}
                onClick={() =>
                  run('试跑完成', async () => {
                    const res = await runQuery(dsn, buildQuery(goldenForm));
                    setTrial({
                      sql: res.compiled.sql,
                      columns: res.columns,
                      rows: res.rows,
                      count: res.row_count,
                    });
                    if (goldenForm.rows === '') {
                      setGoldenForm((f) => (f ? { ...f, rows: String(res.row_count) } : f));
                    }
                  })
                }
              >
                试跑一次
              </button>
              <button
                className="btn"
                disabled={busy}
                onClick={() =>
                  run('编译完成（未执行）', async () => {
                    const res = await compileQuery(dsn, buildQuery(goldenForm));
                    setTrial({ sql: res.compiled.sql, columns: [], rows: [], count: 0 });
                  })
                }
              >
                只看 SQL
              </button>
              <button
                className="btn"
                onClick={() => {
                  setGoldenForm(null);
                  setTrial(null);
                }}
              >
                取消
              </button>
              <span className="hint">
                期望值至少要有一项：行数 / 首行首列 / 列名。没有期望的样例不算通过。
              </span>
            </div>

            {trial ? (
              <div style={{ marginTop: 16 }}>
                <pre className="sqlbox">{trial.sql}</pre>
                {trial.columns.length ? (
                  <div className="preview-wrap" style={{ marginTop: 10 }}>
                    <table className="tbl dense compact">
                      <thead>
                        <tr>
                          {trial.columns.map((c) => (
                            <th key={c}>{c}</th>
                          ))}
                        </tr>
                      </thead>
                      <tbody>
                        {trial.rows.slice(0, 8).map((row, i) => (
                          <tr key={String(i)}>
                            {row.map((cell, j) => (
                              <td key={String(j)} className={typeof cell === 'number' ? 'num' : undefined}>
                                {cell == null ? '' : String(cell)}
                              </td>
                            ))}
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                ) : null}
                <div className="tiny" style={{ marginTop: 6 }}>
                  共 {trial.count} 行{trial.count > 8 ? '（只显示前 8 行）' : ''}
                </div>
              </div>
            ) : null}
          </section>
        ) : null}
      </div>
    );
  };

  const tabs: Array<[Tab, string, number]> = [
    ['ontology', '本体与关系', counts.entities + counts.relationships],
    ['metrics', '指标口径库', counts.metrics],
    ['dimensions', '维度与字典', counts.dimensions],
    ['rules', '业务规则', counts.rules],
    ['aliases', '同义词映射', counts.aliases],
    ['goldens', '问答回归', counts.goldens],
  ];

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Semantic layer</div>
          <h1>业务知识</h1>
          <div className="sub">
            从库里发现、由人核准的口径资产：实体与关系、指标、维度、规则、同义词、回归样例
          </div>
        </div>
      </div>

      <div className="card pad">
        <div className="row" style={{ flexWrap: 'wrap', gap: 10 }}>
          <select
            value={dsn}
            onChange={(e) => switchDsn(e.target.value)}
            style={{ flex: '0 0 300px' }}
          >
            {sources.map((s) => (
              <option key={s.id} value={s.id}>
                {s.label || s.id} · {s.driver}
              </option>
            ))}
            {sources.length === 0 ? <option value="">（还没有数据源）</option> : null}
          </select>
          <label className="row tiny" style={{ gap: 6 }}>
            <input
              type="checkbox"
              checked={sample}
              onChange={(e) => setSample(e.target.checked)}
              style={{ width: 'auto' }}
            />
            抽样值域（关掉更快，代价是枚举维度退化成文本）
          </label>
          <button
            className="btn primary"
            disabled={busy || !dsn}
            onClick={() =>
              run('发现完成（已核准的条目不会被覆盖）', async () => {
                const res = await discoverBizModel(dsn, sample);
                setModel(res.model);
                await refreshGoldens();
              })
            }
          >
            {model ? '重新发现' : '发现业务模型'}
          </button>
          <button
            className="btn"
            disabled={busy || !dsn}
            onClick={() => run('已刷新', async () => loadModel(dsn))}
          >
            刷新
          </button>
          <div className="grow" />
          <div className="tiny" title={db?.path ?? ''}>
            语义库{' '}
            {db?.enabled ? (
              <b style={{ color: 'var(--ok)' }}>已接入</b>
            ) : (
              <b style={{ color: 'var(--ink-3)' }}>未启用</b>
            )}
            {db?.enabled && db.counts
              ? ` · bm_model ${db.counts.bm_model ?? 0} / gl_run ${db.counts.gl_run ?? 0}`
              : ''}
          </div>
          <div className="tiny">
            资产目录：<code className="sql-inline">bizmodels/{dsn || '…'}.json</code>
          </div>
        </div>
        {notice ? <div className="callout-line">{notice}</div> : null}
        {error ? (
          <div className="note risk" style={{ marginTop: 12 }}>
            <b>没成功：</b>
            {error}
          </div>
        ) : null}
      </div>

      <div className="row" style={{ gap: 8, margin: '18px 0 0' }}>
        <input
          type="text"
          value={query}
          placeholder="过滤当前页签：名称 / 标识 / 公式 / 别名 / 值域"
          onChange={(e) => setQuery(e.target.value)}
          style={{ maxWidth: 380 }}
        />
        {q ? (
          <button className="btn small" onClick={() => setQuery('')}>
            清除
          </button>
        ) : null}
        <span className="tiny">
          {q ? `已过滤 · 匹配 ${countsVisible} 条` : '按名称、标识、公式、别名过滤'}
        </span>
      </div>

      <div className="chip-toggles" style={{ margin: '12px 0' }}>
        {tabs.map(([key, label, n]) => (
          <button key={key} className={`tog ${tab === key ? 'on' : ''}`} onClick={() => setTab(key)}>
            {label}
            <span className="tiny" style={{ marginLeft: 6, opacity: 0.7 }}>
              {n}
            </span>
          </button>
        ))}
      </div>

      {loading ? (
        <div className="card pad">
          <span className="spin" /> 读取业务模型…
        </div>
      ) : model ? (
        <>
          {tab === 'ontology' ? renderOntology() : null}
          {tab === 'metrics' ? renderMetrics() : null}
          {tab === 'dimensions' ? renderDimensions() : null}
          {tab === 'rules' ? renderRules() : null}
          {tab === 'aliases' ? renderAliases() : null}
          {tab === 'goldens' ? renderGoldens() : null}
        </>
      ) : (
        <div className="card pad">
          <b>{dsn || '这个数据源'} 还没有业务模型</b>
          <div className="hint" style={{ marginTop: 6 }}>
            点上面的「发现业务模型」：服务端会连库读列类型、备注、主键外键与值域，产出实体/关系/指标/维度，
            并把不确定的地方列成待确认问题。
          </div>
        </div>
      )}
    </>
  );
};
