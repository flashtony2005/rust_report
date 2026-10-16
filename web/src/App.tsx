import { useEffect, useMemo, useRef, useState } from 'react'
import type { FUniver } from '@univerjs/presets'

import {
  applyValueEdit,
  assertEditable,
  downloadDefJson,
  exportDefJson,
  type CellValue,
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
  exportReportFile,
  fetchMeta,
  fetchSnapshot,
  fetchSnapshotWithDef,
  metaToRows,
  mountUniver,
  pickReportFile,
  saveReport,
  type ParamRow,
} from './report'
import MenuBar, { type MenuGroup } from './menubar'
import SidePanes from './panes'
import {
  applyColWidths,
  applyGridResize,
  applyRowBreakPages,
  applyRowHeights,
  mmToPx,
} from './rowcol'
import {
  applyDatasetAdd,
  applyDatasetRemove,
  type DatasetInput,
} from './datasets'
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
const ERROR_PREFIXES = ['装载失败', '入库失败', '请先', '未找到', '编辑失败', '编辑面']

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
 * rpx 报表预览 + 定义 JSON 通道（M4 + M7-W W1）+ 编辑面（M7-W W2-1）。
 *
 * 页面结构：顶栏（标题 + 文件选择 + 渲染 / 另存为 / **导出 JSON**）/ 参数表单
 * （W1-2 起按 `/meta` 自动带出）/ 状态栏 / Univer 容器（flex 撑满）。
 * 数据管道：选文件（`.rpx` 或 `.json`）→ 点「渲染」→ `POST /api/render` →
 * 装载快照；点「另存为」→ `POST /reports` 拿 id → 按 `/meta` 刷新参数表单；
 * 失败原因**显式上屏**（不静默白屏）。流程见 `report.ts`。
 *
 * W2-1 编辑面的三条口径（实现在 `designer.ts`）：
 * * **持有原始定义 JSON**（渲染时读文件文本）并对它打补丁 —— 不拿快照反推
 *   （快照有损，见 `stylemap.ts` 文件头）；
 * * **只跟 `SheetValueChanged` 事件**补用户真改过的那一格 —— 不做整体回灌，
 *   否则 Univer 会把 `"0123"` 这类值按数值归一化，没碰过的格子也被改坏；
 * * 编辑失败（越界 / 落在合并矩形内 / 该格由表达式驱动）**显式上屏**，
 *   不猜、不吞。
 *
 * ⚠ 不用 `StrictMode`：dev 下 effect 双执行会二次 `createUniver` 到同一容器
 * （Univer 是 canvas 重量级挂载，非幂等）。骨架期如实取舍，见 `docs/spec-m4.md` §7。
 * ⚠ 重渲染会二次挂载：先 `dispose()` 旧实例再建新的（否则同一容器叠两份 canvas）。
 * ⚠ 配色全部走 Design Token（`:root` 变量）；图标一律内联 SVG（统一描边，不用 emoji）。
 */
