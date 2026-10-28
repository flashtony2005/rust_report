import React, { useState } from 'react';
import { store } from '../../services/store';
import { useToast } from '../Toast';
import { parseCsv } from '../../services/reportEngine';
import { RAW_SAMPLE_CSV } from '../../data/seedData';

interface Props {
  initialDsn?: string;
  initialSql?: string;
  onClose: () => void;
  onSelect: (pick: { dsn: string; sql: string; rows: number; columns: string[] }) => void;
}

export const SqlPickerModal: React.FC<Props> = ({
  initialDsn,
  initialSql,
  onClose,
  onSelect,
}) => {
  const { toast } = useToast();
  const datasources = store.getDatasources();
  const [dsn, setDsn] = useState(initialDsn || datasources[0]?.id || 'demo-sqlite');
  const [sql, setSql] = useState(initialSql || 'SELECT region, store, month, cat, amount FROM sales;');
  const [previewData, setPreviewData] = useState<{ columns: string[]; rows: any[][] } | null>(null);
  const [stateText, setStateText] = useState('');
  const [tablesList, setTablesList] = useState<string[]>([]);

  const handleListTables = () => {
    setStateText('读库表中…');
    setTimeout(() => {
      setTablesList(['sales', 'categories', 'stores', 'inventory']);
      setStateText('库表读取完毕');
    }, 200);
  };

  const handlePreview = () => {
    if (!sql.trim()) {
      toast('先写 SQL', true);
      return;
    }
    setStateText('连接查询中…');
    setTimeout(() => {
      const parsed = parseCsv(RAW_SAMPLE_CSV);
      setPreviewData({
        columns: parsed.columns,
        rows: parsed.preview,
      });
      setStateText(`${parsed.rows} 行 · ${parsed.columns.length} 列（预览前 ${parsed.preview.length} 行）`);
    }, 300);
  };

  const handleConfirm = () => {
    const parsed = parseCsv(RAW_SAMPLE_CSV);
    onSelect({
      dsn,
      sql,
      rows: parsed.rows,
      columns: parsed.columns,
    });
  };

  return (
    <div className="modal-mask">
      <div className="modal" style={{ maxWidth: '760px' }}>
        <h2>从数据库取数</h2>
        <div className="stack" style={{ marginTop: '12px' }}>
          <div className="row" style={{ gap: '10px' }}>
            <div style={{ flex: 1 }}>
              <label className="field">数据源</label>
              <select value={dsn} onChange={(e) => setDsn(e.target.value)}>
                {datasources.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.label}（{d.driver}）
                  </option>
                ))}
              </select>
            </div>
            <div style={{ flex: '0 0 auto', alignSelf: 'flex-end' }}>
              <button type="button" className="btn" onClick={handleListTables}>
                列出库表
              </button>
            </div>
          </div>

          {tablesList.length > 0 && (
            <div className="tiny muted" style={{ lineHeight: '1.8' }}>
              点表名填入 SQL：
              {tablesList.map((t, i) => (
                <span key={t}>
                  {i > 0 && '、'}
                  <a
                    href="#"
                    style={{ fontWeight: 600, color: 'var(--accent)' }}
                    onClick={(e) => {
                      e.preventDefault();
                      setSql(`SELECT * FROM ${t} LIMIT 100;`);
                    }}
                  >
                    {t}
                  </a>
                </span>
              ))}
            </div>
          )}

          <div>
            <label className="field">SQL（原样交给数据库执行）</label>
            <textarea
              rows={4}
              value={sql}
              onChange={(e) => setSql(e.target.value)}
              placeholder="SELECT region, category, amount FROM sales"
            />
          </div>

          <div className="row" style={{ alignItems: 'center', gap: '12px' }}>
            <button type="button" className="btn" onClick={handlePreview}>
              预览前 20 行
            </button>
            <span className="tiny">{stateText}</span>
          </div>

          {previewData && (
            <div style={{ maxHeight: '200px', overflowY: 'auto' }}>
              <table className="cols">
                <thead>
                  <tr>
                    {previewData.columns.map((c) => (
                      <th key={c}>{c}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {previewData.rows.map((row, i) => (
                    <tr key={i}>
                      {row.map((val, j) => (
                        <td key={j}>{String(val)}</td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>

        <div className="row" style={{ justifyContent: 'flex-end', marginTop: '16px' }}>
          <button type="button" className="btn" onClick={onClose}>
            取消
          </button>
          <button type="button" className="btn primary" onClick={handleConfirm}>
            用这条 SQL
          </button>
        </div>
      </div>
    </div>
  );
};
