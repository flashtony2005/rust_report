/**
 * 后端连通性：把「Rust 服务端到底在不在」显示出来。
 *
 * 为什么值得单独一个 hook：这个壳（`app/`）是别人按 mock 写的，页面上的数字全是
 * 浏览器造的假数据。**唯一能证明它连着真引擎的，就是侧栏那颗灯**。
 * 灯灭着，界面看着再完整也只是原型（见 docs/merge-master-analysis.md §0.2）。
 *
 * 判据两条，缺一不可：
 *   1. `GET /api/health` 返回 `ok`；
 *   2. `GET /api/reports?limit=1` 能拿到分页体（说明库目录也被服务端读到了）。
 */
import { useCallback, useEffect, useState } from 'react';
import { ApiError, health, listReports } from './api';

export type BackendState = 'loading' | 'ok' | 'down';

export interface BackendStatus {
  state: BackendState;
  /** 报表库内份数（`total`，服务端分页体给的）。 */
  reports: number | null;
  /** 失败原因（连不上/鉴权/库目录没配），直接展示给用户，不要吞掉。 */
  error: string | null;
  retry: () => void;
}

export function useBackendStatus(): BackendStatus {
  const [state, setState] = useState<BackendState>('loading');
  const [reports, setReports] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [nonce, setNonce] = useState(0);

  const retry = useCallback(() => setNonce((n) => n + 1), []);

  useEffect(() => {
    let alive = true;
    setState('loading');
    setError(null);

    (async () => {
      try {
        const text = await health();
        if (text.trim() !== 'ok') throw new Error(`/health 返回 ${JSON.stringify(text)}`);
        const page = await listReports({ limit: 1 });
        if (!alive) return;
        setReports(typeof page.total === 'number' ? page.total : page.items.length);
        setState('ok');
      } catch (e) {
        if (!alive) return;
        const msg =
          e instanceof ApiError
            ? e.status === 401
              ? '需要 Bearer token（服务端启了 --auth-token）'
              : `${e.code}：${e.message}`
            : e instanceof Error
              ? e.message
              : String(e);
        setError(msg || '连不上 8090');
        setState('down');
      }
    })();

    return () => {
      alive = false;
    };
  }, [nonce]);

  return { state, reports, error, retry };
}
