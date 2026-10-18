import { lazy, Suspense, useEffect, useMemo, useRef, useState } from 'react'
import type { FUniver } from '@univerjs/presets'
import { LifecycleStages, type ICellData } from '@univerjs/core'

import {
  assertEditable,
  downloadDefJson,
  exportDefJson,
  type ReportDef,
} from './designer'
import {
  CELL_PROPS_DEFAULTS,
  applyCellPropToRange,
  applyCellPropsToRange,
  cellPropsFromDef,
  type CellPropsEdit,
} from './cellprops'
import {
  downloadBlob,
  mountUniver,
  pickReportFile,
} from './report'
import { listReports, getReport, getRuns, saveTemplate, loadWorkbook, artifactUrl, sourceUrl, LibraryError, type ReportDetail, type RunRecord } from './library-api'
import MenuBar, { type MenuGroup } from './menubar'
import SidePanes from './panes'
import DefinitionSource from './DefinitionSource'
import type { PrintWorkbook } from './PrintPreview'
const PrintPreview=lazy(()=>import('./PrintPreview'))
import {
  applyColWidths,
  applyGridResize,
  applyRowBreakPages,
  applyRowHeights,
  mmToPx,
} from './rowcol'
import TemplateCellPane from './TemplateCellPane'
import TemplateDatasets from './TemplateDatasets'
import { artifact, galleryFiles } from './gallery-data'
import { applyTemplateCell, applyTemplateValues, parseTemplate, templateCellData, templateInputValue, templateWorkbook, type CellDraft } from './template'
import {
  PRINT_SETUP_DEFAULTS,
  applyPrintSetup,
  printSetupOf,
  swapPaperOrientation,
} from './reportsetup'
import {
  PANEL_DEFAULTS,
  applyFieldToRange,
  applyPanelToRange,
  clearPanelRange,
  panelFromUniverStyle,
  resetRangeToDefaults,
  type PanelField,
  type PanelRange,
  type PanelStyle,
} from './stylepanel'

/** 状态栏里算「出错」的前缀（红色上屏）。新加出错文案必须登记进来，
 * 否则会退化成灰色提示 —— 那是**静默降级**，用户看不出失败了。 */
const ERROR_PREFIXES = ['装载失败', '入库失败', '请先', '未找到', '编辑失败', '编辑面', '运行失败', '导出失败']

/** W2-3：属性编辑的状态栏标签（边标签随之搬迁 panes.tsx，这里本地小 map）。 */
const SIDE_LABEL: Record<string, string> = { top: '上', right: '右', bottom: '下', left: '左' }
function propsEditLabel(edit: CellPropsEdit): string {
  switch (edit.kind) {
    case 'borderSide':
      return `边框·${SIDE_LABEL[edit.side] ?? edit.side}`
    case 'borderStyle':
      return '边框线型'
    case 'borderColor':
      return '边框颜色'
    case 'format':
      return '数字格式'
    case 'rotation':
      return '旋转角'
  }
}

/**
 * JSON 是模板的唯一编辑源；Univer 展示原始格，Rust 生成只读运行结果。
 * 打开、编辑、下载、入库和运行都读取当前持有的定义，不从扩展结果反推。
 * Univer 实例和事件在切换工作簿时一起释放；不启用 React StrictMode 的双挂载。
 */
