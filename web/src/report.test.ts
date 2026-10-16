/**
 * 纯逻辑单测（M7-W W0-2）：`report.ts` 的两个纯函数。
 *
 * 为什么先测这两个：本仓纪律是「判据有效性一律用故障注入证明」
 * （`docs/code-review.md`），而前端此前**没有任何机器判据**（无测试运行器）。
 * 这两个函数是 `report.ts` 里唯一不碰网络/DOM 的部分 —— 先把地基的可验证性
 * 立起来（`docs/spec-m7-web.md` §4 W0），再谈设计器。
 *
 * ⚠ `fetchSnapshot` / `mountUniver` **故意不在此测**：它们要真网络与真 Univer
 * canvas，属 W1/W2 的端到端范畴，不该塞进纯逻辑单测。
 */
import { describe, expect, it } from 'vitest'

import { paramsToObject, pickReportFile, type ParamRow } from './report'

/** 造一个 `File`（jsdom/node 下 `new File([], name)` 就够，只看 `.name`）。 */
function file(name: string): File {
  return new File([], name)
}

describe('pickReportFile', () => {
  it('挑出 .rpx（大小写不敏感）', () => {
    expect(pickReportFile([file('a.csv'), file('t.rpx')])?.name).toBe('t.rpx')
    expect(pickReportFile([file('T.RPX')])?.name).toBe('T.RPX')
    expect(pickReportFile([file('t.Rpx')])?.name).toBe('t.Rpx')
  })

  it('挑不到返回 undefined（调用方据此抛「未选择 .rpx 模板文件」）', () => {
    expect(pickReportFile([])).toBeUndefined()
    expect(pickReportFile([file('a.csv'), file('b.json')])).toBeUndefined()
  })

  it('多个 .rpx 时取第一个（与改前的 `Array.find` 同口径）', () => {
    expect(pickReportFile([file('a.rpx'), file('b.rpx')])?.name).toBe('a.rpx')
  })

  it('不把 .rpx.csv 之类的后缀当模板', () => {
    expect(pickReportFile([file('a.rpx.csv')])).toBeUndefined()
  })
})

describe('paramsToObject', () => {
  /** 单行快写。 */
  const one = (name: string, value: string) => paramsToObject([{ name, value }])

  it('整数字面量 → number', () => {
    expect(one('topN', '10')).toEqual({ topN: 10 })
    expect(one('n', '-7')).toEqual({ n: -7 })
    expect(one('n', '0')).toEqual({ n: 0 })
  })

  it('小数与科学计数 → number', () => {
    expect(one('rate', '3.14')).toEqual({ rate: 3.14 })
    expect(one('r', '.5')).toEqual({ r: 0.5 })
    expect(one('r', '2.')).toEqual({ r: 2 })
    expect(one('r', '1.0e3')).toEqual({ r: 1000 })
    expect(one('r', '-2.5E-2')).toEqual({ r: -0.025 })
  })

  /**
   * ⚠ **既有实现的真实口径**（写测试时才发现文档与实现不符）：
   * 小数正则 `/^-?(\d+\.\d*|\.\d+)([eE][+-]?\d+)?$/` **要求有小数点**，
   * 所以**没有小数点的科学计数不是数字** —— `1e3` 走字符串分支。
   *
   * `paramsToObject` 的文档此前笼统写「小数/科学计数 → Number」，与实现不符。
   * **这条测试钉住实现**；若要把 `1e3` 也当数字，**先改测试再改实现**
   * （本仓纪律：判据先行），并同步 `form.rs::json_to_value` 的口径 ——
   * 前端猜类型、服务端不猜，两边不一致会静默出错。
   */
  it('无小数点的科学计数是字符串（既有行为：正则要求小数点）', () => {
    expect(one('r', '1e3')).toEqual({ r: '1e3' })
    expect(one('r', '1E3')).toEqual({ r: '1E3' })
    expect(one('r', '2e-3')).toEqual({ r: '2e-3' })
  })

  it('其余 → 字符串（不猜类型）', () => {
    expect(one('d', '2024-01-01')).toEqual({ d: '2024-01-01' })
    expect(one('s', 'abc')).toEqual({ s: 'abc' })
    expect(one('s', '')).toEqual({ s: '' })
    // 明显不是合法数字的形态必须留在字符串，不能变成 NaN。
    expect(one('s', '1.2.3')).toEqual({ s: '1.2.3' })
    expect(one('s', '1e')).toEqual({ s: '1e' })
    expect(one('s', '+5')).toEqual({ s: '+5' })
  })

  it('数字走 trim（首尾空格仍解析）', () => {
    expect(one('n', '  42  ')).toEqual({ n: 42 })
    expect(one('r', ' 3.5 ')).toEqual({ r: 3.5 })
  })

  /** ⚠ 这条钉住**既有行为**：字符串值**不 trim**。见 `paramsToObject` 的文档。 */
  it('字符串值不 trim（既有行为，刻意保留）', () => {
    expect(one('s', '  abc  ')).toEqual({ s: '  abc  ' })
  })

  it('空名（trim 后）整行跳过', () => {
    expect(paramsToObject([{ name: '', value: '1' }, { name: '   ', value: '2' }])).toEqual({})
  })

  it('空行 + 有效行混排：只留有效的', () => {
    const rows: ParamRow[] = [
      { name: '', value: 'x' },
      { name: 'topN', value: '10' },
      { name: '  ', value: 'y' },
      { name: 'region', value: 'A' },
    ]
    expect(paramsToObject(rows)).toEqual({ topN: 10, region: 'A' })
  })

  it('同名参数后者覆盖前者（与 `obj[n] = …` 的赋值语义一致）', () => {
    expect(
      paramsToObject([
        { name: 'p', value: '1' },
        { name: 'p', value: '2' },
      ]),
    ).toEqual({ p: 2 })
  })

  it('空数组 → 空对象（调用方据此**不提交** params 段）', () => {
    expect(paramsToObject([])).toEqual({})
  })

  it('键序 = 声明序（JSON.stringify 的顺序即提交顺序）', () => {
    expect(
      Object.keys(
        paramsToObject([
          { name: 'b', value: '2' },
          { name: 'a', value: '1' },
        ]),
      ),
    ).toEqual(['b', 'a'])
  })
})
