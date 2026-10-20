// 找出 DOM 里所有带 scrollbar 语义的 class，以及 Univer 内部滚动容器的真实 class。
// 用法：node scroll_probe3.mjs <playwright目录>
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const base = process.env.RPX_WEB_BASE || 'http://127.0.0.1:5288'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
await page.goto(base + '/gallery.html#store-ranking', { waitUntil: 'networkidle' })
await page.waitForSelector('.gallery-sheet canvas', { timeout: 30000 })
await page.waitForTimeout(2500)

const out = await page.evaluate(() => {
  const res = { scrollbarClasses: [], univerScrollables: [] }
  const seen = new Set()
  document.querySelectorAll('*').forEach((el) => {
    const cls = el.className && el.className.toString ? el.className.toString() : ''
    if (!cls) return
    if (/scroll/i.test(cls) && !seen.has(cls)) { seen.add(cls); res.scrollbarClasses.push(cls) }
    if (/univer/.test(cls) && (el.scrollHeight > el.clientHeight + 1 || el.scrollWidth > el.clientWidth + 1)) {
      const cs = getComputedStyle(el)
      res.univerScrollables.push({
        cls, oy: cs.overflowY, ox: cs.overflowX,
        sw: el.scrollWidth, cw: el.clientWidth, sh: el.scrollHeight, ch: el.clientHeight,
        sbw: cs.scrollbarWidth, sbcolor: cs.scrollbarColor,
      })
    }
  })
  return res
})
console.log('--- 含 scroll 语义的 class:')
for (const c of out.scrollbarClasses) console.log('  ', c)
console.log('--- Univer 内部可滚动元素:')
for (const s of out.univerScrollables) console.log('  ', JSON.stringify(s))
await browser.close()
