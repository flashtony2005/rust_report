import { describe, expect, it } from 'vitest'
import { isReportSource, pickReportFile } from './report'
const file = (name: string) => new File([], name)

describe('JSON source selection', () => {
 it('opens JSON source and leaves RPX to explicit CLI import', () => {
  expect(pickReportFile([file('a.rpx'),file('b.JSON')])?.name).toBe('b.JSON')
  expect(pickReportFile([file('a.rpx')])).toBeUndefined()
  expect(isReportSource('x.rpx')).toBe(false)
  expect(isReportSource('x.json')).toBe(true)
  expect(isReportSource('x.json.csv')).toBe(false)
 })
})

