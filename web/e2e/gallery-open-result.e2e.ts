import { expect, test } from '@playwright/test'

/**
 * 「在设计器中打开 ↗」（`/?gallery=<run base>`）的真判据：打开之后
 * **「运行结果」按钮必须可用**，且装载的是同一运行目录里的 workbook.json。
 *
 * 为什么需要它：这条入口原来只装载模板与数据文件，`printWorkbook` 留 null，
 * 「运行结果」按钮永远是 `disabled`——点不动也不报错（静默不可用）。
 * 同一提交还修掉一条竞态：模板库首项的自动装载会把示例模板连同结果一起顶掉。
 *
 * 依赖 `bash scripts/build_r_model_gallery.sh` 的产物（`web/public/report-gallery/`
 * 不入库）；没有产物时跳过，不让 CI 因为「没跑生成脚本」变红。
 */
const BASE = '/report-gallery/run-1792293895313098000/insurance-policy'

test.beforeAll(async ({ request }) => {
  const response = await request.get(`${BASE}/workbook.json`)
  test.skip(!response.ok(), '示例尚未生成：先跑 bash scripts/build_r_model_gallery.sh')
})

test('示例入口打开后「运行结果」可用，且装载同一运行的 workbook', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(String(error)))
  let workbookBody = ''
  page.on('response', async response => {
    if (response.url().endsWith(`${BASE}/workbook.json`)) workbookBody = await response.text()
  })

  await page.goto(`/?gallery=${encodeURIComponent(BASE)}`)

  const runResult = page.getByRole('button', { name: '运行结果', exact: true })
  await expect(runResult).toBeEnabled({ timeout: 20_000 })
  // 装载的确是这份示例的运行结果（标题在 workbook.json 里），不是别的报表
  expect(workbookBody).toContain('财产基本险')

  await runResult.click()
  await expect(runResult).toHaveAttribute('aria-pressed', 'true')
  expect(errors).toEqual([])
})
