import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { message, confirm } from "@tauri-apps/plugin-dialog";
import { Panel, Group, Separator } from "react-resizable-panels";
import { BranchTree } from "./components/BranchTree";
import { StageView } from "./components/StageView";
import { CommitHistory, GitCommit } from "./components/CommitHistory";
import { CommitTreeView, TreeNode } from "./components/CommitTreeView";
import { FileViewer } from "./components/FileViewer";
import {
  DeleteBranchDialog,
  BranchDeleteInfo,
} from "./components/DeleteBranchDialog";
import { CreateBranchDialog } from "./components/CreateBranchDialog";
import { CreateTagDialog } from "./components/CreateTagDialog";
import { StashDialog } from "./components/StashDialog";
import { StashDiffDialog } from "./components/StashDiffDialog";
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

interface GitStash {
  index: number;
  oid: string;
  message: string;
  timestamp: string;
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

function findTreeNodeByPath(nodes: TreeNode[], path: string): TreeNode | null {
  for (const node of nodes) {
    if (node.path === path) return node;
    if (node.children) {
      const found = findTreeNodeByPath(node.children, path);
      if (found) return found;
    }
  }
  return null;
}

function RepoView({ repoPath }: RepoViewProps) {
  const [repoName, setRepoName] = useState("");
  const [branches, setBranches] = useState<GitBranch[]>([]);
  const [remotes, setRemotes] = useState<GitRemote[]>([]);
  const [tags, setTags] = useState<string[]>([]);
  const [submodules, setSubmodules] = useState<GitSubmodule[]>([]);
  const [stashes, setStashes] = useState<GitStash[]>([]);
  const [selectedBranch, setSelectedBranch] = useState<string | null>(null);
  const [selectedCommit, setSelectedCommit] = useState<GitCommit | null>(null);
  const [commitFiles, setCommitFiles] = useState<CommitFile[]>([]);
  const [collapsedFiles, setCollapsedFiles] = useState<Set<number>>(new Set());
  const [showBackToTop, setShowBackToTop] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string>("");
  const [commitViewMode, setCommitViewMode] = useState<"detail" | "tree">(
    "detail"
  );
  const [commitTree, setCommitTree] = useState<TreeNode[]>([]);
  const [treeLoading, setTreeLoading] = useState(false);
  const [selectedTreeFile, setSelectedTreeFile] = useState<TreeNode | null>(
    null
  );
  const [jumpTarget, setJumpTarget] = useState<{
    id: string;
    nonce: number;
  } | null>(null);
  const [deleteBranchTarget, setDeleteBranchTarget] = useState<{
    branch: string;
    remoteContext?: string;
    info: BranchDeleteInfo;
  } | null>(null);
  const [createBranchFrom, setCreateBranchFrom] = useState<string | null>(null);
  const [createTagFrom, setCreateTagFrom] = useState<string | null>(null);
  const [tagContextMenu, setTagContextMenu] = useState<{
    x: number;
    y: number;
    tag: string;
  } | null>(null);
  const [stashDialogOpen, setStashDialogOpen] = useState(false);
  const [stashDiffTarget, setStashDiffTarget] = useState<{
    oid: string;
    message: string;
  } | null>(null);
  const [stashContextMenu, setStashContextMenu] = useState<{
    x: number;
    y: number;
    index: number;
  } | null>(null);
  const selectedCommitRef = useRef(selectedCommit);
  selectedCommitRef.current = selectedCommit;

  useEffect(() => {
    const handleClick = () => {
      setTagContextMenu(null);
      setStashContextMenu(null);
    };
    window.addEventListener("click", handleClick);
    return () => window.removeEventListener("click", handleClick);
  }, []);

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
    stashes: false,
  });
  const [remoteCollapsed, setRemoteCollapsed] = useState<{
    [key: string]: boolean;
  }>({});

  const loadBranches = useCallback(async () => {
    try {
      const branchList = await invoke<GitBranch[]>("get_branches", {
        path: repoPath,
      });
      setBranches(branchList);
    } catch (error) {
      console.error("Failed to load branches:", error);
    }
  }, [repoPath]);

  const loadRemotes = useCallback(async () => {
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
  }, [repoPath]);

  const loadSubmodules = useCallback(async () => {
    try {
      const submoduleList = await invoke<GitSubmodule[]>("get_submodules", {
        path: repoPath,
      });
      setSubmodules(submoduleList);
    } catch (error) {
      console.error("Failed to load submodules:", error);
    }
  }, [repoPath]);

  const loadStashes = useCallback(async () => {
    try {
      const stashList = await invoke<GitStash[]>("get_stashes", {
        path: repoPath,
      });
      setStashes(stashList);
    } catch (error) {
      console.error("Failed to load stashes:", error);
    }
  }, [repoPath]);

  useEffect(() => {
    const name = repoPath.split(/[/\\]/).pop() || repoPath;
    setRepoName(name);
    loadBranches();
    loadRemotes();
    loadSubmodules();
    loadStashes();

    // Start watching the repo
    invoke("watch_repo", { repoPath }).catch((error) =>
      console.error("Failed to start watching repo:", error)
    );

    // Listen for repo changes
    const unlisten = listen("repo-changed", () => {
      loadBranches();
      loadRemotes();
      loadSubmodules();
      loadStashes();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [repoPath, loadBranches, loadRemotes, loadSubmodules, loadStashes]);

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

  const handleDeleteBranch = async (branch: string, remoteContext?: string) => {
    try {
      const info = await invoke<BranchDeleteInfo>("get_branch_delete_info", {
        path: repoPath,
        branchName: branch,
        remoteContext: remoteContext ?? null,
      });
      setDeleteBranchTarget({ branch, remoteContext, info });
    } catch (error) {
      await message(`Failed to inspect branch: ${error}`, {
        title: "Delete Branch",
        kind: "error",
      });
    }
  };

  const handleConfirmDeleteBranch = async (options: {
    deleteLocal: boolean;
    remotesToDelete: string[];
  }) => {
    if (!deleteBranchTarget) return;
    const { branch } = deleteBranchTarget;
    try {
      if (options.deleteLocal) {
        await invoke("delete_local_branch", {
          path: repoPath,
          branchName: branch,
        });
      }
      for (const remoteName of options.remotesToDelete) {
        await invoke("delete_remote_branch", {
          path: repoPath,
          remoteName,
          branchName: branch,
        });
      }
      if (
        selectedBranch === branch ||
        options.remotesToDelete.some(
          (remoteName) => selectedBranch === `${remoteName}/${branch}`
        )
      ) {
        setSelectedBranch(null);
      }
      await loadBranches();
      await loadRemotes();
      for (const remoteName of options.remotesToDelete) {
        await loadRemoteBranches(remoteName);
      }
      showStatus(`Deleted branch "${branch}"`);
    } catch (error) {
      await message(`Failed to delete branch: ${error}`, {
        title: "Delete Branch",
        kind: "error",
      });
    } finally {
      setDeleteBranchTarget(null);
    }
  };

  const handleCreateBranch = async (fromBranch: string) => {
    setCreateBranchFrom(fromBranch);
  };

  const handleConfirmCreateBranch = async (
    branchName: string,
    checkout: boolean
  ) => {
    if (!createBranchFrom) return;
    try {
      await invoke("create_branch", {
        path: repoPath,
        branchName,
        fromBranch: createBranchFrom,
      });
      if (checkout) {
        await invoke("checkout_branch", { path: repoPath, branchName });
      }
      await loadBranches();
      showStatus(`Created branch "${branchName}"`);
    } catch (error) {
      await message(`Failed to create branch: ${error}`, {
        title: "Create Branch Error",
        kind: "error",
      });
    } finally {
      setCreateBranchFrom(null);
    }
  };

  const handleCreateTag = async (fromRef: string) => {
    setCreateTagFrom(fromRef);
  };

  const handleConfirmCreateTag = async (
    tagName: string,
    tagMessage: string
  ) => {
    if (!createTagFrom) return;
    try {
      await invoke("create_tag", {
        path: repoPath,
        tagName,
        fromRef: createTagFrom,
        message: tagMessage || null,
      });
      await loadTags();
      showStatus(`Created tag "${tagName}"`);
    } catch (error) {
      await message(`Failed to create tag: ${error}`, {
        title: "Create Tag Error",
        kind: "error",
      });
    } finally {
      setCreateTagFrom(null);
    }
  };

  const handleDeleteTag = async (tagName: string) => {
    const confirmed = await confirm(`Delete tag "${tagName}"?`, {
      title: "Delete Tag",
      kind: "warning",
    });
    if (!confirmed) return;
    try {
      await invoke("delete_tag", { path: repoPath, tagName });
      await loadTags();
      showStatus(`Deleted tag "${tagName}"`);
    } catch (error) {
      await message(`Failed to delete tag: ${error}`, {
        title: "Delete Tag Error",
        kind: "error",
      });
    }
  };

  const handleCherryPick = async (commitId: string) => {
    try {
      await invoke("cherry_pick", { path: repoPath, commitId });
      showStatus(`Cherry-picked ${commitId.substring(0, 7)}`);
    } catch (error) {
      await message(`Failed to cherry-pick: ${error}`, {
        title: "Cherry-pick Error",
        kind: "error",
      });
    }
  };

  const handleSaveStash = async (
    stashMessage: string | null,
    includeUntracked: boolean
  ) => {
    try {
      await invoke("save_stash", {
        path: repoPath,
        message: stashMessage,
        includeUntracked,
      });
      await loadStashes();
      showStatus("Stashed changes");
    } catch (error) {
      await message(`Failed to stash changes: ${error}`, {
        title: "Stash Error",
        kind: "error",
      });
    } finally {
      setStashDialogOpen(false);
    }
  };

  const handleApplyStash = async (index: number) => {
    try {
      await invoke("apply_stash", { path: repoPath, index });
      showStatus("Applied stash");
    } catch (error) {
      await message(`Failed to apply stash: ${error}`, {
        title: "Apply Stash Error",
        kind: "error",
      });
    } finally {
      await loadStashes();
    }
  };

  const handlePopStash = async (index: number) => {
    try {
      await invoke("pop_stash", { path: repoPath, index });
      showStatus("Popped stash");
    } catch (error) {
      await message(`Failed to pop stash: ${error}`, {
        title: "Pop Stash Error",
        kind: "error",
      });
    } finally {
      await loadStashes();
    }
  };

  const handleDropStash = async (index: number, stashMessage: string) => {
    const confirmed = await confirm(`Drop stash "${stashMessage}"?`, {
      title: "Drop Stash",
      kind: "warning",
    });
    if (!confirmed) return;
    try {
      await invoke("drop_stash", { path: repoPath, index });
      await loadStashes();
      showStatus("Dropped stash");
    } catch (error) {
      await message(`Failed to drop stash: ${error}`, {
        title: "Drop Stash Error",
        kind: "error",
      });
    }
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

  const handlePush = async (branchName: string, remoteName: string) => {
    try {
      showStatus(`Pushing to ${remoteName}...`, 0);
      await invoke("push_remote", {
        path: repoPath,
        remoteName,
        branchName,
      });
      await loadBranches();
      await loadRemotes();
      showStatus(`Push to ${remoteName} complete`);
    } catch (error) {
      showStatus("");
      await message(`Failed to push to ${remoteName}: ${error}`, {
        title: "Push Error",
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

  const handleCommitSelect = useCallback(
    async (commit: GitCommit | null) => {
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
    },
    [repoPath]
  );

  useEffect(() => {
    const commit = selectedCommitRef.current;
    if (commitViewMode !== "tree" || !commit) return;
    setTreeLoading(true);
    invoke<TreeNode[]>("get_commit_tree", {
      path: repoPath,
      commitId: commit.id,
    })
      .then((nodes) => {
        setCommitTree(nodes);
        setSelectedTreeFile((prev) =>
          prev ? findTreeNodeByPath(nodes, prev.path) : null
        );
      })
      .catch((error) => console.error("Failed to load commit tree:", error))
      .finally(() => setTreeLoading(false));
  }, [commitViewMode, selectedCommit?.id, repoPath]);

  const handleJumpToCommit = useCallback((commitId: string) => {
    setJumpTarget((prev) => ({ id: commitId, nonce: (prev?.nonce ?? 0) + 1 }));
  }, []);

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
    // console.log("Scroll position:", target.scrollTop);
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
                className={`section-item ${currentView === "stage" ? "selected" : ""}`}
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
                    selectedBranch={
                      currentView === "stage" ? null : selectedBranch
                    }
                    onSelectBranch={(branch) => {
                      setSelectedBranch(branch);
                      setCurrentView(null);
                    }}
                    onCheckoutBranch={handleCheckoutBranch}
                    onCreateBranch={handleCreateBranch}
                    onCreateTag={handleCreateTag}
                    onFetch={handleFetch}
                    onPull={handlePull}
                    onPush={handlePush}
                    onDeleteBranch={handleDeleteBranch}
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
                            selectedBranch={
                              currentView === "stage" ? null : selectedBranch
                            }
                            onSelectBranch={(branch) => {
                              setSelectedBranch(`${remote.name}/${branch}`);
                              setCurrentView(null);
                            }}
                            onDeleteBranch={handleDeleteBranch}
                            remoteName={remote.name}
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
                      onContextMenu={(e) => {
                        e.preventDefault();
                        setTagContextMenu({ x: e.clientX, y: e.clientY, tag });
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
            <div className="sidebar-section collapsible">
              <div
                className="section-header"
                onClick={() => toggleSection("stashes")}
              >
                <span>Stashes</span>
                <span className="collapse-icon">
                  {collapsed.stashes ? "▶" : "▼"}
                </span>
              </div>
              {!collapsed.stashes && (
                <div className="section-body">
                  {stashes.map((stash) => (
                    <div
                      key={stash.index}
                      className="branch-item"
                      style={{ paddingLeft: "16px" }}
                      onContextMenu={(e) => {
                        e.preventDefault();
                        setStashContextMenu({
                          x: e.clientX,
                          y: e.clientY,
                          index: stash.index,
                        });
                      }}
                    >
                      <span
                        className="folder-icon"
                        style={{ visibility: "hidden" }}
                      >
                        ▼
                      </span>
                      <span className="item-icon">📥</span>
                      <span className="stash-message">{stash.message}</span>
                      <span className="stash-date">
                        {formatDate(stash.timestamp)}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
        <div className="main-content">
          {currentView === "stage" ? (
            <StageView
              repoPath={repoPath}
              onStash={() => setStashDialogOpen(true)}
            />
          ) : selectedBranch ? (
            <Group orientation="vertical">
              <Panel defaultSize={60} minSize={20}>
                <div className="history-panel">
                  <CommitHistory
                    repoPath={repoPath}
                    onCommitSelect={handleCommitSelect}
                    currentBranch={selectedBranch}
                    headBranchName={branches.find((b) => b.is_head)?.name}
                    jumpTarget={jumpTarget}
                    commitViewMode={commitViewMode}
                    onCommitViewModeChange={setCommitViewMode}
                    onCreateBranch={handleCreateBranch}
                    onCreateTag={handleCreateTag}
                    onDeleteTag={handleDeleteTag}
                    onCherryPick={handleCherryPick}
                    onApplyStash={() => handleApplyStash(0)}
                    onPopStash={() => handlePopStash(0)}
                    onDropStash={() =>
                      handleDropStash(
                        0,
                        stashes.find((s) => s.index === 0)?.message ?? ""
                      )
                    }
                  />
                </div>
              </Panel>
              <Separator className="resize-handle-horizontal" />
              <Panel defaultSize={40} minSize={20}>
                {commitViewMode === "tree" && selectedCommit ? (
                  <Group orientation="horizontal">
                    <Panel defaultSize={30} minSize={15}>
                      {treeLoading ? (
                        <div className="file-viewer-status">
                          Loading tree...
                        </div>
                      ) : (
                        <CommitTreeView
                          nodes={commitTree}
                          selectedPath={selectedTreeFile?.path ?? null}
                          onSelectFile={setSelectedTreeFile}
                        />
                      )}
                    </Panel>
                    <Separator className="resize-handle-vertical" />
                    <Panel defaultSize={70} minSize={30}>
                      <FileViewer
                        key={`${selectedTreeFile?.path ?? "none"}:${selectedCommit.id}`}
                        repoPath={repoPath}
                        commitId={selectedCommit.id}
                        file={selectedTreeFile}
                        onJumpToCommit={handleJumpToCommit}
                      />
                    </Panel>
                  </Group>
                ) : (
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
                                selectedCommit.branches.length > 0 &&
                                selectedCommit.branches.map((branch, index) => (
                                  <span
                                    key={index}
                                    className={
                                      branch.is_head
                                        ? "branch-badge-local-head"
                                        : branch.is_remote
                                          ? "branch-badge-remote"
                                          : "branch-badge-local"
                                    }
                                  >
                                    {branch.name}
                                  </span>
                                ))}
                              {selectedCommit.tags &&
                                selectedCommit.tags.length > 0 &&
                                selectedCommit.tags.map((tag, index) => (
                                  <span key={index} className="tag-badge">
                                    {tag}
                                  </span>
                                ))}
                            </span>
                          </div>
                          <div className="detail-row">
                            <span className="detail-label">Parents</span>
                            <span className="detail-value">
                              {selectedCommit.parents.map((parent, i) => (
                                <span key={parent}>
                                  <span className="parent-sha">{parent}</span>
                                  {i < selectedCommit.parents.length - 1 &&
                                    ", "}
                                </span>
                              ))}
                            </span>
                          </div>
                          <div className="detail-row author-row">
                            <span className="detail-label">Author</span>
                            <div className="author-info">
                              <div className="avatar">
                                {selectedCommit.author.name
                                  .charAt(0)
                                  .toUpperCase()}
                              </div>
                              <div className="author-details">
                                <div className="author-name">
                                  {selectedCommit.author.name} &lt;
                                  {selectedCommit.author.email}
                                  &gt;
                                </div>
                                <div className="author-date">
                                  {formatDate(selectedCommit.author.timestamp)}
                                </div>
                                {selectedCommit.committer.name ===
                                  selectedCommit.author.name &&
                                  selectedCommit.committer.email ===
                                    selectedCommit.author.email &&
                                  selectedCommit.committer.timestamp !==
                                    selectedCommit.author.timestamp && (
                                    <div className="commit-date">
                                      {formatDate(
                                        selectedCommit.committer.timestamp
                                      )}{" "}
                                      (Commit date)
                                    </div>
                                  )}
                              </div>
                            </div>
                          </div>
                          {(selectedCommit.committer.name !==
                            selectedCommit.author.name ||
                            selectedCommit.committer.email !==
                              selectedCommit.author.email) && (
                            <div className="detail-row author-row">
                              <span className="detail-label">Committer</span>
                              <div className="author-info">
                                <div className="avatar">
                                  {selectedCommit.committer.name
                                    .charAt(0)
                                    .toUpperCase()}
                                </div>
                                <div className="author-details">
                                  <div className="author-name">
                                    {selectedCommit.committer.name} &lt;
                                    {selectedCommit.committer.email}
                                    &gt;
                                  </div>
                                  <div className="author-date">
                                    {formatDate(
                                      selectedCommit.committer.timestamp
                                    )}
                                  </div>
                                </div>
                              </div>
                            </div>
                          )}
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
                                    if (file.status.toLowerCase() === "added")
                                      return "🟢";
                                    if (file.status.toLowerCase() === "deleted")
                                      return "🔴";
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
                                          onClick={() =>
                                            toggleFileCollapse(idx)
                                          }
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
                )}
              </Panel>
            </Group>
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
      {deleteBranchTarget && (
        <DeleteBranchDialog
          branchName={deleteBranchTarget.branch}
          info={deleteBranchTarget.info}
          onConfirm={handleConfirmDeleteBranch}
          onCancel={() => setDeleteBranchTarget(null)}
        />
      )}
      {createBranchFrom && (
        <CreateBranchDialog
          fromBranch={createBranchFrom}
          onConfirm={handleConfirmCreateBranch}
          onCancel={() => setCreateBranchFrom(null)}
        />
      )}
      {createTagFrom && (
        <CreateTagDialog
          fromRef={createTagFrom}
          onConfirm={handleConfirmCreateTag}
          onCancel={() => setCreateTagFrom(null)}
        />
      )}
      {stashDialogOpen && (
        <StashDialog
          onConfirm={handleSaveStash}
          onCancel={() => setStashDialogOpen(false)}
        />
      )}
      {stashDiffTarget && (
        <StashDiffDialog
          repoPath={repoPath}
          oid={stashDiffTarget.oid}
          message={stashDiffTarget.message}
          onClose={() => setStashDiffTarget(null)}
        />
      )}
      {tagContextMenu && (
        <div
          className="context-menu"
          style={{ left: tagContextMenu.x, top: tagContextMenu.y }}
        >
          <div
            className="context-menu-item"
            onClick={() => {
              handleDeleteTag(tagContextMenu.tag);
              setTagContextMenu(null);
            }}
          >
            Delete Tag...
          </div>
        </div>
      )}
      {stashContextMenu && (
        <div
          className="context-menu"
          style={{ left: stashContextMenu.x, top: stashContextMenu.y }}
        >
          <div
            className="context-menu-item"
            onClick={() => {
              handlePopStash(stashContextMenu.index);
              setStashContextMenu(null);
            }}
          >
            Pop Stash
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              handleApplyStash(stashContextMenu.index);
              setStashContextMenu(null);
            }}
          >
            Apply Stash
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              const stash = stashes.find(
                (s) => s.index === stashContextMenu.index
              );
              if (stash)
                setStashDiffTarget({ oid: stash.oid, message: stash.message });
              setStashContextMenu(null);
            }}
          >
            View Diff
          </div>
          <div className="context-menu-separator" />
          <div
            className="context-menu-item"
            onClick={() => {
              const stash = stashes.find(
                (s) => s.index === stashContextMenu.index
              );
              handleDropStash(stashContextMenu.index, stash?.message ?? "");
              setStashContextMenu(null);
            }}
          >
            Drop Stash...
          </div>
        </div>
      )}
    </div>
  );
}

export default RepoView;
