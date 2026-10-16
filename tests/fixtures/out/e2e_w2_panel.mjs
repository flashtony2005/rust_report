/**
 * W2-2 浏览器端到端驱动（M7-W）：样式面板「读 → 选区同步 → 改 → 导出 → 服务端快照」。
 *
 * 用法：编排见同目录 `e2e_w2_panel.sh`（起 rpx-server 8090 + vite 5288 后调本脚本）。
 *
 * 判据（每条打印 ✓/✗，任一 ✗ ⇒ exit 1；服务端地址/前端地址/playwright 路径可用
 * 环境变量 RPX_SRV_URL / RPX_WEB_URL / RPX_PLAYWRIGHT 覆盖）：
 *
 * | # | 判据 |
 * | --- | --- |
 * | E1 | 选 `def_style_demo.json` → 渲染 → 状态栏「已装载定义 JSON」+ canvas 出现 |
 * | E2 | **读方向**：默认选区 A1（bold+红+18）⇒ 面板 B 按下 / 18 / #ff0000 |
 * | E3 | **选区同步**：点击 B1（无样式）⇒ 全缺省；点回 A1 ⇒ 样式回来 |
 * | E4 | **写方向**：B1 点 B ⇒ 状态栏更新 + 面板 B 按下 |
 * | E5 | **写方向（颜色/字号）**：B1 底色 → #ffff00；字号 → 20 |
 * | E6 | **elide**：A2（italic 格）关闭 I ⇒ 面板 I 弹起 |
 * | E7 | **导出**：点「导出 JSON」捕获产物文本 |
 * | E8 | **导出产物逐键断言**：A1 原样 / B1 三键 / A2 删 italic 键 |
 * | E9 | **服务端往返**：产物 → `POST /render` → 快照样式逐项一致（坐标系一并验证） |
 *
 * ⚠ 定位用**几何点击**而不是方向键：实测（2026-10-16 首跑）键盘方案在
 *   「面板输入框夺走焦点」后不可靠（number 输入会把 ArrowDown 旋成值增减、
 *   且 Univer 容器失焦后不认方向键），而鼠标点击与焦点无关、天然确定。
 *   几何按 Univer 缺省量出（本机实测）：行头宽 46、列头高 20、列宽 88、行高 24；
 *   点击后一律用**面板回读自证**（E2/E3 互为对照），Univer 升级改了缺省会红。
 *
 * ⚠ 本脚本是**证据生成器**，不是 unit test：它驱动真浏览器 + 真服务端。
 * 机器判据（纯逻辑）在 `web/src/stylepanel.test.ts`（G9 覆盖）。
 */
import { createRequire } from 'node:module'
import { writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url)) // tests/fixtures/out
const ROOT = resolve(HERE, '..', '..', '..') // 仓库根
const WEB_URL = process.env.RPX_WEB_URL ?? 'http://127.0.0.1:5288'
const SRV_URL = process.env.RPX_SRV_URL ?? 'http://127.0.0.1:8090'
/** playwright 库路径（本机 = managed node 自带 @playwright/cli 里的 1.64；内核 chromium-1247 已装）。 */
const PW =
  process.env.RPX_PLAYWRIGHT ??
  'C:/Users/Administrator/.workbuddy/binaries/node/versions/22.22.2-6/node_modules/@playwright/cli/node_modules/playwright'

const FAILED = []
const rec = (id, ok, detail) => {
  console.log(`[w2e2e] ${ok ? '✓' : '✗'} ${id} :: ${detail}`)
  if (!ok) FAILED.push(id)
}

/** 等页面正文出现某段文本（状态栏口径）；超时返回 false（不抛，留给断言报）。 */
const waitText = async (page, text, timeout = 15000) => {
  try {
    await page.waitForFunction((t) => document.body.innerText.includes(t), text, { timeout })
    return true
  } catch {
    return false
  }
}

/** 读样式面板的 DOM 显示态（select / number / 三个 aria-pressed / 两个 color）。 */
const readPanel = (page) =>
  page.evaluate(() => {
    const q = (s) => document.querySelector(s)
    const btn = (name) =>
      [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === name)
    const colors = [...document.querySelectorAll('input[type=color]')]
    return {
      fontName: q('select')?.value ?? null,
      fontSize: q('input[type=number]')?.value ?? null,
      bold: btn('B')?.getAttribute('aria-pressed') ?? null,
      italic: btn('I')?.getAttribute('aria-pressed') ?? null,
      underline: btn('U')?.getAttribute('aria-pressed') ?? null,
      foreColor: colors[0]?.value ?? null,
      backColor: colors[1]?.value ?? null,
      disabled: !!q('input[type=number]')?.disabled,
    }
  })

