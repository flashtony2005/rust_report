import React, { useState } from 'react';
import { useToast } from '../Toast';
import { parseCsv } from '../../services/reportEngine';
import { SqlPickerModal } from './SqlPickerModal';

interface Props {
  onClose: () => void;
  onSubmit: (title: string, description: string, datasetOverride?: any) => void;
}

export const NewRequirementModal: React.FC<Props> = ({ onClose, onSubmit }) => {
  const { toast } = useToast();
  const [title, setTitle] = useState('');
  const [desc, setDesc] = useState('');
  const [fileState, setFileState] = useState('将使用示例数据');
  const [customDataset, setCustomDataset] = useState<any>(null);
  const [showSqlPicker, setShowSqlPicker] = useState(false);

  const handleFileUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const f = e.target.files?.[0];
    if (!f) return;
    setFileState('解析中…');
    const reader = new FileReader();
    reader.onload = (event) => {
      try {
        const text = String(event.target?.result || '');
        const parsed = parseCsv(text);
        if (!parsed.columns.length) throw new Error('CSV 未检测到有效列');
        setCustomDataset({
          kind: 'file',
          file: f.name,
          rows: parsed.rows,
          columns: parsed.columns,
          column_meta: parsed.column_meta,
          preview: parsed.preview,
          encoding: 'utf-8',
        });
        setFileState(`${f.name} · ${parsed.rows} 行 · ${parsed.columns.length} 列`);
        toast(`成功载入 ${f.name}（${parsed.rows} 行）`);
      } catch (err: any) {
        setFileState('解析失败：' + err.message);
        toast(err.message, true);
      }
    };
    reader.readAsText(f);
  };

  const handleUseSample = () => {
    setCustomDataset(null);
    setFileState('将使用示例数据（90 行门店销售）');
  };

  const handleSubmit = () => {
    if (!title.trim()) {
      toast('先给需求起个标题', true);
      return;
    }
    onSubmit(title.trim(), desc.trim(), customDataset);
  };

  return (
    <>
      <div className="modal-mask">
        <div className="modal">
          <h2>新建需求</h2>
          <div className="sub muted" style={{ fontSize: '12px' }}>
            先明确需求与数据源；模板会在下一步自动给出候选。
          </div>
          <div className="stack" style={{ marginTop: '16px' }}>
            <div>
              <label className="field">标题</label>
              <input
                type="text"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder="例：华东区门店销售汇总"
                autoFocus
              />
            </div>
            <div>
              <label className="field">一句话需求（可选，用于检索模板）</label>
              <textarea
                value={desc}
                onChange={(e) => setDesc(e.target.value)}
                placeholder="例：按大区、门店汇总销售额，带合计行"
                rows={3}
              />
            </div>
            <div>
              <label className="field">数据源</label>
              <div className="row" style={{ flexWrap: 'wrap', gap: '8px' }}>
                <button type="button" className="btn small" onClick={handleUseSample}>
                  用示例数据（90 行门店销售）
                </button>
                <label className="btn small" style={{ cursor: 'pointer' }}>
                  上传 CSV
                  <input type="file" accept=".csv,.tsv" hidden onChange={handleFileUpload} />
                </label>
                <button type="button" className="btn small" onClick={() => setShowSqlPicker(true)}>
                  连接数据库
                </button>
                <span className="tiny" style={{ marginLeft: '4px', alignSelf: 'center' }}>
                  {fileState}
                </span>
              </div>
            </div>
          </div>
          <div className="row" style={{ justifyContent: 'flex-end', marginTop: '20px' }}>
            <button type="button" className="btn" onClick={onClose}>
              取消
            </button>
            <button type="button" className="btn primary" onClick={handleSubmit}>
              创建并下一步
            </button>
          </div>
        </div>
      </div>

      {showSqlPicker && (
        <SqlPickerModal
          onClose={() => setShowSqlPicker(false)}
          onSelect={(pick) => {
            setCustomDataset({
              kind: 'sql',
              dsn: pick.dsn,
              sql: pick.sql,
              rows: pick.rows,
              columns: pick.columns,
            });
            setFileState(`数据库 ${pick.dsn} · ${pick.rows} 行 · ${pick.columns.length} 列`);
            setShowSqlPicker(false);
          }}
        />
      )}
    </>
  );
};
