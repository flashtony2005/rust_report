// 画廊截图：首屏（侧栏 + 新例子）+ 三份新例子的结果画布。
// 用法：node gallery_shot.mjs <playwright目录> <输出目录>
import { mkdirSync } from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3]
mkdirSync(outDir, { recursive: true })
const base = process.env.RPX_WEB_BASE || 'http://127.0.0.1:5288'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })

// 1) 画廊首屏（含顶栏 logo 与侧栏分组）
await page.goto(base + '/gallery.html', { waitUntil: 'networkidle' })
await page.waitForTimeout(1200)
await page.screenshot({ path: outDir + '/gallery_home.png' })

// 2) 三份新例子
for (const id of ['pareto-contribution', 'budget-variance', 'store-ranking']) {
  await page.goto(base + '/gallery.html#' + id, { waitUntil: 'networkidle' })
  await page.waitForSelector('.gallery-sheet canvas', { timeout: 30000 })
  await page.waitForTimeout(2500)
  await page.screenshot({ path: `${outDir}/ex_${id}.png` })
  console.log('shot', id)
}

await browser.close()
console.log('done')
