import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { ONTOLOGY } from '../data/ontology';
import { ONTOLOGY_SUMMARY, resolve, toOntologyTrace } from '../services/ontology';
import {
  ApiError,
  fetchOntology,
  getOntology,
  listDatasources,
  listOntologies,
  saveOntology,
  type Datasource,
  type OntoGraph,
  type OntologyGraphReply,
  type OntologyListItem,
  type OntologyTrace,
} from '../services/api';

/**
 * 本体大脑 · 解析器 —— 四步把一句话拆成安全 SQL。
 *
 * 排版原则：只用仓库自己的 CSS 变量与类（这个工程没有 Tailwind），一步一张卡、
 * 一块只说一件事；步骤条固定在顶部，正文与侧栏按 8px 节奏对齐。
 *
 * 数据来源分两层，页面上分两块讲清楚：
 *   1. **四步推理**（下方四个步骤）—— 仍用 `data/ontology.ts` 的示例夹具驱动，
 *      因为这是"教学核心"：任意模型的行级聚合要等服务端 `/query/run`，这里只演示方法。
 *   2. **真实模型图谱**（顶部卡片）—— 来自后端 `GET /bizmodels/{dsn}/ontology`，
 *      把该数据源真正的实体/关系/口径画出来；没有模型就明说退回示例。
 *   3. **四步痕迹**（保存 / 历史）—— `POST /ontology` 把当前演示结论落库，下次可复用，
 *      对应 `onto_*` 这组"记忆"表。
 */

const PRESETS = [
  { label: '客户等级 × LTV × 逾期率', intent: '按客户等级看 LTV 和逾期率' },
  { label: '大区 × 授信总额', intent: '按所属大区分析授信总额与逾期率' },
];

const STEPS = [
  { no: 1, title: '识别', sub: '命中的词 → 表与字段' },
  { no: 2, title: '定主表', sub: '谁在中心，谁在分支' },
  { no: 3, title: '分头聚合', sub: '防扇出：先收敛再连' },
  { no: 4, title: '组装输出', sub: 'SQL / Polars / 预览' },
] as const;

type StepNo = 1 | 2 | 3 | 4;

