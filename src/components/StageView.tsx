import { useCallback, useEffect, useRef, useState } from "react";
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

type FileList = "unstaged" | "staged";

interface StageViewProps {
  repoPath: string;
  onStash?: () => void;
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    target.isContentEditable
  );
}

function rangeSelect(
  files: FileStatus[],
  anchor: string | null,
  target: string
): Set<string> {
  if (!anchor) return new Set([target]);
  const paths = files.map((f) => f.path);
  const a = paths.indexOf(anchor);
  const b = paths.indexOf(target);
  if (a === -1 || b === -1) return new Set([target]);
  const [start, end] = a < b ? [a, b] : [b, a];
  return new Set(paths.slice(start, end + 1));
}

export function StageView({ repoPath, onStash }: StageViewProps) {
  const [unstagedFiles, setUnstagedFiles] = useState<FileStatus[]>([]);
  const [stagedFiles, setStagedFiles] = useState<FileStatus[]>([]);
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(new Set());
  const [focusedList, setFocusedList] = useState<FileList>("unstaged");
  const [anchorPath, setAnchorPath] = useState<string | null>(null);
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

  const selectedFileRef = useRef(selectedFile);
  const selectedStagedRef = useRef(selectedStaged);
  const selectedPathsRef = useRef(selectedPaths);
  const focusedListRef = useRef(focusedList);
  const unstagedFilesRef = useRef(unstagedFiles);
  const stagedFilesRef = useRef(stagedFiles);
  selectedFileRef.current = selectedFile;
  selectedStagedRef.current = selectedStaged;
  selectedPathsRef.current = selectedPaths;
  focusedListRef.current = focusedList;
  unstagedFilesRef.current = unstagedFiles;
  stagedFilesRef.current = stagedFiles;

  const getStatusPriority = (status: string) => {
    const s = status[0].toUpperCase();
    if (s === "D") return 2;
    if (s === "M") return 1;
    return 0;
  };

  const loadDiff = useCallback(
    async (filePath: string, staged: boolean) => {
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
    },
    [repoPath]
  );

  const activateFile = useCallback(
    (filePath: string, staged: boolean) => {
      setSelectedFile(filePath);
      setSelectedStaged(staged);
      loadDiff(filePath, staged);
    },
    [loadDiff]
  );

  const loadStatus = useCallback(async () => {
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

      const validUnstaged = new Set(unstaged.map((f) => f.path));
      const validStaged = new Set(staged.map((f) => f.path));
      const list = focusedListRef.current;
      const valid = list === "staged" ? validStaged : validUnstaged;

      setSelectedPaths((prev) => {
        const next = new Set([...prev].filter((p) => valid.has(p)));
        return next;
      });

      const currentSelectedFile = selectedFileRef.current;
      if (currentSelectedFile) {
        const fileExists = selectedStagedRef.current
          ? validStaged.has(currentSelectedFile)
          : validUnstaged.has(currentSelectedFile);
        if (!fileExists) {
          setSelectedFile(null);
          setDiff("");
        }
      }

      if (unstaged.length === 0 && staged.length === 0) {
        setSelectedFile(null);
        setSelectedPaths(new Set());
        setDiff("");
      }
    } catch (error) {
      console.error("Failed to load status:", error);
    }
  }, [repoPath]);

  useEffect(() => {
    loadStatus();

    const unlisten = listen("repo-changed", () => {
      loadStatus();
    });

    return () => {
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, [loadStatus]);

  useEffect(() => {
    const handleClick = () => setContextMenu(null);
    window.addEventListener("click", handleClick);
    return () => window.removeEventListener("click", handleClick);
  }, []);

  const handleFileClick = (
    e: React.MouseEvent,
    filePath: string,
    list: FileList
  ) => {
    const files = list === "unstaged" ? unstagedFiles : stagedFiles;
    const staged = list === "staged";
    setFocusedList(list);
    setContextMenu(null);

    if (e.shiftKey) {
      const next = rangeSelect(files, anchorPath, filePath);
      setSelectedPaths(next);
      activateFile(filePath, staged);
      return;
    }

    if (e.metaKey || e.ctrlKey) {
      // Keep selection within the focused list only
      const listPaths = new Set(files.map((f) => f.path));
      setSelectedPaths((prev) => {
        const next = new Set([...prev].filter((p) => listPaths.has(p)));
        if (next.has(filePath)) next.delete(filePath);
        else next.add(filePath);
        return next;
      });
      setAnchorPath(filePath);
      activateFile(filePath, staged);
      return;
    }

    setSelectedPaths(new Set([filePath]));
    setAnchorPath(filePath);
    activateFile(filePath, staged);
  };

  const handleContextMenu = (
    e: React.MouseEvent,
    file: string,
    staged: boolean,
    status: string
  ) => {
    e.preventDefault();
    const list: FileList = staged ? "staged" : "unstaged";
    setFocusedList(list);
    setSelectedPaths((prev) => {
      if (prev.has(file)) return prev;
      return new Set([file]);
    });
    setAnchorPath(file);
    activateFile(file, staged);
    setContextMenu({ x: e.clientX, y: e.clientY, file, staged, status });
  };

  const stageFiles = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;
      setContextMenu(null);
      setSelectedPaths(new Set());
      const errors: string[] = [];
      for (const filePath of paths) {
        try {
          await invoke("stage_file", { path: repoPath, filePath });
        } catch (error) {
          errors.push(`${filePath}: ${error}`);
        }
      }
      await loadStatus();
      if (errors.length > 0) {
        await message(`Failed to stage:\n${errors.join("\n")}`, {
          title: "Error",
          kind: "error",
        });
      }
    },
    [repoPath, loadStatus]
  );

  const unstageFiles = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;
      setContextMenu(null);
      setSelectedPaths(new Set());
      const errors: string[] = [];
      for (const filePath of paths) {
        try {
          await invoke("unstage_file", { path: repoPath, filePath });
        } catch (error) {
          errors.push(`${filePath}: ${error}`);
        }
      }
      await loadStatus();
      if (errors.length > 0) {
        await message(`Failed to unstage:\n${errors.join("\n")}`, {
          title: "Error",
          kind: "error",
        });
      }
    },
    [repoPath, loadStatus]
  );

  const handleStageFile = async (file: string) => {
    await stageFiles([file]);
  };

  const handleUnstageFile = async (file: string) => {
    await unstageFiles([file]);
  };

  const selectedInFocusedList = useCallback(() => {
    const list = focusedListRef.current;
    const files =
      list === "unstaged"
        ? unstagedFilesRef.current
        : stagedFilesRef.current;
    const valid = new Set(files.map((f) => f.path));
    return [...selectedPathsRef.current].filter((p) => valid.has(p));
  }, []);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (isEditableTarget(e.target)) return;

      const mod = e.metaKey || e.ctrlKey;
      const key = e.key.toLowerCase();

      // Ctrl/Cmd+A — select all in focused list
      if (mod && key === "a" && !e.altKey) {
        e.preventDefault();
        e.stopPropagation();
        const files =
          focusedListRef.current === "unstaged"
            ? unstagedFilesRef.current
            : stagedFilesRef.current;
        if (files.length === 0) return;
        setSelectedPaths(new Set(files.map((f) => f.path)));
        setAnchorPath(files[0].path);
        activateFile(files[0].path, focusedListRef.current === "staged");
        return;
      }

      // s / Ctrl+S — stage; u / Ctrl+U — unstage; Enter — same as focused list
      const paths = selectedInFocusedList();
      if (paths.length === 0) return;

      if ((key === "s" || e.code === "KeyS") && !e.altKey && !e.shiftKey) {
        if (focusedListRef.current !== "unstaged") return;
        e.preventDefault();
        e.stopPropagation();
        void stageFiles(paths);
        return;
      }

      if ((key === "u" || e.code === "KeyU") && !e.altKey && !e.shiftKey) {
        if (focusedListRef.current !== "staged") return;
        e.preventDefault();
        e.stopPropagation();
        void unstageFiles(paths);
        return;
      }

      if (e.key === "Enter" && !mod && !e.altKey && !e.shiftKey) {
        e.preventDefault();
        e.stopPropagation();
        if (focusedListRef.current === "unstaged") void stageFiles(paths);
        else void unstageFiles(paths);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activateFile, selectedInFocusedList, stageFiles, unstageFiles]);

  const handleStageHunk = async (hunkHeader?: string, hunkLines?: string) => {
    if (!selectedFile || !hunkHeader || !hunkLines) return;

    try {
      if (selectedStaged) {
        await invoke("unstage_hunk", {
          path: repoPath,
          filePath: selectedFile,
          fullDiff: diff,
          hunkHeader,
          hunkLines,
        });
      } else {
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

  const handleCommit = async () => {
    try {
      await invoke("create_commit", {
        path: repoPath,
        message: commitMessage,
        amend,
      });
      setCommitMessage("");
      setAmend(false);
      loadStatus();
    } catch (error) {
      await message(`Failed to commit: ${error}`, {
        title: "Commit Error",
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

  const contextSelection = (() => {
    if (!contextMenu) return [] as string[];
    const list = contextMenu.staged ? stagedFiles : unstagedFiles;
    const valid = new Set(list.map((f) => f.path));
    const selected = [...selectedPaths].filter((p) => valid.has(p));
    if (selected.length > 0) return selected;
    return [contextMenu.file];
  })();

  const renderFileList = (list: FileList, files: FileStatus[]) => {
    const staged = list === "staged";
    return (
      <div
        className={`stage-panel ${focusedList === list ? "focused" : ""}`}
        tabIndex={0}
        onMouseDown={() => setFocusedList(list)}
      >
        <div className="panel-header">
          {staged ? "Staged Changes" : "Unstaged Changes"}
          {focusedList === list && selectedPaths.size > 1 ? (
            <span className="selection-count">{selectedPaths.size} selected</span>
          ) : null}
        </div>
        <div className="panel-content">
          {files.map((file) => {
            const isSelected = selectedPaths.has(file.path);
            const isActive =
              selectedFile === file.path && selectedStaged === staged;
            return (
              <div
                key={file.path}
                className={`file-item ${isSelected ? "selected" : ""} ${isActive ? "active" : ""}`}
                onClick={(e) => handleFileClick(e, file.path, list)}
                onDoubleClick={() =>
                  staged
                    ? handleUnstageFile(file.path)
                    : handleStageFile(file.path)
                }
                onContextMenu={(e) =>
                  handleContextMenu(e, file.path, staged, file.status)
                }
              >
                <span className="file-status">
                  {file.status[0].toUpperCase()}
                </span>
                <span style={{ color: getStatusColor(file.status) }}>
                  {file.path}
                </span>
              </div>
            );
          })}
        </div>
      </div>
    );
  };

  return (
    <div className="stage-view">
      <div className="stage-main">
        {selectedFile ? (
          <>
            <div className="stage-diff-header">
              {selectedStaged ? "Staged" : "Unstaged"} changes for{" "}
              {selectedFile}
              {selectedPaths.size > 1 ? ` (+${selectedPaths.size - 1} more)` : ""}
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
        {renderFileList("unstaged", unstagedFiles)}
        <div className="stage-panel commit-panel">
          <div className="panel-header">Commit Message</div>
          <div className="panel-content">
            <textarea
              className="commit-message"
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
              placeholder="Enter commit message..."
              spellCheck={false}
              autoCorrect="off"
              autoCapitalize="off"
            />
            <div className="commit-actions">
              <button
                className="stash-button"
                disabled={
                  stagedFiles.length === 0 && unstagedFiles.length === 0
                }
                onClick={onStash}
              >
                Stash
              </button>
              <div className="commit-actions-right">
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
                  onClick={handleCommit}
                >
                  Commit
                </button>
              </div>
            </div>
          </div>
        </div>
        {renderFileList("staged", stagedFiles)}
      </div>
      {contextMenu && (
        <div
          className="context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          {contextMenu.staged ? (
            <div
              className="context-menu-item"
              onClick={() => void unstageFiles(contextSelection)}
            >
              {contextSelection.length > 1
                ? `Unstage ${contextSelection.length} Files`
                : `Unstage ${contextMenu.file}`}
            </div>
          ) : (
            <>
              <div
                className="context-menu-item"
                onClick={() => void stageFiles(contextSelection)}
              >
                {contextSelection.length > 1
                  ? `Stage ${contextSelection.length} Files`
                  : `Stage ${contextMenu.file}`}
              </div>
              {contextSelection.length === 1 &&
                contextMenu.status === "modified" && (
                  <div
                    className="context-menu-item"
                    onClick={() => handleDiscardChanges(contextMenu.file)}
                  >
                    Discard changes to {contextMenu.file}
                  </div>
                )}
              {contextSelection.length === 1 && (
                <div
                  className="context-menu-item"
                  onClick={() => handleIgnoreFile(contextMenu.file)}
                >
                  Ignore {contextMenu.file}
                </div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
