/**
 * W2 准入探针（M7-W）：Univer 1.0.4 编辑 API 的**运行时**可用性。
 *
 * # 为什么要有这个页面
 *
 * `docs/spec-m7-web.md` §7 坑位 4 要求「装完依赖后先做一次 API 可用性探针，再
 * 决定 W2 的分期」—— 类型声明只能证明 API **存在**，证不了它**运行时真能用**：
 * `setFontLine` 到底落不落、`SheetValueChanged` 到底触发不触发、`setReadOnly`
 * 到底拦不拦写入、`sheets-numfmt` 的 facade 到底挂没挂上。本页面把每一项**实测**，
 * 结果同时写进 DOM（`<pre>`）与 `window.__probeResults`（供 agent-browser 抓取）。
 *
 * ⚠ 这是**开发期探针**，不是产品页面：不进 `index.html`、不进主导航。
 * 入口 `web/probe.html`（vite 多页入口），dev 下 `http://127.0.0.1:5288/probe.html`。
 *
 * # 探针结果写进哪
 *
 * 实测结论回填 `docs/spec-m7-web.md` §7 坑位 4（把「待验证」换成事实）。
 */
import { createRoot } from 'react-dom/client'

import { useEffect, useRef, useState } from 'react'
import { LocaleType, createUniver, mergeLocales, type FUniver } from '@univerjs/presets'
import { UniverSheetsCorePreset } from '@univerjs/preset-sheets-core'
import UniverPresetSheetsCoreZhCN from '@univerjs/preset-sheets-core/locales/zh-CN'
import '@univerjs/preset-sheets-core/lib/index.css'

/** 一条探针记录：能力标识 + 调用的 API + 是否成立 + 实测值。 */
type ProbeRow = { id: string; api: string; ok: boolean; observed: string }

/** 把任意值压成一行可读文本（颜色对象 `{rgb}` 之类要展开）。 */
function show(v: unknown): string {
  if (v === undefined) return 'undefined'
  try {
    return JSON.stringify(v)
  } catch {
    return String(v)
  }
}

/**
 * 跑全部探针。每一项都**实测**：调 API → 读回 → 比对，不猜。
 *
 * ⚠ 每一项独立 try/catch：一个 API 抛错不能让后面的探针全废（探针的目的是
 * 「量出哪些能用」，不是「第一个失败就停」）。
 */
