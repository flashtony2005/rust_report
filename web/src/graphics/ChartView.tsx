import { useEffect, useRef, useState } from 'react'
import * as echarts from 'echarts'
import { chartOption, type GraphicContent } from '../graphics'
import ScriptChart from './ScriptChart'

/**
 * 单个图形的渲染器（统计图 / ECharts option / ECharts JS / 图片）。
 *
 * # 为什么单独成文件
 *
 * 打印预览与报表实例库都要画同一批图形，但两者是**不同页面**：
 * 打印预览按分页 HTML 表格叠绝对定位，实例库按卡片列。渲染逻辑只有这一份，
 * 布局各自负责 —— 复制一份必然分叉（改了一处忘了另一处）。
 *
 * ⚠ `content.kind` 决定走哪条路，**四种必须全覆盖**：
 * `chart`（28 种内置统计图，`chartOption` 翻译成 ECharts option）、
 * `echarts`（后端算好的 option JSON）、`echarts_js`（JS 模板，沙箱 iframe）、
 * `image` / `cell_background`（data URI 图片）。
 * 漏一种的后果是**空白**而不是报错，所以这里不设默认分支。
 */
export default function ChartView({
  content,
  width,
  height,
}: {
  content: GraphicContent
  width: number
  height: number
}) {
  const ref = useRef<HTMLDivElement>(null)
  const [error, setError] = useState('')
  useEffect(() => {
    if (!ref.current || (content.kind !== 'chart' && content.kind !== 'echarts')) return
    const chart = echarts.init(ref.current, undefined, { renderer: 'svg', width, height })
    try {
      chart.setOption(
        (content.kind === 'chart' ? chartOption(content) : { ...content.option, animation: false }) as echarts.EChartsOption,
      )
      setError('')
    } catch (error) {
      setError(`图形生成失败：${String(error)}`)
    }
    return () => chart.dispose()
  }, [content, width, height])
  if (content.kind === 'image' || content.kind === 'cell_background')
    return <img src={content.source} width={width} height={height} alt="报表图片" style={{ objectFit: 'contain' }} />
  if (content.kind === 'echarts_js') return <ScriptChart source={content.source} width={width} height={height} />
  return (
    <>
      <div ref={ref} style={{ width, height }} />
      {error && <p role="alert">{error}</p>}
    </>
  )
}
