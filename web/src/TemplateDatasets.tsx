import { useState } from 'react'
import type { ReportDef } from './designer'
import { applyDatasetAdd, applyDatasetRemove, applyDatasetUpdate, datasetsOf, type DatasetInput } from './datasets'
import './template.css'

export default function TemplateDatasets({ def, files, disabled, onChange, onFiles, onError, onDirtyChange }: {
  def: ReportDef | null
  files: File[]
  disabled: boolean
  onChange: (def: ReportDef) => void
  onFiles: (files: File[]) => void
  onError: (message: string) => void
  onDirtyChange: (dirty: boolean) => void
}) {
  const fresh = (): DatasetInput => ({ name: 'ds1', file: { file_name: '', encoding: 'UTF-8' } })
  const [draft, setDraft] = useState<DatasetInput>(fresh)
  const [editing, setEditing] = useState<number | null>(null)
  const [dirty, setDirty] = useState(false)
  const excel = /\.(xlsx|xls|xlsm|xlsb|ods)$/i.test(draft.file?.file_name ?? '')
  const changeDraft = (update: DatasetInput | ((draft: DatasetInput) => DatasetInput)) => {
    setDraft(update)
    setDirty(true)
    onDirtyChange(true)
  }
  const reset = () => { setEditing(null); setDraft(fresh()); setDirty(false); onDirtyChange(false) }
  const commit = () => {
    if (!def) return
    if (!draft.name?.trim()) { onError('请填写数据集名称，例如 ds1'); return }
    const duplicate = datasetsOf(def).some((d, i) => i !== editing && d.name === draft.name)
    if (duplicate) { onError(`数据集 ${draft.name} 已存在，请修改名称或编辑已有数据集`); return }
    const result = editing === null ? applyDatasetAdd(def, draft) : applyDatasetUpdate(def, editing, draft)
    if (!result.def) { onError(result.error ?? '数据集定义不合法'); return }
    onChange(result.def)
    reset()
  }
  // 外框（section + h3）已由属性面板的分组头承担，这里只留字段体；
  // 保留 .template-pane 是为了复用其表单样式，padding/border 用内联收掉。
  return <div className="template-pane" aria-label="数据源与数据集" style={{ padding: 0, borderBottom: 'none', background: 'transparent' }}>
    <fieldset disabled={disabled || !def}>
      <label>上传 Excel / CSV / TSV<input aria-label="上传数据文件" type="file" multiple accept=".xlsx,.xls,.xlsm,.xlsb,.ods,.csv,.tsv,.txt" onChange={e => {
        const selected = Array.from(e.target.files ?? [])
        onFiles(selected)
        if (selected.length === 1 && !datasetsOf(def!).some(d => d.file?.file_name === selected[0].name)) changeDraft(d => ({ name: d.name, file: { file_name: selected[0].name, encoding: 'UTF-8', delimiter: /\.csv$/i.test(selected[0].name) ? ',' : '\t' } }))
        e.target.value = ''
      }}/></label>
      <p>{files.length ? `已上传：${files.map(f => f.name).join('、')}` : '运行文件数据集前，请上传模板引用的数据文件。'}</p>
      <ul className="template-datasets">{(def ? datasetsOf(def) : []).map((dataset, index) => <li key={index}>
        <strong>{dataset.name}</strong> · {dataset.file ? `文件：${dataset.file.file_name}` : `SQL：${dataset.sql?.dsn}`}
        {dataset.file && <p>{files.some(f => f.name === dataset.file?.file_name) ? '数据文件已就绪' : '尚未上传对应数据文件'}</p>}
        <div className="template-actions"><button disabled={dirty} aria-label={`编辑数据集 ${dataset.name}`} onClick={() => {
          setEditing(index)
          setDraft(dataset.file ? { name: dataset.name, file: { ...dataset.file } } : { name: dataset.name, sql: { ...dataset.sql! } })
        }}>编辑</button><button disabled={dirty} aria-label={`删除数据集 ${dataset.name}`} onClick={() => {
          if (!def) return
          const result = applyDatasetRemove(def, index)
          if (result.def) onChange(result.def)
          reset()
        }}>删除</button></div>
      </li>)}</ul>
      <label>数据集名称<input aria-label="数据集名称" value={draft.name ?? ''} onChange={e => changeDraft(d => ({ ...d, name: e.target.value }))}/></label>
      <label>数据集类型<select aria-label="数据集类型" value={draft.file ? 'file' : 'sql'} onChange={e => changeDraft(d => e.target.value === 'file' ? { name: d.name, file: { file_name: '', encoding: 'UTF-8' } } : { name: d.name, sql: { dsn: '', sql: '' } })}>
        <option value="file">文件数据集</option><option value="sql">SQL 数据集</option>
      </select></label>
      {draft.file ? <>
        <label>数据文件名<input aria-label="数据文件名" list="template-data-files" value={draft.file.file_name} onChange={e => changeDraft(d => ({ ...d, file: { ...d.file!, file_name: e.target.value } }))}/></label>
        <datalist id="template-data-files">{files.map(f => <option key={f.name} value={f.name}/>)}</datalist>
        {excel ? <label>Excel 工作表<input aria-label="Excel 工作表" placeholder="留空使用第一张工作表" value={draft.file.sheet_name ?? ''} onChange={e => changeDraft(d => ({ ...d, file: { ...d.file!, sheet_name: e.target.value || undefined } }))}/></label> : <>
          <label>文件编码<select aria-label="数据文件编码" value={draft.file.encoding ?? 'GBK'} onChange={e => changeDraft(d => ({ ...d, file: { ...d.file!, encoding: e.target.value } }))}><option value="UTF-8">UTF-8</option><option value="GBK">GBK</option></select></label>
          <label>CSV 分隔符<select aria-label="CSV 分隔符" value={draft.file.delimiter ?? '\t'} onChange={e => changeDraft(d => ({ ...d, file: { ...d.file!, delimiter: e.target.value } }))}><option value={'\t'}>TAB（制表符）</option><option value=",">逗号</option><option value=";">分号</option></select></label>
        </>}
        <p>Excel 读取指定工作表的首行列名和缓存值；CSV 按编码与分隔符解析。可以添加多个不同来源的数据集。</p>
      </> : <>
        <label>数据源名称<input aria-label="数据源名称" value={draft.sql?.dsn ?? ''} onChange={e => changeDraft(d => ({ ...d, sql: { dsn: e.target.value, sql: d.sql?.sql ?? '' } }))}/></label>
        <label>SQL 查询<textarea aria-label="SQL 查询" rows={4} value={draft.sql?.sql ?? ''} onChange={e => changeDraft(d => ({ ...d, sql: { dsn: d.sql?.dsn ?? '', sql: e.target.value } }))}/></label>
        <p>数据源名称须对应管理员在服务端配置的数据库连接。</p>
      </>}
      <div className="template-actions"><button onClick={commit}>{editing === null ? '添加数据集' : '保存数据集'}</button>
        {(editing !== null || dirty) && <button onClick={reset}>取消编辑数据集</button>}</div>
      {dirty && <p role="status">数据集有未保存修改，请先添加、保存或取消编辑。</p>}
    </fieldset>
  </div>
}
