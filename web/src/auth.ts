const TOKEN_KEY = 'rpx.auth.token'
export const AUTH_REQUIRED_EVENT = 'rpx-auth-required'

export function authToken(): string {
  try { return sessionStorage.getItem(TOKEN_KEY) ?? '' } catch { return '' }
}
export function setAuthToken(token: string): void {
  try {
    if (token) sessionStorage.setItem(TOKEN_KEY, token)
    else sessionStorage.removeItem(TOKEN_KEY)
  } catch { /* sessionStorage unavailable: keep in-memory only for this call */ }
}
export function clearAuthToken(): void {
  setAuthToken('')
}

export async function authStatus(): Promise<boolean> {
  const response = await fetch('/api/auth/status')
  if (!response.ok) throw new Error(`auth status HTTP ${response.status}`)
  const value = (await response.json()) as { required?: boolean }
  return value.required === true
}
