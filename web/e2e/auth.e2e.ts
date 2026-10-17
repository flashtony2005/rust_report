import { expect, test } from '@playwright/test'

test('auth required: login then enter the library', async ({ page }) => {
  await page.route('**/api/auth/status', (route) =>
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ required: true }) }),
  )
  await page.route('**/api/reports**', (route) =>
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ items: [], total: 0 }) }),
  )

  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'RPX 报表库' })).toBeVisible()
  await page.getByPlaceholder('Access token').fill('secret-token')
  await page.getByRole('button', { name: '进入报表库' }).click()
  await expect(page.getByLabel('报表库')).toBeVisible()
})

test('no auth: goes straight to the library', async ({ page }) => {
  await page.route('**/api/auth/status', (route) =>
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ required: false }) }),
  )
  await page.route('**/api/reports**', (route) =>
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ items: [], total: 0 }) }),
  )

  await page.goto('/')
  await expect(page.getByLabel('报表库')).toBeVisible()
})
