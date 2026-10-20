// 实测：画廊「报表结果」画布上拖拽到底改没改东西，打印预览跟不跟。
// 用法：node layout_probe.mjs <playwright目录> <输出json>
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

const out = {}

// 1) 画布区是否有列宽拖拽把手（Univer 的列头右缘）
const canvasBox = await page.locator('.gallery-sheet').first().boundingBox()
out.canvasBox = canvasBox

// 量一量画布上表头的列分隔线位置（用画布像素的暗色竖线推断太脆，
// 改用 Univer 自身的 DOM：列头是 canvas，但数据区有 aria/role 提示）
// 这里直接测「画布能否响应拖拽」：按住列头右缘拖 60px，看画布内容是否变化。
const before = await page.screenshot({ clip: canvasBox })

// 找列头高度带：表头行大约在画布顶部往下 20-40px
const headY = canvasBox.y + 30
// 列 B 右缘大约在画布左侧 200px 处（从截图量过：A 列约 100，B 列约 100）
for (const [label, x] of [['colA_right', canvasBox.x + 100], ['colB_right', canvasBox.x + 200]]) {
  const b2 = await page.screenshot({ clip: canvasBox })
  await page.mouse.move(x, headY)
  await page.mouse.down()
  await page.mouse.move(x + 60, headY, { steps: 12 })
  await page.mouse.up()
  await page.waitForTimeout(600)
  const after = await page.screenshot({ clip: canvasBox })
  out[label] = { changed: !before.equals(after) }
  out[`${label}_after`] = after.length
}

// 2) 打开打印预览，读它用的列宽
await page.getByRole('button', { name: '打印预览' }).click()
await page.waitForSelector('.rpx-print-preview', { timeout: 20000 })
await page.waitForTimeout(1200)
out.preview = await page.evaluate(() => {
  const page1 = document.querySelector('.rpx-print-page')
  const cols = [...(page1?.querySelectorAll('colgroup col') ?? [])].map(c => c.style.width)
  const table = page1?.querySelector('table')
  return { colWidths: cols, tableWidth: table?.style.width ?? null, pageCount: document.querySelectorAll('.rpx-print-page').length }
})

out.errors = errors
writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(JSON.stringify(out, null, 1))
await browser.close()
