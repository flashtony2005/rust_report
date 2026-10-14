import { useEffect, useRef, useState } from 'react'
import type { FUniver } from '@univerjs/presets'

import { fetchSnapshot, mountUniver, type ParamRow } from './report'

/**
 * rpx 报表预览（M4）。
 *
 * 页面结构：顶栏（标题 + 文件选择 + 渲染按钮）/ 参数表单（M4-04，动态行）/
 * 状态栏 / Univer 容器（flex 撑满）。
 * 数据管道：选文件 → 填参数（可选）→ 点「渲染」→ `POST /api/render` → 装载快照；
 * 流程见 `report.ts`；失败原因**显式上屏**（不静默白屏）。
 *
 * ⚠ 不用 `StrictMode`：dev 下 effect 双执行会二次 `createUniver` 到同一容器
 * （Univer 是 canvas 重量级挂载，非幂等）。骨架期如实取舍，见 `docs/spec-m4.md` §7。
 * ⚠ 重渲染会二次挂载：先 `dispose()` 旧实例再建新的（否则同一容器叠两份 canvas）。
 * ⚠ 配色全部走 Design Token（`:root` 变量）；图标一律内联 SVG（统一描边，不用 emoji）。
 */
export default function App() {
  const containerRef = useRef<HTMLDivElement>(null)
  const apiRef = useRef<FUniver | null>(null)
  const [files, setFiles] = useState<FileList | null>(null)
  const [params, setParams] = useState<ParamRow[]>([])
  const [status, setStatus] = useState('请选择 .rpx 模板（可一起选中其同目录 CSV），填参数后点渲染')

  const render = async () => {
    if (!files) {
      setStatus('请先选择 .rpx 模板文件')
      return
    }
    setStatus('正在渲染…')
    try {
      const snapshot = await fetchSnapshot(Array.from(files), params)
      apiRef.current?.dispose()
      if (!containerRef.current) return
      apiRef.current = mountUniver(containerRef.current)
      apiRef.current.createWorkbook(snapshot as Parameters<typeof apiRef.current.createWorkbook>[0])
      setStatus('')
    } catch (e) {
      setStatus(`装载失败：${e instanceof Error ? e.message : String(e)}`)
    }
  }

  // 卸载时收掉 Univer 实例（canvas + 监听）。
  useEffect(
    () => () => {
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
          <span style={{ color: 'var(--rpx-fg-muted)' }}>· POST /api/render → Univer</span>
        </span>
        <input
          type="file"
          multiple
          accept=".rpx,.csv"
          onChange={(e) => setFiles(e.target.files)}
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
      </header>
      {/* M4-04 参数表单：动态行（v1 手工核对口径，spec-m4 §5）。 */}
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
            color:
              status.startsWith('装载失败') || status.startsWith('请先')
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
