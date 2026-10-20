/**
 * 「重算」命令生成器（**CLI 等价写法**）。
 *
 * # 现在的正路
 *
 * 设计器「结果」按钮直接打 `POST /reports/{id}/runs`，服务端跑引擎后回包
 * （见 `library-api.ts::executeReport`）—— 用户不需要命令行。
 *
 * # 本模块还留着干什么
 *
 * 给**脚本 / agent / 排查**用：同一次重算在命令行里的等价写法，拼好放进按钮的
 * `title` 与服务端日志对照。引擎始终在 Rust 侧执行，浏览器只发请求
 * （`web_executes_engine: false` 说的是「浏览器是否执行引擎」，与触发方式无关）。
 *
 * # 库根目录怎么来
 *
 * 服务端 `GET /reports/{id}` 返回 `template_path`（模板的**绝对路径**），
 * 形如 `<库根>/<id>/<template>.json`。库根 = 去掉最后两级。
 *
 * ⚠ 路径是 Windows 反斜杠。命令要能在 cmd / PowerShell 里直接跑，所以
 * 库根两侧加引号（反斜杠在引号内不会被转义），其余参数按 CLI 约定拼。
 */

/** 拼「重算」命令所需的输入。 */
export interface RerunCommandInput {
  /** 报表 id（库内目录名）。 */
  id: string
  /** 当前修订号（`If-Match` 用的那个）。 */
  revision: string
  /** 模板绝对路径（服务端 `template_path`）。 */
  templatePath?: string
}

/**
 * 从模板绝对路径反推库根目录。
 *
 * `<库根>/<id>/<template>.json` ⇒ 去掉最后两级 ⇒ `<库根>`。
 * 路径分隔符两种都认（Windows `\` 与 POSIX `/`）。
 */
export function libraryRoot(templatePath: string | undefined): string | undefined {
  if (!templatePath) return undefined
  const normalized = templatePath.replace(/[\\/]+$/, '')
  const parts = normalized.split(/[\\/]/)
  if (parts.length < 3) return undefined
  return parts.slice(0, -2).join('\\')
}

/**
 * 生成重算命令。
 *
 * @returns 命令文本；缺库根路径时返回 undefined（此时不能给命令）
 */
export function rerunCommand(input: RerunCommandInput): string | undefined {
  const root = libraryRoot(input.templatePath)
  if (!root) return undefined
  return `rpx report run ${input.id} --if-match ${input.revision} --library "${root}"`
}
