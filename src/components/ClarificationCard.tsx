import React, { useState } from 'react';
import { ClarificationQuestion, TriageResult } from '../types';

interface Props {
  triage: TriageResult;
  onConfirm: (optionsOverride: Record<string, string>) => void;
  onCancel: () => void;
}

export const ClarificationCard: React.FC<Props> = ({ triage, onConfirm, onCancel }) => {
  // Initialize selections with default option for each question
  const [selections, setSelections] = useState<Record<string, string>>(() => {
    const init: Record<string, string> = {};
    (triage.clarifications || []).forEach((q) => {
      const def = q.options.find((o) => o.isDefault) || q.options[0];
      if (def) init[q.id] = def.id;
    });
    return init;
  });

  const handleSelectOption = (questionId: string, optionId: string) => {
    setSelections((prev) => ({
      ...prev,
      [questionId]: optionId,
    }));
  };

  const handleSubmit = () => {
    onConfirm(selections);
  };

  return (
    <div
      className="card pad"
      style={{
        marginTop: '12px',
        border: '1.5px solid #d97706',
        background: '#fffbeb',
        borderRadius: '12px',
        boxShadow: '0 4px 12px rgba(217, 119, 6, 0.08)',
      }}
    >
      {/* Header */}
      <div className="row" style={{ alignItems: 'flex-start' }}>
        <div className="grow">
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '4px' }}>
            <span
              style={{
                background: '#fef3c7',
                color: '#92400e',
                border: '1px solid #f59e0b',
                padding: '2px 8px',
                borderRadius: '6px',
                fontSize: '12px',
                fontWeight: 700,
              }}
            >
              🟡 可以做到 · 要先补
            </span>
            <span style={{ fontSize: '13px', fontWeight: 600, color: '#78350f' }}>
              检测到口径二义性 / 统计区间未闭环
            </span>
          </div>
          <div style={{ fontSize: '12.5px', color: '#92400e' }}>
            报表引擎秉承「错误成本不对称」原则：坚决不胡猜，先问清口径再生成真报表！
          </div>
        </div>
        <button type="button" className="btn small" onClick={onCancel} style={{ color: '#78350f' }}>
          放弃 ✕
        </button>
      </div>

      {/* Clarification Questions List */}
      <div style={{ marginTop: '14px', display: 'flex', flexDirection: 'column', gap: '14px' }}>
        {(triage.clarifications || []).map((q, idx) => (
          <div
            key={q.id}
            style={{
              background: '#ffffff',
              border: '1px solid #fde68a',
              borderRadius: '8px',
              padding: '12px 14px',
            }}
          >
            <div style={{ fontSize: '13px', fontWeight: 700, color: '#1e293b', marginBottom: '2px' }}>
              {idx + 1}. {q.title}
            </div>
            {q.hint && (
              <div style={{ fontSize: '11.5px', color: '#64748b', marginBottom: '8px' }}>
                {q.hint}
              </div>
            )}

            <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
              {q.options.map((opt) => {
                const isSelected = selections[q.id] === opt.id;
                return (
                  <label
                    key={opt.id}
                    style={{
                      display: 'flex',
                      alignItems: 'flex-start',
                      gap: '8px',
                      padding: '8px 10px',
                      borderRadius: '6px',
                      border: isSelected ? '1.5px solid #2563eb' : '1px solid #e2e8f0',
                      background: isSelected ? '#eff6ff' : '#f8fafc',
                      cursor: 'pointer',
                      transition: 'all 0.15s ease',
                    }}
                    onClick={() => handleSelectOption(q.id, opt.id)}
                  >
                    <input
                      type="radio"
                      name={`q_${q.id}`}
                      checked={isSelected}
                      onChange={() => handleSelectOption(q.id, opt.id)}
                      style={{ marginTop: '3px' }}
                    />
                    <div style={{ flex: 1 }}>
                      <div
                        style={{
                          fontSize: '12.5px',
                          fontWeight: isSelected ? 700 : 500,
                          color: isSelected ? '#1d4ed8' : '#334155',
                        }}
                      >
                        {opt.label}
                        {opt.isDefault && (
                          <span
                            style={{
                              marginLeft: '6px',
                              background: '#dbeafe',
                              color: '#1e40af',
                              fontSize: '10px',
                              padding: '1px 5px',
                              borderRadius: '4px',
                            }}
                          >
                            推荐默认
                          </span>
                        )}
                      </div>
                      {opt.desc && (
                        <div style={{ fontSize: '11px', color: '#64748b', marginTop: '2px' }}>
                          {opt.desc}
                        </div>
                      )}
                    </div>
                  </label>
                );
              })}
            </div>
          </div>
        ))}
      </div>

      {/* Action buttons */}
      <div style={{ marginTop: '16px', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div className="tiny" style={{ color: '#b45309' }}>
          确定后将自动解除阻塞，直接驱动 spec → 编译器 → 真实执行生成报表
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          <button type="button" className="btn small" onClick={onCancel}>
            取消
          </button>
          <button
            type="button"
            className="btn small primary"
            style={{ background: '#d97706', borderColor: '#b45309', fontWeight: 600 }}
            onClick={handleSubmit}
          >
            ✓ 确定所选口径并生成报表 →
          </button>
        </div>
      </div>
    </div>
  );
};
