import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { ONTOLOGY } from '../data/ontology';
import {
  ONTOLOGY_SUMMARY,
  resolve,
  toOntologyTrace,
  toRealTrace,
  resolveFromLexicon,
  buildGroupSql,
  type RealResult,
} from '../services/ontology';
import {
  ApiError,
  listDatasources,
  listOntologies,
  getOntology,
  saveOntology,
  fetchLexicon,
  fetchCanon,
  type Datasource,
  type Lexicon,
  type CanonItem,
  type OntologyListItem,
  type OntologyTrace,
} from '../services/api';
import { readSavedDsn, rememberDsn } from './OntologyView';

/**
 * 四步解析（#/resolve）—— 从"本体大脑·解析器"拆出来的第二页。
 * 职责：一句话 → 四步定表 → 分头聚合 → 安全 SQL。
 * 每次点「解析」自动把痕迹落 SQLite（.ontology.db，status='auto'），历史默认展开，
 * 可回放、可一键重新解析；数据源选择与实体关系页共享（记住上次的）。
 * 四步推理仍用 data/ontology.ts 示例夹具驱动（教学核心）：任意模型的行级聚合要等服务端 /query/run。
 */

const STEPS = [
  { no: 1, title: '识别', sub: '命中的词 → 表与字段' },
  { no: 2, title: '定主表', sub: '谁在中心，谁在分支' },
  { no: 3, title: '分头聚合', sub: '防扇出：先收敛再连' },
  { no: 4, title: '组装输出', sub: 'SQL / Polars / 预览' },
] as const;

type StepNo = 1 | 2 | 3 | 4;

/** 真模式人工修订：一行命中（维度或度量），落点可选、别名可改、可启停。 */
type EditRow = {
  token: string;
  landing: string;
  col: string;
  alias: string;
  on: boolean;
  options: { landing: string; label: string }[];
};

