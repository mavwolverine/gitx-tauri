import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { message } from "@tauri-apps/plugin-dialog";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { BranchTree } from "./components/BranchTree";
import { StageView } from "./components/StageView";
import { CommitHistory, GitCommit } from "./components/CommitHistory";
import "./RepoView.css";

interface RepoViewProps {
  repoPath: string;
}

interface GitBranch {
  name: string;
  is_head: boolean;
}

interface GitRemote {
  name: string;
  url: string;
  branches?: string[];
}

interface GitSubmodule {
  name: string;
  path: string;
  url: string;
}

interface CommitFile {
  path: string;
  old_path?: string;
  status: string;
  additions: number;
  deletions: number;
  lines: DiffLine[];
}

interface DiffLine {
  old_lineno?: number;
  new_lineno?: number;
  origin: string;
  content: string;
}

function RepoView({ repoPath }: RepoViewProps) {
  const [repoName, setRepoName] = useState("");
  const [branches, setBranches] = useState<GitBranch[]>([]);
  const [remotes, setRemotes] = useState<GitRemote[]>([]);
  const [tags, setTags] = useState<string[]>([]);
  const [submodules, setSubmodules] = useState<GitSubmodule[]>([]);
  const [selectedBranch, setSelectedBranch] = useState<string | null>(null);
  const [selectedCommit, setSelectedCommit] = useState<GitCommit | null>(null);
  const [commitFiles, setCommitFiles] = useState<CommitFile[]>([]);
  const [collapsedFiles, setCollapsedFiles] = useState<Set<number>>(new Set());
  const [showBackToTop, setShowBackToTop] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string>("");

  const showStatus = (message: string, duration = 3000) => {
    setStatusMessage(message);
    if (duration > 0) {
      setTimeout(() => setStatusMessage(""), duration);
    }
  };
  const [currentView, setCurrentView] = useState<"stage" | null>("stage");
  const [collapsed, setCollapsed] = useState({
    branches: false,
    remotes: false,
    tags: true,
    submodules: false,
  });
  const [remoteCollapsed, setRemoteCollapsed] = useState<{
    [key: string]: boolean;
  }>({});

  useEffect(() => {
    const name = repoPath.split(/[/\\]/).pop() || repoPath;
    setRepoName(name);
    loadBranches();
    loadRemotes();
    loadSubmodules();

    // Start watching the repo
    invoke("watch_repo", { repoPath }).catch((error) =>
      console.error("Failed to start watching repo:", error)
    );

    // Listen for repo changes
    const unlisten = listen("repo-changed", () => {
      loadBranches();
      loadRemotes();
      loadSubmodules();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [repoPath]);

  const loadBranches = async () => {
    try {
      const branchList = await invoke<GitBranch[]>("get_branches", {
        path: repoPath,
      });
      setBranches(branchList);
    } catch (error) {
      console.error("Failed to load branches:", error);
    }
  };

  const loadRemotes = async () => {
    try {
      const remoteList = await invoke<GitRemote[]>("get_remotes", {
        path: repoPath,
      });
      // Deduplicate by name
      const uniqueRemotes = remoteList.filter(
        (remote, index, self) =>
          index === self.findIndex((r) => r.name === remote.name)
      );
      setRemotes(uniqueRemotes);
      const initialCollapsed: { [key: string]: boolean } = {};
      uniqueRemotes.forEach((remote) => {
        initialCollapsed[remote.name] = true;
      });
      setRemoteCollapsed(initialCollapsed);
    } catch (error) {
      console.error("Failed to load remotes:", error);
    }
  };

  const loadRemoteBranches = async (remoteName: string) => {
    try {
      const branches = await invoke<string[]>("get_remote_branches", {
        path: repoPath,
        remoteName,
      });
      setRemotes((prev) =>
        prev.map((r) => (r.name === remoteName ? { ...r, branches } : r))
      );
    } catch (error) {
      console.error("Failed to load remote branches:", error);
    }
  };

  const loadTags = async () => {
    try {
      const tagList = await invoke<string[]>("get_tags", { path: repoPath });
      setTags(tagList);
    } catch (error) {
      console.error("Failed to load tags:", error);
    }
  };

  const loadSubmodules = async () => {
    try {
      const submoduleList = await invoke<GitSubmodule[]>("get_submodules", {
        path: repoPath,
      });
      setSubmodules(submoduleList);
    } catch (error) {
      console.error("Failed to load submodules:", error);
    }
  };

  const handleCheckoutBranch = async (branchName: string) => {
    try {
      await invoke("checkout_branch", { path: repoPath, branchName });
      await loadBranches();
    } catch (error) {
      await message(`Failed to checkout branch: ${error}`, {
        title: "Checkout Error",
        kind: "error",
      });
    }
  };

  const handleCreateBranch = async () => {
    await message("Coming soon", {
      title: "Create Branch",
      kind: "info",
    });
  };

  const handleCreateTag = async () => {
    await message("Coming soon", {
      title: "Create Tag",
      kind: "info",
    });
  };

  const handleFetch = async (_branch: string, remote: string) => {
    try {
      showStatus(`Fetching from ${remote}...`, 0);
      await invoke("fetch_remote", { path: repoPath, remoteName: remote });
      await loadBranches();
      await loadRemotes();
      showStatus(`Fetch from ${remote} complete`);
    } catch (error) {
      showStatus("");
      await message(`Failed to fetch from ${remote}: ${error}`, {
        title: "Fetch Error",
        kind: "error",
      });
    }
  };

  const handlePull = async (_branch: string, remote: string) => {
    try {
      showStatus(`Pulling from ${remote}...`, 0);
      await invoke("pull_remote", { path: repoPath, remoteName: remote });
      await loadBranches();
      await loadRemotes();
      showStatus(`Pull from ${remote} complete`);
    } catch (error) {
      showStatus("");
      await message(`Failed to pull from ${remote}: ${error}`, {
        title: "Pull Error",
        kind: "error",
      });
    }
  };

  const openSubmodule = async (submodulePath: string) => {
    try {
      const fullPath = `${repoPath}/${submodulePath}`;
      await invoke("open_repo_window", { repoPath: fullPath });
    } catch (error) {
      console.error("Failed to open submodule:", error);
    }
  };

  const toggleRemote = (remoteName: string) => {
    const isCurrentlyCollapsed = remoteCollapsed[remoteName];
    setRemoteCollapsed((prev) => ({
      ...prev,
      [remoteName]: !prev[remoteName],
    }));

    if (isCurrentlyCollapsed) {
      const remote = remotes.find((r) => r.name === remoteName);
      if (remote && !remote.branches) {
        loadRemoteBranches(remoteName);
      }
    }
  };

  const toggleSection = (section: keyof typeof collapsed) => {
    const isCurrentlyCollapsed = collapsed[section];
    setCollapsed((prev) => ({ ...prev, [section]: !prev[section] }));

    if (section === "tags" && isCurrentlyCollapsed && tags.length === 0) {
      loadTags();
    }
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

  const handleCommitSelect = async (commit: GitCommit | null) => {
    setSelectedCommit(commit);
    if (commit) {
      try {
        const files = await invoke<CommitFile[]>("get_commit_diff", {
          path: repoPath,
          commitId: commit.id,
        });
        setCommitFiles(files);
      } catch (error) {
        console.error("Failed to load commit diff:", error);
        setCommitFiles([]);
      }
    } else {
      setCommitFiles([]);
    }
  };

  const scrollToFile = (index: number) => {
    const element = document.getElementById(`file-diff-${index}`);
    if (element) {
      element.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  };

  const scrollToTop = () => {
    const container = document.querySelector(".commit-details-panel");
    if (container) {
      container.scrollTo({ top: 0, behavior: "smooth" });
    }
  };

  const handleDiffScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.target as HTMLDivElement;
    console.log("Scroll position:", target.scrollTop);
    setShowBackToTop(target.scrollTop > 300);
  };

  const toggleFileCollapse = (index: number) => {
    setCollapsedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  };

  return (
    <div className="repo-view">
      <div className="repo-content">
        <div className="sidebar">
          <div className="sidebar-header">
            <h2>{repoName}</h2>
          </div>
          <div className="sidebar-content">
            <div className="sidebar-section">
              <div
                className="section-item"
                onClick={() => setCurrentView("stage")}
              >
                <span className="item-icon">📝</span>
                Stage
              </div>
            </div>
            <div className="sidebar-section collapsible">
              <div
                className="section-header"
                onClick={() => toggleSection("branches")}
              >
                <span>Branches</span>
                <span className="collapse-icon">
                  {collapsed.branches ? "▶" : "▼"}
                </span>
              </div>
              {!collapsed.branches && (
                <div className="section-body">
                  <BranchTree
                    branches={branches}
                    selectedBranch={selectedBranch}
                    onSelectBranch={(branch) => {
                      setSelectedBranch(branch);
                      setCurrentView(null);
                    }}
                    onCheckoutBranch={handleCheckoutBranch}
                    onCreateBranch={handleCreateBranch}
                    onCreateTag={handleCreateTag}
                    onFetch={handleFetch}
                    onPull={handlePull}
                    remotes={remotes}
                  />
                </div>
              )}
            </div>
            <div className="sidebar-section collapsible">
              <div
                className="section-header"
                onClick={() => toggleSection("remotes")}
              >
                <span>Remotes</span>
                <span className="collapse-icon">
                  {collapsed.remotes ? "▶" : "▼"}
                </span>
              </div>
              {!collapsed.remotes && (
                <div className="section-body">
                  {remotes.map((remote) => (
                    <div key={remote.name}>
                      <div
                        className="branch-folder"
                        style={{ paddingLeft: "16px" }}
                        onClick={() => toggleRemote(remote.name)}
                      >
                        <span className="folder-icon">
                          {remoteCollapsed[remote.name] ? "▶" : "▼"}
                        </span>
                        🌐 {remote.name}
                      </div>
                      {!remoteCollapsed[remote.name] && remote.branches && (
                        <div>
                          <BranchTree
                            branches={remote.branches.map((name) => ({
                              name: name,
                              is_head: false,
                            }))}
                            selectedBranch={selectedBranch}
                            onSelectBranch={(branch) => {
                              setSelectedBranch(`${remote.name}/${branch}`);
                              setCurrentView(null);
                            }}
                            level={1}
                            prefix={`remote-${remote.name}`}
                          />
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
            <div className="sidebar-section collapsible">
              <div
                className="section-header"
                onClick={() => toggleSection("tags")}
              >
                <span>Tags</span>
                <span className="collapse-icon">
                  {collapsed.tags ? "▶" : "▼"}
                </span>
              </div>
              {!collapsed.tags && (
                <div className="section-body">
                  {tags.map((tag) => (
                    <div
                      key={tag}
                      className="branch-item"
                      style={{ paddingLeft: "16px" }}
                      onClick={async () => {
                        try {
                          const commitSha = await invoke<string>(
                            "get_tag_commit",
                            {
                              path: repoPath,
                              tagName: tag,
                            }
                          );

                          setTimeout(() => {
                            const element = document.querySelector(
                              `[data-commit-id="${commitSha}"]`
                            );
                            if (element) {
                              element.scrollIntoView({
                                behavior: "smooth",
                                block: "center",
                              });
                              setTimeout(() => {
                                (element as HTMLElement).click();
                              }, 300);
                            }
                          }, 50);
                        } catch (error) {
                          console.error("Failed to load tag commit:", error);
                        }
                      }}
                    >
                      <span
                        className="folder-icon"
                        style={{ visibility: "hidden" }}
                      >
                        ▼
                      </span>
                      <span className="item-icon">🏷️</span>
                      {tag}
                    </div>
                  ))}
                </div>
              )}
            </div>
            <div className="sidebar-section collapsible">
              <div
                className="section-header"
                onClick={() => toggleSection("submodules")}
              >
                <span>Submodules</span>
                <span className="collapse-icon">
                  {collapsed.submodules ? "▶" : "▼"}
                </span>
              </div>
              {!collapsed.submodules && (
                <div className="section-body">
                  {submodules.map((submodule) => (
                    <div
                      key={submodule.path}
                      className="branch-item"
                      style={{ paddingLeft: "16px" }}
                      onDoubleClick={() => openSubmodule(submodule.path)}
                    >
                      <span
                        className="folder-icon"
                        style={{ visibility: "hidden" }}
                      >
                        ▼
                      </span>
                      <span className="item-icon">📦</span>
                      {submodule.name}
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
        <div className="main-content">
          {currentView === "stage" ? (
            <StageView repoPath={repoPath} />
          ) : selectedBranch ? (
            <PanelGroup direction="vertical">
              <Panel defaultSize={60} minSize={20}>
                <div className="history-panel">
                  <CommitHistory
                    repoPath={repoPath}
                    onCommitSelect={handleCommitSelect}
                    currentBranch={selectedBranch}
                  />
                </div>
              </Panel>
              <PanelResizeHandle className="resize-handle-horizontal" />
              <Panel defaultSize={40} minSize={20}>
                <div
                  className="commit-details-panel"
                  onScroll={handleDiffScroll}
                >
                  {selectedCommit ? (
                    <>
                      <div className="details-header">
                        <div className="detail-row">
                          <span className="detail-label">Subject</span>
                          <span className="detail-value">
                            {selectedCommit.message.split("\n")[0]}
                          </span>
                        </div>
                        <div className="detail-row">
                          <span className="detail-label">ID</span>
                          <span className="detail-value">
                            {selectedCommit.id}
                            {selectedCommit.branches &&
                              selectedCommit.branches.length > 0 && (
                                <span
                                  className={
                                    selectedCommit.branches[0].includes("/")
                                      ? "branch-badge-remote"
                                      : "branch-badge-local"
                                  }
                                >
                                  {selectedCommit.branches[0]}
                                </span>
                              )}
                          </span>
                        </div>
                        <div className="detail-row">
                          <span className="detail-label">Parents</span>
                          <span className="detail-value">
                            {selectedCommit.parents.map((parent, i) => (
                              <span key={parent}>
                                <span className="parent-sha">{parent}</span>
                                {i < selectedCommit.parents.length - 1 && ", "}
                              </span>
                            ))}
                          </span>
                        </div>
                        <div className="detail-row author-row">
                          <span className="detail-label">Author</span>
                          <div className="author-info">
                            <div className="avatar">
                              {selectedCommit.author.charAt(0).toUpperCase()}
                            </div>
                            <div className="author-details">
                              <div className="author-name">
                                {selectedCommit.author} &lt;
                                {selectedCommit.email}
                                &gt;
                              </div>
                              <div className="author-date">
                                {formatDate(selectedCommit.timestamp)}
                              </div>
                              <div className="commit-date">
                                {formatDate(selectedCommit.timestamp)} (Commit
                                date)
                              </div>
                            </div>
                          </div>
                        </div>
                      </div>
                      <div className="commit-message-section">
                        <div className="message-content">
                          {selectedCommit.message}
                        </div>
                      </div>
                      <div className="commit-files-section">
                        {commitFiles.length > 0 ? (
                          <>
                            <div className="file-list-header">
                              <span className="file-list-title">
                                Files ({commitFiles.length})
                              </span>
                              <button
                                className="collapse-all-btn"
                                onClick={() => {
                                  if (
                                    collapsedFiles.size === commitFiles.length
                                  ) {
                                    setCollapsedFiles(new Set());
                                  } else {
                                    setCollapsedFiles(
                                      new Set(commitFiles.map((_, i) => i))
                                    );
                                  }
                                }}
                              >
                                {collapsedFiles.size === commitFiles.length
                                  ? "Expand All"
                                  : "Collapse All"}
                              </button>
                            </div>
                            <div className="file-list">
                              {commitFiles.map((file, idx) => {
                                const getFileIcon = () => {
                                  if (file.status === "Added") return "🟢";
                                  if (file.status === "Deleted") return "🔴";
                                  return "🟠";
                                };

                                return (
                                  <div
                                    key={idx}
                                    className="file-list-item"
                                    onClick={() => scrollToFile(idx)}
                                  >
                                    <span className="file-path">
                                      <span className="file-status-icon">
                                        {getFileIcon()}
                                      </span>
                                      {file.path}
                                    </span>
                                    <span className="file-changes">
                                      {file.additions > 0 ? (
                                        <span className="additions">
                                          +{file.additions}
                                        </span>
                                      ) : (
                                        <span></span>
                                      )}
                                      {file.deletions > 0 ? (
                                        <span className="deletions">
                                          -{file.deletions}
                                        </span>
                                      ) : (
                                        <span></span>
                                      )}
                                    </span>
                                  </div>
                                );
                              })}
                            </div>
                            <div className="file-diff-container">
                              {commitFiles.map((file, idx) => (
                                <div
                                  key={idx}
                                  id={`file-diff-${idx}`}
                                  className="file-section"
                                >
                                  <div className="file-diff-header">
                                    <span className="file-header-left">
                                      <span
                                        className="collapse-icon"
                                        onClick={() => toggleFileCollapse(idx)}
                                      >
                                        {collapsedFiles.has(idx) ? "▶" : "▼"}
                                      </span>
                                      <span className="file-path">
                                        {file.path}
                                      </span>
                                    </span>
                                    <span className="file-changes">
                                      {file.additions > 0 && (
                                        <span className="additions">
                                          +{file.additions}
                                        </span>
                                      )}
                                      {file.deletions > 0 && (
                                        <span className="deletions">
                                          -{file.deletions}
                                        </span>
                                      )}
                                    </span>
                                  </div>
                                  {!collapsedFiles.has(idx) && (
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

                                        const content = line.content.replace(
                                          /\n$/,
                                          ""
                                        );

                                        return (
                                          <div key={i} className={className}>
                                            <span className="line-number">
                                              {line.old_lineno || ""}
                                            </span>
                                            <span className="line-number">
                                              {line.new_lineno || ""}
                                            </span>
                                            <span className="line-origin">
                                              {line.origin === " "
                                                ? " "
                                                : line.origin}
                                            </span>
                                            <span className="line-content">
                                              {content || " "}
                                            </span>
                                          </div>
                                        );
                                      })}
                                    </div>
                                  )}
                                </div>
                              ))}
                            </div>
                            {showBackToTop && (
                              <button
                                className="back-to-top-btn"
                                onClick={scrollToTop}
                              >
                                ↑ Top
                              </button>
                            )}
                          </>
                        ) : (
                          <div className="file-diff">
                            <pre>Loading diff...</pre>
                          </div>
                        )}
                      </div>
                    </>
                  ) : (
                    <div className="no-selection">
                      Select a commit to view details
                    </div>
                  )}
                </div>
              </Panel>
            </PanelGroup>
          ) : (
            <div className="content-placeholder">
              Select an item from the sidebar
            </div>
          )}
        </div>
      </div>
      {statusMessage && (
        <div className="status-bar">
          <span>{statusMessage}</span>
        </div>
      )}
    </div>
  );
}

export default RepoView;
