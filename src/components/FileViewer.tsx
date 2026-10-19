import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TreeNode } from "./CommitTreeView";
import "./FileViewer.css";

interface FileViewerProps {
  repoPath: string;
  commitId: string;
  file: TreeNode | null;
  // Owned by the parent so the tab survives switching files (#3).
  activeTab: FileViewerTab;
  onTabChange: (tab: FileViewerTab) => void;
  onJumpToCommit: (commitId: string) => void;
}

interface FileContent {
  is_binary: boolean;
  size: number;
  content: string | null;
}

interface BlameLine {
  line_no: number;
  content: string;
  commit_id: string;
  author: string;
  timestamp: string;
}

interface GitAuthor {
  name: string;
  email: string;
  timestamp: string;
}

interface FileHistoryEntry {
  id: string;
  message: string;
  author: GitAuthor;
}

export type FileViewerTab = "source" | "blame" | "history";

interface BlameGroup {
  commit_id: string;
  author: string;
  timestamp: string;
  lines: BlameLine[];
}

function groupBlameLines(lines: BlameLine[]): BlameGroup[] {
  const groups: BlameGroup[] = [];
  for (const line of lines) {
    const last = groups[groups.length - 1];
    if (last && last.commit_id === line.commit_id) {
      last.lines.push(line);
    } else {
      groups.push({
        commit_id: line.commit_id,
        author: line.author,
        timestamp: line.timestamp,
        lines: [line],
      });
    }
  }
  return groups;
}

const formatDate = (timestamp: string) => {
  const date = new Date(parseInt(timestamp) * 1000);
  return `${date.toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  })} at ${date.toLocaleTimeString("en-US", {
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  })}`;
};

export function FileViewer({
  repoPath,
  commitId,
  file,
  activeTab,
  onTabChange,
  onJumpToCommit,
}: FileViewerProps) {
  // Each view keeps showing the previous commit's data until the new data
  // arrives, so the scroll container isn't emptied and its scroll position
  // survives switching commits.
  const [content, setContent] = useState<FileContent | null>(null);
  const [contentFailed, setContentFailed] = useState(false);
  const [blame, setBlame] = useState<{
    commitId: string;
    lines: BlameLine[];
  } | null>(null);
  const [history, setHistory] = useState<{
    commitId: string;
    entries: FileHistoryEntry[];
  } | null>(null);

  const filePath = file?.path;
  const isBinary = content?.is_binary ?? false;
  const blameCommitId = blame?.commitId;
  const historyCommitId = history?.commitId;

  useEffect(() => {
    if (!filePath) return;
    // Ignore responses for a commit the user has already moved past.
    let cancelled = false;
    invoke<FileContent>("get_file_content", {
      path: repoPath,
      commitId,
      filePath,
    })
      .then((result) => {
        if (cancelled) return;
        setContent(result);
        setContentFailed(false);
      })
      .catch((error) => {
        console.error("Failed to load file content:", error);
        if (!cancelled) setContentFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [filePath, repoPath, commitId]);

  useEffect(() => {
    if (!filePath || activeTab !== "blame" || isBinary) return;
    if (blameCommitId === commitId) return;
    let cancelled = false;
    invoke<BlameLine[]>("get_file_blame", {
      path: repoPath,
      commitId,
      filePath,
    })
      .then((lines) => {
        if (!cancelled) setBlame({ commitId, lines });
      })
      .catch((error) => {
        console.error("Failed to load blame:", error);
        if (!cancelled) setBlame({ commitId, lines: [] });
      });
    return () => {
      cancelled = true;
    };
  }, [activeTab, filePath, repoPath, commitId, isBinary, blameCommitId]);

  useEffect(() => {
    if (!filePath || activeTab !== "history") return;
    if (historyCommitId === commitId) return;
    let cancelled = false;
    invoke<FileHistoryEntry[]>("get_file_history", {
      path: repoPath,
      commitId,
      filePath,
    })
      .then((entries) => {
        if (!cancelled) setHistory({ commitId, entries });
      })
      .catch((error) => {
        console.error("Failed to load file history:", error);
        if (!cancelled) setHistory({ commitId, entries: [] });
      });
    return () => {
      cancelled = true;
    };
  }, [activeTab, filePath, repoPath, commitId, historyCommitId]);

  if (!file) {
    return <div className="no-selection">Select a file to view</div>;
  }

  return (
    <div className="file-viewer">
      <div className="file-viewer-header">
        <span className="file-viewer-path">{file.path}</span>
        <div className="branch-filter-bar">
          <div className="filter-btn-group">
            <button
              className={`filter-btn ${activeTab === "source" ? "active" : ""}`}
              onClick={() => onTabChange("source")}
            >
              Source
            </button>
            <button
              className={`filter-btn ${activeTab === "blame" ? "active" : ""}`}
              onClick={() => onTabChange("blame")}
            >
              Blame
            </button>
            <button
              className={`filter-btn ${activeTab === "history" ? "active" : ""}`}
              onClick={() => onTabChange("history")}
            >
              History
            </button>
          </div>
        </div>
      </div>
      <div className="file-viewer-body">
        {activeTab === "source" &&
          (contentFailed ? (
            <div className="file-viewer-status">Failed to load file</div>
          ) : content === null ? (
            <div className="file-viewer-status">Loading...</div>
          ) : content?.is_binary ? (
            <div className="file-viewer-status">
              Binary file ({content.size} bytes)
            </div>
          ) : (
            <div className="source-view">
              {(content?.content ?? "").split("\n").map((line, i) => (
                <div key={i} className="source-line">
                  <span className="line-number">{i + 1}</span>
                  <span className="line-content">{line || " "}</span>
                </div>
              ))}
            </div>
          ))}
        {activeTab === "blame" &&
          (content?.is_binary ? (
            <div className="file-viewer-status">
              Binary file ({content.size} bytes)
            </div>
          ) : blame === null ? (
            <div className="file-viewer-status">Loading...</div>
          ) : (
            <div className="blame-view">
              {groupBlameLines(blame.lines).map((group) => (
                <div key={group.lines[0].line_no} className="blame-group">
                  <div
                    className="blame-meta"
                    title={`${group.author} — ${formatDate(group.timestamp)}`}
                  >
                    <span
                      className="blame-sha"
                      onClick={() => onJumpToCommit(group.commit_id)}
                      title={`Jump to commit ${group.commit_id}`}
                    >
                      {group.commit_id.substring(0, 7)}
                    </span>
                    <span className="blame-author">{group.author}</span>
                  </div>
                  <div className="blame-group-lines">
                    {group.lines.map((line) => (
                      <div key={line.line_no} className="blame-line">
                        <span className="line-number">{line.line_no}</span>
                        <span className="line-content">
                          {line.content || " "}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          ))}
        {activeTab === "history" &&
          (history === null ? (
            <div className="file-viewer-status">Loading...</div>
          ) : history.entries.length === 0 ? (
            <div className="file-viewer-status">No history found</div>
          ) : (
            <div className="file-history-list">
              {history.entries.map((entry) => (
                <div
                  key={entry.id}
                  className="file-history-item"
                  onClick={() => onJumpToCommit(entry.id)}
                >
                  <div className="file-history-message">
                    {entry.message.split("\n")[0]}
                  </div>
                  <div className="file-history-meta">
                    <span className="sha">{entry.id.substring(0, 7)}</span>
                    <span className="author">{entry.author.name}</span>
                    <span className="date">
                      {formatDate(entry.author.timestamp)}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          ))}
      </div>
    </div>
  );
}