export const OntoResolveView: React.FC = () => {
  const [text, setText] = useState('');
  const [intent, setIntent] = useState('');
  const [step, setStep] = useState<StepNo>(1);
  const [tab, setTab] = useState<'sql' | 'polars'>('sql');

  // —— 数据源（记住上次的） ——
  const [sources, setSources] = useState<Datasource[]>([]);
  const [dsn, setDsn] = useState('');
  // —— 真模型词表（postgres 有；无则回示例夹具） ——
  const [lex, setLex] = useState<Lexicon | null>(null);
  const [lexMode, setLexMode] = useState<'loading' | 'real' | 'none'>('loading');
  // 标准口径（权威 SQL 命中）
  const [canon, setCanon] = useState<CanonItem | null>(null);

  // —— 历史记录（每次解析自动落库） ——
  const [history, setHistory] = useState<OntologyListItem[]>([]);
  const [histMsg, setHistMsg] = useState<string | null>(null);
  const [trace, setTrace] = useState<OntologyTrace | null>(null);
  // 两步行内确认（预览面板 webview 会吞掉 window.confirm 原生弹窗，不能用）
  const [armedDel, setArmedDel] = useState<string | null>(null);

  const res = useMemo(() => resolve(intent), [intent]);

  /** 真模式解析结果（词表就绪时每轮 intent 实时现算）。 */
  const real = useMemo<RealResult | null>(
    () => (lexMode === 'real' && lex ? resolveFromLexicon(intent, lex) : null),
    [intent, lex, lexMode],
  );

  const loadLexicon = useCallback(async (d: string) => {
    if (!d) return;
    setLexMode('loading');
    try {
      const lx = await fetchLexicon(d);
      if (lx?.mode === 'real') {
        setLex(lx);
        setLexMode('real');
      } else {
        setLex(null);
        setLexMode('none');
      }
    } catch {
      setLex(null);
      setLexMode('none');
    }
  }, []);

  // —— 真模式人工修订状态：每步可选/可改，SQL 可编辑 ——
  const [editDims, setEditDims] = useState<EditRow[]>([]);
  const [editMets, setEditMets] = useState<EditRow[]>([]);
  const [editCenter, setEditCenter] = useState<string | null>(null);
  const [editedSql, setEditedSql] = useState<string | null>(null);
  const [savingConf, setSavingConf] = useState(false);
  const [confMsg, setConfMsg] = useState<string | null>(null);

  // 解析结果变化时，用词表重建可编辑行（同 token 的多张表落点作为可选项，备份表排后）
  useEffect(() => {
    if (canon) return; // 标准口径命中时不重建词表行、不清 SQL
    if (!real || !lex) {
      setEditDims([]);
      setEditMets([]);
      setEditCenter(null);
      setEditedSql(null);
      return;
    }
    const optFor = (token: string, kind: 'dim' | 'metric') =>
      (kind === 'dim' ? lex.dims : lex.metrics)
        .filter((w) => w.token === token)
        .sort((a, b) => (a.junk ? 1 : 0) - (b.junk ? 1 : 0))
        .map((w) => ({ landing: w.landing, label: `${w.landing}${w.junk ? '（备份表）' : ''}` }));
    const mk = (h: RealResult['hits'][number]): EditRow => ({
      token: h.token,
      landing: h.landing,
      col: h.landing.split('.')[2],
      alias: h.kind === 'metric' ? `${h.token}合计` : h.token,
      on: true,
      options: optFor(h.token, h.kind === 'metric' ? 'metric' : 'dim'),
    });
    const dedup = (rows: EditRow[]) => {
      const seen = new Set<string>();
      return rows.filter((r) => !seen.has(r.token) && seen.add(r.token));
    };
    setEditDims(dedup(real.dimTokens.map(mk)));
    setEditMets(dedup(real.metricHits.map(mk)));
    setEditCenter(real.center);
    setEditedSql(null);
  }, [real, lex, canon]);

  /** 按当前选择生成 SQL（人工改过的 SQL 用 editedSql 覆盖）。 */
  const liveSql = useMemo(() => {
    if (canon && !editCenter) return editedSql ?? canon.sql;
    if (!editCenter) return '';
    return (
      editedSql ??
      buildGroupSql(
        editCenter,
        editDims.filter((d) => d.on).map((d) => ({ col: d.col, alias: d.alias })),
        editMets.filter((m) => m.on).map((m) => ({ col: m.col, alias: m.alias })),
      )
    );
  }, [canon, editCenter, editDims, editMets, editedSql]);

  const updDim = (i: number, patch: Partial<EditRow>) =>
    setEditDims((xs) => xs.map((x, k) => (k === i ? { ...x, ...patch, ...(patch.landing ? { col: patch.landing.split('.')[2] } : {}) } : x)));
  const updMet = (i: number, patch: Partial<EditRow>) =>
    setEditMets((xs) => xs.map((x, k) => (k === i ? { ...x, ...patch, ...(patch.landing ? { col: patch.landing.split('.')[2] } : {}) } : x)));

  const centerCandidates = useMemo(() => {
    const set = new Set<string>();
    [...editDims, ...editMets].forEach((r) => r.options.forEach((o) => set.add(o.landing.split('.').slice(0, 2).join('.'))));
    if (editCenter) set.add(editCenter);
    return [...set].sort();
  }, [editDims, editMets, editCenter]);

  /** 切中心表：有该表落点的行跟着切，没有的行自动停用。 */
  const switchCenter = (t: string) => {
    setEditCenter(t);
    setEditDims((xs) =>
      xs.map((x) => {
        const opt = x.options.find((o) => o.landing.startsWith(`${t}.`));
        return opt ? { ...x, landing: opt.landing, col: opt.landing.split('.')[2], on: true } : { ...x, on: false };
      }),
    );
    setEditMets((xs) =>
      xs.map((x) => {
        const opt = x.options.find((o) => o.landing.startsWith(`${t}.`));
        return opt ? { ...x, landing: opt.landing, col: opt.landing.split('.')[2], on: true } : { ...x, on: false };
      }),
    );
    setEditedSql(null);
  };

  /** 保存人工修订版（status=confirmed；步骤 edited=1、命中带启停与别名）。 */
  const saveConfirmed = async () => {
    const centerEntity = editCenter ?? canon?.center_entity ?? null;
    if (!dsn || !centerEntity) return;
    setSavingConf(true);
    setConfMsg(null);
    const onDims = editDims.filter((d) => d.on);
    const onMets = editMets.filter((m) => m.on);
    const hits = [
      ...onDims.map((d, i) => ({ ord: i, token: d.token, kind: 'dim', target: d.landing, selected: true, alias: d.alias, evidence: '人工修订' })),
      ...onMets.map((m, i) => ({ ord: onDims.length + i, token: m.token, kind: 'metric', target: m.landing, selected: true, alias: m.alias, evidence: '人工修订' })),
    ];
    const steps = [
      {
        step_no: 1,
        kind: 'identify',
        title: canon ? '识别（标准口径·人工确认）' : '识别（真词表·人工修订）',
        detail: canon ? `标准口径：${canon.title}` : `启用 ${hits.length} 条命中（落点可选、别名可改）`,
        edited: 1,
      },
      { step_no: 2, kind: 'center', title: '定主表', detail: `中心实体 ${centerEntity}（可切换/标准口径）`, edited: 1 },
      {
        step_no: 4,
        kind: 'compose',
        title: '组装输出',
        sql: liveSql,
        detail: editedSql != null ? 'SQL 已手工修改' : '按当前选择生成',
        edited: editedSql != null ? 1 : 0,
      },
    ];
    try {
      await saveOntology({
        dsn,
        question: intent || '(未命名)',
        status: 'confirmed',
        center_entity: centerEntity,
        fanout_edges: 0,
        sql: liveSql,
        steps,
        hits,
      });
      setConfMsg(`已保存人工修订版（confirmed）`);
      void loadHistory(dsn);
    } catch (e) {
      setConfMsg(e instanceof ApiError ? `${e.code}：${e.message}` : '保存失败');
    } finally {
      setSavingConf(false);
    }
  };

  const loadHistory = useCallback(async (d: string) => {
    if (!d) return;
    try {
      const r = await listOntologies(d, 20);
      setHistory(r.items);
      setHistMsg(null);
    } catch (e) {
      setHistMsg(e instanceof ApiError ? `${e.code}：${e.message}` : '读取历史失败');
    }
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const page = await listDatasources();
        const items = page.items ?? [];
        setSources(items);
        const saved = readSavedDsn();
        const def =
          items.find((s) => s.id === saved)?.id ??
          items.find((s) => s.id === 'demo-sqlite')?.id ??
          items[0]?.id ??
          '';
        if (def) {
          setDsn(def);
          void loadHistory(def);
          void loadLexicon(def);
        }
      } catch {
        /* 拉不到数据源，四步演示照常用示例夹具跑 */
      }
    })();
  }, [loadHistory, loadLexicon]);

  const switchDsn = (v: string) => {
    setDsn(v);
    setTrace(null);
    setCanon(null);
    rememberDsn(v);
    void loadHistory(v);
    void loadLexicon(v);
  };

  const submit = async (e?: React.FormEvent) => {
    e?.preventDefault();
    const next = text.trim();
    if (!next) return;
    setIntent(next);
    setStep(1);
    setTrace(null);
    setCanon(null);
    setEditedSql(null);
    if (!dsn) return;
    // 1) 标准口径优先（权威 SQL）
    if (lexMode === 'real') {
      try {
        const r = await fetchCanon(dsn, next);
        if (r?.item) {
          const item = r.item;
          setCanon(item);
          setEditedSql(item.sql);
          saveOntology({
            dsn,
            question: next,
            status: 'auto',
            center_entity: item.center_entity || null,
            fanout_edges: 0,
            sql: item.sql,
            steps: [
              { step_no: 1, kind: 'identify', title: `标准口径命中（${item.title}）`, detail: item.basis },
              { step_no: 2, kind: 'center', title: '定主表', detail: `中心实体 ${item.center_entity || '—'}` },
              { step_no: 4, kind: 'compose', title: '组装输出', sql: item.sql, detail: '标准口径 SQL（权威）' },
            ],
            hits: [],
          })
            .then(() => loadHistory(dsn))
            .catch(() => {
              /* 留痕失败不挡解析 */
            });
          return;
        }
      } catch {
        /* 命中查询失败 → 退回词表解析 */
      }
      // 2) 词表解析
      if (lex) {
        const rr = resolveFromLexicon(next, lex);
        saveOntology(toRealTrace(dsn, next, rr))
          .then(() => loadHistory(dsn))
          .catch(() => {
            /* 留痕失败不挡解析 */
          });
        return;
      }
    }
    // 3) 示例夹具
    const resNext = resolve(next);
    saveOntology({ ...toOntologyTrace(dsn, next, resNext), status: 'auto' })
      .then(() => loadHistory(dsn))
      .catch(() => {
        /* 留痕失败不挡解析 */
      });
  };

  /** 删除一条历史（两步行内确认：第一次点进入确认态，再点才真删）。 */
  const delHistory = (h: OntologyListItem) => {
    if (armedDel !== h.id) {
      setArmedDel(h.id);
      return;
    }
    setArmedDel(null);
    fetch(`/api/ontology/${encodeURIComponent(h.id)}`, { method: 'DELETE' })
      .then((r) => r.json())
      .then(() => {
        if (trace?.id === h.id) setTrace(null);
        void loadHistory(dsn);
      })
      .catch(() => setHistMsg('删除失败'));
  };

  /** 从历史一键重新解析：填回问题、从头走四步。 */
  const rerun = (h: OntologyListItem) => {
    setText(h.question);
    setIntent(h.question);
    setStep(1);
    setTrace(null);
    window.scrollTo({ top: 0, behavior: 'smooth' });
  };

  const openTrace = async (id: string) => {
    try {
      const r = await getOntology(id);
      setTrace(r);
    } catch (e) {
      setTrace(null);
      setHistMsg(e instanceof ApiError ? `${e.code}：${e.message}` : '读取失败');
    }
  };

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Resolver</div>
          <h1>四步解析</h1>
          <div className="sub">一句话 → 四步定表 → 分头聚合 → 安全 SQL；每次解析自动进历史</div>
        </div>
        <span
          className={`badge ${lexMode === 'real' ? 'ran' : ''}`}
          title={
            lexMode === 'real'
              ? '词表来自库注释（维度=文本列、度量=数值列）；解析痕迹落 SQLite（.ontology.db）'
              : '该数据源没有真模型词表，四步用示例夹具演示；postgres 数据源会自动加载词表'
          }
        >
          {lexMode === 'real'
            ? `真模型词表 · ${lex?.counts?.dims ?? 0} 维度 / ${lex?.counts?.metrics ?? 0} 度量`
            : lexMode === 'loading'
              ? '词表加载中…'
              : '示例夹具（无真模型词表）'}
        </span>
      </div>

      {/* 数据源选择（记住上次的，与实体关系页共享） */}
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
        <span className="tiny muted">解析痕迹按这个数据源归档</span>
      </div>

      <form className="ask" onSubmit={submit}>
        <div className="ask-row">
          <input
            type="text"
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="输入一句话需求，例：按单位统计实发工资"
          />
          <button type="submit" className="btn primary">
            解析
          </button>
        </div>
        <div className="ask-foot">
          {real ? (
            <span className="tiny">
              当前：<b>{real.hits.length}</b> 条命中 ·
              需要 <b>{real.entities.length}</b> 张表 ·
              {real.dimTokens.length ? ` 按「${real.dimTokens.map((d) => d.token).join('、')}」分组` : ' 未命中维度（将输出全局合计）'}
            </span>
          ) : (
            <span className="tiny">
              当前：<b>{res.hits.filter((h) => h.kind !== 'entity').length}</b> 条命中 ·
              需要 <b>{res.entities.length}</b> 张表 ·
              {res.risk.multiple ? ' 检测到并列放大边' : ' 单放大边'}
            </span>
          )}
        </div>
      </form>

      {/* 历史记录：每次解析自动落库，可回放、可重新解析 */}
      <section className="card pad" style={{ marginTop: 16 }}>
        <div className="eyebrow">History</div>
        <h2 className="step-h">解析历史（自动留痕，可回放 / 重新解析）</h2>
        {histMsg ? (
          <div className="tiny" style={{ color: '#ff8a8a' }}>{histMsg}</div>
        ) : history.length === 0 ? (
          <div className="tiny muted">
            还没有历史 —— 点上方「解析」后自动记一条（含命中与四步结论）。
          </div>
        ) : (
          <div className="stack" style={{ gap: 8 }}>
            {history.map((h) => (
              <div key={h.id} className="row" style={{ gap: 10 }}>
                <button type="button" className="btn small" onClick={() => rerun(h)}>
                  重新解析
                </button>
                <button type="button" className="btn small ghost" onClick={() => openTrace(h.id)}>
                  查看
                </button>
                <button
                  type="button"
                  className={`btn small ${armedDel === h.id ? 'primary' : 'ghost'}`}
                  onClick={() => delHistory(h)}
                >
                  {armedDel === h.id ? '确认删除？' : '删除'}
                </button>
                <span className="grow" style={{ fontSize: 13 }}>
                  {h.question}
                </span>
                <span className="tiny muted">
                  <span className={`pill ${h.status === 'confirmed' ? 'ok' : ''}`}>{h.status}</span>
                  {' · '}
                  {h.center_entity ?? '—'} · {h.fanout_edges} 放大边 ·{' '}
                  {(h.created_at || '').slice(0, 19).replace('T', ' ')}
                </span>
              </div>
            ))}
          </div>
        )}
        {trace ? (
          <div className="card pad" style={{ marginTop: 12 }}>
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
          {step === 1 ? (
            canon ? <CanonHit c={canon} />
            : real ? (
              <RealIdentify r={real} dims={editDims} mets={editMets} onDim={updDim} onMet={updMet} />
            ) : <StepIdentify res={res} />
          ) : null}
          {step === 2 ? (
            canon ? <CanonBasis c={canon} />
            : real ? (
              <RealCenter r={real} center={editCenter} candidates={centerCandidates} onSwitch={switchCenter} />
            ) : <StepCenter res={res} />
          ) : null}
          {step === 3 ? (
            canon ? <CanonGrain c={canon} />
            : real ? <RealGrain r={real} /> : <StepGrain res={res} />
          ) : null}
          {step === 4 ? (
            real ? (
              <RealOutput
                sql={liveSql}
                edited={editedSql != null}
                onEdit={setEditedSql}
                onRegen={() => setEditedSql(null)}
                saving={savingConf}
                msg={confMsg}
                onSaveConfirmed={saveConfirmed}
                canSave={!!(editCenter || canon)}
              />
            ) : <StepOutput res={res} tab={tab} setTab={setTab} />
          ) : null}

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
          {real ? (
            <>
              <div className="card pad rail-card">
                <div className="rail-head">真模型词表</div>
                <div className="rail-stat">
                  <span>维度词</span>
                  <b>{lex?.counts?.dims ?? 0}</b>
                </div>
                <div className="rail-stat">
                  <span>度量词</span>
                  <b>{lex?.counts?.metrics ?? 0}</b>
                </div>
                <div className="rail-stat">
                  <span>时间词</span>
                  <b>{lex?.counts?.times ?? 0}</b>
                </div>
                <div className="tiny" style={{ marginTop: 8 }}>
                  实体 {lex?.counts?.entities ?? 0} 张（service.* 表+视图，词来自列注释）
                </div>
              </div>
              <div className="card pad rail-card">
                <div className="rail-head">本次用到的表</div>
                {real.entities.map((id) => (
                  <div key={id} className="rail-line">
                    <code className="sql-inline">{id}</code>
                    <span className="tiny">{id === real.center ? '中心表' : '命中'}</span>
                  </div>
                ))}
                {real.entities.length === 0 ? (
                  <div className="tiny muted">无命中</div>
                ) : null}
              </div>
            </>
          ) : (
          <>
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
          </>
          )}
        </aside>
        ) : null}
      </div>
    </>
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

