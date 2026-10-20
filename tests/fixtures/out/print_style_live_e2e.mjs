/**
 * 「结果里改样式 → 打印预览要跟着变」的判据。
 *
 * 走用户的真实路径：设计器 → 点「结果」→ 在结果画布上给某格加粗 → 打印预览里
 * 同一格必须变成粗体（此前预览只吃静态快照，永远看不到这类改动）。
 *
 * 用法：node print_style_live_e2e.mjs <playwright 模块> <输出目录>
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
await page.selectOption('select[aria-label="报表库"]', 'store-ranking')
await page.waitForTimeout(2500)
await page.locator('.rpx-toolbar button.tb.primary').click()
await page.waitForFunction(() => {
  const bar = document.querySelector('.rpx-runbar')
  return !!bar && !bar.classList.contains('running')
}, { timeout: 120000 })
await page.waitForTimeout(2000)

/** 打开预览、读第 row 行第 col 列单元格的字重，然后关闭预览。 */
const weightOf = async (row, col, shot) => {
  await page.locator('.rpx-toolbar button', { hasText: '打印预览' }).first().click()
  await page.waitForSelector('.rpx-print-page', { timeout: 30000 })
  await page.waitForTimeout(800)
  const weight = await page.evaluate(({ row, col }) => {
    const tr = document.querySelectorAll('.rpx-print-page tr')[row]
    const td = tr?.children[col]
    return td ? getComputedStyle(td).fontWeight : null
  }, { row, col })
  if (shot) { fs.mkdirSync(outDir, { recursive: true }); await page.screenshot({ path: `${outDir}/${shot}` }) }
  await page.locator('.rpx-print-controls button', { hasText: '关闭' }).first().click()
  await page.waitForTimeout(500)
  return weight
}

const before = await weightOf(6, 0)

// 在结果画布上给 (6,0) 加粗（只读只锁数据编辑，格式命令仍可执行）
await page.evaluate(async () => {
  const api = window.__rpxProbeApi
  api.getActiveWorkbook().getActiveSheet().getRange(6, 0).activate()
  await api.executeCommand('sheet.command.set-bold', { value: true })
})
await page.waitForTimeout(1000)

const after = await weightOf(6, 0, 'print_style_live.png')

const report = { before, after, pageErrors: errors, pass: before === '400' && after === '700' && errors.length === 0 }
fs.mkdirSync(outDir, { recursive: true })
console.log(JSON.stringify(report, null, 1))
await browser.close()
