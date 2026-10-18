import { useEffect, useRef, useState, type CSSProperties } from 'react'
import { createPortal } from 'react-dom'
import type { Graphic } from './graphics'
import ChartView from './graphics/ChartView'
import { printValue } from './print-value'

interface Cell {
  t?:number
  v?: unknown
  s?: { ff?: string; fs?: number; bl?: number; it?: number; cl?: { rgb: string }; bg?: { rgb: string }; ht?: number; vt?: number; n?: { pattern: string } }
}
interface Sheet {
  name: string
  printRowCount: number; printColumnCount: number
  defaultRowHeight: number; defaultColumnWidth: number
  rowData: Record<string, { h: number; hd?: number }>
  columnData: Record<string, { w: number; hd?: number }>
  cellData: Record<string, Record<string, Cell>>
  mergeData: Array<{ startRow: number; endRow: number; startColumn: number; endColumn: number }>
  rpxGraphics?: Graphic[]
  pageBackground?: string
  printSize: { width: number; height: number; margins: number[] }
  printOptions?:{autoSize:boolean;wrapNumbers:boolean;borderWidth:number;borderColor:string;overlay:boolean}
}
export interface PrintWorkbook { sheetOrder: string[]; sheets: Record<string, Sheet> }

function cellStyle(cell?: Cell): CSSProperties {
  const s = cell?.s
  return { fontFamily: s?.ff, fontSize: s?.fs, fontWeight: s?.bl ? 'bold' : 'normal', fontStyle: s?.it ? 'italic' : 'normal', color: s?.cl?.rgb, backgroundColor: s?.bg?.rgb,
    textAlign: s?.ht === 2 ? 'center' : s?.ht === 3 ? 'right' : 'left', verticalAlign: s?.vt === 1 ? 'top' : s?.vt === 3 ? 'bottom' : 'middle', whiteSpace: 'pre-wrap', overflow: 'hidden', padding: 0 }
}

