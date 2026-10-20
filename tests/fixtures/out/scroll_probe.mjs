// 枚举画廊页里所有可滚动元素（scrollHeight > clientHeight），用于滚动条样式定点。
// 用法：node scroll_probe.mjs <playwright目录> <输出json>
import { writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outFile = process.argv[3]
const base = process.env.RPX_WEB_BASE || 'http://127.0.0.1:5288'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
await page.goto(base + '/gallery.html', { waitUntil: 'networkidle' })
await page.waitForTimeout(1500)

const found = await page.evaluate(() => {
  const out = []
  document.querySelectorAll('*').forEach((el) => {
    const cs = getComputedStyle(el)
    const scrolls = el.scrollHeight > el.clientHeight + 1 || el.scrollWidth > el.clientWidth + 1
    if (!scrolls) return
    out.push({
      tag: el.tagName.toLowerCase(),
      cls: el.className && el.className.toString ? el.className.toString().slice(0, 70) : '',
      overflowY: cs.overflowY,
      overflowX: cs.overflowX,
      sh: el.scrollHeight,
      ch: el.clientHeight,
      sw: el.scrollWidth,
      cw: el.clientWidth,
    })
  })
  return out
})
writeFileSync(outFile, JSON.stringify(found, null, 1), 'utf-8')
console.log('scrollable elements:', found.length)
for (const f of found) console.log(` ${f.tag}.${f.cls}  oy=${f.overflowY} ox=${f.overflowX} ${f.sw}x${f.sh} / ${f.cw}x${f.ch}`)
await browser.close()
