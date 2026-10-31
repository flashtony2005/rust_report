import React, { useEffect, useState } from 'react';
import { getReport, saveTemplate, type ReportDetail } from '../services/api';
import { reasonOf } from '../services/reports';

/**
 * 设计器（轻量版）—— 精密排版与模板单元格设计工作台。
 *
 * 遵循 Awwwards / Webby 高级工程美学：
 * - 瑞士制表般的网格坐标标尺与单元格对齐
 * - 精密公式输入条（A1 坐标定位、fx 语法微标识、快捷回车应用）
 * - 完整的修订状态校验与微动效反馈
 */

/** rpx-report/v1 单元格中本页用到的字段。 */
interface CellDef {
  row: number;
  col: number;
  text?: string;
  expr?: string;
  literal?: { value?: unknown };
  merge?: { rows?: number; cols?: number };
  bold?: boolean;
  font_size?: number;
  fore_color?: string;
  back_color?: string;
  h_align?: string;
  v_align?: string;
  format?: string;
  extend_mode?: string;
  [k: string]: unknown;
}

interface ReportDef {
  schema?: string;
  version?: number;
  format?: string;
  grid: { rows: number; cols: number };
  cells?: CellDef[];
  datasets?: Array<{ name?: string; [k: string]: unknown }>;
  [k: string]: unknown;
}

/** 单元格的展示内容：公式带 `=` 前缀，让用户一眼分清"文本"与"公式"。 */
function cellDisplay(c: CellDef | undefined): string {
  if (!c) return '';
  if (typeof c.text === 'string') return c.text;
  if (typeof c.expr === 'string') return `=${c.expr}`;
  return c.literal?.value == null ? '' : String(c.literal.value);
}

/** 单元格的可编辑内容（与展示一致）。 */
function cellContent(c: CellDef | undefined): string {
  return cellDisplay(c);
}

/** 逻辑坐标 → 地址（A1 风格），与 5288 的 `template.ts::address` 同规则。 */
function address(row0: number, col0: number): string {
  let s = '';
  for (let c = col0 + 1; c > 0; c = Math.floor((c - 1) / 26)) {
    s = String.fromCharCode(65 + (((c - 1) % 26) + 26) % 26) + s;
  }
  return `${s}${row0 + 1}`;
}

/** 合并区域里"非左上角"的槽（这些格不可编辑，提示先编辑左上角）。 */
function coveredSlots(def: ReportDef): Set<string> {
  const out = new Set<string>();
  for (const c of def.cells ?? []) {
    const rows = c.merge?.rows ?? 1;
    const cols = c.merge?.cols ?? 1;
    for (let r = 0; r < rows; r++) {
      for (let col = 0; col < cols; col++) {
        if (r === 0 && col === 0) continue;
        out.add(`${c.row + r}-${c.col + col}`);
      }
    }
  }
  return out;
}

function cellStyle(c: CellDef | undefined): React.CSSProperties {
  const s: React.CSSProperties = {
    padding: '8px 14px',
    border: '1px solid var(--border)',
    minWidth: 90,
    verticalAlign: 'middle',
    fontVariantNumeric: 'tabular-nums',
  };
  if (!c) return s;
  if (c.bold) s.fontWeight = 700;
  if (typeof c.font_size === 'number') s.fontSize = `${Math.max(10, c.font_size)}px`;
  if (typeof c.fore_color === 'string') s.color = c.fore_color;
  if (typeof c.back_color === 'string') s.backgroundColor = c.back_color;
  if (c.h_align === 'center') s.textAlign = 'center';
  else if (c.h_align === 'right') s.textAlign = 'right';
  else s.textAlign = 'left';
  return s;
}

interface Props {
  reportId: string;
  onBack: () => void;
}

