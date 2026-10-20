// 决定性探针 v2：二维扫描（x × y）找列头分隔线，判定只读态能否拖列宽。
// v1 的教训：y 取 box.y+12 落在公式栏（y≈296-301 有 24x24 的 ✕✓ 按钮），
// 必须在整个 sheet 区域做二维扫描。
// 用法：node drag_scan2.mjs <playwright目录> <输出json>
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

// 所有 canvas 的位置（分清主画布与列头/行头画布）
const canvases = await page.evaluate(() => {
  const el = document.querySelector('.gallery-sheet')
  return [...el.querySelectorAll('canvas')].map(c => {
    const r = c.getBoundingClientRect()
    return { x: r.x, y: r.y, w: r.width, h: r.height }
  })
})

const out = { box, canvases, hits: [] }
const base_ = await readWidths()
out.before = base_

outer:
for (let y = box.y + 20; y < box.y + 120; y += 6) {
  for (let x = box.x + 40; x < box.x + 200; x += 4) {
    const before = await readWidths()
    await page.mouse.move(x, y)
    await page.mouse.down()
    await page.mouse.move(x + 40, y, { steps: 6 })
    await page.mouse.up()
    await page.waitForTimeout(60)
    const after = await readWidths()
    if (JSON.stringify(after) !== JSON.stringify(before)) {
      out.hits.push({ x, y, before, after })
      if (out.hits.length >= 3) break outer
    }
  }
}

out.result = out.hits.length ? '列宽可拖（只读态也生效）' : '二维扫描全部未改变活态列宽 ⇒ 只读态禁止拖列宽'
writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log('canvases:', JSON.stringify(out.canvases))
console.log('hits:', out.hits.length, out.result)
if (out.hits.length) console.log(JSON.stringify(out.hits[0], null, 1))
await browser.close()
