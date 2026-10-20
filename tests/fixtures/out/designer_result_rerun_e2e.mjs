/**
 * E2E（用户报的那个场景）：**设计器改一格格子 → 点「结果」→ 结果真的变了**。
 *
 * 覆盖三件事：
 * 1. 有未落库改动时点「结果」会**先保存**（否则引擎算的还是旧定义）；
 * 2. 服务端运行记录 +1，`result_state` 变 `current_revision`；
 * 3. 新 run 的 `workbook.json` 里能找到刚改的文字 —— 这是「结果反映设计」的硬证据。
 *
 * 用法：node designer_result_rerun_e2e.mjs <playwright 模块> <输出目录>
 */
import fs from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3] ?? 'F:/project/report/rpx/tests/fixtures/out/gallery_shots'
const base = process.env.RPX_WEB_BASE || 'http://127.0.0.1:5288'
const edited = `门店E2E${Date.now().toString().slice(-6)}`

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', error => errors.push(String(error)))

await page.goto(base + '/', { waitUntil: 'networkidle' })
await page.waitForTimeout(2500)

const options = await page.locator('select[aria-label="报表库"] option').allTextContents()
const index = options.findIndex(text => text.includes('门店销售'))
await page.selectOption('select[aria-label="报表库"]', { index })
await page.waitForTimeout(3000)
const id = await page.$eval('select[aria-label="报表库"]', select => select.value)

const api = async path => (await page.request.get(`http://127.0.0.1:8090${path}`)).json()
const before = await api(`/reports/${encodeURIComponent(id)}/runs?limit=500`)
// 记下原始格文本：E2E 会**真的写进示例库**，跑完必须还原（示例库不留测试数据）。
const beforeDetail = await api(`/reports/${encodeURIComponent(id)}`)
const originalText = beforeDetail.template?.cells?.find(cell => cell.row === 1 && cell.col === 1)?.text ?? ''

// 设计视图 → 选中一个数据格 → 改内容 → 应用
await page.getByRole('button', { name: '设计', exact: true }).click()
await page.waitForTimeout(1500)
const canvas = page.locator('canvas').first()
const box = await canvas.boundingBox()
if (box) { await page.mouse.click(box.x + 60, box.y + 60); await page.waitForTimeout(1200) }

const cellInput = page.locator('input[aria-label="单元格内容"], textarea[aria-label="单元格内容"]').first()
await cellInput.fill(edited)
await page.waitForTimeout(600)
await page.getByRole('button', { name: '应用单元格定义' }).click()
await page.waitForTimeout(1200)

const dirtyHint = await page.locator('.rpx-rerun').count()
const statusBefore = (await page.locator('.rpx-status .msg').first().textContent())?.trim()

// 点「结果」—— 期望：自动保存 + 调引擎重算 + 切结果视图
await page.locator('.rpx-toolbar button.tb.primary').click()
await page.waitForFunction(() => {
  const text = document.querySelector('.rpx-status .msg')?.textContent ?? ''
  return text.includes('已重新生成结果') || text.includes('重新生成失败') || text.includes('保存失败')
}, { timeout: 180000 })
await page.waitForTimeout(2000)

const statusAfter = (await page.locator('.rpx-status .msg').first().textContent())?.trim()
const after = await api(`/reports/${encodeURIComponent(id)}/runs?limit=500`)
const detail = await api(`/reports/${encodeURIComponent(id)}`)
const runId = detail.display_run_id
const workbook = runId
  ? await api(`/reports/${encodeURIComponent(id)}/runs/${encodeURIComponent(runId)}/artifacts/workbook.json`)
  : null

fs.mkdirSync(outDir, { recursive: true })
await page.screenshot({ path: `${outDir}/designer_result_rerun.png` })

// ── 还原 ────────────────────────────────────────────────────────────────
// 把模板改回原文 + 重算一次，示例库回到「干净且结果最新」的状态。
const detailNow = await api(`/reports/${encodeURIComponent(id)}`)
const template = detailNow.template
const cell = template?.cells?.find(item => item.row === 1 && item.col === 1)
if (cell && originalText) cell.text = originalText
const put = await page.request.put(`http://127.0.0.1:8090/reports/${encodeURIComponent(id)}/template`, {
  headers: { 'Content-Type': 'application/json', 'If-Match': detailNow.revision },
  data: template,
})
await page.request.post(`http://127.0.0.1:8090/reports/${encodeURIComponent(id)}/runs`, {
  headers: { 'Content-Type': 'application/json' },
  data: {},
})

const report = {
  id,
  edited,
  originalText,
  restoredStatus: put.status(),
  // 编辑后应出现「结果不是最新」提示（dirty 分支）
  dirtyHintVisible: dirtyHint > 0,
  statusBefore,
  statusAfter,
  runsBefore: before.items.length,
  runsAfter: after.items.length,
  resultState: detail.result_state,
  displayRunId: runId,
  // 硬证据：新 run 的 workbook 里出现刚改的文字
  editedTextInWorkbook: JSON.stringify(workbook ?? {}).includes(edited),
  pageErrors: errors,
  pass: after.items.length === before.items.length + 1
    && String(statusAfter).includes('已重新生成结果')
    && detail.result_state === 'current_revision'
    && JSON.stringify(workbook ?? {}).includes(edited)
    && errors.length === 0,
}
console.log(JSON.stringify(report, null, 1))
await browser.close()
