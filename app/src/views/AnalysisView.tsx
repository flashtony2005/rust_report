import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ApiError,
  getAnalysisOptions,
  listDatasources,
  runAnalysis,
  type AnalysisDoc,
  type AnalysisOptions,
  type Datasource,
} from '../services/api';

/**
 * 归因分析（`#/analysis`）—— 把「为什么降了」拆成几步**受控查询**。
 *
 * 页面口径（与 `rpx-analysis/v1` 一致，改一处要改两处）：
 * - 每步都是正规查询（IR → 计划 → 编译 → 执行），**页面上没有 SQL 输入框**；
 * - 结论只分 `observed`（看到的变化）与 `computed`（算出来的贡献），**没有"因果"**；
 * - 占比的分母是**上一层变化量**：有成员在涨时占比之和会超过 100%，页面要如实显示；
 * - 预算（步数/行数/top_k）是硬顶，触顶时 `caveats` 里会说；
 * - 拆不动就说为什么（增长 / 无维度 / 空窗口 / 查询失败），不装懂。
 */

const GRAINS: Array<[string, string]> = [
  ['day', '按天'],
  ['week', '按周'],
  ['month', '按月'],
  ['quarter', '按季'],
  ['year', '按年'],
];

const WINDOWS: Array<[string, string]> = [
  ['last_complete', '上一个完整周期'],
  ['current', '当期（未走完）'],
  ['custom', '自定义区间'],
];

const fmt = (v: number | null | undefined, digits = 0) => {
  if (v == null || Number.isNaN(v)) return '—';
  return v.toLocaleString('zh-CN', { minimumFractionDigits: digits, maximumFractionDigits: digits });
};

const pct = (v: number | null | undefined) => {
  if (v == null || Number.isNaN(v)) return '—';
  return `${v >= 0 ? '+' : ''}${(v * 100).toFixed(1)}%`;
};

const statusLabel: Record<string, string> = {
  ok: '拆解完成',
  partial: '部分结论',
  failed: '查不出来',
};

