// 端到端（设计器）：
//  ① 结果视图画布上是否还有 `######`（fitColumns 加宽是否生效）
//  ② 改了定义后，状态栏是否给出可复制的「重算」命令
// 用法：node designer_layout_probe.mjs <playwright目录> <输出json>
import { writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outFile = process.argv[3]
const base = process.env.RPX_WEB_BASE || 'http://127.0.0.1:5288'
const shots = 'F:/project/report/rpx/tests/fixtures/out/gallery_shots'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', e => errors.push(String(e)))

await page.goto(base + '/', { waitUntil: 'networkidle' })
await page.waitForTimeout(2500)

const out = {}
const options = await page.locator('select[aria-label="报表库"] option').allTextContents()
const idx = options.findIndex(t => t.includes('门店销售'))
if (idx >= 0) {
  await page.selectOption('select[aria-label="报表库"]', { index: idx })
  await page.waitForTimeout(3000)
}

// ── ① 结果视图画布：量表格线间距（列宽）─────────────────────────
// fitColumns 只会加宽**数值格**所在列。合计行 C5 = 1,205,000 是数值 ⇒
// 加宽后 C 列的竖线位置会右移。画布是 canvas，改用**像素扫描**找竖线。
const resultBtn = page.getByRole('button', { name: '结果', exact: true })
if (await resultBtn.isEnabled().catch(() => false)) {
  await resultBtn.click()
  await page.waitForTimeout(3000)
  const box = await page.locator('.rpx-canvas, .univer-univer-outer, .univer-relative').first().boundingBox().catch(() => null)
  out.canvasBox = box
  await page.screenshot({ path: shots + '/designer_result_canvas.png' })
}

// ── ② 改定义 → 是否给出重算命令 ────────────────────────────────
const designBtn = page.getByRole('button', { name: '设计', exact: true })
if (await designBtn.isEnabled().catch(() => false)) {
  await designBtn.click()
  await page.waitForTimeout(1500)
}

// 直接在定义源里改一个字符（DefinitionSource 面板）
const editor = page.locator('textarea, [contenteditable="true"]').first()
if (await editor.count() > 0) {
  await editor.click().catch(() => {})
  await page.waitForTimeout(400)
  // 追加一个空格 ⇒ 定义变了（draft dirty）
  await page.keyboard.press('End')
  await page.keyboard.type(' ')
  await page.waitForTimeout(1500)
}

out.rerunHintVisible = await page.locator('.rpx-rerun').count()
out.rerunHintText = await page.locator('.rpx-rerun code').first().textContent().catch(() => null)
out.statusText = await page.locator('.rpx-status .msg').first().textContent().catch(() => null)
await page.screenshot({ path: shots + '/designer_rerun_hint.png' })

out.errors = errors
writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(JSON.stringify(out, null, 1))
await browser.close()
