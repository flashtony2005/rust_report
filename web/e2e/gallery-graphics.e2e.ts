import { expect, test } from '@playwright/test'

/**
 * 实例库「图形」页的真判据：图形必须**真的画出来**，不是空白卡片。
 *
 * 为什么需要它：`rpxGraphics` 只是 JSON，四类内容（chart / echarts /
 * echarts_js / image）各走一条完全不同的渲染路径，任何一条接错都表现为
 * **空白而不是报错** —— 单测只能证明 `chartOption` 翻译对了，证明不了
 * 「页面里确实出现了一个 SVG / iframe」。
 *
 * 依赖 `bash scripts/build_r_model_gallery.sh` 的产物（`web/public/report-gallery/`
 * 不入库）；没有产物时跳过，不让 CI 因为「没跑生成脚本」变红。
 */
const cases = [
  { id: 'chart-sales-dashboard', charts: 4 },
  { id: 'chart-tasks-timeline', charts: 3 },
  { id: 'echarts-option-template', charts: 1 },
  { id: 'echarts-js-formatter', charts: 1 },
]

test.beforeAll(async ({ request }) => {
  const response = await request.get('/report-gallery/catalog.json')
  test.skip(!response.ok(), '示例尚未生成：先跑 bash scripts/build_r_model_gallery.sh')
})

for (const { id, charts } of cases) {
  test(`${id} 的图形页画出 ${charts} 张图`, async ({ page }) => {
    const errors: string[] = []
    page.on('pageerror', error => errors.push(String(error)))
    await page.goto(`/gallery.html#${id}`)
    await expect(page.locator('.gallery-graphic')).toHaveCount(charts, { timeout: 20_000 })
    // ECharts 走 svg 渲染器；echarts_js 是 sandbox iframe。两者都算「画出来了」。
    const rendered =
      (await page.locator('.gallery-graphic svg').count()) +
      (await page.locator('.gallery-graphic iframe').count())
    expect(rendered).toBeGreaterThanOrEqual(charts)
    await expect(page.locator('.gallery-graphic [role=alert]')).toHaveCount(0)
    expect(errors).toEqual([])
  })
}
