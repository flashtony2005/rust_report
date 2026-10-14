import { useEffect, useRef, useState } from 'react'

import { fetchSnapshot, mountUniver } from './report'

/**
 * rpx 报表预览（M4 骨架）。
 *
 * 页面结构：顶栏（标题 + 状态） / Univer 容器（flex 撑满）。
 * 装载流程见 `report.ts`；失败原因**显式上屏**（不静默白屏）。
 *
 * ⚠ 不用 `StrictMode`：dev 下 effect 双执行会二次 `createUniver` 到同一容器
 * （Univer 是 canvas 重量级挂载，非幂等）。骨架期如实取舍，见 `docs/spec-m4.md` §7。
 */
export default function App() {
  const containerRef = useRef<HTMLDivElement>(null)
  const [status, setStatus] = useState('正在装载…')

  useEffect(() => {
    let disposed = false
    ;(async () => {
      try {
        const snapshot = await fetchSnapshot()
        if (disposed || !containerRef.current) return
        const api = mountUniver(containerRef.current)
        api.createWorkbook(snapshot as Parameters<typeof api.createWorkbook>[0])
        setStatus('')
      } catch (e) {
        setStatus(`装载失败：${e instanceof Error ? e.message : String(e)}`)
      }
    })()
    return () => {
      disposed = true
    }
  }, [])

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
      <header
        style={{
          padding: '8px 14px',
          borderBottom: '1px solid #d8dee4',
          fontSize: 14,
          fontFamily: 'system-ui, sans-serif',
          background: '#f6f8fa',
        }}
      >
        rpx 报表预览 <span style={{ color: '#8b949e' }}>· 快照 → Univer（M4 骨架）</span>
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
