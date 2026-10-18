/**
 * 右侧属性面板（重构版）：按**对象层次**分两个页签 —— 单元格 / 报表。
 *
 * ## 为什么分页签
 *
 * 旧版把七个分区（公式与主格 / 数据集 / 样式 / 属性 / 行列 / 报表参数 / 报表设置）
 * 平铺在同一滚动列里，等于把「单元格的属性」和「整份报表的属性」混为一谈：
 * 改一个字号要滚过三个分区，且「样式」与「属性」本是同一对象的属性却被拆开。
 * 现在按产品 IDE 的惯例分两层：
 *
 * | 页签 | 分组 | 对应定义 JSON |
 * | --- | --- | --- |
 * | 单元格 | 内容 / 字体 / 边框与格式 / 行列 | `cells[]`、`row_props[]`、`col_props[]` |
 * | 报表 | 页面 / 分页与版面 / 网格 / 参数 / 数据集 | `print_setup`、`grid`、`params[]`、`datasets[]` |
 *
 * ## 三条不变量（与重构前逐字一致）
 *
 * 1. **读方向是定义 JSON 派生**：面板显示值来自 `def`，没有选区时显示缺省。
 * 2. **写方向逐键补丁 + 缺省即删键**：由 `stylepanel` / `cellprops` / `rowcol` /
 *    `reportsetup` 四个纯函数模块负责，本组件只做 UI。
 * 3. **`title` 属性是 E2E 选择器**：控件上的 `title` 文案保持原文，勿改。
 *
 * ## 分组跳转
 *
 * 「报表」菜单与快捷键（Ctrl+B / F11）需要真正落到某分组：App 通过
 * [`FocusSignal`] 传入 `{ id, seq }`，对应分组会滚进视野并短暂高亮。旧版只
 * `setSideVisible(true)` + 一句状态文案，面板纹丝不动 —— 那是假跳转。
 */
import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'

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
import { PRINT_SETUP_DEFAULTS } from './reportsetup'
import TemplateParameters from './TemplateParameters'
import type { ReportDef } from './designer'
import './template.css'

/** 两个页签。 */
export type PaneTab = 'cell' | 'report'

/** 九个可定位分组（菜单跳转与高亮的锚点）。 */
export type PaneGroupId =
  | 'content'
  | 'font'
  | 'border'
  | 'rowcol'
  | 'page'
  | 'paginate'
  | 'grid'
  | 'params'
  | 'datasets'

/** 分组中文名（菜单文案与状态提示共用同一份，避免两处漂移）。 */
export const GROUP_LABEL: Record<PaneGroupId, string> = {
  content: '内容与主格',
  font: '字体',
  border: '边框与格式',
  rowcol: '行列',
  page: '页面设置',
  paginate: '分页与版面',
  grid: '网格尺寸',
  params: '报表参数',
  datasets: '数据集',
}

/** 分组所属页签（菜单跳转要顺手切页签）。 */
export const GROUP_TAB: Record<PaneGroupId, PaneTab> = {
  content: 'cell',
  font: 'cell',
  border: 'cell',
  rowcol: 'cell',
  page: 'report',
  paginate: 'report',
  grid: 'report',
  params: 'report',
  datasets: 'report',
}

/** 跳转信号：`seq` 用递增序号保证「同一个分组连点两次」也能重新定位。 */
export type FocusSignal = { id: PaneGroupId; seq: number }

/** 边框四边的中文标签。 */
const BORDER_SIDE_LABELS: Record<BorderSideName, string> = {
  top: '上',
  right: '右',
  bottom: '下',
  left: '左',
}

/** 线型码表的中文标签。 */
const LINE_STYLE_LABELS: Record<string, string> = {
  none: '无',
  dotted: '点线',
  dashed: '虚线',
  solid: '实线',
  double: '双线',
}

