import React, { useState } from 'react';
import { store } from '../services/store';
import { useToast } from '../components/Toast';
import { TemplateCell } from '../types';
import { UniverViewer } from '../components/UniverViewer';

interface Props {
  reqId: string;
  onBack: () => void;
  onOpenRuns: (id: string, runId?: string) => void;
}

export const DesignerView: React.FC<Props> = ({ reqId, onBack, onOpenRuns }) => {
  const { toast } = useToast();
  const req = store.getRequirement(reqId);

  const [designMode, setDesignMode] = useState<'template' | 'result'>('template');
  const [resultViewType, setResultViewType] = useState<'univer' | 'print'>('univer');
  const [selectedCoord, setSelectedCoord] = useState<{ row: number; col: number } | null>({
    row: 2,
    col: 1,
  });

  if (!req || !req.template) {
    return (
      <div className="card pad">
        <b>还没有模板</b>
        <div className="muted" style={{ marginTop: '6px' }}>
          先在需求页「编译出稿」。
        </div>
        <div style={{ marginTop: '12px' }}>
          <button type="button" className="btn" onClick={onBack}>
            回需求
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
    toast(`已写回模板定义（rev ${tpl.rev}）`);
  };

  const handleRun = () => {
    const { run } = store.runRequirement(req.id);
    toast(`运行完成：${run.status}`);
    setDesignMode('result');
  };

  const updateSelectedCell = (patch: Partial<TemplateCell>) => {
    if (!selectedCoord) return;
    store.patchTemplateCells(req.id, [{ row: selectedCoord.row, col: selectedCoord.col, ...patch }]);
  };

  const latestRun = req.runs[0];

  return (
    <>
      <div className="row" style={{ marginBottom: '10px', gap: '8px' }}>
        <a
          href="#"
          className="tiny"
          onClick={(e) => {
            e.preventDefault();
            onBack();
          }}
        >
          ← 需求
        </a>
        <span className="tiny">/</span>
        <span className="tiny">模板设计器</span>
      </div>

      <div className="page-head" style={{ marginBottom: '12px' }}>
        <div className="grow">
          <div className="eyebrow">Template Studio</div>
          <h1 style={{ fontSize: '22px' }}>{req.title}</h1>
          <div className="sub">
            rev {tpl.rev ?? '—'} · 网格 {grid.rows} × {grid.cols} · 改动写回模板定义，重新运行即生效
          </div>
        </div>
        <div className="row" style={{ gap: '8px' }}>
          <button
            type="button"
            className={`btn small ${designMode === 'template' ? 'primary' : ''}`}
            onClick={() => setDesignMode('template')}
          >
            模板编辑
          </button>
          <button
            type="button"
            className={`btn small ${designMode === 'result' ? 'primary' : ''}`}
            onClick={() => setDesignMode('result')}
          >
            结果预览
          </button>
          <button type="button" className="btn small" onClick={handleSave}>
            保存
          </button>
          <button type="button" className="btn small primary" onClick={handleRun}>
            运行
          </button>
        </div>
      </div>

      {designMode === 'template' ? (
        <div className="designer">
          {/* Left panel: Fields */}
          <div className="dz-side">
            <div className="tiny">数据集</div>
            <div style={{ fontWeight: 600, margin: '2px 0 2px' }}>
              {req.dataset.file || req.dataset.dsn}
            </div>
            <div className="tiny">
              {req.dataset.rows} 行 · {req.dataset.columns.length} 列
            </div>
            <div className="tiny" style={{ marginTop: '14px' }}>
              字段列表
            </div>
            <div style={{ marginTop: '6px' }}>
              {req.dataset.columns.map((c) => {
                const isM = c === 'amount' || (req.roles?.measures || []).includes(c);
                return (
                  <div key={c} className="field-chip">
                    {c}
                    <span className="ty">{isM ? '数值' : '维度'}</span>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Center: Spreadsheet Grid */}
          <div className="dz-main">
            <div className="paper">
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
                          const isSel =
                            selectedCoord?.row === row && selectedCoord?.col === col;

                          const style: React.CSSProperties = {
                            fontWeight: cell.bold ? 'bold' : 'normal',
                            fontSize: cell.font_size ? `${cell.font_size}px` : undefined,
                            color: cell.fore_color || undefined,
                            backgroundColor: cell.back_color || undefined,
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

                          const expr = cell.expr || cell.formula;

                          return (
                            <td
                              key={col}
                              className={`${expr ? 'expr' : ''} ${isSel ? 'sel' : ''}`}
                              rowSpan={rowspans[key] || 1}
                              colSpan={colspans[key] || 1}
                              style={style}
                              onClick={() => handleCellClick(row, col)}
                            >
                              {expr ? (
                                <>
                                  <span className="fx">ƒ</span>
                                  <span title={String(expr)}>
                                    {String(expr).slice(0, 16)}
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

          {/* Right panel: Cell Property Inspector */}
          <div className="dz-side right">
            {selectedCell ? (
              <>
                <h3 style={{ margin: '0 0 2px', fontSize: '13px' }}>单元格属性</h3>
                <div className="tiny">
                  R{selectedCell.row}C{selectedCell.col}
                  {selectedCell.expr || selectedCell.formula ? ' · 公式格' : ''}
                </div>
                <div style={{ marginTop: '14px' }}>
                  <div className="props-row">
                    <span className="lab">内容</span>
                    {selectedCell.expr || selectedCell.formula ? (
                      <span className="tiny">
                        {String(selectedCell.expr || selectedCell.formula)}
                      </span>
                    ) : (
                      <input
                        type="text"
                        value={selectedCell.text ?? ''}
                        onChange={(e) => updateSelectedCell({ text: e.target.value })}
                      />
                    )}
                  </div>
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
                    <span className="lab">字形</span>
                    <button
                      type="button"
                      className={`tog ${selectedCell.bold ? 'on' : ''}`}
                      onClick={() => updateSelectedCell({ bold: !selectedCell.bold })}
                    >
                      B
                    </button>
                  </div>
                  <div className="props-row">
                    <span className="lab">水平对齐</span>
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
                    <span className="lab">边框</span>
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
                        上
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
                        下
                      </button>
                    </div>
                  </div>
                </div>
                <div className="hint" style={{ marginTop: '18px' }}>
                  合并、表达式列不在这里改 —— 用需求页的维度/度量重新编译。
                </div>
              </>
            ) : (
              <>
                <h3 style={{ margin: '0 0 6px', fontSize: '13px' }}>单元格属性</h3>
                <div className="hint">在左边点选一个单元格进行样式调整。</div>
              </>
            )}
          </div>
        </div>
      ) : (
        /* Result Preview */
        <div style={{ marginTop: '12px' }}>
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
                ? `运行 ${latestRun.run_id} · 模板 rev ${latestRun.template_rev} · 耗时 ${latestRun.duration_ms} ms`
                : '尚未运行报表，请点击右上角「运行」执行'}
            </div>
            {latestRun && (
              <div className="seg" style={{ display: 'flex', gap: '4px' }}>
                <button
                  type="button"
                  className={`tog ${resultViewType === 'univer' ? 'on' : ''}`}
                  style={{
                    background: resultViewType === 'univer' ? '#10b981' : undefined,
                    color: resultViewType === 'univer' ? '#ffffff' : undefined,
                    fontWeight: 600,
                  }}
                  onClick={() => setResultViewType('univer')}
                >
                  ★ Univer 电子表格
                </button>
                <button
                  type="button"
                  className={`tog ${resultViewType === 'print' ? 'on' : ''}`}
                  onClick={() => setResultViewType('print')}
                >
                  打印排版视图
                </button>
              </div>
            )}
          </div>

          {latestRun ? (
            resultViewType === 'univer' ? (
              <UniverViewer
                snapshot={latestRun.univerSnapshot}
                title={`${req.title} · 计算结果`}
                height="480px"
              />
            ) : latestRun.artifacts['report.html'] ? (
              <div
                dangerouslySetInnerHTML={{
                  __html: latestRun.artifacts['report.html'],
                }}
                style={{
                  background: '#fffcf7',
                  border: '1px solid var(--line)',
                  borderRadius: '8px',
                  padding: '16px',
                }}
              />
            ) : (
              <div className="empty">暂无排版 HTML</div>
            )
          ) : (
            <div className="empty" style={{ padding: '40px', background: '#fff', borderRadius: '8px' }}>
              尚未执行报表，请点击右上角「运行」按钮生成 Univer 快照与计算结果
            </div>
          )}
        </div>
      )}
    </>
  );
};
