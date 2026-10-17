import { expect, it } from 'vitest'
import { printValue } from './print-value'

it('打印与画布使用相同百分比、金额格式，文字和零基期提示不变', () => {
  expect(printValue(1949.5 / 2200, '0.00%')).toBe('88.61%')
  expect(printValue(3399.5, '#,##0.00')).toBe('3,399.50')
  expect(printValue('基期为零', '0.00%')).toBe('基期为零')
  expect(printValue(null, '0.00%')).toBe('')
})
