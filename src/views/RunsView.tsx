import React, { useState } from 'react';
import { store } from '../services/store';
import { useToast } from '../components/Toast';
import { Run } from '../types';
import { UniverViewer } from '../components/UniverViewer';

interface Props {
  reqId?: string;
  initialRunId?: string;
  onBack?: () => void;
  onOpenRequirement: (id: string) => void;
}

export const RunsView: React.FC<Props> = ({
  reqId,
  initialRunId,
  onBack,
  onOpenRequirement,
}) => {
  const { toast } = useToast();
  const allRuns = store.getRuns(reqId);
  const targetReq = reqId ? store.getRequirement(reqId) : undefined;

  const [selectedRunId, setSelectedRunId] = useState<string>(
    initialRunId || allRuns[0]?.run_id || ''
  );
  const [viewTab, setViewTab] = useState<'univer' | 'print' | 'table' | 'json'>('univer');

  const selectedRun: Run | undefined =
    allRuns.find((r) => r.run_id === selectedRunId) || allRuns[0];

  const handleRunAgain = () => {
    if (!targetReq) return;
    const { run } = store.runRequirement(targetReq.id);
    setSelectedRunId(run.run_id);
    toast(`再次执行完成：${run.run_id} · ${run.duration_ms} ms`);
  };

  const handleDownload = (name: string, content: any) => {
    const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    a.click();
    URL.revokeObjectURL(url);
    toast(`已下载 ${name}`);
  };

  const fmtTime = (iso?: string) => {
    if (!iso) return '—';
    const d = new Date(iso);
    if (isNaN(d.getTime())) return iso;
    const p = (n: number) => String(n).padStart(2, '0');
    return `${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
  };

  const fmtMs = (ms?: number) => {
    if (ms == null) return '—';
    return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;
  };

  const sizeHint = (name: string) => {
    const map: Record<string, string> = {
      'report.csv': '表格 · UTF-8',
      'report.xlsx': '工作簿 · 公式保留',
      'report.html': '打印版 · 可直接发',
      'report-print.xlsx': '套打版式 · 分页',
      'snapshot.json': 'Univer · 官方标准快照',
    };
    return map[name] || '产物';
  };

  return (
    <>
      {reqId && (
        <div className="row" style={{ marginBottom: '10px', gap: '8px' }}>
          <a
            href="#"
            className="tiny"
            onClick={(e) => {
              e.preventDefault();
              onOpenRequirement(reqId);
            }}
          >
            ← 需求
          </a>
          <span className="tiny">/</span>
          <span className="tiny">运行中心</span>
        </div>
      )}

      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">{reqId ? 'Run Detail' : 'Archive'}</div>
          <h1 style={{ fontSize: '24px' }}>
            {reqId ? `${targetReq?.title || '需求'} · 运行中心` : '运行记录'}
          </h1>
          <div className="sub">
            {reqId
              ? '同一定义可以换数据/参数反复跑；每次运行都留档'
              : '所有需求的运行留档：同一定义换数据/参数反复跑，每次都可回溯、可下载'}
          </div>
        </div>
        {reqId && (
          <button
            type="button"
            className="btn small primary"
            onClick={() => onOpenRequirement(reqId)}
          >
            回需求页
          </button>
        )}
      </div>

      <div className="runs">
        {/* Left: Runs List */}
        <div className="card" style={{ overflow: 'hidden' }}>
          {allRuns.length > 0 ? (
            allRuns.map((x) => (
              <div
                key={x.run_id}
                className={`run-row ${
                  selectedRun && x.run_id === selectedRun.run_id ? 'sel' : ''
                }`}
                style={{ cursor: 'pointer' }}
                onClick={() => setSelectedRunId(x.run_id)}
              >
                <div className="row">
                  <div className="t">{x.run_id}</div>
                  <div style={{ marginLeft: 'auto' }}>
                    <span
                      className={`badge ${
                        x.status === 'succeeded' ? 'ran' : 'failed'
                      }`}
                    >
                      {x.status === 'succeeded' ? '成功' : '失败'}
                    </span>
                  </div>
                </div>
                <div className="m">
                  {fmtTime(x.finished_at)} · 模板 rev {x.template_rev ?? '—'} ·{' '}
                  {fmtMs(x.duration_ms)}
                </div>
                <div className="m">
                  {x.shape?.rows ?? '?'} 行 × {x.shape?.cols ?? '?'} 列 · 产物{' '}
                  {Object.keys(x.artifacts || {}).filter((n) => !n.endsWith('.json')).length}{' '}
                  件
                </div>
              </div>
            ))
          ) : (
            <div className="empty">还没有运行记录</div>
          )}
        </div>

        {/* Right: Selected Run Detail */}
        <div className="card pad">
          {selectedRun ? (
            <>
              <div className="row" style={{ alignItems: 'flex-start' }}>
                <div>
                  <div style={{ fontSize: '16px', fontWeight: 700 }}>
                    运行 {selectedRun.run_id}
                  </div>
                  <div className="tiny" style={{ marginTop: '4px' }}>
                    模板 rev {selectedRun.template_rev ?? '—'} ·{' '}
                    {fmtTime(selectedRun.finished_at)} · 耗时 {fmtMs(selectedRun.duration_ms)} ·
                    引擎 {selectedRun.engine_version}
                  </div>
                </div>
                <div style={{ marginLeft: 'auto', display: 'flex', gap: '8px' }}>
                  {targetReq && (
                    <button
                      type="button"
                      className="btn small"
                      onClick={handleRunAgain}
                    >
                      再跑一次
                    </button>
                  )}
                </div>
              </div>

              {/* Artifacts */}
              <div className="sec" style={{ marginTop: '16px' }}>
                <div className="tiny">产物文件（点卡片下载）</div>
                <div className="arts" style={{ marginTop: '8px' }}>
                  {Object.keys(selectedRun.artifacts || {})
                    .map((name) => (
                      <div
                        key={name}
                        className="art"
                        style={{
                          cursor: 'pointer',
                          borderColor: name === 'snapshot.json' ? '#10b981' : undefined,
                          background: name === 'snapshot.json' ? '#f0fdf4' : undefined,
                        }}
                        onClick={() => handleDownload(name, selectedRun.artifacts[name])}
                      >
                        <b style={{ color: name === 'snapshot.json' ? '#047857' : undefined }}>
                          {name}
                        </b>
                        <span>{sizeHint(name)}</span>
                        <span style={{ color: name === 'snapshot.json' ? '#059669' : 'var(--accent)', fontWeight: 600 }}>
                          下载 ↓
                        </span>
                      </div>
                    ))}
                </div>
              </div>

              {/* Preview */}
              <div className="sec" style={{ marginTop: '16px' }}>
                <div className="row" style={{ alignItems: 'center', flexWrap: 'wrap', gap: '8px' }}>
                  <div className="tiny">
                    {viewTab === 'univer'
                      ? 'Univer Sheets 交互式电子表格（可点选单元格、看公式、查求和与冻结表头）'
                      : viewTab === 'print'
                      ? '打印视图快照（与下载的 report.html 完全同源）'
                      : viewTab === 'table'
                      ? '结构化表格快照'
                      : 'Univer IWorkbookData 快照 JSON 结构'}
                  </div>
                  <div className="seg" style={{ marginLeft: 'auto', display: 'flex', gap: '4px' }}>
                    <button
                      type="button"
                      className={`tog ${viewTab === 'univer' ? 'on' : ''}`}
                      style={{
                        background: viewTab === 'univer' ? '#10b981' : undefined,
                        color: viewTab === 'univer' ? '#ffffff' : undefined,
                        fontWeight: 600,
                      }}
                      onClick={() => setViewTab('univer')}
                    >
                      ★ Univer 电子表格
                    </button>
                    <button
                      type="button"
                      className={`tog ${viewTab === 'print' ? 'on' : ''}`}
                      onClick={() => setViewTab('print')}
                    >
                      打印排版
                    </button>
                    <button
                      type="button"
                      className={`tog ${viewTab === 'table' ? 'on' : ''}`}
                      onClick={() => setViewTab('table')}
                    >
                      数据表格
                    </button>
                    <button
                      type="button"
                      className={`tog ${viewTab === 'json' ? 'on' : ''}`}
                      onClick={() => setViewTab('json')}
                    >
                      快照契约 JSON
                    </button>
                  </div>
                </div>

                {viewTab === 'univer' ? (
                  <div style={{ marginTop: '10px' }}>
                    <UniverViewer
                      snapshot={selectedRun.univerSnapshot}
                      title={selectedRun.req_title}
                      height="440px"
                      onDownloadSnapshot={() =>
                        handleDownload('snapshot.json', selectedRun.artifacts['snapshot.json'])
                      }
                    />
                  </div>
                ) : viewTab === 'print' ? (
                  <div
                    style={{
                      marginTop: '10px',
                      maxHeight: '480px',
                      overflowY: 'auto',
                      border: '1px solid var(--line)',
                      borderRadius: '8px',
                      background: '#fffcf7',
                      padding: '16px',
                    }}
                    dangerouslySetInnerHTML={{
                      __html: selectedRun.artifacts['report.html'] || '',
                    }}
                  />
                ) : viewTab === 'table' ? (
                  <div
                    style={{
                      marginTop: '10px',
                      maxHeight: '420px',
                      overflowY: 'auto',
                    }}
                  >
                    {selectedRun.computedData ? (
                      <table className="cols">
                        <thead>
                          <tr>
                            {selectedRun.computedData.headers.map((h) => (
                              <th key={h}>{h}</th>
                            ))}
                          </tr>
                        </thead>
                        <tbody>
                          {selectedRun.computedData.rows.map((r, i) => (
                            <tr key={i}>
                              {r.map((v, j) => (
                                <td key={j}>
                                  {typeof v === 'number'
                                    ? v.toLocaleString('zh-CN')
                                    : String(v)}
                                </td>
                              ))}
                            </tr>
                          ))}
                          {selectedRun.computedData.summaryRow && (
                            <tr style={{ fontWeight: 'bold', background: 'var(--paper-3)' }}>
                              {selectedRun.computedData.summaryRow.map((v, j) => (
                                <td key={j}>
                                  {typeof v === 'number'
                                    ? v.toLocaleString('zh-CN')
                                    : String(v)}
                                </td>
                              ))}
                            </tr>
                          )}
                        </tbody>
                      </table>
                    ) : (
                      <div className="empty">暂无明细数据</div>
                    )}
                  </div>
                ) : (
                  <div
                    style={{
                      marginTop: '10px',
                      maxHeight: '420px',
                      overflowY: 'auto',
                      background: '#1e293b',
                      borderRadius: '8px',
                      padding: '14px',
                    }}
                  >
                    <pre
                      style={{
                        margin: 0,
                        color: '#38bdf8',
                        fontSize: '12px',
                        fontFamily: 'Consolas, monospace',
                      }}
                    >
                      {selectedRun.artifacts['snapshot.json'] ||
                        JSON.stringify(selectedRun.univerSnapshot, null, 2)}
                    </pre>
                  </div>
                )}
              </div>

              {/* Execution Info */}
              <div className="sec" style={{ marginTop: '16px' }}>
                <div className="tiny">参数与可验证凭证</div>
                <div className="kv" style={{ marginTop: '4px' }}>
                  模板 rev {selectedRun.template_rev ?? '—'} · 分页{' '}
                  {selectedRun.layout?.page_count ?? 1} 页 · 参数{' '}
                  {JSON.stringify(selectedRun.parameters || {})}
                </div>
                <div className="tiny" style={{ marginTop: '4px' }}>
                  输入数据哈希：
                  {(selectedRun.inputs || []).map((i) => `${i.kind}:${i.dataset || 'main'} (${i.sha256})`).join(' · ')}
                </div>
              </div>
            </>
          ) : (
            <div className="empty">请在左侧选择一次运行查看详情</div>
          )}
        </div>
      </div>
    </>
  );
};