export default function App() {
  const containerRef = useRef<HTMLDivElement>(null)
  const apiRef = useRef<FUniver | null>(null)
  /** W2-1：单元格改动事件的订阅句柄（`addEvent` 返回的 `IDisposable`）。
   * 重渲染会二次 `createWorkbook`，必须先把旧监听 dispose 掉，否则叠两份。 */
  const eventDisposableRef = useRef<{ dispose: () => void } | null>(null)
  /** W2-2：选区变化（`CommandExecuted` 过滤 `sheet.operation.set-selections`）
   * 的订阅句柄；同 `eventDisposableRef` 的 dispose 纪律。 */
  const selectionDisposableRef = useRef<{ dispose: () => void } | null>(null)
  /** W2-2：样式面板显示态。`null` = 当前没有选区可读（或未渲染）⇒ 面板显示
   * 缺省并整体置灰（`.rpx` 源没有定义 JSON 可打补丁，见 `assertEditable`）。 */
  const [panel, setPanel] = useState<PanelStyle | null>(null)
  /** W2-3：当前选区的坐标快照（属性面板显示态的派生来源；`null` = 无选区）。 */
  const [selection, setSelection] = useState<PanelRange | null>(null)
  const [files, setFiles] = useState<FileList | null>(null)
  const [params, setParams] = useState<ParamRow[]>([])
  /** 「另存为」拿到的服务端 id（W1-3）；换文件即失效（id 属于旧文件）。 */
  const [saved, setSaved] = useState<{ id: number; name: string } | null>(null)
  /** W2-1：持有的定义 JSON（**打了补丁的那份**）。只有 `.json` 报表源才有；
   * `.rpx` 与未渲染时为 null ⇒ 编辑面不可用（`assertEditable` 给文案）。 */
  const [def, setDef] = useState<ReportDef | null>(null)
  /** W2-1：`def` 的镜像 ref —— 事件回调是闭包，若直接读 `def` 状态会拿到
   * 订阅那一刻的**旧值**（改一格后 `setDef` 的新值进不来）。补丁逻辑一律
   * 读 `defRef.current`，保证永远 patch 在最新持有的 JSON 上。 */
  const defRef = useRef<ReportDef | null>(null)
  /** 同时更新 `def` 状态与 `defRef` 镜像（W2-1 专用 setter）。 */
  const setDefBoth = (next: ReportDef | null) => {
    defRef.current = next
    setDef(next)
  }
  /** W2-1：报表源形态，用于「.rpx 不能编辑」这条能力边界的提示。 */
  const [sourceKind, setSourceKind] = useState<'json' | 'rpx' | null>(null)
  const [status, setStatus] = useState(
    '请选择 .rpx 或 .json 报表源（可一起选中其同目录 CSV），点渲染；点「另存为」后可自动带出参数清单',
  )

  /** W2-5：报表源文件名（重渲染时 def 上传体沿用这个名字）。 */
  const [reportName, setReportName] = useState('report-definition.json')

  /**
   * 装载快照 + 订阅事件（W2-5 抽取：`render` 与 `renderFromDef` 共用的后半程）。
   *
   * 原逻辑逐字搬入（dispose 旧实例 → mount → createWorkbook → 两事件订阅），
   * 只把「held / kind 的来源」参数化 —— 从文件渲染时 held 来自文件文本，
   * 补丁后重渲染时 held 就是**当前持有的 def**（补丁不丢的唯一保证）。
   */
  const attachSnapshot = (snapshot: unknown, held: ReportDef | null, kind: 'json' | 'rpx') => {
    apiRef.current?.dispose()
    if (!containerRef.current) return
    apiRef.current = mountUniver(containerRef.current)
    apiRef.current.createWorkbook(snapshot as Parameters<typeof apiRef.current.createWorkbook>[0])
    setDefBoth(held)
    setSourceKind(kind)
    setStatus(
      held
        ? '已装载定义 JSON，可直接编辑单元格内容；改完点「导出 JSON」拿回定义文件'
        : '已装载（.rpx 模板只读预览）',
    )
    // W2-1：订阅单元格改动事件。**只补用户真改过的那一格**（理由见
    // `designer.ts` 文件头：整体回灌会把 `"0123"` 这类值按数值归一化，
    // 没碰过的格子也被改坏）。旧监听先 dispose，避免二次挂载叠两份。
    eventDisposableRef.current?.dispose()
    eventDisposableRef.current = apiRef.current.addEvent(
      apiRef.current.Event.SheetValueChanged,
      (params) => {
        const ranges = params.effectedRanges
        // 批量/多区域编辑（粘贴、填充）暂不做内容补丁 —— 超出 W2-1 单格口径；
        // 多区域只补第一格是**静默半截**，故直接跳过，不假装成功。
        if (!ranges || ranges.length !== 1) return
        // 只处理「值改动」（`SET_RANGE_VALUES`）。合并/结构类改动（如
        // 合并矩形增减）不进**内容**补丁，否则把结构动作误当成内容写坏。
        const id = (params.payload as { id: string }).id
        if (id !== 'sheet.mutation.set-range-values') return
        const range = ranges[0]
        onCellEdited(range.getRow(), range.getColumn(), range.getValue())
      },
    )
    // W2-2：订阅选区变化 —— 面板显示态跟着用户选中的格子走。facade 的
    // sheets 事件表里**没有**选区专用事件（`f-event.d.ts` 的 getter 清单里
    // 无 Selection*）⇒ 用命令级事件 `CommandExecuted` 按 id 过滤：
    // `sheet.operation.set-selections` 是选区落定的命令
    // （`@univerjs/sheets/lib/es/index.js:5081` 实测）。
    selectionDisposableRef.current?.dispose()
    selectionDisposableRef.current = apiRef.current.addEvent(
      apiRef.current.Event.CommandExecuted,
      (e) => {
        if (e.id === 'sheet.operation.set-selections') syncPanel()
      },
    )
    syncPanel()
  }

  const render = async () => {
    if (!files) {
      setStatus('请先选择报表源文件')
      return
    }
    const report = pickReportFile(Array.from(files))
    if (!report) {
      setStatus('未找到报表源文件（.rpx 或 .json）')
      return
    }
    setStatus('正在渲染…')
    try {
      // W2-5：**持有 def 时优先用它**——点「渲染」是刷新画布，不是丢弃补丁
      // 重新读盘（那会把用户的全部编辑静默回滚，比报错恶劣得多）。
      const held = defRef.current
      if (held) {
        const snapshot = await fetchSnapshotWithDef(
          JSON.stringify(held),
          reportName,
          Array.from(files),
          params,
        )
        attachSnapshot(snapshot, held, 'json')
        return
      }
      const snapshot = await fetchSnapshot(Array.from(files), params)
      // W2-1：**先读定义 JSON 文本再挂 Univer** —— 编辑面要持原始 JSON 打补丁。
      // 只有 `.json` 才有；`.rpx` 是二进制，持不住 ⇒ 编辑面不可用（显式告知）。
      const isJson = report.name.toLowerCase().endsWith('.json')
      let fromFile: ReportDef | null = null
      if (isJson) {
        const text = await report.text()
        fromFile = JSON.parse(text) as ReportDef
        if (!fromFile || typeof fromFile !== 'object' || !fromFile.grid) {
          throw new Error('定义 JSON 缺 grid 字段，无法进入编辑面')
        }
      }
      setReportName(report.name)
      attachSnapshot(snapshot, fromFile, isJson ? 'json' : 'rpx')
    } catch (e) {
      setDefBoth(null)
      setSourceKind(null)
      setStatus(`装载失败：${e instanceof Error ? e.message : String(e)}`)
    }
  }

  /**
   * W2-1：把一格的值改动补丁进持有的定义 JSON。
   *
   * 失败**不丢弃改动也不猜**：把服务端会拒的理由原样上屏（`designer.ts` 的
   * 四条约束各自给一句话），让用户知道为什么这一格改不了。
   */
  const onCellEdited = (row0: number, col0: number, value: CellValue) => {
    const cur = defRef.current
    if (!cur) return
    const r = applyValueEdit(cur, row0, col0, value)
    if (r.error) {
      setStatus(`编辑失败：${r.error}`)
      return
    }
    setDefBoth(r.def ?? cur)
    const warn = r.warnings.length > 0 ? `（${r.warnings.join('；')}）` : ''
    setStatus(`已更新定义 JSON${warn}；点「导出 JSON」拿回文件`)
  }

  /** W2-1：导出（编辑面唯一出口）。 */
  const exportJson = () => {
    const cur = defRef.current
    if (!cur) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先渲染 .json 报表源'}`)
      return
    }
    downloadDefJson(cur, 'report-definition.json')
    setStatus(`已导出定义 JSON（${exportDefJson(cur).length} 字符）`)
  }

  /** W3-4：导出 Excel/CSV（`POST /export`，M6-03 服务端已就绪）——优先导出
   * **打过补丁的 def**（与「渲染」持有 def 优先同口径），否则回落磁盘源。 */
  const exportFile = async (fmt: 'csv' | 'xlsx') => {
    const cur = defRef.current
    if (!files && !cur) {
      setStatus('请先选择报表源文件')
      return
    }
    setStatus(`正在导出 ${fmt.toUpperCase()}…`)
    try {
      const { blob, filename } = await exportReportFile(
        cur ? exportDefJson(cur) : null,
        Array.from(files ?? []),
        params,
        fmt,
      )
      downloadBlob(blob, filename)
      setStatus(`已导出 ${filename}（${blob.size} 字节）`)
    } catch (e) {
      setStatus(`导出失败：${e instanceof Error ? e.message : String(e)}`)
    }
  }

  // -------------------------------------------------------------------------
  // W3-4 菜单栏（产品式 IDE 菜单；与顶栏按钮并存不重复）
  // -------------------------------------------------------------------------

  /** W3-5：右侧属性面板整体显隐（「视图」菜单；折叠是纯 UI，无判据含义）。 */
  const [sideVisible, setSideVisible] = useState(true)

  /** W3-4：菜单全集。禁用的判据与顶栏按钮一致（无文件/无 def 时置灰）。 */
  const menuGroups: MenuGroup[] = [
    {
      label: '文件',
      items: [
        { label: '渲染', onPick: () => void render(), disabled: !files, hint: 'POST /api/render 装载快照（持有 def 时优先用它）' },
        { label: '另存为（入库）', onPick: () => void save(), disabled: !files, hint: 'POST /reports 拿 id，并按 /meta 刷新参数表单' },
        { sep: true },
        { label: '导出 JSON（定义）', onPick: exportJson, disabled: !def, hint: '把打过补丁的定义 JSON 下载回本地' },
        { label: '导出 Excel（.xlsx）', onPick: () => void exportFile('xlsx'), disabled: !files && !def, hint: 'POST /api/export?fmt=xlsx（M6-03）' },
        { label: '导出 CSV（.csv）', onPick: () => void exportFile('csv'), disabled: !files && !def, hint: 'POST /api/export?fmt=csv（M6-03）' },
      ],
    },
    {
      label: '格式',
      items: [
        { label: '样式面板', onPick: () => setSideVisible(true), hint: '右侧「样式」分区（W2-2 七字段）' },
        { label: '属性面板（边框/格式/旋转）', onPick: () => setSideVisible(true), hint: '右侧「属性」分区（W2-3）' },
      ],
    },
    {
      label: '报表',
      items: [
        { label: '报表设置（纸张/边距/分页）', onPick: () => setSideVisible(true), hint: '右侧「报表设置」分区（W3-1，print_setup）' },
        { label: '行属性（行高/断页/网格）', onPick: () => setSideVisible(true), hint: '右侧「行列」分区（W2-5）' },
        { label: '数据集', onPick: () => setSideVisible(true), hint: '右侧「数据集」分区（W2-5）' },
        { label: '报表参数', onPick: () => setSideVisible(true), hint: '右侧「报表参数」分区（W1-2）' },
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
              'rpx Web 设计器 · 定义 JSON 通道（M7-W W1）+ 编辑面（W2-1~W2-5）+ 报表设置（W3-1）；链路：/render → Univer 快照，/export → 文件字节',
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
    const cur = currentSelection()
    if (!cur) {
      setStatus('编辑失败：请先在表格里选中单元格，再改样式')
      return null
    }
    const held = defRef.current
    if (!held) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先渲染 .json 报表源'}`)
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
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先渲染 .json 报表源'}`)
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
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先渲染 .json 报表源'}`)
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
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先渲染 .json 报表源'}`)
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

  /** W2-5：调整网格 —— 校验 + 改 JSON + **用持有 def 重渲染**（画布按新尺寸装载）。 */
  const onGridResize = async () => {
    const held = defRef.current
    if (!held) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先渲染 .json 报表源'}`)
      return
    }
    const rows = Number(gridDraft.rows)
    const cols = Number(gridDraft.cols)
    const r = applyGridResize(held, rows, cols)
    if (r.error || !r.def) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    setDefBoth(r.def)
    setStatus('网格已调整，正在按新尺寸重渲染…')
    try {
      const snapshot = await fetchSnapshotWithDef(
        JSON.stringify(r.def),
        reportName,
        Array.from(files ?? []),
        params,
      )
      attachSnapshot(snapshot, r.def, 'json')
      setStatus('网格已调整并重渲染；点「导出 JSON」拿回文件')
    } catch (e) {
      setStatus(`重渲染失败：${e instanceof Error ? e.message : String(e)}`)
    }
  }

  /** W2-5：数据集草稿（添加行）。修改已有条目 v1 不做 UI —— 删了重加，
   * 或导出 JSON 手改（`datasets.ts::applyDatasetUpdate` 语义已有单测钉住）。 */
  const [dsDraft, setDsDraft] = useState<DatasetInput>({ name: '', file: { file_name: '', encoding: 'GBK' } })

  /** W2-5：添加数据集（校验失败上屏，成功后清空草稿）。 */
  const onDatasetAdd = () => {
    const held = defRef.current
    if (!held) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先渲染 .json 报表源'}`)
      return
    }
    const r = applyDatasetAdd(held, dsDraft)
    if (r.error || !r.def) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    setDefBoth(r.def)
    setDsDraft({ name: '', file: { file_name: '', encoding: 'GBK' } })
    const warn = r.warnings.length > 1 ? `（${r.warnings[0]}）` : ''
    setStatus(`已添加数据集${warn}；点「导出 JSON」拿回文件`)
  }

  /** W2-5：删除第 i 条数据集。 */
  const onDatasetRemove = (i: number) => {
    const held = defRef.current
    if (!held) return
    const r = applyDatasetRemove(held, i)
    if (r.error || !r.def) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    setDefBoth(r.def)
    setStatus('已删除数据集；点「导出 JSON」拿回文件')
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
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先渲染 .json 报表源'}`)
      return
    }
    const r = applyPrintSetup(held, { [field]: value })
    if (r.error || !r.def) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    setDefBoth(r.def)
    setStatus(`已更新报表设置 ${field}；点「渲染」生效，或「导出 JSON」拿回文件`)
  }

  /** W3-1：横竖互换（纸张宽高对调）。 */
  const onSwapPaper = () => {
    const held = defRef.current
    if (!held) {
      setStatus(`编辑面不可用：${assertEditable(sourceKind) ?? '请先渲染 .json 报表源'}`)
      return
    }
    const r = swapPaperOrientation(held)
    if (r.error || !r.def) {
      setStatus(`编辑失败：${r.error ?? '值不合法'}`)
      return
    }
    setDefBoth(r.def)
    setStatus('已互换纸张方向；点「渲染」生效')
  }

  /** 「另存为」（W1-3 + W1-2）：入库拿 id → 按 `/meta` 刷新参数表单。 */
  const save = async () => {
    if (!files) {
      setStatus('请先选择报表源文件')
      return
    }
    const report = pickReportFile(Array.from(files))
    if (!report) {
      setStatus('未找到报表源文件（.rpx 或 .json）')
      return
    }
    setStatus('正在入库…')
    try {
      const { id, name } = await saveReport(report)
      setSaved({ id, name })
      // W1-2：清单是权威来源 —— **替换**整个表单，不再手工填名。
      const items = await fetchMeta(id)
      setParams(metaToRows(items))
      setStatus(
        `已入库：${name}（id=${id}）；参数表单已按服务端声明刷新（${items.length} 条）`,
      )
    } catch (e) {
      setStatus(`入库失败：${e instanceof Error ? e.message : String(e)}`)
    }
  }

  // 卸载时收掉单元格监听 + 选区监听 + Univer 实例（canvas + 监听）。
  useEffect(
    () => () => {
      eventDisposableRef.current?.dispose()
      selectionDisposableRef.current?.dispose()
      apiRef.current?.dispose()
    },
    [],
  )

  const setRow = (i: number, patch: Partial<ParamRow>) =>
    setParams((rows) => rows.map((r, j) => (j === i ? { ...r, ...patch } : r)))

  /** W2-2：面板可用性 = 持有定义 JSON（`.rpx` 源没有可打补丁的 JSON）。 */
  const panelDisabled = !def
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
          rpx 报表预览{' '}
          <span style={{ color: 'var(--rpx-fg-muted)' }}>
            · POST /api/render + /reports → Univer
          </span>
        </span>
        <input
          type="file"
          multiple
          accept=".rpx,.json,.csv"
          onChange={(e) => {
            setFiles(e.target.files)
            // 换文件 ⇒ 旧 id 失效（id 属于上一个文件，不能留在界面上冒充）。
            setSaved(null)
          }}
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
          渲染
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
          另存为
        </button>
        {/* W2-1：编辑面唯一出口 —— 把打过补丁的定义 JSON 下载回来。
            只有渲染过 .json 源才有内容（`def` 非空），否则置灰（能力边界可见，
            而不是点了才报错）。`def` 也兼作「可导出」的真理来源（见 `onCellEdited`
            / `exportJson` 走 `defRef` 读最新值，避免闭包旧值）。 */}
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
          导出 JSON
        </button>
        {saved ? (
          <span style={{ color: 'var(--rpx-fg-muted)', fontSize: 12 }}>
            已入库 id={saved.id}
          </span>
        ) : null}
      </header>
      {/* W3-4 菜单栏：产品式 IDE 菜单（文件/格式/报表/视图/帮助）。
          与顶栏按钮**并存不重复**：按钮服务高频操作，菜单服务全量入口 +
          面板显隐（右侧属性面板的开关只挂在菜单上）。 */}
      <MenuBar groups={menuGroups} />
      
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
        <div ref={containerRef} style={{ flex: 1, minWidth: 0, minHeight: 0 }} />
        {sideVisible ? (
          <SidePanes
            api={{
              def,
              selection,
              panelDisabled,
              pv,
              onPanelChange,
              onPanelReset,
              propsPanel,
              onPropsChange,
              onRowHeightChange,
              onColWidthChange,
              onBreakPageChange,
              gridDraft,
              setGridDraft,
              onGridResize,
              params,
              setParams,
              setRow,
              dsDraft,
              setDsDraft,
              onDatasetAdd,
              onDatasetRemove,
              setupPanel,
              onSetupChange,
              onSwapPaper,
            }}
          />
        ) : null}
      </div>
    </div>
  )
}
