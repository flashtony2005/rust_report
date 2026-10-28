import React, { useState, useMemo } from 'react';
import { IWorkbookData, IWorksheetData, UniverCellData } from '../types';
import { useToast } from './Toast';

interface Props {
  snapshot?: IWorkbookData;
  title?: string;
  onDownloadSnapshot?: () => void;
  height?: string | number;
}

export const UniverViewer: React.FC<Props> = ({
  snapshot,
  title = '报表输出工作簿',
  onDownloadSnapshot,
  height = '520px',
}) => {
  const { toast } = useToast();
  const [activeSheetKey, setActiveSheetKey] = useState<string>(
    snapshot?.sheetOrder?.[0] || 'rpx-sheet-1'
  );
  const [selectedCell, setSelectedCell] = useState<{ row: number; col: number }>({
    row: 0,
    col: 0,
  });
  const [showJsonModal, setShowJsonModal] = useState<boolean>(false);
  const [zoom, setZoom] = useState<number>(100);

  const sheet: IWorksheetData | undefined = snapshot?.sheets?.[activeSheetKey];

  // Coordinates helper
  const colToLetter = (colIdx: number): string => {
    let temp = colIdx;
    let letter = '';
    while (temp >= 0) {
      letter = String.fromCharCode((temp % 26) + 65) + letter;
      temp = Math.floor(temp / 26) - 1;
    }
    return letter;
  };

  // Convert row, col to Excel coord e.g. A1, B2
  const activeCoordName = `${colToLetter(selectedCell.col)}${selectedCell.row + 1}`;

  // Current active cell data
  const currentCellData: UniverCellData | undefined =
    sheet?.cellData?.[String(selectedCell.row)]?.[String(selectedCell.col)];

  // Determine row count and column count
  const rowCount = useMemo(() => {
    if (!sheet) return 15;
    const maxDataRow = Math.max(
      ...Object.keys(sheet.cellData || {}).map((k) => Number(k)),
      0
    );
    return Math.max(maxDataRow + 6, 20);
  }, [sheet]);

  const colCount = useMemo(() => {
    if (!sheet) return 8;
    let maxCol = 4;
    Object.values(sheet.cellData || {}).forEach((cols) => {
      Object.keys(cols).forEach((c) => {
        maxCol = Math.max(maxCol, Number(c));
      });
    });
    return Math.max(maxCol + 4, 10);
  }, [sheet]);

  // Aggregate stats for current column / selection
  const selectionStats = useMemo(() => {
    if (!sheet) return { count: 1, sum: 0, avg: 0, hasNum: false };
    const curVal = currentCellData?.v;
    if (typeof curVal === 'number') {
      return { count: 1, sum: curVal, avg: curVal, hasNum: true };
    }
    // Calculate for active column if numeric
    let sum = 0;
    let numCount = 0;
    let totalCount = 0;
    for (let r = 1; r < rowCount; r++) {
      const v = sheet.cellData?.[String(r)]?.[String(selectedCell.col)]?.v;
      if (v !== undefined && v !== '') {
        totalCount++;
        if (typeof v === 'number') {
          sum += v;
          numCount++;
        }
      }
    }
    return {
      count: totalCount || 1,
      sum,
      avg: numCount ? Math.round(sum / numCount) : 0,
      hasNum: numCount > 0,
    };
  }, [sheet, currentCellData, selectedCell, rowCount]);

  const copySnapshot = () => {
    if (!snapshot) return;
    navigator.clipboard.writeText(JSON.stringify(snapshot, null, 2));
    toast('已复制 Univer IWorkbookData 快照 JSON');
  };

  if (!snapshot || !sheet) {
    return (
      <div
        style={{
          border: '1px solid var(--line)',
          borderRadius: '8px',
          padding: '32px',
          textAlign: 'center',
          color: 'var(--ink-3)',
          background: '#fff',
        }}
      >
        暂无 Univer 工作簿快照数据
      </div>
    );
  }

  return (
    <div
      className="univer-container"
      style={{
        border: '1px solid #c9d2de',
        borderRadius: '8px',
        background: '#ffffff',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        boxShadow: '0 2px 8px rgba(0,0,0,0.06)',
        fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
      }}
    >
      {/* 1. Univer Title & Top Actions Bar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          padding: '8px 12px',
          background: '#f8fafc',
          borderBottom: '1px solid #e2e8f0',
          gap: '12px',
          flexWrap: 'wrap',
        }}
      >
        {/* Univer Logo / Identity Badge */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <div
            style={{
              width: '24px',
              height: '24px',
              borderRadius: '5px',
              background: 'linear-gradient(135deg, #10b981 0%, #059669 100%)',
              color: '#ffffff',
              fontWeight: 800,
              fontSize: '13px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              boxShadow: '0 1px 3px rgba(16,185,129,0.3)',
            }}
          >
            U
          </div>
          <span style={{ fontWeight: 700, fontSize: '13px', color: '#1e293b' }}>
            Univer Sheets
          </span>
          <span
            style={{
              fontSize: '11px',
              padding: '1px 6px',
              background: '#ecfdf5',
              color: '#065f46',
              border: '1px solid #a7f3d0',
              borderRadius: '4px',
              fontWeight: 600,
            }}
          >
            v1.2.0 快照直连
          </span>
        </div>

        <div style={{ color: '#64748b', fontSize: '12px', fontWeight: 500 }}>
          {snapshot.name || title}
        </div>

        <div style={{ marginLeft: 'auto', display: 'flex', gap: '8px', alignItems: 'center' }}>
          <button
            type="button"
            className="btn small"
            style={{ fontSize: '11.5px', padding: '3px 8px', height: '26px' }}
            onClick={() => setShowJsonModal(true)}
          >
            {'{ }'} 查看 IWorkbookData 契约
          </button>
          <button
            type="button"
            className="btn small"
            style={{ fontSize: '11.5px', padding: '3px 8px', height: '26px' }}
            onClick={copySnapshot}
          >
            复制快照
          </button>
          {onDownloadSnapshot && (
            <button
              type="button"
              className="btn small primary"
              style={{ fontSize: '11.5px', padding: '3px 10px', height: '26px' }}
              onClick={onDownloadSnapshot}
            >
              下载 snapshot.json ↓
            </button>
          )}
        </div>
      </div>

      {/* 2. Univer Toolbar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '6px',
          padding: '4px 10px',
          background: '#ffffff',
          borderBottom: '1px solid #e2e8f0',
          fontSize: '12px',
          color: '#334155',
          overflowX: 'auto',
        }}
      >
        <button
          type="button"
          title="撤销 (Undo)"
          style={{ border: 'none', background: 'transparent', padding: '4px 6px', borderRadius: '4px', cursor: 'pointer' }}
          onClick={() => toast('Univer 交互状态：处于快照渲染模式')}
        >
          ↶
        </button>
        <button
          type="button"
          title="重做 (Redo)"
          style={{ border: 'none', background: 'transparent', padding: '4px 6px', borderRadius: '4px', cursor: 'pointer' }}
        >
          ↷
        </button>
        <div style={{ width: '1px', height: '16px', background: '#e2e8f0', margin: '0 4px' }} />

        {/* Font Family & Size */}
        <select
          style={{
            height: '24px',
            fontSize: '11px',
            border: '1px solid #cbd5e1',
            borderRadius: '3px',
            padding: '0 4px',
            color: '#334155',
          }}
          defaultValue="Segoe UI"
        >
          <option>Segoe UI</option>
          <option>Noto Sans SC</option>
          <option>Arial</option>
        </select>
        <select
          style={{
            height: '24px',
            fontSize: '11px',
            border: '1px solid #cbd5e1',
            borderRadius: '3px',
            padding: '0 4px',
            color: '#334155',
          }}
          defaultValue="11"
        >
          <option>9</option>
          <option>10</option>
          <option>11</option>
          <option>12</option>
          <option>14</option>
          <option>16</option>
        </select>

        <div style={{ width: '1px', height: '16px', background: '#e2e8f0', margin: '0 4px' }} />

        {/* Styles: B, I, Color, BG */}
        <button
          type="button"
          title="加粗"
          style={{
            fontWeight: 700,
            border: 'none',
            background: currentCellData?.s?.bl ? '#e0f2fe' : 'transparent',
            padding: '2px 7px',
            borderRadius: '3px',
            cursor: 'pointer',
          }}
        >
          B
        </button>
        <button
          type="button"
          title="斜体"
          style={{
            fontStyle: 'italic',
            border: 'none',
            background: 'transparent',
            padding: '2px 7px',
            borderRadius: '3px',
            cursor: 'pointer',
          }}
        >
          I
        </button>
        <button
          type="button"
          title="单元格底色"
          style={{
            border: 'none',
            background: 'transparent',
            padding: '2px 6px',
            borderRadius: '3px',
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            gap: '2px',
          }}
        >
          <span>🎨</span>
          <span
            style={{
              width: '10px',
              height: '3px',
              background: currentCellData?.s?.bg?.rgb || '#efe9df',
              display: 'inline-block',
            }}
          />
        </button>
        <button
          type="button"
          title="全边框"
          style={{
            border: 'none',
            background: 'transparent',
            padding: '2px 6px',
            borderRadius: '3px',
            cursor: 'pointer',
          }}
        >
          ⊞
        </button>

        <div style={{ width: '1px', height: '16px', background: '#e2e8f0', margin: '0 4px' }} />

        {/* Alignments */}
        <button
          type="button"
          title="居左对齐"
          style={{
            border: 'none',
            background: currentCellData?.s?.ht === 1 ? '#e0f2fe' : 'transparent',
            padding: '2px 6px',
            borderRadius: '3px',
            cursor: 'pointer',
          }}
        >
          ⇤
        </button>
        <button
          type="button"
          title="居中对齐"
          style={{
            border: 'none',
            background: currentCellData?.s?.ht === 2 ? '#e0f2fe' : 'transparent',
            padding: '2px 6px',
            borderRadius: '3px',
            cursor: 'pointer',
          }}
        >
          ⇔
        </button>
        <button
          type="button"
          title="居右对齐"
          style={{
            border: 'none',
            background: currentCellData?.s?.ht === 3 ? '#e0f2fe' : 'transparent',
            padding: '2px 6px',
            borderRadius: '3px',
            cursor: 'pointer',
          }}
        >
          ⇥
        </button>

        <div style={{ width: '1px', height: '16px', background: '#e2e8f0', margin: '0 4px' }} />

        {/* Number formatting badges */}
        <button
          type="button"
          title="货币格式 (¥)"
          style={{
            border: 'none',
            background: 'transparent',
            padding: '2px 6px',
            borderRadius: '3px',
            fontWeight: 600,
            cursor: 'pointer',
          }}
        >
          ¥
        </button>
        <button
          type="button"
          title="百分比 (%)"
          style={{
            border: 'none',
            background: 'transparent',
            padding: '2px 6px',
            borderRadius: '3px',
            fontWeight: 600,
            cursor: 'pointer',
          }}
        >
          %
        </button>
        <button
          type="button"
          title="千分位逗号 (,)"
          style={{
            border: 'none',
            background: 'transparent',
            padding: '2px 6px',
            borderRadius: '3px',
            fontWeight: 600,
            cursor: 'pointer',
          }}
        >
          ,
        </button>

        <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span style={{ fontSize: '11px', color: '#64748b' }}>
            {sheet.freeze?.ySplit ? `❄ 已冻结第 ${sheet.freeze.ySplit} 行表头` : ''}
          </span>
        </div>
      </div>

      {/* 3. Univer Formula Bar (fx) */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          padding: '4px 10px',
          background: '#ffffff',
          borderBottom: '1px solid #cbd5e1',
          fontSize: '12px',
        }}
      >
        {/* Active Cell Coord Box */}
        <div
          style={{
            width: '64px',
            height: '24px',
            border: '1px solid #cbd5e1',
            borderRadius: '3px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontWeight: 600,
            color: '#1e293b',
            background: '#f8fafc',
            fontSize: '12px',
          }}
        >
          {activeCoordName}
        </div>

        {/* Formula Symbol fx */}
        <span
          style={{
            fontStyle: 'italic',
            fontWeight: 700,
            color: '#64748b',
            fontFamily: 'serif',
            fontSize: '14px',
          }}
        >
          fx
        </span>

        {/* Formula / Value Input Bar */}
        <div
          style={{
            flex: 1,
            height: '24px',
            border: '1px solid #e2e8f0',
            borderRadius: '3px',
            padding: '0 8px',
            display: 'flex',
            alignItems: 'center',
            color: '#1e293b',
            background: '#ffffff',
            fontSize: '12.5px',
          }}
        >
          {currentCellData?.f ? (
            <span style={{ color: '#0369a1', fontFamily: 'monospace', fontWeight: 600 }}>
              {currentCellData.f}
            </span>
          ) : (
            <span>{currentCellData?.v !== undefined ? String(currentCellData.v) : ''}</span>
          )}
        </div>
      </div>

      {/* 4. Univer Interactive Grid Viewport */}
      <div
        style={{
          height,
          overflow: 'auto',
          position: 'relative',
          background: '#f1f5f9',
        }}
      >
        <table
          style={{
            borderCollapse: 'separate',
            borderSpacing: 0,
            backgroundColor: '#ffffff',
            fontSize: '12px',
            width: 'max-content',
            minWidth: '100%',
          }}
        >
          {/* Column Header (A, B, C, D...) */}
          <thead>
            <tr>
              {/* Corner Cell */}
              <th
                style={{
                  position: 'sticky',
                  top: 0,
                  left: 0,
                  zIndex: 30,
                  width: '42px',
                  minWidth: '42px',
                  height: '26px',
                  background: '#f1f5f9',
                  borderRight: '1px solid #cbd5e1',
                  borderBottom: '1px solid #cbd5e1',
                }}
              />
              {Array.from({ length: colCount }, (_, cIdx) => {
                const colLetter = colToLetter(cIdx);
                const colWidth = sheet.columnData?.[String(cIdx)]?.w || sheet.defaultColumnWidth || 100;
                const isColActive = selectedCell.col === cIdx;
                return (
                  <th
                    key={cIdx}
                    style={{
                      position: 'sticky',
                      top: 0,
                      zIndex: 20,
                      width: `${colWidth}px`,
                      minWidth: `${colWidth}px`,
                      maxWidth: `${colWidth}px`,
                      height: '26px',
                      background: isColActive ? '#e2e8f0' : '#f8fafc',
                      borderRight: '1px solid #cbd5e1',
                      borderBottom: '1px solid #cbd5e1',
                      color: isColActive ? '#0f172a' : '#64748b',
                      fontWeight: isColActive ? 700 : 500,
                      fontSize: '11px',
                      textAlign: 'center',
                      userSelect: 'none',
                    }}
                  >
                    {colLetter}
                  </th>
                );
              })}
            </tr>
          </thead>

          <tbody>
            {Array.from({ length: rowCount }, (_, rIdx) => {
              const isFrozenHeader = sheet.freeze?.ySplit ? rIdx < sheet.freeze.ySplit : rIdx === 0;
              const isRowActive = selectedCell.row === rIdx;
              const rowHeight = sheet.rowData?.[String(rIdx)]?.h || (isFrozenHeader ? 34 : 26);

              return (
                <tr key={rIdx} style={{ height: `${rowHeight}px` }}>
                  {/* Row Header (1, 2, 3...) */}
                  <td
                    style={{
                      position: 'sticky',
                      left: 0,
                      zIndex: 10,
                      width: '42px',
                      minWidth: '42px',
                      background: isRowActive ? '#e2e8f0' : '#f8fafc',
                      borderRight: '1px solid #cbd5e1',
                      borderBottom: '1px solid #e2e8f0',
                      color: isRowActive ? '#0f172a' : '#64748b',
                      fontWeight: isRowActive ? 700 : 500,
                      fontSize: '11px',
                      textAlign: 'center',
                      userSelect: 'none',
                    }}
                  >
                    {rIdx + 1}
                  </td>

                  {/* Cells */}
                  {Array.from({ length: colCount }, (_, cIdx) => {
                    const cell = sheet.cellData?.[String(rIdx)]?.[String(cIdx)];
                    const isSelected = selectedCell.row === rIdx && selectedCell.col === cIdx;
                    const colWidth = sheet.columnData?.[String(cIdx)]?.w || sheet.defaultColumnWidth || 100;

                    // Parse styles from Univer IStyleData
                    const bg = cell?.s?.bg?.rgb || (isFrozenHeader ? '#f8fafc' : '#ffffff');
                    const color = cell?.s?.cl?.rgb || '#1e293b';
                    const bold = cell?.s?.bl === 1;
                    const italic = cell?.s?.it === 1;
                    const align =
                      cell?.s?.ht === 3 ? 'right' : cell?.s?.ht === 2 ? 'center' : 'left';

                    const hasFormula = Boolean(cell?.f);

                    return (
                      <td
                        key={cIdx}
                        onClick={() => setSelectedCell({ row: rIdx, col: cIdx })}
                        style={{
                          width: `${colWidth}px`,
                          minWidth: `${colWidth}px`,
                          maxWidth: `${colWidth}px`,
                          height: `${rowHeight}px`,
                          backgroundColor: isSelected ? '#eff6ff' : bg,
                          color,
                          fontWeight: bold ? 700 : 400,
                          fontStyle: italic ? 'italic' : 'normal',
                          textAlign: align,
                          padding: '3px 8px',
                          borderRight: isSelected ? '2px solid #2563eb' : '1px solid #e2e8f0',
                          borderBottom: isSelected
                            ? '2px solid #2563eb'
                            : isFrozenHeader
                            ? '2px solid #94a3b8'
                            : '1px solid #e2e8f0',
                          borderTop: isSelected ? '2px solid #2563eb' : undefined,
                          borderLeft: isSelected ? '2px solid #2563eb' : undefined,
                          outline: isSelected ? '1px solid #2563eb' : 'none',
                          outlineOffset: '-1px',
                          whiteSpace: 'nowrap',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                          cursor: 'cell',
                          position: 'relative',
                        }}
                      >
                        {/* Little formula indicator badge */}
                        {hasFormula && (
                          <span
                            title={`公式: ${cell?.f}`}
                            style={{
                              position: 'absolute',
                              top: '1px',
                              right: '2px',
                              fontSize: '9px',
                              color: '#2563eb',
                              fontFamily: 'serif',
                              fontWeight: 700,
                              lineHeight: 1,
                            }}
                          >
                            ƒ
                          </span>
                        )}

                        {cell?.v !== undefined
                          ? typeof cell.v === 'number'
                            ? cell.v.toLocaleString('zh-CN')
                            : String(cell.v)
                          : ''}

                        {/* Active Selection Handle at bottom-right corner */}
                        {isSelected && (
                          <div
                            style={{
                              position: 'absolute',
                              right: '-3px',
                              bottom: '-3px',
                              width: '6px',
                              height: '6px',
                              backgroundColor: '#2563eb',
                              border: '1px solid #ffffff',
                              cursor: 'crosshair',
                            }}
                          />
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

      {/* 5. Univer Bottom Sheet Tabs & Status Bar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '4px 12px',
          background: '#f8fafc',
          borderTop: '1px solid #e2e8f0',
          fontSize: '11.5px',
          color: '#475569',
          flexWrap: 'wrap',
          gap: '10px',
        }}
      >
        {/* Sheet Tabs */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
          {(snapshot.sheetOrder || ['rpx-sheet-1']).map((sKey) => {
            const sh = snapshot.sheets[sKey];
            const isActive = activeSheetKey === sKey;
            return (
              <button
                key={sKey}
                type="button"
                onClick={() => setActiveSheetKey(sKey)}
                style={{
                  padding: '4px 12px',
                  background: isActive ? '#ffffff' : '#f1f5f9',
                  color: isActive ? '#0f172a' : '#64748b',
                  fontWeight: isActive ? 600 : 500,
                  border: '1px solid #cbd5e1',
                  borderBottom: isActive ? '2px solid #10b981' : '1px solid #cbd5e1',
                  borderRadius: '4px 4px 0 0',
                  cursor: 'pointer',
                  fontSize: '11.5px',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                }}
              >
                <span>📊 {sh?.name || 'Sheet1'}</span>
              </button>
            );
          })}
          <button
            type="button"
            title="新增工作表"
            style={{
              border: 'none',
              background: 'transparent',
              fontSize: '14px',
              color: '#64748b',
              cursor: 'pointer',
              padding: '2px 6px',
            }}
            onClick={() => toast('多 Sheet 特性按 M4-05 标准运行')}
          >
            ＋
          </button>
        </div>

        {/* Aggregates Summary for Selected Range */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
          <span>
            活动格: <b>{activeCoordName}</b>
          </span>
          <span>
            计数: <b>{selectionStats.count}</b>
          </span>
          {selectionStats.hasNum && (
            <>
              <span>
                求和: <b>¥{selectionStats.sum.toLocaleString('zh-CN')}</b>
              </span>
              <span>
                平均值: <b>¥{selectionStats.avg.toLocaleString('zh-CN')}</b>
              </span>
            </>
          )}

          {/* Zoom controls */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '4px', marginLeft: '6px' }}>
            <button
              type="button"
              style={{ border: 'none', background: 'transparent', cursor: 'pointer', fontSize: '12px' }}
              onClick={() => setZoom((z) => Math.max(50, z - 10))}
            >
              －
            </button>
            <span style={{ fontSize: '11px', minWidth: '38px', textAlign: 'center' }}>
              {zoom}%
            </span>
            <button
              type="button"
              style={{ border: 'none', background: 'transparent', cursor: 'pointer', fontSize: '12px' }}
              onClick={() => setZoom((z) => Math.min(150, z + 10))}
            >
              ＋
            </button>
          </div>
        </div>
      </div>

      {/* Snapshot JSON Modal */}
      {showJsonModal && (
        <div
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0,0,0,0.5)',
            zIndex: 9999,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            padding: '20px',
          }}
          onClick={() => setShowJsonModal(false)}
        >
          <div
            style={{
              background: '#ffffff',
              borderRadius: '8px',
              maxWidth: '780px',
              width: '100%',
              maxHeight: '85vh',
              display: 'flex',
              flexDirection: 'column',
              boxShadow: '0 20px 25px -5px rgba(0, 0, 0, 0.1)',
              overflow: 'hidden',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                padding: '12px 16px',
                borderBottom: '1px solid #e2e8f0',
                background: '#f8fafc',
              }}
            >
              <div>
                <b style={{ fontSize: '14px' }}>Univer IWorkbookData 快照契约</b>
                <div style={{ fontSize: '11.5px', color: '#64748b' }}>
                  严格遵循 docs.univer.ai 官方标准，可直接供 @univerjs/preset-sheets-core 消费
                </div>
              </div>
              <button
                type="button"
                className="btn small"
                onClick={() => setShowJsonModal(false)}
              >
                ✕ 关闭
              </button>
            </div>
            <div style={{ padding: '16px', overflowY: 'auto', flex: 1, background: '#1e1e1e' }}>
              <pre
                style={{
                  margin: 0,
                  fontSize: '12px',
                  color: '#4ec9b0',
                  fontFamily: 'Consolas, monospace',
                  whiteSpace: 'pre-wrap',
                }}
              >
                {JSON.stringify(snapshot, null, 2)}
              </pre>
            </div>
            <div
              style={{
                display: 'flex',
                justifyContent: 'flex-end',
                padding: '10px 16px',
                borderTop: '1px solid #e2e8f0',
                gap: '8px',
              }}
            >
              <button type="button" className="btn small" onClick={copySnapshot}>
                复制 JSON
              </button>
              <button
                type="button"
                className="btn small primary"
                onClick={() => setShowJsonModal(false)}
              >
                确定
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
