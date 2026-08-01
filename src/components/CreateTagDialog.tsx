import { useState } from "react";
import "./CreateTagDialog.css";

interface CreateTagDialogProps {
  fromRef: string;
  onConfirm: (tagName: string, message: string) => void;
  onCancel: () => void;
}

export function CreateTagDialog({
  fromRef,
  onConfirm,
  onCancel,
}: CreateTagDialogProps) {
  const [name, setName] = useState("");
  const [message, setMessage] = useState("");
  const trimmed = name.trim();
  const displayFrom = /^[0-9a-f]{40}$/i.test(fromRef)
    ? fromRef.substring(0, 7)
    : fromRef;

  const submit = () => {
    if (trimmed) onConfirm(trimmed, message.trim());
  };

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div className="dialog-box" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-title">Create Tag</div>
        <div className="dialog-message">From &quot;{displayFrom}&quot;</div>
        <input
          className="dialog-input"
          type="text"
          autoFocus
          spellCheck={false}
          autoCorrect="off"
          autoCapitalize="off"
          autoComplete="off"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) submit();
          }}
          placeholder="Tag name"
        />
        <textarea
          className="dialog-textarea"
          spellCheck={false}
          autoCorrect="off"
          autoCapitalize="off"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          placeholder="Message (optional — leave blank for a lightweight tag)"
        />
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
