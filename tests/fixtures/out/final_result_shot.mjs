/**
 * 拍一张「结果」按钮跑完之后的干净截图（示例库已还原）。
 * 用法：node final_result_shot.mjs <playwright 模块> <输出目录>
 */
import fs from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3] ?? 'F:/project/report/rpx/tests/fixtures/out/gallery_shots'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', error => errors.push(String(error)))
await page.goto('http://127.0.0.1:5288/', { waitUntil: 'networkidle' })
await page.waitForTimeout(2500)
const options = await page.locator('select[aria-label="报表库"] option').allTextContents()
const index = options.findIndex(text => text.includes('门店销售'))
await page.selectOption('select[aria-label="报表库"]', { index })
await page.waitForTimeout(2500)
await page.locator('.rpx-toolbar button.tb.primary').click()
await page.waitForFunction(() => {
  const text = document.querySelector('.rpx-status .msg')?.textContent ?? ''
  return text.includes('已重新生成结果') || text.includes('重新生成失败')
}, { timeout: 180000 })
await page.waitForTimeout(2000)
fs.mkdirSync(outDir, { recursive: true })
await page.screenshot({ path: `${outDir}/result_after_clean.png` })
const status = (await page.locator('.rpx-status .msg').first().textContent())?.trim()
console.log(JSON.stringify({ status, pageErrors: errors }, null, 1))
await browser.close()
