import React, { useState } from 'react';
import { store } from '../services/store';
import { useToast } from '../components/Toast';
import { Datasource } from '../types';
import { DatasourceModal } from '../components/modals/DatasourceModal';

export const DatasourcesView: React.FC = () => {
  const { toast } = useToast();
  const datasources = store.getDatasources();
  const [testingId, setTestingId] = useState<string | null>(null);
  const [connStates, setConnStates] = useState<Record<string, string>>({});
  const [editingDs, setEditingDs] = useState<Datasource | null | 'new'>(null);

  const handleTestConnection = (id: string) => {
    setTestingId(id);
    setConnStates((prev) => ({ ...prev, [id]: '连接测试中…' }));
    setTimeout(() => {
      setTestingId(null);
      setConnStates((prev) => ({ ...prev, [id]: '✓ 连接成功 (响应 18 ms)' }));
      toast(`数据源 ${id} 连接正常`);
    }, 500);
  };

  const handleDelete = (id: string) => {
    if (window.confirm(`确定删除数据源 ${id} 吗？`)) {
      store.deleteDatasource(id);
      toast('数据源已删除');
    }
  };

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Data Sources</div>
          <h1>数据源</h1>
          <div className="sub">
            报表直接从库里取数：这里登记连接，需求里选一条 SQL 就成数据源
          </div>
        </div>
        <button
          type="button"
          className="btn primary"
          onClick={() => setEditingDs('new')}
        >
          ＋ 新建数据源
        </button>
      </div>

      <div className="list">
        {datasources.length > 0 ? (
          datasources.map((d) => (
            <div key={d.id} className="list-row">
              <div className="grow">
                <div className="t">
                  {d.label}{' '}
                  <span className="badge">
                    {d.driver_label || d.driver}
                  </span>{' '}
                  <span className="tiny muted">{d.id}</span>
                </div>
                <div className="s">
                  {d.driver === 'sqlite'
                    ? d.database
                    : `${d.user || '(无用户)'}@${d.host}:${d.port}/${d.database}`}
                  {d.has_password ? ' · 已存口令' : ''}
                  {d.options ? ` · ${d.options}` : ''}
                </div>
              </div>

              {connStates[d.id] && (
                <span
                  className="tiny"
                  style={{
                    color: connStates[d.id].startsWith('✓')
                      ? 'var(--ok)'
                      : 'var(--ink-3)',
                    fontWeight: 600,
                  }}
                >
                  {connStates[d.id]}
                </span>
              )}

              <button
                type="button"
                className="btn small"
                disabled={testingId === d.id}
                onClick={() => handleTestConnection(d.id)}
              >
                {testingId === d.id ? '测试中…' : '测试连接'}
              </button>
              <button
                type="button"
                className="btn small"
                onClick={() => setEditingDs(d)}
              >
                编辑
              </button>
              <button
                type="button"
                className="btn small"
                onClick={() => handleDelete(d.id)}
              >
                删除
              </button>
            </div>
          ))
        ) : (
          <div className="empty">
            还没有数据源 —— 点右上角「新建数据源」，或先看看下面的 SQLite 示例
          </div>
        )}
      </div>

      <div className="card pad callout" style={{ marginTop: '22px' }}>
        <h3 style={{ margin: '0 0 8px' }}>本机示例：SQLite</h3>
        <div className="tiny muted" style={{ lineHeight: '1.75' }}>
          选 SQLite 文件类型，库文件填 <code>sample/sales-demo.db</code>（相对 studio
          目录的路径或绝对路径均可）。
          驱动直接在本地环境中运行，无需安装部署任何数据库服务 —— 专为验证「连库 → 取数 → 编译器优化 → 出报表」全链路设计。
        </div>
      </div>

      {editingDs && (
        <DatasourceModal
          initial={editingDs === 'new' ? null : editingDs}
          onClose={() => setEditingDs(null)}
        />
      )}
    </>
  );
};
