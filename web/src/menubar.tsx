/**
 * 菜单栏（M7-W W3-4）：产品式 IDE 菜单（对齐 reportLite.exe 的「菜单 + 属性
 * 面板」布局，产品菜单源码 `AppMenu.java` + 框架 `RPX.java:387`）。
 *
 * ## 菜单全集与 Web 映射（只列**有出品**的项；产品有但 Web 无实现的不列——
 * 「菜单里点了没反应」比「菜单里没有」更糟）
 *
 * | 菜单 | 项 | 落点 |
 * | --- | --- | --- |
 * | 文件 | 渲染 / 另存为（入库）/ 导出 JSON / 导出 Excel / 导出 CSV | App 既有 handler |
 * | 格式 | 样式面板 / 属性面板（边框格式旋转）显隐 | 右侧面板开关 |
 * | 报表 | 报表设置 / 行属性 / 数据集 / 参数 显隐 | 右侧面板开关 |
 * | 视图 | 右侧面板整体显隐 | `onToggleSide` |
 * | 帮助 | 关于（版本 + 链路说明） | 状态栏文案 |
 *
 * ⚠ 编辑类菜单（撤销/重做/剪贴板）**不做**：Univer 画布自带键盘/右键，
 * 再包一层菜单是重复 UI（产品那套是 Swing 需要）。
 */
import { useEffect, useRef, useState, type CSSProperties } from 'react'

/** 单个菜单项（分隔线用 `sep: true`；禁用态由调用方按持有状态算）。 */
export type MenuItem =
  | { sep: true }
  | {
      label: string
      onPick: () => void
      disabled?: boolean
      hint?: string
      /** 二级子菜单（产品「导出(E)→」「插入(I)→」同款；有 children 的项点击只展开）。 */
      children?: MenuItem[]
    }

/** 一个下拉菜单（标签 + 项表）。 */
export type MenuGroup = { label: string; items: MenuItem[] }

const menuBtnStyle: CSSProperties = {
  fontSize: 13,
  padding: '4px 10px',
  border: 'none',
  background: 'transparent',
  color: 'var(--rpx-fg)',
  cursor: 'pointer',
  borderRadius: 4,
}

const dropdownStyle: CSSProperties = {
  position: 'absolute',
  top: '100%',
  left: 0,
  minWidth: 180,
  background: '#fff',
  border: '1px solid var(--rpx-border)',
  borderRadius: 6,
  boxShadow: '0 4px 16px rgba(31,35,40,.12)',
  zIndex: 50,
  padding: '4px 0',
}

const itemStyle: CSSProperties = {
  display: 'block',
  width: '100%',
  textAlign: 'left',
  fontSize: 13,
  padding: '5px 14px',
  border: 'none',
  background: 'transparent',
  color: 'var(--rpx-fg)',
  cursor: 'pointer',
  // 菜单项一律不换行：长文案（如「放弃本地修改并读取共享源码」）会把行高
  // 撑成两倍，把下拉框拉得参差不齐。
  whiteSpace: 'nowrap',
}

/** 菜单栏：点击标签开下拉；点项执行并关闭；点别处/ESC 关闭。
 *  `brand` 是栏右侧的产品名（旧版顶栏的「rpx 报表设计器」标题搬迁至此——
 *  菜单占左、产品名占右，符合桌面 IDE 的习惯）。 */
