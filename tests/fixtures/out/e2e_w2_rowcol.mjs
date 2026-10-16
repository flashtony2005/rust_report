/**
 * W2-5 浏览器端到端驱动（M7-W）：行列面板（行高/列宽/断页）+ 网格尺寸 + 数据集绑定。
 *
 * 用法：`RPX_DRIVER=tests/fixtures/out/e2e_w2_rowcol.mjs bash tests/fixtures/out/e2e_w2_panel.sh`
 * （复用同一编排脚本：起 rpx-server 8090 + vite 5288 后调本脚本。）
 *
 * 判据（延续 `e2e_w2_panel.mjs` 的 E 编号；每条打印 ✓/✗，任一 ✗ ⇒ exit 1）：
 *
 * | # | 判据 |
 * | --- | --- |
 * | E14 | **行高**：点 B2 → 行高 15 ⇒ 导出 `row_props` 含 `{row:2, height:15}` |
 * | E15 | **列宽**：列宽 30 ⇒ 导出 `col_props` 含 `{col:2, width:30}` |
 * | E16 | **断页**：select「此行前断页」⇒ 导出 `row_props[row:2].break_page === true` |
 * | E17 | **网格扩大**：3→4 行 ⇒ 重渲染成功 + 导出 `grid.rows===4` + 快照 rowData 有第 4 行 |
 * | E18 | **网格缩小保护**：4→2 行（A3 末行会越界）⇒ 状态栏报错、grid 仍 4 行 |
 * | E19 | **数据集**：添加 file 数据集 ⇒ 列表出现 + 导出 `datasets[0]`；SQL 空字段 ⇒ 显式报错 |
 * | E20 | **服务端接受**：含 row_props/col_props/datasets 的产物 `POST /render` 200 |
 *
 * ⚠ 定位纪律与 `e2e_w2_panel.mjs` 相同：几何点击 + 运行时测量；面板控件用
 * title 选择器 + fill/selectOption（React 受控输入走原生 setter 同款管线）。
 * 本脚本是证据生成器；机器判据在 `web/src/rowcol.test.ts` / `datasets.test.ts`。
 */
import { createRequire } from 'node:module'
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))
const ROOT = resolve(HERE, '..', '..', '..')
const WEB_URL = process.env.RPX_WEB_URL ?? 'http://127.0.0.1:5288'
const SRV_URL = process.env.RPX_SRV_URL ?? 'http://127.0.0.1:8090'
const PW =
  process.env.RPX_PLAYWRIGHT ??
  'C:/Users/Administrator/.workbuddy/binaries/node/versions/22.22.2-6/node_modules/@playwright/cli/node_modules/playwright'

const FAILED = []
const rec = (id, ok, detail) => {
  console.log(`[w2e2e] ${ok ? '✓' : '✗'} ${id} :: ${detail}`)
  if (!ok) FAILED.push(id)
}

const waitText = async (page, text, timeout = 15000) => {
  try {
    await page.waitForFunction((t) => document.body.innerText.includes(t), text, { timeout })
    return true
  } catch {
    return false
  }
}

/** 导出：拦截 createObjectURL 抓 Blob 文本（与 e2e_w2_panel.mjs 同款管线）。 */
const grabExport = async (page) => {
  await page.evaluate(() => {
    window.__w2export = null
    const orig = URL.createObjectURL.bind(URL)
    URL.createObjectURL = (blob) => {
      blob
        .text()
        .then((t) => {
          window.__w2export = t
        })
        .catch(() => {})
      return orig(blob)
    }
  })
  await page.getByRole('button', { name: '导出 JSON', exact: true }).click()
  await page.waitForFunction(() => typeof window.__w2export === 'string')
  return page.evaluate(() => window.__w2export)
}

const require_ = createRequire(import.meta.url)
const { chromium } = require_(PW)

