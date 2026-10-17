import { useEffect, useId, useRef, useState } from 'react'
import runtime from 'echarts/dist/echarts.min.js?url'

/** 模板体在独立文档执行；页面代码与报表脚本不共享 DOM 或存储。 */
export default function ScriptChart({source,width,height}:{source:string;width:number;height:number}){
  const ref=useRef<HTMLIFrameElement>(null),token=useId(),[error,setError]=useState('')
  useEffect(()=>{
    const listener=(event:MessageEvent)=>{
      if(event.source===ref.current?.contentWindow&&event.data?.token===token&&event.data?.error)setError(String(event.data.error))
    }
    window.addEventListener('message',listener);return()=>window.removeEventListener('message',listener)
  },[token])
  const script=(value:string)=>JSON.stringify(value).replaceAll('<','\\u003c')
  const runtimeUrl=new URL(runtime,window.location.href).href
  const document=`<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline' 'unsafe-eval' ${window.location.origin}; style-src 'unsafe-inline'; img-src data:"><style>html,body{margin:0;width:100%;height:100%;overflow:hidden}</style></head><body><div id="chart" style="width:${width}px;height:${height}px"></div><script src="${runtimeUrl}"></script><script>try{const container=document.getElementById('chart');const chart=echarts.init(container,undefined,{renderer:'svg',width:${width},height:${height}});const result=new Function('chart','container','width','height',${script(source)})(chart,container,${width},${height});if(result)chart.setOption({...result,animation:false});}catch(error){parent.postMessage({token:${script(token)},error:String(error)},'*')}</script></body></html>`
  return <><iframe ref={ref} title="ECharts JavaScript 模板" sandbox="allow-scripts" srcDoc={document} width={width} height={height} style={{border:0}}/>{error&&<p role="alert">模板生成失败：{error}</p>}</>
}