/** 三个位样式开关：加粗 / 斜体 / 下划线。 */
const PANEL_TOGGLES = [
  ['bold', 'B', { fontWeight: 'bold' }],
  ['italic', 'I', { fontStyle: 'italic' }],
  ['underline', 'U', { textDecoration: 'underline' }],
] as const satisfies readonly (readonly [PanelField, string, CSSProperties])[]

/**
 * 面板 API —— App 持有的全部状态与 handler 打包注入。
 * 签名与重构前逐字保持（`PaneApi` 是 App ↔ 面板的唯一契约）。
 */
export type PaneApi = {
  // 选区/持有
  def: ReportDef | null
  selection: { row0: number; col0: number; height: number; width: number } | null
  panelDisabled: boolean
  // 样式
  pv: PanelStyle
  onPanelChange: (field: PanelField, raw: string | number | boolean) => void
  onPanelReset: () => void
  // 属性
  propsPanel: ReturnType<typeof cellPropsFromDef>
  onPropsChange: (edit: CellPropsEdit) => void
  // 行列
  onRowHeightChange: (mm: number | null) => void
  onColWidthChange: (mm: number | null) => void
  onBreakPageChange: (on: boolean | null) => void
  gridDraft: { rows: number; cols: number }
  setGridDraft: (fn: (g: { rows: number; cols: number }) => { rows: number; cols: number }) => void
  onGridResize: () => void
  onParametersChange: (def: ReportDef) => void
  // 报表设置
  setupPanel: Record<keyof typeof PRINT_SETUP_DEFAULTS, number | boolean>
  onSetupChange: (field: keyof typeof PRINT_SETUP_DEFAULTS, value: number | boolean) => void
  onSwapPaper: () => void
  /** 结果视图下「返回模板设计」。 */
  onBackToDesign: () => void
}

// ---------------------------------------------------------------------------
// 小组件
// ---------------------------------------------------------------------------

/** 网格里的一行：左标签 + 右控件。 */
function KV({ k, children }: { k: string; children: ReactNode }) {
  return (
    <>
      <span className="k">{k}</span>
      <span className="v">{children}</span>
    </>
  )
}

/**
 * 一个属性分组。
 * `focus` 命中本组时滚进视野并高亮 1.2 秒（菜单/快捷键跳转的落点）。
 */
function Group({
  id,
  title,
  action,
  focus,
  children,
}: {
  id: PaneGroupId
  title: string
  action?: ReactNode
  focus: FocusSignal | null
  children: ReactNode
}) {
  const ref = useRef<HTMLDivElement>(null)
  const [flash, setFlash] = useState(false)
  useEffect(() => {
    if (!focus || focus.id !== id) return
    ref.current?.scrollIntoView({ block: 'nearest' })
    setFlash(true)
    const timer = setTimeout(() => setFlash(false), 1200)
    return () => clearTimeout(timer)
  }, [focus, id])
  return (
    <div ref={ref} className={flash ? 'rpx-group flash' : 'rpx-group'} data-group={id}>
      <div className="rpx-group-head">
        <h4>{title}</h4>
        {action ? <span className="act">{action}</span> : null}
      </div>
      {children}
    </div>
  )
}

// ---------------------------------------------------------------------------
// 单元格页签
// ---------------------------------------------------------------------------

