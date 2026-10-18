import { useEffect, useState } from 'react'
import type { ReportDef } from './designer'
import { address, cellDraft, type CellDraft } from './template'
import './template.css'

/**
 * 「单元格 → 内容与主格」分组的字段体（不含外框：分组标题由
 * [`PropertiesPanel`](./PropertiesPanel.tsx) 提供）。
 *
 * 改写要点：旧版是一整块纵向表单（每个 label 占满一行，主格说明文字占三行），
 * 在 340px 的侧栏里把「合并行数 / 合并列数」挤成上下两行。现在统一走
 * `.rpx-grid` 两列网格，和「字体」「边框」两组对齐。
 */
export default function TemplateCellPane({ def, selection, onApply, onDirtyChange, disabled }: {
  def: ReportDef | null
  selection: { row0: number; col0: number } | null
  disabled: boolean
  onApply: (row0: number, col0: number, draft: CellDraft) => boolean
  onDirtyChange: (dirty: boolean) => void
}) {
  const [editing, setEditing] = useState({ row0: 0, col0: 0 })
  const [draft, setDraft] = useState<CellDraft | null>(null)
  const [dirty, setDirty] = useState(false)
  const row = selection?.row0 ?? 0, col = selection?.col0 ?? 0
  useEffect(() => {
    if (!def || dirty) return
    setEditing({ row0: row, col0: col })
    setDraft(cellDraft(def, row, col))
  }, [def, row, col, dirty])
  const update = (patch: Partial<CellDraft>) => {
    setDraft(current => current && { ...current, ...patch })
    setDirty(true)
    onDirtyChange(true)
  }
  const reset = () => { setDirty(false); onDirtyChange(false) }
  const off = disabled || !draft
  return (
    <fieldset disabled={off} style={{ border: 0, padding: 0, margin: 0, minWidth: 0 }}>
      <div className="rpx-grid">
        <span className="k">地址</span>
        <span className="v" style={{ fontFamily: 'monospace', color: 'var(--rpx-accent)' }}>
          {address(editing.row0, editing.col0)}
          {dirty ? <span style={{ fontFamily: 'system-ui', color: '#9a6700', fontSize: 11 }}>· 未应用</span> : null}
        </span>

        <span className="k">类型</span>
        <span className="v">
          <select className="rpx-in" aria-label="单元格内容类型" value={draft?.kind ?? 'text'} onChange={e => update({ kind: e.target.value as CellDraft['kind'] })}>
            <option value="text">静态文本</option>
            <option value="expr">报表公式</option>
          </select>
        </span>

        <span className="span2">
          <label style={{ display: 'block' }}>
            <span style={{ display: 'block', fontSize: 12, color: 'var(--rpx-fg-muted)', marginBottom: 4 }}>
              {draft?.kind === 'expr' ? '报表公式' : '单元格文本'}
            </span>
            <textarea
              aria-label="单元格内容"
              rows={2}
              value={draft?.content ?? ''}
              onChange={e => update({ content: e.target.value })}
              placeholder="ds1.group(region)"
              style={{ width: '100%', boxSizing: 'border-box', font: '12px monospace', padding: '5px 7px', border: '1px solid var(--rpx-border)', borderRadius: 4, resize: 'vertical' }}
            />
          </label>
        </span>

        <span className="k">扩展</span>
        <span className="v">
          <select className="rpx-in" aria-label="扩展方向" value={draft?.extend ?? 'default'} onChange={e => update({ extend: e.target.value as CellDraft['extend'] })}>
            <option value="default">自动判定</option>
            <option value="none">不扩展</option>
            <option value="vertical">纵向扩展 ↓</option>
            <option value="horizontal">横向扩展 →</option>
          </select>
        </span>

        <span className="k">左主格</span>
        <span className="v">
          <input className="rpx-in" aria-label="左主格" value={draft?.left ?? ''} onChange={e => update({ left: e.target.value })} placeholder="留空自动 / `0 根 / D6" />
        </span>

        <span className="k">上主格</span>
        <span className="v">
          <input className="rpx-in" aria-label="上主格" value={draft?.top ?? ''} onChange={e => update({ top: e.target.value })} placeholder="留空自动 / `0 根 / E4" />
        </span>

        <span className="k">合并</span>
        <span className="v">
          <input className="rpx-in tiny" aria-label="合并行数" type="number" min={1} value={draft?.rows ?? 1} onChange={e => update({ rows: Number(e.target.value) })} />
          <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>行</span>
          <input className="rpx-in tiny" aria-label="合并列数" type="number" min={1} value={draft?.cols ?? 1} onChange={e => update({ cols: Number(e.target.value) })} />
          <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>列</span>
        </span>
      </div>

      <div className="rpx-actions">
        <button className="rpx-btn primary" disabled={!dirty} onClick={() => { if (draft && onApply(editing.row0, editing.col0, draft)) reset() }}>
          应用单元格定义
        </button>
        {dirty ? <button className="rpx-btn" onClick={reset}>放弃修改</button> : null}
      </div>
      <p className="rpx-hint">
        留空自动推导；填 <code>{'`0'}</code> 设为根、停止该轴继承。普通左主格须纵向扩展，上主格须横向扩展。
      </p>
      {dirty ? <p className="rpx-hint" role="status">此格有未应用修改，请先应用或放弃，再运行或保存。</p> : null}
    </fieldset>
  )
}
