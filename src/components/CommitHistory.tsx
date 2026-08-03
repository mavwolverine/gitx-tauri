import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./CommitHistory.css";

interface CommitHistoryProps {
  repoPath: string;
  onCommitSelect: (commit: GitCommit | null) => void;
  currentBranch?: string;
  jumpTarget?: { id: string; nonce: number } | null;
  commitViewMode: "detail" | "tree";
  onCommitViewModeChange: (mode: "detail" | "tree") => void;
  onCreateBranch?: (fromCommit: string) => void;
  onCreateTag?: (fromCommit: string) => void;
  onDeleteTag?: (tagName: string) => void;
  onApplyStash?: () => void;
  onPopStash?: () => void;
  onDropStash?: () => void;
}

interface GitAuthor {
  name: string;
  email: string;
  timestamp: string;
}

interface GitBranch {
  name: string;
  is_head: boolean;
  is_remote: boolean;
}

interface GitCommit {
  id: string;
  message: string;
  author: GitAuthor;
  committer: GitAuthor;
  parents: string[];
  branches?: GitBranch[];
  tags?: string[];
  is_stash: boolean;
  lane: number;
  lines: GraphLine[];
}

interface GraphLine {
  upper: boolean;
  from: number;
  to: number;
  color: number;
}

export function CommitHistory({
  repoPath,
  onCommitSelect,
  currentBranch,
  jumpTarget,
  commitViewMode,
  onCommitViewModeChange,
  onCreateBranch,
  onCreateTag,
  onDeleteTag,
  onApplyStash,
  onPopStash,
  onDropStash,
}: CommitHistoryProps) {
  const [commits, setCommits] = useState<{
    All: GitCommit[];
    Local: GitCommit[];
    Current: GitCommit[];
  }>({ All: [], Local: [], Current: [] });
  const [selectedCommit, setSelectedCommit] = useState<GitCommit | null>(null);
  const [loading, setLoading] = useState(false);
  const [graphWidth, setGraphWidth] = useState(60);
  const [messageWidth, setMessageWidth] = useState(400);
  const [branchFilter, setBranchFilter] = useState<string>("All");
  const [rowContextMenu, setRowContextMenu] = useState<{
    x: number;
    y: number;
    commitId: string;
  } | null>(null);
  const [tagBadgeContextMenu, setTagBadgeContextMenu] = useState<{
    x: number;
    y: number;
    tag: string;
  } | null>(null);
  const [stashBadgeContextMenu, setStashBadgeContextMenu] = useState<{
    x: number;
    y: number;
  } | null>(null);
  const selectedCommitRef = useRef(selectedCommit);
  selectedCommitRef.current = selectedCommit;
  const commitsRef = useRef(commits);
  commitsRef.current = commits;
  const branchFilterRef = useRef(branchFilter);
  branchFilterRef.current = branchFilter;

  useEffect(() => {
    const handleClick = () => {
      setRowContextMenu(null);
      setTagBadgeContextMenu(null);
      setStashBadgeContextMenu(null);
    };
    window.addEventListener("click", handleClick);
    return () => window.removeEventListener("click", handleClick);
  }, []);

  const loadAllCommits = useCallback(async () => {
    try {
      setLoading(true);
      // Load all commits, local commits, and current branch commits
      const [allList, localList, currentList] = await Promise.all([
        invoke<GitCommit[]>("get_commits", {
          path: repoPath,
          limit: 10000,
          localOnly: false,
          branchName: undefined,
        }),
        invoke<GitCommit[]>("get_commits", {
          path: repoPath,
          limit: 10000,
          localOnly: true,
          branchName: undefined,
        }),
        currentBranch
          ? invoke<GitCommit[]>("get_commits", {
              path: repoPath,
              limit: 10000,
              localOnly: false,
              branchName: currentBranch,
            })
          : Promise.resolve([]),
      ]);

      // Store all variants
      setCommits({
        All: allList,
        Local: localList,
        Current: currentList,
      });

      // Get branch HEAD commit and select it
      if (allList.length > 0 && currentBranch) {
        try {
          const branchHeadSha = await invoke<string>("get_branch_head", {
            path: repoPath,
            branchName: currentBranch,
          });
          const headCommit = allList.find((c) => c.id === branchHeadSha);
          const commitToSelect = headCommit || allList[0];
          setSelectedCommit(commitToSelect);
          onCommitSelect(commitToSelect);

          // Scroll to selected commit
          setTimeout(() => {
            const element = document.querySelector(
              `[data-commit-id="${commitToSelect.id}"]`
            );
            if (element) {
              element.scrollIntoView({ behavior: "smooth", block: "center" });
            }
          }, 100);
        } catch {
          // Fallback to first commit if branch head lookup fails
          setSelectedCommit(allList[0]);
          onCommitSelect(allList[0]);
        }
      }
    } catch (error) {
      console.error("Failed to load commits:", error);
    } finally {
      setLoading(false);
    }
  }, [repoPath, currentBranch, onCommitSelect]);

  useEffect(() => {
    loadAllCommits();
  }, [loadAllCommits]);

  useEffect(() => {
    const unlisten = listen("repo-changed", () => {
      loadAllCommits();
    });

    return () => {
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, [loadAllCommits]);

  useEffect(() => {
    // Scroll to selected commit when filter changes
    const commit = selectedCommitRef.current;
    if (commit) {
      setTimeout(() => {
        const element = document.querySelector(
          `[data-commit-id="${commit.id}"]`
        );
        if (element) {
          element.scrollIntoView({ behavior: "smooth", block: "center" });
        }
      }, 0);
    }
  }, [branchFilter]);

  useEffect(() => {
    if (!jumpTarget) return;
    const found = commitsRef.current.All.find((c) => c.id === jumpTarget.id);
    if (found) {
      if (branchFilterRef.current !== "All") setBranchFilter("All");
      setSelectedCommit(found);
      onCommitSelect(found);
      setTimeout(() => {
        const element = document.querySelector(
          `[data-commit-id="${found.id}"]`
        );
        if (element)
          element.scrollIntoView({ behavior: "smooth", block: "center" });
      }, 0);
    }
  }, [jumpTarget, onCommitSelect]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;

      const target = e.target as HTMLElement;
      if (
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.isContentEditable
      ) {
        return;
      }

      const commit = selectedCommitRef.current;
      if (!commit) return;

      const list =
        commitsRef.current[
          branchFilterRef.current as keyof typeof commitsRef.current
        ] || [];
      const idx = list.findIndex((c) => c.id === commit.id);
      if (idx === -1) return;

      const nextIdx = e.key === "ArrowUp" ? idx - 1 : idx + 1;
      if (nextIdx < 0 || nextIdx >= list.length) return;

      e.preventDefault();
      const next = list[nextIdx];
      setSelectedCommit(next);
      onCommitSelect(next);
      setTimeout(() => {
        const element = document.querySelector(`[data-commit-id="${next.id}"]`);
        if (element)
          element.scrollIntoView({ behavior: "smooth", block: "center" });
      }, 0);
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onCommitSelect]);

  const handleGraphResize = (e: React.MouseEvent) => {
    const startX = e.clientX;
    const startWidth = graphWidth;

    const handleMouseMove = (moveEvent: MouseEvent) => {
      const diff = moveEvent.clientX - startX;
      setGraphWidth(Math.max(40, startWidth + diff));
    };

    const handleMouseUp = () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
  };

  const handleResize = (e: React.MouseEvent) => {
    const startX = e.clientX;
    const startWidth = messageWidth;

    const handleMouseMove = (moveEvent: MouseEvent) => {
      const diff = moveEvent.clientX - startX;
      setMessageWidth(Math.max(200, startWidth + diff));
    };

    const handleMouseUp = () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
  };

  const handleCommitClick = (commit: GitCommit) => {
    setSelectedCommit(commit);
    onCommitSelect(commit);
  };

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

  const filteredCommits = commits[branchFilter as keyof typeof commits] || [];

  return (
    <div className="commit-history">
      <div className="branch-filter-bar">
        <div className="filter-btn-group">
          <button
            className={`filter-btn ${branchFilter === "All" ? "active" : ""}`}
            onClick={() => setBranchFilter("All")}
          >
            All
          </button>
          <button
            className={`filter-btn ${branchFilter === "Local" ? "active" : ""}`}
            onClick={() => setBranchFilter("Local")}
          >
            Local
          </button>
          {currentBranch && (
            <button
              className={`filter-btn ${branchFilter === "Current" ? "active" : ""}`}
              onClick={() => setBranchFilter("Current")}
            >
              "{currentBranch}"
            </button>
          )}
        </div>
        <div className="filter-btn-group">
          <button
            className={`filter-btn icon-btn ${commitViewMode === "detail" ? "active" : ""}`}
            onClick={() => onCommitViewModeChange("detail")}
            title="Detailed View"
            aria-label="Detailed View"
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
              <rect x="0" y="1" width="2" height="2" />
              <rect x="4" y="1" width="12" height="2" />
              <rect x="0" y="7" width="2" height="2" />
              <rect x="4" y="7" width="12" height="2" />
              <rect x="0" y="13" width="2" height="2" />
              <rect x="4" y="13" width="12" height="2" />
            </svg>
          </button>
          <button
            className={`filter-btn icon-btn ${commitViewMode === "tree" ? "active" : ""}`}
            onClick={() => onCommitViewModeChange("tree")}
            title="Tree View"
            aria-label="Tree View"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
            >
              <rect x="6.5" y="0.5" width="3" height="3" fill="currentColor" />
              <line x1="8" y1="3.5" x2="8" y2="8" />
              <line x1="2" y1="8" x2="14" y2="8" />
              <line x1="2" y1="8" x2="2" y2="10.5" />
              <line x1="8" y1="8" x2="8" y2="10.5" />
              <line x1="14" y1="8" x2="14" y2="10.5" />
              <rect x="0.5" y="10.5" width="3" height="3" fill="currentColor" />
              <rect x="6.5" y="10.5" width="3" height="3" fill="currentColor" />
              <rect
                x="12.5"
                y="10.5"
                width="3"
                height="3"
                fill="currentColor"
              />
            </svg>
          </button>
        </div>
      </div>
      <div className="commit-table-container">
        <table className="commit-table">
          <thead>
            <tr>
              <th style={{ width: "60px" }}>
                SHA
                <div className="column-resizer"></div>
              </th>
              <th style={{ width: `${graphWidth}px` }}>
                <div
                  className="column-resizer"
                  onMouseDown={handleGraphResize}
                ></div>
              </th>
              <th style={{ width: `${messageWidth}px` }}>
                Subject
                <div
                  className="column-resizer"
                  onMouseDown={handleResize}
                ></div>
              </th>
              <th style={{ width: "125px" }}>
                Author
                <div className="column-resizer"></div>
              </th>
              <th style={{ width: "235px" }}>
                Date
                <div className="column-resizer"></div>
              </th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr>
                <td colSpan={5} className="loading">
                  Loading commits...
                </td>
              </tr>
            ) : (
              filteredCommits.map((commit) => {
                return (
                  <tr
                    key={commit.id}
                    data-commit-id={commit.id}
                    className={
                      selectedCommit?.id === commit.id ? "selected" : ""
                    }
                    onClick={() => handleCommitClick(commit)}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setRowContextMenu({
                        x: e.clientX,
                        y: e.clientY,
                        commitId: commit.id,
                      });
                    }}
                  >
                    <td className="sha">{commit.id.substring(0, 7)}</td>
                    <td className="graph">
                      <svg width={graphWidth} height="26">
                        {/* Draw lines */}
                        {commit.lines.map((line, idx) => {
                          const y1 = line.upper ? 0 : 13;
                          const y2 = line.upper ? 13 : 26;
                          return (
                            <line
                              key={idx}
                              x1={line.from * 15 + 7.5}
                              y1={y1}
                              x2={line.to * 15 + 7.5}
                              y2={y2}
                              stroke={`hsl(${(line.color * 60) % 360}, 70%, 60%)`}
                              strokeWidth="2"
                            />
                          );
                        })}
                        {/* Draw circle */}
                        <circle
                          cx={commit.lane * 15 + 7.5}
                          cy="13"
                          r="4"
                          fill="white"
                        />
                      </svg>
                    </td>
                    <td className="message">
                      {commit.is_stash && (
                        <span
                          className="stash-badge"
                          onContextMenu={
                            onApplyStash || onPopStash || onDropStash
                              ? (e) => {
                                  e.preventDefault();
                                  e.stopPropagation();
                                  setStashBadgeContextMenu({
                                    x: e.clientX,
                                    y: e.clientY,
                                  });
                                }
                              : undefined
                          }
                        >
                          refs/stash
                        </span>
                      )}
                      {commit.branches &&
                        commit.branches.map((branch) => {
                          const className = branch.is_head
                            ? "branch-badge-local-head"
                            : branch.is_remote
                              ? "branch-badge-remote"
                              : "branch-badge-local";
                          return (
                            <span key={branch.name} className={className}>
                              {branch.name}
                            </span>
                          );
                        })}
                      {commit.tags &&
                        commit.tags.map((tag) => (
                          <span
                            key={tag}
                            className="tag-badge"
                            onContextMenu={
                              onDeleteTag
                                ? (e) => {
                                    e.preventDefault();
                                    e.stopPropagation();
                                    setTagBadgeContextMenu({
                                      x: e.clientX,
                                      y: e.clientY,
                                      tag,
                                    });
                                  }
                                : undefined
                            }
                          >
                            {tag}
                          </span>
                        ))}
                      {commit.message.split("\n")[0]}
                    </td>
                    <td className="author">{commit.author.name}</td>
                    <td className="date">
                      {formatDate(commit.committer.timestamp)}
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>
      {rowContextMenu && (onCreateBranch || onCreateTag) && (
        <div
          className="context-menu"
          style={{ left: rowContextMenu.x, top: rowContextMenu.y }}
        >
          {onCreateBranch && (
            <div
              className="context-menu-item"
              onClick={() => {
                onCreateBranch(rowContextMenu.commitId);
                setRowContextMenu(null);
              }}
            >
              Create Branch...
            </div>
          )}
          {onCreateTag && (
            <div
              className="context-menu-item"
              onClick={() => {
                onCreateTag(rowContextMenu.commitId);
                setRowContextMenu(null);
              }}
            >
              Create Tag...
            </div>
          )}
        </div>
      )}
      {tagBadgeContextMenu && onDeleteTag && (
        <div
          className="context-menu"
          style={{ left: tagBadgeContextMenu.x, top: tagBadgeContextMenu.y }}
        >
          <div
            className="context-menu-item"
            onClick={() => {
              onDeleteTag(tagBadgeContextMenu.tag);
              setTagBadgeContextMenu(null);
            }}
          >
            Delete Tag...
          </div>
        </div>
      )}
      {stashBadgeContextMenu && (onApplyStash || onPopStash || onDropStash) && (
        <div
          className="context-menu"
          style={{
            left: stashBadgeContextMenu.x,
            top: stashBadgeContextMenu.y,
          }}
        >
          {onPopStash && (
            <div
              className="context-menu-item"
              onClick={() => {
                onPopStash();
                setStashBadgeContextMenu(null);
              }}
            >
              Pop Stash
            </div>
          )}
          {onApplyStash && (
            <div
              className="context-menu-item"
              onClick={() => {
                onApplyStash();
                setStashBadgeContextMenu(null);
              }}
            >
              Apply Stash
            </div>
          )}
          {onDropStash && (
            <>
              <div className="context-menu-separator" />
              <div
                className="context-menu-item"
                onClick={() => {
                  onDropStash();
                  setStashBadgeContextMenu(null);
                }}
              >
                Drop Stash...
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}

export type { GitCommit };