export const OntologyView: React.FC = () => {
  const [text, setText] = useState(PRESETS[0].intent);
  const [intent, setIntent] = useState(PRESETS[0].intent);
  const [step, setStep] = useState<StepNo>(1);
  const [tab, setTab] = useState<'sql' | 'polars'>('sql');

  // —— 真实模型图谱 + 数据源选择 ——
  const [sources, setSources] = useState<Datasource[]>([]);
  const [dsn, setDsn] = useState('');
  const [graph, setGraph] = useState<OntoGraph | null>(null);
  const [hasModel, setHasModel] = useState(false);
  const [graphState, setGraphState] = useState<'loading' | 'ok' | 'none' | 'down'>('loading');
  const [graphMsg, setGraphMsg] = useState<string | null>(null);

  // —— 四步痕迹：保存 + 历史复用 ——
  const [saving, setSaving] = useState(false);
  const [saveMsg, setSaveMsg] = useState<string | null>(null);
  const [saveErr, setSaveErr] = useState(false);
  const [history, setHistory] = useState<OntologyListItem[]>([]);
  const [historyState, setHistoryState] = useState<'idle' | 'ok' | 'down'>('idle');
  const [historyOpen, setHistoryOpen] = useState(false);
  const [trace, setTrace] = useState<OntologyTrace | null>(null);

  const res = useMemo(() => resolve(intent), [intent]);

  const loadHistory = useCallback(async (d: string) => {
    if (!d) return;
    try {
      const r = await listOntologies(d, 20);
      setHistory(r.items);
      if (r.store === 'disabled') {
        setHistoryState('down');
        setGraphMsg('语义库未启用（服务端没开 --semantic-db），保存/读取历史会被拒；图谱读取不受影响');
      } else {
        setHistoryState('ok');
      }
    } catch (e) {
      setHistoryState('down');
      setGraphMsg(e instanceof ApiError ? `${e.code}：${e.message}` : '读取历史失败');
    }
  }, []);

  const loadDsn = useCallback(
    async (v: string) => {
      if (!v) return;
      setGraphState('loading');
      try {
        const r: OntologyGraphReply = await fetchOntology(v);
        if (r.has_model && r.ontology) {
          setGraph(r.ontology);
          setHasModel(true);
          setGraphState('ok');
        } else {
          setGraph(null);
          setHasModel(false);
          setGraphState('none');
        }
      } catch (e) {
        setGraph(null);
        setHasModel(false);
        setGraphState('down');
        setGraphMsg(e instanceof ApiError ? `${e.code}：${e.message}` : '读取模型失败');
      }
      await loadHistory(v);
    },
    [loadHistory],
  );

  useEffect(() => {
    (async () => {
      try {
        const page = await listDatasources();
        const items = page.items ?? [];
        setSources(items);
        const def = items.find((s) => s.id === 'demo-sqlite')?.id ?? items[0]?.id ?? '';
        if (def) {
          setDsn(def);
          await loadDsn(def);
        } else {
          setGraphState('none');
        }
      } catch {
        // 即使数据源拉不到，四步演示照常用示例夹具跑
        setGraphState('none');
      }
    })();
  }, [loadDsn]);

  const switchDsn = (v: string) => {
    setDsn(v);
    setTrace(null);
    void loadDsn(v);
  };

  const submit = (e?: React.FormEvent) => {
    e?.preventDefault();
    const next = text.trim();
    if (!next) return;
    setIntent(next);
    setStep(1);
  };

  const pickPreset = (preset: string) => {
    setText(preset);
    setIntent(preset);
    setStep(1);
  };

  const saveTrace = async () => {
    if (!dsn) {
      setSaveMsg('先选一个数据源再保存');
      setSaveErr(true);
      return;
    }
    setSaving(true);
    setSaveMsg(null);
    setSaveErr(false);
    try {
      const req = toOntologyTrace(dsn, intent, res);
      const r = await saveOntology(req);
      setSaveMsg(`已保存本次演示结论（${r.id}）`);
      await loadHistory(dsn);
    } catch (e) {
      setSaveErr(true);
      setSaveMsg(e instanceof ApiError ? `${e.code}：${e.message}` : '保存失败');
    } finally {
      setSaving(false);
    }
  };

  const openTrace = async (id: string) => {
    try {
      const r = await getOntology(id);
      setTrace(r);
    } catch (e) {
      setTrace(null);
      setSaveMsg(e instanceof ApiError ? `${e.code}：${e.message}` : '读取失败');
      setSaveErr(true);
    }
  };

  const badge = (() => {
    if (graphState === 'down') return { cls: '', text: '后端未连接 · 示例演示' };
    if (graphState === 'ok' && graph) {
      return {
        cls: 'ran',
        text: `真实模型已接入 · ${graph.entities.length} 实体 / ${graph.edges.length} 关系`,
      };
    }
    if (graphState === 'none') return { cls: '', text: '该数据源无本体模型 · 用示例演示' };
    return { cls: '', text: '加载中…' };
  })();

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Ontology</div>
          <h1>本体大脑 · 解析器</h1>
          <div className="sub">一句话 → 四步定表 → 分头聚合 → 安全 SQL；每步可单独核对</div>
        </div>
        <span
          className={`badge ${badge.cls}`}
          title="顶部卡片读真实模型图谱（catalog.model）；四步演示用示例夹具，因为任意模型的行级聚合要等服务端 /query/run"
        >
          {badge.text}
        </span>
      </div>

      {/* 数据源选择 + 保存/历史 */}
      <div className="row" style={{ gap: 10, flexWrap: 'wrap', marginTop: 14 }}>
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
        <button type="button" className="btn small" onClick={saveTrace} disabled={saving}>
          {saving ? '保存中…' : '保存解析结论'}
        </button>
        <button
          type="button"
          className="btn small"
          onClick={() => setHistoryOpen((v) => !v)}
          disabled={historyState === 'down' && history.length === 0}
        >
          {historyOpen ? '收起历史' : '历史解析'}
        </button>
        {saveMsg ? (
          <span className={`tiny ${saveErr ? 'err' : ''}`} style={saveErr ? { color: '#ff8a8a' } : undefined}>
            {saveMsg}
          </span>
        ) : null}
      </div>

      {/* 真实模型图谱 */}
      <section className="card pad" style={{ marginTop: 16 }}>
        <div className="eyebrow">Real Ontology</div>
        <h2 className="step-h">真实模型图谱（来自后端 catalog.model）</h2>
        <p className="step-p">
          这里画的是<b>该数据源真正的</b>实体 / 关系 / 口径，不是示例。没有关系（单表模型）时，
          下方四步演示也能成立，只是没有扇出可看。
        </p>
        {graphState === 'ok' && graph ? (
          <OntologyGraph graph={graph} />
        ) : graphState === 'loading' ? (
          <div className="tiny">加载中…</div>
        ) : graphState === 'down' ? (
          <div className="tiny" style={{ color: '#ff8a8a' }}>{graphMsg}</div>
        ) : (
          <div className="tiny muted">
            该数据源还没有本体模型（或语义库未启用）。下方四步演示用示例夹具跑通推理逻辑。
          </div>
        )}
      </section>

      {/* 历史解析（复用 / 审计） */}
      {historyOpen ? (
        <section className="card pad" style={{ marginTop: 16 }}>
          <div className="eyebrow">History</div>
          <h2 className="step-h">历史解析（人确认过的结论，可复用）</h2>
          {historyState === 'down' && history.length === 0 ? (
            <div className="tiny" style={{ color: '#ff8a8a' }}>{graphMsg}</div>
          ) : history.length === 0 ? (
            <div className="tiny muted">还没有保存过解析结论（点上方「保存解析结论」即可落库）。</div>
          ) : (
            <div className="stack" style={{ gap: 8 }}>
              {history.map((h) => (
                <div key={h.id} className="row" style={{ gap: 10 }}>
                  <button type="button" className="btn small" onClick={() => openTrace(h.id)}>
                    查看
                  </button>
                  <span className="grow" style={{ fontSize: 13 }}>
                    {h.question}
                  </span>
                  <span className="tiny muted">
                    {h.center_entity ?? '—'} · {h.fanout_edges} 放大边 ·{' '}
                    {h.created_at.slice(0, 19).replace('T', ' ')}
                  </span>
                </div>
              ))}
            </div>
          )}
          {trace ? (
            <div className="card pad" style={{ marginTop: 12, background: '#1a2233' }}>
              <div className="rail-head">#{trace.id}</div>
              <div className="tiny">{trace.question} · 中心 {trace.center_entity ?? '—'}</div>
              <pre className="sqlbox" style={{ maxHeight: 240, overflow: 'auto', marginTop: 8 }}>
                {trace.sql ?? '（无 SQL）'}
              </pre>
              <div className="tiny muted" style={{ marginTop: 6 }}>
                命中 {trace.hits.length} 条 · 步骤 {trace.steps.length} 步 · 状态 {trace.status}
              </div>
            </div>
          ) : null}
        </section>
      ) : null}

      <form className="ask" onSubmit={submit}>
        <div className="ask-row">
          <input
            type="text"
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="例：按客户等级看 LTV 和逾期率"
          />
          <button type="submit" className="btn primary">
            解析
          </button>
        </div>
        <div className="ask-foot">
          <span className="tiny">
            规则匹配（维度词 / 度量词 → 表.字段），不做语义猜测。
            当前：<b>{res.hits.filter((h) => h.kind !== 'entity').length}</b> 条命中 ·
            需要 <b>{res.entities.length}</b> 张表 ·
            {res.risk.multiple ? ' 检测到并列放大边' : ' 单放大边'}
          </span>
          <span className="row" style={{ gap: 8 }}>
            {PRESETS.map((p) => (
              <button
                key={p.intent}
                type="button"
                className={`btn small ${intent === p.intent ? 'primary' : ''}`}
                onClick={() => pickPreset(p.intent)}
              >
                {p.label}
              </button>
            ))}
          </span>
        </div>
      </form>

      {/* 步骤条：固定在正文上方，点哪步看哪步 */}
      <div className="stepper" role="tablist" aria-label="解析四步">
        {STEPS.map((s) => (
          <button
            key={s.no}
            type="button"
            role="tab"
            aria-selected={step === s.no}
            className={`step ${step === s.no ? 'on' : ''} ${step > s.no ? 'done' : ''}`}
            onClick={() => setStep(s.no)}
          >
            <span className="no">{step > s.no ? '✓' : s.no}</span>
            <span className="grow" style={{ minWidth: 0 }}>
              <span className="tt">{s.title}</span>
              <span className="ss">{s.sub}</span>
            </span>
          </button>
        ))}
      </div>

      <div className={step <= 2 ? 'split' : ''} style={{ marginTop: 18 }}>
        <div className="stack">
          {step === 1 ? <StepIdentify res={res} /> : null}
          {step === 2 ? <StepCenter res={res} /> : null}
          {step === 3 ? <StepGrain res={res} /> : null}
          {step === 4 ? <StepOutput res={res} tab={tab} setTab={setTab} /> : null}

          <div className="stepnav">
            <button
              type="button"
              className="btn small"
              onClick={() => setStep((s) => (s > 1 ? ((s - 1) as StepNo) : s))}
              disabled={step === 1}
            >
              ← 上一步
            </button>
            <span className="tiny">
              第 {step} / 4 步 · {STEPS[step - 1].title}
            </span>
            <button
              type="button"
              className="btn small primary"
              onClick={() => setStep((s) => (s < 4 ? ((s + 1) as StepNo) : s))}
              disabled={step === 4}
            >
              下一步 →
            </button>
          </div>
        </div>

        {step <= 2 ? (
        <aside className="stack">
          <div className="card pad rail-card">
            <div className="rail-head">本体规模</div>
            <div className="rail-stat">
              <span>实体</span>
              <b>{ONTOLOGY_SUMMARY.entities}</b>
            </div>
            <div className="rail-stat">
              <span>关系</span>
              <b>{ONTOLOGY_SUMMARY.edges}</b>
            </div>
            <div className="rail-stat">
              <span>口径</span>
              <b>{ONTOLOGY_SUMMARY.metrics}</b>
            </div>
            <div className="tiny" style={{ marginTop: 8 }}>
              行数：客户 {ONTOLOGY_SUMMARY.rows.customer} · 订单 {ONTOLOGY_SUMMARY.rows.order} · 贷款{' '}
              {ONTOLOGY_SUMMARY.rows.loan}
            </div>
          </div>

          <div className="card pad rail-card">
            <div className="rail-head">本次用到的表</div>
            {res.entities.map((id) => (
              <div key={id} className="rail-line">
                <code className="sql-inline">{id}</code>
                <span className="tiny">
                  {res.edges.filter((e) => e.to === id).map((e) => `${e.cardinality} 放大`).join(' / ') || '主表'}
                </span>
              </div>
            ))}
            <div className="rail-head" style={{ marginTop: 16 }}>
              口径
            </div>
            {res.metrics.map((id) => {
              const m = ONTOLOGY.metrics.find((x) => x.id === id);
              return (
                <div key={id} className="rail-line">
                  <span style={{ fontSize: 12 }}>{m?.label ?? id}</span>
                  <span className="mono">{id}</span>
                </div>
              );
            })}
          </div>
        </aside>
        ) : null}
      </div>
    </>
  );
};

