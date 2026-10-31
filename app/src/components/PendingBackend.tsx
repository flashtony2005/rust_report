import React from 'react';

/**
 * 「待后端」页面的统一外壳。
 *
 * 为什么不做成空白或"敬请期待"：这些页面背后是**语义层**（数据源探测、业务模型、查询 IR、
 * 归因分析），目前只有 Python 实现（`studio/`，已冻结），Rust 侧正在搬
 * （`crates/rpx-semantic`，见 `docs/rpx-semantic-port.md`）。
 *
 * 所以这里必须写清三件事：**这个页面要什么端、端还没在 Rust 上、现在能用什么代替**。
 * 不许用前端 mock 把空状态填满 —— 那会让人以为功能已经好了（`app/README.md` 的第一条红线）。
 */
export interface PendingBackendProps {
  title: string;
  eyebrow: string;
  /** 一句话说明这一页将来做什么。 */
  summary: string;
  /** 需要的 Rust 端点（写路径与用途，未来照这张表实现）。 */
  endpoints: Array<{ method: string; path: string; note: string }>;
  /** 现在能用的替代路径（指向真能跑的地方，别写"敬请期待"）。 */
  fallback?: React.ReactNode;
}

export const PendingBackend: React.FC<PendingBackendProps> = ({
  title,
  eyebrow,
  summary,
  endpoints,
  fallback,
}) => (
  <>
    <div className="page-head">
      <div className="grow">
        <div className="eyebrow">{eyebrow}</div>
        <h1>{title}</h1>
        <div className="sub">{summary}</div>
      </div>
      <span className="badge analyzed">待后端</span>
    </div>

    <div className="card pad">
      <div className="eyebrow" style={{ marginBottom: 10 }}>
        Rust 侧还缺这些端点
      </div>
      <div className="list" style={{ border: 0, boxShadow: 'none', background: 'transparent' }}>
        {endpoints.map((e) => (
          <div className="list-row" key={e.method + e.path} style={{ cursor: 'default', paddingLeft: 0 }}>
            <code className="sql-inline" style={{ flex: '0 0 auto' }}>
              {e.method} {e.path}
            </code>
            <div className="grow s" style={{ marginTop: 0 }}>
              {e.note}
            </div>
          </div>
        ))}
      </div>
      <div className="hint" style={{ marginTop: 16, lineHeight: 1.9 }}>
        语义层目前在 <code>studio/</code>（Python，已冻结），正逐块搬到 <code>crates/rpx-semantic</code>。
        迁移完成后这一页才接真数据；在那之前，**这里不放演示数字** —— 假的比空的更糟。
      </div>
      {fallback ? <div style={{ marginTop: 16 }}>{fallback}</div> : null}
    </div>
  </>
);
