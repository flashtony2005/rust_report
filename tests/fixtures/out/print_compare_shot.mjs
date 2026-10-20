// 视觉证明：拖列宽前后的打印预览对比图。
// 用法：node print_compare_shot.mjs <playwright目录> <输出目录>
import { mkdirSync } from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3]
mkdirSync(outDir, { recursive: true })
const base = process.env.RPX_WEB_BASE || 'http://127.0.0.1:5288'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })

await page.goto(base + '/gallery.html#store-ranking', { waitUntil: 'networkidle' })
await page.waitForSelector('.gallery-sheet canvas', { timeout: 30000 })
await page.waitForTimeout(2500)

// 拖前预览
await page.getByRole('button', { name: '打印预览' }).click()
await page.waitForSelector('.rpx-print-preview', { timeout: 20000 })
await page.waitForTimeout(900)
await page.screenshot({ path: outDir + '/print_before.png', clip: { x: 380, y: 40, width: 920, height: 620 } })
await page.getByRole('button', { name: '关闭' }).click()
await page.waitForTimeout(600)

// 拖 A 列右缘 +40px
await page.mouse.move(358, 326.59375)
await page.mouse.down()
await page.mouse.move(398, 326.59375, { steps: 15 })
await page.mouse.up()
await page.waitForTimeout(1200)

// 拖后预览
await page.getByRole('button', { name: '打印预览' }).click()
await page.waitForSelector('.rpx-print-preview', { timeout: 20000 })
await page.waitForTimeout(900)
await page.screenshot({ path: outDir + '/print_after.png', clip: { x: 380, y: 40, width: 920, height: 620 } })

console.log('done')
await browser.close()
