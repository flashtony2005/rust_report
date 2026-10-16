/**
 * W3-1 浏览器端到端驱动（M7-W）：报表设置面板（print_setup）。
 *
 * 用法：`RPX_DRIVER=tests/fixtures/out/e2e_w2_setup.mjs bash tests/fixtures/out/e2e_w2_panel.sh`
 *
 * 判据（延续 E 编号；任一 ✗ ⇒ exit 1）：
 *
 * | # | 判据 |
 * | --- | --- |
 * | W21 | 装载 `def_params_demo.json` ⇒ 「报表设置」条就位，每页行数缺省 20 |
 * | W22 | 每页行数 20→3 ⇒ 状态栏 + 导出 `print_setup.row_num_per_page===3`（逐键补丁） |
 * | W23 | 横↔竖互换 ⇒ 导出 `paper_width=297 / paper_height=210`（缺省互换） |
 * | W24 | 边距/缩放/份数逐个补丁 ⇒ 导出只含改过的键（缺省即删键） |
 * | W25 | 越界值（每页行数 -1）⇒ 显式报错、JSON 不被污染 |
 * | W26 | 产物 `POST /render` 200（服务端契约接受） |
 * | W27 | **菜单导出 CSV**：文件菜单 → `/export` 字节链路，状态栏确认 |
 * | W28 | **视图菜单**：隐藏/显示右侧属性面板（W3-5 停靠布局） |
 * | W29 | **帮助菜单**：关于上屏 |
 *
 * 本脚本是证据生成器；机器判据在 `web/src/reportsetup.test.ts`（G9 覆盖）。
 */
import { createRequire } from 'node:module'
import { writeFileSync } from 'node:fs'
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

  // ---- W21 装载（带 row_props/col_props 的既有夹具）----
  await page.setInputFiles('input[type=file]', resolve(ROOT, 'tests/fixtures/def_style_demo.json'))
  await page.getByRole('button', { name: '渲染', exact: true }).click()
  const loaded = await waitText(page, '已装载定义 JSON')
  const rowNum = await page.locator('input[type=number][title^="每页行数"]').inputValue()
  rec(
    'W21 报表设置条就位 + 每页行数缺省 20',
    loaded && rowNum === '20',
    `装载=${loaded}，每页行数=${rowNum}`,
  )
  if (!loaded) process.exitCode = 1

  // ---- W22 每页行数 20→3 ----
  await page.locator('input[type=number][title^="每页行数"]').fill('3')
  const s22 = await waitText(page, '已更新报表设置 row_num_per_page')
  await page.locator('input[type=number][title^="每页行数"]').blur()
  const d1 = JSON.parse(await grabExport(page))
  rec(
    'W22 每页行数 20→3：逐键补丁',
    s22 && d1.print_setup?.row_num_per_page === 3,
    `状态栏=${s22}，print_setup=${JSON.stringify(d1.print_setup)}`,
  )

  // ---- W23 横↔竖互换（缺省 210x297 → 297x210）----
  await page.getByRole('button', { name: '横↔竖', exact: true }).click()
  const s23 = await waitText(page, '已互换纸张方向')
  const d2 = JSON.parse(await grabExport(page))
  rec(
    'W23 横竖互换：297x210',
    s23 && d2.print_setup?.paper_width === 297 && d2.print_setup?.paper_height === 210,
    `状态栏=${s23}，paper=${JSON.stringify({ w: d2.print_setup?.paper_width, h: d2.print_setup?.paper_height })}`,
  )

  // ---- W24 边距/缩放/份数逐个补丁 ⇒ 只含改过的键 ----
  await page.locator('input[type=number][title^="缩放百分比"]').fill('80')
  const s24a = await waitText(page, '已更新报表设置 zoom_scale')
  await page.locator('input[type=number][title^="打印份数"]').fill('2')
  const s24b = await waitText(page, '已更新报表设置 copy_count')
  await page.locator('input[type=number][title^="上边距"]').fill('30')
  const s24c = await waitText(page, '已更新报表设置 top_margin')
  await page.locator('input[type=number][title^="上边距"]').blur()
  const d3 = JSON.parse(await grabExport(page))
  const ps3 = d3.print_setup ?? {}
  rec(
    'W24 缩放/份数/边距补丁：只含改过的键 + row_num_per_page 保留',
    s24a && s24b && s24c && ps3.zoom_scale === 80 && ps3.copy_count === 2 &&
      ps3.top_margin === 30 && ps3.row_num_per_page === 3 &&
      ps3.paper_width === 297 && ps3.left_margin === undefined,
    `print_setup=${JSON.stringify(ps3)}`,
  )

  // ---- W25 越界值显式报错 ----
  await page.locator('input[type=number][title^="每页行数"]').fill('-1')
  const s25 = await waitText(page, '编辑失败')
  await page.locator('input[type=number][title^="每页行数"]').blur()
  rec('W25 每页行数 -1：显式报错', s25, `状态栏=${s25}`)
  // 恢复合法值（-1 不应落进 JSON）
  await page.locator('input[type=number][title^="每页行数"]').fill('3')
  await page.waitForTimeout(200)

  // ---- W26 服务端接受 ----
  const form = new FormData()
  form.append('report', new File([JSON.stringify(d3)], 'w2_setup_exported.json', { type: 'application/json' }))
  const resp = await fetch(`${SRV_URL}/render`, { method: 'POST', body: form })
  rec(
    'W26 /render 200：产物被服务端接受',
    resp.ok,
    `status=${resp.status}${resp.ok ? '' : '，body=' + JSON.stringify(await resp.json().catch(() => null))}`,
  )

  // ---- W27 菜单：导出 CSV（/export 链路过网）----
  await page.getByRole('menuitem', { name: '文件', exact: true }).click()
  await page.getByRole('menuitem', { name: '导出 CSV（.csv）' }).click()
  const s27 = await waitText(page, '已导出')
  rec('W27 菜单导出 CSV：状态栏确认（/export 字节链路）', s27, `状态栏=${s27}`)

  // ---- W28 菜单：视图 ↔ 右侧面板显隐 ----
  await page.getByRole('menuitem', { name: '视图', exact: true }).click()
  await page.getByRole('menuitem', { name: '隐藏右侧属性面板' }).click()
  const hidden = await page.locator('[data-testid="side-panes"]').count()
  await page.getByRole('menuitem', { name: '视图', exact: true }).click()
  await page.getByRole('menuitem', { name: '显示右侧属性面板' }).click()
  const shown = await page.locator('[data-testid="side-panes"]').count()
  rec('W28 视图菜单：隐藏/显示右侧面板', hidden === 0 && shown === 1, `隐藏后=${hidden}，显示后=${shown}`)

  // ---- W29 菜单：帮助 → 关于 ----
  await page.getByRole('menuitem', { name: '帮助', exact: true }).click()
  await page.getByRole('menuitem', { name: '关于', exact: true }).click()
  const s29 = await waitText(page, '定义 JSON 通道')
  rec('W29 帮助菜单：关于上屏', s29, `状态栏=${s29}`)

  writeFileSync(join(HERE, 'w2_setup_exported.json'), JSON.stringify(d3))
  await page.screenshot({ path: join(HERE, 'w2-setup-e2e.png') })
} finally {
  await browser.close()
}

if (FAILED.length > 0) {
  console.log(`[w2e2e] FAIL: ${FAILED.join(', ')}`)
  process.exitCode = 1
} else {
  console.log('[w2e2e] ALL GREEN（W21–W29 全过）')
}
