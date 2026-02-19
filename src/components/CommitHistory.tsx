import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./CommitHistory.css";

interface CommitHistoryProps {
  repoPath: string;
  onCommitSelect: (commit: GitCommit | null) => void;
  currentBranch?: string;
}

interface GitCommit {
  id: string;
  message: string;
  author: string;
  email: string;
  timestamp: string;
  parents: string[];
  branches?: string[];
  tags?: string[];
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

  useEffect(() => {
    loadAllCommits();
  }, [repoPath, currentBranch]);

  useEffect(() => {
    // Scroll to selected commit when filter changes
    if (selectedCommit) {
      setTimeout(() => {
        const element = document.querySelector(
          `[data-commit-id="${selectedCommit.id}"]`
        );
        if (element) {
          element.scrollIntoView({ behavior: "smooth", block: "center" });
        }
      }, 0);
    }
  }, [branchFilter]);

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

  const loadAllCommits = async () => {
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
                      {commit.branches &&
                        commit.branches.map((branch) => {
                          const isRemote = branch.includes("/");
                          const className = isRemote
                            ? "remote-branch-tag"
                            : "local-branch-tag";
                          return (
                            <span key={branch} className={className}>
                              {branch}
                            </span>
                          );
                        })}
                      {commit.tags &&
                        commit.tags.map((tag) => (
                          <span key={tag} className="tag-badge">
                            {tag}
                          </span>
                        ))}
                      {commit.message.split("\n")[0]}
                    </td>
                    <td className="author">{commit.author}</td>
                    <td className="date">{formatDate(commit.timestamp)}</td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export type { GitCommit };
