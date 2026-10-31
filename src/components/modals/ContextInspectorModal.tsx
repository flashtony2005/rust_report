import React, { useState } from 'react';
import { ContextSnapshot } from '../../types';

interface Props {
  context: ContextSnapshot;
  onClose: () => void;
}

export const ContextInspectorModal: React.FC<Props> = ({ context, onClose }) => {
  const [selectedTable, setSelectedTable] = useState(context.tables[0]?.name || '');

  const curTable = context.tables.find((t) => t.name === selectedTable) || context.tables[0];

  return (
    <div className="modal-backdrop" onClick={onClose} style={{ zIndex: 1100 }}>
      <div
        className="modal"
        style={{ maxWidth: '840px', width: '92%' }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="row" style={{ alignItems: 'flex-start' }}>
          <div className="grow">
            <div className="eyebrow" style={{ color: '#059669' }}>
              Physical Schema Grounding
            </div>
            <h2 style={{ fontSize: '18px', margin: '4px 0' }}>
              上下文事实检视器（Context Inspector）
            </h2>
            <div className="sub" style={{ fontSize: '13px' }}>
              客观物理世界探查事实 · 数据源 <code>{context.dsnLabel}</code> · 基准时间 <code>{context.asOfDate}</code>
            </div>
          </div>
          <button type="button" className="btn small" onClick={onClose}>
            关闭 ✕
          </button>
        </div>

        {/* Top summary cards */}
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
            gap: '10px',
            marginTop: '16px',
          }}
        >
          <div style={{ background: '#f8fafc', padding: '10px 12px', borderRadius: '8px', border: '1px solid #e2e8f0' }}>
            <div style={{ fontSize: '11px', color: '#64748b' }}>已探查物理表</div>
            <div style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
              {context.tables.length} 张表
            </div>
            <div style={{ fontSize: '12px', color: '#059669' }}>
              合计 {context.tables.reduce((acc, t) => acc + t.rows, 0)} 行事实记录
            </div>
          </div>

          <div style={{ background: '#f8fafc', padding: '10px 12px', borderRadius: '8px', border: '1px solid #e2e8f0' }}>
            <div style={{ fontSize: '11px', color: '#64748b' }}>有效业务规则 (Active Rules)</div>
            <div style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
              {context.activeRules.length} 条已生效
            </div>
            <div style={{ fontSize: '12px', color: '#64748b' }}>杜绝大模型无根据猜想</div>
          </div>

          <div style={{ background: '#f8fafc', padding: '10px 12px', borderRadius: '8px', border: '1px solid #e2e8f0' }}>
            <div style={{ fontSize: '11px', color: '#64748b' }}>时间锚点 (As of Date)</div>
            <div style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
              {context.asOfDate}
            </div>
            <div style={{ fontSize: '12px', color: '#059669' }}>物理最新有效结账日</div>
          </div>
        </div>

        {/* Table Selector Tabs */}
        <div style={{ marginTop: '16px' }}>
          <div className="tiny" style={{ marginBottom: '6px' }}>
            选择物理表检视字段画像与枚举基数：
          </div>
          <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
            {context.tables.map((t) => (
              <button
                key={t.name}
                type="button"
                className={`tog ${t.name === selectedTable ? 'on' : ''}`}
                style={{
                  fontWeight: t.name === selectedTable ? 700 : 500,
                  background: t.name === selectedTable ? '#0f172a' : '#f1f5f9',
                  color: t.name === selectedTable ? '#fff' : '#334155',
                }}
                onClick={() => setSelectedTable(t.name)}
              >
                <code>{t.name}</code> ({t.label} · {t.rows} 行)
              </button>
            ))}
          </div>
        </div>

        {/* Selected Table Columns Spec */}
        {curTable && (
          <div style={{ marginTop: '14px', border: '1px solid #e2e8f0', borderRadius: '8px', overflow: 'hidden' }}>
            <table className="cols" style={{ width: '100%', margin: 0, fontSize: '12.5px' }}>
              <thead>
                <tr style={{ background: '#f8fafc' }}>
                  <th style={{ width: '22%' }}>物理列名 (Column)</th>
                  <th style={{ width: '16%' }}>数据库类型</th>
                  <th style={{ width: '28%' }}>业务含义 / 备注</th>
                  <th style={{ width: '16%' }}>基数 (Cardinality)</th>
                  <th>探查样例值</th>
                </tr>
              </thead>
              <tbody>
                {curTable.columns.map((c) => (
                  <tr key={c.name}>
                    <td>
                      <code>{c.name}</code>
                      {c.isKey && (
                        <span
                          style={{
                            marginLeft: '6px',
                            background: '#e0e7ff',
                            color: '#3730a3',
                            padding: '1px 5px',
                            borderRadius: '4px',
                            fontSize: '10px',
                            fontWeight: 600,
                          }}
                        >
                          PK
                        </span>
                      )}
                    </td>
                    <td className="tiny">{c.type}</td>
                    <td style={{ color: '#334155' }}>{c.comment || '—'}</td>
                    <td className="tiny">{c.cardinality != null ? `${c.cardinality} 个去重值` : '—'}</td>
                    <td>
                      <code style={{ background: '#f1f5f9', padding: '2px 6px', borderRadius: '4px' }}>
                        {c.sample ?? '—'}
                      </code>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {/* Active Rules List */}
        <div style={{ marginTop: '16px', background: '#f8fafc', padding: '12px 14px', borderRadius: '8px', border: '1px solid #e2e8f0' }}>
          <div style={{ fontSize: '12px', fontWeight: 700, color: '#334155', marginBottom: '6px' }}>
            🛡️ 编译期生效的业务防翻车规则（来自模型约束）：
          </div>
          <ul style={{ margin: 0, paddingLeft: '18px', fontSize: '12px', color: '#475569', lineHeight: 1.6 }}>
            {context.activeRules.map((rule, idx) => (
              <li key={idx}>{rule}</li>
            ))}
          </ul>
        </div>

        <div style={{ marginTop: '16px', display: 'flex', justifyContent: 'flex-end' }}>
          <button type="button" className="btn primary small" onClick={onClose}>
            完成检视并返回
          </button>
        </div>
      </div>
    </div>
  );
};