async function runProbe(api: FUniver): Promise<ProbeRow[]> {
  const rows: ProbeRow[] = []
  const rec = (id: string, call: string, ok: boolean, observed: unknown) =>
    rows.push({ id, api: call, ok, observed: show(observed) })
  const step = async (id: string, call: string, fn: () => unknown | Promise<unknown>) => {
    try {
      const observed = await fn()
      // ok 由调用方通过 rec 决定（有的项是「读回相等」，有的是「不抛错」）。
      return observed
    } catch (e) {
      rec(id, call, false, `抛错：${e instanceof Error ? e.message : String(e)}`)
      return undefined
    }
  }

  const wb = api.createWorkbook({
    id: 'probe-wb',
    appVersion: '1.0.4',
    name: 'W2 探针',
    sheetOrder: ['s1'],
    styles: {},
    locale: LocaleType.ZH_CN,
    sheets: {
      s1: {
        id: 's1',
        name: 'Sheet1',
        cellData: { 0: { 0: { v: '原文' } } },
        rowCount: 20,
        columnCount: 8,
      },
    },
  })
  const sheet = wb.getActiveSheet()
  if (!sheet) {
    rec('sheet', 'getActiveSheet()', false, '返回 null')
    return rows
  }

  // ---- W2-1 单元格内容 ----
  await step('value', "range.setValue('改后')", async () => {
    const r = sheet.getRange(0, 0)
    r.setValue('改后')
    const back = r.getValue()
    rec('value', "setValue('改后') → getValue()", back === '改后', back)
  })

  // ---- W2-1 合并 ----
  await step('merge', 'range.merge()', async () => {
    const r = sheet.getRange(2, 0, 1, 2)
    r.merge()
    const merged = r.isMerged()
    r.breakApart()
    const afterBreak = r.isMerged()
    rec('merge', 'merge() → isMerged() → breakApart() → isMerged()', merged && !afterBreak, {
      merged,
      afterBreak,
    })
  })

  // ---- W2-2 样式 7 字段（逐个写 → 读回 IStyleData 的短码） ----
  const r0 = sheet.getRange(4, 0)
  await step('style', 'setFontFamily/setFontSize/setFontColor', async () => {
    r0.setFontFamily('Times New Roman')
    r0.setFontSize(18)
    r0.setFontColor('#ff0000')
    const s = r0.getCellStyleData()
    rec(
      'style.font',
      "setFontFamily('Times New Roman') + setFontSize(18) + setFontColor('#ff0000')",
      s?.ff === 'Times New Roman' && s?.fs === 18,
      { ff: s?.ff, fs: s?.fs, cl: s?.cl },
    )
  })

  await step('style2', 'setFontWeight/setFontStyle/setFontLine', async () => {
    r0.setFontWeight('bold')
    r0.setFontStyle('italic')
    r0.setFontLine('underline')
    const s = r0.getCellStyleData()
    rec(
      'style.biu',
      "setFontWeight('bold') + setFontStyle('italic') + setFontLine('underline')",
      s?.bl === 1 && s?.it === 1 && s?.ul !== undefined,
      { bl: s?.bl, it: s?.it, ul: s?.ul },
    )
  })

  await step('style3', 'setBackgroundColor', async () => {
    r0.setBackgroundColor('#ffff00')
    const s = r0.getCellStyleData()
    rec('style.bg', "setBackgroundColor('#ffff00')", s?.bg?.rgb === '#ffff00', { bg: s?.bg })
  })

  // ---- W2-3 边框 ----
  await step('border', 'setBorder', async () => {
    const r = sheet.getRange(6, 0)
    r.setBorder(api.Enum.BorderType.ALL, api.Enum.BorderStyleTypes.THIN, '#0000ff')
    const s = r.getCellStyleData()
    // `IStyleData.bd?: IBorderData`，四边键 `t/r/b/l`，每边 `{s, cl}`。
    const b = s?.bd?.t
    rec('border', 'setBorder(BorderType.ALL, BorderStyleTypes.THIN, "#0000ff")', b !== undefined, {
      bd: s?.bd,
    })
  })

  // ---- W2-3 数字格式（sheets-numfmt 的 facade 是否挂上） ----
  await step('numfmt', 'setNumberFormat', async () => {
    const r = sheet.getRange(8, 0)
    const has = typeof (r as unknown as { setNumberFormat?: unknown }).setNumberFormat === 'function'
    if (!has) {
      rec('numfmt', 'setNumberFormat', false, 'facade 上**没有** setNumberFormat')
      return
    }
    r.setNumberFormat('#,##0.00')
    const back = (
      r as unknown as { getNumberFormat: () => string }
    ).getNumberFormat()
    rec('numfmt', "setNumberFormat('#,##0.00') → getNumberFormat()", back === '#,##0.00', back)
  })

  // ---- W2-3 旋转 ----
  await step('rotation', 'setTextRotation', async () => {
    const r = sheet.getRange(10, 0)
    r.setTextRotation(45)
    const s = r.getCellStyleData()
    const tr = s?.tr as { a?: number } | undefined
    rec('rotation', 'setTextRotation(45)', tr?.a === 45, { tr: s?.tr })
  })

  // ---- W2-2/3 水平对齐（⚠ 'right' 在类型里不存在，'normal' 才是右对齐） ----
  await step('align', 'setHorizontalAlignment', async () => {
    const r = sheet.getRange(12, 0)
    r.setHorizontalAlignment('center')
    const center = r.getCellStyleData()?.ht
    r.setHorizontalAlignment('normal')
    const normal = r.getCellStyleData()?.ht
    rec('align', "setHorizontalAlignment('center' | 'normal')", center === 2, {
      center,
      normal,
      note: 'normal = 右对齐（类型里没有 right）',
    })
  })

  // ---- W2-5 网格尺寸 ----
  await step('geometry', 'setColumnWidth/setRowHeight', async () => {
    sheet.setColumnWidth(1, 120)
    sheet.setRowHeight(1, 40)
    const cw = sheet.getColumnWidth(1)
    const rh = sheet.getRowHeight(1)
    rec('geometry', 'setColumnWidth(1,120) + setRowHeight(1,40)', cw === 120 && rh === 40, {
      columnWidth: cw,
      rowHeight: rh,
    })
  })

  // ---- W2-5 行列属性（⚠ 必须读回，不能只判「不抛错」） ----
  await step('rowcustom', 'setRowCustom/setColumnCustom', async () => {
    const beforeH = sheet.getRowHeight(0)
    sheet.setRowCustom({ 0: { h: 30, custom: {} } })
    const afterH = sheet.getRowHeight(0)
    sheet.setColumnCustom({ 0: { w: 20, custom: {} } })
    const afterW = sheet.getColumnWidth(0)
    rec(
      'rowcustom',
      'setRowCustom({0:{h:30}}) + setColumnCustom({0:{w:20}}) → getRowHeight/getColumnWidth',
      afterH === 30 && afterW === 20,
      { rowHeight: { before: beforeH, after: afterH }, columnWidth: afterW },
    )
  })

  // ---- 事件：SheetValueChanged 是否触发（W2-1「改完即更新」的依赖） ----
  await step('event', 'addEvent(SheetValueChanged)', async () => {
    let fired = 0
    api.addEvent(api.Event.SheetValueChanged, () => {
      fired += 1
    })
    sheet.getRange(16, 0).setValue('触发事件')
    // Univer 的命令是异步派发的 ⇒ 让出一个宏任务再读。
    await new Promise((r) => setTimeout(r, 50))
    rec('event', "addEvent(Event.SheetValueChanged) + setValue()", fired > 0, { fired })
  })

  // ---- 只读：setReadOnly 之后 setValue 是否真的被拦 ----
  await step('readonly', 'getWorkbookPermission().setReadOnly()', async () => {
    await wb.getWorkbookPermission().setReadOnly()
    const r = sheet.getRange(18, 0)
    let blocked = false
    let err = ''
    try {
      r.setValue('只读下写入')
    } catch (e) {
      blocked = true
      err = e instanceof Error ? e.message : String(e)
    }
    const back = r.getValue()
    await wb.getWorkbookPermission().setEditable()
    const r2 = sheet.getRange(18, 0)
    r2.setValue('可写后写入')
    rec(
      'readonly',
      'setReadOnly() → setValue() → setEditable() → setValue()',
      back !== '只读下写入' && r2.getValue() === '可写后写入',
      { blocked, err, readOnlyValue: back, afterEditable: r2.getValue() },
    )
  })

  // ---- 撤销（⚠ 必须验「值真的回退」，不能只看 undo() 的返回值） ----
  await step('undo', 'univerAPI.undo()', async () => {
    const r = sheet.getRange(19, 0)
    r.setValue('撤销前')
    r.setValue('撤销后')
    const mid = r.getValue()
    const returned = await api.undo()
    await new Promise((res) => setTimeout(res, 50))
    const back = r.getValue()
    rec(
      'undo',
      "setValue('撤销前') → setValue('撤销后') → undo() → getValue()",
      back === '撤销前',
      { mid, returned, back },
    )
  })

  // ---- W2-4 打印设置：sheets 有没有任何页面设置 API？（静态已证没有，
  //      这里用「facade 上不存在匹配方法」再量一次，让探针页自证完整。） ----
  await step('printsetup', '页面设置 API 是否存在', async () => {
    const wbKeys = Object.getOwnPropertyNames(
      Object.getPrototypeOf(wb) as object,
    ).filter((k) => /page|print|paper|margin|orient/i.test(k))
    const sheetKeys = Object.getOwnPropertyNames(
      Object.getPrototypeOf(sheet) as object,
    ).filter((k) => /page|print|paper|margin|orient/i.test(k))
    const found = [...wbKeys, ...sheetKeys]
    rec(
      'printsetup',
      'Object.getOwnPropertyNames(prototype) 里 /page|print|paper|margin|orient/',
      false,
      found.length === 0
        ? 'facade 上**没有任何**页面设置方法 ⇒ W2-4 无法在 Univer 编辑态表达'
        : { found },
    )
  })

  return rows
}

