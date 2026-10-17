/** 固定斜投影的三维几何，使用 ECharts SVG custom series，打印时保留矢量。 */
import { color, type CustomSeriesRenderItem } from 'echarts'
import type { ChartModel } from '../graphics'
const shade=(c:string,n:number)=>color.lift(c,n)
const polygon=(points:number[][],fill:string)=>({type:'polygon' as const,shape:{points},style:{fill,stroke:fill}})

export function pie3d(model:ChartModel,colors:string[]):Record<string,unknown>{
  const values=model.series[0].values.map(v=>Math.max(0,v??0)),total=values.reduce((a,b)=>a+b,0)
  let start=(model.appearance?.pieRotation??0)*Math.PI/180
  const data=values.map((v,i)=>{const a=start;start+=total?v/total*Math.PI*2:0;return [i,a,start]})
  const renderItem:CustomSeriesRenderItem=(_params,api)=>{
    const i=Number(api.value(0)),a=Number(api.value(1)),b=Number(api.value(2)),r=Math.min(api.getWidth()*.36,api.getHeight()*.36)
    const cx=api.getWidth()/2,cy=api.getHeight()*.48,depth=model.appearance?.pieDepth??16,fill=colors[i%colors.length]
    const point=(angle:number,z=0)=>[cx+Math.cos(angle)*r,cy+Math.sin(angle)*r*.6+z]
    const top:number[][]=[[cx,cy]],children:ReturnType<typeof polygon>[]=[]
    const steps=Math.max(2,Math.ceil((b-a)*32))
    for(let n=0;n<=steps;n++)top.push(point(a+(b-a)*n/steps))
    for(let n=0;n<steps;n++){
      const x=a+(b-a)*n/steps,y=a+(b-a)*(n+1)/steps
      if(Math.sin((x+y)/2)>0)children.push(polygon([point(x),point(y),point(y,depth),point(x,depth)],shade(fill,-.3)))
    }
    children.push(polygon(top,fill))
    return {type:'group',children}
  }
  return {series:[{type:'custom',coordinateSystem:'none',renderItem,data,tooltip:{show:false}}],graphic:model.categories.map((c,i)=>({type:'text',left:12,top:38+i*18,style:{text:String(c??''),fill:colors[i%colors.length],fontSize:11}}))}
}

export function cartesian3d(model:ChartModel,colors:string[]):Record<string,unknown>{
  const horizontal=['clustered_bar_3d','stacked_bar_3d'].includes(model.type),stacked=model.type.startsWith('stacked'),clustered=model.type.startsWith('clustered')
  const line=model.type==='line_3d'||model.type==='area_3d',area=model.type==='area_3d'
  const positive=model.categories.map(()=>0),negative=model.categories.map(()=>0)
  const series=model.series.map((s,si)=>{
    const fill=colors[si%colors.length],depth=clustered||stacked?8:8+si*7
    const data=s.values.flatMap((v,i)=>{
      if(v==null)return []
      const base=stacked?(v>=0?positive[i]:negative[i]):0
      if(stacked){if(v>=0)positive[i]+=v;else negative[i]+=v}
      return [[i,base,base+v]]
    })
    const renderItem:CustomSeriesRenderItem=(_params,api)=>{
      const category=Number(api.value(0)),low=Number(api.value(1)),high=Number(api.value(2))
      const from=api.coord!(horizontal?[low,category]:[category,low]),to=api.coord!(horizontal?[high,category]:[category,high])
      const band=Math.abs((api.size!(horizontal?[0,1]:[1,0]) as number[])[horizontal?1:0])*.6
      const width=clustered?band/model.series.length:band,offset=clustered?-band/2+width*(si+.5):0
      const p=(x:number,y:number,z=0)=>[x+z,y-z*.5]
      if(line){
        const index=data.findIndex(v=>v[0]===category),prev=data[index-1]
        if(!prev)return null
        const a=api.coord!([prev[0],prev[2]]),b=api.coord!([category,high]),baseA=api.coord!([prev[0],0]),baseB=api.coord!([category,0])
        const points=area?[p(a[0],a[1],depth),p(b[0],b[1],depth),p(baseB[0],baseB[1],depth),p(baseA[0],baseA[1],depth)]:[p(a[0],a[1]),p(b[0],b[1]),p(b[0],b[1],depth),p(a[0],a[1],depth)]
        return polygon(points,fill)
      }
      const x0=horizontal?Math.min(from[0],to[0]):from[0]+offset-width/2,x1=horizontal?Math.max(from[0],to[0]):from[0]+offset+width/2
      const y0=horizontal?from[1]+offset-width/2:Math.min(from[1],to[1]),y1=horizontal?from[1]+offset+width/2:Math.max(from[1],to[1])
      return {type:'group',children:[polygon([p(x0,y0),p(x1,y0),p(x1,y1),p(x0,y1)],fill),polygon([p(x1,y0),p(x1,y0,depth),p(x1,y1,depth),p(x1,y1)],shade(fill,-.25)),polygon([p(x0,y0),p(x0,y0,depth),p(x1,y0,depth),p(x1,y0)],shade(fill,.25))]}
    }
    return {name:s.name,type:'custom',renderItem,encode:horizontal?{x:[1,2],y:0}:{x:0,y:[1,2]},data}
  })
  const categories={type:'category',data:model.categories},values={type:'value'}
  return {xAxis:horizontal?values:categories,yAxis:horizontal?categories:values,series}
}
