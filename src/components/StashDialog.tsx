import { useState } from "react";

interface StashDialogProps {
  onConfirm: (message: string | null, includeUntracked: boolean) => void;
  onCancel: () => void;
}

export function StashDialog({ onConfirm, onCancel }: StashDialogProps) {
  const [message, setMessage] = useState("");
  const [includeUntracked, setIncludeUntracked] = useState(false);

  const submit = () => {
    const trimmed = message.trim();
    onConfirm(trimmed || null, includeUntracked);
  };

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div className="dialog-box" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-title">Stash Changes</div>
        <textarea
          className="dialog-textarea"
          autoFocus
          spellCheck={false}
          autoCorrect="off"
          autoCapitalize="off"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) submit();
          }}
          placeholder="Message (optional)"
        />
        <label className="dialog-checkbox">
          <input
            type="checkbox"
            checked={includeUntracked}
            onChange={(e) => setIncludeUntracked(e.target.checked)}
          />
          Include untracked files
        </label>
        <div className="dialog-actions">
          <button className="dialog-btn" onClick={onCancel}>
            Cancel
          </button>
          <button className="dialog-btn dialog-btn-primary" onClick={submit}>
            Stash
          </button>
        </div>
      </div>
    </div>
  );
}
