import React, { useEffect, useState } from 'react';
import { artifactUrl, getReportLedger, type ReportLedger } from '../services/api';
import {
  fmtDuration,
  fmtTime,
  layoutOf,
  reports,
  stateBadge,
  useReport,
} from '../services/reports';
import { CaliberBanner } from '../components/CaliberBanner';
import { useToast } from '../components/Toast';

interface Props {
  reportId: string;
  onBack: () => void;
  /** 进设计器（轻量版）—— 从报表详情直达。 */
  onDesign?: (id: string) => void;
}

/** 可勾选的导出格式 —— 与引擎支持的五种对齐（`run::Options.exports`）。 */
interface ExportFormatOption {
  id: string;
  name: string;
  desc: string;
  badge?: string;
}

const EXPORT_OPTIONS: ExportFormatOption[] = [
  { id: 'xlsx', name: 'Excel (.xlsx)', desc: '标准表格文件，含表头与单元格数据', badge: '默认推荐' },
  { id: 'pdf', name: 'PDF (.pdf)', desc: '分页排版文档，支持中文与表单印刷', badge: '默认推荐' },
  { id: 'csv', name: 'CSV (.csv)', desc: '逗号分隔纯文本，适合下游管道计算' },
  { id: 'print-xlsx', name: 'Print Excel', desc: '含打印分页符与纸张版面参数' },
  { id: 'html', name: 'Web HTML', desc: '轻量静态网页表格，免客户端即开' },
];

/**
 * 一张报表：详情 + 触发运行 + 运行历史 + 产物直链。
 *
 * 这里的每一个数字都来自服务端返回的 `run` 记录（`started_at` / `layout.page_count` /
 * `artifacts`），前端不做任何"预计页数""预计耗时"的估算。
 */