/** 设置 `<input type=color>`：走原生 setter + input 事件 —— 直接改 `.value`
 * 绕不过 React 的受控值跟踪（它是合成事件比较 tracker 后决定放不放行）。 */
const setColor = (page, index, value) =>
  page.evaluate(
    ([i, v]) => {
      const el = document.querySelectorAll('input[type=color]')[i]
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
      setter.call(el, v)
      el.dispatchEvent(new Event('input', { bubbles: true }))
    },
    [index, value],
  )

const require_ = createRequire(import.meta.url)
const { chromium } = require_(PW)

/** Univer 缺省度量（本机实测；点击后由面板回读自证，升级漂移会红）。 */
const HEAD_W = 46 // 行头宽
const HEAD_H = 20 // 列头高
const COL_W = 88 // 缺省列宽（core `defaultColumnWidth: 88`）
const ROW_H = 24 // 缺省行高

const browser = await chromium.launch({ headless: true })
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
  page.setDefaultTimeout(15000)
  await page.goto(WEB_URL)

  // ---- E1 装载 ----
  await page.setInputFiles('input[type=file]', resolve(ROOT, 'tests/fixtures/def_style_demo.json'))
  await page.getByRole('button', { name: '渲染', exact: true }).click()
  const loaded = await waitText(page, '已装载定义 JSON')
  const canvasCount = await page.locator('canvas').count()
  rec('E1 装载 .json → Univer', loaded && canvasCount > 0, `状态栏「已装载」=${loaded}，canvas=${canvasCount}`)
  if (!loaded) {
    console.log('[w2e2e] 页面正文：', (await page.evaluate(() => document.body.innerText)).slice(0, 400))
  }
  await page.waitForTimeout(600) // 等 canvas 首帧 + 选区就绪

  // 主画布（面积最大者）= 表格区；格子中心 = 画布左上 + 表头偏移 + 行列累积。
  const canvases = await page.evaluate(() =>
    [...document.querySelectorAll('canvas')].map((c) => {
      const r = c.getBoundingClientRect()
      return { w: Math.round(r.width), h: Math.round(r.height), x: Math.round(r.x), y: Math.round(r.y) }
    }),
  )
  const main = canvases.reduce((a, b) => (a.w * a.h >= b.w * b.h ? a : b))
  const cell = (r, c) => ({
    x: main.x + HEAD_W + c * COL_W + COL_W / 2,
    y: main.y + HEAD_H + r * ROW_H + ROW_H / 2,
  })
  const clickCell = async (r, c) => {
    const p = cell(r, c)
    await page.mouse.click(p.x, p.y)
    await page.waitForTimeout(350)
  }

  // ---- E2 读方向（默认选区应是 A1）----
  const p1 = await readPanel(page)
  rec(
    'E2 读方向：A1(bold+红+18) → 面板',
    p1.disabled === false &&
      p1.bold === 'true' &&
      p1.fontSize === '18' &&
      p1.foreColor.toLowerCase() === '#ff0000' &&
      p1.italic === 'false' &&
      p1.fontName === '宋体',
    JSON.stringify(p1),
  )
  await page.screenshot({ path: join(HERE, 'w2-panel-load.png') })

  // ---- E3 选区同步（点 B1 普通格 → 全缺省；点回 A1 → 样式回来）----
  await clickCell(0, 1)
  const p2 = await readPanel(page)
  await clickCell(0, 0)
  const p2b = await readPanel(page)
  rec(
    'E3 选区同步：点 B1 → 全缺省；点回 A1 → 样式回来',
    p2.bold === 'false' &&
      p2.fontSize === '12' &&
      p2.foreColor.toLowerCase() === '#000000' &&
      p2b.bold === 'true' &&
      p2b.fontSize === '18' &&
      p2b.foreColor.toLowerCase() === '#ff0000',
    `B1=${JSON.stringify(p2)}；A1=${JSON.stringify(p2b)}`,
  )

  // ---- E4 写方向：B1 加粗 ----
  await clickCell(0, 1)
  await page.getByRole('button', { name: 'B', exact: true }).click()
  const s4 = await waitText(page, '已更新样式 bold')
  const p3 = await readPanel(page)
  rec('E4 写方向：B1 点 B', s4 && p3.bold === 'true', `状态栏=${s4}，面板 bold=${p3.bold}`)

  // ---- E5 写方向：B1 底色 + 字号 ----
  await setColor(page, 1, '#ffff00')
  const s5 = await waitText(page, '已更新样式 backColor')
  const p4 = await readPanel(page)
  await page.locator('input[type=number]').fill('20')
  const s5b = await waitText(page, '已更新样式 fontSize')
  const p5 = await readPanel(page)
  rec(
    'E5 写方向：B1 底色 #ffff00 + 字号 20',
    s5 && s5b && p4.backColor.toLowerCase() === '#ffff00' && p5.fontSize === '20',
    `状态栏=${s5}/${s5b}，backColor=${p4.backColor}，fontSize=${p5.fontSize}`,
  )
  // 收掉输入框焦点（后续只用鼠标点击定位，但别让浏览器把焦点留在 number 上）。
  await page.locator('input[type=number]').blur()
  await page.waitForTimeout(200)

  // ---- E6 elide：A2 关闭斜体 ----
  await clickCell(1, 0) // A2（斜体格）
  const p6 = await readPanel(page)
  await page.getByRole('button', { name: 'I', exact: true }).click()
  const s6 = await waitText(page, '已更新样式 italic')
  const p7 = await readPanel(page)
  rec(
    'E6 elide：A2 关闭 I',
    p6.italic === 'true' && s6 && p7.italic === 'false',
    `进 A2 时 italic=${p6.italic}，状态栏=${s6}，改后 italic=${p7.italic}`,
  )

  // ---- E7 导出（拦截 createObjectURL 抓 Blob 文本，走真导出路径）----
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
  const exported = await page.evaluate(() => window.__w2export)
  writeFileSync(join(HERE, 'w2_panel_exported.json'), exported)
  rec('E7 导出 JSON', typeof exported === 'string' && exported.includes('"grid"'), `${exported.length} 字符 → w2_panel_exported.json`)

  // ---- E8 导出产物逐键断言 ----
  const def = JSON.parse(exported)
  const at = (r, c) => (def.cells ?? []).find((x) => x.row === r && x.col === c)
  const [c11, c12, c21] = [at(1, 1), at(1, 2), at(2, 1)]
  rec(
    'E8a 未动格原样（A1 仍 bold+红+18）',
    c11?.bold === true && c11?.fore_color === '#FF0000' && c11?.font_size === 18 && c11?.text === '样式演示',
    JSON.stringify(c11),
  )
  rec(
    'E8b B1 三键落盘（bold/back_color 大写/font_size 20）',
    c12?.bold === true && c12?.back_color === '#FFFF00' && c12?.font_size === 20 && c12?.text === '普通格',
    JSON.stringify(c12),
  )
  rec(
    'E8c A2 的 italic 键被删（elide），text 保留',
    !!c21 && !('italic' in c21) && c21.text === '斜体格',
    JSON.stringify(c21),
  )

  // ---- E9 服务端往返：产物 → /render → 快照样式 ----
  const form = new FormData()
  form.append('report', new File([exported], 'w2_panel_exported.json', { type: 'application/json' }))
  const resp = await fetch(`${SRV_URL}/render`, { method: 'POST', body: form })
  const snap = await resp.json()
  const sheet = Object.values(snap.sheets ?? {})[0]
  const cd = sheet?.cellData ?? {}
  const s00 = cd['0']?.['0']?.s
  const s01 = cd['0']?.['1']?.s
  const s10 = cd['1']?.['0']?.s
  rec('E9a 快照 A1 = JSON 声明（bl/fs/cl）', s00?.bl === 1 && s00?.fs === 18 && s00?.cl?.rgb === '#FF0000', JSON.stringify(s00))
  rec('E9b 快照 B1 = 面板改后的值（bl/fs/bg）', s01?.bl === 1 && s01?.fs === 20 && s01?.bg?.rgb === '#FFFF00', JSON.stringify(s01))
  rec('E9c 快照 A2 无 it（elide 过网生效）', s10 !== undefined && s10.it === undefined, JSON.stringify(s10))
  rec('E9d 值未被样式补丁破坏', cd['0']?.['1']?.v === '普通格' && cd['1']?.['0']?.v === '斜体格', `${cd['0']?.['1']?.v}/${cd['1']?.['0']?.v}`)

  await page.screenshot({ path: join(HERE, 'w2-panel-e2e.png') })
} finally {
  await browser.close()
}

if (FAILED.length > 0) {
  console.log(`[w2e2e] FAIL: ${FAILED.join(', ')}`)
  process.exitCode = 1
} else {
  console.log('[w2e2e] ALL GREEN（E1–E9 全过）')
}
