import { afterEach, expect, it, vi } from 'vitest'
import { galleryFiles } from './gallery-data'

afterEach(() => vi.unstubAllGlobals())

it('设计器入口装载固定版本的定义及附件，同一 Excel 多工作表只取一次文件', async () => {
  const definition = { datasets: [
    { name: 'products', file: { file_name: 'sources.xlsx', sheet_name: '产品' } },
    { name: 'budget', file: { file_name: 'sources.xlsx', sheet_name: '预算' } },
    { name: 'refunds', file: { file_name: 'refunds.csv', delimiter: ',' } },
    { name: 'actuals', sql: { dsn: 'operations-db', sql: 'SELECT * FROM ledger' } },
  ] }
  const fetch = vi.fn(async (url: string) => new Response(url.endsWith('/template.json') ? JSON.stringify(definition) : 'data'))
  vi.stubGlobal('fetch', fetch)
  const files = await galleryFiles('/report-gallery/run-123/mixed-data-sources')
  expect(files.map(f => f.name)).toEqual(['template.json', 'sources.xlsx', 'refunds.csv'])
  expect(JSON.parse(await files[0].text())).toEqual(definition)
  expect(fetch).toHaveBeenCalledTimes(3)
  expect(fetch.mock.calls.every(([url]) => url.startsWith('/report-gallery/run-123/'))).toBe(true)
})

it('运行目录或附件无效时显示错误，保留清晰的失败状态', async () => {
  await expect(galleryFiles('https://example.com')).rejects.toThrow('无效')
  vi.stubGlobal('fetch', vi.fn(async () => new Response('missing', { status: 404 })))
  await expect(galleryFiles('/report-gallery/run-123/demo')).rejects.toThrow('HTTP 404')
})