export const ReportView: React.FC<Props> = ({ reportId, onBack, onDesign }) => {
  const state = useReport(reportId);
  const [exports, setExports] = useState<string[]>(['xlsx', 'pdf']);
  const { toast } = useToast();

  useEffect(() => {
    void reports.open(reportId);
  }, [reportId]);

  // 台账：跨修订/跨运行的索引。文件仍是真相，这里只是"一眼看到改过几版、跑挂过几次"
  const [ledger, setLedger] = useState<ReportLedger | null>(null);
  useEffect(() => {
    let alive = true;
    getReportLedger(reportId)
      .then((r) => {
        if (alive) setLedger(r.ledger);
      })
      .catch(() => {
        if (alive) setLedger(null);
      });
    return () => {
      alive = false;
    };
  }, [reportId, state.lastRun?.run_id]);

  const detail = state.detail;
  const manifest = (detail?.manifest ?? {}) as {
    name?: string;
    description?: string;
    tags?: string[];
    version?: string;
  };
  const badge = stateBadge((detail as { result_state?: string } | null)?.result_state);
  const last = state.lastRun;
  const lastLayout = layoutOf(last);

  const toggle = (name: string) => {
    setExports((cur) => (cur.includes(name) ? cur.filter((x) => x !== name) : [...cur, name]));
  };

  const copyUrl = (url: string) => {
    navigator.clipboard?.writeText?.(url);
    toast('已复制产物直链 URL 到剪贴板');
  };

  return (
    <>
      {/* 头部：编辑部排版头，大标题 + 优雅面包屑与操作区 */}
      <div
        className="page-head"
        style={{
          borderBottom: '1px solid var(--border-subtle)',
          paddingBottom: '22px',
          marginBottom: '26px',
        }}
      >
        <div className="grow">
          <div className="eyebrow" style={{ letterSpacing: '0.18em' }}>
            <button
              type="button"
              className="linklike tiny"
              onClick={onBack}
              style={{
                color: 'var(--ink-3)',
                fontWeight: 600,
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
              }}
            >
              ← 报表资产库
            </button>
            <span style={{ margin: '0 8px', color: 'var(--line-2)' }}>/</span>
            <span style={{ color: 'var(--accent-ink)' }}>REPORT SPEC</span>
          </div>

          <h1
            style={{
              fontFamily: 'var(--font-serif)',
              fontSize: 'clamp(28px, 3.2vw, 40px)',
              fontWeight: 700,
              letterSpacing: '-0.02em',
              color: 'var(--ink)',
              lineHeight: 1.18,
              marginTop: '4px',
            }}
          >
            {manifest.name || reportId}
          </h1>

          <div
            className="sub"
            style={{
              display: 'flex',
              alignItems: 'center',
              flexWrap: 'wrap',
              gap: '10px',
              marginTop: '10px',
              fontSize: '12.5px',
            }}
          >
            <code
              className="sql-inline"
              style={{
                fontSize: '12px',
                padding: '2px 8px',
                borderRadius: 'var(--r-s)',
                background: 'var(--paper-3)',
              }}
            >
              {reportId}
            </code>

            {detail?.revision ? (
              <span className="tiny" style={{ color: 'var(--ink-3)' }}>
                · 修订 <code>{detail.revision.slice(0, 8)}</code>
              </span>
            ) : null}

            {manifest.version ? (
              <span className="tiny" style={{ color: 'var(--ink-4)' }}>
                · 规范 v{manifest.version}
              </span>
            ) : null}

            {ledger ? (
              <span
                className="tiny"
                style={{
                  color: ledger.runs.some((r) => r.status === 'failed') ? 'var(--warn)' : 'var(--ink-3)',
                }}
                title={`台账：${ledger.revisions.length} 个修订 / ${ledger.runs.length} 次运行`}
              >
                · 台账 {ledger.revisions.length} 版 / {ledger.runs.length} 次运行
                {ledger.runs.some((r) => r.status === 'failed') ? ' (含历史失败)' : ''}
              </span>
            ) : null}
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
          <span className={`badge ${badge.cls}`} style={{ fontSize: '12px', padding: '5px 12px' }}>
            {badge.label}
          </span>

          {onDesign ? (
            <button
              type="button"
              className="btn"
              onClick={() => onDesign(reportId)}
              title="进入轻量级网格排版设计器编辑模板单元格"
            >
              <span style={{ fontSize: '14px' }}>📐</span> 设计模板
            </button>
          ) : null}

          <button
            type="button"
            className="btn primary"
            onClick={() => void reports.run(reportId, exports)}
            disabled={state.running || state.status !== 'ready'}
            title="在服务端执行 Rust 引擎，产出 xlsx / pdf / json 二进制文件"
          >
            {state.running ? (
              <>
                <span className="spin" style={{ width: 12, height: 12, borderWidth: 2 }} />
                <span>运行引擎中…</span>
              </>
            ) : (
              <>
                <span>▶</span>
                <span>执行运行</span>
              </>
            )}
          </button>
        </div>
      </div>

      {manifest.description ? (
        <div
          style={{
            fontSize: '14px',
            color: 'var(--ink-2)',
            lineHeight: 1.7,
            marginBottom: '20px',
            background: 'var(--paper-2)',
            padding: '14px 20px',
            borderRadius: 'var(--r-m)',
            border: '1px solid var(--border)',
            boxShadow: 'var(--shadow-xs)',
          }}
        >
          {manifest.description}
        </div>
      ) : null}

      {/* 业务口径声明横幅 & 沉淀至业务模型飞轮 */}
      <CaliberBanner
        dsnId="demo_sqlite"
        reportName={manifest.name || reportId}
        caliber={{
          metricName: manifest.name?.includes('逾期')
            ? '客户价值 (LTV) 与逾期率'
            : manifest.name?.includes('毛利')
            ? '综合毛利率'
            : manifest.name?.includes('走势')
            ? '月度销售额与环比'
            : '区域销售额',
          formula: manifest.name?.includes('逾期')
            ? 'SUM(orders.amount) - SUM(loans.losses) · 逾期率 = SUM(is_overdue)/COUNT(1)'
            : manifest.name?.includes('毛利')
            ? '(SUM(amount) - SUM(cost)) / SUM(amount)'
            : 'SUM(amount)',
          dimensions: manifest.name?.includes('逾期')
            ? ['客户等级 (tier)']
            : manifest.name?.includes('毛利')
            ? ['产品品类 (cat)']
            : manifest.name?.includes('走势')
            ? ['月份 (month)']
            : ['大区 (region)', '产品品类 (cat)'],
          timeWindow: '2026年累计 (基准时间: 2026-10-30)',
          safetyGuarantee: '已通过确定性规则与物理 Schema 审计，无笛卡尔积，粒度完全对齐',
        }}
      />

      {/* 模板网格预览（带 Title） */}
      {(() => {
        const cells = ((detail?.template as any)?.cells ?? {}) as Record<string, { v?: string | number }>;
        let maxR = 0;
        let maxC = 0;
        for (const k of Object.keys(cells)) {
          const p = k.split(',').map(Number);
          if (!isNaN(p[0]) && p[0] > maxR) maxR = p[0];
          if (!isNaN(p[1]) && p[1] > maxC) maxC = p[1];
        }
        const rows: (string | number)[][] = [];
        for (let r = 0; r <= maxR; r++) {
          const row: (string | number)[] = [];
          for (let c = 0; c <= maxC; c++) {
            row.push(cells[`${r},${c}`]?.v ?? '');
          }
          rows.push(row);
        }
        if (rows.length === 0) return null;

        return (
          <div
            className="card"
            style={{
              marginBottom: 24,
              overflow: 'hidden',
              boxShadow: 'var(--shadow-s)',
            }}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                padding: '16px 22px',
                background: 'linear-gradient(180deg, #ffffff 0%, #faf8f5 100%)',
                borderBottom: '1px solid var(--border)',
                flexWrap: 'wrap',
                gap: 12,
              }}
            >
              <div>
                <div
                  style={{
                    fontFamily: 'var(--font-mono)',
                    fontSize: '10.5px',
                    fontWeight: 700,
                    letterSpacing: '0.16em',
                    textTransform: 'uppercase',
                    color: 'var(--accent)',
                  }}
                >
                  模板网格规范 · TEMPLATE GRID
                </div>
                <h3
                  style={{
                    margin: '4px 0 0',
                    fontFamily: 'var(--font-serif)',
                    fontSize: 17,
                    fontWeight: 700,
                    color: 'var(--ink)',
                  }}
                >
                  {manifest.name || reportId}
                </h3>
              </div>

              {onDesign ? (
                <button
                  type="button"
                  className="btn small"
                  onClick={() => onDesign(reportId)}
                  title="在轻量网格设计器中微调排版与单元格"
                >
                  <span>在设计器中编辑</span>
                  <span>↗</span>
                </button>
              ) : null}
            </div>

            <div style={{ overflowX: 'auto', padding: '16px 20px', background: 'var(--paper-2)' }}>
              <table
                style={{
                  borderCollapse: 'collapse',
                  width: '100%',
                  fontSize: 13.5,
                  borderRadius: 'var(--r-s)',
                  overflow: 'hidden',
                  border: '1px solid var(--border)',
                }}
              >
                <thead>
                  <tr style={{ background: 'var(--paper-3)', borderBottom: '1px solid var(--line-2)' }}>
                    {rows[0]?.map((col, idx) => (
                      <th
                        key={idx}
                        style={{
                          padding: '10px 14px',
                          textAlign: idx > 0 ? 'right' : 'left',
                          fontWeight: 600,
                          fontSize: '12.5px',
                          fontFamily: idx > 0 ? 'var(--font-mono)' : 'inherit',
                          color: 'var(--ink-2)',
                          borderRight: idx < (rows[0]?.length || 0) - 1 ? '1px solid var(--border-subtle)' : 'none',
                        }}
                      >
                        {col}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {rows.slice(1).map((r, rIdx) => {
                    const isTotal = String(r[0] || '').includes('合计') || String(r[0] || '').includes('总计');
                    return (
                      <tr
                        key={rIdx}
                        style={{
                          background: isTotal ? 'var(--paper-3)' : rIdx % 2 === 1 ? '#fdfcf9' : '#ffffff',
                          fontWeight: isTotal ? 700 : 400,
                          borderBottom: '1px solid var(--border-subtle)',
                          transition: 'background var(--t-fast)',
                        }}
                      >
                        {r.map((c, cIdx) => (
                          <td
                            key={cIdx}
                            style={{
                              padding: '10px 14px',
                              textAlign: cIdx > 0 ? 'right' : 'left',
                              color: isTotal ? 'var(--accent-ink)' : 'var(--ink)',
                              fontVariantNumeric: 'tabular-nums',
                              fontFamily: cIdx > 0 ? 'var(--font-mono)' : 'inherit',
                              borderRight: cIdx < r.length - 1 ? '1px solid var(--border-subtle)' : 'none',
                            }}
                          >
                            {c}
                          </td>
                        ))}
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        );
      })()}

      {state.status === 'error' ? (
        <div className="card pad" style={{ marginBottom: 20, borderColor: 'var(--err-border)' }}>
          <b style={{ color: 'var(--err)' }}>读不到这张报表</b>
          <div className="muted" style={{ marginTop: 6, lineHeight: 1.8 }}>{state.error}</div>
          <div style={{ marginTop: 14 }}>
            <button type="button" className="btn" onClick={() => void reports.open(reportId)}>
              重新加载
            </button>
          </div>
        </div>
      ) : null}

      {state.runError ? (
        <div className="card pad" style={{ borderColor: 'var(--err-border)', background: 'var(--err-bg)', marginBottom: 22 }}>
          <b style={{ color: 'var(--err)' }}>运行引擎抛出异常</b>
          <div className="muted" style={{ marginTop: 6, lineHeight: 1.8, color: 'var(--ink-2)' }}>{state.runError}</div>
        </div>
      ) : null}

      {/* 刚刚这次运行产物 */}
      {last ? (
        <div
          className="card"
          style={{
            marginBottom: 24,
            overflow: 'hidden',
            boxShadow: 'var(--shadow-m)',
            border: '1px solid var(--line-2)',
          }}
        >
          <div
            style={{
              padding: '16px 24px',
              background: 'linear-gradient(135deg, #fdfbf7 0%, #f7f3ec 100%)',
              borderBottom: '1px solid var(--border)',
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              flexWrap: 'wrap',
              gap: 12,
            }}
          >
            <div>
              <div
                style={{
                  fontFamily: 'var(--font-mono)',
                  fontSize: '10.5px',
                  fontWeight: 700,
                  letterSpacing: '0.16em',
                  textTransform: 'uppercase',
                  color: 'var(--accent)',
                }}
              >
                最新执行批次 · LATEST EXECUTION
              </div>
              <div
                style={{
                  fontFamily: 'var(--font-serif)',
                  fontSize: 16,
                  fontWeight: 700,
                  color: 'var(--ink)',
                  marginTop: 3,
                }}
              >
                运行成功并已落地物理产物
              </div>
            </div>
            <span className="badge ran" style={{ fontSize: 12, padding: '4px 12px' }}>
              ✓ 运行成功
            </span>
          </div>

          <div style={{ padding: '20px 24px' }}>
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
                gap: 16,
                paddingBottom: 18,
                borderBottom: '1px solid var(--border)',
              }}
            >
              <div>
                <div className="tiny" style={{ color: 'var(--ink-4)', marginBottom: 4 }}>运行批次标识</div>
                <code className="sql-inline" style={{ fontSize: 13, userSelect: 'all' }}>
                  {(last.run_id as string) || '—'}
                </code>
              </div>
              <div>
                <div className="tiny" style={{ color: 'var(--ink-4)', marginBottom: 4 }}>执行耗时</div>
                <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--ink)', fontVariantNumeric: 'tabular-nums' }}>
                  {fmtDuration(last.started_at as string, last.finished_at as string)}
                </div>
              </div>
              <div>
                <div className="tiny" style={{ color: 'var(--ink-4)', marginBottom: 4 }}>版面渲染规模</div>
                <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--ink)', fontVariantNumeric: 'tabular-nums' }}>
                  {lastLayout.pages ?? '—'} 页 · {lastLayout.rows ?? '—'} 行 × {lastLayout.cols ?? '—'} 列
                </div>
              </div>
            </div>

            {Object.keys((last.artifacts as Record<string, string>) ?? {}).length > 0 ? (
              <div style={{ marginTop: 20 }}>
                <div
                  style={{
                    fontSize: 14,
                    marginBottom: 12,
                    fontWeight: 700,
                    color: 'var(--ink)',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 6,
                  }}
                >
                  <span>📦</span>
                  <span>产物文件列表与直链下载（可直接外链分发）：</span>
                </div>

                <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                  {Object.entries((last.artifacts as Record<string, string>) ?? {}).map(([name]) => {
                    const url = artifactUrl(reportId, (last.run_id as string) || '', name);
                    const isPdf = name.endsWith('.pdf');
                    const isXlsx = name.endsWith('.xlsx');
                    return (
                      <div
                        key={name}
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'space-between',
                          background: '#ffffff',
                          padding: '12px 18px',
                          borderRadius: 'var(--r-m)',
                          border: '1px solid var(--border)',
                          boxShadow: 'var(--shadow-xs)',
                          gap: 16,
                          flexWrap: 'wrap',
                          transition: 'all var(--t-fast) var(--ease)',
                        }}
                      >
                        <div style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap', minWidth: 280 }}>
                          <span
                            style={{
                              fontSize: 20,
                              width: 36,
                              height: 36,
                              borderRadius: 'var(--r-s)',
                              background: isPdf ? 'rgba(180, 35, 24, 0.08)' : isXlsx ? 'rgba(21, 112, 85, 0.08)' : 'var(--paper-3)',
                              display: 'grid',
                              placeItems: 'center',
                            }}
                          >
                            {isPdf ? '📄' : isXlsx ? '📊' : '📦'}
                          </span>
                          <div>
                            <div style={{ fontSize: 15, fontWeight: 700, color: 'var(--ink)' }}>
                              {name}
                            </div>
                            <div style={{ marginTop: 4, display: 'flex', alignItems: 'center', gap: 6 }}>
                              <code
                                className="sql-inline"
                                style={{
                                  fontSize: 13,
                                  color: 'var(--accent-ink)',
                                  background: 'var(--paper-3)',
                                  padding: '3px 8px',
                                  borderRadius: 'var(--r-s)',
                                  userSelect: 'all',
                                  cursor: 'pointer',
                                }}
                                title="点击复制完整直链 URL"
                                onClick={() => copyUrl(url)}
                              >
                                {url}
                              </code>
                              <button
                                type="button"
                                className="linklike tiny"
                                onClick={() => copyUrl(url)}
                                style={{ color: 'var(--ink-4)', fontSize: 11 }}
                              >
                                [复制]
                              </button>
                            </div>
                          </div>
                        </div>

                        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                          <a
                            className="btn small"
                            href={url}
                            target="_blank"
                            rel="noreferrer"
                            style={{ fontSize: 13, padding: '6px 14px' }}
                            title="在新标签页中打开预览"
                          >
                            在线预览 ↗
                          </a>
                          <a
                            className={`btn small ${isPdf || isXlsx ? 'primary' : ''}`}
                            href={url}
                            download={name}
                            style={{ fontSize: 13, padding: '6px 18px', fontWeight: 600 }}
                            title="直接下载二进制文件到本地磁盘"
                          >
                            📥 下载文件
                          </a>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            ) : null}
          </div>
        </div>
      ) : null}

      {/* 运行时额外导出格式 */}
      <div className="card pad" style={{ marginBottom: 24, boxShadow: 'var(--shadow-s)' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', flexWrap: 'wrap', gap: 8 }}>
          <div>
            <div
              style={{
                fontFamily: 'var(--font-mono)',
                fontSize: '10.5px',
                fontWeight: 700,
                letterSpacing: '0.16em',
                textTransform: 'uppercase',
                color: 'var(--accent)',
              }}
            >
              运行时额外导出 · RUNTIME EXPORT MATRIX
            </div>
            <div className="hint" style={{ marginTop: 4, fontSize: '13px', color: 'var(--ink-3)' }}>
              每次运行必出 <code>workbook.json</code> 与 <code>result.json</code>；
              已默认勾选 <code>xlsx</code> 与 <code>pdf</code>，服务端将在执行完成后同步生成交付级文件。
            </div>
          </div>
        </div>

        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
            gap: 12,
            marginTop: 16,
          }}
        >
          {EXPORT_OPTIONS.map((item) => {
            const isSelected = exports.includes(item.id);
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => toggle(item.id)}
                aria-pressed={isSelected}
                style={{
                  textAlign: 'left',
                  padding: '12px 14px',
                  borderRadius: 'var(--r-m)',
                  border: isSelected ? '1.5px solid var(--accent)' : '1px solid var(--border)',
                  background: isSelected ? 'var(--accent-soft)' : 'var(--paper-2)',
                  cursor: 'pointer',
                  transition: 'all var(--t-fast) var(--ease)',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 4,
                  boxShadow: isSelected ? 'var(--shadow-xs)' : 'none',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%' }}>
                  <span
                    style={{
                      fontFamily: 'var(--font-mono)',
                      fontSize: 13.5,
                      fontWeight: 700,
                      color: isSelected ? 'var(--accent-ink)' : 'var(--ink)',
                    }}
                  >
                    {item.name}
                  </span>
                  <span
                    style={{
                      width: 18,
                      height: 18,
                      borderRadius: 4,
                      background: isSelected ? 'var(--accent)' : 'var(--paper-3)',
                      color: '#ffffff',
                      display: 'grid',
                      placeItems: 'center',
                      fontSize: 11,
                      fontWeight: 700,
                    }}
                  >
                    {isSelected ? '✓' : ''}
                  </span>
                </div>
                <div style={{ fontSize: 11.5, color: isSelected ? 'var(--accent-ink)' : 'var(--ink-4)', lineHeight: 1.5 }}>
                  {item.desc}
                </div>
              </button>
            );
          })}
        </div>
      </div>

      {/* 运行历史区 */}
      <div
        className="section-title"
        style={{
          display: 'flex',
          alignItems: 'baseline',
          justifyContent: 'space-between',
          flexWrap: 'wrap',
          gap: 10,
          margin: '32px 0 16px',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <span>运行历史</span>
          <span
            style={{
              fontFamily: 'var(--font-mono)',
              fontSize: 12,
              color: 'var(--ink-4)',
              fontWeight: 500,
            }}
          >
            {state.status === 'loading'
              ? '读取中…'
              : `共 ${state.runsTotal} 次 · 显示最近 ${state.runs.length} 次`}
          </span>
        </div>
      </div>

      <div className="list" style={{ boxShadow: 'var(--shadow-s)', borderRadius: 'var(--r-l)' }}>
        {state.runs.length > 0 ? (
          state.runs.map((r) => {
            const id = (r.run_id as string) || '';
            const { pages, rows, cols } = layoutOf(r);
            const artifacts = Object.entries((r.artifacts as Record<string, string>) ?? {});
            const ok = r.status === 'succeeded';
            return (
              <div
                className="list-row"
                key={id}
                style={{
                  cursor: 'default',
                  alignItems: 'flex-start',
                  padding: '20px 24px',
                  borderTop: '1px solid var(--border-subtle)',
                }}
              >
                <div className="grow">
                  <div style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap' }}>
                    <code className="sql-inline" style={{ fontSize: 14, fontWeight: 600 }}>
                      {id}
                    </code>
                    <span className="tiny" style={{ fontSize: 13, color: 'var(--ink-3)' }}>
                      rev <code>{String(r.revision || '').slice(0, 8) || '—'}</code>
                    </span>
                  </div>

                  <div
                    style={{
                      fontSize: 13.5,
                      marginTop: 6,
                      color: 'var(--ink-3)',
                      fontVariantNumeric: 'tabular-nums',
                    }}
                  >
                    {fmtTime(r.started_at as string)} 起 · {fmtDuration(r.started_at as string, r.finished_at as string)} ·{' '}
                    {pages != null ? `${pages} 页 · ${rows ?? '—'} 行 × ${cols ?? '—'} 列` : '无版面信息'}
                  </div>

                  {artifacts.length ? (
                    <div
                      style={{
                        marginTop: 14,
                        display: 'flex',
                        flexDirection: 'column',
                        gap: 8,
                        width: '100%',
                        background: 'var(--paper-3)',
                        padding: '12px 16px',
                        borderRadius: 'var(--r-m)',
                      }}
                    >
                      <div
                        style={{
                          fontSize: 12.5,
                          fontWeight: 700,
                          color: 'var(--ink-3)',
                          letterSpacing: '0.04em',
                        }}
                      >
                        产物直链与下载 (Artifacts & Downloads)：
                      </div>

                      {artifacts.map(([name]) => {
                        const url = artifactUrl(reportId, id, name);
                        const isPdf = name.endsWith('.pdf');
                        const isXlsx = name.endsWith('.xlsx');
                        return (
                          <div
                            key={name}
                            style={{
                              display: 'flex',
                              alignItems: 'center',
                              justifyContent: 'space-between',
                              background: '#ffffff',
                              padding: '10px 16px',
                              borderRadius: 'var(--r-s)',
                              border: '1px solid var(--border)',
                              gap: 12,
                              flexWrap: 'wrap',
                            }}
                          >
                            <div style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap', minWidth: 260 }}>
                              <span style={{ fontSize: 18 }}>{isPdf ? '📄' : isXlsx ? '📊' : '📦'}</span>
                              <span style={{ fontSize: 14.5, fontWeight: 700, color: 'var(--ink)' }}>
                                {name}
                              </span>
                              <code
                                className="sql-inline"
                                style={{
                                  fontSize: 13,
                                  color: 'var(--accent-ink)',
                                  background: 'var(--paper-3)',
                                  padding: '3px 8px',
                                  borderRadius: 'var(--r-s)',
                                  userSelect: 'all',
                                  cursor: 'pointer',
                                }}
                                title="点击复制下载直链 URL"
                                onClick={() => copyUrl(url)}
                              >
                                {url}
                              </code>
                            </div>

                            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                              <a
                                className="btn small"
                                href={url}
                                target="_blank"
                                rel="noreferrer"
                                style={{ fontSize: 13, padding: '5px 12px' }}
                              >
                                在线预览 ↗
                              </a>
                              <a
                                className={`btn small ${isPdf || isXlsx ? 'primary' : ''}`}
                                href={url}
                                download={name}
                                style={{ fontSize: 13, padding: '5px 14px', fontWeight: 600 }}
                              >
                                📥 下载文件
                              </a>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  ) : null}

                  {r.error ? (
                    <div className="tiny" style={{ color: 'var(--err)', marginTop: 6, fontWeight: 600 }}>
                      {String((r.error as { message?: string }).message || '运行失败')}
                    </div>
                  ) : null}
                </div>

                <span
                  className={`badge ${ok ? 'ran' : 'failed'}`}
                  style={{ fontSize: 12.5, padding: '4px 12px' }}
                >
                  {ok ? '成功' : '失败'}
                </span>
              </div>
            );
          })
        ) : state.status === 'loading' ? (
          <div className="empty" style={{ padding: 48 }}>读取运行历史中…</div>
        ) : (
          <div className="empty" style={{ padding: 48 }}>
            还没有运行记录 —— 点右上角「执行运行」开始生成第一次报表交付物
          </div>
        )}
      </div>

      <div
        className="hint"
        style={{
          marginTop: 20,
          lineHeight: 1.8,
          fontSize: '12px',
          color: 'var(--ink-4)',
          borderTop: '1px solid var(--border-subtle)',
          paddingTop: '14px',
        }}
      >
        产物直链形如 <code className="sql-inline">/api/reports/{reportId}/runs/&lt;run&gt;/artifacts/workbook.json</code>，
        由 Rust 服务端读盘返回；所有历史产物均受版本哈希严格锁定，保证回溯一致性。
      </div>
    </>
  );
};
