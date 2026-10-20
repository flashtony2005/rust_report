/**
 * 「结果」按钮的重算通道：`POST /reports/{id}/runs`。
 *
 * 契约要点（改坏了等于把「点了结果却看到旧结果」的老问题请回来）：
 * 1. 必须是 **POST 同一个资源路径**（`/reports/{id}/runs`），不是 `/render`；
 * 2. 必须带 `If-Match: <revision>` —— 服务端据此拒绝基于旧定义的计算；
 * 3. 请求体缺省是 `{}`（不带导出，只出 workbook.json 等固定产物）。
 */
import { afterEach, describe, expect, it, vi } from 'vitest'
import { executeReport } from './library-api'

describe('executeReport', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('POST 到 /api/reports/{id}/runs 并带 If-Match 修订', async () => {
    const fetch = vi.fn(async () => new Response(JSON.stringify({ run: { run_id: 'run-1', status: 'succeeded' } }), { status: 200 }))
    vi.stubGlobal('fetch', fetch)

    const started = await executeReport('store ranking/x', 'rev-1')

    expect(started.run.run_id).toBe('run-1')
    const [url, init] = fetch.mock.calls[0] as unknown as [string, RequestInit]
    expect(url).toBe('/api/reports/store%20ranking%2Fx/runs')
    expect(init.method).toBe('POST')
    expect(new Headers(init.headers).get('If-Match')).toBe('rev-1')
    expect(init.body).toBe('{}')
  })

  it('导出与参数原样透传（服务端只认这三个字段）', async () => {
    const fetch = vi.fn(async () => new Response(JSON.stringify({ run: { run_id: 'run-2' } }), { status: 200 }))
    vi.stubGlobal('fetch', fetch)

    await executeReport('r', 'rev-2', { exports: ['xlsx'], params: { 月份: '2026-10' } })

    const [, init] = fetch.mock.calls[0] as unknown as [string, RequestInit]
    expect(JSON.parse(String(init.body))).toEqual({ exports: ['xlsx'], params: { 月份: '2026-10' } })
  })

  it('412 修订冲突要冒泡成 LibraryError（UI 据此提示重新读取）', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(
      JSON.stringify({ error: { code: 'REVISION_CONFLICT', message: '源码已变化' } }),
      { status: 412 },
    )))

    await expect(executeReport('r', 'old')).rejects.toMatchObject({ status: 412, message: '源码已变化' })
  })
})
