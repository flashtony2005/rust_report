import React, { useCallback, useEffect, useState } from 'react';
import {
  ApiError,
  fetchOntology,
  listDatasources,
  type Datasource,
  type OntoGraph,
  type OntologyGraphReply,
} from '../services/api';

/**
 * 本体 · 实体与关系（图谱页）—— 从"本体大脑·解析器"拆出来的第一页。
 * 只管两件事：真模型图谱 + 实体/关系目录（列出 / 搜索 / 分页 / 属性 / 刷新）。
 * 四步解析拆到了 OntoResolveView（#/resolve）；两页共享记住的数据源选择（localStorage）。
 */

const SAVED_DSN_KEY = 'rpx.onto-dsn';

/** 记住/读取数据源选择（两个本体页共享同一份）。 */
export function rememberDsn(dsn: string) {
  try {
    localStorage.setItem(SAVED_DSN_KEY, dsn);
  } catch {
    /* 隐私模式忽略 */
  }
}

export function readSavedDsn(): string | null {
  try {
    return localStorage.getItem(SAVED_DSN_KEY);
  } catch {
    return null;
  }
}

export const OntologyView: React.FC = () => {
  // —— 真实模型图谱 + 数据源选择（记住上次的） ——
  const [sources, setSources] = useState<Datasource[]>([]);
  const [dsn, setDsn] = useState('');
  const [graph, setGraph] = useState<OntoGraph | null>(null);
  const [graphState, setGraphState] = useState<'loading' | 'ok' | 'none' | 'down'>('loading');
  const [graphMsg, setGraphMsg] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshedAt, setRefreshedAt] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportMsg, setExportMsg] = useState<string | null>(null);

  /** 导出本体为 MD 文件（实体清单 + 关系清单 + 源码证据，为后续工作做上下文）。 */
  const exportMd = async () => {
    if (!dsn || exporting) return;
    setExporting(true);
    setExportMsg(null);
    try {
      const r = await fetch(`/api/bizmodels/${encodeURIComponent(dsn)}/export`);
      const rep = await r.json();
      if (rep?.ok) setExportMsg(`已导出：${rep.path}`);
      else setExportMsg(`导出失败：${rep?.error?.message || '未知错误'}`);
    } catch {
      setExportMsg('导出失败：服务不可达');
    } finally {
      setExporting(false);
    }
  };

  const loadDsn = useCallback(async (v: string) => {
    if (!v) return;
    setGraphState('loading');
    try {
      const r: OntologyGraphReply = await fetchOntology(v);
      if (r.has_model && r.ontology) {
        setGraph(r.ontology);
        setGraphState('ok');
      } else {
        setGraph(null);
        setGraphState('none');
      }
    } catch (e) {
      setGraph(null);
      setGraphState('down');
      setGraphMsg(e instanceof ApiError ? `${e.code}：${e.message}` : '读取模型失败');
    }
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const page = await listDatasources();
        const items = page.items ?? [];
        setSources(items);
        // 优先记住的选择，其次 demo-sqlite，再退第一条
        const saved = readSavedDsn();
        const def =
          items.find((s) => s.id === saved)?.id ??
          items.find((s) => s.id === 'demo-sqlite')?.id ??
          items[0]?.id ??
          '';
        if (def) {
          setDsn(def);
          void loadDsn(def);
        } else {
          setGraphState('none');
        }
      } catch {
        setGraphState('none');
      }
    })();
  }, [loadDsn]);

  const switchDsn = (v: string) => {
    setDsn(v);
    setRefreshedAt(null);
    rememberDsn(v);
    void loadDsn(v);
  };

  /** 刷新图谱：重新连库现算（外键/同名列候选可能随库结构变化）。 */
  const refreshGraph = async () => {
    if (!dsn || refreshing) return;
    setRefreshing(true);
    try {
      const r: OntologyGraphReply = await fetchOntology(dsn);
      if (r.has_model && r.ontology) {
        setGraph(r.ontology);
        setGraphState('ok');
      } else {
        setGraph(null);
        setGraphState('none');
      }
      setRefreshedAt(new Date().toLocaleTimeString('zh-CN', { hour12: false }));
    } catch (e) {
      setGraphState('down');
      setGraphMsg(e instanceof ApiError ? `${e.code}：${e.message}` : '刷新失败');
    } finally {
      setRefreshing(false);
    }
  };

  const badge = (() => {
    if (graphState === 'down') return { cls: '', text: '后端未连接' };
    if (graphState === 'ok' && graph) {
      return {
        cls: 'ran',
        text: `真实模型已接入 · ${graph.entities.length} 实体 / ${graph.edges.length} 关系`,
      };
    }
    if (graphState === 'none') return { cls: '', text: '该数据源无本体模型' };
    return { cls: '', text: '加载中…' };
  })();

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Ontology</div>
          <h1>本体 · 实体与关系</h1>
          <div className="sub">
            真连库现算：真实外键 + 源码确认 + 同名列候选；四步解析在「四步解析」页
          </div>
        </div>
        <span className={`badge ${badge.cls}`}>{badge.text}</span>
      </div>

      {/* 数据源选择（记住上次的） */}
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
        <button type="button" className="btn small" onClick={exportMd} disabled={exporting || graphState !== 'ok'}>
          {exporting ? '导出中…' : '⬇ 导出 MD'}
        </button>
        <span className="tiny muted">选择会被记住，两个本体页共用</span>
        {exportMsg ? (
          <span className="tiny" style={{ color: exportMsg.startsWith('已导出') ? 'var(--ok, #2e7d32)' : '#ff8a8a' }}>
            {exportMsg}
          </span>
        ) : null}
      </div>

      {/* 真实模型图谱 */}
      <section className="card pad" style={{ marginTop: 16 }}>
        <div className="eyebrow">Real Ontology</div>
        <h2 className="step-h">真实模型图谱（连库现算）</h2>
        <p className="step-p">
          这里画的是<b>该数据源真正的</b>实体 / 关系，不是示例。关系 = 真实外键 + 源码确认 +
          同名列候选，每条都带依据。
        </p>
        {graphState === 'ok' && graph ? (
          <OntologyGraph graph={graph} />
        ) : graphState === 'loading' ? (
          <div className="tiny">加载中…</div>
        ) : graphState === 'down' ? (
          <div className="tiny" style={{ color: '#ff8a8a' }}>{graphMsg}</div>
        ) : (
          <div className="tiny muted">该数据源还没有可画的本体关系。</div>
        )}
      </section>

      {/* 实体 · 关系目录：列出 / 搜索 / 分页 / 点实体看属性 / 刷新 */}
      {graphState === 'ok' && graph ? (
        <OntologyCatalog
          graph={graph}
          dsn={dsn}
          onRefresh={refreshGraph}
          refreshing={refreshing}
          refreshedAt={refreshedAt}
        />
      ) : null}
    </>
  );
};

