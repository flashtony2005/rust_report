// 出图：结果画布上画边框 → 打印预览同位置也有边框（供人工核对）。
import fs from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3]
const COLOR = '#C00000'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
try {
  await page.goto('http://127.0.0.1:5288/', { waitUntil: 'networkidle' })
  await page.waitForTimeout(2500)
  await page.selectOption('select[aria-label="报表库"]', 'store-ranking')
  await page.waitForTimeout(3000)
  const tab = page.locator('button[aria-pressed]', { hasText: '结果' }).first()
  if (await tab.count()) await tab.click()
  await page.waitForFunction(
    () => {
      const bar = document.querySelector('.rpx-runbar')
      return !bar || !bar.classList.contains('running')
    },
    { timeout: 120000 },
  )
  await page.waitForFunction(
    () => document.querySelector('.rpx-canvas')?.getAttribute('data-sheet-ready') === 'true',
    { timeout: 30000 },
  )
  await page.waitForTimeout(1500)

  // 数据行上画一圈框（华南那 3 行 × 前 4 列），便于一眼看出「画布 → 预览」一致
  await page.evaluate(async color => {
    const api = window.__rpxProbeApi
    const workbook = api.getActiveWorkbook()
    const sheet = workbook.getActiveSheet()
    await api.executeCommand('sheet.command.set-border-basic', {
      unitId: workbook.getId(),
      subUnitId: sheet.getSheetId(),
      value: { type: 'outside', color, style: 13 },
      ranges: [{ startRow: 3, endRow: 5, startColumn: 0, endColumn: 3 }],
    })
    await new Promise(resolve => setTimeout(resolve, 600))
  }, COLOR)

  fs.mkdirSync(outDir, { recursive: true })
  await page.screenshot({ path: `${outDir}/border_canvas.png` })

  await page.locator('.rpx-toolbar button', { hasText: '打印预览' }).first().click()
  await page.waitForSelector('.rpx-print-page', { timeout: 30000 })
  await page.waitForTimeout(1200)
  await page.screenshot({ path: `${outDir}/border_preview.png` })
  console.log('shots ok')
} finally {
  await browser.close()
}
