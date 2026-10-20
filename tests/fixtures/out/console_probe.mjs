// 诊断：抓页面 console，看合并结果 col0 是什么。
// 用法：node console_probe.mjs <playwright目录> <输出json>
import { writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outFile = process.argv[3]
const base = process.env.RPX_WEB_BASE || 'http://127.0.0.1:5288'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const logs = []
page.on('console', m => logs.push(m.type() + ': ' + m.text()))
page.on('pageerror', e => logs.push('pageerror: ' + String(e)))

await page.goto(base + '/gallery.html#store-ranking', { waitUntil: 'networkidle' })
await page.waitForSelector('.gallery-sheet canvas', { timeout: 30000 })
await page.waitForTimeout(2500)
logs.push('--- drag now ---')

await page.mouse.move(358, 326.59375)
await page.mouse.down()
await page.mouse.move(398, 326.59375, { steps: 15 })
await page.mouse.up()
await page.waitForTimeout(1500)

const out = { logs: logs.filter(l => l.includes('rpx') || l.includes('error')).slice(0, 30) }
writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(out.logs.join('\n'))
await browser.close()