/* -------------------------------------------------------------------------- */
/* 真实模型图谱：实体为节点、关系为有向边（带基数徽标）                          */
/* -------------------------------------------------------------------------- */

const OntologyGraph: React.FC<{ graph: OntoGraph }> = ({ graph }) => {
  const n = graph.entities.length;
  const nodeW = 156;
  const nodeH = 60;
  // 小图谱（≤12 实体）：一行排开、弧形边；大图谱：网格排布、直线边（否则 200+ 节点排成 4 万像素长条）
  const grid = n > 12;
  const cols = grid ? Math.max(6, Math.min(10, Math.ceil(Math.sqrt(n) * 1.4))) : n;
  const slot = nodeW + 34;
  const rowH = nodeH + 56;
  const W = Math.max(640, cols * slot);
  const H = grid ? Math.ceil(n / cols) * rowH : 150;
  const pos = graph.entities.map((_, i) => ({
    x: (i % cols) * slot + (slot - nodeW) / 2,
    y: Math.floor(i / cols) * rowH + rowH / 2 - nodeH / 2,
  }));
  const cy = H / 2;
  const apexY = 16;
  const center = (i: number) => ({
    cx: pos[i].x + nodeW / 2,
    cy: pos[i].y + nodeH / 2,
  });
  return (
    <svg
      className="onto-graph"
      viewBox={`0 0 ${W} ${H}`}
      width="100%"
      height={grid ? Math.min(H, 560) : H}
      style={grid ? { overflow: 'auto', display: 'block' } : undefined}
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
        let d: string;
        let mx: number;
        let my: number;
        if (grid) {
          // 直线边：任意两节点（可能跨行）
          const a = center(fi);
          const b = center(ti);
          d = `M ${a.cx} ${a.cy} L ${b.cx} ${b.cy}`;
          mx = (a.cx + b.cx) / 2;
          my = (a.cy + b.cy) / 2;
        } else {
          const x1 = fi < ti ? pos[fi].x + nodeW : pos[fi].x;
          const x2 = fi < ti ? pos[ti].x : pos[ti].x + nodeW;
          mx = (x1 + x2) / 2;
          my = apexY + 4;
          d = `M ${x1} ${cy} C ${x1} ${apexY + 34}, ${x2} ${apexY + 34}, ${x2} ${cy}`;
        }
        return (
          <g key={`${e.from}-${e.to}-${idx}`}>
            <path d={d} fill="none" stroke="#7c93ff" strokeWidth={1.6} markerEnd="url(#onto-arrow)" />
            <g transform={`translate(${mx}, ${my})`}>
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
            {ent.id.replace('service.', '')}
          </text>
          <text x={14} y={44} fontSize={12} fill="#9fb0c9">
            {ent.name === ent.id ? '' : ent.name.slice(0, 12)}
          </text>
        </g>
      ))}
    </svg>
  );
};

