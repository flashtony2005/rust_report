// 权威判定：读 Univer 活态列宽（api.getActiveWorkbook().save()），
// 对比拖拽前 / 拖拽后，再对比打印预览用的列宽。
// 用法：node live_state_probe.mjs <playwright目录> <输出json>
import { writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outFile = process.argv[3]
const base = process.env.RPX_WEB_BASE || 'http://127.0.0.1:5288'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', e => errors.push(String(e)))

await page.goto(base + '/gallery.html#store-ranking', { waitUntil: 'networkidle' })
await page.waitForSelector('.gallery-sheet canvas', { timeout: 30000 })
await page.waitForTimeout(2500)

const readLive = () => page.evaluate(() => {
  const api = window.__rpxProbeApi
  if (!api) return { error: 'probe api missing' }
  const wb = api.getActiveWorkbook?.()
  const snap = wb?.save?.()
  if (!snap) return { error: 'save() returned nothing' }
  const sheetId = snap.sheetOrder?.[0]
  const sheet = snap.sheets?.[sheetId]
  const colData = sheet?.columnData ?? {}
  const widths = {}
  for (const [k, v] of Object.entries(colData)) widths[k] = v?.w
  return {
    sheetId,
    widths,
    defaultColumnWidth: sheet?.defaultColumnWidth,
    editable: wb?.getActiveSheet?.()?.getWorksheetPermission?.()?.editable ?? 'n/a',
  }
})

const out = {}
out.liveBefore = await readLive()

const box = await page.locator('.gallery-sheet').first().boundingBox()
const headY = box.y + 30
const x = box.x + 100
await page.mouse.move(x, headY)
await page.mouse.down()
await page.mouse.move(x + 60, headY, { steps: 15 })
await page.mouse.up()
await page.waitForTimeout(900)

out.liveAfter = await readLive()

// 打印预览的列宽
await page.getByRole('button', { name: '打印预览' }).click()
await page.waitForSelector('.rpx-print-preview', { timeout: 20000 })
await page.waitForTimeout(1000)
out.preview = await page.evaluate(() => {
  const p = document.querySelector('.rpx-print-page')
  return {
    cols: [...(p?.querySelectorAll('colgroup col') ?? [])].map(c => c.style.width),
    pages: document.querySelectorAll('.rpx-print-page').length,
  }
})

out.errors = errors
writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(JSON.stringify(out, null, 1))
await browser.close()
