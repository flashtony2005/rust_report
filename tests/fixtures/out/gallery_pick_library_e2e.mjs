/**
 * 深链模式（`?gallery=`）的完整闭环判据：
 *   1. 「报表库」下拉**必须有数据**（此前深链下空列表 —— 提示让用户去选，却没得选）；
 *   2. 选一个库内报表 → 点「结果」→ 绿色提示条「已重新生成结果 · run-…」。
 *
 * 用法：node gallery_pick_library_e2e.mjs <playwright 模块> <输出目录>
 */
import fs from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3] ?? 'F:/project/report/rpx/tests/fixtures/out/gallery_shots'
const galleryBase = '/report-gallery/run-1792474103732136600/store-ranking'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', error => errors.push(String(error)))

await page.goto(`http://127.0.0.1:5288/?gallery=${galleryBase}`, { waitUntil: 'networkidle' })
await page.waitForTimeout(3500)

const select = page.locator('select[aria-label="报表库"]')
const options = await select.locator('option').allTextContents()
const index = options.findIndex(text => text.includes('门店销售'))
const report = { optionCount: options.length, optionSample: options.slice(0, 5), pickedIndex: index }

if (index > 0) {
  await select.selectOption({ index })
  await page.waitForTimeout(3000)
  report.savedId = await select.inputValue()
  await page.locator('.rpx-toolbar button.tb.primary').click()
  await page.waitForSelector('.rpx-runbar', { timeout: 10000 }).catch(() => {})
  await page.waitForFunction(() => {
    const bar = document.querySelector('.rpx-runbar')
    return !!bar && !bar.classList.contains('running')
  }, { timeout: 180000 }).catch(() => {})
  await page.waitForTimeout(800)
  report.barClass = await page.locator('.rpx-runbar').first().getAttribute('class').catch(() => null)
  report.barText = (await page.locator('.rpx-runbar').first().textContent().catch(() => null))?.trim()
  report.status = (await page.locator('.rpx-status .msg').first().textContent().catch(() => null))?.trim()
}

fs.mkdirSync(outDir, { recursive: true })
await page.screenshot({ path: `${outDir}/gallery_pick_library.png` })
report.pageErrors = errors
report.pass = options.length > 1
  && index > 0
  && String(report.barClass).includes('ok')
  && String(report.barText).includes('已重新生成结果')
  && errors.length === 0
console.log(JSON.stringify(report, null, 1))
await browser.close()
