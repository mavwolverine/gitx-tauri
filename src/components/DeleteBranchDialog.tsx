import { useState } from "react";
import "./DeleteBranchDialog.css";

export interface BranchDeleteInfo {
  has_local: boolean;
  remote_names: string[];
}

interface DeleteBranchDialogProps {
  branchName: string;
  info: BranchDeleteInfo;
  onConfirm: (options: {
    deleteLocal: boolean;
    remotesToDelete: string[];
  }) => void;
  onCancel: () => void;
}

export function DeleteBranchDialog({
  branchName,
  info,
  onConfirm,
  onCancel,
}: DeleteBranchDialogProps) {
  const hasRemotes = info.remote_names.length > 0;
  const both = info.has_local && hasRemotes;
  const [selectedRemotes, setSelectedRemotes] = useState<Set<string>>(
    new Set(info.remote_names)
  );

  const toggleRemote = (name: string) => {
    setSelectedRemotes((prev) => {
      const next = new Set(prev);
      if (next.has(name)) {
        next.delete(name);
      } else {
        next.add(name);
      }
      return next;
    });
  };

  const message = both
    ? `Delete local and remote branch "${branchName}"?`
    : hasRemotes
      ? `Delete remote branch "${branchName}"?`
      : `Delete local branch "${branchName}"?`;

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div className="dialog-box" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-title">Delete Branch</div>
        <div className="dialog-message">{message}</div>
        {both && (
          <div className="dialog-remote-list">
            {info.remote_names.map((name) => (
              <label key={name} className="dialog-checkbox">
                <input
                  type="checkbox"
                  checked={selectedRemotes.has(name)}
                  onChange={() => toggleRemote(name)}
                />
                Also delete from remote ({name})
              </label>
            ))}
          </div>
        )}
        <div className="dialog-actions">
          <button className="dialog-btn" onClick={onCancel}>
            Cancel
          </button>
          <button
            className="dialog-btn dialog-btn-danger"
            onClick={() =>
              onConfirm({
                deleteLocal: info.has_local,
                remotesToDelete: both
                  ? Array.from(selectedRemotes)
                  : info.remote_names,
              })
            }
          >
            Delete
          </button>
        </div>
      </div>
    </div>
  );
}
