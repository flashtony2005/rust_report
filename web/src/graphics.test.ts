import { describe, expect, it } from 'vitest'
import { chartOption, type ChartModel } from './graphics'
const model: ChartModel = { kind: 'chart', type: 'column', title: '金额', categories: ['A', 'B'], series: [{ name: '金额', values: [10, 20] }, { name: '占比', values: [1, 2] }] }
describe('图形数据到 ECharts 配置', () => {
  it('双轴柱线保留原始值和每个系列的轴', () => {
    const option = chartOption({ ...model, type: 'column_line' })
    expect(option.series).toMatchObject([{ type: 'bar', data: [10, 20], yAxisIndex: 0 }, { type: 'line', data: [1, 2], yAxisIndex: 1 }])
  })
  it('条形图交换分类轴且堆积系列共用一组', () => {
    const option = chartOption({ ...model, type: 'stacked_bar' })
    expect(option.yAxis).toEqual({ type: 'category', data: ['A', 'B'] })
    expect(option.series).toMatchObject([{ stack: 'total' }, { stack: 'total' }])
  })
  it('散点和时序使用真实横坐标', () => {
    for (const type of ['scatter', 'time_series']) {
      expect((chartOption({ ...model, type }).series as Array<unknown>)[0]).toMatchObject({ data: [['A', 10], ['B', 20]] })
    }
  })
  it('每个雷达系列共享同一分类顺序', () => {
    const option = chartOption({ ...model, type: 'radar' })
    expect(option.radar).toEqual({ indicator: [{ name: 'A' }, { name: 'B' }] })
    expect(option.series).toMatchObject([{ data: [{ name: '金额', value: [10, 20] }, { name: '占比', value: [1, 2] }] }])
  })
})
