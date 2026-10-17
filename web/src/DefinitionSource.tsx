import { useEffect, useState } from 'react'
import type { ReportDef } from './designer'
import { parseTemplate } from './template'
export default function DefinitionSource({ value, onApply, onDirty, onError }: {
 value: ReportDef | null; onApply: (value: ReportDef) => boolean; onDirty: (dirty: boolean) => void; onError: (message: string) => void
}) {
 const [text, setText] = useState('')
 const [dirty, setDirty] = useState(false)
 useEffect(() => { if (!dirty) setText(value ? JSON.stringify(value, null, 2) : '') }, [value, dirty])
 const change = (dirty: boolean) => { setDirty(dirty); onDirty(dirty) }
 return <details style={{ padding: '0 14px' }}><summary>JSON 定义源码{dirty ? ' · 有未应用修改' : ''}</summary>
  <textarea aria-label="JSON 定义源码" value={text} disabled={!value} style={{ width: '100%', height: 250, fontFamily: 'monospace' }} onChange={e => { setText(e.target.value); change(true) }} />
  <button disabled={!dirty} onClick={() => { try { if (onApply(parseTemplate(text))) change(false) } catch (error) { onError(String(error)) } }}>应用源码草稿</button>
  <button disabled={!dirty} onClick={() => { setText(value ? JSON.stringify(value, null, 2) : ''); change(false) }}>放弃源码草稿</button>
 </details>
}
