import React, { useState } from 'react';
import { store } from '../services/store';
import { useToast } from '../components/Toast';
import { TemplateCell, CanonicalReportDoc } from '../types';
import { UniverViewer } from '../components/UniverViewer';
import { buildCanonicalReport } from '../services/reportEngine';

interface Props {
  reqId: string;
  onBack: () => void;
  onOpenRuns: (id: string, runId?: string) => void;
}

type ProjectionTab = 'design' | 'univer' | 'web' | 'print' | 'json' | 'export';

export const DesignerView: React.FC<Props> = ({ reqId, onBack, onOpenRuns }) => {
  const { toast } = useToast();
  const req = store.getRequirement(reqId);

  const [activeTab, setActiveTab] = useState<ProjectionTab>('design');
  const [selectedCoord, setSelectedCoord] = useState<{ row: number; col: number } | null>({
    row: 2,
    col: 1,
  });
  const [searchTerm, setSearchTerm] = useState('');
  const [copiedJson, setCopiedJson] = useState(false);

  if (!req || !req.template) {
    return (
      <div className="card pad">
        <b>还没有报表模板</b>
        <div className="muted" style={{ marginTop: '6px' }}>
          先在需求页「编译出稿」或让 Agent 自动解析生成报表模板。
        </div>
        <div style={{ marginTop: '12px' }}>
          <button type="button" className="btn" onClick={onBack}>
            ← 返回需求
          </button>
        </div>
      </div>
    );
  }

  const tpl = req.template;
  const grid = tpl.grid || { rows: 4, cols: 4 };

  const cellMap = new Map<string, TemplateCell>();
  (tpl.cells || []).forEach((c) => cellMap.set(`${c.row},${c.col}`, c));

  const covered = new Set<string>();
  const rowspans: Record<string, number> = {};
  const colspans: Record<string, number> = {};

  (tpl.cells || []).forEach((c) => {
    if (c.merge && (c.merge.rows > 1 || c.merge.cols > 1)) {
      const rs = c.merge.rows || 1;
      const cs = c.merge.cols || 1;
      rowspans[`${c.row},${c.col}`] = rs;
      colspans[`${c.row},${c.col}`] = cs;
      for (let r = c.row; r < c.row + rs; r++) {
        for (let col = c.col; col < c.col + cs; col++) {
          if (r !== c.row || col !== c.col) covered.add(`${r},${col}`);
        }
      }
    }
  });

  const selectedCell = selectedCoord
    ? (tpl.cells || []).find((c) => c.row === selectedCoord.row && c.col === selectedCoord.col)
    : null;

  const handleCellClick = (r: number, c: number) => {
    setSelectedCoord({ row: r, col: c });
  };

  const handleSave = () => {
    toast(`已保存模板定义（rev ${tpl.rev}）`);
  };

  const handleRun = () => {
    const { run } = store.runRequirement(req.id);
    toast(`引擎执行完成：${run.status} · 共生成 ${run.shape.rows} 行 × ${run.shape.cols} 列`);
    setActiveTab('univer');
  };

  const handleRecompileFormulas = () => {
    store.recompileFormulasByAgent(req.id);
    const updated = store.getRequirement(req.id);
    const formulaCount = updated?.template?.cells.filter((c) => !!c.formula).length || 0;
    toast(`🤖 Agent 已完成公式编译：成功推导 ${formulaCount} 个聚合公式与审计依据`);
  };

  const handleVerifyHuman = () => {
    store.verifyRequirementTemplate(req.id, '人类数据专家 (Lead Reviewer)');
    toast('✓ 人类专家审核核准：报表结构与 Agent 编译公式已锁定通过！');
  };

  const handleUnverify = () => {
    if (req.template) {
      req.template.verified = false;
      req.status = 'compiled';
      req.status_label = '待人类审核';
      store.notify();
      toast('已解除核准锁定，可重新修改网格或重新编译公式');
    }
  };

  const updateSelectedCell = (patch: Partial<TemplateCell>) => {
    if (!selectedCoord) return;
    store.patchTemplateCells(req.id, [{ row: selectedCoord.row, col: selectedCoord.col, ...patch }]);
  };

  const canonicalDoc: CanonicalReportDoc = buildCanonicalReport(req);
  const canonicalJsonString = JSON.stringify(canonicalDoc, null, 2);

  const handleCopyJson = () => {
    navigator.clipboard.writeText(canonicalJsonString);
    setCopiedJson(true);
    toast('已复制统一规范 JSON 报表定义');
    setTimeout(() => setCopiedJson(false), 2000);
  };

  const handleDownloadJson = () => {
    const blob = new Blob([canonicalJsonString], { type: 'application/json;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `report-${req.id}-canonical.json`;
    a.click();
    URL.revokeObjectURL(url);
    toast('已下载统一规范 JSON 报表');
  };

  const handleExportCsv = () => {
    const latestRun = req.runs[0] || store.runRequirement(req.id).run;
    if (!latestRun.computedData) {
      toast('暂无可导出的计算数据');
      return;
    }
    const { headers, rows, summaryRow } = latestRun.computedData;
    const all = [headers, ...rows, ...(summaryRow ? [summaryRow] : [])];
    const csvContent = all.map((r) => r.map((c: any) => `"${String(c ?? '').replace(/"/g, '""')}"`).join(',')).join('\n');
    const blob = new Blob(['\uFEFF' + csvContent], { type: 'text/csv;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${req.title}.csv`;
    a.click();
    URL.revokeObjectURL(url);
    toast('已导出 CSV / Excel 兼容文件');
  };

  const latestRun = req.runs[0];
  const formulaCells = (tpl.cells || []).filter((c) => !!c.formula);

  return (
    <>
      {/* Breadcrumb Navigation */}
      <div className="row" style={{ marginBottom: '10px', gap: '8px' }}>
        <a
          href="#"
          className="tiny"
          onClick={(e) => {
            e.preventDefault();
            onBack();
          }}
        >
          ← 返回需求详情
        </a>
        <span className="tiny">/</span>
        <span className="tiny" style={{ fontWeight: 600 }}>报表设计器 & 多表层投影</span>
      </div>

      {/* Human-in-the-Loop Verification Callout Banner */}
      <div
        style={{
          marginBottom: '14px',
          padding: '12px 16px',
          borderRadius: '8px',
          background: tpl.verified ? '#f0fdf4' : '#fffbeb',
          border: `1px solid ${tpl.verified ? '#86efac' : '#fde68a'}`,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          flexWrap: 'wrap',
          gap: '12px',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
          <span style={{ fontSize: '18px' }}>{tpl.verified ? '✓' : '👤'}</span>
          <div>
            <div style={{ fontWeight: 600, fontSize: '13px', color: tpl.verified ? '#166534' : '#92400e' }}>
              {tpl.verified ? '人类专家已审核核准此报表与公式' : '待人类专家在设计器中审阅并确认'}
            </div>
            <div className="tiny" style={{ color: tpl.verified ? '#15803d' : '#b45309' }}>
              {tpl.verified
                ? `签署人: ${tpl.verifier || '人类数据专家'} · 核准时间: ${tpl.verified_at ? new Date(tpl.verified_at).toLocaleString() : '刚刚'} · 报表架构与公式推导已锁定一致`
                : `Agent 已自动编译 ${formulaCells.length} 个单元格公式。人类专家审阅推导依据后点击核准，确保从头到尾一致`}
            </div>
          </div>
        </div>

        <div style={{ display: 'flex', gap: '8px' }}>
          {tpl.verified ? (
            <button
              type="button"
              className="btn small"
              onClick={handleUnverify}
              style={{ fontSize: '12px', color: '#64748b' }}
            >
              解除核准（重新微调）
            </button>
          ) : (
            <button
              type="button"
              className="btn small"
              onClick={handleVerifyHuman}
              style={{
                background: '#16a34a',
                color: '#ffffff',
                borderColor: '#15803d',
                fontWeight: 600,
                fontSize: '12px',
              }}
            >
              ✓ 人类专家审核核准此报表与公式 (HITL Confirm)
            </button>
          )}
        </div>
      </div>

      {/* Page Header */}
      <div className="page-head" style={{ marginBottom: '14px' }}>
        <div className="grow">
          <div className="eyebrow">Canonical Report Engine · Unified JSON Specification</div>
          <h1 style={{ fontSize: '20px', margin: '4px 0 2px' }}>{req.title}</h1>
          <div className="sub" style={{ display: 'flex', gap: '12px', alignItems: 'center', flexWrap: 'wrap' }}>
            <span>修订 rev {tpl.rev ?? 1}</span>
            <span>·</span>
            <span>网格 {grid.rows} × {grid.cols}</span>
            <span>·</span>
            <span>{formulaCells.length} 个 Agent 编译公式</span>
            <span>·</span>
            <span
              style={{
                color: tpl.verified ? '#16a34a' : '#b45309',
                fontWeight: 600,
              }}
            >
              {tpl.verified ? '● 已核准' : '○ 待人类核准'}
            </span>
          </div>
        </div>

        <div className="row" style={{ gap: '8px' }}>
          <button
            type="button"
            className="btn small"
            onClick={handleRecompileFormulas}
            title="让 Agent 智能推导各行列汇总公式及审计推导依据"
            style={{ display: 'flex', alignItems: 'center', gap: '4px', borderColor: '#d97706', color: '#b45309' }}
          >
            🤖 Agent 重新编译公式
          </button>
          <button type="button" className="btn small" onClick={handleSave}>
            保存修改
          </button>
          <button
            type="button"
            className="btn small primary"
            onClick={handleRun}
            style={{ fontWeight: 600 }}
          >
            ▶ 运行渲染引擎
          </button>
        </div>
      </div>

      {/* Surface Projection Switcher Tabs */}
      <div
        style={{
          display: 'flex',
          gap: '6px',
          borderBottom: '1px solid var(--line)',
          paddingBottom: '8px',
          marginBottom: '16px',
          overflowX: 'auto',
        }}
      >
        <button
          type="button"
          className={`tog ${activeTab === 'design' ? 'on' : ''}`}
          onClick={() => setActiveTab('design')}
          style={{
            fontWeight: activeTab === 'design' ? 600 : 'normal',
            background: activeTab === 'design' ? '#292524' : undefined,
            color: activeTab === 'design' ? '#ffffff' : undefined,
          }}
        >
          📐 网格与公式设计 (Grid & Formulas)
        </button>

        <button
          type="button"
          className={`tog ${activeTab === 'univer' ? 'on' : ''}`}
          onClick={() => setActiveTab('univer')}
          style={{
            fontWeight: activeTab === 'univer' ? 600 : 'normal',
            background: activeTab === 'univer' ? '#10b981' : undefined,
            color: activeTab === 'univer' ? '#ffffff' : undefined,
          }}
        >
          📊 Univer 电子表格投影
        </button>

        <button
          type="button"
          className={`tog ${activeTab === 'web' ? 'on' : ''}`}
          onClick={() => setActiveTab('web')}
          style={{
            fontWeight: activeTab === 'web' ? 600 : 'normal',
            background: activeTab === 'web' ? '#2563eb' : undefined,
            color: activeTab === 'web' ? '#ffffff' : undefined,
          }}
        >
          🌐 网页交互版投影 (Web HTML)
        </button>

        <button
          type="button"
          className={`tog ${activeTab === 'print' ? 'on' : ''}`}
          onClick={() => setActiveTab('print')}
          style={{
            fontWeight: activeTab === 'print' ? 600 : 'normal',
            background: activeTab === 'print' ? '#d97706' : undefined,
            color: activeTab === 'print' ? '#ffffff' : undefined,
          }}
        >
          📄 PDF / 打印排版投影
        </button>

        <button
          type="button"
          className={`tog ${activeTab === 'export' ? 'on' : ''}`}
          onClick={() => setActiveTab('export')}
          style={{
            fontWeight: activeTab === 'export' ? 600 : 'normal',
            background: activeTab === 'export' ? '#059669' : undefined,
            color: activeTab === 'export' ? '#ffffff' : undefined,
          }}
        >
          📗 Excel 导出 (XLS / CSV)
        </button>

        <button
          type="button"
          className={`tog ${activeTab === 'json' ? 'on' : ''}`}
          onClick={() => setActiveTab('json')}
          style={{
            fontWeight: activeTab === 'json' ? 600 : 'normal',
            background: activeTab === 'json' ? '#4f46e5' : undefined,
            color: activeTab === 'json' ? '#ffffff' : undefined,
          }}
        >
          📋 统一 JSON 规范定义 (Single Source)
        </button>
      </div>

      {/* TAB 1: Grid & Formulas Designer */}
      {activeTab === 'design' && (
        <div className="designer">
          {/* Left panel: Fields & Dataset */}
          <div className="dz-side">
            <div className="tiny" style={{ fontWeight: 600, color: '#64748b' }}>
              底层数据契约 (Dataset)
            </div>
            <div style={{ fontWeight: 600, margin: '4px 0 2px' }}>
              {req.dataset.file || req.dataset.dsn}
            </div>
            <div className="tiny">
              {req.dataset.rows} 行 · {req.dataset.columns.length} 列 · UTF-8
            </div>

            <div className="tiny" style={{ marginTop: '16px', fontWeight: 600, color: '#64748b' }}>
              可用字段与度量
            </div>
            <div style={{ marginTop: '6px' }}>
              {req.dataset.columns.map((c) => {
                const isM = c === 'amount' || (req.roles?.measures || []).includes(c);
                return (
                  <div key={c} className="field-chip">
                    <span>{c}</span>
                    <span className="ty">{isM ? '度量指标' : '维度字段'}</span>
                  </div>
                );
              })}
            </div>

            <div className="tiny" style={{ marginTop: '18px', fontWeight: 600, color: '#64748b' }}>
              Agent 编译公式清单 ({formulaCells.length})
            </div>
            <div style={{ marginTop: '6px', display: 'flex', flexDirection: 'column', gap: '6px' }}>
              {formulaCells.length === 0 ? (
                <div className="tiny muted">当前模板无公式，点击上方「Agent 重新编译公式」即可生成</div>
              ) : (
                formulaCells.map((fc) => (
                  <div
                    key={`${fc.row}-${fc.col}`}
                    onClick={() => setSelectedCoord({ row: fc.row, col: fc.col })}
                    style={{
                      padding: '6px 8px',
                      background: selectedCoord?.row === fc.row && selectedCoord?.col === fc.col ? '#fef3c7' : '#fafaf9',
                      border: '1px solid var(--line)',
                      borderRadius: '4px',
                      cursor: 'pointer',
                      fontSize: '11px',
                    }}
                  >
                    <div style={{ display: 'flex', justifyContent: 'space-between', fontFamily: 'monospace' }}>
                      <b>R{fc.row}C{fc.col}</b>
                      <span style={{ color: '#b45309' }}>{fc.formula}</span>
                    </div>
                    {fc.formula_audit && (
                      <div className="tiny muted" style={{ marginTop: '2px', lineHeight: 1.3 }}>
                        {fc.formula_audit}
                      </div>
                    )}
                  </div>
                ))
              )}
            </div>
          </div>

          {/* Center: Grid View */}
          <div className="dz-main">
            <div className="paper">
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  marginBottom: '8px',
                }}
              >
                <span className="tiny muted">
                  点击单元格查看 Agent 公式推导依据与样式属性
                </span>
                <span className="tiny muted">
                  单据规范版本: <code style={{ fontSize: '11px' }}>rpx-report/v1</code>
                </span>
              </div>
              <table className="grid">
                <tbody>
                  {Array.from({ length: grid.rows }, (_, rIdx) => {
                    const row = rIdx + 1;
                    return (
                      <tr key={row}>
                        {Array.from({ length: grid.cols }, (_, cIdx) => {
                          const col = cIdx + 1;
                          const key = `${row},${col}`;
                          if (covered.has(key)) return null;

                          const cell = cellMap.get(key) || { row, col, text: '' };
                          const isSel = selectedCoord?.row === row && selectedCoord?.col === col;

                          const style: React.CSSProperties = {
                            fontWeight: cell.bold ? 'bold' : 'normal',
                            fontSize: cell.font_size ? `${cell.font_size}px` : undefined,
                            color: cell.fore_color || undefined,
                            backgroundColor: isSel
                              ? '#fef3c7'
                              : cell.formula
                              ? '#fffbeb'
                              : cell.back_color || undefined,
                            textAlign:
                              cell.h_align === 1 || cell.h_align === 'left'
                                ? 'left'
                                : cell.h_align === 2 || cell.h_align === 'center'
                                ? 'center'
                                : cell.h_align === 3 || cell.h_align === 'right'
                                ? 'right'
                                : 'left',
                            verticalAlign:
                              cell.v_align === 1 || cell.v_align === 'top'
                                ? 'top'
                                : cell.v_align === 2 || cell.v_align === 'middle'
                                ? 'middle'
                                : 'bottom',
                            borderBottom: cell.border?.bottom ? '1.5px solid #475467' : undefined,
                            borderTop: cell.border?.top ? '1.5px solid #475467' : undefined,
                          };

                          const expr = cell.formula || cell.expr;

                          return (
                            <td
                              key={col}
                              className={`${expr ? 'expr' : ''} ${isSel ? 'sel' : ''}`}
                              rowSpan={rowspans[key] || 1}
                              colSpan={colspans[key] || 1}
                              style={style}
                              onClick={() => handleCellClick(row, col)}
                            >
                              {cell.formula ? (
                                <>
                                  <span className="fx" style={{ background: '#d97706', color: '#fff' }}>
                                    ƒx
                                  </span>
                                  <span title={String(cell.formula)}>
                                    {String(cell.formula).slice(0, 16)}
                                  </span>
                                </>
                              ) : cell.expr ? (
                                <>
                                  <span className="fx">ƒ</span>
                                  <span title={String(cell.expr)}>
                                    {String(cell.expr).slice(0, 16)}
                                  </span>
                                </>
                              ) : (
                                cell.text ?? ''
                              )}
                            </td>
                          );
                        })}
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>

          {/* Right panel: Cell Property & Formula Inspector */}
          <div className="dz-side right">
            {selectedCell ? (
              <>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                  <h3 style={{ margin: 0, fontSize: '13px' }}>单元格属性检视</h3>
                  <span className="tiny" style={{ fontFamily: 'monospace' }}>
                    R{selectedCell.row}C{selectedCell.col}
                  </span>
                </div>

                {/* Agent Compiled Formula Section */}
                {selectedCell.formula ? (
                  <div
                    style={{
                      marginTop: '12px',
                      padding: '10px',
                      background: '#fffbeb',
                      border: '1px solid #fde68a',
                      borderRadius: '6px',
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginBottom: '6px' }}>
                      <span style={{ fontSize: '12px' }}>🤖</span>
                      <span style={{ fontSize: '12px', fontWeight: 600, color: '#92400e' }}>
                        Agent 自动编译公式
                      </span>
                    </div>

                    <div className="props-row" style={{ marginTop: '6px' }}>
                      <span className="lab" style={{ color: '#92400e' }}>公式</span>
                      <input
                        type="text"
                        value={selectedCell.formula || ''}
                        onChange={(e) => updateSelectedCell({ formula: e.target.value })}
                        style={{ fontFamily: 'monospace', fontWeight: 600 }}
                      />
                    </div>

                    <div style={{ marginTop: '8px' }}>
                      <div className="tiny" style={{ color: '#92400e', fontWeight: 600 }}>
                        推导依据 / 审计依据 (Audit):
                      </div>
                      <div
                        className="tiny"
                        style={{
                          marginTop: '2px',
                          color: '#78350f',
                          background: '#fff',
                          padding: '6px',
                          borderRadius: '4px',
                          border: '1px solid #fef3c7',
                          lineHeight: 1.4,
                        }}
                      >
                        {selectedCell.formula_audit || 'Agent 根据维度层级与度量聚合推导求和范围'}
                      </div>
                    </div>
                  </div>
                ) : selectedCell.expr ? (
                  <div
                    style={{
                      marginTop: '12px',
                      padding: '10px',
                      background: '#f8fafc',
                      border: '1px solid #e2e8f0',
                      borderRadius: '6px',
                    }}
                  >
                    <div className="tiny" style={{ fontWeight: 600, color: '#475467' }}>
                      数据字段绑定表达式
                    </div>
                    <code
                      style={{
                        display: 'block',
                        marginTop: '4px',
                        padding: '4px 6px',
                        background: '#fff',
                        border: '1px solid #cbd5e1',
                        borderRadius: '4px',
                        fontSize: '11px',
                      }}
                    >
                      {selectedCell.expr}
                    </code>
                  </div>
                ) : (
                  <div className="props-row" style={{ marginTop: '12px' }}>
                    <span className="lab">文本内容</span>
                    <input
                      type="text"
                      value={selectedCell.text ?? ''}
                      onChange={(e) => updateSelectedCell({ text: e.target.value })}
                    />
                  </div>
                )}

                {/* Typography & Alignment controls */}
                <div style={{ marginTop: '16px' }}>
                  <div className="props-row">
                    <span className="lab">字号</span>
                    <input
                      type="number"
                      min={8}
                      max={32}
                      value={selectedCell.font_size || 12}
                      onChange={(e) =>
                        updateSelectedCell({ font_size: Number(e.target.value) || 12 })
                      }
                    />
                  </div>

                  <div className="props-row">
                    <span className="lab">粗体</span>
                    <button
                      type="button"
                      className={`tog ${selectedCell.bold ? 'on' : ''}`}
                      onClick={() => updateSelectedCell({ bold: !selectedCell.bold })}
                    >
                      B (粗体)
                    </button>
                  </div>

                  <div className="props-row">
                    <span className="lab">对齐</span>
                    <div className="chip-toggles">
                      {(['left', 'center', 'right'] as const).map((k) => {
                        const h =
                          selectedCell.h_align === 1 || selectedCell.h_align === 'left'
                            ? 'left'
                            : selectedCell.h_align === 2 || selectedCell.h_align === 'center'
                            ? 'center'
                            : 'right';
                        return (
                          <button
                            key={k}
                            type="button"
                            className={`tog ${h === k ? 'on' : ''}`}
                            onClick={() => updateSelectedCell({ h_align: k })}
                          >
                            {{ left: '左', center: '中', right: '右' }[k]}
                          </button>
                        );
                      })}
                    </div>
                  </div>

                  <div className="props-row">
                    <span className="lab">边框线</span>
                    <div className="chip-toggles">
                      <button
                        type="button"
                        className={`tog ${selectedCell.border?.top ? 'on' : ''}`}
                        onClick={() =>
                          updateSelectedCell({
                            border: {
                              ...selectedCell.border,
                              top: !selectedCell.border?.top,
                            },
                          })
                        }
                      >
                        上边框
                      </button>
                      <button
                        type="button"
                        className={`tog ${selectedCell.border?.bottom ? 'on' : ''}`}
                        onClick={() =>
                          updateSelectedCell({
                            border: {
                              ...selectedCell.border,
                              bottom: !selectedCell.border?.bottom,
                            },
                          })
                        }
                      >
                        下边框
                      </button>
                    </div>
                  </div>
                </div>

                <div className="hint" style={{ marginTop: '16px', lineHeight: 1.4 }}>
                  人类专家可随时修改公式与格式；核准后整体写入规范 JSON。
                </div>
              </>
            ) : (
              <>
                <h3 style={{ margin: '0 0 6px', fontSize: '13px' }}>单元格属性</h3>
                <div className="hint">在左边点选任意单元格查看 Agent 公式与样式。</div>
              </>
            )}
          </div>
        </div>
      )}

      {/* TAB 2: Univer Spreadsheet Projection */}
      {activeTab === 'univer' && (
        <div style={{ marginTop: '8px' }}>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              marginBottom: '10px',
              flexWrap: 'wrap',
              gap: '8px',
            }}
          >
            <div className="tiny">
              {latestRun
                ? `最新执行批次: ${latestRun.run_id} · 模板修订: rev ${latestRun.template_rev} · 耗时 ${latestRun.duration_ms} ms · 状态: ${latestRun.status}`
                : '尚未渲染计算结果，请点击右上角「运行渲染引擎」'}
            </div>
            <div style={{ display: 'flex', gap: '8px' }}>
              <button type="button" className="btn small" onClick={handleRun}>
                ↻ 重新计算并刷新 Univer
              </button>
            </div>
          </div>

          {latestRun?.univerSnapshot ? (
            <UniverViewer
              snapshot={latestRun.univerSnapshot}
              title={`${req.title} · Univer 电子表格投影`}
              height="520px"
            />
          ) : (
            <div className="empty" style={{ padding: '60px', background: '#fff', borderRadius: '8px', textAlign: 'center' }}>
              <div style={{ fontSize: '24px', marginBottom: '8px' }}>📊</div>
              <div style={{ fontWeight: 600, marginBottom: '6px' }}>尚未生成 Univer 电子表格快照</div>
              <div className="tiny muted" style={{ marginBottom: '14px' }}>
                点击右上角「运行渲染引擎」，引擎将按规范 JSON 编译数据并投影为 Univer 工作簿
              </div>
              <button type="button" className="btn primary small" onClick={handleRun}>
                立即运行渲染
              </button>
            </div>
          )}
        </div>
      )}

      {/* TAB 3: Interactive Web HTML Projection */}
      {activeTab === 'web' && (
        <div style={{ marginTop: '8px' }}>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              marginBottom: '12px',
              flexWrap: 'wrap',
              gap: '8px',
            }}
          >
            <div className="tiny">
              网页交互版投影：响应式轻量 Web 表格，支持即时搜索与汇总统计
            </div>
            <input
              type="text"
              placeholder="搜索表格内容..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              style={{ padding: '4px 8px', fontSize: '12px', width: '200px' }}
            />
          </div>

          {latestRun?.computedData ? (
            <div
              style={{
                background: '#fff',
                border: '1px solid var(--line)',
                borderRadius: '8px',
                overflow: 'hidden',
                boxShadow: '0 1px 3px rgba(0,0,0,0.05)',
              }}
            >
              <div
                style={{
                  padding: '14px 18px',
                  background: '#f8fafc',
                  borderBottom: '1px solid #e2e8f0',
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                }}
              >
                <div>
                  <h3 style={{ margin: 0, fontSize: '15px' }}>{req.title}</h3>
                  <div className="tiny muted">
                    来源：Canonical JSON 规范报表投影 · 引擎版本 {latestRun.engine_version}
                  </div>
                </div>
                <span
                  style={{
                    fontSize: '11px',
                    padding: '2px 8px',
                    background: '#e0f2fe',
                    color: '#0369a1',
                    borderRadius: '4px',
                    fontWeight: 600,
                  }}
                >
                  HTML 响应式
                </span>
              </div>

              <div style={{ overflowX: 'auto' }}>
                <table style={{ width: '100%', borderCollapse: 'collapse', textAlign: 'left', fontSize: '13px' }}>
                  <thead>
                    <tr style={{ background: '#f1f5f9', borderBottom: '2px solid #cbd5e1' }}>
                      {latestRun.computedData.headers.map((h, i) => (
                        <th
                          key={i}
                          style={{
                            padding: '10px 14px',
                            fontWeight: 600,
                            color: '#334155',
                            textAlign: i === 0 ? 'left' : 'right',
                          }}
                        >
                          {h}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {latestRun.computedData.rows
                      .filter((row) =>
                        !searchTerm || row.some((val) => String(val).toLowerCase().includes(searchTerm.toLowerCase()))
                      )
                      .map((row, rIdx) => (
                        <tr
                          key={rIdx}
                          style={{
                            borderBottom: '1px solid #f1f5f9',
                            background: rIdx % 2 === 0 ? '#ffffff' : '#fafaf9',
                          }}
                        >
                          {row.map((val: any, cIdx: number) => (
                            <td
                              key={cIdx}
                              style={{
                                padding: '8px 14px',
                                textAlign: cIdx === 0 ? 'left' : 'right',
                                fontFamily: typeof val === 'number' ? 'monospace' : 'inherit',
                              }}
                            >
                              {typeof val === 'number' ? val.toLocaleString() : val}
                            </td>
                          ))}
                        </tr>
                      ))}
                    {latestRun.computedData.summaryRow && (
                      <tr
                        style={{
                          borderTop: '2px solid #94a3b8',
                          background: '#fffcf7',
                          fontWeight: 'bold',
                        }}
                      >
                        {latestRun.computedData.summaryRow.map((val: any, cIdx: number) => (
                          <td
                            key={cIdx}
                            style={{
                              padding: '10px 14px',
                              textAlign: cIdx === 0 ? 'left' : 'right',
                              color: '#92400e',
                              fontFamily: typeof val === 'number' ? 'monospace' : 'inherit',
                            }}
                          >
                            {typeof val === 'number' ? val.toLocaleString() : val}
                          </td>
                        ))}
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
            </div>
          ) : (
            <div className="empty" style={{ padding: '40px', background: '#fff', borderRadius: '8px', textAlign: 'center' }}>
              点击右上角「运行渲染引擎」后查看交互式 Web 表格投影
            </div>
          )}
        </div>
      )}

      {/* TAB 4: PDF / Print Layout Projection */}
      {activeTab === 'print' && (
        <div style={{ marginTop: '8px' }}>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              marginBottom: '12px',
              flexWrap: 'wrap',
              gap: '8px',
            }}
          >
            <div className="tiny">
              PDF / 打印套打排版：标准 A4 纸张页面规约，包含标题栏、核准印签区与页脚
            </div>
            <button
              type="button"
              className="btn small"
              onClick={() => window.print()}
              style={{ fontWeight: 600, display: 'flex', alignItems: 'center', gap: '4px' }}
            >
              🖨️ 调用系统打印 / 导出 PDF
            </button>
          </div>

          <div
            style={{
              maxWidth: '800px',
              margin: '0 auto',
              background: '#ffffff',
              boxShadow: '0 4px 14px rgba(0,0,0,0.08)',
              border: '1px solid #e2e8f0',
              padding: '40px 48px',
              borderRadius: '2px',
              minHeight: '600px',
            }}
          >
            {/* Header of Printed Paper */}
            <div style={{ borderBottom: '2px solid #1e293b', paddingBottom: '16px', marginBottom: '24px' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                <div>
                  <h1 style={{ margin: 0, fontSize: '20px', letterSpacing: '0.5px', color: '#0f172a' }}>
                    {req.title}
                  </h1>
                  <div style={{ fontSize: '12px', color: '#64748b', marginTop: '4px' }}>
                    报表单号: {req.id} · 规范版本: {canonicalDoc.schema} · 修订 rev {tpl.rev}
                  </div>
                </div>
                <div style={{ textAlign: 'right', fontSize: '11px', color: '#64748b' }}>
                  <div>打印时间: {new Date().toLocaleDateString()}</div>
                  <div style={{ marginTop: '2px' }}>
                    状态: <b style={{ color: tpl.verified ? '#16a34a' : '#b45309' }}>{tpl.verified ? '人类专家已核准' : '草拟待审核'}</b>
                  </div>
                </div>
              </div>
            </div>

            {/* Print Table */}
            {latestRun?.computedData ? (
              <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '12px', marginBottom: '30px' }}>
                <thead>
                  <tr style={{ borderBottom: '1.5px solid #0f172a', background: '#f8fafc' }}>
                    {latestRun.computedData.headers.map((h, i) => (
                      <th
                        key={i}
                        style={{
                          padding: '8px 10px',
                          textAlign: i === 0 ? 'left' : 'right',
                          fontWeight: 600,
                          color: '#0f172a',
                        }}
                      >
                        {h}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {latestRun.computedData.rows.map((r, rIdx) => (
                    <tr key={rIdx} style={{ borderBottom: '1px solid #e2e8f0' }}>
                      {r.map((val: any, cIdx: number) => (
                        <td
                          key={cIdx}
                          style={{
                            padding: '6px 10px',
                            textAlign: cIdx === 0 ? 'left' : 'right',
                          }}
                        >
                          {typeof val === 'number' ? val.toLocaleString() : val}
                        </td>
                      ))}
                    </tr>
                  ))}
                  {latestRun.computedData.summaryRow && (
                    <tr style={{ borderTop: '2px solid #0f172a', fontWeight: 'bold' }}>
                      {latestRun.computedData.summaryRow.map((val: any, cIdx: number) => (
                        <td
                          key={cIdx}
                          style={{
                            padding: '8px 10px',
                            textAlign: cIdx === 0 ? 'left' : 'right',
                          }}
                        >
                          {typeof val === 'number' ? val.toLocaleString() : val}
                        </td>
                      ))}
                    </tr>
                  )}
                </tbody>
              </table>
            ) : (
              <div className="muted tiny" style={{ padding: '40px', textAlign: 'center' }}>
                请先运行报表渲染引擎生成打印数据
              </div>
            )}

            {/* Sign-off signature footer for printed documents */}
            <div
              style={{
                marginTop: '40px',
                paddingTop: '20px',
                borderTop: '1px dashed #cbd5e1',
                display: 'flex',
                justifyContent: 'space-between',
                fontSize: '11px',
                color: '#64748b',
              }}
            >
              <div>制表 Agent: RPX Auto-Compiler (v2.4)</div>
              <div>
                审核专家确认: {tpl.verified ? `✓ ${tpl.verifier} (${tpl.verified_at?.slice(0, 10)})` : '______________________ (签字)'}
              </div>
              <div>第 1 页 / 共 1 页</div>
            </div>
          </div>
        </div>
      )}

      {/* TAB 5: Excel Workbook Export */}
      {activeTab === 'export' && (
        <div style={{ marginTop: '8px' }}>
          <div className="card pad" style={{ maxWidth: '680px', margin: '0 auto' }}>
            <h3 style={{ margin: '0 0 6px', fontSize: '15px' }}>📗 Excel / XLS 工作簿导出</h3>
            <div className="muted tiny" style={{ marginBottom: '16px', lineHeight: 1.5 }}>
              报表引擎将规范 JSON 中的单元格公式（如 <code style={{ fontSize: '11px' }}>=SUM(...)</code>）和格式元数据直接导出为 Excel 工作簿格式，保证在 Microsoft Excel 和 WPS 中开箱即用，公式可被二次计算。
            </div>

            <div
              style={{
                background: '#f8fafc',
                padding: '14px',
                borderRadius: '6px',
                border: '1px solid #e2e8f0',
                marginBottom: '16px',
              }}
            >
              <div style={{ fontWeight: 600, fontSize: '13px', marginBottom: '6px' }}>导出概览</div>
              <div className="tiny" style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px' }}>
                <div>目标文件: <b>{req.title}.xlsx / .csv</b></div>
                <div>编码格式: <b>UTF-8 带 BOM (Excel 兼容)</b></div>
                <div>公式保留: <b>{formulaCells.length} 个 Agent 编译公式</b></div>
                <div>审核状态: <b>{tpl.verified ? '已核准锁定' : '草拟中'}</b></div>
              </div>
            </div>

            <div style={{ display: 'flex', gap: '10px' }}>
              <button
                type="button"
                className="btn primary"
                onClick={handleExportCsv}
                style={{ fontWeight: 600, display: 'flex', alignItems: 'center', gap: '6px' }}
              >
                📥 下载 Excel (XLSX / CSV) 工作簿
              </button>
            </div>
          </div>
        </div>
      )}

      {/* TAB 6: Canonical JSON Specification Document */}
      {activeTab === 'json' && (
        <div style={{ marginTop: '8px' }}>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              marginBottom: '10px',
              flexWrap: 'wrap',
              gap: '8px',
            }}
          >
            <div>
              <div style={{ fontWeight: 600, fontSize: '13px' }}>
                统一规范 JSON 报表定义 (Canonical Report Specification)
              </div>
              <div className="tiny muted">
                单据协议 <code style={{ fontSize: '11px' }}>rpx-report/v1</code> · 从头到尾一致 · 支持 Univer、Web、PDF、Excel 多端等价投影
              </div>
            </div>

            <div style={{ display: 'flex', gap: '8px' }}>
              <button
                type="button"
                className="btn small"
                onClick={handleCopyJson}
                style={{ fontWeight: 600 }}
              >
                {copiedJson ? '✓ 已复制' : '复制 JSON'}
              </button>
              <button
                type="button"
                className="btn small"
                onClick={handleDownloadJson}
                style={{ fontWeight: 600 }}
              >
                下载 report.json
              </button>
            </div>
          </div>

          <pre
            style={{
              background: '#0f172a',
              color: '#f8fafc',
              padding: '16px',
              borderRadius: '8px',
              fontSize: '12px',
              lineHeight: 1.45,
              overflowX: 'auto',
              maxHeight: '520px',
              fontFamily: 'Consolas, Monaco, "Courier New", monospace',
            }}
          >
            {canonicalJsonString}
          </pre>
        </div>
      )}
    </>
  );
};
