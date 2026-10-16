/**
 * 数据集绑定补丁单测（M7-W W2-5b）。
 *
 * 判据分四组：
 *
 * 1. **校验镜像 `parse_dataset`**（`reportdef/parse.rs:277`）—— sql/file 都给、
 *    都不给、dsn/sql/file_name 缺失各自显式报错（错误口径对齐服务端语义）。
 * 2. **增/删/改**（`applyDatasetAdd` / `Remove` / `Update`）—— 下标定位、
 *    越界显式报错、Update 是整条替换（否则删不掉 dsn）、未知键不无端保留。
 * 3. **重名告警不拦截**（`parse.rs` 无重名校验已核源码 ⇒ 前端不比服务端严）。
 * 4. **纯函数性** + 读方向（`datasetsOf` / `datasetSummary`）。
 */
import { describe, expect, it } from 'vitest'

import type { ReportDef } from './designer'
import {
  applyDatasetAdd,
  applyDatasetRemove,
  applyDatasetUpdate,
  datasetSummary,
  datasetsOf,
  type DatasetInput,
} from './datasets'

const baseDef = (): ReportDef => ({
  version: 1,
  grid: { rows: 3, cols: 2 },
  cells: [{ row: 1, col: 1, text: 'A' }],
})

const sqlDs = (): DatasetInput => ({ name: 'ds1', sql: { dsn: 'kingbase', sql: 'SELECT 1' } })
const fileDs = (): DatasetInput => ({ name: 'f1', file: { file_name: 'emp.csv' } })

const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T

// ---------------------------------------------------------------------------
// 1. 校验（镜像 parse_dataset）
// ---------------------------------------------------------------------------

describe('校验：镜像 parse_dataset', () => {
  it('sql 与 file 都给 ⇒ 报错（互斥）', () => {
    const both: DatasetInput = {
      name: 'x',
      sql: { dsn: 'a', sql: 'SELECT 1' },
      file: { file_name: 'b.csv' },
    }
    const r = applyDatasetAdd(baseDef(), both)
    expect(r.error).toContain('互斥')
    expect(r.def).toBeUndefined()
  })

  it('都不给 ⇒ 报错', () => {
    expect(applyDatasetAdd(baseDef(), { name: 'x' }).error).toContain('必须给')
    expect(applyDatasetAdd(baseDef(), {}).error).toContain('必须给')
  })

  it('sql 分支：dsn / sql 缺失各自报错', () => {
    expect(applyDatasetAdd(baseDef(), { sql: { dsn: '', sql: 'SELECT 1' } }).error).toContain('dsn')
    expect(applyDatasetAdd(baseDef(), { sql: { dsn: 'a', sql: '  ' } }).error).toContain('sql 语句')
  })

  it('file 分支：file_name 缺失报错；encoding 可选（缺省 GBK 由服务端落）', () => {
    expect(applyDatasetAdd(baseDef(), { file: { file_name: '  ' } }).error).toContain('file_name')
    const r = applyDatasetAdd(baseDef(), fileDs())
    expect(r.error).toBeUndefined()
    expect(r.def?.datasets).toEqual([{ name: 'f1', file: { file_name: 'emp.csv' } }])
  })

  it('Update 同样先过校验（替换不豁免）', () => {
    const def = applyDatasetAdd(baseDef(), fileDs()).def!
    expect(applyDatasetUpdate(def, 0, {}).error).toContain('必须给')
  })
})

// ---------------------------------------------------------------------------
// 2. 增 / 删 / 改
// ---------------------------------------------------------------------------

