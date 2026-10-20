// 诊断：活态 columnData 的 key 形态与项结构。
import { createRequire } from 'node:module'
const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])

const b = await chromium.launch()
const p = await b.newPage({ viewport: { width: 1680, height: 1000 } })
await p.goto((process.env.RPX_WEB_BASE || 'http://127.0.0.1:5288') + '/gallery.html#store-ranking', { waitUntil: 'networkidle' })
await p.waitForSelector('.gallery-sheet canvas', { timeout: 30000 })
await p.waitForTimeout(2500)
const r = await p.evaluate(() => {
  const api = window.__rpxProbeApi
  const wb = api.getActiveWorkbook()
  const snap = wb.save()
  const raw = snap.sheets
  const sheet = raw instanceof Map ? raw.get(snap.sheetOrder[0]) : raw[snap.sheetOrder[0]]
  const cd = sheet.columnData
  const keys = Object.keys(cd)
  return {
    keys,
    keyTypes: keys.map(k => typeof k),
    entries: keys.map(k => [k, cd[k]]),
    rowDataKeys: Object.keys(sheet.rowData || {}),
    rowSample: sheet.rowData && sheet.rowData[Object.keys(sheet.rowData || {})[0]],
  }
})
console.log(JSON.stringify(r, null, 1))
await b.close()