export default function PrintPreview({ workbook, onClose }: { workbook: PrintWorkbook; onClose: () => void }) {
  const [mirror,setMirror]=useState(false)
  const pagesRef=useRef<HTMLDivElement>(null)
  const first = workbook.sheets[workbook.sheetOrder[0]]
  useEffect(()=>{
    const canvas=document.createElement('canvas'),ctx=canvas.getContext('2d')
    if(!ctx)return
    for(const cell of pagesRef.current?.querySelectorAll<HTMLTableCellElement>('td[data-auto-size=true]')??[]){
      const style=getComputedStyle(cell),size=parseFloat(style.fontSize)
      ctx.font=`${style.fontWeight} ${size}px ${style.fontFamily}`
      const width=Math.max(...(cell.textContent??'').split('\n').map(line=>ctx.measureText(line).width))
      const ratio=Math.min(1,cell.clientWidth/Math.max(1,width))
      cell.style.fontSize=`${size*ratio}px`
    }
  },[workbook])
  return createPortal(<div className="rpx-print-preview" role="dialog" aria-label="图形与打印预览">
    <style>{`
      .rpx-print-preview{position:fixed;inset:0;z-index:10000;overflow:auto;background:#e8eaed;color:#111;font-family:system-ui,sans-serif}
      .rpx-print-controls{position:sticky;top:0;padding:12px;background:#fff;border-bottom:1px solid #ccc;z-index:10;display:flex;gap:16px;align-items:center}
      .rpx-print-page{position:relative;margin:20px auto;background-color:white;background-size:100% 100%;box-shadow:0 1px 8px #999;box-sizing:border-box;overflow:hidden;break-after:page}
      .rpx-print-page:last-child{break-after:auto}
      @page{size:${first.printSize.width}mm ${first.printSize.height}mm;margin:0}
      @media print{#app,.rpx-print-controls{display:none!important}.rpx-print-preview{position:static;overflow:visible;background:white}.rpx-print-page{margin:0;box-shadow:none;print-color-adjust:exact;-webkit-print-color-adjust:exact}}
    `}</style>
    <div className="rpx-print-controls"><strong>图形与打印预览</strong><span>{workbook.sheetOrder.length} 页</span><label><input type="checkbox" checked={mirror} onChange={e=>setMirror(e.target.checked)}/>镜像打印</label><button onClick={() => window.print()}>打印 / 保存为 PDF</button><button onClick={onClose}>关闭</button></div>
    <div ref={pagesRef}>
    {workbook.sheetOrder.map(id => {
      const s = workbook.sheets[id]
      const widths = Array.from({ length: s.printColumnCount }, (_, c) => s.columnData[c]?.hd ? 0 : s.columnData[c]?.w ?? s.defaultColumnWidth)
      const heights = Array.from({ length: s.printRowCount }, (_, r) => s.rowData[r]?.hd ? 0 : s.rowData[r]?.h ?? s.defaultRowHeight)
      const graphics = (s.rpxGraphics ?? []).filter(g=>widths[g.col]>0&&heights[g.row]>0)
      const options=s.printOptions
      const margins = s.printSize.margins
      const contentWidth = Math.max(widths.reduce((a,b)=>a+b,0), ...graphics.map(g=>widths.slice(0,g.col).reduce((a,b)=>a+b,0)+g.width))
      const contentHeight = Math.max(heights.reduce((a,b)=>a+b,0), ...graphics.map(g=>heights.slice(0,g.row).reduce((a,b)=>a+b,0)+g.height))
      const scale = Math.min(1, (s.printSize.width-margins[1]-margins[3])*96/25.4/Math.max(1,contentWidth), (s.printSize.height-margins[0]-margins[2])*96/25.4/Math.max(1,contentHeight))
      const pageBackground=options?.overlay?s.pageBackground:undefined
      return <section key={id} className="rpx-print-page" aria-label={s.name} style={{ width: `${s.printSize.width}mm`, height: `${s.printSize.height}mm`, padding: margins.map(m => `${m}mm`).join(' '), backgroundImage: pageBackground ? `url(${pageBackground})` : undefined,transform:mirror?'scaleX(-1)':undefined }}>
        <div style={{ position: 'relative', transform:`scale(${scale})`, transformOrigin:'top left' }}>
          <table style={{ borderCollapse: 'collapse', tableLayout: 'fixed', width: widths.reduce((a, b) => a + b, 0) }}>
            <colgroup>{widths.map((w, i) => <col key={i} style={{ width: w, visibility: w ? undefined : 'collapse' }} />)}</colgroup>
            <tbody>{heights.map((h, r) => <tr key={r} style={{ height: h, display: h ? undefined : 'none' }}>{widths.map((w, c) => {
              const merge = s.mergeData.find(m => r >= m.startRow && r <= m.endRow && c >= m.startColumn && c <= m.endColumn)
              if (merge && (r !== merge.startRow || c !== merge.startColumn)) return null
              const cell = s.cellData[r]?.[c]
              const hasImage = graphics.some(g => g.row === r && g.col === c && g.content.kind === 'image')
              const background = graphics.find(g=>g.row===r && g.col===c && g.content.kind==='cell_background')?.content
              const style=cellStyle(cell)
              if (pageBackground && style.backgroundColor==='#FFFFFF') style.backgroundColor='transparent'
              return <td key={c} data-auto-size={!!options?.autoSize} rowSpan={merge ? merge.endRow - r + 1 : 1} colSpan={merge ? merge.endColumn - c + 1 : 1} style={{ ...style, fontSize:style.fontSize?`${style.fontSize}pt`:undefined,overflowWrap:options?.wrapNumbers?'anywhere':undefined,whiteSpace:typeof cell?.v==='number'&&!options?.wrapNumbers?'pre':style.whiteSpace,border:options?.borderWidth?`${options.borderWidth}pt solid ${options.borderColor}`:undefined,backgroundImage:background?.kind==='cell_background'?`url(${background.source})`:undefined,backgroundSize:'100% 100%', display: w ? undefined : 'none' }}>{hasImage ? '' : printValue(cell?.v, cell?.s?.n?.pattern)}</td>
            })}</tr>)}</tbody>
          </table>
          {graphics.filter(g => g.content.kind!=='cell_background' && widths[g.col] > 0 && heights[g.row] > 0).map((g, i) => <div key={i} style={{ position: 'absolute', left: widths.slice(0, g.col).reduce((a, b) => a + b, 0), top: heights.slice(0, g.row).reduce((a, b) => a + b, 0), pointerEvents: 'auto' }}>
            <ChartView content={g.content} width={g.width} height={g.height} />
          </div>)}
        </div>
      </section>
    })}</div>
  </div>, document.body)
}
