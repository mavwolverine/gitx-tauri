import React, { useState, useEffect } from "react";

interface GitBranch {
  name: string;
  is_head: boolean;
}

interface BranchTree {
  [key: string]: string[] | BranchTree;
}

interface BranchTreeProps {
  branches: GitBranch[];
  selectedBranch: string | null;
  onSelectBranch: (branch: string) => void;
  onCheckoutBranch?: (branch: string) => void;
  onCreateBranch?: (fromBranch: string) => void;
  onCreateTag?: (fromBranch: string) => void;
  onFetch?: (branch: string, remote: string) => void;
  onPull?: (branch: string, remote: string) => void;
  level?: number;
  prefix?: string;
  remotes?: Array<{ name: string }>;
}

export function BranchTree({
  branches,
  selectedBranch,
  onSelectBranch,
  onCheckoutBranch,
  onCreateBranch,
  onCreateTag,
  onFetch,
  onPull,
  level = 0,
  prefix = "",
  remotes = [],
}: BranchTreeProps) {
  const [folderCollapsed, setFolderCollapsed] = useState<{
    [key: string]: boolean;
  }>({});
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    branch: string;
    isHead: boolean;
    remote: string | null;
  } | null>(null);

  useEffect(() => {
    const handleClick = () => setContextMenu(null);
    window.addEventListener("click", handleClick);
    return () => window.removeEventListener("click", handleClick);
  }, []);

  const getRemoteForBranch = (_branch: string): string | null => {
    // Default to 'origin' if it exists, otherwise first remote
    // TODO: Implement branch-specific remote lookup
    if (remotes.length === 0) return null;
    const hasOrigin = remotes.some((r) => r.name === "origin");
    return hasOrigin ? "origin" : remotes[0].name;
  };

  const buildBranchTree = (branches: GitBranch[]): BranchTree => {
    const tree: BranchTree = {};
    branches.forEach((branch) => {
      const parts = branch.name.split("/");
      if (parts.length === 1) {
        if (!tree._branches) tree._branches = [];
        (tree._branches as string[]).push(branch.name);
      } else {
        const [prefix, ...rest] = parts;
        if (!tree[prefix]) tree[prefix] = {};
        let current: Record<string, unknown> = tree[prefix] as Record<
          string,
          unknown
        >;
        for (let i = 0; i < rest.length - 1; i++) {
          if (!current[rest[i]]) current[rest[i]] = {};
          current = current[rest[i]] as Record<string, unknown>;
        }
        if (!current._branches) current._branches = [];
        (current._branches as string[]).push(branch.name);
      }
    });
    return tree;
  };

  const renderTree = (
    tree: BranchTree,
    level: number,
    prefix: string,
  ) => {
    const elements: React.ReactElement[] = [];
    const rootBranches = tree._branches as string[] | undefined;
    const folders = Object.keys(tree).filter((k) => k !== "_branches");

    // Combine branches and folders for sorting
    const items: Array<{ type: "branch" | "folder"; name: string }> = [];

    if (rootBranches) {
      rootBranches.forEach((name) => items.push({ type: "branch", name }));
    }
    folders.forEach((name) => items.push({ type: "folder", name }));

    // Sort all items together
    items.sort((a, b) => a.name.localeCompare(b.name));

    // Render sorted items
    items.forEach((item) => {
      if (item.type === "branch") {
        const branch = branches.find((b) => b.name === item.name);
        const isSelected = selectedBranch === item.name;
        elements.push(
          <div
            key={item.name}
            className={`branch-item ${isSelected ? "selected" : ""}`}
            style={{ paddingLeft: `${16 + level * 12}px` }}
            onClick={() => onSelectBranch(item.name)}
            onDoubleClick={() => {
              if (onCheckoutBranch && !branch?.is_head) {
                onCheckoutBranch(item.name);
              }
            }}
            onContextMenu={(e) => {
              e.preventDefault();
              setContextMenu({
                x: e.clientX,
                y: e.clientY,
                branch: item.name,
                isHead: branch?.is_head || false,
                remote: getRemoteForBranch(item.name),
              });
            }}
          >
            <span className="folder-icon" style={{ visibility: "hidden" }}>
              ▼
            </span>
            <span className="item-icon">⎇</span>
            {item.name.split("/").pop()}
            {branch?.is_head && <span className="current-branch">✓</span>}
          </div>
        );
      } else {
        const folderKey = prefix ? `${prefix}/${item.name}` : item.name;
        const isCollapsed = folderCollapsed[folderKey];

        elements.push(
          <div
            key={`${prefix}-${item.name}`}
            className="branch-folder"
            style={{ paddingLeft: `${16 + level * 12}px` }}
            onClick={() =>
              setFolderCollapsed((prev) => ({
                ...prev,
                [folderKey]: !prev[folderKey],
              }))
            }
          >
            <span className="folder-icon">{isCollapsed ? "▶" : "▼"}</span>
            📁 {item.name}
          </div>
        );

        if (!isCollapsed) {
          elements.push(
            ...renderTree(tree[item.name] as BranchTree, level + 1, folderKey)
          );
        }
      }
    });

    return elements;
  };

  return (
    <>
      {renderTree(buildBranchTree(branches), level, prefix)}
      {contextMenu && (
        <div
          className="context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          {onCheckoutBranch && (
            <div
              className={`context-menu-item ${contextMenu.isHead ? "disabled" : ""}`}
              onClick={() => {
                if (!contextMenu.isHead) {
                  onCheckoutBranch(contextMenu.branch);
                  setContextMenu(null);
                }
              }}
            >
              Checkout
            </div>
          )}
          {onCheckoutBranch && (onCreateBranch || onCreateTag) && (
            <div className="context-menu-separator" />
          )}
          {onCreateBranch && (
            <div
              className="context-menu-item"
              onClick={() => {
                onCreateBranch(contextMenu.branch);
                setContextMenu(null);
              }}
            >
              Create Branch...
            </div>
          )}
          {onCreateTag && (
            <div
              className="context-menu-item"
              onClick={() => {
                onCreateTag(contextMenu.branch);
                setContextMenu(null);
              }}
            >
              Create Tag...
            </div>
          )}
          {(onCreateBranch || onCreateTag) && (onFetch || onPull) && (
            <div className="context-menu-separator" />
          )}
          {onFetch && (
            <div
              className={`context-menu-item ${!contextMenu.remote ? "disabled" : ""}`}
              onClick={() => {
                if (contextMenu.remote) {
                  onFetch(contextMenu.branch, contextMenu.remote);
                  setContextMenu(null);
                }
              }}
            >
              {contextMenu.remote
                ? `Fetch ${contextMenu.remote}`
                : "Fetch (no remote)"}
            </div>
          )}
          {onPull && (
            <div
              className={`context-menu-item ${!contextMenu.isHead || !contextMenu.remote ? "disabled" : ""}`}
              onClick={() => {
                if (contextMenu.isHead && contextMenu.remote) {
                  onPull(contextMenu.branch, contextMenu.remote);
                  setContextMenu(null);
                }
              }}
            >
              {contextMenu.remote
                ? `Pull '${contextMenu.remote}' and update '${contextMenu.branch.split("/").pop()}'`
                : "Pull (no remote)"}
            </div>
          )}
        </div>
      )}
    </>
  );
}