export default function App() {
  const [printWorkbook, setPrintWorkbook] = useState<PrintWorkbook | null>(null)
  const [showPrint, setShowPrint] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const apiRef = useRef<FUniver | null>(null)
  /** W2-1：单元格改动事件的订阅句柄（`addEvent` 返回的 `IDisposable`）。
   * 重渲染会二次 `createWorkbook`，必须先把旧监听 dispose 掉，否则叠两份。 */
  const eventDisposableRef = useRef<{ dispose: () => void } | null>(null)
  /** W2-2：选区变化（`CommandExecuted` 过滤 `sheet.operation.set-selections`）
   * 的订阅句柄；同 `eventDisposableRef` 的 dispose 纪律。 */
  const selectionDisposableRef = useRef<{ dispose: () => void } | null>(null)
  const commandDisposableRef = useRef<{ dispose: () => void } | null>(null)
  const lifecycleDisposableRef = useRef<{ dispose: () => void } | null>(null)
  /** W2-2：样式面板显示态。`null` = 当前没有选区可读（或未渲染）⇒ 面板显示
   * 缺省并整体置灰（`.rpx` 源没有定义 JSON 可打补丁，见 `assertEditable`）。 */
  const [panel, setPanel] = useState<PanelStyle | null>(null)
  /** W2-3：当前选区的坐标快照（属性面板显示态的派生来源；`null` = 无选区）。 */
  const [selection, setSelection] = useState<PanelRange | null>(null)
  const [files, setFiles] = useState<File[]>([])
  const [attachments, setAttachments] = useState<File[]>([])
  const [view, setViewState] = useState<'design' | 'result'>('design')
  const viewRef = useRef<'design' | 'result'>('design')
  const writingRef = useRef(false)
  const [cellDirty, setCellDirty] = useState(false)
  const cellDirtyRef = useRef(false)
  const datasetDirtyRef = useRef(false)
  const requestRef = useRef(0)
  const [documentKey, setDocumentKey] = useState(0)
  const [sheetReady, setSheetReady] = useState(false)
  const setView = (next: 'design' | 'result') => { viewRef.current = next; setViewState(next) }
  const setCellPending = (dirty: boolean) => { cellDirtyRef.current = dirty; setCellDirty(dirty) }
  const runFiles = () => [...new Map([...files, ...attachments].map(f => [f.name, f])).values()]
  const jsonDirtyRef = useRef(false)
  const readyToLeaveCell = () => {
    if (jsonDirtyRef.current) { setStatus('请先应用或放弃 JSON 源码草稿'); return false }
    if (!cellDirtyRef.current && !datasetDirtyRef.current) return true
    setStatus(cellDirtyRef.current ? '请先应用或放弃当前单元格的修改，再运行或保存' : '请先添加、保存或取消当前数据集的修改，再运行或保存')
    return false
  }
  const invalidateResult = () => { requestRef.current++; setShowPrint(false) }
  const ownWrite = (write: () => void) => {
    writingRef.current = true
    try { write() } finally { writingRef.current = false }
  }
  const addFiles = (incoming: File[]) => {
    invalidateResult()
    setAttachments(current => [...new Map([...current, ...incoming].map(f => [f.name, f])).values()])
  }
  /** 「另存为」拿到的服务端 id（W1-3）；换文件即失效（id 属于旧文件）。 */
  const [saved, setSaved] = useState<ReportDetail | null>(null)
  const [reports, setReports] = useState<ReportDetail[]>([])
  const [runs, setRuns] = useState<RunRecord[]>([])
  const [selectedRun, setSelectedRun] = useState('')
  const sourceRef = useRef('')
  const [sharedRevision, setSharedRevision] = useState('')
  /** W2-1：持有的定义 JSON（**打了补丁的那份**）。只有 `.json` 报表源才有；
   * `.rpx` 与未渲染时为 null ⇒ 编辑面不可用（`assertEditable` 给文案）。 */
  const [def, setDef] = useState<ReportDef | null>(null)
  /** W2-1：`def` 的镜像 ref —— 事件回调是闭包，若直接读 `def` 状态会拿到
   * 订阅那一刻的**旧值**（改一格后 `setDef` 的新值进不来）。补丁逻辑一律
   * 读 `defRef.current`，保证永远 patch 在最新持有的 JSON 上。 */
  const defRef = useRef<ReportDef | null>(null)
  /** 同时更新 `def` 状态与 `defRef` 镜像（W2-1 专用 setter）。 */
  const setDefBoth = (next: ReportDef | null) => {
    if (next !== defRef.current) invalidateResult()
    defRef.current = next
    setDef(next)
  }
  /** W2-1：报表源形态，用于「.rpx 不能编辑」这条能力边界的提示。 */
  const [sourceKind, setSourceKind] = useState<'json' | 'rpx' | null>(null)
  const [status, setStatus] = useState(
    '新建或打开 JSON 模板；在右侧配置数据集、公式和主格，由 agent 执行引擎生成结果',
  )

  /** W2-5：报表源文件名（重渲染时 def 上传体沿用这个名字）。 */
  const [reportName, setReportName] = useState('report-definition.json')

  /** 装载设计工作簿或运行快照；仅模板设计订阅内容修改。 */
  const attachSnapshot = (snapshot: unknown, held: ReportDef | null, kind: 'json' | 'rpx', design = false, selected?: { row0: number; col0: number }) => {
    eventDisposableRef.current?.dispose()
    selectionDisposableRef.current?.dispose()
    commandDisposableRef.current?.dispose()
    lifecycleDisposableRef.current?.dispose()
    apiRef.current?.dispose()
    setDefBoth(held)
    setSourceKind(kind)
    setSheetReady(false)
    setView(design ? 'design' : 'result')
    if (!design) setPrintWorkbook(snapshot as PrintWorkbook)
    if (!containerRef.current) return
    const api = mountUniver(containerRef.current, design)
    apiRef.current = api
    const workbook = api.createWorkbook(snapshot as Parameters<typeof api.createWorkbook>[0])
    workbook?.setEditable(design && !!held)
    commandDisposableRef.current = api.addEvent(api.Event.BeforeCommandExecute, event => {
      if (!design || writingRef.current || !event.id.startsWith('sheet.mutation.')) return
      if (event.id === 'sheet.mutation.set-range-values' && defRef.current) {
        if (cellDirtyRef.current) { event.cancel = true; setStatus('请先应用或放弃右侧单元格修改，再编辑画布'); return }
        const matrix = event.params?.cellValue as Record<string, Record<string, ICellData | null>> | undefined
        const edits = Object.entries(matrix ?? {}).flatMap(([r, row]) => Object.entries(row).map(([c, cell]) => ({
          row0: Number(r), col0: Number(c), value: templateInputValue(cell), cell,
        })))
        if (edits.some(({ cell }) => cell && !['v', 'f', 'p'].some(key => key in cell))) {
          event.cancel = true
          setStatus('编辑失败：请使用右侧样式面板设置单元格格式')
          return
        }
        const result = applyTemplateValues(defRef.current, edits)
        if (!result.def) { event.cancel = true; setStatus(`编辑失败：${result.error}`); return }
        // Force report formulas to text before Univer's own formula engine sees them.
        for (const edit of edits) {
          const cell = result.def.cells?.find(c => c.row === edit.row0 + 1 && c.col === edit.col0 + 1)
          if (cell && matrix) matrix[edit.row0][edit.col0] = templateCellData(cell)
        }
        return
      }
      // Native structural changes cannot preserve report expressions and head addresses.
      event.cancel = true
      setStatus('编辑失败：请使用右侧模板面板设置样式、合并区域和行列尺寸')
    })
    eventDisposableRef.current = api.addEvent(api.Event.SheetValueChanged, event => {
      if (viewRef.current !== 'design' || writingRef.current || !defRef.current) return
      if ((event.payload as { id: string }).id !== 'sheet.mutation.set-range-values') return
      const edits: { row0: number; col0: number; value: unknown }[] = []
      for (const range of event.effectedRanges ?? []) {
        const data = range.getCellDatas()
        for (let r = 0; r < range.getHeight(); r++) for (let c = 0; c < range.getWidth(); c++) {
          const cell = data[r]?.[c]
          edits.push({ row0: range.getRow() + r, col0: range.getColumn() + c,
            value: templateInputValue(cell) })
        }
      }
      const result = applyTemplateValues(defRef.current, edits)
      if (!result.def) {
        // Restore the authoritative template after an invalid paste.
        queueMicrotask(() => { showTemplate(); setStatus(`编辑失败：${result.error}`) })
        return
      }
      setDefBoth(result.def)
      setStatus('已更新模板内容；可保存模板或运行生成报表')
      syncPanel()
    })
    selectionDisposableRef.current = api.addEvent(api.Event.CommandExecuted, event => {
      if (event.id === 'sheet.operation.set-selections') syncPanel()
    })
    const onReady = () => queueMicrotask(() => {
      if (apiRef.current !== api) return
      const sheet = workbook?.getActiveSheet()
      if (design && sheet) sheet.setActiveRange(sheet.getRange(selected?.row0 ?? 0, selected?.col0 ?? 0))
      syncPanel()
      setSheetReady(true)
    })
    if (api.getCurrentLifecycleStage() >= LifecycleStages.Steady) onReady()
    else lifecycleDisposableRef.current = api.addEvent(api.Event.LifeCycleChanged, ({ stage }) => {
      if (stage === LifecycleStages.Steady) onReady()
    })
  }

  const showTemplate = (held = defRef.current, selected?: { row0: number; col0: number }) => {
    if (!held || cellDirtyRef.current) return
    attachSnapshot(templateWorkbook(held, reportName), held, 'json', true, selected)
    setStatus('模板设计：原始公式由 agent 调用 Rust 引擎执行，主格关系在右侧设置')
  }

  const openFiles = async (selected: File[]) => {
    if (!selected?.length) return
    const report = pickReportFile(Array.from(selected))
    if (!report) { addFiles(Array.from(selected)); return }
    if (!readyToLeaveCell()) return
    try {
      const held = report.name.toLowerCase().endsWith('.json') ? parseTemplate(await report.text()) : null
      setFiles(selected)
      setAttachments([])
      setReportName(report.name)
      setCellPending(false)
      datasetDirtyRef.current = false
      setDocumentKey(key => key + 1)
      setPrintWorkbook(null)
      setShowPrint(false)
      setSaved(null)
      setRuns([])
      setSelectedRun('')
      setSharedRevision('')
      if (held) {
        attachSnapshot(templateWorkbook(held, report.name), held, 'json', true)
        setStatus(`已打开模板 ${report.name}；无需数据即可编辑，运行前上传对应数据文件`)
      } else {
        eventDisposableRef.current?.dispose()
        selectionDisposableRef.current?.dispose()
        commandDisposableRef.current?.dispose()
        lifecycleDisposableRef.current?.dispose()
        apiRef.current?.dispose()
        apiRef.current = null
        setSelection(null)
        setPanel(null)
        setDefBoth(null)
        setSourceKind('rpx')
        setView('result')
        setStatus('已选择 RPX 模板，请使用 rpx report import-rpx 显式导入为 JSON 定义')
      }
    } catch (error) { setStatus(`装载失败：${error instanceof Error ? error.message : String(error)}`) }
  }

  const draftChanged = () => JSON.stringify(defRef.current) !== sourceRef.current
  const refreshLibrary = async (id = saved?.id, replaceDraft = false) => {
    if (!readyToLeaveCell()) return
    const request = ++requestRef.current
    try {
      const listing = await listReports()
      const chosen = id ?? listing.items[0]?.id
      if (!chosen) { setStatus('模板库为空，请用 rpx library init --presets 创建预制报表'); return }
      const detail = await getReport(chosen)
      const history = await getRuns(chosen)
      const run = detail.display_run_id
      const workbook = run ? await loadWorkbook(chosen, run) as PrintWorkbook : null
      if (request !== requestRef.current) return
      setReports(listing.items)
      setSharedRevision(detail.revision)
      if (replaceDraft || !defRef.current || !draftChanged()) {
        sourceRef.current = JSON.stringify(detail.template)
        setReportName(`${detail.manifest.name}.json`)
        setDocumentKey(key => key + 1)
        attachSnapshot(templateWorkbook(detail.template, detail.manifest.name), detail.template, 'json', true)
        setSaved(detail)
      } else if (saved && detail.revision !== saved.revision) {
        setStatus('共享源码已更新；本地草稿已保留，保存时将检查修订冲突')
      }
      if (!replaceDraft && saved && draftChanged()) {
        setSaved(current => current ? { ...detail, revision: current.revision } : current)
      }
      setRuns(history.items)
      setSelectedRun(run ?? '')
      setPrintWorkbook(workbook)
      if (replaceDraft || !draftChanged()) setStatus(detail.result_state === 'older_revision' ? '定义已修改，结果对应旧修订；等待 agent 执行' : '已读取模板库与保存的运行结果')
    } catch (error) { setStatus(`读取失败：${error instanceof Error ? error.message : String(error)}`) }
  }
  const render = async () => {
    if (!saved) { setStatus('请从模板库选择报表；本地模板可通过 rpx run 执行'); return }
    if (!readyToLeaveCell()) return
    try {
      if (!selectedRun) { await refreshLibrary(); return }
      const workbook = await loadWorkbook(saved.id, selectedRun)
      attachSnapshot(workbook, defRef.current, 'json')
      setStatus(`正在查看已保存的运行 ${selectedRun}`)
    } catch (error) { setStatus(`结果读取失败：${error instanceof Error ? error.message : String(error)}`) }
  }

  const onTemplateCellApply = (row0: number, col0: number, draft: CellDraft): boolean => {
    if (!defRef.current || viewRef.current !== 'design') return false
    const result = applyTemplateCell(defRef.current, row0, col0, draft)
    if (!result.def) { setStatus(`编辑失败：${result.error}`); return false }
    setCellPending(false)
    datasetDirtyRef.current = false
    showTemplate(result.def, { row0, col0 })
    setStatus('已应用单元格定义，可保存共享定义，由 agent 运行查看扩展结果')
    return true
  }

  /** W2-1：导出（编辑面唯一出口）。 */
  const exportJson = () => {
    const cur = defRef.current
    if (!cur) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先新建或打开 JSON 模板'}`)
      return
    }
    if (!readyToLeaveCell()) return
    downloadDefJson(cur, /\.json$/i.test(reportName.trim()) ? reportName.trim() : `${reportName.trim() || 'report-definition'}.json`)
    setStatus(`已导出定义 JSON（${exportDefJson(cur).length} 字符）`)
  }

  /** W3-4：导出 Excel/CSV（`POST /export`，M6-03 服务端已就绪）——优先导出
   * **打过补丁的 def**（与「渲染」持有 def 优先同口径），否则回落磁盘源。 */
  const exportFile = async (fmt: 'csv' | 'xlsx' | 'print-xlsx') => {
    if (!saved || !selectedRun) { setStatus('请先选择一次成功运行'); return }
    const name = fmt === 'csv' ? 'report.csv' : fmt === 'xlsx' ? 'report.xlsx' : 'report-print.xlsx'
    const run = runs.find(run => run.run_id === selectedRun)
    if (!run?.artifacts[name]) { setStatus(`该次运行未生成 ${name}；请由 agent 使用 --export ${fmt} 再执行`); return }
    try {
      const response = await fetch(artifactUrl(saved.id, selectedRun, name))
      if (!response.ok) throw new Error(`HTTP ${response.status}`)
      downloadBlob(await response.blob(), name)
      setStatus(`已下载运行 ${selectedRun} 的 ${name}`)
    } catch (error) { setStatus(`下载失败：${String(error)}`) }
  }

  // -------------------------------------------------------------------------
  // W3-4 菜单栏（产品式 IDE 菜单；与顶栏按钮并存不重复）
  // -------------------------------------------------------------------------

  /** W3-5：右侧属性面板整体显隐（「视图」菜单；折叠是纯 UI，无判据含义）。 */
  const [sideVisible, setSideVisible] = useState(true)

  /** W2-5：调整网格（面板输入的值）。 */
  const onGridResize = async () => {
    const rows = Number(gridDraft.rows)
    const cols = Number(gridDraft.cols)
    await resizeGridAndRender(rows, cols)
  }

  /** W3-7：新建报表（产品「文件→新建报表 Ctrl+N」）——12×8 定义 JSON
   * 直接进编辑面（不经磁盘文件），A1 放一个字面量当锚点。 */
  const newReport = async () => {
    if (!readyToLeaveCell()) return
    const fresh: ReportDef = { version: 1, grid: { rows: 12, cols: 8 }, cells: [{ row: 1, col: 1, text: '新报表' }] }
    setFiles([])
    setAttachments([])
    setSaved(null)
    setRuns([])
    setSelectedRun('')
    setPrintWorkbook(null)
    setSharedRevision('')
    setCellPending(false)
    setDocumentKey(key => key + 1)
    setReportName('new-report.json')
    attachSnapshot(templateWorkbook(fresh, 'new-report.json'), fresh, 'json', true)
    setStatus('已新建模板；配置数据集，选中单元格编写公式和主格关系')
  }

  /** W3-7：打开报表（产品「文件→打开报表 Ctrl+O」）——触发顶栏文件选择。
   * 顶栏 input 是既有的唯一文件入口（E2E 也用它），不另造。 */
  const openReport = () => {
    const input = document.querySelector('input[type=file]') as HTMLInputElement | null
    if (input) input.click()
    else setStatus('未找到文件选择控件')
  }

  /** W3-7：编辑菜单的插入/删除（产品「插入(I)→/删除(D)→」的 Web 子集）。
   * v1 只做**追加行/列**与**删末行/列**——中间行列的插入/删除需要 cells/
   * row_props 内容平移（产品会平移），未实现前不开放（无平移 = 静默错位）。 */
  const insertRow = () => {
    const held = defRef.current
    if (held) void resizeGridAndRender(held.grid.rows + 1, held.grid.cols)
  }
  const insertCol = () => {
    const held = defRef.current
    if (held) void resizeGridAndRender(held.grid.rows, held.grid.cols + 1)
  }
  const deleteLastRow = () => {
    const held = defRef.current
    if (held) void resizeGridAndRender(held.grid.rows - 1, held.grid.cols)
  }
  const deleteLastCol = () => {
    const held = defRef.current
    if (held) void resizeGridAndRender(held.grid.rows, held.grid.cols - 1)
  }

  /** W3-7：产品快捷键（文件/报表两组里 Web 有真实出口的）。
   * Ctrl+N 新建 / Ctrl+O 打开 / Ctrl+S 保存（导出 JSON）/ F9 预览（渲染）/
   * Ctrl+B 报表属性面板 / F11 数据集面板。⛔ Ctrl+Z/Y/X/C/V/F：Univer 画布
   * 自带（`docs/spec-m7-web.md` §W3-7 的对照表），再挂一层会双触发。 */
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null
      const typing = !!t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT')
      if (e.ctrlKey && !e.shiftKey && e.key.toLowerCase() === 'n') {
        e.preventDefault()
        void newReport()
      } else if (e.ctrlKey && e.key.toLowerCase() === 'o') {
        e.preventDefault()
        openReport()
      } else if (e.ctrlKey && e.key.toLowerCase() === 's') {
        e.preventDefault()
        void save()
      } else if (e.key === 'F9') {
        e.preventDefault()
        void render()
      } else if (e.ctrlKey && e.key.toLowerCase() === 'b') {
        if (typing) return
        e.preventDefault()
        setSideVisible(true)
        setStatus('已定位右侧「报表设置」分区（报表属性 Ctrl+B）')
      } else if (e.key === 'F11') {
        if (typing) return
        e.preventDefault()
        setSideVisible(true)
        setStatus('已定位右侧「数据集」分区（数据集 F11）')
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  /** W3-4：菜单全集。禁用的判据与顶栏按钮一致（无文件/无 def 时置灰）。 */
  const menuGroups: MenuGroup[] = [
    {
      // 产品「文件(F)」对齐：新建/打开/保存/另存为/预览/导出→。
      // ⛔ 未做（单文档 SPA 或浏览器沙箱边界，spec-m7-web §W3-7 对照表）：
      // 新建报表组（多报表连续页码）、快速打开（=打开）、关闭/关闭全部、
      // 最近文件/最近连接、退出。
      label: '文件',
      items: [
        { label: '新建报表(N)\tCtrl+N', onPick: () => void newReport(), hint: '新建 12×8 模板，直接进入设计' },
        { label: '打开报表(O)\tCtrl+O', onPick: openReport, hint: '选择 .rpx / .json（可同选 CSV）' },
        { label: '打开数据集(D)', onPick: () => { setSideVisible(true); setStatus('右侧「数据集」分区：管理 datasets[] 引用') }, hint: '产品 F11 同款；Web 落在右侧面板' },
        { sep: true },
        { label: '保存(S)\tCtrl+S', onPick: () => void save(), disabled: !def, hint: '把打过补丁的定义 JSON 下载回本地' },
        { label: '另存为(A)', onPick: () => void save(), disabled: !files.length && !def, hint: '保存当前编辑中的模板到服务端' },
        { label: '查看结果(F9)', onPick: () => void render(), disabled: !files.length && !def, hint: '读取已保存结果' },
        {
          label: '导出(E)',
          onPick: () => {},
          children: [
            { label: 'Excel（.xlsx）', onPick: () => void exportFile('xlsx'), disabled: !files.length && !def, hint: '下载该次运行已生成的 Excel' },
            { label: '打印版 Excel（.xlsx）', onPick: () => void exportFile('print-xlsx'), disabled: !files.length && !def },
            { label: 'CSV（.csv）', onPick: () => void exportFile('csv'), disabled: !files.length && !def, hint: '下载该次运行已生成的 CSV' },
            { label: '定义 JSON（.json）', onPick: () => void save(), disabled: !def, hint: '导出打过补丁的定义 JSON' },
          ],
        },
      ],
    },
    {
      // 产品「编辑(E)」对齐：⛔ 撤销/重做/剪贴板/搜索 = Univer 画布自带
      // （再挂一层菜单会双触发，spec §W3-7）；⛔ 生成报表/向导新建 = 产品从
      // 数据集反向生成模板，Web 设计器是正向（先模板后绑数据）。
      label: '编辑',
      items: [
        {
          label: '插入(I)',
          onPick: () => {},
          disabled: !def || view !== 'design',
          children: [
            { label: '追加行', onPick: insertRow, hint: '网格行数 +1（内容不动）' },
            { label: '追加列', onPick: insertCol, hint: '网格列数 +1（内容不动）' },
          ],
        },
        {
          label: '删除(D)',
          onPick: () => {},
          disabled: !def || view !== 'design',
          children: [
            { label: '删末行', onPick: deleteLastRow, hint: '行数 -1；末行有内容时显式报错（不静默错位）' },
            { label: '删末列', onPick: deleteLastCol, hint: '列数 -1；末列有内容时显式报错' },
          ],
        },
      ],
    },
    {
      // 产品「报表(R)」对齐：参数/数据集/报表属性/行属性/列属性/边框设置。
      // ⛔ 未做（定义 JSON 无槽或产品独有）：统计图、条形码、常用计算列、
      // 表达式对话框、子报表、数据类型、图片、数据图层、设置密码。
      label: '报表',
      items: [
        { label: '参数(A)', onPick: () => setSideVisible(true), hint: '右侧「报表参数」分区（/meta 声明）' },
        { label: '数据集\tF11', onPick: () => { setSideVisible(true); setStatus('右侧「数据集」分区') }, hint: '产品 F11 同款' },
        { label: '报表属性(B)\tCtrl+B', onPick: () => { setSideVisible(true); setStatus('右侧「报表设置」分区（纸张/边距/分页）') }, hint: '产品 Ctrl+B 同款' },
        { label: '行属性(R)', onPick: () => setSideVisible(true), hint: '右侧「行列」分区（行高/断页）' },
        { label: '列属性(C)', onPick: () => setSideVisible(true), hint: '右侧「行列」分区（列宽）' },
        { label: '边框设置(K)', onPick: () => setSideVisible(true), hint: '右侧「属性」分区（四边边框/线型/色）' },
      ],
    },
    {
      label: '视图',
      items: [
        {
          label: sideVisible ? '隐藏右侧属性面板' : '显示右侧属性面板',
          onPick: () => setSideVisible(!sideVisible),
          hint: '画布空间紧张时收起（产品 IDE 同款停靠区）',
        },
      ],
    },
    {
      label: '帮助',
      items: [
        {
          label: '关于',
          onPick: () =>
            setStatus(
              'rpx Web 设计器 · 定义 JSON 通道（M7-W W1）+ 编辑面（W2-1~W2-5）+ 报表设置（W3-1）+ 产品菜单布局（W3-4/W3-7）；链路：/render → Univer 快照，/export → 文件字节',
            ),
        },
      ],
    },
  ]

  // -------------------------------------------------------------------------
  // W2-2 样式面板（7 字段；逐字段补丁进定义 JSON，见 `stylepanel.ts` 文件头）
  // -------------------------------------------------------------------------

  /** W2-2：当前选区（活动工作簿 → 活动表 → 选区 → 活动范围）。
   *
   * 面板显示态取选区**起点**那一格的样式（多格混合态显示留待后续），
   * 而改值作用于**整个选区**——与 Univer 画布的行为一致（画布会把选区每格都改）。 */
  const currentSelection = () => {
    const sheet = apiRef.current?.getActiveWorkbook()?.getActiveSheet() ?? null
    const range = sheet?.getSelection()?.getActiveRange() ?? null
    return sheet && range ? { sheet, range } : null
  }

  /** W2-2：把面板显示态刷新为当前选区的样式（选区没了 ⇒ 面板回 null 置灰）。
   * W2-3：同时记录选区坐标快照（属性面板的显示态由它派生）。 */
  const syncPanel = () => {
    const cur = currentSelection()
    if (!cur) {
      setPanel(null)
      setSelection(null)
      return
    }
    const cell = cur.sheet.getRange(cur.range.getRow(), cur.range.getColumn())
    setPanel(panelFromUniverStyle(cell.getCellStyleData()))
    setSelection({
      row0: cur.range.getRow(),
      col0: cur.range.getColumn(),
      height: cur.range.getHeight(),
      width: cur.range.getWidth(),
    })
  }

  /** W2-2：面板操作的公共前置（选区 + 定义 JSON 都就位）。不满足 ⇒ 上屏原因并返回 null。 */
  const panelContext = () => {
    if (viewRef.current !== 'design') return null
    const cur = currentSelection()
    if (!cur) {
      setStatus('编辑失败：请先在表格里选中单元格，再改样式')
      return null
    }
    const held = defRef.current
    if (!held) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先新建或打开 JSON 模板'}`)
      return null
    }
    return {
      cur,
      held,
      range: {
        row0: cur.range.getRow(),
        col0: cur.range.getColumn(),
        height: cur.range.getHeight(),
        width: cur.range.getWidth(),
      },
    }
  }

  /** W2-2：面板字段改动 —— 先落 Univer（画布立即反馈），再补定义 JSON（只动一个键）。
   *
   * ⚠ 顺序刻意：Univer 落地失败就**不**补 JSON —— 否则「JSON 说改了、画布没改」
   * 是单边不一致；JSON 补丁是纯函数，不引入新失败面。 */
  const onPanelChange = (field: PanelField, raw: string | number | boolean) => {
    const ctx = panelContext()
    if (!ctx) return
    const r = applyPanelToRange(ctx.held, ctx.range, field, raw)
    if (r.error || r.normalized === undefined) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    try {
      applyFieldToRange(ctx.cur.range, field, r.normalized)
    } catch (e) {
      setStatus(`编辑失败：Univer 侧落地失败（${e instanceof Error ? e.message : String(e)}）`)
      return
    }
    if (r.def) setDefBoth(r.def)
    setPanel((p) => (p ? ({ ...p, [field]: r.normalized } as PanelStyle) : p))
    const warn = r.warnings.length > 0 ? `（${r.warnings.join('；')}）` : ''
    setStatus(`已更新样式 ${field}${warn}；点「导出 JSON」拿回文件`)
  }

  /** W2-2：「恢复缺省」—— 7 个样式字段全删（Univer 侧同步落回缺省）。 */
  const onPanelReset = () => {
    const ctx = panelContext()
    if (!ctx) return
    const r = clearPanelRange(ctx.held, ctx.range)
    if (r.error) {
      setStatus(`编辑失败：${r.error}`)
      return
    }
    try {
      resetRangeToDefaults(ctx.cur.range)
    } catch (e) {
      setStatus(`编辑失败：Univer 侧落地失败（${e instanceof Error ? e.message : String(e)}）`)
      return
    }
    if (r.def) setDefBoth(r.def)
    setPanel({ ...PANEL_DEFAULTS })
    const warn = r.warnings.length > 0 ? `（${r.warnings.join('；')}）` : ''
    setStatus(`已恢复缺省样式${warn}；点「导出 JSON」拿回文件`)
  }

  // -------------------------------------------------------------------------
  // W2-3 属性面板（边框 / 数字格式 / 旋转）
  // -------------------------------------------------------------------------

  /** W2-3 面板显示态：**派生**自（定义 JSON × 选区起点格）。
   *
   * 与 W2-2 相反，这三项不在服务端快照里 ⇒ 画布读不回来 ⇒ 显示态的唯一来源是
   * 持有的定义 JSON（`cellprops.ts` 文件头）。派生而非独立 state：任何一次补丁
   * 都会让 `def` 变化、自动重算，不会出现「面板值比 JSON 旧一格」的漂移。 */
  const propsPanel = useMemo(
    () => (def && selection ? cellPropsFromDef(def, selection) : CELL_PROPS_DEFAULTS),
    [def, selection],
  )

  /** W2-3：属性编辑 —— 与 W2-2 同规：先落 Univer（视觉反馈），再补定义 JSON（只动一个键）。 */
  const onPropsChange = (edit: CellPropsEdit) => {
    const ctx = panelContext()
    if (!ctx) return
    const r = applyCellPropsToRange(ctx.held, ctx.range, edit)
    if (r.error || !r.normalized) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    try {
      applyCellPropToRange(ctx.cur.range, r.normalized, propsPanel)
    } catch (e) {
      setStatus(`编辑失败：Univer 侧落地失败（${e instanceof Error ? e.message : String(e)}）`)
      return
    }
    if (r.def) setDefBoth(r.def)
    const warn = r.warnings.length > 0 ? `（${r.warnings.join('；')}）` : ''
    setStatus(`已更新${propsEditLabel(edit)}${warn}；点「导出 JSON」拿回文件`)
  }

  // -------------------------------------------------------------------------
  // W2-5 行列面板（行高/列宽/断页）+ 网格尺寸 + 数据集绑定
  // -------------------------------------------------------------------------

  /** W2-5：选区覆盖的**逻辑行号**清单（Univer 0 起 → JSON 1 起，`designer.ts` §4.1）。
   * 行高/列宽/断页都是**整行属性**，作用于选区覆盖的每一行 —— 与画布上
   * 「选中多行改行高」的直觉一致。 */
  const selectedRows = (): number[] => {
    if (!selection) return []
    return Array.from({ length: selection.height }, (_, i) => selection.row0 + i + 1)
  }
  /** 选区覆盖的逻辑列号清单。 */
  const selectedCols = (): number[] => {
    if (!selection) return []
    return Array.from({ length: selection.width }, (_, i) => selection.col0 + i + 1)
  }

  /** W2-5：改行高（mm）—— 补 `row_props[].height` + Univer `setRowHeight`（px）。
   * `null` = 恢复缺省（删声明；画布上先落回快照缺省值，重渲染彻底归位）。 */
  const onRowHeightChange = (mm: number | null) => {
    const held = defRef.current
    if (!held) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先新建或打开 JSON 模板'}`)
      return
    }
    const rows = selectedRows()
    if (rows.length === 0) {
      setStatus('编辑失败：请先在表格里选中单元格，再改行高')
      return
    }
    const r = applyRowHeights(held, rows, mm)
    if (r.error || !r.def) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    try {
      const sheet = apiRef.current?.getActiveWorkbook()?.getActiveSheet()
      const px = mm === null ? undefined : mmToPx(mm)
      for (const row of rows) {
        // Univer 吃 0 起行号；恢复缺省时按契约缺省换算落回（快照几何的 mm→px 同式）。
        sheet?.setRowHeight(row - 1, px ?? mmToPx(8))
      }
    } catch (e) {
      setStatus(`编辑失败：Univer 侧落地失败（${e instanceof Error ? e.message : String(e)}）`)
      return
    }
    setDefBoth(r.def)
    const warn = r.warnings.length > 0 ? `（${r.warnings.join('；')}）` : ''
    setStatus(`已更新行高${warn}；点「导出 JSON」拿回文件`)
  }

  /** W2-5：改列宽（mm），口径同 [`onRowHeightChange]。 */
  const onColWidthChange = (mm: number | null) => {
    const held = defRef.current
    if (!held) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先新建或打开 JSON 模板'}`)
      return
    }
    const cols = selectedCols()
    if (cols.length === 0) {
      setStatus('编辑失败：请先在表格里选中单元格，再改列宽')
      return
    }
    const r = applyColWidths(held, cols, mm)
    if (r.error || !r.def) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    try {
      const sheet = apiRef.current?.getActiveWorkbook()?.getActiveSheet()
      const px = mm === null ? undefined : mmToPx(mm)
      for (const col of cols) {
        sheet?.setColumnWidth(col - 1, px ?? mmToPx(25))
      }
    } catch (e) {
      setStatus(`编辑失败：Univer 侧落地失败（${e instanceof Error ? e.message : String(e)}）`)
      return
    }
    setDefBoth(r.def)
    const warn = r.warnings.length > 0 ? `（${r.warnings.join('；')}）` : ''
    setStatus(`已更新列宽${warn}；点「导出 JSON」拿回文件`)
  }

  /** W2-5：改断页 —— 纯 JSON 补丁（Univer 画布无断页概念），重渲染生效。 */
  const onBreakPageChange = (on: boolean | null) => {
    const held = defRef.current
    if (!held) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先新建或打开 JSON 模板'}`)
      return
    }
    const rows = selectedRows()
    if (rows.length === 0) {
      setStatus('编辑失败：请先在表格里选中单元格，再改断页')
      return
    }
    const r = applyRowBreakPages(held, rows, on)
    if (r.error || !r.def) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    setDefBoth(r.def)
    const warn = r.warnings.length > 0 ? `（${r.warnings.join('；')}）` : ''
    setStatus(`已更新断页${warn}；点「导出 JSON」拿回文件`)
  }

  /** W2-5：网格尺寸草稿（数字输入 + 「调整网格」按钮确认——不即时应用，
   * 避免「先输 1 再输 12」的中途值把网格真缩成 1 行）。 */
  const [gridDraft, setGridDraft] = useState({ rows: 0, cols: 0 })
  // def 的网格变化（含重渲染装载）时把草稿同步成当前值。
  useEffect(() => {
    if (def) setGridDraft({ rows: def.grid.rows, cols: def.grid.cols })
  }, [def?.grid.rows, def?.grid.cols])

  /** W3-7：网格调整 + 重渲染的公共通路（面板「调整网格」与编辑菜单
   * 「插入→/删除→」共用；越界校验 + 失败上屏同规）。 */
  const resizeGridAndRender = async (rows: number, cols: number) => {
    if (viewRef.current !== 'design' || !readyToLeaveCell()) return
    const held = defRef.current
    if (!held) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先新建或打开 JSON 模板'}`)
      return
    }
    const r = applyGridResize(held, rows, cols)
    if (r.error || !r.def) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    showTemplate(r.def)
    setStatus('已调整模板网格')
  }

  // -------------------------------------------------------------------------
  // W3-1 报表设置（print_setup；PDF 课程 1.4/9.4 的「报表设置」产品形态里
  // 定义 JSON 有槽的那部分。读 JSON 派生、逐键补丁、**重渲染生效**）
  // -------------------------------------------------------------------------

  /** W3-1：面板显示态（派生自持有 def；未声明字段显示契约缺省）。 */
  const setupPanel = useMemo(
    () => (def ? printSetupOf(def) : { ...PRINT_SETUP_DEFAULTS }),
    [def],
  )

  /** W3-1：单字段改动 —— 纯 JSON 补丁（Univer 无落地腿：纸张/分页只在服务端
   * 出快照时兑现，与 W2-5 断页同模式；顶部「渲染」已持有 def 优先）。 */
  const onSetupChange = (field: keyof typeof PRINT_SETUP_DEFAULTS, value: number | boolean) => {
    const held = defRef.current
    if (!held) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先新建或打开 JSON 模板'}`)
      return
    }
    const r = applyPrintSetup(held, { [field]: value })
    if (r.error || !r.def) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    setDefBoth(r.def)
    setStatus(`已更新报表设置 ${field}；保存后由 agent 执行生效，或「导出 JSON」拿回文件`)
  }

  /** W3-1：横竖互换（纸张宽高对调）。 */
  const onSwapPaper = () => {
    const held = defRef.current
    if (!held) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先新建或打开 JSON 模板'}`)
      return
    }
    const r = swapPaperOrientation(held)
    if (r.error || !r.def) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    setDefBoth(r.def)
    setStatus('已互换纸张方向；保存后由 agent 执行生效')
  }

  /** 「另存为」（W1-3 + W1-2）：入库拿 id → 按 `/meta` 刷新参数表单。 */
  const save = async () => {
    if (!readyToLeaveCell()) return
    if (!saved || !defRef.current) { setStatus('本地模板可下载 JSON；请用 CLI 创建或导入报表包后共同编辑'); return }
    const held = defRef.current
    try {
      const detail = await saveTemplate(saved.id, saved.revision, held)
      sourceRef.current = JSON.stringify(held)
      setSaved(detail)
      setSharedRevision(detail.revision)
      setReports(current => current.map(r => r.id === detail.id ? detail : r))
      setStatus(detail.result_state === 'older_revision' ? '定义已保存；结果对应旧修订，等待 agent 执行' : '定义已保存')
    } catch (error) {
      setStatus(error instanceof LibraryError && error.status === 412
        ? '修订冲突：共享源码已改变，本地草稿已保留。可下载草稿并与最新定义比较'
        : `保存失败：${error instanceof Error ? error.message : String(error)}；草稿已保留`)
    }
  }

  useEffect(() => {
    // 示例库入口（?gallery=）已显式指定要打开的报表：跳过模板库首项的自动装载。
    // 否则两条装载链路并行互相覆盖——不仅模板被顶掉，库里附带的 workbook 还会
    // 把示例的运行结果挤掉，「运行结果」看到的是另一份报表。
    if (new URLSearchParams(location.search).get('gallery')) return
    void refreshLibrary(undefined, true)
  }, [])

  useEffect(() => {
    const base = new URLSearchParams(location.search).get('gallery')
    if (!base) return
    let current = true
    void galleryFiles(base).then(async files => {
      if (!current) return
      await openFiles(files)
      if (!current) return
      // 同一运行目录里的 workbook.json 一并装载，「运行结果」按钮立即可用
      // （否则 printWorkbook 为 null，按钮永远是禁用的）。读取失败时明确上屏，
      // 不静默留一个点不动的按钮。
      try {
        setPrintWorkbook(await (await artifact(`${base}/workbook.json`)).json() as PrintWorkbook)
      } catch (error) { setStatus(`装载失败：读取运行结果 workbook.json 失败（${String(error)}）`) }
    }).catch(error => { if (current) setStatus(`装载失败：${String(error)}`) })
    return () => { current = false }
    // 此入口只在首次装载时读取固定运行版本。
  }, [])

  // 卸载时收掉单元格监听 + 选区监听 + Univer 实例（canvas + 监听）。
  useEffect(
    () => () => {
      eventDisposableRef.current?.dispose()
      selectionDisposableRef.current?.dispose()
      commandDisposableRef.current?.dispose()
      lifecycleDisposableRef.current?.dispose()
      apiRef.current?.dispose()
    },
    [],
  )

  /** W2-2：面板可用性 = 持有定义 JSON（`.rpx` 源没有可打补丁的 JSON）。 */
  const panelDisabled = !def || view !== 'design' || !sheetReady
  /** W2-2：面板显示值（没有选区时显示缺省，与置灰状态一致）。 */
  const pv: PanelStyle = panel ?? PANEL_DEFAULTS

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
      <style>{`
        :root {
          --rpx-border: #d8dee4;
          --rpx-bg-muted: #f6f8fa;
          --rpx-fg: #1f2328;
          --rpx-fg-muted: #57606a;
          --rpx-danger: #cf222e;
          --rpx-accent: #0969da;
        }
      `}</style>
      <header
        style={{
          padding: '8px 14px',
          borderBottom: '1px solid var(--rpx-border)',
          fontSize: 14,
          fontFamily: 'system-ui, sans-serif',
          color: 'var(--rpx-fg)',
          background: 'var(--rpx-bg-muted)',
          display: 'flex',
          alignItems: 'center',
          gap: 12,
        }}
      >
        <span>
          rpx 报表设计器
        </span>
        <a href="/gallery.html" style={{ fontSize: 13 }}>报表实例库</a>
        <input
          type="file"
          multiple
          accept=".json"
          onChange={(e) => { const selected = Array.from(e.target.files ?? []); e.target.value = ''; void openFiles(selected) }}
          style={{ fontSize: 13 }}
        />
        <button
          onClick={render}
          style={{
            fontSize: 13,
            padding: '4px 14px',
            border: '1px solid var(--rpx-border)',
            borderRadius: 6,
            background: 'var(--rpx-accent)',
            color: '#fff',
            cursor: 'pointer',
          }}
        >
          查看保存的结果
        </button>
        {/* 「另存为」（W1-3）：入库拿 id，并按 `/meta` 刷新参数表单（W1-2）。 */}
        <button
          onClick={save}
          style={{
            fontSize: 13,
            padding: '4px 14px',
            border: '1px solid var(--rpx-border)',
            borderRadius: 6,
            background: '#fff',
            color: 'var(--rpx-fg)',
            cursor: 'pointer',
          }}
        >
          保存共享定义
        </button>
        <button
          onClick={exportJson}
          disabled={!def}
          title="把编辑后的定义 JSON 下载回本地（仅 .json 报表源可用）"
          style={{
            fontSize: 13,
            padding: '4px 14px',
            border: '1px solid var(--rpx-border)',
            borderRadius: 6,
            background: def ? '#fff' : 'var(--rpx-bg-muted)',
            color: def ? 'var(--rpx-fg)' : 'var(--rpx-fg-muted)',
            cursor: def ? 'pointer' : 'not-allowed',
          }}
        >
          保存模板 JSON
        </button>
        {saved ? (
          <span style={{ color: 'var(--rpx-fg-muted)', fontSize: 12 }}>
            报表 {saved.id} · 草稿基于 {saved.revision.slice(0, 12)} · 当前共享 {sharedRevision.slice(0, 12)} · {draftChanged() ? '有本地草稿' : '已保存'}
          </span>
        ) : null}
      </header>
      {/* W3-4 菜单栏：产品式 IDE 菜单（文件/格式/报表/视图/帮助）。
          与顶栏按钮**并存不重复**：按钮服务高频操作，菜单服务全量入口 +
          面板显隐（右侧属性面板的开关只挂在菜单上）。 */}
      <div className="template-toolbar">
        <label>报表库 <select aria-label="报表库" value={saved?.id ?? ''} onChange={event => {
          if (draftChanged() && defRef.current) { setStatus('请先保存共享定义或下载草稿，再切换报表'); return }
          void refreshLibrary(event.target.value, true)
        }}><option value="">选择报表</option>{reports.map(report => <option key={report.id} value={report.id}>{report.manifest.name}</option>)}</select></label>
        <button onClick={() => void refreshLibrary()}>刷新库与结果</button>
        <button disabled={!def} onClick={() => { setCellPending(false); datasetDirtyRef.current = false; jsonDirtyRef.current = false; void refreshLibrary(saved?.id, true) }}>放弃本地修改并读取共享源码</button>
        <span>{saved?.result_state === 'older_revision' ? '结果对应旧修订' : saved?.result_state === 'current_revision' ? '已有同修订结果' : '尚未生成结果'}</span>
        <label>运行记录 <select aria-label="运行记录" value={selectedRun} onChange={event => { setSelectedRun(event.target.value); setPrintWorkbook(null) }}>
          <option value="">选择运行</option>{runs.map(run => <option key={run.run_id} value={run.run_id}>{run.status} · {run.run_id} · {run.revision.slice(0, 12)}</option>)}
        </select></label>
        {saved?.latest_attempt && <span>最近尝试：{saved.latest_attempt.status}</span>}
      </div>
      <DefinitionSource key={`source-${documentKey}`} value={def} onDirty={dirty => { jsonDirtyRef.current = dirty }} onApply={next => {
        if (cellDirtyRef.current || datasetDirtyRef.current) { setStatus('请先应用或放弃侧栏修改，再应用 JSON 源码草稿'); return false }
        showTemplate(next); setStatus('已应用 JSON 源码草稿，可保存共享定义'); return true
      }} onError={message => setStatus(`源码修改失败：${message}`)} />
      {selectedRun && <details style={{ padding: '0 14px' }}><summary>运行修订、实际参数与诊断</summary>
        {saved && runs.find(run => run.run_id === selectedRun)?.source_files?.map(path => <a key={path} href={sourceUrl(saved.id, selectedRun, path)} target="_blank" rel="noreferrer" style={{ marginRight: 12 }}>查看当时源码 {path}</a>)}
        <pre>{JSON.stringify(runs.find(run => run.run_id === selectedRun), null, 2)}</pre></details>}
      <div className="template-toolbar">
        <button onClick={() => void newReport()}>新建模板</button>
        <button disabled={!def} aria-pressed={view === 'design'} onClick={() => showTemplate()}>模板设计</button>
        <button disabled={!printWorkbook} aria-pressed={view === 'result'} onClick={() => { if (printWorkbook && readyToLeaveCell()) attachSnapshot(printWorkbook, defRef.current, sourceKind ?? 'json') }}>运行结果</button>
        <label>导出文件名 <input aria-label="模板名称" value={reportName} onChange={e => setReportName(e.target.value)}/></label>
        <span>{view === 'design' ? `设计网格 ${def?.grid.rows ?? '—'} × ${def?.grid.cols ?? '—'}` : '运行结果只读，返回模板设计可继续编辑'}{cellDirty ? ' · 当前格有未应用修改' : ''}</span>
        <button disabled={!printWorkbook} onClick={() => setShowPrint(true)}>图形与打印预览</button>
      </div>
      <MenuBar groups={menuGroups} />
      {showPrint && printWorkbook && <Suspense fallback={<p role="status">正在装载打印预览…</p>}><PrintPreview workbook={printWorkbook} onClose={() => setShowPrint(false)} /></Suspense>}
      
      {status ? (
        <div
          style={{
            padding: 14,
            fontSize: 13,
            fontFamily: 'system-ui, sans-serif',
            color: ERROR_PREFIXES.some((p) => status.startsWith(p))
              ? 'var(--rpx-danger)'
              : 'var(--rpx-fg-muted)',
          }}
        >
          {status}
        </div>
      ) : null}
      {/* W3-5 主体：Univer 画布（flex 撑满）+ 右侧停靠属性面板（产品
          `RPX.java` 的 desk + 属性栏布局）。侧栏整体显隐由「视图」菜单控制。 */}
      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <div ref={containerRef} data-sheet-ready={sheetReady} aria-busy={!!apiRef.current && !sheetReady} style={{ flex: 1, minWidth: 0, minHeight: 0, pointerEvents: sheetReady ? 'auto' : 'none' }} />
        <div style={{ display: sideVisible ? 'contents' : 'none' }}>
          <SidePanes
            cellEditor={<TemplateCellPane key={`cell-${documentKey}`} def={def} selection={selection} disabled={panelDisabled} onApply={onTemplateCellApply} onDirtyChange={setCellPending} />}
            datasetEditor={<TemplateDatasets key={`datasets-${documentKey}`} def={def} files={runFiles().filter(f => !f.name.toLowerCase().endsWith('.json') && !f.name.toLowerCase().endsWith('.rpx'))} disabled={panelDisabled}
              onFiles={addFiles} onDirtyChange={dirty => { datasetDirtyRef.current = dirty }}
              onChange={next => { setDefBoth(next); setStatus('已更新模板数据集；可保存共享定义') }} onError={message => setStatus(`编辑失败：${message}`)} />}
            api={{
              def,
              selection,
              panelDisabled,
              pv,
              onPanelChange: (field, value) => ownWrite(() => onPanelChange(field, value)),
              onPanelReset: () => ownWrite(onPanelReset),
              propsPanel,
              onPropsChange: edit => ownWrite(() => onPropsChange(edit)),
              onRowHeightChange: mm => ownWrite(() => onRowHeightChange(mm)),
              onColWidthChange: mm => ownWrite(() => onColWidthChange(mm)),
              onBreakPageChange,
              gridDraft,
              setGridDraft,
              onGridResize,
              onParametersChange: next => { setDefBoth(next); setStatus('已更新参数声明，可保存共享定义') },
              setupPanel,
              onSetupChange,
              onSwapPaper,
            }}
          />
        </div>
      </div>
    </div>
  )
}
