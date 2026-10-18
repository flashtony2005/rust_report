import { expect, test } from '@playwright/test'

/**
 * 界面骨架重构的回归锁。
 *
 * 重构前的实测病灶（这三条判据一一对应）：
 * 1. 顶部有 6 层横栏（45+47+47+31+两个折叠块 = 约 170px），菜单栏排在**最后**
 *    一行、还在状态文案下面；状态消息独占一行夹在中间。
 * 2. 同一动作有 3~4 个入口且名字互相串：header「保存共享定义」/ 菜单「保存」/
 *    菜单「另存为」都调同一个 `save()`；「查看保存的结果」与「运行结果」语义互换。
 * 3. 右侧面板七段平铺在一列里，单元格属性与报表属性混排；「报表」菜单的六项
 *    全是 `setSideVisible(true)` + 一句状态文案，面板不滚动也不高亮（假跳转）。
 *
 * 依赖模板库有报表；库为空时跳过。
 */
test.beforeAll(async ({ request }) => {
  const response = await request.get('/api/reports?limit=500')
  const body = response.ok() ? await response.json().catch(() => ({})) : {}
  test.skip(!(body as { items?: unknown[] }).items?.length, '模板库为空：先跑 rpx library init --presets')
})

test('菜单栏置顶，顶部压成三条，状态栏落底', async ({ page }) => {
  await page.goto('/')

  const menubar = page.locator('[role=menubar]')
  const toolbar = page.locator('.rpx-toolbar')
  const context = page.locator('.rpx-context')
  const status = page.locator('.rpx-status')

  await expect(menubar).toBeVisible()
  await expect(toolbar).toBeVisible()
  await expect(context).toBeVisible()
  await expect(status).toBeVisible()

  // 顺序：菜单 → 工具条 → 上下文条；状态栏在所有主体之后
  const ys = await Promise.all([menubar, toolbar, context, status].map(async l => (await l.boundingBox())?.y ?? -1))
  expect(ys[0]).toBeLessThan(ys[1])
  expect(ys[1]).toBeLessThan(ys[2])
  expect(ys[3]).toBeGreaterThan(ys[2])

  // 顶部只剩三条横栏（重构前是 4 条 + 2 个折叠块）
  expect(await page.locator('.template-toolbar').count()).toBe(0)

  // 旧顶栏的重复入口已收进菜单：页面上不再有常驻的「保存共享定义」「保存模板 JSON」
  await expect(page.getByRole('button', { name: '保存共享定义' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '保存模板 JSON' })).toHaveCount(0)
})

test('属性面板双页签按对象分层', async ({ page }) => {
  await page.goto('/')

  const cellGroups = await page.locator('.rpx-props-body [data-group]').evaluateAll(nodes => nodes.map(n => n.getAttribute('data-group')))
  expect(cellGroups).toEqual(['content', 'font', 'border', 'rowcol'])

  await page.getByRole('tab', { name: '报表' }).click()
  const reportGroups = await page.locator('.rpx-props-body [data-group]').evaluateAll(nodes => nodes.map(n => n.getAttribute('data-group')))
  expect(reportGroups).toEqual(['page', 'paginate', 'grid', 'params', 'datasets'])
})

test('报表菜单的分组跳转真的落到分组（切页签 + 高亮）', async ({ page }) => {
  await page.goto('/')
  // 先停在「单元格」页签，跳转必须自动切到「报表」
  await expect(page.getByRole('tab', { name: '单元格' })).toHaveAttribute('aria-selected', 'true')

  await page.getByRole('menuitem', { name: '报表', exact: true }).click()
  await page.getByRole('menuitem', { name: '分页与版面' }).click()

  await expect(page.getByRole('tab', { name: '报表' })).toHaveAttribute('aria-selected', 'true')
  await expect(page.locator('.rpx-props-body [data-group=paginate]')).toHaveClass(/flash/, { timeout: 3000 })
  await expect(page.locator('.rpx-status .msg')).toContainText('分页与版面')
})

test('底部抽屉承接定义源码与运行诊断', async ({ page }) => {
  await page.goto('/')
  const source = page.getByLabel('JSON 定义源码')
  await expect(source).toBeHidden()

  await page.getByRole('button', { name: /^定义源码/ }).click()
  await expect(source).toBeVisible()

  await page.getByRole('button', { name: /^运行诊断/ }).click()
  await expect(source).toBeHidden()
  await expect(page.locator('.rpx-drawer-body').nth(1)).toBeVisible()
})