export const AnalysisView: React.FC = () => {
  const [sources, setSources] = useState<Datasource[]>([]);
  const [dsn, setDsn] = useState('');
  const [options, setOptions] = useState<AnalysisOptions | null>(null);
  const [metric, setMetric] = useState('');
  const [timeField, setTimeField] = useState('');
  const [grain, setGrain] = useState('month');
  const [dim1, setDim1] = useState('');
  const [dim2, setDim2] = useState('');
  const [window, setWindow] = useState('last_complete');
  const [start, setStart] = useState('');
  const [end, setEnd] = useState('');
  const [maxSteps, setMaxSteps] = useState(5);
  const [maxRows, setMaxRows] = useState(200);
  const [topK, setTopK] = useState(3);

  const [doc, setDoc] = useState<AnalysisDoc | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [openSteps, setOpenSteps] = useState<Record<string, boolean>>({});

  const loadOptions = useCallback(async (target: string) => {
    if (!target) return;
    try {
      const opts = await getAnalysisOptions(target);
      setOptions(opts);
      // 换数据源时把参数重置成模型给的缺省值（别把上一个库的维度带过来）
      setMetric(opts.defaults.metric);
      setTimeField(opts.defaults.time_field);
      setGrain(opts.defaults.grain || 'month');
      setDim1(opts.defaults.dimensions[0] ?? '');
      setDim2(opts.defaults.dimensions[1] ?? '');
      setError(null);
    } catch (e) {
      setOptions(null);
      setError(errText(e));
    }
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const page = await listDatasources();
        setSources(page.items ?? []);
        if (page.items?.length) {
          setDsn(page.items[0].id);
        }
      } catch (e) {
        setError(errText(e));
      }
    })();
  }, []);

  useEffect(() => {
    if (dsn) void loadOptions(dsn);
  }, [dsn, loadOptions]);

  const textDims = options?.text_dimensions ?? [];
  const dateDims = options?.date_dimensions ?? [];
  const dim2Choices = useMemo(
    () => textDims.filter((d) => d.id !== dim1),
    [textDims, dim1],
  );

  const go = async () => {
    if (!dsn) return;
    setBusy(true);
    setError(null);
    try {
      const res = await runAnalysis({
        dsn,
        metric: metric || undefined,
        time_field: timeField || undefined,
        grain: window === 'custom' ? grain : grain,
        dimensions: [dim1, dim2].filter(Boolean),
        window: window === 'custom' ? undefined : window,
        time_range:
          window === 'custom' && start && end ? { start, end } : undefined,
        budget: { max_steps: maxSteps, max_rows: maxRows, top_k: topK },
      });
      setDoc(res.document);
      // 默认把失败的那一步展开，其余收起（细节按需看）
      const open: Record<string, boolean> = {};
      for (const s of res.document.steps) open[s.id] = !s.ok;
      setOpenSteps(open);
    } catch (e) {
      setError(errText(e));
      setDoc(null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Analysis</div>
          <h1>归因分析</h1>
          <div className="sub">
            整体 → 按维度 → 再钻一层：每一步都是受控查询（只收 IR，没有 SQL 入口），
            结论只分「看到的」与「算出来的」
          </div>
        </div>
        <button className="btn primary" disabled={busy || !dsn} onClick={go}>
          {busy ? '拆解中…' : '开始拆解'}
        </button>
      </div>

      <section className="card pad" style={{ marginBottom: 18 }}>
        <div className="eyebrow">输入</div>
        <div className="form-grid" style={{ marginTop: 10 }}>
          <div>
            <label className="field">数据源</label>
            <select aria-label="数据源" value={dsn} onChange={(e) => setDsn(e.target.value)}>
              {sources.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.label || s.id}
                </option>
              ))}
              {sources.length === 0 ? <option value="">（先到「数据源」页登记一条）</option> : null}
            </select>
          </div>
          <div>
            <label className="field">指标</label>
            <select aria-label="指标" value={metric} onChange={(e) => setMetric(e.target.value)}>
              {(options?.metrics ?? []).map((m) => (
                <option key={m.id} value={m.id}>
                  {m.label || m.id}
                  {m.status === 'draft' ? '（草稿）' : ''}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="field">时间字段</label>
            <select aria-label="时间字段" value={timeField} onChange={(e) => setTimeField(e.target.value)}>
              {dateDims.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.label || d.id}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="field">粒度</label>
            <select aria-label="粒度" value={grain} onChange={(e) => setGrain(e.target.value)}>
              {GRAINS.map(([v, l]) => (
                <option key={v} value={v}>
                  {l}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="field">第一层维度</label>
            <select aria-label="第一层维度" value={dim1} onChange={(e) => setDim1(e.target.value)}>
              <option value="">（不拆）</option>
              {textDims.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.label || d.id}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="field">第二层维度（下钻）</label>
            <select aria-label="第二层维度" value={dim2} onChange={(e) => setDim2(e.target.value)}>
              <option value="">（不下钻）</option>
              {dim2Choices.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.label || d.id}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="field">窗口</label>
            <select
              aria-label="窗口"
              value={window}
              onChange={(e) => {
                setWindow(e.target.value);
                if (e.target.value === 'custom' && doc?.windows) {
                  setStart(doc.windows.cur_start);
                  setEnd(doc.windows.cur_end);
                }
              }}
            >
              {WINDOWS.map(([v, l]) => (
                <option key={v} value={v}>
                  {l}
                </option>
              ))}
            </select>
          </div>
          {window === 'custom' ? (
            <>
              <div>
                <label className="field">本期开始</label>
                <input aria-label="本期开始" type="date" value={start} onChange={(e) => setStart(e.target.value)} />
              </div>
              <div>
                <label className="field">本期结束（不含）</label>
                <input aria-label="本期结束" type="date" value={end} onChange={(e) => setEnd(e.target.value)} />
              </div>
            </>
          ) : null}
          <div style={{ gridColumn: 'span 3' }}>
            <label className="field">
              预算 · 步数 / 单步行数 / 每层成员数（默认 5 / 200 / 3，硬顶 8 / 500 / 5）
            </label>
            <div className="row" style={{ gap: 8, maxWidth: 360 }}>
              <input
                type="number"
                aria-label="最大步数"
                min={1}
                max={8}
                value={maxSteps}
                onChange={(e) => setMaxSteps(Number(e.target.value))}
                title="默认 5 步，硬顶 8 步"
              />
              <input
                type="number"
                aria-label="单步行数"
                min={1}
                max={500}
                value={maxRows}
                onChange={(e) => setMaxRows(Number(e.target.value))}
                title="默认 200 行，硬顶 500 行"
              />
              <input
                type="number"
                aria-label="每层成员数"
                min={1}
                max={5}
                value={topK}
                onChange={(e) => setTopK(Number(e.target.value))}
                title="默认 3 个成员，硬顶 5 个"
              />
            </div>
          </div>
        </div>
        <div className="hint" style={{ marginTop: 10 }}>
          对比期由编译期平移（与本期等长）；「上一个完整周期」不会拿半个月对半个月。
          预算填超硬顶会被后端钳住，触顶时 <code className="sql-inline">caveats</code> 里会说。
        </div>
      </section>

      {error ? (
        <div className="callout-line" style={{ borderColor: 'var(--err)' }}>
          {error}
        </div>
      ) : null}

      {!doc && !busy ? (
        <section className="card pad">
          <div className="empty">
            选好指标与维度，点右上「开始拆解」。
            <br />
            第一步先看整体变化；只有在<b>下降</b>时才会继续按维度拆，再对降得最多的成员下钻一层。
          </div>
        </section>
      ) : null}

      {doc ? (
        <>
          <div className="row" style={{ alignItems: 'flex-end', marginBottom: 12 }}>
            <div className="grow">
              <div className="eyebrow">结论 · {doc.id}</div>
              <h2 style={{ margin: '4px 0 0', fontSize: 20 }}>{doc.summary}</h2>
            </div>
            <span className={`badge ${doc.status === 'ok' ? 'ran' : doc.status === 'partial' ? 'analyzed' : 'failed'}`}>
              {statusLabel[doc.status] ?? doc.status}
            </span>
          </div>

          {doc.totals ? (
            <div className="kpis four" style={{ marginBottom: 20 }}>
              <div className="kpi">
                <div className="lab">本期</div>
                <div className="num">{fmt(doc.totals.cur, 0)}</div>
                <div className="tiny">
                  {doc.windows.cur_start} ~ {doc.windows.cur_end}
                </div>
              </div>
              <div className="kpi">
                <div className="lab">对比期</div>
                <div className="num">{fmt(doc.totals.prev, 0)}</div>
                <div className="tiny">
                  {doc.windows.prev_start} ~ {doc.windows.prev_end}
                </div>
              </div>
              <div className="kpi">
                <div className="lab">变化</div>
                <div
                  className="num"
                  style={{ color: (doc.totals.delta ?? 0) < 0 ? 'var(--err)' : 'var(--ok)' }}
                >
                  {fmt(doc.totals.delta, 0)}
                </div>
                <div className="tiny">{pct(doc.totals.growth)}</div>
              </div>
              <div className="kpi">
                <div className="lab">覆盖度</div>
                <div className="num">
                  {doc.coverage == null ? '—' : `${(doc.coverage * 100).toFixed(1)}%`}
                </div>
                <div className="tiny">
                  {doc.coverage != null && doc.coverage > 1.001
                    ? '有成员在涨，占比之和超 100%'
                    : '被点名成员占整体降幅'}
                </div>
              </div>
            </div>
          ) : null}

          <section className="card pad" style={{ marginBottom: 18 }}>
            <div className="eyebrow">结论清单</div>
            <div className="list" style={{ marginTop: 10 }}>
              {doc.findings.map((f, i) => (
                <div key={`${i}-${f.what}`} className="list-row">
                  <span className={`pill ${f.kind === 'observed' ? '' : 'ok'}`}>
                    {f.kind === 'observed' ? '看到' : '算出'}
                  </span>
                  <div className="grow">
                    <div className="t">{f.what}</div>
                    <div className="s">
                      依据 {f.evidence.join('、')}
                      {f.share != null ? ` · 占比 ${(f.share * 100).toFixed(1)}%` : ''}
                    </div>
                  </div>
                </div>
              ))}
              {doc.findings.length === 0 ? <div className="empty">没有结论</div> : null}
            </div>
          </section>

          <section className="card pad" style={{ marginBottom: 18 }}>
            <div className="eyebrow">拆解步骤（{doc.steps.length} 步 · 每步都可复现）</div>
            <div style={{ marginTop: 10 }}>
              {doc.steps.map((s) => {
                const open = openSteps[s.id] ?? false;
                return (
                  <div key={s.id} className="card" style={{ padding: 0, marginBottom: 10 }}>
                    <button
                      className="row"
                      onClick={() => setOpenSteps((m) => ({ ...m, [s.id]: !open }))}
                      style={{
                        width: '100%',
                        background: 'transparent',
                        border: 0,
                        cursor: 'pointer',
                        padding: '10px 14px',
                        textAlign: 'left',
                        gap: 10,
                      }}
                    >
                      <span className="mono tiny">{s.id}</span>
                      <span className="pill">{s.kind}</span>
                      <span className="grow">{s.title}</span>
                      <span className="tiny">
                        {s.row_count} 行 {s.ok ? '' : '· 失败'}
                      </span>
                      <span className="tiny">{open ? '收起' : '展开'}</span>
                    </button>
                    {open ? (
                      <div style={{ padding: '0 14px 14px' }}>
                        {s.error ? (
                          <div className="callout-line" style={{ borderColor: 'var(--err)' }}>
                            {s.error.code}：{s.error.message}
                          </div>
                        ) : null}
                        {s.sql ? <pre className="sqlbox">{s.sql}</pre> : null}
                        {s.columns.length && s.rows.length ? (
                          <div className="preview-wrap">
                            <table className="tbl dense compact">
                              <thead>
                                <tr>
                                  {s.columns.map((c) => (
                                    <th key={c}>{c}</th>
                                  ))}
                                </tr>
                              </thead>
                              <tbody>
                                {s.rows.slice(0, 8).map((row, i) => (
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
                  </div>
                );
              })}
            </div>
          </section>

          {doc.caveats.length ? (
            <section className="card pad">
              <div className="eyebrow">必须说明的边界</div>
              <ul className="hint" style={{ margin: '10px 0 0', paddingLeft: 18 }}>
                {doc.caveats.map((c) => (
                  <li key={c} style={{ marginBottom: 6 }}>
                    {c}
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
        </>
      ) : null}
    </>
  );
};

function errText(e: unknown): string {
  if (e instanceof ApiError) return e.message;
  return e instanceof Error ? e.message : String(e);
}
