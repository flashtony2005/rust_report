/**
 * 轻量交互式网格（零依赖，不引 Univer / AG Grid）。
 *
 * 能力：
 *  - **拖动表头左右换列序**（pointer 事件，不是 HTML5 DnD，触屏也能用）
 *  - **拖动列右边缘改列宽**（双击列边复位该列）
 *  - 列序与列宽按 `storageKey` 记到 localStorage，刷新/重开报表仍在
 *  - 表头吸顶、斑马行、数字右对齐等宽数字
 *
 * 只做展示与交互，不改数据：`rows[0]` 当表头，其余为数据行。
 */
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';

export interface LightGridProps {
  /** 第一行是表头，其余是数据行 */
  rows: (string | number)[][];
  /** 记忆键（一般用 reportId）；不传则不持久化 */
  storageKey?: string;
  /** 紧凑模式（行高更小） */
  dense?: boolean;
}

interface GridPrefs {
  order: number[];
  widths: Record<number, number>;
  /** 单元格自动换行（默认开） */
  wrap: boolean;
}

const MIN_W = 60;
const MAX_W = 640;
const RESIZE_HANDLE = 7;

/** 估算默认列宽：中文按 2 个字宽算。
 *  跨列合并的横幅行（标题/脚注）只有 1 个非空格，会把列宽撑爆 → 跳过；
 *  单格按 18 个字宽上限折算，超长文本交给省略号。 */
function autoWidth(colIndex: number, rows: (string | number)[][], col: number): number {
  let best = 6;
  for (const r of rows) {
    if (!r) continue;
    const filled = r.filter((v) => v !== '' && v != null).length;
    if (filled < 2) continue; // 合并横幅行不参与列宽估算
    const v = r[col];
    if (v == null || v === '') continue;
    const s = String(v);
    let len = 0;
    for (const ch of s) len += /[\u4e00-\u9fa5\uff00-\uffef]/.test(ch) ? 2 : 1;
    if (len > 18) len = 18;
    if (len > best) best = len;
  }
  const base = colIndex === 0 ? 132 : 96;
  return Math.max(base, Math.min(MAX_W, Math.round(16 + best * 7.2)));
}

function loadPrefs(key: string | undefined, colCount: number, defaults: number[]): GridPrefs {
  const fallback: GridPrefs = {
    order: Array.from({ length: colCount }, (_, i) => i),
    widths: Object.fromEntries(defaults.map((w, i) => [i, w])),
    wrap: true,
  };
  if (!key) return fallback;
  try {
    const raw = localStorage.getItem(`rpx.grid.${key}`);
    if (!raw) return fallback;
    const j = JSON.parse(raw) as Partial<GridPrefs>;
    const order =
      Array.isArray(j.order) && j.order.length === colCount && j.order.every((v) => v >= 0 && v < colCount)
        ? j.order
        : fallback.order;
    const widths: Record<number, number> = { ...fallback.widths };
    if (j.widths && typeof j.widths === 'object') {
      for (const [k, v] of Object.entries(j.widths)) {
        const idx = Number(k);
        if (Number.isFinite(idx) && typeof v === 'number' && v >= MIN_W && v <= MAX_W) widths[idx] = Math.round(v);
      }
    }
    return { order, widths, wrap: typeof j.wrap === 'boolean' ? j.wrap : true };
  } catch {
    return fallback;
  }
}

