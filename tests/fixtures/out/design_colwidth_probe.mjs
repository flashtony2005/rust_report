/**
 * 探针：设计器「设计」视图里拖列宽，到底发了哪些命令 / mutation？
 * 目的一：确认被 BeforeCommandExecute 拦掉的是哪个 id；目的二：拿到 params 形状。
 *
 * 用法：node design_colwidth_probe.mjs <playwright 模块> <输出 json>
 */
import fs from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outFile = process.argv[3] ?? 'F:/project/report/rpx/tests/fixtures/out/design_colwidth_probe.json'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', error => errors.push(String(error)))

await page.goto('http://127.0.0.1:5288/', { waitUntil: 'networkidle' })
await page.waitForTimeout(3000)
// 选一个库内报表（首项即是）
await page.getByRole('button', { name: '设计', exact: true }).click()
await page.waitForTimeout(2000)

const hooked = await page.evaluate(() => {
  const api = window.__rpxProbeApi
  if (!api) return false
  window.__probeLog = []
  api.addEvent(api.Event.BeforeCommandExecute, event => {
    if (!String(event.id).startsWith('sheet.mutation.')) return
    let params = ''
    try { params = JSON.stringify(event.params).slice(0, 800) } catch { params = '<unserializable>' }
    window.__probeLog.push({ id: event.id, params })
  })
  return true
})

const before = await page.evaluate(() => {
  const api = window.__rpxProbeApi
  const snap = api.getActiveWorkbook().save()
  const raw = snap.sheets
  const sheet = raw instanceof Map ? raw.get(snap.sheetOrder[0]) : raw[snap.sheetOrder[0]]
  return { columnData: sheet?.columnData, rowData: sheet?.rowData }
})

const canvas = page.locator('.rpx-canvas').first()
const box = await canvas.boundingBox()
const attempts = []
let hit = null
// 二维扫描找列头分隔线：命中即记录（每次小幅拖动 24px）
outer: for (const dy of [12, 24, 40, 60, 84, 108, 132]) {
  for (let dx = 120; dx <= 320; dx += 4) {
    const x = box.x + dx
    const y = box.y + dy
    await page.evaluate(() => { window.__probeLog = [] })
    await page.mouse.move(x, y)
    await page.mouse.down()
    await page.mouse.move(x + 24, y, { steps: 4 })
    await page.mouse.up()
    const log = await page.evaluate(() => window.__probeLog)
    if (log.some(item => /column-width|row-height/.test(item.id))) {
      hit = { dx, dy, log }
      break outer
    }
    attempts.push({ dx, dy, ids: log.map(item => item.id) })
  }
}

const after = await page.evaluate(() => {
  const api = window.__rpxProbeApi
  const snap = api.getActiveWorkbook().save()
  const raw = snap.sheets
  const sheet = raw instanceof Map ? raw.get(snap.sheetOrder[0]) : raw[snap.sheetOrder[0]]
  return { columnData: sheet?.columnData, rowData: sheet?.rowData }
})

const result = { hooked, box, hit, attemptCount: attempts.length, sample: attempts.slice(0, 6), before, after, errors }
fs.writeFileSync(outFile, JSON.stringify(result, null, 1), 'utf-8')
console.log(JSON.stringify({ hooked, hit, attemptCount: attempts.length, sample: attempts.slice(0, 5), errors }, null, 1))
await browser.close()
