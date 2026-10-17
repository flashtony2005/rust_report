import type { CustomSeriesRenderItem } from 'echarts'
import type { ChartModel } from '../graphics'

function time(value: string | number | null): number {
  const n = typeof value === 'number' ? value : Date.parse(String(value))
  if (!Number.isFinite(n)) throw new Error(`无效时间：${String(value)}`)
  return n
}
export function intervalOption(model: ChartModel, colors: string[]): Record<string, unknown> {
  if (!model.tasks) throw new Error('缺少任务数据')
  const t = model.tasks
  const categories = [...new Set(t.categories.map(String))]
  const states = [...new Set(t.states.map(String))]
  const milestone = model.type === 'milestone'
  const renderItem: CustomSeriesRenderItem = (_params, api) => {
    const a = api.coord!([Number(api.value(1)), Number(api.value(0))])
    const b = api.coord!([Number(api.value(2)), Number(api.value(0))])
    const h = Math.max(2, Math.abs((api.size!([0, 1]) as number[])[1]) * .6)
    const fill = colors[Number(api.value(3)) % colors.length]
    return milestone ? { type: 'polygon', shape: { points: [[a[0],a[1]-h/2],[a[0]+h/2,a[1]],[a[0],a[1]+h/2],[a[0]-h/2,a[1]]] }, style: { fill } }
      : { type: 'rect', shape: { x: a[0], y: a[1]-h/2, width: Math.max(2,b[0]-a[0]), height: h }, style: { fill } }
  }
  const data = t.categories.map((c,i) => {
    const start=time(t.starts[i]), end=time(t.ends[i])
    if (end < start) throw new Error('结束时间不能早于开始时间')
    return [categories.indexOf(String(c)), start, end, states.indexOf(String(t.states[i]))]
  })
  return { xAxis:{type:'time'}, yAxis:{type:'category',data:categories,inverse:true},
    series:states.map((state,i)=>({name:state || '任务',type:'custom',renderItem,encode:{x:[1,2],y:0,tooltip:[1,2]},data:data.filter(v=>v[3]===i)})) }
}

export function rangeOption(model:ChartModel):Record<string,unknown>{
  const renderItem:CustomSeriesRenderItem=(_params,api)=>{
    const low=api.coord!([Number(api.value(0)),Number(api.value(1))]), high=api.coord!([Number(api.value(0)),Number(api.value(2))])
    const mid=api.coord!([Number(api.value(0)),Number(api.value(3))]), w=Math.abs((api.size!([1,0]) as number[])[0])*.35
    const stroke=String(api.visual('color'))
    const line=(x1:number,y1:number,x2:number,y2:number)=>({type:'line' as const,shape:{x1,y1,x2,y2},style:{stroke,lineWidth:2}})
    return {type:'group',children:model.type==='range' ? [
      {type:'rect',shape:{x:low[0]-w/2,y:high[1],width:w,height:low[1]-high[1]},style:{fill:stroke,opacity:.4}},line(mid[0]-w/2,mid[1],mid[0]+w/2,mid[1]),
    ] : [line(low[0],low[1],high[0],high[1]),line(low[0]-w/2,low[1],low[0]+w/2,low[1]),line(high[0]-w/2,high[1],high[0]+w/2,high[1]),{type:'circle',shape:{cx:mid[0],cy:mid[1],r:3},style:{fill:stroke}}]}
  }
  const data=model.categories.flatMap((_c,i)=>{
    const low=model.series[0].values[i],high=model.series[1].values[i]
    return low==null||high==null?[]:[[i,low,high,model.series[2]?.values[i]??(low+high)/2]]
  })
  return {xAxis:{type:'category',data:model.categories},yAxis:{type:'value'},series:[{type:'custom',renderItem,encode:{x:0,y:[1,2,3],tooltip:[1,2,3]},data}]}
}
