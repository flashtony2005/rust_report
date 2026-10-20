/**
 * 「点结果没反应」的回归判据：两条装载路径都必须在**画布上方**给出可见提示。
 *
 *   A. `/`（库内报表）→ 点「结果」→ 绿色提示「已重新生成结果 · run-…」
 *   B. `/?gallery=…`（示例深链，无库内 id）→ 按钮**不能是禁用**，点了要出红色说明
 *
 * 用法：node result_button_feedback_e2e.mjs <playwright 模块> <输出目录>
 */
import fs from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3] ?? 'F:/project/report/rpx/tests/fixtures/out/gallery_shots'
const galleryBase = '/report-gallery/run-1792474103732136600/store-ranking'

const browser = await chromium.launch()
const report = {}

const visit = async (url, key) => {
  const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
  const errors = []
  page.on('pageerror', error => errors.push(String(error)))
  await page.goto(url, { waitUntil: 'networkidle' })
  await page.waitForTimeout(3500)
  const button = page.locator('.rpx-toolbar button.tb.primary')
  const disabled = await button.isDisabled()
  await button.click({ timeout: 5000 }).catch(error => { errors.push(String(error).slice(0, 120)) })
  // 提示条应立刻出现（running 或直接 ok/error）
  await page.waitForSelector('.rpx-runbar', { timeout: 10000 }).catch(() => {})
  const runningText = (await page.locator('.rpx-runbar').first().textContent().catch(() => null))?.trim()
  // 等最终态
  await page.waitForFunction(() => {
    const bar = document.querySelector('.rpx-runbar')
    return !!bar && !bar.classList.contains('running')
  }, { timeout: 180000 }).catch(() => {})
  await page.waitForTimeout(600)
  const bar = page.locator('.rpx-runbar').first()
  const state = {
    url,
    buttonDisabled: disabled,
    runningText,
    barClass: await bar.getAttribute('class').catch(() => null),
    barText: (await bar.textContent().catch(() => null))?.trim(),
    status: (await page.locator('.rpx-status .msg').first().textContent().catch(() => null))?.trim(),
    errors,
  }
  await page.screenshot({ path: `${outDir}/${key}.png` })
  report[key] = state
  await page.close()
}

await visit('http://127.0.0.1:5288/', 'runbar_ok')
await visit(`http://127.0.0.1:5288/?gallery=${galleryBase}`, 'runbar_error')

report.pass =
  report.runbar_ok.buttonDisabled === false
  && String(report.runbar_ok.barClass).includes('ok')
  && String(report.runbar_ok.barText).includes('已重新生成结果')
  && report.runbar_error.buttonDisabled === false
  && String(report.runbar_error.barClass).includes('error')
  && String(report.runbar_error.barText).includes('库内 id')
  && report.runbar_ok.errors.length === 0
  && report.runbar_error.errors.length === 0

fs.mkdirSync(outDir, { recursive: true })
console.log(JSON.stringify(report, null, 1))
await browser.close()