function CellTab({ api, cellEditor, focus }: { api: PaneApi; cellEditor: ReactNode; focus: FocusSignal | null }) {
  const disabled = api.panelDisabled
  const selection = api.selection
  return (
    <>
      <Group id="content" title={GROUP_LABEL.content} focus={focus}>
        {cellEditor}
      </Group>

      <Group
        id="font"
        title={GROUP_LABEL.font}
        focus={focus}
        action={
          <button
            className="rpx-btn"
            onClick={api.onPanelReset}
            disabled={disabled}
            title="把 7 个样式字段全部打回缺省（= 从定义 JSON 里删掉这 7 个键）"
          >
            恢复缺省
          </button>
        }
      >
        <div className="rpx-grid">
          <KV k="字体">
            <select
              className="rpx-in"
              value={api.pv.fontName}
              disabled={disabled}
              title="字体族（定义 JSON 的 font_name；改回「宋体」= 删该键）"
              onChange={(e) => api.onPanelChange('fontName', e.target.value)}
            >
              {fontOptions(api.pv.fontName).map((f) => (
                <option key={f} value={f}>
                  {f}
                </option>
              ))}
            </select>
            <input
              className="rpx-in num"
              type="number"
              min={1}
              max={32767}
              value={api.pv.fontSize}
              disabled={disabled}
              title="字号（1..=32767 的整数；改回 12 = 删该键）"
              onChange={(e) => api.onPanelChange('fontSize', e.target.value)}
            />
          </KV>
          <KV k="字形">
            {PANEL_TOGGLES.map(([field, label, labelStyle]) => (
              <button
                key={field}
                className="rpx-tog"
                aria-pressed={api.pv[field]}
                disabled={disabled}
                title={`${label}（定义 JSON 的 ${field}）`}
                onClick={() => api.onPanelChange(field, !api.pv[field])}
                style={labelStyle}
              >
                {label}
              </button>
            ))}
          </KV>
          <KV k="颜色">
            <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
              <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>字</span>
              <input
                className="rpx-swatch"
                type="color"
                value={api.pv.foreColor}
                disabled={disabled}
                title="字体颜色（定义 JSON 的 fore_color；改回 #000000 = 删该键）"
                onChange={(e) => api.onPanelChange('foreColor', e.target.value)}
              />
            </label>
            <label style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
              <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>底</span>
              <input
                className="rpx-swatch"
                type="color"
                value={api.pv.backColor}
                disabled={disabled}
                title="背景色（定义 JSON 的 back_color；改回 #FFFFFF = 删该键）"
                onChange={(e) => api.onPanelChange('backColor', e.target.value)}
              />
            </label>
            {disabled ? <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>仅 JSON 报表源可改</span> : null}
          </KV>
        </div>
      </Group>

      <Group id="border" title={GROUP_LABEL.border} focus={focus}>
        <div className="rpx-grid">
          <KV k="边框">
            {BORDER_SIDES.map((s) => (
              <button
                key={s}
                className="rpx-tog"
                aria-pressed={api.propsPanel.sides[s]}
                disabled={disabled}
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
              >
                {BORDER_SIDE_LABELS[s]}
              </button>
            ))}
            <input
              className="rpx-swatch"
              type="color"
              value={api.propsPanel.borderColor}
              disabled={disabled}
              title="边框颜色（定义 JSON 的 border.*.color；#000000 = 删该键）"
              onChange={(e) => api.onPropsChange({ kind: 'borderColor', color: e.target.value })}
            />
          </KV>
          <KV k="线型">
            <select
              className="rpx-in"
              value={api.propsPanel.borderStyle}
              disabled={disabled}
              title="边框线型（改它只作用于已勾选的边，不凭空给没边框的边加边框）"
              onChange={(e) => api.onPropsChange({ kind: 'borderStyle', style: e.target.value })}
            >
              {borderStyleOptions(api.propsPanel.borderStyle).map((s) => (
                <option key={s} value={s}>
                  {LINE_STYLE_LABELS[s] ?? s}
                </option>
              ))}
            </select>
          </KV>
          <KV k="格式">
            <select
              className="rpx-in"
              value={api.propsPanel.format}
              disabled={disabled}
              title="数字格式（定义 JSON 的 format；「（无格式）」= 删该键）"
              onChange={(e) => api.onPropsChange({ kind: 'format', value: e.target.value })}
            >
              {formatOptions(api.propsPanel.format).map((f) => (
                <option key={f} value={f}>
                  {f === '' ? '（无格式）' : f}
                </option>
              ))}
            </select>
          </KV>
          <KV k="旋转">
            <input
              className="rpx-in num"
              type="number"
              min={0}
              max={360}
              value={api.propsPanel.rotation}
              disabled={disabled}
              title="文字旋转角（定义 JSON 的 rotation；面板开放 0..360 整数，0 = 删该键）"
              onChange={(e) => api.onPropsChange({ kind: 'rotation', value: e.target.value })}
            />
            <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>度</span>
          </KV>
        </div>
      </Group>

      <Group id="rowcol" title={GROUP_LABEL.rowcol} focus={focus}>
        <div className="rpx-grid">
          <KV k="行高">
            <input
              className="rpx-in num"
              type="number"
              min={0.1}
              step={0.5}
              value={
                selection && api.def
                  ? (rowHeightMm(api.def, selection.row0 + 1) ?? '')
                  : DEFAULT_ROW_MM
              }
              disabled={disabled}
              title="行高（mm，定义 JSON 的 row_props[].height；清空 = 删声明回到缺省 8mm）"
              onChange={(e) => api.onRowHeightChange(e.target.value === '' ? null : Number(e.target.value))}
            />
            <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>mm</span>
          </KV>
          <KV k="列宽">
            <input
              className="rpx-in num"
              type="number"
              min={0.1}
              step={0.5}
              value={
                selection && api.def
                  ? (colWidthMm(api.def, selection.col0 + 1) ?? '')
                  : DEFAULT_COL_MM
              }
              disabled={disabled}
              title="列宽（mm，定义 JSON 的 col_props[].width；清空 = 删声明回到缺省 25mm）"
              onChange={(e) => api.onColWidthChange(e.target.value === '' ? null : Number(e.target.value))}
            />
            <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>mm</span>
          </KV>
          <KV k="断页">
            <select
              className="rpx-in"
              value={
                selection && api.def
                  ? rowBreakPage(api.def, selection.row0 + 1) === true
                    ? 'on'
                    : rowBreakPage(api.def, selection.row0 + 1) === false
                      ? 'off'
                      : ''
                  : ''
              }
              disabled={disabled}
              title="行断页（定义 JSON 的 row_props[].break_page；画布无此概念，重渲染生效）"
              onChange={(e) => api.onBreakPageChange(e.target.value === '' ? null : e.target.value === 'on')}
            >
              <option value="">（未声明）</option>
              <option value="on">此行前断页</option>
              <option value="off">不断页</option>
            </select>
          </KV>
        </div>
      </Group>
    </>
  )
}

