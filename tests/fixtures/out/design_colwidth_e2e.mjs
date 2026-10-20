/**
 * 「设计器不能拖列宽」的回归判据（含行高、含还原）。
 *
 * 判据三层：
 *   1. 设计视图拖列头分隔线 → 活态列宽变化，状态栏出「已更新列宽 … mm」；
 *   2. 行高走同一套 mutation（`sheet.mutation.set-worksheet-row-height`）→ 也进定义；
 *   3. 点「结果」重算后，**服务端新 run 的 workbook** 列宽/行高跟着变（服务端快照是权威）。
 *
 * ⚠ 会真写库：脚本末尾把 `col_props`/`row_props` 还原并重算一次，示例库不留测试数据。
 *
 * 用法：node design_colwidth_e2e.mjs <playwright 模块> <输出目录>
 */
import fs from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3] ?? 'F:/project/report/rpx/tests/fixtures/out/gallery_shots'
const ID = 'store-ranking'
const SERVER = 'http://127.0.0.1:8090'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', error => errors.push(String(error)))

const api = async path => (await page.request.get(`${SERVER}${path}`)).json()
const original = (await api(`/reports/${ID}`)).template

await page.goto('http://127.0.0.1:5288/', { waitUntil: 'networkidle' })
await page.waitForTimeout(3000)
await page.selectOption('select[aria-label="报表库"]', ID)
await page.waitForTimeout(3000)
await page.getByRole('button', { name: '设计', exact: true }).click()
// Univer 生命周期到 Steady 才放开 pointer-events（实测约 2-3s）
await page.waitForFunction(() => document.querySelector('.rpx-canvas')?.getAttribute('data-sheet-ready') === 'true', { timeout: 20000 })

const box = await page.locator('.rpx-canvas').first().boundingBox()
const readCol = () => page.evaluate(() => window.__rpxProbeApi.getActiveWorkbook().getActiveSheet().getColumnWidth(0))
const colBefore = await readCol()

// 1px 扫描找列头分隔线：拖动 30px，活态列宽一变就是命中了
let hit = null
outer: for (const dy of [10, 18, 26, 34, 42, 50]) {
  for (let dx = 100; dx <= 260; dx += 1) {
    await page.mouse.move(box.x + dx, box.y + dy)
    await page.mouse.down()
    await page.mouse.move(box.x + dx + 30, box.y + dy, { steps: 6 })
    await page.mouse.up()
    const width = await readCol()
    if (Math.abs(width - colBefore) > 1) { hit = { dx, dy, width }; break outer }
  }
}
const colStatus = (await page.locator('.rpx-status .msg').first().textContent())?.trim()

// 行高：同一 mutation 通道（`set-worksheet-row-height`），用 API 精准触发，免得再扫几何
const rowBefore = await page.evaluate(() => window.__rpxProbeApi.getActiveWorkbook().getActiveSheet().getRowHeight(0))
await page.evaluate(() => window.__rpxProbeApi.getActiveWorkbook().getActiveSheet().setRowHeight(0, 60))
await page.waitForTimeout(600)
const rowAfter = await page.evaluate(() => window.__rpxProbeApi.getActiveWorkbook().getActiveSheet().getRowHeight(0))
const rowStatus = (await page.locator('.rpx-status .msg').first().textContent())?.trim()

// 点「结果」：自动保存 def（含新列宽/行高）→ 重算 → 读服务端权威快照
await page.locator('.rpx-toolbar button.tb.primary').click()
await page.waitForFunction(() => {
  const bar = document.querySelector('.rpx-runbar')
  return !!bar && !bar.classList.contains('running')
}, { timeout: 180000 })
await page.waitForTimeout(1000)

const detail = await api(`/reports/${ID}`)
const workbook = await (await page.request.get(`${SERVER}/reports/${ID}/runs/${detail.display_run_id}/artifacts/workbook.json`)).json()
const sheet = workbook.sheets[workbook.sheetOrder[0]]
const runCol = sheet.columnData?.['0']?.w ?? sheet.defaultColumnWidth ?? null
const runRow = sheet.rowData?.['0']?.h ?? sheet.defaultRowHeight ?? null

fs.mkdirSync(outDir, { recursive: true })
await page.screenshot({ path: `${outDir}/design_colwidth.png` })

// ── 还原 ────────────────────────────────────────────────────────────────
const now = await api(`/reports/${ID}`)
const restored = now.template
restored.col_props = original.col_props
restored.row_props = original.row_props
const put = await page.request.put(`${SERVER}/reports/${ID}/template`, {
  headers: { 'Content-Type': 'application/json', 'If-Match': now.revision },
  data: restored,
})
await page.request.post(`${SERVER}/reports/${ID}/runs`, { headers: { 'Content-Type': 'application/json' }, data: {} })
const final = await api(`/reports/${ID}`)

const report = {
  colBefore,
  hit,
  colStatus,
  rowBefore,
  rowAfter,
  rowStatus,
  runCol,
  runRow,
  restoreStatus: put.status(),
  finalColProps: final.template?.col_props,
  finalState: final.result_state,
  pageErrors: errors,
  pass: !!hit
    && String(colStatus).includes('已更新列宽')
    && String(rowStatus).includes('已更新行高')
    && typeof runCol === 'number' && Math.abs(runCol - hit.width) < 2
    && typeof runRow === 'number' && Math.abs(runRow - 60) < 2
    && put.status() === 200
    && JSON.stringify(final.template?.col_props) === JSON.stringify(original.col_props)
    && errors.length === 0,
}
console.log(JSON.stringify(report, null, 1))
await browser.close()