export default function Probe() {
  const containerRef = useRef<HTMLDivElement>(null)
  const [rows, setRows] = useState<ProbeRow[]>([])
  const [done, setDone] = useState(false)

  useEffect(() => {
    let disposed = false
    const boot = async () => {
      if (!containerRef.current) return
      const { univerAPI } = createUniver({
        locale: LocaleType.ZH_CN,
        locales: { [LocaleType.ZH_CN]: mergeLocales(UniverPresetSheetsCoreZhCN) },
        presets: [UniverSheetsCorePreset({ container: containerRef.current })],
      })
      const results = await runProbe(univerAPI)
      if (disposed) return
      setRows(results)
      setDone(true)
      // 供 agent-browser `eval` 抓取（DOM 里也有同一份）。
      ;(window as unknown as { __probeResults?: ProbeRow[] }).__probeResults = results
      console.log('W2 探针结果', results)
    }
    void boot()
    return () => {
      disposed = true
    }
  }, [])

  const passed = rows.filter((r) => r.ok).length

  return (
    <div style={{ padding: 16, fontFamily: 'system-ui, sans-serif', fontSize: 13 }}>
      <h2 style={{ fontSize: 15 }}>W2 准入探针 · Univer 1.0.4 编辑 API 运行时可用性</h2>
      <div style={{ color: '#57606a', marginBottom: 8 }}>
        {done ? `完成：${passed}/${rows.length} 项通过` : '探针运行中…'}
      </div>
      <table style={{ borderCollapse: 'collapse', width: '100%' }}>
        <thead>
          <tr>
            <th style={th}>能力</th>
            <th style={th}>调用的 API</th>
            <th style={th}>结果</th>
            <th style={th}>实测值</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td style={td}>{r.id}</td>
              <td style={{ ...td, fontFamily: 'ui-monospace, monospace' }}>{r.api}</td>
              <td style={{ ...td, color: r.ok ? '#1a7f37' : '#cf222e' }}>{r.ok ? '✓' : '✗'}</td>
              <td style={{ ...td, fontFamily: 'ui-monospace, monospace' }}>{r.observed}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div ref={containerRef} style={{ height: 320, marginTop: 12, border: '1px solid #d8dee4' }} />
    </div>
  )
}

const th: React.CSSProperties = {
  textAlign: 'left',
  borderBottom: '1px solid #d8dee4',
  padding: '4px 8px',
}
const td: React.CSSProperties = {
  borderBottom: '1px solid #eee',
  padding: '4px 8px',
  verticalAlign: 'top',
}

// 挂载入口（`probe.html` 指向本文件）。⚠ 与 `main.tsx` 分开：探针页不导产品
// 组件，产品页也不该被探针的副作用污染。
const root = document.getElementById('root')
if (root) {
  createRoot(root).render(<Probe />)
}
