// 端到端验收：画廊「报表结果」拖列宽 → 打印预览的列宽跟着变。
// 步骤：开预览读列宽 → 关 → 拖 A 列右缘 +40px → 再开预览读列宽 → 比对。
// 用法：node print_layout_e2e.mjs <playwright目录> <输出json>
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

await page.goto(base + '/gallery.html#store-ranking', { waitUntil: 'networkidle' })
await page.waitForSelector('.gallery-sheet canvas', { timeout: 30000 })
await page.waitForTimeout(2500)

const readPreview = async () => {
  await page.getByRole('button', { name: '打印预览' }).click()
  await page.waitForSelector('.rpx-print-preview', { timeout: 20000 })
  await page.waitForTimeout(900)
  const data = await page.evaluate(() => {
    const p = document.querySelector('.rpx-print-page')
    return {
      cols: [...(p?.querySelectorAll('colgroup col') ?? [])].map(c => c.style.width),
      pages: document.querySelectorAll('.rpx-print-page').length,
    }
  })
  await page.getByRole('button', { name: '关闭' }).click()
  await page.waitForTimeout(500)
  return data
}

const out = {}
out.before = await readPreview()

// 拖 A 列右缘（分隔线 x=358, y=326 —— 由 drag_scan2.mjs 二维扫描定位）
const box = await page.locator('.gallery-sheet').first().boundingBox()
const x = 358
const y = 326.59375
await page.mouse.move(x, y)
await page.mouse.down()
await page.mouse.move(x + 40, y, { steps: 15 })
await page.mouse.up()
await page.waitForTimeout(1000)

out.after = await readPreview()

out.col0Before = out.before.cols[0]
out.col0After = out.after.cols[0]
out.changed = out.col0Before !== out.col0After
out.errors = errors
writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(JSON.stringify(out, null, 1))
console.log(out.changed ? 'PASS: 预览列宽跟着画布拖拽变了' : 'FAIL: 预览没变')
await browser.close()