export default function MenuBar({ groups, brand }: { groups: MenuGroup[]; brand?: string }) {
  const [open, setOpen] = useState<number | null>(null)
  const [sub, setSub] = useState<number | null>(null)
  const barRef = useRef<HTMLDivElement>(null)

  // 点菜单外区域或按 ESC ⇒ 收起（产品同款交互；不挂全局 blur 会漏掉画布点击）
  useEffect(() => {
    if (open === null) return
    const close = (e: MouseEvent) => {
      if (!barRef.current?.contains(e.target as Node)) {
        setOpen(null)
        setSub(null)
      }
    }
    const esc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setOpen(null)
        setSub(null)
      }
    }
    document.addEventListener('mousedown', close)
    document.addEventListener('keydown', esc)
    return () => {
      document.removeEventListener('mousedown', close)
      document.removeEventListener('keydown', esc)
    }
  }, [open])

  return (
    <div
      ref={barRef}
      role="menubar"
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 2,
        padding: '2px 8px',
        borderBottom: '1px solid var(--rpx-border)',
        background: 'var(--rpx-bg-muted)',
        fontFamily: 'system-ui, sans-serif',
      }}
    >
      {groups.map((g, gi) => (
        <div key={g.label} style={{ position: 'relative' }}>
          <button
            role="menuitem"
            aria-haspopup="menu"
            aria-expanded={open === gi}
            onClick={() => {
              setSub(null)
              setOpen(open === gi ? null : gi)
            }}
            style={{
              ...menuBtnStyle,
              ...(open === gi ? { background: '#fff', border: '1px solid var(--rpx-border)' } : {}),
            }}
          >
            {g.label}
          </button>
          {open === gi ? (
            <div role="menu" style={dropdownStyle}>
              {g.items.map((it, ii) =>
                'sep' in it ? (
                  <div
                    key={ii}
                    style={{ height: 1, background: 'var(--rpx-border)', margin: '4px 0' }}
                  />
                ) : (
                  <div key={ii} style={{ position: 'relative' }}>
                    <button
                      role="menuitem"
                      disabled={it.disabled}
                      title={it.hint}
                      aria-haspopup={it.children ? 'menu' : undefined}
                      onClick={() => {
                        if (it.children) {
                          setSub(sub === ii ? null : ii)
                          return
                        }
                        setOpen(null)
                        it.onPick()
                      }}
                      onMouseEnter={(e) => {
                        if (!it.disabled) e.currentTarget.style.background = 'var(--rpx-bg-muted)'
                        if (it.children) setSub(ii)
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.background = 'transparent'
                      }}
                      style={{
                        ...itemStyle,
                        cursor: it.disabled ? 'not-allowed' : 'pointer',
                        color: it.disabled ? 'var(--rpx-fg-muted)' : 'var(--rpx-fg)',
                      }}
                    >
                      {it.label}
                      {it.children ? (
                        <span style={{ float: 'right', color: 'var(--rpx-fg-muted)' }}>▸</span>
                      ) : null}
                    </button>
                    {it.children && sub === ii ? (
                      <div
                        role="menu"
                        style={{ ...dropdownStyle, top: 0, left: '100%', marginLeft: 1 }}
                      >
                        {it.children.map((c, ci) =>
                          'sep' in c ? (
                            <div
                              key={ci}
                              style={{
                                height: 1,
                                background: 'var(--rpx-border)',
                                margin: '4px 0',
                              }}
                            />
                          ) : (
                            <button
                              key={ci}
                              role="menuitem"
                              disabled={c.disabled}
                              title={c.hint}
                              onClick={() => {
                                setOpen(null)
                                setSub(null)
                                c.onPick()
                              }}
                              onMouseEnter={(e) => {
                                if (!c.disabled)
                                  e.currentTarget.style.background = 'var(--rpx-bg-muted)'
                              }}
                              onMouseLeave={(e) => {
                                e.currentTarget.style.background = 'transparent'
                              }}
                              style={{
                                ...itemStyle,
                                whiteSpace: 'nowrap',
                                cursor: c.disabled ? 'not-allowed' : 'pointer',
                                color: c.disabled ? 'var(--rpx-fg-muted)' : 'var(--rpx-fg)',
                              }}
                            >
                              {c.label}
                            </button>
                          ),
                        )}
                      </div>
                    ) : null}
                  </div>
                ),
              )}
            </div>
          ) : null}
        </div>
      ))}
      {brand ? (
        <span
          style={{
            marginLeft: 'auto',
            paddingRight: 6,
            fontSize: 12,
            color: 'var(--rpx-fg-muted)',
            whiteSpace: 'nowrap',
          }}
        >
          {brand}
        </span>
      ) : null}
    </div>
  )
}
