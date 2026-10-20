/**
 * 复现「点结果没反应」：分别测两条装载路径下工具条「结果」按钮的状态。
 *   A. `/`（模板库首项）—— 应有 saved，按钮可用
 *   B. `/?gallery=<base>`（示例库深链）—— 若 saved 为空则按钮 disabled，点了毫无反应
 * 用法：node result_button_diag.mjs <playwright 模块> <base>
 */
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const base = process.argv[3] ?? '/report-gallery/run-1792474103732136600/store-ranking'

const browser = await chromium.launch()
const out = {}

const inspect = async (url, key) => {
  const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
  const errors = []
  const posts = []
  page.on('pageerror', error => errors.push(String(error)))
  page.on('request', request => {
    if (request.method() === 'POST') posts.push(`${request.method()} ${new URL(request.url()).pathname}`)
  })
  await page.goto(url, { waitUntil: 'networkidle' })
  await page.waitForTimeout(3500)
  const button = page.locator('.rpx-toolbar button.tb.primary')
  const state = {
    url,
    buttonText: (await button.textContent().catch(() => null))?.trim(),
    disabled: await button.isDisabled().catch(() => null),
    savedId: await page.$eval('select[aria-label="报表库"]', select => select.value).catch(() => null),
    statusBefore: (await page.locator('.rpx-status .msg').first().textContent().catch(() => null))?.trim(),
  }
  // 真点一次（disabled 时 Playwright 会等超时，所以先判）
  if (!state.disabled) {
    await button.click({ timeout: 5000 }).catch(error => { state.clickError = String(error).slice(0, 120) })
    await page.waitForTimeout(6000)
    state.statusAfter = (await page.locator('.rpx-status .msg').first().textContent().catch(() => null))?.trim()
    state.posts = posts
  } else {
    // disabled：模拟用户点击，看有没有任何反应
    await button.click({ force: true, timeout: 3000 }).catch(() => {})
    await page.waitForTimeout(1500)
    state.statusAfterForceClick = (await page.locator('.rpx-status .msg').first().textContent().catch(() => null))?.trim()
    state.posts = posts
  }
  state.errors = errors
  out[key] = state
  await page.close()
}

await inspect('http://127.0.0.1:5288/', 'library')
await inspect(`http://127.0.0.1:5288/?gallery=${base}`, 'galleryDeepLink')

console.log(JSON.stringify(out, null, 1))
await browser.close()
