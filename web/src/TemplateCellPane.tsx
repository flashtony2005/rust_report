import { useEffect, useState } from 'react'
import type { ReportDef } from './designer'
import { address, cellDraft, type CellDraft } from './template'
import './template.css'

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
  return <section className="template-pane" aria-label="公式与主格">
    <h3>公式与主格 <span>{address(editing.row0, editing.col0)}</span></h3>
    <p>{disabled ? '切换到模板设计后编辑。' : '选择模板格，填写内容与主格关系。'}</p>
    <fieldset disabled={disabled || !draft}>
      <label>内容类型<select aria-label="单元格内容类型" value={draft?.kind ?? 'text'} onChange={e => update({ kind: e.target.value as CellDraft['kind'] })}>
        <option value="text">静态文本</option><option value="expr">报表公式</option>
      </select></label>
      <label>{draft?.kind === 'expr' ? '报表公式' : '单元格文本'}<textarea aria-label="单元格内容" rows={3} value={draft?.content ?? ''} onChange={e => update({ content: e.target.value })} placeholder="ds1.group(region)" /></label>
      <label>扩展方向<select aria-label="扩展方向" value={draft?.extend ?? 'default'} onChange={e => update({ extend: e.target.value as CellDraft['extend'] })}>
        <option value="default">自动判定</option><option value="none">不扩展</option><option value="vertical">纵向扩展 ↓</option><option value="horizontal">横向扩展 →</option>
      </select></label>
      <div className="template-pair"><label>左主格<input aria-label="左主格" value={draft?.left ?? ''} onChange={e => update({ left: e.target.value })} placeholder="留空自动 / `0 根 / D6" /></label>
        <label>上主格<input aria-label="上主格" value={draft?.top ?? ''} onChange={e => update({ top: e.target.value })} placeholder="留空自动 / `0 根 / E4" /></label></div>
      <p>留空自动推导；填写 <code>{'`0'}</code> 设为根，停止该轴继承。普通左主格须纵向扩展，上主格须横向扩展，可以位于右侧或下方。</p>
      <div className="template-pair"><label>合并行数<input aria-label="合并行数" type="number" min={1} value={draft?.rows ?? 1} onChange={e => update({ rows: Number(e.target.value) })}/></label>
        <label>合并列数<input aria-label="合并列数" type="number" min={1} value={draft?.cols ?? 1} onChange={e => update({ cols: Number(e.target.value) })}/></label></div>
      <div className="template-actions"><button disabled={!dirty} onClick={() => { if (draft && onApply(editing.row0, editing.col0, draft)) reset() }}>应用单元格定义</button>
        {dirty && <button onClick={reset}>放弃修改</button>}</div>
      {dirty && <p role="status">此格有未应用修改，请先应用或放弃，再运行或保存。</p>}
    </fieldset>
  </section>
}
