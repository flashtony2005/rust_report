import { useEffect, useState, type ReactNode } from 'react'
import { AUTH_REQUIRED_EVENT, authStatus, authToken, clearAuthToken, setAuthToken } from './auth'

type Phase = 'checking' | 'login' | 'ready' | 'error'

/** 服务端启用 Bearer 鉴权时，先显示登录；无鉴权时直接放行。 */
export default function AuthGate({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<Phase>('checking')
  const [detail, setDetail] = useState('')
  const [token, setToken] = useState('')

  useEffect(() => {
    let alive = true
    authStatus()
      .then((required) => {
        if (!alive) return
        setPhase(required && !authToken() ? 'login' : 'ready')
      })
      .catch((error: unknown) => {
        if (!alive) return
        setDetail(String(error))
        setPhase('error')
      })
    const onRequired = () => {
      clearAuthToken()
      setPhase('login')
    }
    window.addEventListener(AUTH_REQUIRED_EVENT, onRequired)
    return () => {
      alive = false
      window.removeEventListener(AUTH_REQUIRED_EVENT, onRequired)
    }
  }, [])

  if (phase === 'checking') return <div style={{ padding: 32 }}>检查登录状态…</div>
  if (phase === 'error') return <div style={{ padding: 32, color: '#b00020' }}>登录检查失败：{detail}</div>
  if (phase === 'login') {
    return (
      <div style={{ maxWidth: 420, margin: '12vh auto', fontFamily: 'system-ui, sans-serif' }}>
        <h1 style={{ fontSize: 22 }}>RPX 报表库</h1>
        <p style={{ color: '#555' }}>该服务启用了 Bearer 鉴权，请输入访问令牌。</p>
        <form
          onSubmit={(event) => {
            event.preventDefault()
            if (!token.trim()) return
            setAuthToken(token.trim())
            setPhase('ready')
          }}
        >
          <input
            autoFocus
            type="password"
            value={token}
            onChange={(event) => setToken(event.target.value)}
            placeholder="Access token"
            style={{ width: '100%', padding: '10px 12px', fontSize: 15, boxSizing: 'border-box' }}
          />
          <button type="submit" style={{ marginTop: 12, padding: '9px 16px' }}>进入报表库</button>
        </form>
      </div>
    )
  }
  return <>{children}</>
}
