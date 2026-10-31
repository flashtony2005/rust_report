import React, { useState } from 'react';
import { store } from '../services/store';
import { useToast } from '../components/Toast';
import { runAttributionAnalysis } from '../services/reportEngine';
import { AnalysisDoc } from '../types';

export const AnalysisView: React.FC = () => {
  const { toast } = useToast();
  const datasources = store.getDatasources();

  const [dsn, setDsn] = useState(datasources[0]?.id || 'demo-sqlite');
  const [metric, setMetric] = useState('amount');
  const [timeField, setTimeField] = useState('month');
  const [grain, setGrain] = useState('month');
  const [asOf, setAsOf] = useState('2026-10-01');
  const [loading, setLoading] = useState(false);
  const [analysisDoc, setAnalysisDoc] = useState<AnalysisDoc | null>(() =>
    runAttributionAnalysis('demo-sqlite', 'amount', 'month', 'month', '2026-10-01')
  );

  const handleRunAnalysis = () => {
    setLoading(true);
    setTimeout(() => {
      const doc = runAttributionAnalysis(dsn, metric, timeField, grain, asOf);
      setAnalysisDoc(doc);
      setLoading(false);
      toast('归因拆解计算完成');
    }, 450);
  };

  const money = (v?: number) =>
    v == null ? '—' : '¥ ' + v.toLocaleString('zh-CN', { maximumFractionDigits: 0 });

  const pct = (v?: number) => (v == null ? '—' : `${(v * 100).toFixed(1)}%`);

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span>ATTRIBUTION ENGINE</span>
            <span style={{ opacity: 0.6 }}>·</span>
            <span>「一句话生成」异常闭环</span>
          </div>
          <h1 style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
            <span>自动化归因与贡献度下钻</span>
            <span
              style={{
                fontSize: '11.5px',
                background: 'linear-gradient(135deg, #1e293b, #0f172a)',
                color: '#f8fafc',
                padding: '3px 9px',
                borderRadius: '6px',
                fontWeight: 600,
              }}
            >
              Waterfall & IR 编译器
            </span>
          </h1>
          <div className="sub" style={{ maxWidth: '75ch' }}>
            当报表指标出现异常时，Agent 自动拆解受控查询（整体 ➔ 维度切片 ➔ 跨层级下钻）。每一步均生成确定性 SQL 与贡献度比例，为人机决策提供透明依据。
          </div>
        </div>
      </div>

      <div className="card pad" style={{ border: '1px solid var(--line)', background: 'var(--paper-2)' }}>
        <div className="row" style={{ flexWrap: 'wrap', gap: '10px' }}>
          <select
            value={dsn}
            onChange={(e) => setDsn(e.target.value)}
            style={{ flex: '0 0 180px' }}
          >
            {datasources.map((x) => (
              <option key={x.id} value={x.id}>
                {x.label || x.id}
              </option>
            ))}
          </select>

          <select
            value={metric}
            onChange={(e) => setMetric(e.target.value)}
            style={{ flex: '0 0 160px' }}
          >
            <option value="amount">销售额 (SUM amount)</option>
            <option value="qty">销售数量 (SUM qty)</option>
          </select>

          <select
            value={timeField}
            onChange={(e) => setTimeField(e.target.value)}
            style={{ flex: '0 0 160px' }}
          >
            <option value="month">按月份维度 (month)</option>
            <option value="date">按日期维度 (date)</option>
          </select>

          <select
            value={grain}
            onChange={(e) => setGrain(e.target.value)}
            style={{ flex: '0 0 110px' }}
          >
            <option value="day">按日 (Day)</option>
            <option value="week">按周 (Week)</option>
            <option value="month">按月 (Month)</option>
            <option value="quarter">按季 (Quarter)</option>
            <option value="year">按年 (Year)</option>
          </select>

          <input
            type="date"
            value={asOf}
            onChange={(e) => setAsOf(e.target.value)}
            title="以哪一天为基准（默认今天）；复盘用"
            style={{ flex: '0 0 150px' }}
          />

          <button
            type="button"
            className="btn primary"
            disabled={loading}
            onClick={handleRunAnalysis}
            style={{ minWidth: '110px' }}
          >
            {loading ? '归因拆解中…' : '执行归因拆解 →'}
          </button>
        </div>

        <div className="hint" style={{ marginTop: '12px', lineHeight: '1.8' }}>
          💡 <strong>归因窗口规则</strong>：自动取「上一个完整周期」与「等长前移对比期」，由编译器自动生成确定性日期过滤条件，避免假波动干扰；维度由业务模型自动挑两个候选文本维度。
        </div>
      </div>

      {analysisDoc && (
        <div className="card pad" style={{ marginTop: '20px', border: '1.5px solid var(--line)', background: '#ffffff' }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '8px' }}>
            <div className="eyebrow" style={{ color: 'var(--accent)' }}>EXECUTIVE CONCLUSION</div>
            <span
              style={{
                fontSize: '11px',
                background: analysisDoc.totals.delta < 0 ? '#fee2e2' : '#dcfce7',
                color: analysisDoc.totals.delta < 0 ? '#991b1b' : '#166534',
                padding: '2px 8px',
                borderRadius: '4px',
                fontWeight: 700,
              }}
            >
              {analysisDoc.totals.delta < 0 ? '⚠️ 指标负向异动' : '🟢 指标平稳增长'}
            </span>
          </div>
          <h2 style={{ margin: '4px 0 6px', fontSize: '20px', fontFamily: 'var(--font-serif)', color: 'var(--ink)' }}>
            {analysisDoc.summary}
          </h2>

          <div className="kpis four" style={{ marginTop: '16px', marginBottom: '14px' }}>
            <div className="kpi">
              <div className="lab">本期分析值</div>
              <div className="num">{money(analysisDoc.totals.cur)}</div>
            </div>
            <div className="kpi">
              <div className="lab">上期对比基准</div>
              <div className="num">{money(analysisDoc.totals.prev)}</div>
            </div>
            <div className="kpi" style={{ background: analysisDoc.totals.delta < 0 ? '#fff1f2' : '#f0fdf4' }}>
              <div className="lab" style={{ color: analysisDoc.totals.delta < 0 ? '#9f1239' : '#166534' }}>变动差额 (Δ)</div>
              <div
                className="num"
                style={{
                  color:
                    analysisDoc.totals.delta < 0
                      ? 'var(--err)'
                      : 'var(--ok)',
                }}
              >
                {money(analysisDoc.totals.delta)}
              </div>
            </div>
            <div className="kpi">
              <div className="lab">实体覆盖率</div>
              <div className="num">{pct(analysisDoc.coverage)}</div>
            </div>
          </div>

          <div className="tiny" style={{ marginTop: '4px', color: 'var(--ink-4)' }}>
            时间窗口：<code>{analysisDoc.windows.cur_start} ~ {analysisDoc.windows.cur_end}</code> vs{' '}
            <code>{analysisDoc.windows.prev_start} ~ {analysisDoc.windows.prev_end}</code>
          </div>

          <div className="section-title" style={{ marginTop: '24px' }}>
            结论事实清单 (Findings & Contributions)
          </div>
          <div className="list">
            {analysisDoc.findings.map((f, i) => (
              <div key={i} className="list-row" style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
                <div className="grow">
                  <div className="t" style={{ fontWeight: 600, color: 'var(--ink)' }}>{f.what}</div>
                  <div className="s" style={{ marginTop: '2px' }}>
                    {f.kind === 'observed' ? '客观现象' : '模型量化贡献'} · 事实依据：{' '}
                    <code>{f.evidence.join('、')}</code>
                  </div>
                </div>
                {f.share != null && (
                  <div style={{ textAlign: 'right', minWidth: '140px' }}>
                    <div style={{ fontSize: '12px', fontWeight: 700, color: 'var(--accent-ink)' }}>
                      贡献度 {pct(f.share)}
                    </div>
                    <div style={{ width: '120px', height: '6px', background: 'var(--paper-3)', borderRadius: '3px', marginTop: '4px', overflow: 'hidden' }}>
                      <div
                        style={{
                          width: `${Math.min(100, Math.round(f.share * 100))}%`,
                          height: '100%',
                          background: 'var(--accent)',
                          borderRadius: '3px',
                        }}
                      />
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>

          <div className="section-title" style={{ marginTop: '24px' }}>
            下钻执行链 (IR ➔ 计划 ➔ 编译 ➔ 执行)
          </div>
          <div className="list">
            {analysisDoc.steps.map((s, i) => (
              <div key={i} className="list-row" style={{ alignItems: 'flex-start' }}>
                <div className="grow">
                  <div className="t" style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                    <span style={{ fontWeight: 700 }}>{s.title}</span>
                    <span className="tiny" style={{ background: 'var(--paper-3)', padding: '1px 6px', borderRadius: '4px' }}>
                      {s.kind} · {s.row_count} 行结果
                    </span>
                    {!s.ok && <span className="badge failed">失败</span>}
                  </div>
                  <div className="s" style={{ marginTop: '4px' }}>{s.intent}</div>
                  <details style={{ marginTop: '8px' }}>
                    <summary className="tiny" style={{ cursor: 'pointer', color: 'var(--accent-ink)', fontWeight: 600 }}>
                      检视此步骤生成的确定性 SQL
                    </summary>
                    <pre className="sqlbox" style={{ marginTop: '6px', padding: '10px 14px', borderRadius: '6px', background: '#0f172a', color: '#e2e8f0', fontSize: '11.5px', overflowX: 'auto' }}>{s.sql}</pre>
                  </details>
                </div>
              </div>
            ))}
          </div>

          <div className="hint" style={{ marginTop: '18px', lineHeight: '1.9', borderTop: '1px solid var(--paper-3)', paddingTop: '10px' }}>
            {analysisDoc.caveats.map((c, i) => (
              <div key={i}>· {c}</div>
            ))}
          </div>
        </div>
      )}
    </>
  );
};
