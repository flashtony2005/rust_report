import { expect, test } from '@playwright/test'

/**
 * 「在设计器中打开 ↗」（`/?gallery=<run base>`）的真判据：打开之后
 * **结果视图按钮必须可用**，且装载的是同一运行目录里的 workbook.json。
 *
 * 为什么需要它：这条入口原来只装载模板与数据文件，`printWorkbook` 留 null，
 * 结果按钮永远是 `disabled`——点不动也不报错（静默不可用）。
 * 同一提交还修掉一条竞态：模板库首项的自动装载会把示例模板连同结果一起顶掉。
 *
 * 第二个 bug（本例第二条）：装好 workbook 之后，**工具栏的「查看结果」仍然点不动**。
 * gallery 入口的模板是磁盘文件、没入库 ⇒ `saved` 为 null，而 `render()` 第一句就
 * 要求 `saved`，于是装上来的运行结果永远显示不出来，状态栏只说
 * 「请从模板库选择报表」。按钮因 `printWorkbook` 非空而**可点**，比 disabled
 * 更坑：点了像没反应。原判据只点了分段控件的「结果」（那条路 onClick 也会
 * attachSnapshot，所以一直是绿的），工具栏这条路没人测。
 *
 * 依赖 `bash scripts/build_r_model_gallery.sh` 的产物（`web/public/report-gallery/`
 * 不入库）；没有产物时跳过，不让 CI 因为「没跑生成脚本」变红。
 */
const BASE = '/report-gallery/run-1792293895313098000/insurance-policy'

test.beforeAll(async ({ request }) => {
  const response = await request.get(`${BASE}/workbook.json`)
  test.skip(!response.ok(), '示例尚未生成：先跑 bash scripts/build_r_model_gallery.sh')
})

test('示例入口打开后结果视图可用，且装载同一运行的 workbook', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(String(error)))
  let workbookBody = ''
  page.on('response', async response => {
    if (response.url().endsWith(`${BASE}/workbook.json`)) workbookBody = await response.text()
  })

  await page.goto(`/?gallery=${encodeURIComponent(BASE)}`)

  // 重构后视图切换是「设计 / 结果」分段控件里的「结果」按钮。
  const resultTab = page.getByRole('button', { name: '结果', exact: true })
  await expect(resultTab).toBeEnabled({ timeout: 20_000 })
  // 装载的确是这份示例的运行结果（标题在 workbook.json 里），不是别的报表
  expect(workbookBody).toContain('财产基本险')

  await resultTab.click()
  await expect(resultTab).toHaveAttribute('aria-pressed', 'true')
  expect(errors).toEqual([])
})

test('工具栏「查看结果」真的把运行结果装上画布（不只切换视图）', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(String(error)))

  await page.goto(`/?gallery=${encodeURIComponent(BASE)}`)
  const status = page.locator('.rpx-status')

  // 走工具栏这条路（分段控件的「结果」只是切 view，装载靠它才真的发生）。
  await page.getByRole('button', { name: '查看结果' }).click({ timeout: 20_000 })
  await expect(page.getByRole('button', { name: '结果', exact: true }))
    .toHaveAttribute('aria-pressed', 'true')
  // 不得退化成「请从模板库选择报表」—— 那是 saved 为 null 时的旧拒绝口径
  await expect(status).not.toContainText('请从模板库选择报表', { timeout: 10_000 })

  // 打印预览是 DOM 渲染（Univer 画布是 canvas，innerText 读不到），
  // 所以拿它证明「画布上装的确实是这份运行结果」而不是模板格。
  await page.getByRole('button', { name: '打印预览' }).click({ timeout: 20_000 })
  const preview = page.getByRole('dialog').or(page.locator('.print-preview, [class*=preview]').first())
  await expect(preview.first()).toContainText('财产基本险', { timeout: 20_000 })
  await expect(preview.first()).toContainText('第一部分', { timeout: 20_000 })
  // 合计行由明细汇总而来（213123+231231+123123+23123）；写死这个数是防
  // 「换了一份 workbook 但页面照样显示」的串档
  await expect(preview.first()).toContainText('1,049,145.00', { timeout: 20_000 })
  expect(errors).toEqual([])
})
