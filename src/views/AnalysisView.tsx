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
          <div className="eyebrow">Analysis</div>
          <h1>归因分析</h1>
          <div className="sub">
            「为什么降了」—— 拆成几步受控查询（整体 → 按维度 → 再钻一层），每一步都留 SQL
          </div>
        </div>
      </div>

      <div className="card pad">
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
            style={{ flex: '0 0 150px' }}
          >
            <option value="amount">销售额 (SUM)</option>
            <option value="qty">销售数量 (SUM)</option>
          </select>

          <select
            value={timeField}
            onChange={(e) => setTimeField(e.target.value)}
            style={{ flex: '0 0 150px' }}
          >
            <option value="month">月份维度 (month)</option>
            <option value="date">日期维度 (date)</option>
          </select>

          <select
            value={grain}
            onChange={(e) => setGrain(e.target.value)}
            style={{ flex: '0 0 100px' }}
          >
            <option value="day">按日</option>
            <option value="week">按周</option>
            <option value="month">按月</option>
            <option value="quarter">按季</option>
            <option value="year">按年</option>
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
          >
            {loading ? '拆解中…' : '拆解 →'}
          </button>
        </div>

        <div className="hint" style={{ marginTop: '10px', lineHeight: '1.8' }}>
          窗口取「上一个完整周期」（拿半个月对半个月出的下降是假的）；对比期 = 等长前移，由编译期算好，SQL
          里只有两个日期参数。维度由业务模型自动挑两个文本维度。
        </div>
      </div>

      {analysisDoc && (
        <div className="card pad" style={{ marginTop: '18px' }}>
          <div className="eyebrow">Conclusion</div>
          <h2 style={{ margin: '6px 0 4px', fontSize: '19px' }}>
            {analysisDoc.summary}
          </h2>

          <div className="kpis four" style={{ marginTop: '14px' }}>
            <div className="kpi">
              <div className="lab">本期</div>
              <div className="num">{money(analysisDoc.totals.cur)}</div>
            </div>
            <div className="kpi">
              <div className="lab">上期</div>
              <div className="num">{money(analysisDoc.totals.prev)}</div>
            </div>
            <div className="kpi">
              <div className="lab">变化</div>
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
              <div className="lab">覆盖度</div>
              <div className="num">{pct(analysisDoc.coverage)}</div>
            </div>
          </div>

          <div className="tiny" style={{ marginTop: '10px' }}>
            窗口 {analysisDoc.windows.cur_start} ~ {analysisDoc.windows.cur_end} vs{' '}
            {analysisDoc.windows.prev_start} ~ {analysisDoc.windows.prev_end}
          </div>

          <div className="section-title">结论事实清单</div>
          <div className="list">
            {analysisDoc.findings.map((f, i) => (
              <div key={i} className="list-row">
                <div className="grow">
                  <div className="t">{f.what}</div>
                  <div className="s">
                    {f.kind === 'observed' ? '看到的现象' : '算出来的贡献'} · 依据{' '}
                    {f.evidence.join('、')}
                  </div>
                </div>
                {f.share != null && (
                  <span
                    className={`badge ${
                      f.kind === 'computed' ? 'compiled' : 'draft'
                    }`}
                  >
                    贡献度 {pct(f.share)}
                  </span>
                )}
              </div>
            ))}
          </div>

          <div className="section-title">
            怎么拆的 <span className="tiny">每步都是 IR → 计划 → 编译 → 执行</span>
          </div>
          <div className="list">
            {analysisDoc.steps.map((s, i) => (
              <div key={i} className="list-row" style={{ alignItems: 'flex-start' }}>
                <div className="grow">
                  <div className="t">
                    {s.title}{' '}
                    <span className="tiny">
                      {s.kind} · {s.row_count} 行
                    </span>
                    {!s.ok && <span className="badge failed">失败</span>}
                  </div>
                  <div className="s">{s.intent}</div>
                  <details style={{ marginTop: '6px' }}>
                    <summary className="tiny">看这一步的 SQL 语句</summary>
                    <pre className="sqlbox">{s.sql}</pre>
                  </details>
                </div>
              </div>
            ))}
          </div>

          <div className="hint" style={{ marginTop: '16px', lineHeight: '1.9' }}>
            {analysisDoc.caveats.map((c, i) => (
              <div key={i}>· {c}</div>
            ))}
          </div>
        </div>
      )}
    </>
  );
};
