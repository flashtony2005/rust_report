/**
 * 右侧停靠属性面板（M7-W W3-5）：产品 IDE「属性面板」布局的 Web 版
 * （产品 `RPX.java:424` 的 `jSPPropertyBar` + `tabRight` 停靠区）。
 *
 * 六个可折叠分区 = 原顶栏横条的搬迁（样式/属性/行列/参数/数据集/报表设置），
 * JSX 逐字保留（`title` 属性是 E2E 选择器，勿改）；分区头点击折叠。
 * 所有 handler 经 [`PaneApi`] 由 App 注入 —— 本组件无自身状态（折叠态除外）。
 */
import { useState, type CSSProperties, type ReactNode } from 'react'

import {
  BORDER_SIDES,
  borderStyleOptions,
  cellPropsFromDef,
  formatOptions,
  type BorderSideName,
  type CellPropsEdit,
} from './cellprops'
import { fontOptions, type PanelField, type PanelStyle } from './stylepanel'
import {
  DEFAULT_COL_MM,
  DEFAULT_ROW_MM,
  colWidthMm,
  rowBreakPage,
  rowHeightMm,
} from './rowcol'
import { datasetSummary, datasetsOf, type DatasetInput } from './datasets'
import { PRINT_SETUP_DEFAULTS } from './reportsetup'
import type { ParamRow } from './report'
import type { ReportDef } from './designer'

/** W2-3：边框四边的中文标签（原 App.tsx 常量，随属性块搬迁）。 */
const BORDER_SIDE_LABELS: Record<BorderSideName, string> = {
  top: '上',
  right: '右',
  bottom: '下',
  left: '左',
}

/** W2-3：线型码表的中文标签（原 App.tsx 常量）。 */
const LINE_STYLE_LABELS: Record<string, string> = {
  none: '无',
  dotted: '点线',
  dashed: '虚线',
  solid: '实线',
  double: '双线',
}

/** W2-2 的三个位样式开关：字段 / 标签 / 标签附加样式（原 App.tsx 常量）。 */
const PANEL_TOGGLES = [
  ['bold', 'B', { fontWeight: 'bold' }],
  ['italic', 'I', { fontStyle: 'italic' }],
  ['underline', 'U', { textDecoration: 'underline' }],
] as const satisfies readonly (readonly [PanelField, string, CSSProperties])[]

/** W2-2 面板的输入控件通用样式（原 App.tsx 常量）。 */
const panelInputStyle: CSSProperties = {
  fontSize: 13,
  padding: '3px 6px',
  border: '1px solid var(--rpx-border)',
  borderRadius: 4,
  background: '#fff',
  color: 'var(--rpx-fg)',
}

/** W2-2 颜色输入样式（原 App.tsx 常量）。 */
const panelColorStyle: CSSProperties = {
  width: 28,
  height: 24,
  padding: 0,
  border: '1px solid var(--rpx-border)',
  borderRadius: 4,
  background: '#fff',
}

/**
 * 面板 API —— App 持有的全部状态与 handler 打包注入。
 * （搬迁自 App.tsx 的同名变量；签名逐字保持，块内 JSX 无需改写。）
 */
export type PaneApi = {
  // 选区/持有
  def: ReportDef | null
  selection: { row0: number; col0: number; height: number; width: number } | null
  panelDisabled: boolean
  // 样式（W2-2）
  pv: PanelStyle
  onPanelChange: (field: PanelField, raw: string | number | boolean) => void
  onPanelReset: () => void
  // 属性（W2-3）
  propsPanel: ReturnType<typeof cellPropsFromDef>
  onPropsChange: (edit: CellPropsEdit) => void
  // 行列（W2-5）
  onRowHeightChange: (mm: number | null) => void
  onColWidthChange: (mm: number | null) => void
  onBreakPageChange: (on: boolean | null) => void
  gridDraft: { rows: number; cols: number }
  setGridDraft: (fn: (g: { rows: number; cols: number }) => { rows: number; cols: number }) => void
  onGridResize: () => void
  // 参数（W1-2）
  params: ParamRow[]
  setParams: (fn: (rows: ParamRow[]) => ParamRow[]) => void
  setRow: (i: number, patch: Partial<ParamRow>) => void
  // 数据集（W2-5）
  dsDraft: DatasetInput
  setDsDraft: (fn: (d: DatasetInput) => DatasetInput) => void
  onDatasetAdd: () => void
  onDatasetRemove: (i: number) => void
  // 报表设置（W3-1）
  setupPanel: Record<keyof typeof PRINT_SETUP_DEFAULTS, number | boolean>
  onSetupChange: (field: keyof typeof PRINT_SETUP_DEFAULTS, value: number | boolean) => void
  onSwapPaper: () => void
}