export const DesignerView: React.FC<Props> = ({ reportId, onBack }) => {
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [error, setError] = useState<string | null>(null);
  const [detail, setDetail] = useState<ReportDetail | null>(null);
  const [def, setDef] = useState<ReportDef | null>(null);
  const [revision, setRevision] = useState('');
  const [selected, setSelected] = useState<{ row0: number; col0: number } | null>(null);
  const [edit, setEdit] = useState('');
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveMsg, setSaveMsg] = useState<{ kind: 'ok' | 'err'; text: string } | null>(null);

  useEffect(() => {
    let alive = true;
    setStatus('loading');
    setSaveMsg(null);
    getReport(reportId)
      .then((d) => {
        if (!alive) return;
        setDetail(d);
        setRevision(d.revision);

        let tpl = (d.template || {}) as any;
        if (tpl && tpl.cells && !Array.isArray(tpl.cells)) {
          const cellArr: CellDef[] = [];
          let maxR = 0;
          let maxC = 0;
          for (const [k, val] of Object.entries(tpl.cells as Record<string, any>)) {
            const [rStr, cStr] = k.split(',');
            const r = parseInt(rStr, 10);
            const c = parseInt(cStr, 10);
            if (!isNaN(r) && !isNaN(c)) {
              if (r > maxR) maxR = r;
              if (c > maxC) maxC = c;
              cellArr.push({
                row: r + 1,
                col: c + 1,
                text: typeof val?.v === 'string' ? val.v : String(val?.v ?? ''),
              });
            }
          }
          tpl = {
            grid: tpl.grid || { rows: maxR + 1, cols: maxC + 1 },
            cells: cellArr,
          };
        } else if (!tpl.grid) {
          tpl.grid = { rows: 5, cols: 5 };
        }

        setDef(tpl);
        setSelected(null);
        setEdit('');
        setDirty(false);
        setStatus('ready');
      })
      .catch((e) => {
        if (!alive) return;
        setStatus('error');
        setError(reasonOf(e));
      });
    return () => {
      alive = false;
    };
  }, [reportId]);

  const manifest = (detail?.manifest ?? {}) as { name?: string; description?: string; tags?: string[] };

  /** 点选单元格：合并区非左上角不可编辑。 */
  const pick = (row0: number, col0: number) => {
    if (!def) return;
    if (coveredSlots(def).has(`${row0 + 1}-${col0 + 1}`)) {
      setSaveMsg({ kind: 'err', text: `${address(row0, col0)} 在合并区域内，请选择合并区域的左上角单元格` });
      return;
    }
    const cell = def.cells?.find((c) => c.row === row0 + 1 && c.col === col0 + 1);
    setSelected({ row0, col0 });
    setEdit(cellContent(cell));
  };

  /** 把面板里的内容应用到本地副本。 */
  const applyEdit = () => {
    if (!def || !selected) return;
    const { row0, col0 } = selected;
    const row = row0 + 1;
    const col = col0 + 1;
    const content = edit;
    const cells = [...(def.cells ?? [])];
    const index = cells.findIndex((c) => c.row === row && c.col === col);
    const cell: CellDef = { ...(cells[index] ?? { row, col }) };
    delete cell.text;
    delete cell.expr;
    delete cell.literal;
    const trimmed = content.trim();
    if (trimmed !== '') {
      if (trimmed.startsWith('=')) cell.expr = trimmed.slice(1);
      else cell.text = trimmed;
    }
    if (index < 0 && (cell.text !== undefined || cell.expr !== undefined)) cells.push(cell);
    else if (index >= 0 && cell.text === undefined && cell.expr === undefined) cells.splice(index, 1);
    else if (index >= 0) cells[index] = cell;
    setDef({ ...def, cells });
    setDirty(true);
    setSaveMsg(null);
  };

  const save = async () => {
    if (!def) return;
    setSaving(true);
    setSaveMsg(null);
    try {
      const next = await saveTemplate(reportId, revision, def);
      setRevision(next.revision);
      setDetail(next);
      setDirty(false);
      setSaveMsg({
        kind: 'ok',
        text: `模板已保存（最新修订 ${next.revision.slice(0, 12)}）；已标记报表缓存为待刷新`,
      });
    } catch (e) {
      setSaveMsg({
        kind: 'err',
        text:
          e instanceof Error && e.message.includes('428')
            ? '模板已被他人修改（修订冲突）：请重新刷新后再编辑'
            : reasonOf(e),
      });
    } finally {
      setSaving(false);
    }
  };

  const deepLink = `http://127.0.0.1:5288/?report=${encodeURIComponent(reportId)}`;

  if (status === 'loading') {
    return (
      <div className="page-head">
        <div className="eyebrow">
          <button type="button" className="linklike tiny" onClick={onBack} style={{ color: 'inherit' }}>
            ← 返回报表
          </button>
        </div>
        <div className="empty" style={{ padding: 60 }}>
          <div className="spin" style={{ width: 16, height: 16, marginBottom: 12 }} />
          <div>读取模板定义中…</div>
        </div>
      </div>
    );
  }

  if (status === 'error') {
    return (
      <div className="page-head">
        <div className="eyebrow">
          <button type="button" className="linklike tiny" onClick={onBack} style={{ color: 'inherit' }}>
            ← 返回报表
          </button>
        </div>
        <div className="card pad" style={{ borderColor: 'var(--err-border)' }}>
          <b style={{ color: 'var(--err)' }}>读不到这张报表的模板定义</b>
          <div className="muted" style={{ marginTop: 8, lineHeight: 1.8 }}>{error}</div>
        </div>
      </div>
    );
  }

  const covered = coveredSlots(def as ReportDef);
  const grid = (def as ReportDef).grid;
  const rows: React.ReactNode[] = [];

  // Column letters header (# | A | B | C...)
  const headerTds: React.ReactNode[] = [
    <th
      key="corner"
      style={{
        background: 'var(--paper-3)',
        border: '1px solid var(--border)',
        width: 48,
        height: 34,
        fontSize: 11,
        fontFamily: 'var(--font-mono)',
        color: 'var(--ink-4)',
        textAlign: 'center',
        userSelect: 'none',
      }}
    >
      #
    </th>,
  ];
  for (let c = 0; c < grid.cols; c++) {
    const isColActive = selected?.col0 === c;
    headerTds.push(
      <th
        key={`col-${c}`}
        style={{
          background: isColActive ? 'var(--accent-soft)' : 'var(--paper-3)',
          border: '1px solid var(--border)',
          padding: '8px 12px',
          fontSize: 12,
          fontFamily: 'var(--font-mono)',
          fontWeight: 700,
          color: isColActive ? 'var(--accent-ink)' : 'var(--ink-3)',
          textAlign: 'center',
          minWidth: 110,
          userSelect: 'none',
          transition: 'all var(--t-fast)',
        }}
      >
        {String.fromCharCode(65 + (c % 26))}
      </th>,
    );
  }
  rows.push(<tr key="header-letters">{headerTds}</tr>);

  for (let r = 0; r < grid.rows; r++) {
    const isRowActive = selected?.row0 === r;
    const tds: React.ReactNode[] = [
      <td
        key={`row-num-${r}`}
        style={{
          background: isRowActive ? 'var(--accent-soft)' : 'var(--paper-3)',
          border: '1px solid var(--border)',
          textAlign: 'center',
          fontSize: 12,
          fontFamily: 'var(--font-mono)',
          color: isRowActive ? 'var(--accent-ink)' : 'var(--ink-4)',
          fontWeight: 700,
          userSelect: 'none',
          width: 48,
          padding: '8px 0',
          transition: 'all var(--t-fast)',
        }}
      >
        {r + 1}
      </td>,
    ];
    for (let c = 0; c < grid.cols; c++) {
      const key = `${r + 1}-${c + 1}`;
      if (covered.has(key)) continue; // 合并区非左上角
      const cell = (def as ReportDef).cells?.find((x) => x.row === r + 1 && x.col === c + 1);
      const isSel = selected?.row0 === r && selected?.col0 === c;
      const spanRows = cell?.merge?.rows ?? 1;
      const spanCols = cell?.merge?.cols ?? 1;
      const hasContent = cell && (cell.text !== undefined || cell.expr !== undefined || cell.literal?.value !== undefined);

      tds.push(
        <td
          key={key}
          rowSpan={spanRows > 1 ? spanRows : undefined}
          colSpan={spanCols > 1 ? spanCols : undefined}
          onClick={() => pick(r, c)}
          style={{
            ...cellStyle(cell),
            padding: '10px 14px',
            border: '1px solid var(--border)',
            background: isSel
              ? '#fffbeb'
              : cell?.back_color || (r === 0 ? 'var(--paper-3)' : '#ffffff'),
            color: isSel ? 'var(--accent-ink)' : cell?.fore_color || 'var(--ink)',
            cursor: 'pointer',
            boxShadow: isSel ? 'inset 0 0 0 2px var(--accent-2), 0 0 10px rgba(217, 119, 6, 0.15)' : 'none',
            fontWeight: cell?.bold || r === 0 ? 700 : 400,
            fontSize: 13.5,
            position: 'relative',
            transition: 'background var(--t-fast), box-shadow var(--t-fast)',
          }}
          title={cell ? `${address(r, c)}${cell.expr ? '（公式）' : cell.text !== undefined ? '（文本）' : ''}` : address(r, c)}
        >
          {cellDisplay(cell) || (hasContent ? '' : <span style={{ color: 'var(--ink-4)', opacity: 0.25 }}>—</span>)}
          {isSel ? (
            <span
              style={{
                position: 'absolute',
                right: -3,
                bottom: -3,
                width: 6,
                height: 6,
                background: 'var(--accent-2)',
                borderRadius: 1,
                pointerEvents: 'none',
              }}
            />
          ) : null}
        </td>,
      );
    }
    rows.push(<tr key={r}>{tds}</tr>);
  }

  const selectedCell = selected
    ? (def as ReportDef).cells?.find((x) => x.row === selected.row0 + 1 && x.col === selected.col0 + 1)
    : undefined;

  return (
    <>
      {/* 顶部标题区 */}
      <div
        className="page-head"
        style={{
          borderBottom: '1px solid var(--border-subtle)',
          paddingBottom: '22px',
          marginBottom: '24px',
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
              ← 返回报表详情
            </button>
            <span style={{ margin: '0 8px', color: 'var(--line-2)' }}>/</span>
            <span style={{ color: 'var(--accent-ink)' }}>TEMPLATE DESIGNER</span>
          </div>

          <h1
            style={{
              fontFamily: 'var(--font-serif)',
              fontSize: 'clamp(28px, 3vw, 38px)',
              fontWeight: 700,
              letterSpacing: '-0.02em',
              color: 'var(--ink)',
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
            <code className="sql-inline" style={{ fontSize: '12px' }}>{reportId}</code>
            <span className="tiny" style={{ color: 'var(--ink-3)' }}>
              · 修订 <code>{revision.slice(0, 8)}</code>
            </span>
            <span className="tiny" style={{ color: 'var(--ink-3)' }}>
              · 规格 {grid.rows} 行 × {grid.cols} 列
            </span>
            <span className="tiny" style={{ color: 'var(--ink-4)' }}>
              · 活跃单元格 {((def as ReportDef).cells ?? []).length} 个
            </span>
            {(def as ReportDef).datasets?.length ? (
              <span className="tiny" style={{ color: 'var(--ink-4)' }}>
                · 数据集 {(def as ReportDef).datasets!.map((d) => d.name ?? '?').join(', ')}
              </span>
            ) : null}
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
          <button
            type="button"
            className="btn"
            onClick={() => window.open(deepLink, '_blank', 'noopener')}
            title={`在完整设计器（Univer，端口 5288）中深度排版 ${reportId}`}
          >
            完整设计器 5288 ↗
          </button>

          <button
            type="button"
            className="btn primary"
            onClick={() => void save()}
            disabled={saving || !dirty || status !== 'ready'}
            title={dirty ? '保存修改并提交新修订' : '当前无改动'}
          >
            {saving ? (
              <>
                <span className="spin" style={{ width: 12, height: 12, borderWidth: 2 }} />
                <span>保存中…</span>
              </>
            ) : dirty ? (
              '保存改动 ✦'
            ) : (
              '已保存'
            )}
          </button>
        </div>
      </div>

      {manifest.description ? (
        <div
          style={{
            fontSize: '13.5px',
            color: 'var(--ink-2)',
            lineHeight: 1.7,
            marginBottom: '18px',
            background: 'var(--paper-2)',
            padding: '12px 18px',
            borderRadius: 'var(--r-m)',
            border: '1px solid var(--border)',
          }}
        >
          {manifest.description}
        </div>
      ) : null}

      {saveMsg ? (
        <div
          style={{
            padding: '12px 18px',
            borderRadius: 'var(--r-m)',
            marginBottom: '18px',
            fontSize: '13px',
            fontWeight: 500,
            background: saveMsg.kind === 'ok' ? 'var(--ok-bg)' : 'var(--err-bg)',
            color: saveMsg.kind === 'ok' ? 'var(--ok)' : 'var(--err)',
            border: `1px solid ${saveMsg.kind === 'ok' ? 'var(--ok-border)' : 'var(--err-border)'}`,
          }}
        >
          {saveMsg.text}
        </div>
      ) : null}

      {/* 设计器主工作台：上方标题栏 + 精密坐标网格 */}
      <div
        className="card"
        style={{
          borderRadius: 'var(--r-l)',
          overflow: 'hidden',
          marginBottom: '20px',
          boxShadow: 'var(--shadow-m)',
          border: '1px solid var(--line-2)',
        }}
      >
        <div
          style={{
            background: 'linear-gradient(180deg, #ffffff 0%, #faf8f5 100%)',
            padding: '14px 22px',
            borderBottom: '1px solid var(--border)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            flexWrap: 'wrap',
            gap: 12,
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <span
              style={{
                fontSize: 18,
                width: 34,
                height: 34,
                borderRadius: 'var(--r-s)',
                background: 'var(--accent-soft)',
                color: 'var(--accent-ink)',
                display: 'grid',
                placeItems: 'center',
              }}
            >
              📑
            </span>
            <div>
              <div
                style={{
                  fontFamily: 'var(--font-serif)',
                  fontSize: 16.5,
                  fontWeight: 700,
                  color: 'var(--ink)',
                }}
              >
                {manifest.name || reportId}
              </div>
              <div className="tiny" style={{ color: 'var(--ink-4)', marginTop: 2 }}>
                模板规格：{grid.rows} 行 × {grid.cols} 列 · 单元格 {((def as ReportDef).cells ?? []).length} 个
              </div>
            </div>
          </div>

          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <span
              style={{
                fontFamily: 'var(--font-mono)',
                fontSize: 11,
                padding: '3px 8px',
                borderRadius: 'var(--r-s)',
                background: 'var(--paper-3)',
                color: 'var(--ink-3)',
                border: '1px solid var(--border)',
              }}
            >
              轻量交互式网格
            </span>
          </div>
        </div>

        {/* 表格容器 */}
        <div style={{ overflowX: 'auto', padding: '18px', background: '#faf9f6' }}>
          <table
            style={{
              borderCollapse: 'collapse',
              width: 'max-content',
              minWidth: '100%',
              fontSize: 13.5,
              background: '#ffffff',
              boxShadow: 'var(--shadow-xs)',
              borderRadius: 'var(--r-s)',
              overflow: 'hidden',
            }}
          >
            <tbody>{rows.map((r) => r)}</tbody>
          </table>
        </div>
      </div>

      {/* 公式与单元格属性编辑栏（Formula Bar & Inspector） */}
      <div
        className="card"
        style={{
          boxShadow: 'var(--shadow-s)',
          border: '1px solid var(--border)',
          overflow: 'hidden',
        }}
      >
        <div
          style={{
            padding: '12px 20px',
            background: 'var(--paper-3)',
            borderBottom: '1px solid var(--border)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            flexWrap: 'wrap',
            gap: 10,
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <span
              style={{
                fontFamily: 'var(--font-mono)',
                fontSize: 11,
                fontWeight: 700,
                letterSpacing: '0.12em',
                textTransform: 'uppercase',
                color: 'var(--accent-ink)',
                background: 'var(--accent-soft)',
                padding: '3px 8px',
                borderRadius: 'var(--r-s)',
              }}
            >
              FORMULA BAR
            </span>

            {selected ? (
              <span style={{ fontSize: 13, color: 'var(--ink-2)', fontWeight: 600 }}>
                选中单元格：<code className="sql-inline" style={{ fontWeight: 700 }}>{address(selected.row0, selected.col0)}</code>
                {selectedCell?.format ? (
                  <span className="tiny" style={{ marginLeft: 8, color: 'var(--ink-4)' }}>
                    格式: {selectedCell.format}
                  </span>
                ) : null}
                {selectedCell?.extend_mode && selectedCell.extend_mode !== 'default' ? (
                  <span className="tiny" style={{ marginLeft: 8, color: 'var(--accent-ink)' }}>
                    扩展: {selectedCell.extend_mode}
                  </span>
                ) : null}
              </span>
            ) : (
              <span className="tiny" style={{ color: 'var(--ink-4)' }}>
                请点击上方网格中的任意单元格开始编辑
              </span>
            )}
          </div>

          {dirty ? (
            <span className="tiny" style={{ color: 'var(--warn)', fontWeight: 600 }}>
              ● 有未保存改动
            </span>
          ) : null}
        </div>

        <div style={{ padding: '18px 20px' }}>
          <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
            <div
              style={{
                width: 36,
                height: 38,
                borderRadius: 'var(--r-s)',
                background: 'var(--paper-3)',
                border: '1px solid var(--line-2)',
                display: 'grid',
                placeItems: 'center',
                fontFamily: 'var(--font-mono)',
                fontSize: 13,
                fontWeight: 700,
                color: 'var(--accent-ink)',
              }}
            >
              fx
            </div>

            <input
              style={{
                flex: 1,
                minWidth: 260,
                padding: '10px 14px',
                fontSize: 14,
                fontFamily: 'var(--font-mono)',
              }}
              value={edit}
              disabled={!selected}
              onChange={(e) => setEdit(e.target.value)}
              placeholder={selected ? '输入文本；或以 = 开头输入公式（例如：=bs.sum(closing)）' : '先点选网格单元格'}
              onKeyDown={(e) => {
                if (e.key === 'Enter') applyEdit();
              }}
            />

            <button
              type="button"
              className="btn"
              disabled={!selected || saving}
              onClick={applyEdit}
              style={{
                padding: '9px 18px',
                fontWeight: 600,
                borderColor: selected ? 'var(--accent)' : 'var(--line-2)',
                color: selected ? 'var(--accent-ink)' : 'inherit',
              }}
            >
              应用修改 ↵
            </button>
          </div>

          <div
            className="tiny muted"
            style={{
              marginTop: 12,
              lineHeight: 1.8,
              fontSize: '12px',
              borderTop: '1px dashed var(--border-subtle)',
              paddingTop: 10,
            }}
          >
            提示：清空输入框并应用将清除该单元格；按回车键即可快速确认。完成后点击右上角「保存改动」写入持久化配置。
          </div>
        </div>
      </div>
    </>
  );
};
