/**
 * 深链「不再让人选第二次」的判据：
 *   1. `?gallery=<base>` —— 取路径最后一段当 id，库里有同名报表就**直接选中**；
 *   2. `?report=<id>` —— 显式指定库内报表（外部传参用）；
 *   3. `?report=<不存在>` —— 明确报错，不静默留空。
 *
 * 用法：node deep_link_select_e2e.mjs <playwright 模块> <输出目录>
 */
import fs from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3] ?? 'F:/project/report/rpx/tests/fixtures/out/gallery_shots'
const galleryBase = '/report-gallery/run-1792474103732136600/store-ranking'

const browser = await chromium.launch()
const report = {}

const visit = async (query, key) => {
  const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
  const errors = []
  page.on('pageerror', error => errors.push(String(error)))
  await page.goto(`http://127.0.0.1:5288/${query}`, { waitUntil: 'networkidle' })
  // 等「报表库」下拉被填上并选中
  await page.waitForFunction(() => {
    const select = document.querySelector('select[aria-label="报表库"]')
    return !!select && select.value.length > 0
  }, { timeout: 20000 }).catch(() => {})
  await page.waitForTimeout(1500)
  const state = {
    value: await page.$eval('select[aria-label="报表库"]', select => select.value).catch(() => null),
    optionCount: await page.locator('select[aria-label="报表库"] option').count(),
    status: (await page.locator('.rpx-status .msg').first().textContent().catch(() => null))?.trim(),
    errors,
  }
  report[key] = state
  await page.screenshot({ path: `${outDir}/${key}.png` })
  await page.close()
}

await visit(`?gallery=${galleryBase}`, 'deeplink_gallery')
await visit('?report=budget-variance', 'deeplink_report')
await visit('?report=no-such-report', 'deeplink_missing')

report.pass =
  report.deeplink_gallery.value === 'store-ranking'
  && report.deeplink_report.value === 'budget-variance'
  && String(report.deeplink_missing.status).includes('没有 id 为 no-such-report')
  && ['deeplink_gallery', 'deeplink_report', 'deeplink_missing'].every(key => report[key].errors.length === 0)

fs.mkdirSync(outDir, { recursive: true })
console.log(JSON.stringify(report, null, 1))
await browser.close()
