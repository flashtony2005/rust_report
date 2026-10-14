/**
 * 快照装载（M4）：取 JSON → Univer 工作簿。
 *
 * 契约：`docs/spec-m4.md` §1/§2 —— 前端**零业务逻辑**，只负责装载；
 * 数据管道 v1 = 本地文件 `/snapshot.json`（`rpx-server snapshot` 产物，§6 工作流）；
 * v2（M4-06，M3-05 后）= 换 `POST /api/reports/{id}/render`，仅改 [`fetchSnapshot`]。
 *
 * ⚠ import 路径以本机安装的 Univer 1.0.4 实际结构为准（2026-10-14 校准，见
 * `web/node_modules/@univerjs/preset-sheets-core`）。
 */
import { LocaleType, createUniver, mergeLocales, type FUniver } from '@univerjs/presets'
import { UniverSheetsCorePreset } from '@univerjs/preset-sheets-core'
import UniverPresetSheetsCoreZhCN from '@univerjs/preset-sheets-core/locales/zh-CN'
import '@univerjs/preset-sheets-core/lib/index.css'

/** 取服务端快照 JSON（`IWorkbookData`，契约见 `docs/spec-m4.md` §4）。 */
export async function fetchSnapshot(): Promise<unknown> {
  // v1：vite `public/` 静态文件；v2 换 fetch('/api/reports/1/render', { method: 'POST' })
  const resp = await fetch('/snapshot.json')
  if (!resp.ok) {
    throw new Error(
      `取快照失败：HTTP ${resp.status} —— 先跑 \`cargo run -p rpx-server -- snapshot <file.rpx> -o web/public/snapshot.json\``,
    )
  }
  return resp.json()
}

/** 在容器里挂 Univer，返回门面 API。 */
export function mountUniver(container: HTMLElement): FUniver {
  const { univerAPI } = createUniver({
    locale: LocaleType.ZH_CN,
    locales: { [LocaleType.ZH_CN]: mergeLocales(UniverPresetSheetsCoreZhCN) },
    presets: [UniverSheetsCorePreset({ container })],
  })
  return univerAPI
}
