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

import {
  isReportSource,
  metaToRows,
  paramsToObject,
  pickReportFile,
  type ParamMetaItem,
  type ParamRow,
} from './report'

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

  /**
   * ⚠ **本测试 2026-10-16（W1-1）刻意反转**：M4 时期这里断言
   * `pickReportFile([a.csv, b.json])` 为 `undefined`（「.json 不是模板」）。
   * W1-1 把定义 JSON 接进 `/render` 后，`.json` **就是**报表源 —— 旧断言
   * 描述的是 M4 的口径，现在它是错的。
   *
   * 按本仓纪律（口径修正加日期注，不改写历史结论）：旧断言原样留在这条注释里，
   * 实测断言翻转。**若 W1-1 被回退，这条必须改回去** —— 它不是可有可无的
   * 措辞，是「.json 通道开着」的判据。
   */
  it('挑出 .json 定义（W1-1：反转了 M4 的「.json 不是模板」断言）', () => {
    expect(pickReportFile([file('a.csv'), file('b.json')])?.name).toBe('b.json')
    expect(pickReportFile([file('d.JSON')])?.name).toBe('d.JSON')
  })

  it('.rpx 与 .json 同时选中时 .rpx 优先（M4 行为逐字不变）', () => {
    expect(pickReportFile([file('b.json'), file('a.rpx')])?.name).toBe('a.rpx')
    // 顺序无关：哪怕 .json 排在前，也仍然先给 .rpx。
    expect(pickReportFile([file('a.rpx'), file('b.json')])?.name).toBe('a.rpx')
  })

  it('挑不到返回 undefined（调用方据此抛「未选择报表源文件」）', () => {
    expect(pickReportFile([])).toBeUndefined()
    expect(pickReportFile([file('a.csv'), file('b.txt')])).toBeUndefined()
  })

  it('多个 .rpx 时取第一个（与改前的 `Array.find` 同口径）', () => {
    expect(pickReportFile([file('a.rpx'), file('b.rpx')])?.name).toBe('a.rpx')
  })

  it('不把 .rpx.csv 之类的后缀当模板', () => {
    expect(pickReportFile([file('a.rpx.csv')])).toBeUndefined()
    // W1-1 加宽后同理：`x.json.csv` 也不是报表源（判据是「以该后缀结尾」）。
    expect(pickReportFile([file('a.json.csv')])).toBeUndefined()
  })
})

describe('isReportSource', () => {
  it('.rpx 与 .json 都是报表源（大小写不敏感）', () => {
    expect(isReportSource('t.rpx')).toBe(true)
    expect(isReportSource('t.JSON')).toBe(true)
    expect(isReportSource('t.csv')).toBe(false)
    expect(isReportSource('t.rpx.csv')).toBe(false)
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

  /**
   * W1-2 的隔离判据：`dynamic` 只是表单标注（`is_static` 派生），
   * **不得泄漏进提交给服务端的 params 对象** —— 服务端 `form.rs::parse_params`
   * 会把不认识的值按「未登记参数」显式报错，泄漏即 400。
   */
  it('dynamic 标记不会泄漏进 params', () => {
    expect(
      paramsToObject([
        { name: 'region', value: 'A', dynamic: true },
        { name: 'topN', value: '10' },
      ]),
    ).toEqual({ region: 'A', topN: 10 })
  })
})

describe('metaToRows', () => {
  /** 造一条 `/meta` 条目（只覆盖 `metaToRows` 真正消费的字段）。 */
  const meta = (over: Partial<ParamMetaItem>): ParamMetaItem => ({
    name: 'p',
    param_type: 0,
    is_static: true,
    data_type: 11,
    default_val: null,
    description: null,
    ...over,
  })

  it('空清单 → 空表单（该报表未声明参数）', () => {
    expect(metaToRows([])).toEqual([])
  })

  it('静态参数：默认值预填进 value，dynamic=false', () => {
    expect(metaToRows([meta({ name: 'startDate', default_val: '2024-01-01' })])).toEqual([
      { name: 'startDate', value: '2024-01-01', dynamic: false },
    ])
  })

  it('动态参数：is_static=false ⇒ dynamic=true，无默认值则留空等人填', () => {
    // param_type=1（PARAM_DYNAMIC）与 2（PARAM_ATTRIBUTE）都派生 is_static=false，
    // 见 `rpx-ds/src/asset.rs::asset.rs` 的 `is_static = param_type == 0`。
    expect(metaToRows([meta({ name: 'region', param_type: 1, is_static: false })])).toEqual([
      { name: 'region', value: '', dynamic: true },
    ])
    expect(metaToRows([meta({ name: 'dept', param_type: 2, is_static: false })])).toEqual([
      { name: 'dept', value: '', dynamic: true },
    ])
  })

  it('default_val 为 null 时 value 是空串（不是 "null"、不是 undefined）', () => {
    expect(metaToRows([meta({ default_val: null })])).toEqual([
      { name: 'p', value: '', dynamic: false },
    ])
  })

  it('顺序 = 清单顺序（服务端按 rowid 排序，即声明序）', () => {
    expect(metaToRows([meta({ name: 'b' }), meta({ name: 'a' })])).toEqual([
      { name: 'b', value: '', dynamic: false },
      { name: 'a', value: '', dynamic: false },
    ])
  })

  /** W1-2 判据的闭环：metaToRows 的产物**能直接进** paramsToObject 提交。 */
  it('产物可直连 paramsToObject（不靠手工再拼一次）', () => {
    const rows: ParamRow[] = metaToRows([
      meta({ name: 'startDate', default_val: '2024-01-01' }),
      meta({ name: 'topN', data_type: 1, default_val: '10' }),
      meta({ name: 'region', param_type: 1, is_static: false }),
    ])
    expect(paramsToObject(rows)).toEqual({ startDate: '2024-01-01', topN: 10, region: '' })
  })
})
