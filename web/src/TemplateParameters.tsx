import type { ReportDef } from './designer'
type Parameter = { name: string; data_type: string; kind?: string; default?: string; description?: string }
const types = ['int', 'long', 'short', 'float', 'double', 'bigdecimal', 'string', 'bool', 'int[]', 'string[]']
export default function TemplateParameters({ def, disabled, onChange }: {
  def: ReportDef | null; disabled: boolean; onChange: (def: ReportDef) => void
}) {
  const params = (Array.isArray(def?.params) ? def.params : []) as Parameter[]
  const update = (next: Parameter[]) => { if (def) onChange({ ...def, params: next }) }
  const change = (index: number, patch: Partial<Parameter>) => update(params.map((p, i) => i === index ? { ...p, ...patch } : p))
  return <fieldset disabled={disabled} style={{ border: 0, padding: 0 }}>
    <p>编辑参数声明；default 保存字符串元数据。实际运行参数由 agent 提供，可在运行记录查看。</p>
    {params.map((param, index) => <div key={index} style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginBottom: 8 }}>
      <input aria-label={`参数名 ${index + 1}`} value={param.name} onChange={e => change(index, { name: e.target.value })} />
      <select aria-label={`参数类型 ${index + 1}`} value={param.data_type} onChange={e => change(index, { data_type: e.target.value })}>{types.map(type => <option key={type}>{type}</option>)}</select>
      <select aria-label={`参数种类 ${index + 1}`} value={param.kind ?? 'static'} onChange={e => change(index, { kind: e.target.value })}>{['static', 'dynamic', 'attribute'].map(kind => <option key={kind}>{kind}</option>)}</select>
      <input aria-label={`参数默认元数据 ${index + 1}`} placeholder="默认值元数据" value={param.default ?? ''} onChange={e => change(index, { default: e.target.value })} />
      <input aria-label={`参数说明 ${index + 1}`} placeholder="说明" value={param.description ?? ''} onChange={e => change(index, { description: e.target.value })} />
      <button onClick={() => update(params.filter((_, i) => i !== index))}>删除参数</button>
    </div>)}
    <button onClick={() => {
      let number = 1
      while (params.some(p => p.name === `param${number}`)) number++
      update([...params, { name: `param${number}`, data_type: 'string' }])
    }}>添加参数声明</button>
  </fieldset>
}
