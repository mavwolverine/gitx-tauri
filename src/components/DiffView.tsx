import "./DiffView.css";

interface DiffViewProps {
  diff: string;
  filename: string;
  onDiscard?: (hunkHeader?: string, hunkLines?: string) => void;
  onStage?: (hunkHeader?: string, hunkLines?: string) => void;
  showActions?: boolean;
  actionLabel?: string;
}

export function DiffView({
  diff,
  filename,
  onDiscard,
  onStage,
  showActions = true,
  actionLabel = "Stage",
}: DiffViewProps) {
  const renderDiff = () => {
    if (!diff) return null;

    const lines = diff.split("\n");
    let startIdx = 0;

    // Skip git diff header lines
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].startsWith("@@")) {
        startIdx = i;
        break;
      }
    }

    let oldLine = 0;
    let newLine = 0;

    return lines.slice(startIdx).map((line, idx) => {
      let className = "diff-line";
      const isHunk = line.startsWith("@@");

      if (isHunk) {
        className += " diff-hunk";
        const match = line.match(/@@ -(\d+),?\d* \+(\d+),?\d* @@/);
        if (match) {
          oldLine = parseInt(match[1]);
          newLine = parseInt(match[2]);
        }
      }

      let oldNum = "";
      let newNum = "";

      if (!isHunk) {
        if (line.startsWith("+")) {
          className += " diff-add";
          newNum = String(newLine++);
        } else if (line.startsWith("-")) {
          className += " diff-remove";
          oldNum = String(oldLine++);
        } else {
          oldNum = String(oldLine++);
          newNum = String(newLine++);
        }
      }

      // Capture the current hunk index for this specific hunk
      const thisHunkIdx = idx;
      const handleStageHunk = () => {
        if (onStage) {
          const hunkHeader = lines[startIdx + thisHunkIdx];
          // Find the next hunk starting from current position
          let nextHunkIdx = -1;
          for (let i = startIdx + thisHunkIdx + 1; i < lines.length; i++) {
            if (lines[i].startsWith("@@")) {
              nextHunkIdx = i;
              break;
            }
          }
          const endIdx = nextHunkIdx >= 0 ? nextHunkIdx : lines.length;
          const hunkLines = lines
            .slice(startIdx + thisHunkIdx + 1, endIdx)
            .join("\n");
          onStage(hunkHeader, hunkLines);
        }
      };

      const handleDiscardHunk = () => {
        if (onDiscard) {
          const hunkHeader = lines[startIdx + thisHunkIdx];
          let nextHunkIdx = -1;
          for (let i = startIdx + thisHunkIdx + 1; i < lines.length; i++) {
            if (lines[i].startsWith("@@")) {
              nextHunkIdx = i;
              break;
            }
          }
          const endIdx = nextHunkIdx >= 0 ? nextHunkIdx : lines.length;
          const hunkLines = lines
            .slice(startIdx + thisHunkIdx + 1, endIdx)
            .join("\n");
          onDiscard(hunkHeader, hunkLines);
        }
      };

      return (
        <div key={idx} className={isHunk ? "diff-hunk-wrapper" : undefined}>
          <div className={className}>
            {!isHunk && (
              <>
                <span className="line-num old-line-num">{oldNum}</span>
                <span className="line-num new-line-num">{newNum}</span>
              </>
            )}
            <span className="line-content">{line || " "}</span>
          </div>
          {isHunk && showActions && (
            <div className="diff-hunk-actions">
              {onDiscard && (
                <button className="diff-hunk-btn" onClick={handleDiscardHunk}>
                  Discard
                </button>
              )}
              {onStage && (
                <button className="diff-hunk-btn" onClick={handleStageHunk}>
                  {actionLabel}
                </button>
              )}
            </div>
          )}
        </div>
      );
    });
  };

  return (
    <div className="diff-panel">
      <div className="diff-panel-header">
        <span>{filename}</span>
      </div>
      <div className="diff-content">{renderDiff()}</div>
    </div>
  );
}
