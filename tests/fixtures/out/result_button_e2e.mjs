/**
 * E2E：「结果」按钮必须**真的调引擎**（`POST /reports/{id}/runs`），而不是读旧快照。
 *
 * 判据三条：
 * 1. 点完按钮后，服务端的运行记录条数 +1；
 * 2. 状态栏出现「已重新生成结果（运行 <run_id>）」；
 * 3. 新 run 的 workbook.json 能被画布装载（无报错）。
 *
 * 用法：node result_button_e2e.mjs <playwright 模块路径> <输出目录>
 */
import fs from 'node:fs'
import { createRequire } from 'node:module'

const [, , playwrightPath, outDir] = process.argv
// Windows 下动态 import 绝对路径会报 ERR_UNSUPPORTED_ESM_URL_SCHEME（要 file:// URL），
// playwright 本身是 CJS，用 createRequire 最省事。
const require = createRequire(import.meta.url)
const { chromium } = require(playwrightPath)
const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', error => errors.push(String(error)))

const api = async path => {
  const response = await page.request.get(`http://127.0.0.1:8090${path}`)
  return await response.json()
}

await page.goto('http://127.0.0.1:5288/', { waitUntil: 'domcontentloaded' })
await page.waitForSelector('.rpx-context select', { timeout: 30000 })
// 等模板库装载完（select 的 value 变成具体 id）
await page.waitForFunction(() => {
  const select = document.querySelector('.rpx-context select')
  return select && select.value.length > 0
}, { timeout: 30000 })

const id = await page.$eval('.rpx-context select', select => select.value)
const before = await api(`/reports/${encodeURIComponent(id)}/runs?limit=500`)
const button = page.locator('.rpx-toolbar button.tb.primary')
const label = await button.textContent()

await button.click()
// 引擎同步回包：等状态栏出结果
await page.waitForFunction(() => {
  const text = document.querySelector('.rpx-status .msg')?.textContent ?? ''
  return text.includes('已重新生成结果') || text.includes('重新生成失败')
}, { timeout: 180000 })

const status = await page.$eval('.rpx-status .msg', element => element.textContent ?? '')
const after = await api(`/reports/${encodeURIComponent(id)}/runs?limit=500`)
const detail = await api(`/reports/${encodeURIComponent(id)}`)
await page.waitForTimeout(1500)
fs.mkdirSync(outDir, { recursive: true })
await page.screenshot({ path: `${outDir}/result_button_after_run.png` })

const report = {
  id,
  buttonLabel: label?.trim(),
  status: status.trim(),
  runsBefore: before.items.length,
  runsAfter: after.items.length,
  resultState: detail.result_state,
  displayRunId: detail.display_run_id,
  pageErrors: errors,
  pass: after.items.length === before.items.length + 1
    && status.includes('已重新生成结果')
    && detail.result_state === 'current_revision'
    && errors.length === 0,
}
console.log(JSON.stringify(report, null, 1))
await browser.close()