const paneHeadStyle: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  width: '100%',
  padding: '6px 10px',
  border: 'none',
  background: 'var(--rpx-bg-muted)',
  fontSize: 13,
  fontWeight: 600,
  color: 'var(--rpx-fg)',
  cursor: 'pointer',
  fontFamily: 'system-ui, sans-serif',
}

const paneBodyStyle: CSSProperties = {
  padding: '6px 10px',
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  flexWrap: 'wrap',
  fontFamily: 'system-ui, sans-serif',
  fontSize: 13,
  color: 'var(--rpx-fg)',
  borderBottom: '1px solid var(--rpx-border)',
}

/** 一个可折叠分区（头 + 体）。折叠态由本组件自持（纯 UI，无判据含义）。 */
function Pane({ title, children }: { title: string; children: ReactNode }) {
  const [open, setOpen] = useState(true)
  return (
    <div>
      <button
        onClick={() => setOpen(!open)}
        aria-expanded={open}
        style={paneHeadStyle}
        title={open ? `收起${title}` : `展开${title}`}
      >
        <span>{title}</span>
        <span style={{ color: 'var(--rpx-fg-muted)', fontWeight: 400 }}>{open ? '▾' : '▸'}</span>
      </button>
      {open ? <div style={paneBodyStyle}>{children}</div> : null}
    </div>
  )
}