export const LightGrid: React.FC<LightGridProps> = ({ rows, storageKey, dense }) => {
  const colCount = rows[0]?.length ?? 0;
  const headSig = (rows[0] ?? []).join('|');
  const defaults = useMemo(
    () => Array.from({ length: colCount }, (_, c) => autoWidth(c, rows, c)),
    // 只按"列数 + 表头文案"重算：数据行变化不该跳列宽
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [colCount, headSig]
  );
  const initial = useMemo(() => loadPrefs(storageKey, colCount, defaults), [storageKey, colCount, defaults]);

  const [order, setOrder] = useState<number[]>(initial.order);
  const [widths, setWidths] = useState<Record<number, number>>(initial.widths);
  const [wrap, setWrap] = useState<boolean>(initial.wrap);
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);
  const [resizing, setResizing] = useState<number | null>(null);

  // 换报表时重置
  useEffect(() => {
    setOrder(initial.order);
    setWidths(initial.widths);
    setWrap(initial.wrap);
  }, [initial]);

  const persist = useCallback(
    (nextOrder: number[], nextWidths: Record<number, number>, nextWrap?: boolean) => {
      if (!storageKey) return;
      try {
        localStorage.setItem(
          `rpx.grid.${storageKey}`,
          JSON.stringify({ order: nextOrder, widths: nextWidths, wrap: nextWrap ?? wrap })
        );
      } catch {
        /* 存不下不影响使用 */
      }
    },
    [storageKey, wrap]
  );

  const move = (from: number, to: number) => {
    if (from === to || from < 0 || to < 0) return;
    const next = [...order];
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    setOrder(next);
    persist(next, widths);
  };

  /* ------------------------------ 列宽拖拽 ------------------------------ */
  const resizeRef = useRef<{ col: number; startX: number; startW: number } | null>(null);
  const onResizeDown = (e: React.PointerEvent, col: number) => {
    e.preventDefault();
    e.stopPropagation();
    resizeRef.current = { col, startX: e.clientX, startW: widths[col] ?? defaults[col] ?? 100 };
    setResizing(col);
    (e.target as HTMLElement).setPointerCapture?.(e.pointerId);
  };
  useEffect(() => {
    if (resizing == null) return;
    const onMove = (ev: PointerEvent) => {
      const st = resizeRef.current;
      if (!st) return;
      const w = Math.max(MIN_W, Math.min(MAX_W, Math.round(st.startW + (ev.clientX - st.startX))));
      setWidths((prev) => ({ ...prev, [st.col]: w }));
    };
    const onUp = () => {
      const st = resizeRef.current;
      setResizing(null);
      resizeRef.current = null;
      if (st) {
        setWidths((prev) => {
          const merged = { ...prev, [st.col]: prev[st.col] ?? st.startW };
          persist(order, merged);
          return merged;
        });
      }
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  }, [resizing, order, persist]);

  /* ------------------------------ 列序拖拽 ------------------------------ */
  const dragRef = useRef<{ from: number; startX: number; started: boolean } | null>(null);
  const onHeadPointerDown = (e: React.PointerEvent, displayIdx: number) => {
    if (e.button !== 0) return;
    dragRef.current = { from: displayIdx, startX: e.clientX, started: false };
  };
  useEffect(() => {
    const onMove = (ev: PointerEvent) => {
      const st = dragRef.current;
      if (!st) return;
      if (!st.started) {
        if (Math.abs(ev.clientX - st.startX) < 8) return;
        st.started = true;
        setDragFrom(st.from);
        document.body.style.cursor = 'grabbing';
        document.body.style.userSelect = 'none';
      }
      const el = document.elementFromPoint(ev.clientX, ev.clientY);
      const th = el?.closest?.('th[data-col]') as HTMLElement | null;
      setDragOver(th ? Number(th.dataset.col) : null);
    };
    const onUp = (ev: PointerEvent) => {
      const st = dragRef.current;
      dragRef.current = null;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      if (!st?.started) {
        setDragFrom(null);
        setDragOver(null);
        return;
      }
      const el = document.elementFromPoint(ev.clientX, ev.clientY);
      const th = el?.closest?.('th[data-col]') as HTMLElement | null;
      if (th && Number.isFinite(Number(th.dataset.col))) move(st.from, Number(th.dataset.col));
      setDragFrom(null);
      setDragOver(null);
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [order, widths]);

  const resetPrefs = () => {
    const freshOrder = Array.from({ length: colCount }, (_, i) => i);
    const freshWidths = Object.fromEntries(defaults.map((w, i) => [i, w]));
    setOrder(freshOrder);
    setWidths(freshWidths);
    persist(freshOrder, freshWidths);
  };

  if (colCount === 0) return null;

  const totalWidth = order.reduce((a, c) => a + (widths[c] ?? defaults[c] ?? 100), 0);
  const pad = dense ? '7px 10px' : '10px 14px';

  return (
    <div data-grid="light">
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 12,
          marginBottom: 8,
          fontSize: 11.5,
          color: 'var(--ink-3)',
        }}
      >
        <span>↔ 拖表头换列序 · ⇥ 拖列边调宽 · 双击列边复位</span>
        <span style={{ flex: 1 }} />
        <button
          type="button"
          className="btn small ghost"
          onClick={() => {
            const next = !wrap;
            setWrap(next);
            persist(order, widths, next);
          }}
          title={wrap ? '当前：长文本自动换行（点击改为单行省略）' : '当前：单行省略（点击改为自动换行）'}
        >
          {wrap ? '¶ 换行：开' : '¶ 换行：关'}
        </button>
        <button type="button" className="btn small ghost" onClick={resetPrefs} title="列序与列宽恢复默认">
          ↺ 复位列
        </button>
      </div>

      <div style={{ overflowX: 'auto', border: '1px solid var(--border)', borderRadius: 'var(--r-s)' }}>
        <table
          style={{
            borderCollapse: 'collapse',
            tableLayout: 'fixed',
            width: Math.max(totalWidth, 640),
            fontSize: 13.5,
            background: '#fff',
          }}
        >
          <colgroup>
            {order.map((c) => (
              <col key={c} style={{ width: widths[c] ?? defaults[c] ?? 100 }} />
            ))}
          </colgroup>

          <thead>
            <tr style={{ background: 'var(--paper-3)' }}>
              {order.map((c, displayIdx) => {
                const isLast = displayIdx === order.length - 1;
                return (
                  <th
                    key={c}
                    data-col={displayIdx}
                    onPointerDown={(e) => onHeadPointerDown(e, displayIdx)}
                    title="按住左右拖动可换列序"
                    style={{
                      position: 'sticky',
                      top: 0,
                      zIndex: 2,
                      padding: pad,
                      paddingRight: RESIZE_HANDLE + 10,
                      textAlign: c === 0 ? 'left' : 'right',
                      fontWeight: 600,
                      fontSize: 12.5,
                      fontFamily: c === 0 ? 'inherit' : 'var(--font-mono)',
                      color: 'var(--ink-2)',
                      background: 'var(--paper-3)',
                      borderBottom: '1px solid var(--line-2)',
                      borderRight: isLast ? 'none' : '1px solid var(--border-subtle)',
                      cursor: 'grab',
                      userSelect: 'none',
                      opacity: dragFrom === displayIdx ? 0.45 : 1,
                      outline: dragOver === displayIdx ? '2px solid var(--accent)' : 'none',
                      outlineOffset: -2,
                      overflow: wrap ? 'visible' : 'hidden',
                      textOverflow: wrap ? 'clip' : 'ellipsis',
                      whiteSpace: wrap ? 'normal' : 'nowrap',
                      wordBreak: wrap ? 'break-word' : 'normal',
                      lineHeight: 1.45,
                    }}
                  >
                    {rows[0][c]}
                    {/* 列宽手柄 */}
                    <span
                      onPointerDown={(e) => onResizeDown(e, c)}
                      onDoubleClick={(e) => {
                        e.stopPropagation();
                        const w = defaults[c] ?? 100;
                        const next = { ...widths, [c]: w };
                        setWidths(next);
                        persist(order, next);
                      }}
                      title="拖动改列宽，双击复位"
                      style={{
                        position: 'absolute',
                        top: 0,
                        right: 0,
                        width: RESIZE_HANDLE,
                        height: '100%',
                        cursor: 'col-resize',
                        background: resizing === c ? 'var(--accent)' : 'transparent',
                        borderRight: isLast ? 'none' : '1px solid transparent',
                      }}
                    />
                  </th>
                );
              })}
            </tr>
          </thead>

          <tbody>
            {rows.slice(1).map((r, rIdx) => {
              const isTotal = String(r[0] || '').includes('合计') || String(r[0] || '').includes('总计');
              const isRate = String(r[0] || '').includes('率');
              return (
                <tr
                  key={rIdx}
                  style={{
                    background: isTotal ? 'var(--paper-3)' : isRate ? '#f4f8fa' : rIdx % 2 === 1 ? '#fdfcf9' : '#ffffff',
                    fontWeight: isTotal || isRate ? 700 : 400,
                  }}
                >
                  {order.map((c, displayIdx) => (
                    <td
                      key={c}
                      style={{
                        padding: pad,
                        textAlign: c === 0 ? 'left' : 'right',
                        color: isTotal ? 'var(--accent-ink)' : 'var(--ink)',
                        fontVariantNumeric: 'tabular-nums',
                        fontFamily: c === 0 ? 'inherit' : 'var(--font-mono)',
                        borderTop: '1px solid var(--border-subtle)',
                        borderRight: displayIdx === order.length - 1 ? 'none' : '1px solid var(--border-subtle)',
                        overflow: wrap ? 'visible' : 'hidden',
                        textOverflow: wrap ? 'clip' : 'ellipsis',
                        whiteSpace: wrap ? 'normal' : 'nowrap',
                        wordBreak: wrap ? 'break-word' : 'normal',
                        lineHeight: 1.5,
                      }}
                    >
                      {r[c] ?? ''}
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
};