/* -------------------------------------------------------------------------- */
/* 真模式步骤卡（词表来自库注释，SQL 是可直接执行的合法语句）                    */
/* -------------------------------------------------------------------------- */

const RealIdentify: React.FC<{
  r: RealResult;
  dims: EditRow[];
  mets: EditRow[];
  onDim: (i: number, patch: Partial<EditRow>) => void;
  onMet: (i: number, patch: Partial<EditRow>) => void;
}> = ({ r, dims, mets, onDim, onMet }) => {
  const row = (x: EditRow, i: number, kind: string, upd: (k: number, p: Partial<EditRow>) => void) => (
    <tr key={`${kind}-${i}`} style={{ opacity: x.on ? 1 : 0.45 }}>
      <td>
        <input type="checkbox" checked={x.on} onChange={(e) => upd(i, { on: e.target.checked })} />
      </td>
      <td>
        <span className={`pill ${kind}`}>{x.token}</span>
      </td>
      <td>
        {x.options.length > 1 ? (
          <select value={x.landing} onChange={(e) => upd(i, { landing: e.target.value })}>
            {x.options.map((o) => (
              <option key={o.landing} value={o.landing}>
                {o.label}
              </option>
            ))}
          </select>
        ) : (
          <span className="mono">{x.landing}</span>
        )}
      </td>
      <td>
        <input type="text" value={x.alias} onChange={(e) => upd(i, { alias: e.target.value })} style={{ width: '95%' }} />
      </td>
    </tr>
  );
  return (
    <section className="card pad">
      <div className="eyebrow">Step 01 · Identify（可编辑）</div>
      <h2 className="step-h">这句话命中了什么 —— 可选落点、可改别名、可停用</h2>
      <p className="step-p">
        勾选决定进不进 SQL；同词多表落点用下拉选（备份表已排后）；<b>别名</b>就是输出列名，直接改。
      </p>
      <table className="tbl compact">
        <thead>
          <tr>
            <th style={{ width: '6%' }}>启用</th>
            <th style={{ width: '16%' }}>命中词</th>
            <th style={{ width: '42%' }}>落点（可选）</th>
            <th>输出别名（可改）</th>
          </tr>
        </thead>
        <tbody>
          {dims.map((x, i) => row(x, i, 'dim', onDim))}
          {mets.map((x, i) => row(x, i, 'metric', onMet))}
          {dims.length + mets.length === 0 ? (
            <tr>
              <td colSpan={4} className="muted">
                没有命中词表 —— 换个说法，或先给相关列补中文注释（注释就是词）
              </td>
            </tr>
          ) : null}
        </tbody>
      </table>
      <div className="callout-line">
        启用 {dims.filter((d) => d.on).length} 个维度 + {mets.filter((m) => m.on).length} 个度量；命中 {r.entities.length} 张表
      </div>
    </section>
  );
};

