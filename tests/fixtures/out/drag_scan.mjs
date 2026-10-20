// 决定性探针：只读态下逐点试拖列头分隔线，找**真实分隔线屏幕坐标**，
// 并判定「拖动是否改到活态列宽」。
// 用法：node drag_scan.mjs <playwright目录> <输出json>
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

const box = await page.locator('.gallery-sheet').first().boundingBox()

const readWidths = () => page.evaluate(() => {
  const api = window.__rpxProbeApi
  const wb = api?.getActiveWorkbook?.()
  const snap = wb?.save?.()
  const sheet = snap?.sheets?.[snap?.sheetOrder?.[0]]
  const w = {}
  for (const [k, v] of Object.entries(sheet?.columnData ?? {})) w[k] = v?.w
  return w
})

// Univer 的行头宽度是固定的；列 0 起点 = 行头右缘。直接从 DOM 量行头。
const header = await page.evaluate(() => {
  const el = document.querySelector('.gallery-sheet')
  const rows = [...el.querySelectorAll('*')].filter(n => {
    const r = n.getBoundingClientRect()
    return r.width > 20 && r.width < 120 && r.height > 10 && r.height < 40 && r.left > 0
  }).slice(0, 8).map(n => {
    const r = n.getBoundingClientRect()
    return { cls: (n.className || '').toString().slice(0, 60), x: r.x, y: r.y, w: r.width, h: r.height }
  })
  return rows
})

const out = { box, header, attempts: [] }
const before = await readWidths()
out.before = before

// 逐点试拖：x 从行头右缘起，每 2px 一档，共 30 档
const startX = box.x + 40
for (let i = 0; i < 30; i++) {
  const x = startX + i * 4
  const y = box.y + 12          // 列头带
  await page.mouse.move(x, y)
  await page.mouse.down()
  await page.mouse.move(x + 40, y, { steps: 8 })
  await page.mouse.up()
  await page.waitForTimeout(120)
  const now = await readWidths()
  const changed = JSON.stringify(now) !== JSON.stringify(before)
  if (changed) {
    out.attempts.push({ x, y, changed: true, now })
    break
  }
}
if (out.attempts.length === 0) out.attempts.push({ changed: false, note: '30 档全试完，活态列宽无变化' })

writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(JSON.stringify(out, null, 1))
await browser.close()
