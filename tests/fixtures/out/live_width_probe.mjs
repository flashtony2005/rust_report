// 实测：画廊画布拖拽后，Univer 活态列宽 vs 打印预览列宽。
// 做法：把 mountUniver 出来的 api 挂到 window（仅本探针用 monkey-patch），
// 拖拽前后各读一次 save() 的 columnData。
// 用法：node live_width_probe.mjs <playwright目录> <输出json>
import { writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outFile = process.argv[3]
const base = process.env.RPX_WEB_BASE || 'http://127.0.0.1:5288'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', e => errors.push(String(e)))

// 在页面脚本执行前注入：拦截 mountUniver 不可行（模块内），改为
// 直接从 Univer 的全局实例拿不到 —— 改走「DOM 事件 + save()」的替代方案：
// 用 Univer 渲染出的 DOM（列头是 canvas）读不到宽度，
// 所以这里改为观察打印预览的列宽与画布的**像素宽度**是否一致。
await page.goto(base + '/gallery.html#store-ranking', { waitUntil: 'networkidle' })
await page.waitForSelector('.gallery-sheet canvas', { timeout: 30000 })
await page.waitForTimeout(2500)

const out = {}
const box = await page.locator('.gallery-sheet').first().boundingBox()
out.box = box

// 1) 打开预览读列宽（拖拽前）
await page.getByRole('button', { name: '打印预览' }).click()
await page.waitForSelector('.rpx-print-preview', { timeout: 20000 })
await page.waitForTimeout(1000)
out.previewBefore = await page.evaluate(() => {
  const p = document.querySelector('.rpx-print-page')
  return {
    cols: [...(p?.querySelectorAll('colgroup col') ?? [])].map(c => c.style.width),
    pages: document.querySelectorAll('.rpx-print-page').length,
  }
})
await page.getByRole('button', { name: '关闭' }).click()
await page.waitForTimeout(500)

// 2) 画布上拖列宽（A 列右缘 +60px）
const headY = box.y + 30
const x = box.x + 100
await page.mouse.move(x, headY)
await page.mouse.down()
await page.mouse.move(x + 60, headY, { steps: 15 })
await page.mouse.up()
await page.waitForTimeout(900)

// 3) 再开预览读列宽（拖拽后）
await page.getByRole('button', { name: '打印预览' }).click()
await page.waitForSelector('.rpx-print-preview', { timeout: 20000 })
await page.waitForTimeout(1000)
out.previewAfter = await page.evaluate(() => {
  const p = document.querySelector('.rpx-print-page')
  return {
    cols: [...(p?.querySelectorAll('colgroup col') ?? [])].map(c => c.style.width),
    pages: document.querySelectorAll('.rpx-print-page').length,
  }
})

out.same = JSON.stringify(out.previewBefore) === JSON.stringify(out.previewAfter)
out.errors = errors
writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(JSON.stringify(out, null, 1))
await browser.close()
