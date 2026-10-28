import React, { useState } from 'react';
import { Datasource, DriverType } from '../../types';
import { store } from '../../services/store';
import { useToast } from '../Toast';

interface Props {
  initial?: Datasource | null;
  onClose: () => void;
}

export const DatasourceModal: React.FC<Props> = ({ initial, onClose }) => {
  const { toast } = useToast();
  const [id, setId] = useState(initial?.id || '');
  const [label, setLabel] = useState(initial?.label || '');
  const [driver, setDriver] = useState<DriverType>(initial?.driver || 'postgres');
  const [host, setHost] = useState(initial?.host || '127.0.0.1');
  const [port, setPort] = useState(initial?.port ? String(initial.port) : '5432');
  const [database, setDatabase] = useState(initial?.database || '');
  const [user, setUser] = useState(initial?.user || '');
  const [password, setPassword] = useState('');
  const [options, setOptions] = useState(initial?.options || '');

  const isSqlite = driver === 'sqlite';

  const handleSave = () => {
    if (!id.trim()) {
      toast('标识不能为空', true);
      return;
    }
    if (!label.trim()) {
      toast('显示名不能为空', true);
      return;
    }
    if (!database.trim()) {
      toast(isSqlite ? '库文件路径不能为空' : '库名不能为空', true);
      return;
    }

    store.saveDatasource({
      id: id.trim(),
      label: label.trim(),
      driver,
      host: isSqlite ? undefined : host.trim(),
      port: isSqlite ? undefined : port.trim(),
      database: database.trim(),
      user: isSqlite ? undefined : user.trim(),
      password: password || undefined,
      options: options.trim(),
    });
    toast('数据源已保存');
    onClose();
  };

  return (
    <div className="modal-mask">
      <div className="modal">
        <h2>{initial ? '编辑数据源' : '新建数据源'}</h2>
        <div className="stack" style={{ marginTop: '14px' }}>
          <div className="row">
            <div style={{ flex: 1 }}>
              <label className="field">标识（英文，报表里引用它）</label>
              <input
                type="text"
                value={id}
                onChange={(e) => setId(e.target.value)}
                disabled={!!initial}
                placeholder="例：sales-pg"
              />
            </div>
            <div style={{ flex: 1 }}>
              <label className="field">显示名</label>
              <input
                type="text"
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder="例：销售库（只读）"
              />
            </div>
          </div>
          <div>
            <label className="field">类型</label>
            <select
              value={driver}
              onChange={(e) => {
                const d = e.target.value as DriverType;
                setDriver(d);
                if (d === 'sqlite') {
                  if (!database) setDatabase('sample/sales-demo.db');
                } else if (d === 'mysql') {
                  setPort('3306');
                } else {
                  setPort('5432');
                }
              }}
            >
              <option value="postgres">PostgreSQL / Kingbase</option>
              <option value="mysql">MySQL</option>
              <option value="sqlite">SQLite 文件</option>
            </select>
          </div>
          {!isSqlite && (
            <div className="row">
              <div style={{ flex: 2 }}>
                <label className="field">主机</label>
                <input
                  type="text"
                  value={host}
                  onChange={(e) => setHost(e.target.value)}
                />
              </div>
              <div style={{ flex: 1 }}>
                <label className="field">端口</label>
                <input
                  type="text"
                  value={port}
                  onChange={(e) => setPort(e.target.value)}
                  placeholder="5432"
                />
              </div>
            </div>
          )}
          <div>
            <label className="field">
              {isSqlite ? '库文件路径（.db 的文件路径）' : '库名'}
            </label>
            <input
              type="text"
              value={database}
              onChange={(e) => setDatabase(e.target.value)}
              placeholder={isSqlite ? 'sample/sales-demo.db' : 'sales_db'}
            />
          </div>
          {!isSqlite && (
            <div className="row">
              <div style={{ flex: 1 }}>
                <label className="field">用户</label>
                <input
                  type="text"
                  value={user}
                  onChange={(e) => setUser(e.target.value)}
                />
              </div>
              <div style={{ flex: 1 }}>
                <label className="field">口令{initial ? '（留空 = 不改）' : ''}</label>
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder={initial?.has_password ? '已存，留空不变' : ''}
                />
              </div>
            </div>
          )}
          <div>
            <label className="field">附加参数（可选，拼在 URL 后面）</label>
            <input
              type="text"
              value={options}
              onChange={(e) => setOptions(e.target.value)}
              placeholder="例：sslmode=disable"
            />
          </div>
        </div>
        <div className="row" style={{ justifyContent: 'flex-end', marginTop: '18px' }}>
          <button type="button" className="btn" onClick={onClose}>
            取消
          </button>
          <button type="button" className="btn primary" onClick={handleSave}>
            保存
          </button>
        </div>
      </div>
    </div>
  );
};
