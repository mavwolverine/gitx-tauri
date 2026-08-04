# Roadmap

Status of GitX-Tauri as a daily-driver Git GUI, and what's planned next.

## Done

- 🌳 Visual git graph with colored branch/merge lanes
- 📊 Commit history browsing (up to 10,000 commits), filtered by All / Local / Current branch
- 🔍 Unified diff viewer (line numbers, change highlighting — no syntax highlighting yet)
- 🌿 Branch management — create, delete, switch, view remote branches
- 🏷️ Tag support — create, delete, navigate to tagged commits
- 📝 Staging — stage/unstage whole files or individual hunks, discard changes
- ✅ Commit creation and amending
- ⬆️⬇️ Fetch / pull / push
- 📥 Clone repositories over SSH
- 🕵️ File blame and per-file history
- 🖥️ Cross-platform (macOS, Linux, Windows)
- 📦 Stash management — save (with optional untracked files), list, apply, pop, drop, and view a stash's diff; the most recent stash also shows as a node in the commit graph, same as `git log --graph`
- 🔎 Commit search — find commits by message, author, or SHA; Enter/Shift+Enter or the ‹/› buttons cycle through matches, jumping to and selecting each one in the graph
- 🍒 Cherry-pick — right-click a commit in the graph to cherry-pick it onto the actual checked-out branch (not whichever branch is just being viewed); auto-commits with the original message/author on success, surfaces a clear error and leaves the repo in `CherryPick` state for manual resolution on conflict; menu item is disabled and relabeled based on a dedicated ancestry check, not the loaded commit list

## Planned — critical gaps

These are the biggest blockers to using GitX-Tauri as a primary Git client.

- [ ] **Merge operations** — no merge UI exists; "merge" currently only appears in a graph-rendering comment. Plan: start with fast-forward / clean three-way merges (`git2::Repository::merge` + a two-parent commit), which is a small addition on top of the existing commit/checkout commands.
- [ ] **Conflict detection & resolution UI** — no support at all today. Scope after clean-merge support ships: surface conflicted files (libgit2 exposes these natively via repo state + index conflict entries) with a resolution view, rather than punting to the terminal.

## Planned — important

- [ ] **Rebase** — standard and interactive
- [ ] **Keyboard shortcuts** — currently only arrow-key navigation in the commit list; no global shortcuts (e.g. Cmd+S to commit, Cmd+F to search)
- [ ] **Undo last commit** — soft reset from the UI
- [ ] **"Diff with `<current branch>`"** — right-click a commit in the graph to diff it against the current branch's HEAD, not just against its own parent (today `get_commit_diff` only ever diffs a commit against `parent(0)`)

## Planned — polish

- [ ] Syntax highlighting in diffs
- [ ] Custom themes
- [ ] Performance optimizations for large repositories

## Reference: what GitX (the original) did

The classic macOS GitX shelled out to `git merge` / `git cherry-pick` directly, showed conflicted files with a generic "conflicted" status badge, and left actual conflict editing to the user's own editor — it explicitly refused to create merge commits from the GUI at all ("GitX cannot commit merges yet. Please commit your changes from the command line."). GitX-Tauri's libgit2-based approach can do better: complete a clean merge from the GUI, and (once conflict UI ships) resolve conflicts without leaving the app.
