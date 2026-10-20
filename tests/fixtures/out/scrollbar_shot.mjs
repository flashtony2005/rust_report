// 侧栏滚动条视觉验证：截图侧栏右缘 + 悬停态对比。
// 用法：node scrollbar_shot.mjs <playwright目录> <输出目录>
import { mkdirSync } from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3]
mkdirSync(outDir, { recursive: true })
const base = process.env.RPX_WEB_BASE || 'http://127.0.0.1:5288'

const browser = await chromium.launch({ headless: process.env.RPX_HEADED ? false : true })
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 }, deviceScaleFactor: 2 })
const errors = []
page.on('pageerror', e => errors.push(String(e)))

await page.goto(base + '/gallery.html#pareto-contribution', { waitUntil: 'networkidle' })
await page.waitForTimeout(2000)

// 侧栏几何：滚动容器 + 滚动条占位
const geo = await page.evaluate(() => {
  const el = document.querySelector('aside.gallery-sidebar')
  if (!el) return null
  const cs = getComputedStyle(el)
  return {
    clientWidth: el.clientWidth,
    offsetWidth: el.offsetWidth,
    scrollHeight: el.scrollHeight,
    clientHeight: el.clientHeight,
    scrollbarWidth: cs.scrollbarWidth,
    scrollbarColor: cs.scrollbarColor,
  }
})
console.log('sidebar geometry:', JSON.stringify(geo))

// 侧栏滚动到中段，让滑块出现在画面中部
await page.evaluate(() => {
  const el = document.querySelector('aside.gallery-sidebar')
  if (el) el.scrollTop = Math.round(el.scrollHeight * 0.35)
})
await page.waitForTimeout(600)
await page.screenshot({ path: outDir + '/sb_normal.png', clip: { x: 190, y: 60, width: 110, height: 640 } })

// 悬停到滑块上（几何定位：滑块在滚动容器右缘）
const box = await page.locator('aside.gallery-sidebar').boundingBox()
if (box) {
  await page.mouse.move(box.x + box.width - 4, box.y + 200)
  await page.waitForTimeout(500)
  await page.screenshot({ path: outDir + '/sb_hover.png', clip: { x: 190, y: 60, width: 110, height: 640 } })
}

// 整页首屏（看顶栏 + 侧栏整体观感）
await page.screenshot({ path: outDir + '/sb_full.png' })

console.log('pageerrors=', errors.length)
await browser.close()
console.log('done')