// ---------------------------------------------------------------------------
// 报表页签
// ---------------------------------------------------------------------------

const MARGIN_FIELDS = [
  ['left_margin', '左'],
  ['right_margin', '右'],
  ['top_margin', '上'],
  ['bottom_margin', '下'],
] as const

const SETUP_TOGGLES = [
  ['back_graph_printed', '背景图'],
  ['column_repeated_group_header', '重复列组头'],
  ['full_screen_preview', '全屏预览'],
  ['blank_line_filled', '空行填充'],
] as const

function ReportTab({ api, datasetEditor, focus }: { api: PaneApi; datasetEditor: ReactNode; focus: FocusSignal | null }) {
  const disabled = api.panelDisabled
  return (
    <>
      <Group id="page" title={GROUP_LABEL.page} focus={focus}>
        <div className="rpx-grid">
          <KV k="纸张">
            <input
              className="rpx-in num"
              type="number"
              min={0}
              step={1}
              value={Number(api.setupPanel.paper_width)}
              disabled={disabled}
              title="纸张宽（mm，定义 JSON 的 print_setup.paper_width）"
              onChange={(e) => api.onSetupChange('paper_width', Number(e.target.value))}
            />
            <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>×</span>
            <input
              className="rpx-in num"
              type="number"
              min={0}
              step={1}
              value={Number(api.setupPanel.paper_height)}
              disabled={disabled}
              title="纸张高（mm，定义 JSON 的 print_setup.paper_height）"
              onChange={(e) => api.onSetupChange('paper_height', Number(e.target.value))}
            />
            <button
              className="rpx-btn"
              onClick={api.onSwapPaper}
              disabled={disabled}
              title="横竖互换（宽高对调）"
            >
              横↔竖
            </button>
          </KV>
          <KV k="边距">
            {MARGIN_FIELDS.map(([field, label]) => (
              <label key={field} style={{ display: 'inline-flex', alignItems: 'center', gap: 3 }}>
                <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>{label}</span>
                <input
                  className="rpx-in tiny"
                  type="number"
                  min={0}
                  step={1}
                  value={Number(api.setupPanel[field])}
                  disabled={disabled}
                  title={`${label}边距（mm，定义 JSON 的 print_setup.${field}）`}
                  onChange={(e) => api.onSetupChange(field, Number(e.target.value))}
                />
              </label>
            ))}
          </KV>
        </div>
      </Group>

      <Group id="paginate" title={GROUP_LABEL.paginate} focus={focus}>
        <div className="rpx-grid">
          <KV k="每页行数">
            <input
              className="rpx-in num"
              type="number"
              min={0}
              step={1}
              value={Number(api.setupPanel.row_num_per_page)}
              disabled={disabled}
              title="每页行数（定义 JSON 的 print_setup.row_num_per_page；分页器直接用它）"
              onChange={(e) => api.onSetupChange('row_num_per_page', Number(e.target.value))}
            />
          </KV>
          <KV k="份数">
            <input
              className="rpx-in num"
              type="number"
              min={0}
              step={1}
              value={Number(api.setupPanel.copy_count)}
              disabled={disabled}
              title="打印份数（定义 JSON 的 print_setup.copy_count）"
              onChange={(e) => api.onSetupChange('copy_count', Number(e.target.value))}
            />
          </KV>
          <KV k="缩放">
            <input
              className="rpx-in num"
              type="number"
              min={0}
              max={32767}
              step={1}
              value={Number(api.setupPanel.zoom_scale)}
              disabled={disabled}
              title="缩放百分比（定义 JSON 的 print_setup.zoom_scale）"
              onChange={(e) => api.onSetupChange('zoom_scale', Number(e.target.value))}
            />
            <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>%</span>
          </KV>
          <KV k="分栏">
            <input
              className="rpx-in num"
              type="number"
              min={0}
              max={32767}
              step={1}
              value={Number(api.setupPanel.table_column_num)}
              disabled={disabled}
              title="分栏数（定义 JSON 的 print_setup.table_column_num）"
              onChange={(e) => api.onSetupChange('table_column_num', Number(e.target.value))}
            />
          </KV>
          <KV k="版面">
            <input
              className="rpx-in tiny"
              type="number"
              min={0}
              max={32767}
              step={1}
              value={Number(api.setupPanel.layout_rows)}
              disabled={disabled}
              title="版面行数（定义 JSON 的 print_setup.layout_rows）"
              onChange={(e) => api.onSetupChange('layout_rows', Number(e.target.value))}
            />
            <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>×</span>
            <input
              className="rpx-in tiny"
              type="number"
              min={0}
              max={32767}
              step={1}
              value={Number(api.setupPanel.layout_cols)}
              disabled={disabled}
              title="版面列数（定义 JSON 的 print_setup.layout_cols）"
              onChange={(e) => api.onSetupChange('layout_cols', Number(e.target.value))}
            />
          </KV>
          <span className="span2" style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
            {SETUP_TOGGLES.map(([field, label]) => (
              <button
                key={field}
                className="rpx-btn"
                aria-pressed={Boolean(api.setupPanel[field])}
                disabled={disabled}
                title={`${label}（定义 JSON 的 print_setup.${field}）`}
                onClick={() => api.onSetupChange(field, !api.setupPanel[field])}
                style={
                  Boolean(api.setupPanel[field]) && !disabled
                    ? { background: '#E8F1FA', borderColor: 'var(--rpx-accent)', color: 'var(--rpx-accent)' }
                    : undefined
                }
              >
                {label}
              </button>
            ))}
          </span>
        </div>
      </Group>

      <Group id="grid" title={GROUP_LABEL.grid} focus={focus}>
        <div className="rpx-grid">
          <KV k="行 × 列">
            <input
              className="rpx-in num"
              type="number"
              min={1}
              value={api.gridDraft.rows}
              disabled={disabled}
              title="网格行数（定义 JSON 的 grid.rows；缩小前会校验既有声明越界）"
              onChange={(e) => api.setGridDraft((g) => ({ ...g, rows: Number(e.target.value) }))}
            />
            <span style={{ fontSize: 11, color: 'var(--rpx-fg-muted)' }}>×</span>
            <input
              className="rpx-in num"
              type="number"
              min={1}
              value={api.gridDraft.cols}
              disabled={disabled}
              title="网格列数（定义 JSON 的 grid.cols）"
              onChange={(e) => api.setGridDraft((g) => ({ ...g, cols: Number(e.target.value) }))}
            />
            <button
              className="rpx-btn primary"
              onClick={api.onGridResize}
              disabled={disabled}
              title="按上面输入的行列数调整网格，并自动按新尺寸重渲染"
            >
              调整网格
            </button>
          </KV>
        </div>
      </Group>

      <Group id="params" title={GROUP_LABEL.params} focus={focus}>
        <TemplateParameters def={api.def} disabled={disabled} onChange={api.onParametersChange} />
      </Group>

      <Group id="datasets" title={GROUP_LABEL.datasets} focus={focus}>
        {datasetEditor}
      </Group>
    </>
  )
}

