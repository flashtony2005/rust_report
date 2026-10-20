/**
 * 复现「设计器里改公式，结果没变 / 没设到」。
 *
 * 走用户的真实路径：设计视图 → 选中合计行的销售额格（C5）→ **画布上直接键入公式**
 * → 回车 → 点「结果」。然后把服务端模板与最新 run 都读出来对照。
 *
 * ⚠ 会真写库：结束时把原 C5 公式写回并重算。
 *
 * 用法：node formula_edit_e2e.mjs <playwright 模块> <输出目录>
 */
import fs from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3] ?? 'F:/project/report/rpx/tests/fixtures/out/gallery_shots'
const ID = 'store-ranking'
const SERVER = 'http://127.0.0.1:8090'
const NEW_FORMULA = 'C4[`0]{}'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', error => errors.push(String(error)))
const api = async path => (await page.request.get(`${SERVER}${path}`)).json()

const original = await api(`/reports/${ID}`)
const originalC5 = original.template.cells.find(cell => cell.row === 5 && cell.col === 3)

await page.goto('http://127.0.0.1:5288/', { waitUntil: 'networkidle' })
await page.waitForTimeout(3000)
await page.selectOption('select[aria-label="报表库"]', ID)
await page.waitForTimeout(3000)
await page.getByRole('button', { name: '设计', exact: true }).click()
await page.waitForFunction(() => document.querySelector('.rpx-canvas')?.getAttribute('data-sheet-ready') === 'true', { timeout: 20000 })

// 用活态 API 选中 C5（0 起：row 4 / col 2），再像用户那样敲键盘
await page.evaluate(() => {
  const sheet = window.__rpxProbeApi.getActiveWorkbook().getActiveSheet()
  sheet.getRange(4, 2).activate()
})
await page.waitForTimeout(400)
await page.keyboard.type(`=${NEW_FORMULA}`, { delay: 30 })
await page.waitForTimeout(300)
// ⚠ 刻意不按 Enter：直接点「结果」，复现「编辑态没提交就切走」的竞态
await page.waitForTimeout(200)
const statusAfterType = (await page.locator('.rpx-status .msg').first().textContent())?.trim()
const canvasValue = await page.evaluate(() => {
  const sheet = window.__rpxProbeApi.getActiveWorkbook().getActiveSheet()
  return sheet.getRange(4, 2).getValue()
})

// 点「结果」：有改动应先自动保存再重算
await page.locator('.rpx-toolbar button.tb.primary').click()
await page.waitForFunction(() => {
  const bar = document.querySelector('.rpx-runbar')
  return !!bar && !bar.classList.contains('running')
}, { timeout: 180000 })
await page.waitForTimeout(1000)
const statusAfterRun = (await page.locator('.rpx-status .msg').first().textContent())?.trim()

const after = await api(`/reports/${ID}`)
const savedC5 = after.template.cells.find(cell => cell.row === 5 && cell.col === 3)
const workbook = await (await page.request.get(`${SERVER}/reports/${ID}/runs/${after.display_run_id}/artifacts/workbook.json`)).json()
const sheet = workbook.sheets[workbook.sheetOrder[0]]
const row = sheet.cellData['11'] ?? {}
const rendered = Object.fromEntries(Object.entries(row).map(([c, cell]) => [c, cell?.v]).sort())

fs.mkdirSync(outDir, { recursive: true })
await page.screenshot({ path: `${outDir}/formula_edit.png` })

// ── 还原 C5 ─────────────────────────────────────────────────────────────
const now = await api(`/reports/${ID}`)
const restored = now.template
const target = restored.cells.find(cell => cell.row === 5 && cell.col === 3)
if (originalC5?.expr !== undefined) target.expr = originalC5.expr
else delete target.expr
if (originalC5?.text !== undefined) target.text = originalC5.text
const put = await page.request.put(`${SERVER}/reports/${ID}/template`, {
  headers: { 'Content-Type': 'application/json', 'If-Match': now.revision },
  data: restored,
})
await page.request.post(`${SERVER}/reports/${ID}/runs`, { headers: { 'Content-Type': 'application/json' }, data: {} })

const report = {
  statusAfterType,
  canvasValue,
  statusAfterRun,
  savedC5: { expr: savedC5?.expr, text: savedC5?.text },
  originalC5: { expr: originalC5?.expr, text: originalC5?.text },
  renderedRow12: rendered,
  restoreStatus: put.status(),
  pageErrors: errors,
  pass: savedC5?.expr === NEW_FORMULA && put.status() === 200,
}
console.log(JSON.stringify(report, null, 1))
await browser.close()
