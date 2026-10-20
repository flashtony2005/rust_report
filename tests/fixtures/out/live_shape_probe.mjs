// 判定：Univer 活态 save() 的快照结构 vs 打印预览需要的字段。
// 用法：node live_shape_probe.mjs <playwright目录> <输出json>
import { writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outFile = process.argv[3]
const base = process.env.RPX_WEB_BASE || 'http://127.0.0.1:5288'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
await page.goto(base + '/gallery.html#store-ranking', { waitUntil: 'networkidle' })
await page.waitForSelector('.gallery-sheet canvas', { timeout: 30000 })
await page.waitForTimeout(2500)

const out = await page.evaluate(() => {
  const api = window.__rpxProbeApi
  const wb = api && api.getActiveWorkbook && api.getActiveWorkbook()
  const snap = wb && wb.save && wb.save()
  if (!snap) return { error: 'no snapshot' }
  const sid = snap.sheetOrder && snap.sheetOrder[0]
  const raw = snap.sheets
  const sheet = raw instanceof Map ? raw.get(sid) : raw && raw[sid]
  const want = ['name', 'printRowCount', 'printColumnCount', 'defaultRowHeight', 'defaultColumnWidth',
    'rowData', 'columnData', 'cellData', 'mergeData', 'rpxGraphics', 'pageBackground', 'printSize', 'printOptions']
  const have = {}
  const missing = []
  for (const k of want) {
    if (sheet && k in sheet) {
      const v = sheet[k]
      have[k] = v && typeof v === 'object' ? (Array.isArray(v) ? 'array(' + v.length + ')' : 'object(' + Object.keys(v).length + ')') : v
    } else missing.push(k)
  }
  const cellData = (sheet && sheet.cellData) || {}
  return {
    topKeys: Object.keys(snap || {}),
    snapType: typeof snap,
    rawSheetsType: raw instanceof Map ? 'Map' : typeof raw,
    sheetFound: !!sheet,
    sheetKeys: sheet ? Object.keys(sheet) : [],
    have, missing,
    graphicsCount: (sheet && sheet.rpxGraphics && sheet.rpxGraphics.length) || 0,
    mergeCount: (sheet && sheet.mergeData && sheet.mergeData.length) || 0,
    cellCount: Object.keys(cellData).length,
    sheetOrder: snap.sheetOrder,
  }
})

writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(JSON.stringify(out, null, 1))
await browser.close()
