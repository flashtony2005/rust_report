import { useEffect, useRef, useState, type CSSProperties } from 'react'
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
  fetchMeta,
  fetchSnapshot,
  metaToRows,
  mountUniver,
  pickReportFile,
  saveReport,
  type ParamRow,
} from './report'
import {
  PANEL_DEFAULTS,
  applyFieldToRange,
  applyPanelToRange,
  clearPanelRange,
  fontOptions,
  panelFromUniverStyle,
  resetRangeToDefaults,
  type PanelField,
  type PanelStyle,
} from './stylepanel'

/** 状态栏里算「出错」的前缀（红色上屏）。新加出错文案必须登记进来，
 * 否则会退化成灰色提示 —— 那是**静默降级**，用户看不出失败了。 */
const ERROR_PREFIXES = ['装载失败', '入库失败', '请先', '未找到', '编辑失败', '编辑面']

/** W2-2 面板的输入控件通用样式（设计 token 与其它控件一致）。 */
const panelInputStyle: CSSProperties = {
  fontSize: 13,
  padding: '3px 6px',
  border: '1px solid var(--rpx-border)',
  borderRadius: 4,
  background: '#fff',
  color: 'var(--rpx-fg)',
}

/** W2-2 颜色输入（`<input type="color">`）的盒子样式。 */
const panelColorStyle: CSSProperties = {
  width: 28,
  height: 24,
  padding: 0,
  border: '1px solid var(--rpx-border)',
  borderRadius: 4,
  background: '#fff',
}