/* -------------------------------------------------------------------------- */
/* 真实模型图谱：实体为节点、关系为有向边（带基数徽标）                          */
/* -------------------------------------------------------------------------- */

const OntologyGraph: React.FC<{ graph: OntoGraph }> = ({ graph }) => {
  const n = graph.entities.length;
  const W = Math.max(640, n * 210);
  const H = 150;
  const nodeW = 156;
  const nodeH = 60;
  const slot = W / n;
  const pos = graph.entities.map((_, i) => ({
    x: slot * i + (slot - nodeW) / 2,
    y: H / 2 - nodeH / 2,
  }));
  const cy = H / 2;
  const apexY = 16;
  return (
    <svg
      className="onto-graph"
      viewBox={`0 0 ${W} ${H}`}
      width="100%"
      height={H}
      role="img"
      aria-label="本体关系图"
    >
      <defs>
        <marker
          id="onto-arrow"
          markerWidth="9"
          markerHeight="9"
          refX="7"
          refY="3"
          orient="auto"
          markerUnits="userSpaceOnUse"
        >
          <path d="M0,0 L7,3 L0,6 Z" fill="#7c93ff" />
        </marker>
      </defs>
      {graph.edges.map((e, idx) => {
        const fi = graph.entities.findIndex((x) => x.id === e.from);
        const ti = graph.entities.findIndex((x) => x.id === e.to);
        if (fi < 0 || ti < 0) return null;
        const x1 = fi < ti ? pos[fi].x + nodeW : pos[fi].x;
        const x2 = fi < ti ? pos[ti].x : pos[ti].x + nodeW;
        const mx = (x1 + x2) / 2;
        const d = `M ${x1} ${cy} C ${x1} ${apexY + 34}, ${x2} ${apexY + 34}, ${x2} ${cy}`;
        return (
          <g key={`${e.from}-${e.to}-${idx}`}>
            <path d={d} fill="none" stroke="#7c93ff" strokeWidth={1.6} markerEnd="url(#onto-arrow)" />
            <g transform={`translate(${mx}, ${apexY + 4})`}>
              <rect x={-27} y={-11} width={54} height={18} rx={9} fill="#1b2233" stroke="#3a4a66" strokeWidth={1} />
              <text x={0} y={2} textAnchor="middle" fontSize={11} fill="#9fb0c9">
                {e.cardinality}
              </text>
            </g>
          </g>
        );
      })}
      {graph.entities.map((ent, i) => (
        <g key={ent.id} transform={`translate(${pos[i].x}, ${pos[i].y})`}>
          <rect width={nodeW} height={nodeH} rx={10} fill="#222b3d" stroke="#3a4a66" strokeWidth={1.5} />
          <text x={14} y={24} fontSize={13} fontWeight={600} fill="#e6ebf5">
            {ent.id}
          </text>
          <text x={14} y={44} fontSize={12} fill="#9fb0c9">
            {ent.name}
          </text>
        </g>
      ))}
    </svg>
  );
};

