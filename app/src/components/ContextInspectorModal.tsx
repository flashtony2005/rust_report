import React, { useState } from 'react';

interface Props {
  onClose: () => void;
  dsnId?: string;
}

export const ContextInspectorModal: React.FC<Props> = ({ onClose, dsnId = 'demo_sqlite' }) => {
  const tables = [
    {
      name: 'customers',
      label: '客户主体档案主表',
      rows: 250,
      columns: [
        { name: 'id', type: 'BIGINT', comment: '客户唯一主键ID', isKey: true, sample: '1001', cardinality: 250 },
        { name: 'name', type: 'VARCHAR(128)', comment: '客户/企业全称', isKey: false, sample: '上海智成科技', cardinality: 250 },
        { name: 'tier', type: 'VARCHAR(16)', comment: '客户会员等级（钻石/白金/黄金）', isKey: false, sample: '钻石', cardinality: 3 },
        { name: 'region', type: 'VARCHAR(32)', comment: '归属大区（华东/华北/华南）', isKey: false, sample: '华东', cardinality: 3 },
        { name: 'credit_limit', type: 'DECIMAL(14,2)', comment: '授信总额度（元）', isKey: false, sample: '500000.00', cardinality: 80 },
      ],
    },
    {
      name: 'orders',
      label: '客户消费交易订单表',
      rows: 1200,
      columns: [
        { name: 'id', type: 'BIGINT', comment: '订单主键ID', isKey: true, sample: '8801', cardinality: 1200 },
        { name: 'customer_id', type: 'BIGINT', comment: '关联客户外键', isKey: false, sample: '1001', cardinality: 250 },
        { name: 'order_date', type: 'TIMESTAMP', comment: '下单时间戳', isKey: false, sample: '2026-09-12 14:20:00', cardinality: 980 },
        { name: 'amount', type: 'DECIMAL(12,2)', comment: '实收交易额（元）', isKey: false, sample: '12800.00', cardinality: 650 },
        { name: 'cost', type: 'DECIMAL(12,2)', comment: '进货成本（元）', isKey: false, sample: '8200.00', cardinality: 610 },
        { name: 'status', type: 'VARCHAR(32)', comment: '订单状态（completed/canceled）', isKey: false, sample: 'completed', cardinality: 2 },
      ],
    },
    {
      name: 'loans',
      label: '金融信贷授信履约表',
      rows: 350,
      columns: [
        { name: 'id', type: 'BIGINT', comment: '贷款合同编号', isKey: true, sample: 'L20260901', cardinality: 350 },
        { name: 'customer_id', type: 'BIGINT', comment: '关联客户外键', isKey: false, sample: '1001', cardinality: 210 },
        { name: 'principal', type: 'DECIMAL(14,2)', comment: '放款本金金额（元）', isKey: false, sample: '150000.00', cardinality: 180 },
        { name: 'is_overdue', type: 'INT', comment: '是否逾期标志位（0正常 / 1逾期）', isKey: false, sample: '0', cardinality: 2 },
        { name: 'overdue_days', type: 'INT', comment: '逾期拖欠天数', isKey: false, sample: '15', cardinality: 45 },
      ],
    },
    {
      name: 'sales',
      label: '区域品类零售明细事实表',
      rows: 90,
      columns: [
        { name: 'region', type: 'VARCHAR(32)', comment: '大区', isKey: false, sample: '华东', cardinality: 3 },
        { name: 'store', type: 'VARCHAR(64)', comment: '门店名称', isKey: false, sample: '上海一店', cardinality: 6 },
        { name: 'month', type: 'DATE', comment: '销售月份', isKey: false, sample: '2026-07', cardinality: 3 },
        { name: 'cat', type: 'VARCHAR(32)', comment: '产品品类', isKey: false, sample: '办公用品', cardinality: 3 },
        { name: 'amount', type: 'DECIMAL(12,2)', comment: '销售额', isKey: false, sample: '28500', cardinality: 88 },
      ],
    },
  ];

  const activeRules = [
    '所有金额统计统一使用实收净额（自动过滤作废与退货订单）',
    '跨客户多事实表关联必须以客户中心表为准，严禁无聚合直接 JOIN（杜绝笛卡尔积）',
    '日期跨度缺省时默认锚定最近完结自然月 (2026-09)',
  ];

  const [selectedTable, setSelectedTable] = useState(tables[0].name);
  const curTable = tables.find((t) => t.name === selectedTable) || tables[0];

  return (
    <div
      className="modal-backdrop"
      onClick={onClose}
      style={{
        position: 'fixed',
        inset: 0,
        backgroundColor: 'rgba(15, 23, 42, 0.65)',
        backdropFilter: 'blur(4px)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 9999,
        padding: '20px',
      }}
    >
      <div
        className="modal"
        style={{
          background: '#ffffff',
          borderRadius: '14px',
          maxWidth: '880px',
          width: '100%',
          maxHeight: '90vh',
          overflowY: 'auto',
          padding: '24px',
          boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.35)',
        }}
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
              客观物理世界探查事实 · 数据源 <code>{dsnId}</code> · 基准时间 <code>2026-10-30</code>
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
              {tables.length} 张物理表
            </div>
            <div style={{ fontSize: '12px', color: '#059669' }}>
              合计 {tables.reduce((acc, t) => acc + t.rows, 0)} 行事实记录
            </div>
          </div>

          <div style={{ background: '#f8fafc', padding: '10px 12px', borderRadius: '8px', border: '1px solid #e2e8f0' }}>
            <div style={{ fontSize: '11px', color: '#64748b' }}>有效业务规则 (Active Rules)</div>
            <div style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
              {activeRules.length} 条已生效
            </div>
            <div style={{ fontSize: '12px', color: '#64748b' }}>杜绝大模型无根据猜想</div>
          </div>

          <div style={{ background: '#f8fafc', padding: '10px 12px', borderRadius: '8px', border: '1px solid #e2e8f0' }}>
            <div style={{ fontSize: '11px', color: '#64748b' }}>时间锚点 (As of Date)</div>
            <div style={{ fontSize: '18px', fontWeight: 700, color: '#0f172a' }}>
              2026-10-30
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
            {tables.map((t) => (
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
            {activeRules.map((rule, idx) => (
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
