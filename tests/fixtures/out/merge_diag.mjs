// 诊断：合并链路的每一环的输入输出。
// 挂探针 → 拖列宽 → 读「活态 columnData」「显示副本 columnData」「快照 columnData」
// 并在前端直接调用 mergeLiveLayout 看合并结果。
// 用法：node merge_diag.mjs <playwright目录> <输出json>
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

const readCols = () => page.evaluate(() => {
  const api = window.__rpxProbeApi
  const wb = api && api.getActiveWorkbook && api.getActiveWorkbook()
  const snap = wb && wb.save && wb.save()
  const sid = snap && snap.sheetOrder && snap.sheetOrder[0]
  const raw = snap && snap.sheets
  const sheet = raw instanceof Map ? raw.get(sid) : raw && raw[sid]
  const cols = {}
  for (const [k, v] of Object.entries((sheet && sheet.columnData) || {})) cols[k] = v && v.w
  return { sheetId: sid, cols }
})

const out = {}
out.before = await readCols()

await page.mouse.move(358, 326.59375)
await page.mouse.down()
await page.mouse.move(398, 326.59375, { steps: 15 })
await page.mouse.up()
await page.waitForTimeout(1200)

out.after = await readCols()

// 直接读页面里 React 的状态：预览用的 printWorkbook 挂在 DOM 上不好读，
// 改为读「打开预览后渲染出的 colgroup 宽度」——已在 e2e 里做过。
// 这里额外读：原始快照 workbook.json 的列宽（通过 fetch 静态资源）
out.snapshotCols = await page.evaluate(async () => {
  const cat = await (await fetch('/report-gallery/catalog.json')).json()
  const rep = cat.reports.find(r => r.id === 'store-ranking')
  const wb = await (await fetch(rep.base + '/workbook.json')).json()
  const sid = wb.sheetOrder[0]
  const sheet = wb.sheets[sid]
  const cols = {}
  for (const [k, v] of Object.entries(sheet.columnData || {})) cols[k] = v && v.w
  return cols
})

writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(JSON.stringify(out, null, 1))
await browser.close()
