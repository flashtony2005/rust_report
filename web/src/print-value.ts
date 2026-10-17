import { numfmt } from '@univerjs/core'

/** 使用画布已有的数字格式库；显示覆盖值（字符串）保持原文。 */
export function printValue(value: unknown, pattern?: string): string {
  return typeof value === 'number' && pattern ? numfmt.format(pattern, value) : String(value ?? '')
}