/* -------------------------------------------------------------------------- */
/* 第 1 步：识别                                                              */
/* -------------------------------------------------------------------------- */

const StepIdentify: React.FC<{ res: ReturnType<typeof resolve> }> = ({ res }) => (
  <section className="card pad">
    <div className="eyebrow">Step 01 · Identify</div>
    <h2 className="step-h">这句话命中了什么</h2>
    <p className="step-p">
      逐词匹配维度与度量，命中就给出去向：<b>落在哪张表的哪个字段</b>。没命中的部分下面会明说是兜底，
      不假装理解了。
    </p>

    <table className="tbl">
      <thead>
        <tr>
          <th style={{ width: '18%' }}>命中词</th>
          <th style={{ width: '14%' }}>类型</th>
          <th style={{ width: '34%' }}>落点</th>
          <th>依据</th>
        </tr>
      </thead>
      <tbody>
        {res.hits.map((h, i) => (
          <tr key={`${h.kind}-${h.token}-${i}`}>
            <td>
              <span className={`pill ${h.kind}`}>{h.token}</span>
            </td>
            <td className="mono">{h.kind}</td>
            <td className="mono">{h.landing}</td>
            <td className="muted">{h.evidence}</td>
          </tr>
        ))}
      </tbody>
    </table>

    <div className="callout-line">
      结论：需要 {res.entities.length} 张表（{res.entities.join(' · ')}），按「{res.dim === 'level' ? '客户等级' : '所属大区'}」分组
    </div>
  </section>
);