/** 右侧停靠属性面板本体。 */
export default function SidePanes({ api }: { api: PaneApi }) {
  return (
    <div
      data-testid="side-panes"
      style={{
        width: 340,
        minWidth: 340,
        overflowY: 'auto',
        borderLeft: '1px solid var(--rpx-border)',
        background: '#fff',
      }}
    >
      <Pane title="样式">

        <span style={{ color: 'var(--rpx-fg-muted)' }}>样式</span>
        <select
          value={api.pv.fontName}
          disabled={api.panelDisabled}
          title="字体族（定义 JSON 的 font_name；改回「宋体」= 删该键）"
          onChange={(e) => api.onPanelChange('fontName', e.target.value)}
          style={panelInputStyle}
        >
          {fontOptions(api.pv.fontName).map((f) => (
            <option key={f} value={f}>
              {f}
            </option>
          ))}
        </select>
        <input
          type="number"
          min={1}
          max={32767}
          value={api.pv.fontSize}
          disabled={api.panelDisabled}
          title="字号（1..=32767 的整数；改回 12 = 删该键）"
          onChange={(e) => api.onPanelChange('fontSize', e.target.value)}
          style={{ ...panelInputStyle, width: 64 }}
        />
        {PANEL_TOGGLES.map(([field, label, labelStyle]) => (
          <button
            key={field}
            aria-pressed={api.pv[field]}
            disabled={api.panelDisabled}
            title={`${label}（定义 JSON 的 ${field}）`}
            onClick={() => api.onPanelChange(field, !api.pv[field])}
            style={{
              width: 26,
              height: 24,
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              border: '1px solid var(--rpx-border)',
              borderRadius: 4,
              cursor: api.panelDisabled ? 'not-allowed' : 'pointer',
              ...(api.pv[field] && !api.panelDisabled
                ? { background: 'var(--rpx-accent)', color: '#fff' }
                : { background: '#fff', color: 'var(--rpx-fg)' }),
              ...labelStyle,
            }}
          >
            {label}
          </button>
        ))}
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          字色
          <input
            type="color"
            value={api.pv.foreColor}
            disabled={api.panelDisabled}
            title="字体颜色（定义 JSON 的 fore_color；改回 #000000 = 删该键）"
            onChange={(e) => api.onPanelChange('foreColor', e.target.value)}
            style={panelColorStyle}
          />
        </label>
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          底色
          <input
            type="color"
            value={api.pv.backColor}
            disabled={api.panelDisabled}
            title="背景色（定义 JSON 的 back_color；改回 #FFFFFF = 删该键）"
            onChange={(e) => api.onPanelChange('backColor', e.target.value)}
            style={panelColorStyle}
          />
        </label>
        <button
          onClick={api.onPanelReset}
          disabled={api.panelDisabled}
          title="把 7 个样式字段全部打回缺省（= 从定义 JSON 里删掉这 7 个键）"
          style={{
            fontSize: 13,
            padding: '3px 10px',
            border: '1px solid var(--rpx-border)',
            borderRadius: 4,
            background: '#fff',
            color: api.panelDisabled ? 'var(--rpx-fg-muted)' : 'var(--rpx-fg)',
            cursor: api.panelDisabled ? 'not-allowed' : 'pointer',
          }}
        >
          恢复缺省
        </button>
        {api.panelDisabled ? (
          <span style={{ color: 'var(--rpx-fg-muted)', fontSize: 12 }}>
            仅定义 JSON（.json）报表源可改样式
          </span>
        ) : null}
      </Pane>
      <Pane title="属性">

        <span style={{ color: 'var(--rpx-fg-muted)' }}>属性</span>
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          边框
          {BORDER_SIDES.map((s) => (
            <button
              key={s}
              aria-pressed={api.propsPanel.sides[s]}
              disabled={api.panelDisabled}
              title={`${BORDER_SIDE_LABELS[s]}边框（勾选 = 定义 JSON 里该边存在；取消 = 删该边）`}
              onClick={() =>
                api.onPropsChange({
                  kind: 'borderSide',
                  side: s,
                  on: !api.propsPanel.sides[s],
                  style: api.propsPanel.borderStyle,
                  color: api.propsPanel.borderColor,
                })
              }
              style={{
                width: 26,
                height: 24,
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                border: '1px solid var(--rpx-border)',
                borderRadius: 4,
                cursor: api.panelDisabled ? 'not-allowed' : 'pointer',
                ...(api.propsPanel.sides[s] && !api.panelDisabled
                  ? { background: 'var(--rpx-accent)', color: '#fff' }
                  : { background: '#fff', color: 'var(--rpx-fg)' }),
              }}
            >
              {BORDER_SIDE_LABELS[s]}
            </button>
          ))}
        </span>
        <select
          value={api.propsPanel.borderStyle}
          disabled={api.panelDisabled}
          title="边框线型（改它只作用于已勾选的边，不凭空给没边框的边加边框）"
          onChange={(e) => api.onPropsChange({ kind: 'borderStyle', style: e.target.value })}
          style={panelInputStyle}
        >
          {borderStyleOptions(api.propsPanel.borderStyle).map((s) => (
            <option key={s} value={s}>
              {LINE_STYLE_LABELS[s] ?? s}
            </option>
          ))}
        </select>
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          边框色
          <input
            type="color"
            value={api.propsPanel.borderColor}
            disabled={api.panelDisabled}
            title="边框颜色（定义 JSON 的 border.*.color；#000000 = 删该键）"
            onChange={(e) => api.onPropsChange({ kind: 'borderColor', color: e.target.value })}
            style={panelColorStyle}
          />
        </label>
        <span style={{ color: 'var(--rpx-border)' }}>|</span>
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          格式
          <select
            value={api.propsPanel.format}
            disabled={api.panelDisabled}
            title="数字格式（定义 JSON 的 format；「（无格式）」= 删该键）"
            onChange={(e) => api.onPropsChange({ kind: 'format', value: e.target.value })}
            style={panelInputStyle}
          >
            {formatOptions(api.propsPanel.format).map((f) => (
              <option key={f} value={f}>
                {f === '' ? '（无格式）' : f}
              </option>
            ))}
          </select>
        </label>
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          旋转
          <input
            type="number"
            min={0}
            max={360}
            value={api.propsPanel.rotation}
            disabled={api.panelDisabled}
            title="文字旋转角（定义 JSON 的 rotation；面板开放 0..360 整数，0 = 删该键）"
            onChange={(e) => api.onPropsChange({ kind: 'rotation', value: e.target.value })}
            style={{ ...panelInputStyle, width: 60 }}
          />
          <span style={{ color: 'var(--rpx-fg-muted)' }}>°</span>
        </label>
      </Pane>
      <Pane title="行列">

        <span style={{ color: 'var(--rpx-fg-muted)' }}>行列</span>
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          行高
          <input
            type="number"
            min={0.1}
            step={0.5}
            value={
              api.selection && api.def
                ? (rowHeightMm(api.def, api.selection.row0 + 1) ?? '')
                : DEFAULT_ROW_MM
            }
            disabled={api.panelDisabled}
            title="行高（mm，定义 JSON 的 row_props[].height；清空 = 删声明回到缺省 8mm）"
            onChange={(e) =>
              api.onRowHeightChange(e.target.value === '' ? null : Number(e.target.value))
            }
            style={{ ...panelInputStyle, width: 64 }}
          />
          <span style={{ color: 'var(--rpx-fg-muted)' }}>mm</span>
        </label>
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          列宽
          <input
            type="number"
            min={0.1}
            step={0.5}
            value={
              api.selection && api.def
                ? (colWidthMm(api.def, api.selection.col0 + 1) ?? '')
                : DEFAULT_COL_MM
            }
            disabled={api.panelDisabled}
            title="列宽（mm，定义 JSON 的 col_props[].width；清空 = 删声明回到缺省 25mm）"
            onChange={(e) =>
              api.onColWidthChange(e.target.value === '' ? null : Number(e.target.value))
            }
            style={{ ...panelInputStyle, width: 64 }}
          />
          <span style={{ color: 'var(--rpx-fg-muted)' }}>mm</span>
        </label>
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          断页
          <select
            value={
              api.selection && api.def
                ? rowBreakPage(api.def, api.selection.row0 + 1) === true
                  ? 'on'
                  : rowBreakPage(api.def, api.selection.row0 + 1) === false
                    ? 'off'
                    : ''
                : ''
            }
            disabled={api.panelDisabled}
            title="行断页（定义 JSON 的 row_props[].break_page；画布无此概念，重渲染生效）"
            onChange={(e) =>
              api.onBreakPageChange(
                e.target.value === '' ? null : e.target.value === 'on',
              )
            }
            style={panelInputStyle}
          >
            <option value="">（未声明）</option>
            <option value="on">此行前断页</option>
            <option value="off">不断页</option>
          </select>
        </label>
        <span style={{ color: 'var(--rpx-border)' }}>|</span>
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          网格
          <input
            type="number"
            min={1}
            value={api.gridDraft.rows}
            disabled={api.panelDisabled}
            title="网格行数（定义 JSON 的 grid.rows；缩小前会校验既有声明越界）"
            onChange={(e) => api.setGridDraft((g) => ({ ...g, rows: Number(e.target.value) }))}
            style={{ ...panelInputStyle, width: 56 }}
          />
          ×
          <input
            type="number"
            min={1}
            value={api.gridDraft.cols}
            disabled={api.panelDisabled}
            title="网格列数（定义 JSON 的 grid.cols）"
            onChange={(e) => api.setGridDraft((g) => ({ ...g, cols: Number(e.target.value) }))}
            style={{ ...panelInputStyle, width: 56 }}
          />
        </label>
        <button
          onClick={api.onGridResize}
          disabled={api.panelDisabled}
          title="按上面输入的行列数调整网格，并自动按新尺寸重渲染"
          style={{
            fontSize: 13,
            padding: '3px 10px',
            border: '1px solid var(--rpx-border)',
            borderRadius: 4,
            background: '#fff',
            color: api.panelDisabled ? 'var(--rpx-fg-muted)' : 'var(--rpx-fg)',
            cursor: api.panelDisabled ? 'not-allowed' : 'pointer',
          }}
        >
          调整网格
        </button>
        {api.panelDisabled ? (
          <span style={{ color: 'var(--rpx-fg-muted)', fontSize: 12 }}>
            仅定义 JSON（.json）报表源可改
          </span>
        ) : null}
      </Pane>
      <Pane title="报表参数">

        <span style={{ color: 'var(--rpx-fg-muted)', marginRight: 8 }}>
          报表参数（未登记的参数服务端会显式报错）
        </span>
        {api.params.map((row, i) => (
          <span
            key={i}
            style={{ display: 'inline-flex', alignItems: 'center', gap: 4, marginRight: 10 }}
          >
            <input
              placeholder="参数名"
              value={row.name}
              onChange={(e) => api.setRow(i, { name: e.target.value })}
              style={{
                width: 110,
                fontSize: 13,
                padding: '3px 6px',
                border: '1px solid var(--rpx-border)',
                borderRadius: 4,
              }}
            />
            <input
              placeholder="值"
              value={row.value}
              onChange={(e) => api.setRow(i, { value: e.target.value })}
              style={{
                width: 130,
                fontSize: 13,
                padding: '3px 6px',
                border: '1px solid var(--rpx-border)',
                borderRadius: 4,
              }}
            />
            {/* 动态/属性参数标注（W1-2）：值须渲染时由用户或上游提供。 */}
            {row.dynamic ? (
              <span
                title="动态参数：is_static=false，渲染时须提供值"
                style={{
                  fontSize: 11,
                  padding: '1px 6px',
                  borderRadius: 8,
                  border: '1px solid var(--rpx-accent)',
                  color: 'var(--rpx-accent)',
                }}
              >
                动态
              </span>
            ) : null}
            <button
              aria-label="删除参数"
              title="删除参数"
              onClick={() => api.setParams((rows) => rows.filter((_, j) => j !== i))}
              style={{
                width: 22,
                height: 22,
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                border: '1px solid var(--rpx-border)',
                borderRadius: 4,
                background: '#fff',
                cursor: 'pointer',
              }}
            >
              {/* 删除图标：内联 SVG（16px，统一描边） */}
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path
                  d="M4 4l8 8M12 4l-8 8"
                  stroke="var(--rpx-danger)"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                />
              </svg>
            </button>
          </span>
        ))}
        <button
          onClick={() => api.setParams((rows) => [...rows, { name: '', value: '' }])}
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: 4,
            fontSize: 13,
            padding: '3px 10px',
            border: '1px solid var(--rpx-border)',
            borderRadius: 4,
            background: '#fff',
            color: 'var(--rpx-fg)',
            cursor: 'pointer',
          }}
        >
          {/* 添加图标：内联 SVG（16px，统一描边） */}
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path
              d="M8 3v10M3 8h10"
              stroke="var(--rpx-accent)"
              strokeWidth="1.5"
              strokeLinecap="round"
            />
          </svg>
          添加参数
        </button>
      </Pane>
      <Pane title="数据集">

        <span style={{ color: 'var(--rpx-fg-muted)' }}>数据集</span>
        {(api.def ? datasetsOf(api.def) : []).map((d, i) => (
          <span
            key={i}
            style={{ display: 'inline-flex', alignItems: 'center', gap: 4, marginRight: 4 }}
          >
            <span
              title={datasetSummary(d)}
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: 4,
                padding: '2px 8px',
                border: '1px solid var(--rpx-border)',
                borderRadius: 4,
                background: '#fff',
              }}
            >
              <b style={{ fontWeight: 600 }}>{d.name ?? '（无名）'}</b>
              <span style={{ color: 'var(--rpx-fg-muted)' }}>{datasetSummary(d)}</span>
            </span>
            <button
              aria-label={`删除数据集 ${d.name ?? i + 1}`}
              title="删除该数据集"
              onClick={() => api.onDatasetRemove(i)}
              style={{
                width: 20,
                height: 20,
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                border: '1px solid var(--rpx-border)',
                borderRadius: 4,
                background: '#fff',
                cursor: api.panelDisabled ? 'not-allowed' : 'pointer',
              }}
            >
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path
                  d="M4 4l8 8M12 4l-8 8"
                  stroke="var(--rpx-danger)"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                />
              </svg>
            </button>
          </span>
        ))}
        {/* 添加行：name + 类型（文件/SQL）+ 按类型的字段。 */}
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          <input
            placeholder="数据集名（可空）"
            value={api.dsDraft.name ?? ''}
            disabled={api.panelDisabled}
            onChange={(e) => api.setDsDraft((d) => ({ ...d, name: e.target.value }))}
            style={{ ...panelInputStyle, width: 110 }}
          />
          <select
            value={api.dsDraft.file !== undefined ? 'file' : 'sql'}
            disabled={api.panelDisabled}
            title="数据集类型：文件（CSV）或 SQL"
            onChange={(e) =>
              api.setDsDraft((d) =>
                e.target.value === 'file'
                  ? { name: d.name, file: { file_name: '', encoding: 'GBK' } }
                  : { name: d.name, sql: { dsn: '', sql: '' } },
              )
            }
            style={panelInputStyle}
          >
            <option value="file">文件</option>
            <option value="sql">SQL</option>
          </select>
          {api.dsDraft.file !== undefined ? (
            <>
              <input
                placeholder="CSV 文件名（须与上传一致）"
                value={api.dsDraft.file.file_name}
                disabled={api.panelDisabled}
                onChange={(e) =>
                  api.setDsDraft((d) => ({
                    ...d,
                    file: { ...d.file!, file_name: e.target.value },
                  }))
                }
                style={{ ...panelInputStyle, width: 170 }}
              />
              <select
                value={api.dsDraft.file.encoding ?? 'GBK'}
                disabled={api.panelDisabled}
                title="CSV 编码（缺省 GBK）"
                onChange={(e) =>
                  api.setDsDraft((d) => ({
                    ...d,
                    file: { file_name: d.file!.file_name, encoding: e.target.value },
                  }))
                }
                style={panelInputStyle}
              >
                <option value="GBK">GBK</option>
                <option value="UTF-8">UTF-8</option>
              </select>
            </>
          ) : (
            <>
              <input
                placeholder="数据源名 dsn"
                value={api.dsDraft.sql?.dsn ?? ''}
                disabled={api.panelDisabled}
                onChange={(e) =>
                  api.setDsDraft((d) => ({
                    ...d,
                    sql: { dsn: e.target.value, sql: d.sql?.sql ?? '' },
                  }))
                }
                style={{ ...panelInputStyle, width: 110 }}
              />
              <input
                placeholder="SQL 语句"
                value={api.dsDraft.sql?.sql ?? ''}
                disabled={api.panelDisabled}
                onChange={(e) =>
                  api.setDsDraft((d) => ({
                    ...d,
                    sql: { dsn: d.sql?.dsn ?? '', sql: e.target.value },
                  }))
                }
                style={{ ...panelInputStyle, width: 190 }}
              />
            </>
          )}
          <button
            onClick={api.onDatasetAdd}
            disabled={api.panelDisabled}
            title="把草稿追加进定义 JSON 的 datasets[]"
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: 4,
              fontSize: 13,
              padding: '3px 10px',
              border: '1px solid var(--rpx-border)',
              borderRadius: 4,
              background: '#fff',
              color: api.panelDisabled ? 'var(--rpx-fg-muted)' : 'var(--rpx-fg)',
              cursor: api.panelDisabled ? 'not-allowed' : 'pointer',
            }}
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path
                d="M8 3v10M3 8h10"
                stroke="var(--rpx-accent)"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
            </svg>
            添加
          </button>
        </span>
        {api.panelDisabled ? (
          <span style={{ color: 'var(--rpx-fg-muted)', fontSize: 12 }}>
            仅定义 JSON（.json）报表源可改
          </span>
        ) : null}
      </Pane>
      <Pane title="报表设置">

        <span style={{ color: 'var(--rpx-fg-muted)' }}>报表设置</span>
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          纸张
          <input
            type="number"
            min={0}
            step={1}
            value={Number(api.setupPanel.paper_width)}
            disabled={api.panelDisabled}
            title="纸张宽（mm，定义 JSON 的 print_setup.paper_width）"
            onChange={(e) => api.onSetupChange('paper_width', Number(e.target.value))}
            style={{ ...panelInputStyle, width: 56 }}
          />
          ×
          <input
            type="number"
            min={0}
            step={1}
            value={Number(api.setupPanel.paper_height)}
            disabled={api.panelDisabled}
            title="纸张高（mm，定义 JSON 的 print_setup.paper_height）"
            onChange={(e) => api.onSetupChange('paper_height', Number(e.target.value))}
            style={{ ...panelInputStyle, width: 56 }}
          />
          <button
            onClick={api.onSwapPaper}
            disabled={api.panelDisabled}
            title="横竖互换（宽高对调）"
            style={{
              fontSize: 12,
              padding: '3px 8px',
              border: '1px solid var(--rpx-border)',
              borderRadius: 4,
              background: '#fff',
              color: api.panelDisabled ? 'var(--rpx-fg-muted)' : 'var(--rpx-fg)',
              cursor: api.panelDisabled ? 'not-allowed' : 'pointer',
            }}
          >
            横↔竖
          </button>
        </span>
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          边距
          {(
            [
              ['left_margin', '左'],
              ['right_margin', '右'],
              ['top_margin', '上'],
              ['bottom_margin', '下'],
            ] as const
          ).map(([field, label]) => (
            <input
              key={field}
              type="number"
              min={0}
              step={1}
              value={Number(api.setupPanel[field])}
              disabled={api.panelDisabled}
              title={`${label}边距（mm，定义 JSON 的 print_setup.${field}）`}
              onChange={(e) => api.onSetupChange(field, Number(e.target.value))}
              style={{ ...panelInputStyle, width: 48 }}
            />
          ))}
        </span>
        <span style={{ color: 'var(--rpx-border)' }}>|</span>
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          每页行数
          <input
            type="number"
            min={0}
            step={1}
            value={Number(api.setupPanel.row_num_per_page)}
            disabled={api.panelDisabled}
            title="每页行数（定义 JSON 的 print_setup.row_num_per_page；分页器直接用它）"
            onChange={(e) => api.onSetupChange('row_num_per_page', Number(e.target.value))}
            style={{ ...panelInputStyle, width: 56 }}
          />
        </label>
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          份数
          <input
            type="number"
            min={0}
            step={1}
            value={Number(api.setupPanel.copy_count)}
            disabled={api.panelDisabled}
            title="打印份数（定义 JSON 的 print_setup.copy_count）"
            onChange={(e) => api.onSetupChange('copy_count', Number(e.target.value))}
            style={{ ...panelInputStyle, width: 56 }}
          />
        </label>
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          缩放
          <input
            type="number"
            min={0}
            max={32767}
            step={1}
            value={Number(api.setupPanel.zoom_scale)}
            disabled={api.panelDisabled}
            title="缩放百分比（定义 JSON 的 print_setup.zoom_scale）"
            onChange={(e) => api.onSetupChange('zoom_scale', Number(e.target.value))}
            style={{ ...panelInputStyle, width: 56 }}
          />
          %
        </label>
        <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          分栏
          <input
            type="number"
            min={0}
            max={32767}
            step={1}
            value={Number(api.setupPanel.table_column_num)}
            disabled={api.panelDisabled}
            title="分栏数（定义 JSON 的 print_setup.table_column_num）"
            onChange={(e) => api.onSetupChange('table_column_num', Number(e.target.value))}
            style={{ ...panelInputStyle, width: 48 }}
          />
        </label>
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
          版面
          <input
            type="number"
            min={0}
            max={32767}
            step={1}
            value={Number(api.setupPanel.layout_rows)}
            disabled={api.panelDisabled}
            title="版面行数（定义 JSON 的 print_setup.layout_rows）"
            onChange={(e) => api.onSetupChange('layout_rows', Number(e.target.value))}
            style={{ ...panelInputStyle, width: 44 }}
          />
          ×
          <input
            type="number"
            min={0}
            max={32767}
            step={1}
            value={Number(api.setupPanel.layout_cols)}
            disabled={api.panelDisabled}
            title="版面列数（定义 JSON 的 print_setup.layout_cols）"
            onChange={(e) => api.onSetupChange('layout_cols', Number(e.target.value))}
            style={{ ...panelInputStyle, width: 44 }}
          />
        </span>
        <span style={{ color: 'var(--rpx-border)' }}>|</span>
        {(
          [
            ['back_graph_printed', '背景图'],
            ['column_repeated_group_header', '重复列组头'],
            ['full_screen_preview', '全屏预览'],
            ['blank_line_filled', '空行填充'],
          ] as const
        ).map(([field, label]) => (
          <button
            key={field}
            aria-pressed={Boolean(api.setupPanel[field])}
            disabled={api.panelDisabled}
            title={`${label}（定义 JSON 的 print_setup.${field}）`}
            onClick={() => api.onSetupChange(field, !api.setupPanel[field])}
            style={{
              fontSize: 12,
              padding: '3px 8px',
              border: '1px solid var(--rpx-border)',
              borderRadius: 4,
              cursor: api.panelDisabled ? 'not-allowed' : 'pointer',
              ...(Boolean(api.setupPanel[field]) && !api.panelDisabled
                ? { background: 'var(--rpx-accent)', color: '#fff' }
                : { background: '#fff', color: 'var(--rpx-fg)' }),
            }}
          >
            {label}
          </button>
        ))}
        {api.panelDisabled ? (
          <span style={{ color: 'var(--rpx-fg-muted)', fontSize: 12 }}>
            仅定义 JSON（.json）报表源可改
          </span>
        ) : null}
      </Pane>
    </div>
  )
}