describe('增删改', () => {
  it('Add：追加并保留其余 JSON', () => {
    const def: ReportDef = { ...baseDef(), params: [{ name: 'p', data_type: 'int' }] }
    const r = applyDatasetAdd(def, sqlDs())
    expect(r.def?.datasets).toEqual([{ name: 'ds1', sql: { dsn: 'kingbase', sql: 'SELECT 1' } }])
    expect(r.def?.params).toEqual([{ name: 'p', data_type: 'int' }])
    expect(r.warnings.join()).toContain('CSV')
  })

  it('Add：多条共存（sql 与 file 混合）', () => {
    let def = applyDatasetAdd(baseDef(), sqlDs()).def!
    def = applyDatasetAdd(def, fileDs()).def!
    expect(def.datasets).toHaveLength(2)
    expect((def.datasets as unknown[])[1]).toEqual({ name: 'f1', file: { file_name: 'emp.csv' } })
  })

  it('Remove：按下标删；越界显式报错', () => {
    let def = applyDatasetAdd(baseDef(), sqlDs()).def!
    def = applyDatasetAdd(def, fileDs()).def!
    const r = applyDatasetRemove(def, 0)
    expect(r.def?.datasets).toEqual([{ name: 'f1', file: { file_name: 'emp.csv' } }])
    expect(applyDatasetRemove(def, 5).error).toContain('下标 5 不存在')
  })

  it('Update：整条替换（换类型 ⇒ 旧键整体作废）', () => {
    let def = applyDatasetAdd(baseDef(), sqlDs()).def!
    const r = applyDatasetUpdate(def, 0, fileDs())
    expect(r.def?.datasets).toEqual([{ name: 'f1', file: { file_name: 'emp.csv' } }])
    expect((r.def?.datasets as Array<Record<string, unknown>> | undefined)?.[0]?.sql).toBeUndefined()
  })

  it('Update：越界报错', () => {
    expect(applyDatasetUpdate(baseDef(), 0, sqlDs()).error).toContain('不存在')
  })

  it('file.encoding 显式给时保留', () => {
    const r = applyDatasetAdd(baseDef(), { file: { file_name: 'a.csv', encoding: 'UTF-8' } })
    expect(r.def?.datasets).toEqual([
      { file: { file_name: 'a.csv', encoding: 'UTF-8' } },
    ])
  })

  it('纯函数：输入 def 不被改写', () => {
    const def = applyDatasetAdd(baseDef(), sqlDs()).def!
    const snapshot = clone(def)
    applyDatasetRemove(def, 0)
    applyDatasetUpdate(def, 0, fileDs())
    expect(def).toEqual(snapshot)
  })
})

// ---------------------------------------------------------------------------
// 3. 重名：告警不拦截
// ---------------------------------------------------------------------------

describe('重名告警', () => {
  it('Add 重名 ⇒ 有告警但仍成功', () => {
    let def = applyDatasetAdd(baseDef(), sqlDs()).def!
    const r = applyDatasetAdd(def, sqlDs())
    expect(r.error).toBeUndefined()
    expect(r.def?.datasets).toHaveLength(2)
    expect(r.warnings.join()).toContain('重复')
  })

  it('Update 改成与别条重名 ⇒ 告警；不重名 ⇒ 无告警', () => {
    let def = applyDatasetAdd(baseDef(), sqlDs()).def!
    def = applyDatasetAdd(def, fileDs()).def!
    const r = applyDatasetUpdate(def, 1, { name: 'ds1', file: { file_name: 'b.csv' } })
    expect(r.error).toBeUndefined()
    expect(r.warnings.join()).toContain('重复')
    const ok = applyDatasetUpdate(def, 1, { name: 'f2', file: { file_name: 'b.csv' } })
    expect(ok.warnings.join()).not.toContain('重复')
  })
})

// ---------------------------------------------------------------------------
// 4. 读方向
// ---------------------------------------------------------------------------

describe('读方向', () => {
  it('datasetsOf：未声明 ⇒ 空数组', () => {
    expect(datasetsOf(baseDef())).toEqual([])
  })

  it('datasetSummary：sql / file / 未声明三种形态', () => {
    expect(datasetSummary({ sql: { dsn: 'kb', sql: 'SELECT a, b FROM t' } })).toContain('SQL · kb')
    expect(datasetSummary({ file: { file_name: 'emp.csv', encoding: 'UTF-8' } })).toContain('文件 · emp.csv · UTF-8')
    expect(datasetSummary({})).toContain('未声明')
  })
})
