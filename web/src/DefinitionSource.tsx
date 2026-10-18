import { useEffect, useState } from 'react'
import type { ReportDef } from './designer'
import { parseTemplate } from './template'

/**
 * 定义 JSON 源码编辑区（底部抽屉的「定义源码」页）。
 *
 * 旧版是一个 `<details>` 折叠块，夹在两条工具条之间——位置尴尬且展开后把
 * 画布往下挤。现在作为底部抽屉的一个页，抽屉收起时**不卸载**（用 CSS 隐
 * 藏），否则草稿会丢。
 */
export default function DefinitionSource({ value, onApply, onDirty, onError }: {
 value: ReportDef | null; onApply: (value: ReportDef) => boolean; onDirty: (dirty: boolean) => void; onError: (message: string) => void
}) {
 const [text, setText] = useState('')
 const [dirty, setDirty] = useState(false)
 useEffect(() => { if (!dirty) setText(value ? JSON.stringify(value, null, 2) : '') }, [value, dirty])
 const change = (dirty: boolean) => { setDirty(dirty); onDirty(dirty) }
 return <div>
  <textarea aria-label="JSON 定义源码" value={text} disabled={!value} style={{ width: '100%', height: 140, fontFamily: 'monospace' }} onChange={e => { setText(e.target.value); change(true) }} />
  <div className="rpx-actions">
   <button className="rpx-btn primary" disabled={!dirty} onClick={() => { try { if (onApply(parseTemplate(text))) change(false) } catch (error) { onError(String(error)) } }}>应用源码草稿</button>
   <button className="rpx-btn" disabled={!dirty} onClick={() => { setText(value ? JSON.stringify(value, null, 2) : ''); change(false) }}>放弃源码草稿</button>
   <span className="rpx-hint" style={{ margin: 0, alignSelf: 'center' }}>
    {dirty ? '源码有未应用修改' : '应用后写入画布与面板；保存前不会提交服务端'}
   </span>
  </div>
 </div>
}