const RealCenter: React.FC<{
  r: RealResult;
  center: string | null;
  candidates: string[];
  onSwitch: (t: string) => void;
}> = ({ r, center, candidates, onSwitch }) => (
  <section className="card pad">
    <div className="eyebrow">Step 02 · Topology（可编辑）</div>
    <h2 className="step-h">谁是中心表 —— 可切换</h2>
    {center ? (
      <>
        <div className="center-box">
          <div className="center-top">
            <div>
              <div className="tiny">中心表（切换后落点自动跟表走，没有该表的行自动停用）</div>
              <div className="center-name">{center}</div>
            </div>
            <span className="badge ran">单表聚合</span>
          </div>
          <div className="row" style={{ gap: 8, marginTop: 8 }}>
            <select value={center} onChange={(e) => onSwitch(e.target.value)} style={{ flex: '0 0 340px' }}>
              {candidates.map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
          </div>
          <div className="muted" style={{ fontSize: 12.5, lineHeight: 1.85 }}>
            {r.why}
          </div>
        </div>
        <div className="note">
          {r.notes.map((n) => (
            <div key={n}>· {n}</div>
          ))}
        </div>
      </>
    ) : (
      <div className="note risk">
        <div>· 没有可定为中心的表 —— 未命中任何维度/度量词。</div>
        {r.notes.map((n) => (
          <div key={n}>· {n}</div>
        ))}
      </div>
    )}
  </section>
);

const RealGrain: React.FC<{ r: RealResult }> = ({ r }) => (
  <section className="card pad">
    <div className="eyebrow">Step 03 · Grains Aligned</div>
    <h2 className="step-h">粒度检查</h2>
    {r.entities.length <= 1 ? (
      <div className="note">
        <div>
          · 本次命中 {r.entities.length} 张表，<b>单表聚合不存在扇出</b>，无需分头收敛。
        </div>
        <div>· 若以后命中多张表，要先按关联键（人 sfzh + 单位 dwdm + 月 ym）各自收敛再连，防止行数放大。</div>
      </div>
    ) : (
      <div className="note risk">
        <div>· 命中 {r.entities.length} 张表：{r.entities.join('、')}</div>
        <div>· 自动连表（v1）未做 —— 需人工确认关联键后再写 JOIN，避免跨表扇出。</div>
      </div>
    )}
  </section>
);

const RealOutput: React.FC<{
  sql: string;
  edited: boolean;
  onEdit: (s: string) => void;
  onRegen: () => void;
  saving: boolean;
  msg: string | null;
  onSaveConfirmed: () => void;
  canSave: boolean;
}> = ({ sql, edited, onEdit, onRegen, saving, msg, onSaveConfirmed, canSave }) => (
  <section className="card pad">
    <div className="eyebrow">Step 04 · Compose（可编辑）</div>
    <h2 className="step-h">生成的 SQL —— 可直接改</h2>
    <textarea
      value={sql}
      onChange={(e) => onEdit(e.target.value)}
      spellCheck={false}
      style={{
        width: '100%',
        minHeight: 220,
        fontFamily: 'JetBrains Mono, monospace',
        fontSize: 13,
        lineHeight: 1.6,
        padding: 10,
      }}
    />
    <div className="row" style={{ gap: 8, marginTop: 10, alignItems: 'center', flexWrap: 'wrap' }}>
      <button type="button" className="btn small" onClick={onRegen}>
        ↻ 按当前选择重新生成
      </button>
      <button type="button" className="btn small primary" disabled={saving || !canSave} onClick={onSaveConfirmed}>
        {saving ? '保存中…' : '💾 保存解析结论（confirmed）'}
      </button>
      {edited ? (
        <span className="tiny" style={{ color: '#b45309', fontWeight: 600 }}>
          SQL 已手工修改（保存时标记 edited=1）
        </span>
      ) : (
        <span className="tiny muted">按当前选择自动生成</span>
      )}
      {msg ? <span className="tiny">{msg}</span> : null}
    </div>
    <div className="note" style={{ marginTop: 12 }}>
      <div>
        · 保存的人工修订版进历史（confirmed，步骤标 edited）；SQL 可直接在 psql / DBeaver
        执行；页面内执行要等 <code className="sql-inline">POST /query/execute</code>。
      </div>
    </div>
  </section>
);

/* -------------------------------------------------------------------------- */
/* 标准口径卡（命中标准问题库时：步骤 1-3 的说明版）                            */
/* -------------------------------------------------------------------------- */

const CanonHit: React.FC<{ c: CanonItem }> = ({ c }) => (
  <section className="card pad">
    <div className="eyebrow">Step 01 · Identify（标准口径）</div>
    <h2 className="step-h">命中标准问题：{c.title}</h2>
    <p className="step-p">
      这个问法在业务上有<b>权威口径</b>（来自源码 / dashboard 原始 SQL / 统计视图定义），
      不走词表临时拼装，直接用权威 SQL —— 结果可核对、口径可追溯。
    </p>
    <div className="callout-line">
      口径编号 <code className="sql-inline">{c.id}</code> · 中心实体{' '}
      <code className="sql-inline">{c.center_entity || '—'}</code> · 匹配规则{' '}
      <code className="sql-inline">{c.match}</code>
    </div>
    <div className="note" style={{ marginTop: 12 }}>
      <div>· {c.basis}</div>
    </div>
  </section>
);

const CanonBasis: React.FC<{ c: CanonItem }> = ({ c }) => (
  <section className="card pad">
    <div className="eyebrow">Step 02 · Topology（口径说明）</div>
    <h2 className="step-h">数据来源与注意事项</h2>
    <div className="center-box">
      <div className="center-top">
        <div>
          <div className="tiny">中心实体</div>
          <div className="center-name">{c.center_entity || '—'}</div>
        </div>
        <span className="badge ran">标准口径</span>
      </div>
      <div className="muted" style={{ fontSize: 12.5, lineHeight: 1.85 }}>
        {c.basis}
      </div>
    </div>
    <div className="note">
      {(c.notes || []).map((n) => (
        <div key={n}>· {n}</div>
      ))}
    </div>
  </section>
);

const CanonGrain: React.FC<{ c: CanonItem }> = ({ c }) => (
  <section className="card pad">
    <div className="eyebrow">Step 03 · Grains Aligned（口径说明）</div>
    <h2 className="step-h">粒度与扇出</h2>
    <div className="note">
      <div>
        · 标准口径基于 {c.center_entity || '权威视图/表'}，其内部已完成聚合，<b>不存在跨表扇出</b>。
      </div>
      <div>· 如需限定月份/单位范围，在 SQL 外层加 WHERE（第 4 步可直接改）。</div>
    </div>
  </section>
);
