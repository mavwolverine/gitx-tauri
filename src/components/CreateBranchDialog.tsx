import { useState } from "react";
import "./CreateBranchDialog.css";

interface CreateBranchDialogProps {
  fromBranch: string;
  onConfirm: (branchName: string, checkout: boolean) => void;
  onCancel: () => void;
}

export function CreateBranchDialog({
  fromBranch,
  onConfirm,
  onCancel,
}: CreateBranchDialogProps) {
  const [name, setName] = useState("");
  const [checkout, setCheckout] = useState(true);
  const trimmed = name.trim();

  const submit = () => {
    if (trimmed) onConfirm(trimmed, checkout);
  };

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div className="dialog-box" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-title">Create Branch</div>
        <div className="dialog-message">From &quot;{fromBranch}&quot;</div>
        <input
          className="dialog-input"
          type="text"
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") submit();
          }}
          placeholder="Branch name"
        />
        <label className="dialog-checkbox">
          <input
            type="checkbox"
            checked={checkout}
            onChange={(e) => setCheckout(e.target.checked)}
          />
          Checkout new branch
        </label>
        <div className="dialog-actions">
          <button className="dialog-btn" onClick={onCancel}>
            Cancel
          </button>
          <button
            className="dialog-btn dialog-btn-primary"
            disabled={!trimmed}
            onClick={submit}
          >
            Create
          </button>
        </div>
      </div>
    </div>
  );
}
