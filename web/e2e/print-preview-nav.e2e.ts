import { expect, test } from '@playwright/test'

/**
 * 打印预览的页导航 + 与结果视图的相关性。
 *
 * 为什么需要它：
 * 1. 预览把所有 sheet 纵向堆叠，没有导航时翻到第 20 页只能靠滚轮——
 *    跳转下拉 + 上/下一页 + 位置指示是快速通道，三者状态必须一致。
 * 2. 多 sheet 时预览必须与结果视图**相关**：从第 5 个 sheet 打开预览
 *    就要落在第 5 页，否则上下文丢失（又回到第 1 页）。
 *
 * 依赖模板库里的多页报表（chart-gallery，29 页）；库为空时跳过。
 */
test.beforeAll(async ({ request }) => {
  const response = await request.get('/api/reports?limit=500')
  const body = response.ok() ? await response.json().catch(() => ({})) : {}
  test.skip(!(body as { items?: unknown[] }).items?.length, '模板库为空：先跑 rpx library init --presets')
})

test('打印预览页导航与结果视图的相关性', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(String(error)))
  await page.goto('/')
  // 初始是设计视图（模板）；进入结果视图才有多个 sheet 标签
  await page.getByRole('button', { name: '运行结果' }).click()
  await page.getByRole('button', { name: '图形与打印预览' }).click()

  const preview = page.locator('.rpx-print-preview')
  await expect(preview).toBeVisible()
  const position = preview.locator('.rpx-print-controls > span')
  const total = Number(/\/\s*(\d+)\s*页/.exec((await position.textContent()) ?? '')?.[1] ?? 0)
  test.skip(total <= 1, '当前报表只有 1 页，无需导航')

  // —— 导航：上/下一页、跳转下拉、位置指示三者一致 ——
  await expect(position).toHaveText(`第 1 / ${total} 页`)
  await expect(preview.getByRole('button', { name: '上一页' })).toBeDisabled()

  await preview.getByRole('button', { name: '下一页' }).click()
  await expect(position).toHaveText(`第 2 / ${total} 页`)

  await preview.getByLabel('跳转到页').selectOption(String(total - 1))
  await expect(position).toHaveText(`第 ${total} / ${total} 页`)
  await expect(preview.getByRole('button', { name: '下一页' })).toBeDisabled()

  await preview.getByRole('button', { name: '上一页' }).click()
  await expect(position).toHaveText(`第 ${total - 1} / ${total} 页`)

  // —— 相关性：结果视图切到第 N 个 sheet，重开预览落在第 N 页 ——
  const target = Math.min(4, total - 1)
  await preview.getByRole('button', { name: '关闭' }).click()
  await expect(preview).toBeHidden()
  await page.getByText(`第 ${target + 1} 页`, { exact: true }).first().click()
  await page.getByRole('button', { name: '图形与打印预览' }).click()
  await expect(position).toHaveText(`第 ${target + 1} / ${total} 页`)
  expect(errors).toEqual([])
})