/* -------------------------------------------------------------------------- */
/* 第 2 步：定主表                                                            */
/* -------------------------------------------------------------------------- */

const StepCenter: React.FC<{ res: ReturnType<typeof resolve> }> = ({ res }) => (
  <section className="card pad">
    <div className="eyebrow">Step 02 · Topology</div>
    <h2 className="step-h">谁是中心表</h2>
    <p className="step-p">
      关系图里只有两类边：<b>1:N</b>（放大，行数会变多）与 <b>N:1</b>（不放大）。
      中心表 = 所有放大边的<b>起点</b>，从它出发左连，行数才不会被改变。
    </p>

    <table className="tbl">
      <thead>
        <tr>
          <th style={{ width: '26%' }}>关系</th>
          <th style={{ width: '14%' }}>基数</th>
          <th>连接条件</th>
        </tr>
      </thead>
      <tbody>
        {res.edges.map((e) => (
          <tr key={`${e.from}-${e.to}`}>
            <td className="mono">
              {e.from} → {e.to}
            </td>
            <td>
              <span className={`pill ${e.cardinality === '1:N' ? 'warn' : 'ok'}`}>{e.cardinality}</span>
            </td>
            <td className="mono muted">{e.on}</td>
          </tr>
        ))}
      </tbody>
    </table>

    <div className="center-box">
      <div className="center-top">
        <div>
          <div className="tiny">中心表</div>
          <div className="center-name">{res.center.entity}</div>
        </div>
        <span className={`badge ${res.risk.multiple ? 'analyzed' : 'ran'}`}>
          {res.center.fanOut} 条放大边
        </span>
      </div>
      <div className="muted" style={{ fontSize: 12.5, lineHeight: 1.85 }}>
        {res.center.why}
      </div>
    </div>

    <div className={`note ${res.risk.multiple ? 'risk' : ''}`}>
      {res.risk.notes.map((n) => (
        <div key={n}>· {n}</div>
      ))}
    </div>
  </section>
);

