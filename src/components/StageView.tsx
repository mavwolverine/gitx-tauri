import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { message } from "@tauri-apps/plugin-dialog";
import { DiffView } from "./DiffView";
import "./StageView.css";

interface FileStatus {
  path: string;
  status: string;
  staged: boolean;
}

interface StageViewProps {
  repoPath: string;
}

export function StageView({ repoPath }: StageViewProps) {
  const [unstagedFiles, setUnstagedFiles] = useState<FileStatus[]>([]);
  const [stagedFiles, setStagedFiles] = useState<FileStatus[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [selectedStaged, setSelectedStaged] = useState<boolean>(false);
  const [diff, setDiff] = useState<string>("");
  const [commitMessage, setCommitMessage] = useState("");
  const [amend, setAmend] = useState(false);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    file: string;
    staged: boolean;
    status: string;
  } | null>(null);

  useEffect(() => {
    loadStatus();

    const unlisten = listen("repo-changed", () => {
      loadStatus();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [repoPath]);

  useEffect(() => {
    const handleClick = () => setContextMenu(null);
    window.addEventListener("click", handleClick);
    return () => window.removeEventListener("click", handleClick);
  }, []);

  const getStatusPriority = (status: string) => {
    const s = status[0].toUpperCase();
    if (s === "D") return 2;
    if (s === "M") return 1;
    return 0;
  };

  const loadStatus = async () => {
    try {
      const files = await invoke<FileStatus[]>("get_status", {
        path: repoPath,
      });
      const unstaged = files
        .filter((f) => !f.staged)
        .sort((a, b) => {
          const statusDiff =
            getStatusPriority(b.status) - getStatusPriority(a.status);
          return statusDiff !== 0 ? statusDiff : a.path.localeCompare(b.path);
        });
      const staged = files
        .filter((f) => f.staged)
        .sort((a, b) => a.path.localeCompare(b.path));
      setUnstagedFiles(unstaged);
      setStagedFiles(staged);

      // Clear selection if selected file no longer exists in the same state
      if (selectedFile) {
        const fileExists = selectedStaged
          ? staged.some((f) => f.path === selectedFile)
          : unstaged.some((f) => f.path === selectedFile);
        console.log("Check file exists:", {
          selectedFile,
          selectedStaged,
          fileExists,
          stagedCount: staged.length,
          unstagedCount: unstaged.length,
        });
        if (!fileExists) {
          setSelectedFile(null);
          setDiff("");
        }
      }

      // Clear selection if no files remain
      if (unstaged.length === 0 && staged.length === 0) {
        setSelectedFile(null);
        setDiff("");
      }
    } catch (error) {
      console.error("Failed to load status:", error);
    }
  };

  const loadDiff = async (filePath: string, staged: boolean) => {
    try {
      const diffText = await invoke<string>("get_diff", {
        path: repoPath,
        filePath,
        staged,
      });
      setDiff(diffText);
    } catch (error) {
      console.error("Failed to load diff:", error);
      setDiff("");
    }
  };

  const handleFileSelect = (filePath: string, staged: boolean) => {
    setSelectedFile(filePath);
    setSelectedStaged(staged);
    loadDiff(filePath, staged);
  };

  const handleContextMenu = (
    e: React.MouseEvent,
    file: string,
    staged: boolean,
    status: string
  ) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, file, staged, status });
  };

  const handleStageFile = async (file: string) => {
    try {
      await invoke("stage_file", { path: repoPath, filePath: file });
      setContextMenu(null);
      loadStatus();
    } catch (error) {
      await message(`Failed to stage file: ${error}`, {
        title: "Error",
        kind: "error",
      });
    }
  };

  const handleStageHunk = async (hunkHeader?: string, hunkLines?: string) => {
    if (!selectedFile || !hunkHeader || !hunkLines) return;

    try {
      if (selectedStaged) {
        // Unstage the hunk
        await invoke("unstage_hunk", {
          path: repoPath,
          filePath: selectedFile,
          fullDiff: diff,
          hunkHeader,
          hunkLines,
        });
      } else {
        // Stage the hunk
        await invoke("stage_hunk", {
          path: repoPath,
          filePath: selectedFile,
          fullDiff: diff,
          hunkHeader,
          hunkLines,
        });
      }
      loadStatus();
      loadDiff(selectedFile, selectedStaged);
    } catch (error) {
      await message(
        `Failed to ${selectedStaged ? "unstage" : "stage"} hunk: ${error}`,
        {
          title: "Error",
          kind: "error",
        }
      );
    }
  };

  const handleUnstageFile = async (file: string) => {
    try {
      await invoke("unstage_file", { path: repoPath, filePath: file });
      setContextMenu(null);
      loadStatus();
    } catch (error) {
      await message(`Failed to unstage file: ${error}`, {
        title: "Error",
        kind: "error",
      });
    }
  };

  const handleIgnoreFile = async (file: string) => {
    try {
      await invoke("ignore_file", { path: repoPath, filePath: file });
      setContextMenu(null);
      loadStatus();
    } catch (error) {
      await message(`Failed to ignore file: ${error}`, {
        title: "Error",
        kind: "error",
      });
    }
  };

  const handleDiscardChanges = async (file: string) => {
    try {
      await invoke("discard_file", { path: repoPath, filePath: file });
      setContextMenu(null);
      loadStatus();
    } catch (error) {
      await message(`Failed to discard changes: ${error}`, {
        title: "Error",
        kind: "error",
      });
    }
  };

  const handleDiscardHunk = async (hunkHeader?: string, hunkLines?: string) => {
    if (!selectedFile || !hunkHeader || !hunkLines) return;

    try {
      await invoke("discard_hunk", {
        path: repoPath,
        filePath: selectedFile,
        fullDiff: diff,
        hunkHeader,
        hunkLines,
      });
      loadStatus();
      loadDiff(selectedFile, selectedStaged);
    } catch (error) {
      await message(`Failed to discard hunk: ${error}`, {
        title: "Error",
        kind: "error",
      });
    }
  };

  const getStatusColor = (status: string) => {
    const s = status[0].toUpperCase();
    if (s === "M") return "#4ade80";
    if (s === "A") return "#60a5fa";
    if (s === "D") return "#f87171";
    return "#9ca3af";
  };

  return (
    <div className="stage-view">
      <div className="stage-main">
        {selectedFile ? (
          <>
            <div className="stage-diff-header">
              {selectedStaged ? "Staged" : "Unstaged"} changes for{" "}
              {selectedFile}
            </div>
            <div className="diff-container">
              <DiffView
                diff={diff}
                filename={selectedFile}
                actionLabel={selectedStaged ? "Unstage" : "Stage"}
                onDiscard={selectedStaged ? undefined : handleDiscardHunk}
                onStage={handleStageHunk}
              />
            </div>
          </>
        ) : (
          <div className="no-file-selected">No file selected</div>
        )}
      </div>
      <div className="stage-bottom">
        <div className="stage-panel">
          <div className="panel-header">Unstaged Changes</div>
          <div className="panel-content">
            {unstagedFiles.map((file) => (
              <div
                key={file.path}
                className={`file-item ${selectedFile === file.path && !selectedStaged ? "selected" : ""}`}
                onClick={() => handleFileSelect(file.path, false)}
                onDoubleClick={() => handleStageFile(file.path)}
                onContextMenu={(e) =>
                  handleContextMenu(e, file.path, false, file.status)
                }
              >
                <span className="file-status">
                  {file.status[0].toUpperCase()}
                </span>
                <span style={{ color: getStatusColor(file.status) }}>
                  {file.path}
                </span>
              </div>
            ))}
          </div>
        </div>
        <div className="stage-panel commit-panel">
          <div className="panel-header">Commit Message</div>
          <div className="panel-content">
            <textarea
              className="commit-message"
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
              placeholder="Enter commit message..."
            />
            <div className="commit-actions">
              <label className="amend-checkbox">
                <input
                  type="checkbox"
                  checked={amend}
                  onChange={(e) => setAmend(e.target.checked)}
                />
                Amend
              </label>
              <button
                className="commit-button"
                disabled={
                  stagedFiles.length === 0 || commitMessage.trim() === ""
                }
              >
                Commit
              </button>
            </div>
          </div>
        </div>
        <div className="stage-panel">
          <div className="panel-header">Staged Changes</div>
          <div className="panel-content">
            {stagedFiles.map((file) => (
              <div
                key={file.path}
                className={`file-item ${selectedFile === file.path && selectedStaged ? "selected" : ""}`}
                onClick={() => handleFileSelect(file.path, true)}
                onDoubleClick={() => handleUnstageFile(file.path)}
                onContextMenu={(e) =>
                  handleContextMenu(e, file.path, true, file.status)
                }
              >
                <span className="file-status">
                  {file.status[0].toUpperCase()}
                </span>
                <span style={{ color: getStatusColor(file.status) }}>
                  {file.path}
                </span>
              </div>
            ))}
          </div>
        </div>
      </div>
      {contextMenu && (
        <div
          className="context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          {contextMenu.staged ? (
            <div
              className="context-menu-item"
              onClick={() => handleUnstageFile(contextMenu.file)}
            >
              Unstage {contextMenu.file}
            </div>
          ) : (
            <>
              <div
                className="context-menu-item"
                onClick={() => handleStageFile(contextMenu.file)}
              >
                Stage {contextMenu.file}
              </div>
              {contextMenu.status === "modified" && (
                <div
                  className="context-menu-item"
                  onClick={() => handleDiscardChanges(contextMenu.file)}
                >
                  Discard changes to {contextMenu.file}
                </div>
              )}
              <div
                className="context-menu-item"
                onClick={() => handleIgnoreFile(contextMenu.file)}
              >
                Ignore {contextMenu.file}
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}
