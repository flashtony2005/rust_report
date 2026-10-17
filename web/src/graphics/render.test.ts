import { describe, expect, it } from 'vitest'
import * as echarts from 'echarts'
import { CHART_TYPES, chartOption, type ChartModel } from '../graphics'
const model:ChartModel={kind:'chart',type:'column',title:'Chart',categories:[1,2,3],series:[{name:'Low',values:[1,2,3]},{name:'High',values:[4,5,6]}]}
describe('28 chart types render actual vector geometry',()=>{
  for(const type of CHART_TYPES)it(type,()=>{
    const chart=echarts.init(null,undefined,{renderer:'svg',ssr:true,width:480,height:300})
    const m={...model,type}
    if(['gauge','gauge_3d'].includes(type)){m.series=[{name:'Value',values:[40]}];m.categories=[1]}
    if(['pie','pie_3d'].includes(type))m.series=[model.series[0]]
    if(['state_timeline','gantt','milestone'].includes(type))Object.assign(m,{tasks:{categories:['A','B'],starts:[0,1000],ends:[1500,3000],states:['Busy','Free']}})
    try{
      chart.setOption(chartOption(m) as echarts.EChartsOption)
      const svg=chart.renderToSVGString()
      expect(svg).toContain('<svg')
      expect(svg.match(/<(?:path|polygon|rect|circle)\b/g)?.length??0).toBeGreaterThan(2)
      expect(svg).not.toContain('NaN')
      expect(svg).not.toContain('Infinity')
    }finally{chart.dispose()}
  })
  it('rejects an inverted task interval',()=>{
    expect(()=>chartOption({...model,type:'gantt',tasks:{categories:['A'],starts:[2000],ends:[1000],states:['Busy']}})).toThrow('结束时间')
  })
})