/* -------------------------------------------------------------------------- */
/* 实体 · 关系目录：列出 / 搜索 / 分页 / 点实体看属性 / 刷新                    */
/* -------------------------------------------------------------------------- */

const PAGE_SIZE = 20;

const OntologyCatalog: React.FC<{
  graph: OntoGraph;
  dsn: string;
  onRefresh: () => void;
  refreshing: boolean;
  refreshedAt: string | null;
}> = ({ graph, dsn, onRefresh, refreshing, refreshedAt }) => {
  const [tab, setTab] = useState<'entity' | 'edge'>('entity');
  const [entQ, setEntQ] = useState('');
  const [edgeQ, setEdgeQ] = useState('');
  const [kind, setKind] = useState<'all' | '1:N' | '源码' | '候选'>('all');
  // 分页
  const [entPage, setEntPage] = useState(1);
  const [edgePage, setEdgePage] = useState(1);
  // 实体详情（列属性：列名 / 中文名 / 类型 / 主键）
  const [sel, setSel] = useState<string | null>(null);
  const [cols, setCols] = useState<{ name: string; cn: string; type: string; ispk: boolean }[] | null>(null);
  const [colsErr, setColsErr] = useState<string | null>(null);
  const [colsLoading, setColsLoading] = useState(false);

  // 筛选条件变了就回到第一页
  useEffect(() => setEntPage(1), [entQ]);
  useEffect(() => setEdgePage(1), [edgeQ, kind]);

  const openEntity = (id: string) => {
    if (sel === id) {
      setSel(null);
      setCols(null);
      return;
    }
    setSel(id);
    setCols(null);
    setColsErr(null);
    setColsLoading(true);
    fetch(`/api/datasources/${encodeURIComponent(dsn)}/columns?table=${encodeURIComponent(id)}`)
      .then((r) => r.json())
      .then((rep: { ok: boolean; items?: { name: string; cn: string; type: string; ispk: boolean }[]; error?: { message?: string } }) => {
        if (rep?.ok && rep.items) setCols(rep.items);
        else setColsErr(rep?.error?.message || '读取列属性失败');
      })
      .catch(() => setColsErr('读取列属性失败'))
      .finally(() => setColsLoading(false));
  };

  const ents = (() => {
    const q = entQ.trim().toLowerCase();
    if (!q) return graph.entities;
    return graph.entities.filter(
      (e) => e.id.toLowerCase().includes(q) || (e.name || '').toLowerCase().includes(q),
    );
  })();

  const edges = (() => {
    const q = edgeQ.trim().toLowerCase();
    return graph.edges.filter((e) => {
      if (kind !== 'all' && e.cardinality !== kind) return false;
      if (!q) return true;
      return `${e.from} ${e.to} ${e.evidence || ''}`.toLowerCase().includes(q);
    });
  })();

  const relCount = (id: string) =>
    graph.edges.filter((x) => x.from === id || x.to === id).length;

  // 分页切片
  const entPages = Math.max(1, Math.ceil(ents.length / PAGE_SIZE));
  const entView = ents.slice((entPage - 1) * PAGE_SIZE, entPage * PAGE_SIZE);
  const edgePages = Math.max(1, Math.ceil(edges.length / PAGE_SIZE));
  const edgeView = edges.slice((edgePage - 1) * PAGE_SIZE, edgePage * PAGE_SIZE);

  const pager = (page: number, pages: number, total: number, setPage: (p: number) => void) => (
    <div className="row" style={{ gap: 8, marginTop: 8, alignItems: 'center' }}>
      <button type="button" className="btn small" disabled={page <= 1} onClick={() => setPage(page - 1)}>
        ← 上一页
      </button>
      <span className="tiny">
        第 {page} / {pages} 页 · 共 {total} 条
      </span>
      <button type="button" className="btn small" disabled={page >= pages} onClick={() => setPage(page + 1)}>
        下一页 →
      </button>
    </div>
  );

  return (
    <section className="card pad" style={{ marginTop: 16 }}>
      <div className="eyebrow">Catalog</div>
      <div className="row" style={{ gap: 10, flexWrap: 'wrap', alignItems: 'center' }}>
        <h2 className="step-h" style={{ margin: 0 }}>
          实体 · 关系目录
        </h2>
        <span className="grow" />
        {refreshedAt ? <span className="tiny muted">上次刷新 {refreshedAt}</span> : null}
        <button type="button" className="btn small" onClick={onRefresh} disabled={refreshing}>
          {refreshing ? '刷新中…' : '↻ 刷新图谱'}
        </button>
      </div>
      <p className="step-p">
        实体 = 主库 service.* 全部表（名称取表注释）；关系 = 真实外键 + 源码确认 + 同名列候选。
        点实体行展开属性（列/类型/注释/主键）；每页 {PAGE_SIZE} 条，可翻页。
      </p>

      <div className="row" style={{ gap: 8, marginBottom: 10 }}>
        <button
          type="button"
          className={`btn small ${tab === 'entity' ? 'primary' : ''}`}
          onClick={() => setTab('entity')}
        >
          实体（{graph.entities.length}）
        </button>
        <button
          type="button"
          className={`btn small ${tab === 'edge' ? 'primary' : ''}`}
          onClick={() => setTab('edge')}
        >
          关系（{graph.edges.length}）
        </button>
      </div>

      {tab === 'entity' ? (
        <>
          <div className="row" style={{ gap: 8, marginBottom: 8 }}>
            <input
              type="text"
              value={entQ}
              onChange={(e) => setEntQ(e.target.value)}
              placeholder="搜实体 id 或名称（如 user、单位、GZ）"
              style={{ flex: 1 }}
            />
            <span className="tiny muted">
              显示 {ents.length} / {graph.entities.length}
            </span>
          </div>
          <table className="tbl compact">
            <thead>
              <tr>
                <th style={{ width: '36%' }}>实体</th>
                <th style={{ width: '32%' }}>名称（表注释）</th>
                <th style={{ width: '12%' }}>类型</th>
                <th>关系数</th>
              </tr>
            </thead>
            <tbody>
              {entView.map((e) => (
                <tr
                  key={e.id}
                  style={{ cursor: 'pointer', background: sel === e.id ? '#f5f1ea' : undefined }}
                  title="点这行 → 展开实体属性（列/类型/注释/主键）"
                  onClick={() => openEntity(e.id)}
                >
                  <td className="mono">{e.id}</td>
                  <td>
                    {e.name === e.id ? <span className="muted">（无注释）</span> : e.name}
                  </td>
                  <td>{e.kind === 'view' ? <span className="pill ok">视图</span> : '表'}</td>
                  <td>{relCount(e.id) || <span className="muted">0</span>}</td>
                </tr>
              ))}
              {entView.length === 0 ? (
                <tr>
                  <td colSpan={4} className="muted">
                    没有匹配的实体
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
          {pager(entPage, entPages, ents.length, setEntPage)}

          {/* 实体详情：列属性 + 相关关系入口 */}
          {sel ? (
            <div className="card pad" style={{ marginTop: 12 }}>
              <div className="row" style={{ gap: 10, alignItems: 'center' }}>
                <div className="rail-head">#{sel}</div>
                <span className="grow" />
                <button
                  type="button"
                  className="btn small"
                  onClick={() => {
                    setEdgeQ(sel);
                    setTab('edge');
                  }}
                >
                  查它的关系 →
                </button>
              </div>
              {colsLoading ? (
                <div className="tiny" style={{ marginTop: 8 }}>读取列属性中…</div>
              ) : colsErr ? (
                <div className="tiny" style={{ color: '#ff8a8a', marginTop: 8 }}>{colsErr}</div>
              ) : cols ? (
                <>
                  <table className="tbl compact" style={{ marginTop: 8 }}>
                    <thead>
                      <tr>
                        <th style={{ width: '28%' }}>列名</th>
                        <th style={{ width: '28%' }}>中文名</th>
                        <th style={{ width: '32%' }}>类型</th>
                        <th>主键</th>
                      </tr>
                    </thead>
                    <tbody>
                      {cols.map((c) => (
                        <tr key={c.name}>
                          <td className="mono">{c.name}</td>
                          <td>{c.cn || <span className="muted">—</span>}</td>
                          <td className="mono muted">{c.type}</td>
                          <td>{c.ispk ? <span className="pill ok">PK</span> : ''}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  <div className="tiny muted" style={{ marginTop: 6 }}>
                    共 {cols.length} 列 · 中文名：本表列注释优先，缺失时用同名列在其它表注释里的多数票兜底
                  </div>
                </>
              ) : null}
            </div>
          ) : null}
        </>
      ) : (
        <>
          <div className="row" style={{ gap: 8, marginBottom: 8 }}>
            <select
              value={kind}
              onChange={(e) => setKind(e.target.value as 'all' | '1:N' | '源码' | '候选')}
              style={{ flex: '0 0 150px' }}
            >
              <option value="all">全部类型</option>
              <option value="1:N">真外键 1:N</option>
              <option value="源码">源码确认</option>
              <option value="候选">同名列候选</option>
            </select>
            <input
              type="text"
              value={edgeQ}
              onChange={(e) => setEdgeQ(e.target.value)}
              placeholder="搜表名或依据（如 user、单位代码）"
              style={{ flex: 1 }}
            />
            <span className="tiny muted">
              显示 {edges.length} / {graph.edges.length}
            </span>
          </div>
          <table className="tbl compact">
            <thead>
              <tr>
                <th style={{ width: '34%' }}>关系（1 → N）</th>
                <th style={{ width: '12%' }}>基数</th>
                <th>依据</th>
              </tr>
            </thead>
            <tbody>
              {edgeView.map((e, i) => (
                <tr key={`${e.from}-${e.to}-${i}`}>
                  <td className="mono">
                    {e.from} → {e.to}
                  </td>
                  <td>
                    <span className={`pill ${e.cardinality === '1:N' ? 'warn' : 'ok'}`}>
                      {e.cardinality}
                    </span>
                  </td>
                  <td className="muted">{e.evidence || '—'}</td>
                </tr>
              ))}
              {edgeView.length === 0 ? (
                <tr>
                  <td colSpan={3} className="muted">
                    没有匹配的关系
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
          {pager(edgePage, edgePages, edges.length, setEdgePage)}
        </>
      )}
    </section>
  );
};
