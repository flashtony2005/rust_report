import React from 'react';
import { SemanticVersion } from '../../types';
import { generateDiff } from '../../services/reportEngine';

interface Props {
  vA: SemanticVersion;
  vB: SemanticVersion;
  onClose: () => void;
}

export const VersionDiffModal: React.FC<Props> = ({ vA, vB, onClose }) => {
  const diff = generateDiff(vA, vB);

  return (
    <div className="modal-mask">
      <div className="modal" style={{ maxWidth: '760px' }}>
        <h2>
          v{diff.a} → v{diff.b}
        </h2>
        <div className="sub muted" style={{ fontSize: '12px' }}>
          {diff.semantic_changed
            ? '口径变了：两个版本的历史结果不可直接比较'
            : '口径没变（只动了展示/版式）'}
        </div>

        {diff.changes.length > 0 ? (
          <table className="cols" style={{ marginTop: '14px' }}>
            <thead>
              <tr>
                <th style={{ width: '88px' }}>级别</th>
                <th style={{ width: '34%' }}>变化项</th>
                <th>之前</th>
                <th>之后</th>
              </tr>
            </thead>
            <tbody>
              {diff.changes.map((c, i) => (
                <tr key={i}>
                  <td>
                    <span
                      className={`badge ${
                        c.level === 'semantic' ? 'failed' : 'compiled'
                      }`}
                    >
                      {c.level}
                    </span>
                  </td>
                  <td>{c.what}</td>
                  <td className="tiny">{String(c.before)}</td>
                  <td className="tiny">{String(c.after)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <div className="hint" style={{ marginTop: '12px' }}>
            两个版本没有差别
          </div>
        )}

        <div className="row" style={{ justifyContent: 'flex-end', marginTop: '16px' }}>
          <button type="button" className="btn" onClick={onClose}>
            关闭
          </button>
        </div>
      </div>
    </div>
  );
};
