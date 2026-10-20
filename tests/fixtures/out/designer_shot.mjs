// 设计器首页截图：验证菜单栏品牌（AI 报表引擎）与设计器外壳。
// 用法：node designer_shot.mjs <playwright目录> <输出目录>
import { mkdirSync } from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require(process.argv[2])
const outDir = process.argv[3]
mkdirSync(outDir, { recursive: true })

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1680, height: 1000 } })
const errors = []
page.on('pageerror', e => errors.push(String(e)))

await page.goto('http://127.0.0.1:5288/', { waitUntil: 'networkidle' })
await page.waitForTimeout(2500)
await page.screenshot({ path: outDir + '/designer_home.png' })
console.log('brand=', await page.locator('header, .menubar, body').first().innerText().then(t => t.slice(0, 120)).catch(() => ''))
console.log('pageerrors=', errors.length)
await browser.close()
console.log('done')
