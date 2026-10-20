// 进一步定位：找出报告画布区里「可见的滚动条」到底由谁渲染。
// 用法：node scroll_probe2.mjs <playwright目录> <输出json>
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

const found = await page.evaluate(() => {
  const out = []
  document.querySelectorAll('*').forEach((el) => {
    const cs = getComputedStyle(el)
    const cls = el.className && el.className.toString ? el.className.toString() : ''
    const isScrollClass = /scroll/i.test(cls)
    const scrolls = el.scrollHeight > el.clientHeight + 1
    if (!scrolls && !isScrollClass) return
    const r = el.getBoundingClientRect()
    out.push({
      tag: el.tagName.toLowerCase(),
      cls: cls.slice(0, 80),
      oy: cs.overflowY,
      sw: el.scrollWidth, cw: el.clientWidth,
      sh: el.scrollHeight, ch: el.clientHeight,
      rect: [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)],
      sbWidth: cs.scrollbarWidth,
    })
  })
  return out
})
writeFileSync(outFile, JSON.stringify(found, null, 1), 'utf-8')
console.log('candidates:', found.length)
for (const f of found) {
  console.log(` ${f.tag}.${f.cls.slice(0, 46)} oy=${f.oy} ${f.sw}x${f.sh}/${f.cw}x${f.ch} rect=${f.rect.join(',')} sbw=${f.sbWidth}`)
}
await browser.close()
