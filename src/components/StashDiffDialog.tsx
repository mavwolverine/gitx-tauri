import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./StashDiffDialog.css";

interface DiffLine {
  old_lineno?: number;
  new_lineno?: number;
  origin: string;
  content: string;
}

interface CommitFile {
  path: string;
  old_path?: string;
  status: string;
  additions: number;
  deletions: number;
  lines: DiffLine[];
}

interface StashDiffDialogProps {
  repoPath: string;
  oid: string;
  message: string;
  onClose: () => void;
}

export function StashDiffDialog({
  repoPath,
  oid,
  message,
  onClose,
}: StashDiffDialogProps) {
  const [files, setFiles] = useState<CommitFile[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    invoke<CommitFile[]>("get_stash_diff", { path: repoPath, oid })
      .then((result) => {
        if (!cancelled) setFiles(result);
      })
      .catch((error) => console.error("Failed to load stash diff:", error))
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [repoPath, oid]);

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div
        className="dialog-box stash-diff-box"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="dialog-title">{message || "Stash"}</div>
        <div className="stash-diff-body">
          {loading ? (
            <div className="file-diff">
              <pre>Loading diff...</pre>
            </div>
          ) : files.length === 0 ? (
            <div className="file-diff">
              <pre>No changes</pre>
            </div>
          ) : (
            files.map((file, idx) => (
              <div key={idx} className="file-section">
                <div className="file-diff-header">
                  <span className="file-header-left">
                    <span className="file-path">{file.path}</span>
                  </span>
                  <span className="file-changes">
                    {file.additions > 0 && (
                      <span className="additions">+{file.additions}</span>
                    )}
                    {file.deletions > 0 && (
                      <span className="deletions">-{file.deletions}</span>
                    )}
                  </span>
                </div>
                <div className="file-diff">
                  {file.lines.map((line, i) => {
                    let className = "diff-line";
                    if (line.origin === "+") {
                      className += " diff-add";
                    } else if (line.origin === "-") {
                      className += " diff-remove";
                    } else if (line.origin === "@") {
                      className += " diff-hunk";
                    }
                    const content = line.content.replace(/\n$/, "");
                    return (
                      <div key={i} className={className}>
                        <span className="line-number">
                          {line.old_lineno || ""}
                        </span>
                        <span className="line-number">
                          {line.new_lineno || ""}
                        </span>
                        <span className="line-origin">
                          {line.origin === " " ? " " : line.origin}
                        </span>
                        <span className="line-content">{content || " "}</span>
                      </div>
                    );
                  })}
                </div>
              </div>
            ))
          )}
        </div>
        <div className="dialog-actions">
          <button className="dialog-btn" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