// ---------------------------------------------------------------------------
// 面板本体
// ---------------------------------------------------------------------------

/**
 * 右侧属性面板。显隐由 App 决定（视图菜单），本组件不管显隐。
 */
export default function PropertiesPanel({
  api,
  cellEditor,
  datasetEditor,
  tab,
  onTab,
  focus,
  view,
}: {
  api: PaneApi
  cellEditor: ReactNode
  datasetEditor: ReactNode
  tab: PaneTab
  onTab: (tab: PaneTab) => void
  focus: FocusSignal | null
  view: 'design' | 'result'
}) {
  return (
    <aside className="rpx-props" data-testid="side-panes" aria-label="属性面板">
      <div className="rpx-props-tabs" role="tablist" aria-label="属性分类">
        {/* 结果视图下两个页签都无内容可改，禁用比「点了没反应」诚实。 */}
        <button role="tab" aria-selected={tab === 'cell'} disabled={view === 'result'} onClick={() => onTab('cell')}>
          单元格
        </button>
        <button role="tab" aria-selected={tab === 'report'} disabled={view === 'result'} onClick={() => onTab('report')}>
          报表
        </button>
        {api.selection ? (
          <span className="addr" title={`当前选区 ${toAddress(api.selection.row0, api.selection.col0)}`}>
            {toAddress(api.selection.row0, api.selection.col0)}
          </span>
        ) : null}
      </div>
      <div className="rpx-props-body">
        {view === 'result' ? (
          <div className="rpx-note">
            运行结果为只读快照，属性不可编辑。
            <div style={{ marginTop: 8 }}>
              <button className="rpx-btn" onClick={api.onBackToDesign}>
                返回模板设计
              </button>
            </div>
          </div>
        ) : tab === 'cell' ? (
          <CellTab api={api} cellEditor={cellEditor} focus={focus} />
        ) : (
          <ReportTab api={api} datasetEditor={datasetEditor} focus={focus} />
        )}
      </div>
    </aside>
  )
}

/** 0 起行列号 → A1 记法。 */
function toAddress(row0: number, col0: number): string {
  let n = col0 + 1
  let letters = ''
  while (n > 0) {
    const rem = (n - 1) % 26
    letters = String.fromCharCode(65 + rem) + letters
    n = Math.floor((n - 1) / 26)
  }
  return `${letters}${row0 + 1}`
}
