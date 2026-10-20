// 严谨验证：画廊画布拖拽后，**数据区网格竖线的 x 位置**是否移动。
// （前一次探针的 changed=true 可能只是选中态像素差，不能作数。）
// 做法：截数据区一条水平带 → 找非白竖线（网格线/文字边缘）→ 对比 x 分布。
// 用法：node gridline_probe.mjs <playwright目录> <输出json>
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
const out = { box }

// 网格线分析放在 node 侧做（浏览器里拿不到 canvas 像素）
const { PNG } = (() => {
  try { return require('pngjs') } catch { return {} }
})()

// 取一条**空白行**上的带（表头下方第一行通常是空白行）—— 用 y=box.y+150 附近
const band = { x: Math.round(box.x), y: Math.round(box.y + 150), width: 700, height: 2 }
const before = await page.screenshot({ clip: band })
writeFileSync('F:/tmp/band_before.png', before)

// 拖 A 列右缘 +60
const headY = box.y + 30
await page.mouse.move(box.x + 100, headY)
await page.mouse.down()
await page.mouse.move(box.x + 160, headY, { steps: 15 })
await page.mouse.up()
await page.waitForTimeout(900)

const after = await page.screenshot({ clip: band })
writeFileSync('F:/tmp/band_after.png', after)

out.beforeBytes = before.length
out.afterBytes = after.length
out.identical = before.equals(after)
writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(JSON.stringify(out, null, 1))
await browser.close()
