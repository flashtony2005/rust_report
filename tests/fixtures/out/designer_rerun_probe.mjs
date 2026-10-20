// 端到端（设计器）：走**真实编辑路径**（右侧单元格面板 → 应用单元格定义），
// 验证状态栏是否给出可复制的「重算」命令。
// 用法：node designer_rerun_probe.mjs <playwright目录> <输出json>
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
await page.selectOption('select[aria-label="报表库"]', { index: idx })
await page.waitForTimeout(3000)

// 设计视图
await page.getByRole('button', { name: '设计', exact: true }).click()
await page.waitForTimeout(1500)

// 选中 A2（门店列的第一个数据格）⇒ 右侧属性面板出单元格页签
const canvas = page.locator('canvas').first()
const cbox = await canvas.boundingBox()
if (cbox) {
  await page.mouse.click(cbox.x + 60, cbox.y + 60)
  await page.waitForTimeout(1200)
}

// 右侧「单元格内容」输入框改一个字
const cellInput = page.locator('input[aria-label="单元格内容"], textarea[aria-label="单元格内容"]').first()
out.hasCellInput = await cellInput.count()
if (out.hasCellInput > 0) {
  await cellInput.fill('门店测试名')
  await page.waitForTimeout(800)
  const applyBtn = page.getByRole('button', { name: '应用单元格定义' })
  out.hasApply = await applyBtn.count()
  if (out.hasApply > 0) {
    await applyBtn.click()
    await page.waitForTimeout(1500)
  }
}

out.rerunHintVisible = await page.locator('.rpx-rerun').count()
out.rerunHintText = await page.locator('.rpx-rerun code').first().textContent().catch(() => null)
out.statusText = await page.locator('.rpx-status .msg').first().textContent().catch(() => null)
await page.screenshot({ path: shots + '/designer_rerun_hint.png' })

out.errors = errors
writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(JSON.stringify(out, null, 1))
await browser.close()
