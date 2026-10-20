// 端到端判据：**结果画布上画的边框要出现在打印预览里**。
//
// 走的是用户真实路径：结果视图里选中一段区域 → 执行 Univer 的「所有边框」命令
// （`sheet.command.set-border-basic`，与工具条边框按钮同一条命令）→ 打开打印预览
// → 检查同一格的 CSS 边框。
//
// 之所以不用「点开边框下拉菜单再点某一项」：那个菜单是 canvas 绘制的浮层，选择器
// 不稳定；命令 id 是它背后的同一条链路，能验证「命令 → 活态 → 合并 → 预览」整段。
//
// 不会写库：全程不点保存，结束时直接 reolad。
import fs from 'node:fs'
import { createRequire } from 'node:module'

// Windows 下 ESM 不收裸盘符路径，统一走 createRequire（与本目录其它 E2E 一致）。
const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3]

const BASE = 'http://127.0.0.1:5288'
const ROW = 2 // 0 起：表头下的第一行数据
const COL = 0
const SPAN = 3 // 横向三格，验「整段都上了框」而不只是第一格

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', error => errors.push(String(error)))

const report = { row: ROW, col: COL, span: SPAN }

try {
  await page.goto(BASE + '/', { waitUntil: 'networkidle' })
  await page.waitForTimeout(2500)
  await page.selectOption('select[aria-label="报表库"]', 'store-ranking')
  await page.waitForTimeout(3000)

  // 切到结果视图（如果已经是结果视图，这次点击只是重算/不动作）
  const resultTab = page.locator('button[aria-pressed]', { hasText: '结果' }).first()
  if (await resultTab.count()) {
    await resultTab.click()
    report.tabClicked = true
  }
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
  await page.waitForTimeout(1200)

  // 画布上画边框（等价于点工具条的「所有边框」）
  const drawn = await page.evaluate(
    async ({ row, col, span }) => {
      const api = window.__rpxProbeApi
      if (!api) return { error: '没有探针 API（非 DEV 构建？）' }
      const workbook = api.getActiveWorkbook()
      const sheet = workbook.getActiveSheet()
      const unitId = workbook.getId()
      const subUnitId = sheet.getSheetId()
      const ok = await api.executeCommand('sheet.command.set-border-basic', {
        unitId,
        subUnitId,
        value: { type: 'all', color: '#FF0000', style: 1 },
        ranges: [{ startRow: row, endRow: row, startColumn: col, endColumn: col + span - 1 }],
      })
      await new Promise(resolve => setTimeout(resolve, 500))
      // 读活态：`s` 可能是内联对象，也可能被换成 styles 表里的 id
      const snapshot = workbook.save()
      const raw = snapshot.sheets
      const live = raw instanceof Map ? raw.get(snapshot.sheetOrder[0]) : raw[snapshot.sheetOrder[0]]
      const styles = snapshot.styles instanceof Map ? Object.fromEntries(snapshot.styles) : snapshot.styles ?? {}
      const styleOf = key => {
        const cell = live.cellData?.[String(row)]?.[String(key)]
        return typeof cell?.s === 'string' ? styles[cell.s] : cell?.s
      }
      return {
        ok: String(ok),
        bd: styleOf(col)?.bd ?? null,
        bdLast: styleOf(col + span - 1)?.bd ?? null,
        styleForm: typeof live.cellData?.[String(row)]?.[String(col)]?.s,
      }
    },
    { row: ROW, col: COL, span: SPAN },
  )
  report.drawn = drawn

  // 打开打印预览
  await page.locator('.rpx-toolbar button', { hasText: '打印预览' }).first().click()
  await page.waitForSelector('.rpx-print-page', { timeout: 30000 })
  await page.waitForTimeout(1200)

  report.preview = await page.evaluate(
    ({ row, col, span }) => {
      const table = document.querySelector('.rpx-print-page table')
      const tr = table?.querySelectorAll('tbody tr')[row]
      if (!tr) return { error: `预览里没有第 ${row} 行` }
      const read = index => {
        const td = tr.children[index]
        if (!td) return null
        const css = getComputedStyle(td)
        return {
          text: (td.textContent || '').slice(0, 8),
          top: `${css.borderTopWidth} ${css.borderTopStyle} ${css.borderTopColor}`,
          bottom: `${css.borderBottomWidth} ${css.borderBottomStyle} ${css.borderBottomColor}`,
          left: `${css.borderLeftWidth} ${css.borderLeftStyle} ${css.borderLeftColor}`,
          right: `${css.borderRightWidth} ${css.borderRightStyle} ${css.borderRightColor}`,
        }
      }
      const cells = []
      for (let i = col; i < col + span; i++) cells.push(read(i))
      return { cells, neighbours: [read(col + span), read(col + span + 1)] }
    },
    { row: ROW, col: COL, span: SPAN },
  )

  fs.mkdirSync(outDir, { recursive: true })
  await page.screenshot({ path: `${outDir}/print_border_e2e.png`, fullPage: false })

  const cells = report.preview?.cells ?? []
  const red = 'rgb(255, 0, 0)'
  report.pass =
    drawn?.ok === 'true' &&
    !!drawn?.bd?.t &&
    cells.length === SPAN &&
    cells.every(cell => cell && cell.top.includes(red) && cell.bottom.includes(red) && cell.left.includes(red) && cell.right.includes(red)) &&
    (report.preview?.neighbours ?? []).every(cell => !cell || !cell.top.includes(red))
  report.errors = errors
} catch (error) {
  report.error = String(error)
  report.errors = errors
  try {
    fs.mkdirSync(outDir, { recursive: true })
    await page.screenshot({ path: `${outDir}/print_border_e2e_fail.png` })
  } catch {}
} finally {
  // 不保存，直接丢弃页面（示例库不被污染）
  await browser.close()
}

console.log(JSON.stringify(report, null, 1))
if (!report.pass) process.exitCode = 1
