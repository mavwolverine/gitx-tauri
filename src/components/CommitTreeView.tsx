import { useState } from "react";
import "./CommitTreeView.css";

export interface TreeNode {
  name: string;
  path: string;
  is_dir: boolean;
  oid: string;
  children?: TreeNode[] | null;
}

interface CommitTreeViewProps {
  nodes: TreeNode[];
  selectedPath: string | null;
  onSelectFile: (node: TreeNode) => void;
}

interface TreeRowProps {
  node: TreeNode;
  depth: number;
  selectedPath: string | null;
  onSelectFile: (node: TreeNode) => void;
}

function TreeRow({ node, depth, selectedPath, onSelectFile }: TreeRowProps) {
  const [expanded, setExpanded] = useState(false);
  const indent = 16 + depth * 16;

  if (node.is_dir) {
    return (
      <div>
        <div
          className="branch-folder"
          style={{ paddingLeft: `${indent}px` }}
          onClick={() => setExpanded((prev) => !prev)}
        >
          <span className="folder-icon">{expanded ? "▼" : "▶"}</span>
          <span className="item-icon">📁</span>
          {node.name}
        </div>
        {expanded &&
          node.children?.map((child) => (
            <TreeRow
              key={child.path}
              node={child}
              depth={depth + 1}
              selectedPath={selectedPath}
              onSelectFile={onSelectFile}
            />
          ))}
      </div>
    );
  }

  return (
    <div
      className={`branch-item ${selectedPath === node.path ? "selected" : ""}`}
      style={{ paddingLeft: `${indent}px` }}
      onClick={() => onSelectFile(node)}
    >
      <span className="folder-icon" style={{ visibility: "hidden" }}>
        ▼
      </span>
      <span className="item-icon">📄</span>
      {node.name}
    </div>
  );
}

export function CommitTreeView({
  nodes,
  selectedPath,
  onSelectFile,
}: CommitTreeViewProps) {
  return (
    <div className="commit-tree-view">
      {nodes.map((node) => (
        <TreeRow
          key={node.path}
          node={node}
          depth={0}
          selectedPath={selectedPath}
          onSelectFile={onSelectFile}
        />
      ))}
    </div>
  );
}
