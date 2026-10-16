import { useEffect, useRef, useState } from 'react'
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

/** 状态栏里算「出错」的前缀（红色上屏）。新加出错文案必须登记进来，
 * 否则会退化成灰色提示 —— 那是**静默降级**，用户看不出失败了。 */
const ERROR_PREFIXES = ['装载失败', '入库失败', '请先', '未找到', '编辑失败', '编辑面']

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

  // 卸载时收掉单元格监听 + Univer 实例（canvas + 监听）。
  useEffect(
    () => () => {
      eventDisposableRef.current?.dispose()
      apiRef.current?.dispose()
    },
    [],
  )

  const setRow = (i: number, patch: Partial<ParamRow>) =>
    setParams((rows) => rows.map((r, j) => (j === i ? { ...r, ...patch } : r)))

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
