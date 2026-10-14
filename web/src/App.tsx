import { useEffect, useRef, useState } from 'react'
import type { FUniver } from '@univerjs/presets'

import { fetchSnapshot, mountUniver } from './report'

/**
 * rpx 报表预览（M4 骨架）。
 *
 * 页面结构：顶栏（标题 + 文件选择 + 状态） / Univer 容器（flex 撑满）。
 * 数据管道 v2（M4-06）：选文件 → `POST /api/render` → 装载快照；流程见
 * `report.ts`；失败原因**显式上屏**（不静默白屏）。
 *
 * ⚠ 不用 `StrictMode`：dev 下 effect 双执行会二次 `createUniver` 到同一容器
 * （Univer 是 canvas 重量级挂载，非幂等）。骨架期如实取舍，见 `docs/spec-m4.md` §7。
 * ⚠ 重选文件会二次挂载：先 `dispose()` 旧实例再建新的（否则同一容器叠两份 canvas）。
 */
export default function App() {
  const containerRef = useRef<HTMLDivElement>(null)
  const apiRef = useRef<FUniver | null>(null)
  const [files, setFiles] = useState<FileList | null>(null)
  const [status, setStatus] = useState('请选择 .rpx 模板（可一起选中其同目录 CSV）')

  useEffect(() => {
    let disposed = false
    if (!files) return
    ;(async () => {
      setStatus('正在渲染…')
      try {
        const snapshot = await fetchSnapshot(Array.from(files))
        if (disposed || !containerRef.current) return
        apiRef.current?.dispose()
        apiRef.current = mountUniver(containerRef.current)
        apiRef.current.createWorkbook(snapshot as Parameters<typeof apiRef.current.createWorkbook>[0])
        setStatus('')
      } catch (e) {
        setStatus(`装载失败：${e instanceof Error ? e.message : String(e)}`)
      }
    })()
    return () => {
      disposed = true
    }
  }, [files])

  // 卸载时收掉 Univer 实例（canvas + 监听）。
  useEffect(
    () => () => {
      apiRef.current?.dispose()
    },
    [],
  )

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
      <header
        style={{
          padding: '8px 14px',
          borderBottom: '1px solid #d8dee4',
          fontSize: 14,
          fontFamily: 'system-ui, sans-serif',
          background: '#f6f8fa',
          display: 'flex',
          alignItems: 'center',
          gap: 12,
        }}
      >
        <span>
          rpx 报表预览 <span style={{ color: '#8b949e' }}>· POST /api/render → Univer</span>
        </span>
        <input
          type="file"
          multiple
          accept=".rpx,.csv"
          onChange={(e) => setFiles(e.target.files)}
          style={{ fontSize: 13 }}
        />
      </header>
      {status ? (
        <div style={{ padding: 14, color: status.startsWith('装载失败') ? '#cf222e' : '#57606a' }}>
          {status}
        </div>
      ) : null}
      <div ref={containerRef} style={{ flex: 1, minHeight: 0 }} />
    </div>
  )
}
