import { intervalOption, rangeOption } from './graphics/intervals'
import { cartesian3d, pie3d } from './graphics/three'
export const CHART_TYPES=['column','column_3d','clustered_column_3d','stacked_column','stacked_column_3d','pie','pie_3d','line','curve','line_3d','area','area_3d','bar','clustered_bar_3d','stacked_bar','stacked_bar_3d','scatter','column_line','dual_line','time_series','state_timeline','gantt','gauge','gauge_3d','radar','milestone','error_bar','range'] as const
export interface ChartModel {
  kind:'chart';type:string;title:string;categories:Array<string|number|null>
  series:Array<{name:string;values:Array<number|null>}>
  tasks?:{categories:Array<string|number|null>;starts:Array<string|number|null>;ends:Array<string|number|null>;states:Array<string|number|null>}|null
  appearance?:{colors:string[];legend:string;background:string;margins:number[];pieRotation:number;pieDepth:number}
}
export type GraphicContent=ChartModel|{kind:'echarts';option:Record<string,unknown>}|{kind:'echarts_js';source:string}|{kind:'image'|'cell_background';source:string}
export interface Graphic{row:number;col:number;width:number;height:number;content:GraphicContent}
const PALETTE=['#5470c6','#91cc75','#fac858','#ee6666','#73c0de','#3ba272','#fc8452','#9a60b4','#ea7ccc']
export function chartOption(model:ChartModel):Record<string,unknown>{
  if(!(CHART_TYPES as readonly string[]).includes(model.type))throw new Error(`未实现的图形：${model.type}`)
  const a=model.appearance,colors=a?.colors.length?a.colors:PALETTE,legend=a?.legend??'bottom',margins=a?.margins??[40,40,40,40]
  const base={animation:false,color:colors,backgroundColor:a?.background??'transparent',title:{text:model.title},tooltip:{renderMode:'richText'},legend:{show:legend!=='none',orient:['left','right'].includes(legend)?'vertical':'horizontal',[legend]:0},grid:{top:margins[0],right:margins[1],bottom:margins[2],left:margins[3],containLabel:true}}
  if(['state_timeline','gantt','milestone'].includes(model.type))return{...base,...intervalOption(model,colors)}
  if(['error_bar','range'].includes(model.type))return{...base,...rangeOption(model)}
  if(model.type==='pie_3d')return{...base,...pie3d(model,colors),legend:{show:false}}
  if(model.type.endsWith('_3d')&&model.type!=='gauge_3d')return{...base,...cartesian3d(model,colors)}
  if(model.type==='pie')return{...base,series:[{type:'pie',radius:'65%',startAngle:a?.pieRotation??90,data:model.categories.map((name,i)=>({name,value:model.series[0].values[i]}))}]}
  if(['gauge','gauge_3d'].includes(model.type))return{...base,series:[{type:'gauge',...(model.type==='gauge_3d'?{axisLine:{lineStyle:{width:24,color:[[1,{type:'radial',x:.5,y:.5,r:.6,colorStops:[{offset:0,color:'#eee'},{offset:.6,color:colors[0]},{offset:1,color:'#222'}]}]],shadowColor:'#555',shadowBlur:5,shadowOffsetY:6}},pointer:{itemStyle:{color:colors[0],shadowBlur:5,shadowOffsetY:4}}}:{}),data:[{name:model.series[0].name,value:model.series[0].values[0]}]}]}
  if(model.type==='radar')return{...base,radar:{indicator:model.categories.map(name=>({name:String(name??'')}))},series:[{type:'radar',data:model.series.map(s=>({name:s.name,value:s.values}))}]}
  const horizontal=['bar','stacked_bar'].includes(model.type),stacked=['stacked_bar','stacked_column'].includes(model.type),dual=['column_line','dual_line'].includes(model.type),time=model.type==='time_series',scatter=model.type==='scatter'
  const categoryAxis={type:'category',data:model.categories},numericAxis={type:'value'}
  return{...base,xAxis:horizontal?numericAxis:time?{type:'time'}:scatter?numericAxis:categoryAxis,yAxis:horizontal?categoryAxis:dual?[numericAxis,numericAxis]:numericAxis,
    series:model.series.map((s,i)=>({name:s.name,type:scatter?'scatter':['line','curve','area','dual_line','time_series'].includes(model.type)||(model.type==='column_line'&&i>0)?'line':'bar',data:time||scatter?s.values.map((v,n)=>[model.categories[n],v]):s.values,...(stacked?{stack:'total'}:{}),...(model.type==='curve'?{smooth:true}:{}),...(model.type==='area'?{areaStyle:{}}:{}),...(dual?{yAxisIndex:i>0?1:0}:{})}))}
}
