import { describe, expect, it } from 'vitest'
import { borderSides, cellBorders } from './print-border'

/**
 * 边框换算的钉子。
 *
 * 为什么值得钉：打印预览是自研 HTML 渲染，Univer 画的边框不会自动上纸 ——
 * 一旦这张对照表错了，用户看到的是「画布有框、纸上没框」或者「1pt 细线印成 3pt 粗线」，
 * 而这两种错误在截图上都很容易被当成「样式没生效」。
 */
describe('borderSides：Univer 边框 → CSS', () => {
  it('四种线型各走各的档位', () => {
    expect(borderSides({ t: { s: 1, cl: { rgb: '#FF0000' } } })).toEqual({ borderTop: '0.75pt solid #FF0000' })
    expect(borderSides({ r: { s: 3, cl: { rgb: '#00FF00' } } })).toEqual({ borderRight: '1pt dotted #00FF00' })
    expect(borderSides({ b: { s: 4, cl: { rgb: '#0000FF' } } })).toEqual({ borderBottom: '1pt dashed #0000FF' })
    expect(borderSides({ l: { s: 7, cl: { rgb: '#123456' } } })).toEqual({ borderLeft: '2.25pt double #123456' })
  })

  it('粗细档位：MEDIUM 1.5pt、THICK 2.25pt', () => {
    expect(borderSides({ t: { s: 8, cl: { rgb: '#000000' } } }).borderTop).toBe('1.5pt solid #000000')
    expect(borderSides({ t: { s: 13, cl: { rgb: '#000000' } } }).borderTop).toBe('2.25pt solid #000000')
  })

  it('none(0) 与未知码都不画（不能把「无边框」画成细线）', () => {
    expect(borderSides({ t: { s: 0, cl: { rgb: '#000000' } } })).toEqual({})
    expect(borderSides({ t: { s: 99, cl: { rgb: '#000000' } } })).toEqual({})
  })

  it('颜色槽两种写法都认，都没有就按黑色', () => {
    expect(borderSides({ t: { s: 1, c: { rgb: '#ABCDEF' } } }).borderTop).toBe('0.75pt solid #ABCDEF')
    expect(borderSides({ t: { s: 1 } }).borderTop).toBe('0.75pt solid #000000')
  })

  it('形状不符（undefined / 数组 / 字符串）⇒ 空，不抛', () => {
    expect(borderSides(undefined)).toEqual({})
    expect(borderSides([])).toEqual({})
    expect(borderSides('bd')).toEqual({})
    expect(borderSides({ t: null })).toEqual({})
  })
})

describe('cellBorders：逐边兜底网格线', () => {
  const grid = '1pt solid #000000'

  it('有显式边框的边不被网格线覆盖，其余边补网格线', () => {
    expect(cellBorders({ t: { s: 1, cl: { rgb: '#FF0000' } } }, grid)).toEqual({
      borderTop: '0.75pt solid #FF0000',
      borderRight: grid,
      borderBottom: grid,
      borderLeft: grid,
    })
  })

  it('没有网格线时只返回显式边框', () => {
    expect(cellBorders({ l: { s: 4, cl: { rgb: '#00FF00' } } }, undefined)).toEqual({ borderLeft: '1pt dashed #00FF00' })
  })

  it('没有网格线也没有边框 ⇒ 空对象（td 上不写 border 属性）', () => {
    expect(cellBorders(undefined, undefined)).toEqual({})
  })
})
