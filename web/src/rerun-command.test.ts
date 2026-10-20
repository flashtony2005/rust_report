/**
 * `rerunCommand` / `libraryRoot` 单测。
 *
 * 契约：Web 端不跑引擎（`web_executes_engine: false`），设计器改完定义后
 * 必须由 CLI 重算；本模块负责把「库根 + id + 修订」拼成可一键复制的命令。
 */
import { describe, expect, it } from 'vitest'
import { libraryRoot, rerunCommand } from './rerun-command'

describe('libraryRoot', () => {
  it('Windows 路径去掉最后两级', () => {
    expect(libraryRoot('F:\\project\\report\\rpx\\examples\\library\\store-ranking\\store-ranking.json'))
      .toBe('F:\\project\\report\\rpx\\examples\\library')
  })

  it('POSIX 路径同样处理（分隔符统一成反斜杠）', () => {
    expect(libraryRoot('/home/u/lib/store-ranking/tpl.json')).toBe('\\home\\u\\lib')
  })

  it('结尾多余分隔符不计入层级', () => {
    expect(libraryRoot('F:\\lib\\id\\tpl.json\\')).toBe('F:\\lib')
  })

  it('层级不足时返回 undefined（无从反推）', () => {
    expect(libraryRoot('tpl.json')).toBeUndefined()
    expect(libraryRoot('a\\b')).toBeUndefined()
    expect(libraryRoot(undefined)).toBeUndefined()
    expect(libraryRoot('')).toBeUndefined()
  })
})

describe('rerunCommand', () => {
  it('拼出可直接执行的命令', () => {
    const command = rerunCommand({
      id: 'store-ranking',
      revision: 'abc123',
      templatePath: 'F:\\project\\report\\rpx\\examples\\library\\store-ranking\\store-ranking.json',
    })
    expect(command).toBe(
      'rpx report run store-ranking --if-match abc123 --library "F:\\project\\report\\rpx\\examples\\library"',
    )
  })

  it('缺模板路径时不给命令（而不是给一条错的）', () => {
    expect(rerunCommand({ id: 'x', revision: 'r' })).toBeUndefined()
  })
})
