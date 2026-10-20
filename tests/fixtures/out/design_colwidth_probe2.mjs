/**
 * 判定「设计器不能拖列宽」到底卡在哪一层：
 *   1. API 直调 setColumnWidth 是否生效（验证 BeforeCommandExecute 的 cancel 是否连列宽一起拦）；
 *   2. 列宽命令 id / mutation 参数形状；
 *   3. 画布是否真的接收指针事件（pointer-events / sheetReady）。
 *
 * 用法：node design_colwidth_probe2.mjs <playwright 模块> <输出 json>
 */
import fs from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outFile = process.argv[3] ?? 'F:/project/report/rpx/tests/fixtures/out/design_colwidth_probe2.json'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', error => errors.push(String(error)))

await page.goto('http://127.0.0.1:5288/', { waitUntil: 'networkidle' })
await page.waitForTimeout(3000)
await page.getByRole('button', { name: '设计', exact: true }).click()
await page.waitForTimeout(2000)

const result = await page.evaluate(async () => {
  const api = window.__rpxProbeApi
  if (!api) return { error: 'no probe api' }
  const sheet = api.getActiveWorkbook().getActiveSheet()
  const log = []
  // 只旁听，不改任何行为
  api.addEvent(api.Event.BeforeCommandExecute, event => {
    if (!String(event.id).startsWith('sheet.mutation.')) return
    let params = ''
    try { params = JSON.stringify(event.params).slice(0, 500) } catch { params = '<unserializable>' }
    log.push({ id: event.id, params })
  })
  const widthBefore = sheet.getColumnWidth(0)
  let thrown = null
  try { sheet.setColumnWidth(0, widthBefore + 30) } catch (error) { thrown = String(error) }
  const widthAfter = sheet.getColumnWidth(0)
  const rowBefore = sheet.getRowHeight(0)
  try { sheet.setRowHeight(0, rowBefore + 10) } catch (error) { thrown = String(error) }
  const rowAfter = sheet.getRowHeight(0)
  const canvas = document.querySelector('.rpx-canvas')
  const style = canvas ? getComputedStyle(canvas) : null
  return {
    widthBefore, widthAfter, rowBefore, rowAfter, thrown, log: log.slice(0, 8),
    pointerEvents: style?.pointerEvents,
    sheetReady: canvas?.getAttribute('data-sheet-ready'),
    busy: canvas?.getAttribute('aria-busy'),
    editable: sheet.isSheetEditable?.(),
  }
})

// 顺便试一次精细拖拽（1px 扫描，只扫一个 y 带）
const box = await page.locator('.rpx-canvas').first().boundingBox()
let dragHit = null
if (box) {
  for (let dx = 130; dx <= 175; dx += 1) {
    await page.mouse.move(box.x + dx, box.y + 30)
    await page.mouse.down()
    await page.mouse.move(box.x + dx + 30, box.y + 30, { steps: 5 })
    await page.mouse.up()
    const width = await page.evaluate(() => window.__rpxProbeApi.getActiveWorkbook().getActiveSheet().getColumnWidth(0))
    if (Math.abs(width - (result.widthBefore ?? 0)) > 1) { dragHit = { dx, width }; break }
  }
}

const out = { ...result, box, dragHit, errors }
fs.writeFileSync(outFile, JSON.stringify(out, null, 1), 'utf-8')
console.log(JSON.stringify(out, null, 1))
await browser.close()
