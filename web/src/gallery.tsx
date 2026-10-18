import { lazy, Suspense, useEffect, useMemo, useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { LifecycleStages } from '@univerjs/core'
import { mountUniver } from './report'
import type { PrintWorkbook } from './PrintPreview'
import type { Graphic } from './graphics'
import ChartView from './graphics/ChartView'
import { artifact, type GalleryCatalog, type GalleryReport } from './gallery-data'
import { fitColumns } from './fit-columns'
import './gallery.css'

const PrintPreview = lazy(() => import('./PrintPreview'))
type Tab = 'result' | 'graphics' | 'definition' | 'sources' | 'checks'
const tabs: [Tab, string][] = [
  ['result', '报表结果'],
  ['graphics', '图形'],
  ['definition', '模板定义'],
  ['sources', '数据来源'],
  ['checks', '运行校验'],
]

function Result({ workbook }: { workbook: PrintWorkbook }) {
  const container = useRef<HTMLDivElement>(null)
  const [error, setError] = useState('')
  const [ready, setReady] = useState(false)
  // 画布按模板列宽画，数字放不下就是一片 `####`（Excel 同款行为）。只读展示按
  // 「看得清」加宽——快照本身不动，打印页数与 PDF 不受影响。
  const display = useMemo(() => fitColumns(workbook), [workbook])
  useEffect(() => {
    if (!container.current) return
    setReady(false)
    const api = mountUniver(container.current, true)
    try { api.createWorkbook(display as Parameters<typeof api.createWorkbook>[0])?.setEditable(false) }
    catch (error) { setError(String(error)) }
    if (api.getCurrentLifecycleStage() >= LifecycleStages.Steady) setReady(true)
    const lifecycle = api.addEvent(api.Event.LifeCycleChanged, ({ stage }) => {
      if (stage === LifecycleStages.Steady) setReady(true)
    })
    return () => { lifecycle.dispose(); api.dispose() }
  }, [display])
  return <>{error && <p role="alert">{error}</p>}<div className="gallery-sheet" ref={container} aria-label="报表结果画布" aria-busy={!ready} /></>
}

function graphicTitle(content: Graphic['content']): string {
  if (content.kind === 'chart') return content.title || content.type
  if (content.kind === 'image' || content.kind === 'cell_background') return '图片'
  return 'ECharts'
}

/**
 * 图形页：把运行结果里的 `rpxGraphics` 逐个画出来。
 *
 * ⚠ 为什么不在「报表结果」的 Univer 画布上就地叠图：Univer 自己管滚动与缩放，
 * 叠加层要跟着它的视口变换走，属于另一套坐标系；打印预览之所以能叠，是因为
 * 它自建了分页 HTML 表格。这里按卡片列，位置与尺寸写在卡片头上，不做假叠加。
 */
function Graphics({ items }: { items: { graphic: Graphic; page: number }[] }) {
  return <div className="gallery-graphics">
    {items.map(({ graphic, page }, i) => <figure className="gallery-graphic" key={i}>
      <figcaption>
        <strong>{graphicTitle(graphic.content)}</strong>
        <span>第 {page + 1} 页 · 格 ({graphic.row + 1}, {graphic.col + 1}) · {graphic.width} × {graphic.height}</span>
      </figcaption>
      <ChartView content={graphic.content} width={graphic.width} height={graphic.height} />
    </figure>)}
  </div>
}

function Detail({ report }: { report: GalleryReport }) {
  const [tab, setTab] = useState<Tab>('result')
  const [chosen, setChosen] = useState(false)
  const [workbook, setWorkbook] = useState<PrintWorkbook | null>(null)
  const [definition, setDefinition] = useState('')
  const [error, setError] = useState('')
  const [print, setPrint] = useState(false)
  const graphics = useMemo(() => {
    if (!workbook) return []
    return workbook.sheetOrder.flatMap((id, page) =>
      (workbook.sheets[id]?.rpxGraphics ?? []).map(graphic => ({ graphic, page })))
  }, [workbook])
  // 含**图表**的报表直接开在「图形」页 —— 否则用户先看到一片空网格，误以为生成失败。
  // 只有图片（image）的例子不开：图片是版面的一部分（条码、印章），
  // 拆成卡片反而看不出排版，且条件出图的透明占位会被当成「20 张图」列出来。
  const hasChart = graphics.some(({ graphic }) =>
    ['chart', 'echarts', 'echarts_js'].includes(graphic.content.kind))
  useEffect(() => { if (!chosen && hasChart) setTab('graphics') }, [chosen, hasChart])
  // 含**图片**的报表直接开打印预览：Univer 画布只画单元格，**不渲染 rpxGraphics**
  // （只有 PrintPreview 画），所以图片类例子在「报表结果」里是一片空白 —— 看着像坏了。
  // 这里只在首次载入时开一次，用户关掉后不会重开。
  const hasImage = graphics.some(({ graphic }) =>
    graphic.content.kind === 'image' || graphic.content.kind === 'cell_background')
  useEffect(() => { if (workbook && hasImage) setPrint(true) }, [workbook, hasImage])
  useEffect(() => {
    let current = true
    if (report.status !== 'succeeded') return
    void Promise.all([
      artifact(`${report.base}/workbook.json`).then(r => r.json() as Promise<PrintWorkbook>),
      artifact(`${report.base}/template.json`).then(r => r.text()),
    ]).then(([workbook, definition]) => { if (current) { setWorkbook(workbook); setDefinition(definition) } })
      .catch(error => { if (current) setError(String(error)) })
    return () => { current = false }
  }, [report])
  return <main className="gallery-detail">
    <div className="gallery-heading">
      <div><span className="gallery-eyebrow">{report.section}</span><h1>{report.title}</h1><p>{report.description}</p></div>
      <div className="gallery-actions">
        <a className="gallery-primary" href={`/?gallery=${encodeURIComponent(report.base)}`}>在设计器中打开 ↗</a>
        <button disabled={!workbook} onClick={() => setPrint(true)}>打印预览</button>
      </div>
    </div>
    <div className="gallery-context">
      <div>{report.features.map(feature => <span className="gallery-tag" key={feature}>{feature}</span>)}</div>
      {report.run && <span>模板 {report.run.template_rows} × {report.run.template_cols} → 结果 {report.run.rows} × {report.run.cols} · {report.run.pages} 页</span>}
    </div>
    <div className="gallery-tabs" role="tablist" aria-label="报表视图">
      {tabs.filter(([id]) => id !== 'graphics' || graphics.length > 0).map(([id, title]) => <button role="tab" key={id} aria-selected={tab === id} onClick={() => { setChosen(true); setTab(id) }}>{title}</button>)}
      {report.status === 'succeeded' && <div className="gallery-downloads"><a href={`${report.base}/report.xlsx`} download>下载 Excel</a><a href={`${report.base}/report.csv`} download>CSV</a><a href={`${report.base}/template.json`} download>JSON 定义</a></div>}
    </div>
    <div className="gallery-body" role="tabpanel" aria-label={tabs.find(([id]) => id === tab)?.[1]}>
      {(report.error || error) ? <div className="gallery-error" role="alert">{report.error || error}</div> : <>
        {tab === 'result' && (workbook ? <Result workbook={workbook} /> : <p className="gallery-loading" role="status">正在读取运行结果…</p>)}
        {tab === 'graphics' && <Graphics items={graphics} />}
        {tab === 'definition' && <div className="gallery-document"><p>这份 JSON 定义包含数据集、公式、扩展方向和主格关系。可在设计器中修改并下载保存。</p><pre aria-label="JSON 报表定义">{definition}</pre></div>}
        {tab === 'sources' && <div className="gallery-document"><h2>同一报表的独立数据来源</h2><p>数据集通过名称绑定公式，通过业务键关联。文件和数据库在执行时读取，当前页面展示已经保存的结果。</p>
          <table><thead><tr><th>数据集</th><th>类型</th><th>文件 / 连接名</th><th>工作表</th><th>附件</th></tr></thead><tbody>{report.run?.inputs.map(input => <tr key={input.name}><td><code>{input.name}</code></td><td>{input.kind}</td><td>{input.file ?? input.dsn}</td><td>{input.sheet ?? '—'}</td><td>{input.file ? <a href={`${report.base}/${encodeURIComponent(input.file)}`} download>下载源文件</a> : '由执行端连接'}</td></tr>)}</tbody></table>
          <h2>Agent 执行</h2><p>在项目目录运行以下命令，重建示例数据库、计算全部模板并保存新一轮结果；刷新此页查看。</p><pre>bash scripts/build_r_model_gallery.sh</pre><p>数据库 URL 与凭证由执行端的独立配置提供。示例采用两个 SQLite 数据库。</p></div>}
        {tab === 'checks' && <div className="gallery-document"><h2>{report.run?.checks.length} 项独立预期全部通过</h2><p>逐项对照源数据计算的预期值，覆盖合计、边界、排名和跨来源公式。</p><table><thead><tr><th>检查</th><th>模板原格（行, 列）</th><th>实际值</th><th>状态</th></tr></thead><tbody>{report.run?.checks.map(check => <tr key={check.label}><td>{check.label}</td><td>{check.origin.join(', ')}</td><td><code>{JSON.stringify(check.actual)}</code></td><td className="gallery-passed">通过</td></tr>)}</tbody></table><p className="gallery-run-id">运行 ID：{report.run_id}</p><a href={`${report.base}/result.json`} download>下载带模板出处的计算结果</a> · <a href={`${report.base}/run.json`} download>下载运行记录</a></div>}
      </>}
    </div>
    <footer className="gallery-status"><span className={report.status === 'succeeded' ? 'gallery-passed' : 'gallery-error'}>{report.status === 'succeeded' ? '● 计算及校验通过' : '● 生成失败'}</span><span>结果由 Rust 报表引擎生成 · Univer 只读展示</span></footer>
    {print && workbook && <Suspense fallback={<p role="status">正在装载打印预览…</p>}><PrintPreview workbook={workbook} onClose={() => setPrint(false)} /></Suspense>}
  </main>
}

function Gallery() {
  const [catalog, setCatalog] = useState<GalleryCatalog | null>(null)
  const [selected, setSelected] = useState(location.hash.slice(1))
  const [error, setError] = useState('')
  useEffect(() => {
    void artifact('/report-gallery/catalog.json').then(r => r.json() as Promise<GalleryCatalog>).then(catalog => {
      if (catalog.version !== 1 || !Array.isArray(catalog.reports)) throw new Error('示例索引格式错误')
      setCatalog(catalog)
    }).catch(error => setError(`示例尚未生成或读取失败：${String(error)}`))
    const change = () => setSelected(location.hash.slice(1))
    window.addEventListener('hashchange', change)
    return () => window.removeEventListener('hashchange', change)
  }, [])
  const active = catalog?.reports.find(report => report.id === selected) ?? catalog?.reports[0]
  return <div className="gallery-app">
    <header className="gallery-top"><a href="/gallery.html" className="gallery-brand"><b>RPX</b><span>报表实例库</span></a><span>报表模型 / 多源数据 / 自由格间运算</span><a href="/">打开报表设计器 ↗</a></header>
    <div className="gallery-layout"><aside className="gallery-sidebar"><div className="gallery-sidebar-title"><strong>模型与实例</strong><span>{catalog?.reports.length ?? '—'} 份</span></div>
      {catalog && [...new Set(catalog.reports.map(report => report.section))].map(section => <nav aria-label={section} key={section}><h2>{section}</h2>{catalog.reports.filter(report => report.section === section).map(report => <a href={`#${report.id}`} key={report.id} aria-current={active?.id === report.id ? 'page' : undefined}><span>{report.title}</span><small>{report.status === 'succeeded' ? '已生成' : '失败'}</small></a>)}</nav>)}
      <div className="gallery-sidebar-note">Agent 执行与保存结果<br />在这里观察、校验和修正模板</div>
    </aside>{active ? <Detail key={active.id} report={active} /> : <main className="gallery-empty"><h1>报表实例库</h1><p role={error ? 'alert' : 'status'}>{error || '正在读取示例目录…'}</p><pre>bash scripts/build_r_model_gallery.sh</pre></main>}</div>
  </div>
}

createRoot(document.getElementById('app')!).render(<Gallery />)