/* -------------------------------------------------------------------------- */
/* 第 3 步：分头聚合（核心）                                                   */
/* -------------------------------------------------------------------------- */

const StepGrain: React.FC<{ res: ReturnType<typeof resolve> }> = ({ res }) => {
  const { naive, grain, inflation, inflationMetric } = res.compare;
  const label = inflationMetric === 'ltv' ? '客户价值' : '授信总额';
  const fmt = (v: number) => `¥${Math.round(v).toLocaleString('zh-CN')}`;
  return (
    <section className="card pad">
      <div className="eyebrow">Step 03 · Grains Aligned</div>
      <h2 className="step-h">不预聚合会怎样，预聚合后又怎样</h2>
      <p className="step-p">
        同一条意图，两种写法。左边是「直接多表 JOIN」，右边是「各自先在 customer_id 收敛再 1:1 连」。
        下面两张表的合计是<b>真算出来的</b>，不是示意。
      </p>

      <div className="diff2">
        <div className="diff-col bad">
          <div className="diff-head">
            <span className="badge failed">直接 JOIN</span>
            <span className="tiny">行数 {naive.totals.rows}</span>
          </div>
          <table className="tbl compact">
            <thead>
              <tr>
                {naive.headers.map((h) => (
                  <th key={h}>{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {naive.rows.map((r, i) => (
                <tr key={i}>
                  {r.map((c, j) => (
                    <td key={j} className={j >= 2 ? 'num' : ''}>
                      {c}
                    </td>
                  ))}
                </tr>
              ))}
              <tr className="tot">
                <td>合计</td>
                <td className="num">{naive.totals.rows} 行</td>
                {naive.cols.map((c) => (
                  <td key={c} className="num">
                    {fmt(naive.totals[c])}
                  </td>
                ))}
              </tr>
            </tbody>
          </table>
          <div className="diff-foot">明细只列被放大的 {naive.rows.length} 个客户；合计按全部 8 个客户算</div>
        </div>

        <div className="diff-col good">
          <div className="diff-head">
            <span className="badge ran">分头聚合</span>
            <span className="tiny">行数 {grain.totals.rows}</span>
          </div>
          <table className="tbl compact">
            <thead>
              <tr>
                {grain.headers.map((h) => (
                  <th key={h}>{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {grain.rows.map((r, i) => (
                <tr key={i}>
                  {r.map((c, j) => (
                    <td key={j} className={j >= 2 ? 'num' : ''}>
                      {c}
                    </td>
                  ))}
                </tr>
              ))}
              <tr className="tot">
                <td>合计</td>
                <td className="num">{grain.totals.rows} 行</td>
                {grain.cols.map((c) => (
                  <td key={c} className="num">
                    {fmt(grain.totals[c])}
                  </td>
                ))}
              </tr>
            </tbody>
          </table>
          <div className="diff-foot">每个客户都已收敛成 1 行，明细同样只列 {grain.rows.length} 个</div>
        </div>
      </div>

      <div className="note risk" style={{ marginTop: 16 }}>
        <div>
          · 直接 JOIN 把 {grain.totals.rows} 行放大到 <b>{naive.totals.rows}</b> 行，
          {label}虚增 <b>{(inflation * 100).toFixed(1)}%</b>
          （{fmt(grain.totals[inflationMetric])} → {fmt(naive.totals[inflationMetric])}）。
        </div>
        <div>· 根因：客户 C001 有 2 笔订单 × 2 笔贷款，展开成 4 行后同一笔金额被重复计入 2 次。</div>
        <div>
          · 处置：<code className="sql-inline">order_grain</code> 与{' '}
          <code className="sql-inline">loan_grain</code> 各自{' '}
          <code className="sql-inline">GROUP BY customer_id</code> 收敛成一行，再左连主表。
        </div>
      </div>
    </section>
  );
};

/* -------------------------------------------------------------------------- */
/* 第 4 步：组装输出                                                          */
/* -------------------------------------------------------------------------- */

const StepOutput: React.FC<{
  res: ReturnType<typeof resolve>;
  tab: 'sql' | 'polars';
  setTab: (t: 'sql' | 'polars') => void;
}> = ({ res, tab, setTab }) => (
  <section className="card pad">
    <div className="eyebrow">Step 04 · Compose</div>
    <h2 className="step-h">拼成可执行的计划</h2>
    <p className="step-p">
      同一份计划，两种落地形式：给数据库的 CTE SQL，或给 Rust 侧 Polars 的惰性管道。
      下面的预览是按示例夹具现算的，与服务端执行结果无关。
    </p>

    <div className="row" style={{ gap: 8, marginBottom: 10 }}>
      <button type="button" className={`btn small ${tab === 'sql' ? 'primary' : ''}`} onClick={() => setTab('sql')}>
        Grain-Aligned SQL
      </button>
      <button type="button" className={`btn small ${tab === 'polars' ? 'primary' : ''}`} onClick={() => setTab('polars')}>
        Polars Lazy
      </button>
      <span className="grow" />
      <span className="tiny">{tab === 'sql' ? 'ANSI SQL · 可直连三方言' : 'Rust · 与 SQL 等价'}</span>
    </div>

    <pre className="sqlbox tall">{tab === 'sql' ? res.sql : res.polars}</pre>

    <div className="section-title" style={{ marginTop: 22 }}>
      结果预览 <span className="tiny">示例夹具现算，不是服务端返回</span>
    </div>
    <table className="tbl compact">
      <thead>
        <tr>
          {res.preview.headers.map((h, i) => (
            <th key={h} className={i > 0 ? 'num' : ''}>
              {h}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {res.preview.rows.map((r, i) => (
          <tr key={i}>
            {r.map((c, j) => (
              <td key={j} className={j > 0 ? 'num' : ''}>
                {c}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>

    <div className="note" style={{ marginTop: 16 }}>
      <div>
        · 执行与「在 Univer 里打开」要等 Rust 端点：
        <code className="sql-inline">POST /query/plan</code>、
        <code className="sql-inline">POST /query/execute</code>。
      </div>
      <div>
        · 端点就位后，这一页只需把 <code className="sql-inline">resolve()</code> 的入参换成真模型，界面不用改。
      </div>
    </div>
  </section>
);