/** W2-2 的三个位样式开关：字段 / 标签 / 标签附加样式（B 粗 I 斜 U 下划线）。 */
const PANEL_TOGGLES = [
  ['bold', 'B', { fontWeight: 'bold' }],
  ['italic', 'I', { fontStyle: 'italic' }],
  ['underline', 'U', { textDecoration: 'underline' }],
] as const satisfies readonly (readonly [PanelField, string, CSSProperties])[]

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
      const snapshot = await fetchSnapshot(Array.from(files), params)
      // W2-1：**先读定义 JSON 文本再挂 Univer** —— 编辑面要持原始 JSON 打补丁。
      // 只有 `.json` 才有；`.rpx` 是二进制，持不住 ⇒ 编辑面不可用（显式告知）。
      const isJson = report.name.toLowerCase().endsWith('.json')
      let held: ReportDef | null = null
      if (isJson) {
        const text = await report.text()
        held = JSON.parse(text) as ReportDef
        if (!held || typeof held !== 'object' || !held.grid) {
          throw new Error('定义 JSON 缺 grid 字段，无法进入编辑面')
        }
      }
      apiRef.current?.dispose()
      if (!containerRef.current) return
      apiRef.current = mountUniver(containerRef.current)
      apiRef.current.createWorkbook(snapshot as Parameters<typeof apiRef.current.createWorkbook>[0])
      setDefBoth(held)
      setSourceKind(isJson ? 'json' : 'rpx')
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

  /** W2-2：把面板显示态刷新为当前选区的样式（选区没了 ⇒ 面板回 null 置灰）。 */
  const syncPanel = () => {
    const cur = currentSelection()
    if (!cur) {
      setPanel(null)
      return
    }
    const cell = cur.sheet.getRange(cur.range.getRow(), cur.range.getColumn())
    setPanel(panelFromUniverStyle(cell.getCellStyleData()))
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
      {/* W2-2 样式面板：7 字段（M7-3）。改哪个字段只补哪个键进定义 JSON
          （逐字段补丁，理由见 `stylepanel.ts` 文件头）；只有渲染过 `.json` 源
          （`def` 非空）才可用，否则整体置灰并在尾部提示原因（能力边界可见，
          而不是点了才报错）。 */}
      <section
        style={{
          padding: '6px 14px',
          borderBottom: '1px solid var(--rpx-border)',
          background: 'var(--rpx-bg-muted)',
          fontFamily: 'system-ui, sans-serif',
          fontSize: 13,
          color: 'var(--rpx-fg)',
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          flexWrap: 'wrap',
        }}
      >
        <span style={{ color: 'var(--rpx-fg-muted)' }}>样式</span>
        <select
          value={pv.fontName}
          disabled={panelDisabled}
          title="字体族（定义 JSON 的 font_name；改回「宋体」= 删该键）"
          onChange={(e) => onPanelChange('fontName', e.target.value)}
          style={panelInputStyle}
        >
          {fontOptions(pv.fontName).map((f) => (
            <option key={f} value={f}>
              {f}
            </option>
          ))}
        </select>
        <input
          type="number"
          min={1}
          max={32767}
          value={pv.fontSize}
          disabled={panelDisabled}
          title="字号（1..=32767 的整数；改回 12 = 删该键）"
          onChange={(e) => onPanelChange('fontSize', e.target.value)}
          style={{ ...panelInputStyle, width: 64 }}
        />
        {PANEL_TOGGLES.map(([field, label, labelStyle]) => (
          <button
            key={field}
            aria-pressed={pv[field]}
            disabled={panelDisabled}
            title={`${label}（定义 JSON 的 ${field}）`}
            onClick={() => onPanelChange(field, !pv[field])}
            style={{
              width: 26,
              height: 24,
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              border: '1px solid var(--rpx-border)',
              borderRadius: 4,
              cursor: panelDisabled ? 'not-allowed' : 'pointer',
              ...(pv[field] && !panelDisabled
                ? { background: 'var(--rpx-accent)', color: '#fff' }
                : { background: '#fff', color: 'var(--rpx-fg)' }),
              ...labelStyle,
            }}
          >
            {label}
          </button>
        ))}
        <label
          style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
          title="字体颜色（定义 JSON 的 fore_color；改回 #000000 = 删该键）"
        >
          字色
          <input
            type="color"
            value={pv.foreColor}
            disabled={panelDisabled}
            onChange={(e) => onPanelChange('foreColor', e.target.value)}
            style={panelColorStyle}
          />
        </label>
        <label
          style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
          title="背景色（定义 JSON 的 back_color；改回 #FFFFFF = 删该键）"
        >
          底色
          <input
            type="color"
            value={pv.backColor}
            disabled={panelDisabled}
            onChange={(e) => onPanelChange('backColor', e.target.value)}
            style={panelColorStyle}
          />
        </label>
        <button
          onClick={onPanelReset}
          disabled={panelDisabled}
          title="把 7 个样式字段全部打回缺省（= 从定义 JSON 里删掉这 7 个键）"
          style={{
            fontSize: 13,
            padding: '3px 10px',
            border: '1px solid var(--rpx-border)',
            borderRadius: 4,
            background: '#fff',
            color: panelDisabled ? 'var(--rpx-fg-muted)' : 'var(--rpx-fg)',
            cursor: panelDisabled ? 'not-allowed' : 'pointer',
          }}
        >
          恢复缺省
        </button>
        {panelDisabled ? (
          <span style={{ color: 'var(--rpx-fg-muted)', fontSize: 12 }}>
            仅定义 JSON（.json）报表源可改样式
          </span>
        ) : null}
      </section>
      {/* W1-2 参数表单：`另存为` 后按 `GET /reports/{id}/meta` 自动带出，
          非静态（`is_static=false`）的行标「动态」；仍可手工增删行。 */}
      <section
        style={{
          padding: '6px 14px',
          borderBottom: '1px solid var(--rpx-border)',
          background: 'var(--rpx-bg-muted)',
          fontFamily: 'system-ui, sans-serif',
          fontSize: 13,
          color: 'var(--rpx-fg)',
        }}
      >
        <span style={{ color: 'var(--rpx-fg-muted)', marginRight: 8 }}>
          报表参数（未登记的参数服务端会显式报错）
        </span>
        {params.map((row, i) => (
          <span
            key={i}
            style={{ display: 'inline-flex', alignItems: 'center', gap: 4, marginRight: 10 }}
          >
            <input
              placeholder="参数名"
              value={row.name}
              onChange={(e) => setRow(i, { name: e.target.value })}
              style={{
                width: 110,
                fontSize: 13,
                padding: '3px 6px',
                border: '1px solid var(--rpx-border)',
                borderRadius: 4,
              }}
            />
            <input
              placeholder="值"
              value={row.value}
              onChange={(e) => setRow(i, { value: e.target.value })}
              style={{
                width: 130,
                fontSize: 13,
                padding: '3px 6px',
                border: '1px solid var(--rpx-border)',
                borderRadius: 4,
              }}
            />
            {/* 动态/属性参数标注（W1-2）：值须渲染时由用户或上游提供。 */}
            {row.dynamic ? (
              <span
                title="动态参数：is_static=false，渲染时须提供值"
                style={{
                  fontSize: 11,
                  padding: '1px 6px',
                  borderRadius: 8,
                  border: '1px solid var(--rpx-accent)',
                  color: 'var(--rpx-accent)',
                }}
              >
                动态
              </span>
            ) : null}
            <button
              aria-label="删除参数"
              title="删除参数"
              onClick={() => setParams((rows) => rows.filter((_, j) => j !== i))}
              style={{
                width: 22,
                height: 22,
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                border: '1px solid var(--rpx-border)',
                borderRadius: 4,
                background: '#fff',
                cursor: 'pointer',
              }}
            >
              {/* 删除图标：内联 SVG（16px，统一描边） */}
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path
                  d="M4 4l8 8M12 4l-8 8"
                  stroke="var(--rpx-danger)"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                />
              </svg>
            </button>
          </span>
        ))}
        <button
          onClick={() => setParams((rows) => [...rows, { name: '', value: '' }])}
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: 4,
            fontSize: 13,
            padding: '3px 10px',
            border: '1px solid var(--rpx-border)',
            borderRadius: 4,
            background: '#fff',
            color: 'var(--rpx-fg)',
            cursor: 'pointer',
          }}
        >
          {/* 添加图标：内联 SVG（16px，统一描边） */}
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path
              d="M8 3v10M3 8h10"
              stroke="var(--rpx-accent)"
              strokeWidth="1.5"
              strokeLinecap="round"
            />
          </svg>
          添加参数
        </button>
      </section>
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
      <div ref={containerRef} style={{ flex: 1, minHeight: 0 }} />
    </div>
  )
}