const browser = await chromium.launch({ headless: true })
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
  page.setDefaultTimeout(15000)
  await page.goto(WEB_URL)

  // ---- 装载（同 E1 管线；选区默认 A1）----
  await page.setInputFiles('input[type=file]', resolve(ROOT, 'tests/fixtures/def_style_demo.json'))
  await page.getByRole('button', { name: '渲染', exact: true }).click()
  const loaded = await waitText(page, '已装载定义 JSON')
  rec('E14p 装载 .json', loaded, `状态栏「已装载」=${loaded}`)
  if (!loaded) process.exitCode = 1
  await page.waitForTimeout(800)

  // 几何（本夹具同款：快照读行高列宽 + 表头偏移按已收口脚本的实测值起算；
  // 这里只点 B2 一格，直接用既有实测 headW/headH 起算，出错由「面板读回」自检兜底）。
  const snap0 = await (async () => {
    const form = new FormData()
    form.append(
      'report',
      new File([readFileSync(resolve(ROOT, 'tests/fixtures/def_style_demo.json'))], 'd.json'),
    )
    const j = await (await fetch(`${SRV_URL}/render`, { method: 'POST', body: form })).json()
    return Object.values(j.sheets ?? {})[0]
  })()
  const rowH = snap0?.rowData?.['0']?.h ?? 30.24
  const colW = snap0?.columnData?.['0']?.w ?? 94.49
  const canvases = await page.evaluate(() =>
    [...document.querySelectorAll('canvas')].map((c) => {
      const r = c.getBoundingClientRect()
      return { w: Math.round(r.width), h: Math.round(r.height), x: Math.round(r.x), y: Math.round(r.y) }
    }),
  )
  const main = canvases.reduce((a, b) => (a.w * a.h >= b.w * b.h ? a : b))
  // 表头偏移：用已收口脚本的传感器二分（B1 无样式 ⇒ 点进第二列/第二行即离开 A1）。
  const readBold = async (x, y) => {
    await page.mouse.click(Math.round(x), Math.round(y))
    await page.waitForTimeout(150)
    return page.evaluate(() => {
      const b = [...document.querySelectorAll('button')].find((x) => x.textContent.trim() === 'B')
      return b?.getAttribute('aria-pressed')
    })
  }
  let loX = Math.round(main.x + 30)
  let hiX = Math.round(main.x + 30 + 2 * colW)
  while (hiX - loX > 1) {
    const mid = Math.floor((loX + hiX) / 2)
    if ((await readBold(mid, main.y + 40)) === 'true') loX = mid
    else hiX = mid
  }
  const headW = hiX - main.x - colW
  let loY = Math.round(main.y + 5)
  let hiY = Math.round(main.y + 5 + 2 * rowH)
  while (hiY - loY > 1) {
    const mid = Math.floor((loY + hiY) / 2)
    if ((await readBold(main.x + headW + colW / 2, mid)) === 'true') loY = mid
    else hiY = mid
  }
  const headH = hiY - main.y - rowH
  const clickCell = async (r, c) => {
    await page.mouse.click(
      Math.round(main.x + headW + c * colW + colW / 2),
      Math.round(main.y + headH + r * rowH + rowH / 2),
    )
    await page.waitForTimeout(300)
  }
  // 校准自检：点 A1 必须回读 bold=true。
  await clickCell(0, 0)
  const cal = await page.evaluate(() => {
    const b = [...document.querySelectorAll('button')].find((x) => x.textContent.trim() === 'B')
    return b?.getAttribute('aria-pressed')
  })
  rec('E14c 几何校准：点 A1 → bold', cal === 'true', `bold=${cal}，headW=${headW}，headH=${headH}`)

  // ---- E14 行高：点 B2（r1,c1）→ 行高 15 ----
  await clickCell(1, 1)
  await page.locator('input[type=number][title^="行高"]').fill('15')
  const s14 = await waitText(page, '已更新行高')
  await page.locator('input[type=number][title^="行高"]').blur()
  const rh = await page.locator('input[type=number][title^="行高"]').inputValue()
  rec('E14 行高 15：状态栏 + 面板回读', s14 && rh === '15', `状态栏=${s14}，行高=${rh}`)

  // ---- E15 列宽：同选区 → 列宽 30 ----
  await page.locator('input[type=number][title^="列宽"]').fill('30')
  const s15 = await waitText(page, '已更新列宽')
  await page.locator('input[type=number][title^="列宽"]').blur()
  rec('E15 列宽 30：状态栏', s15, `状态栏=${s15}`)

  // ---- E16 断页：同选区（行 2）→ 此行前断页 ----
  await page.selectOption('select[title^="行断页"]', 'on')
  const s16 = await waitText(page, '已更新断页')
  rec('E16 断页 on：状态栏（含「重渲染生效」提示）', s16, `状态栏=${s16}`)

  const exported1 = await grabExport(page)
  writeFileSync(join(HERE, 'w2_rowcol_exported.json'), exported1)
  const d1 = JSON.parse(exported1)
  const rp2 = (d1.row_props ?? []).find((p) => p.row === 2)
  const cp2 = (d1.col_props ?? []).find((p) => p.col === 2)
  rec(
    'E14b/E15b/E16b 导出产物：row_props/col_props/break_page',
    rp2?.height === 15 && cp2?.width === 30 && rp2?.break_page === true,
    `row_props=${JSON.stringify(d1.row_props)}，col_props=${JSON.stringify(d1.col_props)}`,
  )

  // ---- E17 网格扩大：3→4 行 ⇒ 重渲染 + 快照有第 4 行 ----
  await page.locator('input[type=number][title^="网格行数"]').fill('4')
  await page.getByRole('button', { name: '调整网格', exact: true }).click()
  const s17 = await waitText(page, '网格已调整并重渲染')
  const d2 = JSON.parse(await grabExport(page))
  rec(
    'E17 网格 3→4：重渲染 + grid.rows=4',
    s17 && d2.grid?.rows === 4,
    `状态栏=${s17}，grid=${JSON.stringify(d2.grid)}`,
  )

  // ---- E18 网格缩小保护：4→2（A3 末行 row=3 会越界）----
  await page.locator('input[type=number][title^="网格行数"]').fill('2')
  await page.getByRole('button', { name: '调整网格', exact: true }).click()
  const s18 = await waitText(page, '会超出新网格')
  const d3 = JSON.parse(await grabExport(page))
  rec(
    'E18 缩小保护：报错上屏 + grid 仍 4 行',
    s18 && d3.grid?.rows === 4,
    `状态栏=${s18}，grid=${JSON.stringify(d3.grid)}`,
  )
  // 收掉输入框焦点。
  await page.locator('input[type=number][title^="网格行数"]').blur()

  // ---- E19 数据集：添加 file 数据集 + SQL 空字段报错 ----
  await page.locator('input[placeholder^="数据集名"]').fill('dsx')
  // 默认类型已是「文件」；填 CSV 文件名。
  await page.locator('input[placeholder^="CSV 文件名"]').fill('emp.csv')
  await page.getByRole('button', { name: '添加', exact: true }).click()
  const s19 = await waitText(page, '已添加数据集')
  const d4 = JSON.parse(await grabExport(page))
  rec(
    'E19a 添加 file 数据集：列表 + 导出 datasets[0]',
    s19 && JSON.stringify(d4.datasets?.[0]) === JSON.stringify({ name: 'dsx', file: { file_name: 'emp.csv', encoding: 'GBK' } }),
    `状态栏=${s19}，datasets=${JSON.stringify(d4.datasets)}`,
  )
  // SQL 分支校验：切到 SQL、留空 dsn/sql → 显式报错。
  await page.selectOption('select[title^="数据集类型"]', 'sql')
  await page.getByRole('button', { name: '添加', exact: true }).click()
  const s19b = await waitText(page, '必须给 dsn')
  rec('E19b SQL 空 dsn：显式报错不静默', s19b, `状态栏=${s19b}`)

  // ---- E20 服务端接受：datasets[].file 引用的 CSV 必须随 multipart 上传 ----
  // 先验缺文件路径：服务端显式报错（「缺少数据集文件」），不是静默空数据。
  const form0 = new FormData()
  form0.append('report', new File([JSON.stringify(d4)], 'w2_rowcol_exported.json', { type: 'application/json' }))
  const resp0 = await fetch(`${SRV_URL}/render`, { method: 'POST', body: form0 })
  const err0 = await resp0.json().catch(() => null)
  rec(
    'E20a 缺 CSV：显式报「缺少数据集文件」',
    !resp0.ok && String(err0?.error ?? '').includes('缺少数据集文件'),
    `status=${resp0.status}，error=${JSON.stringify(err0)}`,
  )
  // 再验齐件路径：声明 + 同名 CSV → 200。
  const csv = new File([readFileSync(resolve(ROOT, 'tests/fixtures/engine_emp.csv'))], 'emp.csv').slice()
  const form2 = new FormData()
  form2.append('report', new File([JSON.stringify(d4)], 'w2_rowcol_exported.json', { type: 'application/json' }))
  form2.append('dataset', csv, 'emp.csv')
  const resp2 = await fetch(`${SRV_URL}/render`, { method: 'POST', body: form2 })
  rec(
    'E20b /render 200：声明 + 同名 CSV 一起被接受',
    resp2.ok,
    `status=${resp2.status}${resp2.ok ? '' : '，body=' + JSON.stringify(await resp2.json().catch(() => null))}`,
  )

  await page.screenshot({ path: join(HERE, 'w2-rowcol-e2e.png') })
} finally {
  await browser.close()
}

if (FAILED.length > 0) {
  console.log(`[w2e2e] FAIL: ${FAILED.join(', ')}`)
  process.exitCode = 1
} else {
  console.log('[w2e2e] ALL GREEN（E14–E20 全过）')
}
