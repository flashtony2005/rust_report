// 诊断：拖列宽时 Univer 到底派发了哪些 CommandExecuted 事件。
// 用法：node cmd_event_probe.mjs <playwright目录> <输出json>
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

await page.evaluate(() => {
  window.__rpxEvents = []
  const el = document.querySelector('.gallery-sheet')
  // 找不到 api 就没法加监听；用 MutationObserver 不行（画布是 canvas）。
  // 改从 React 侧拿不到 —— 直接挂全局错误兜底。
})

// 通过页面里的探针 api 挂监听
const hooked = await page.evaluate(() => {
  const api = window.__rpxProbeApi
  if (!api) return 'no api'
  if (window.__rpxHooked) return 'already'
  window.__rpxHooked = true
  window.__rpxEvents = []
  const d = api.addEvent(api.Event.CommandExecuted, e => {
    window.__rpxEvents.push({ id: e.id, params: e.params ? Object.keys(e.params) : null })
  })
  window.__rpxDispose = d
  return 'hooked'
})

const x = 358
const y = 326.59375
await page.mouse.move(x, y)
await page.mouse.down()
await page.mouse.move(x + 40, y, { steps: 15 })
await page.mouse.up()
await page.waitForTimeout(1200)

const events = await page.evaluate(() => window.__rpxEvents || [])
const out = { hooked, count: events.length, ids: [...new Set(events.map(e => e.id))], sample: events.slice(0, 12) }
writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(JSON.stringify(out, null, 1))
await browser.close()
