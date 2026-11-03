//! Regression tests that check the app's git operations against the `git`
//! command line on real repositories.

use super::*;
use git2::Repository;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::process::Command;

thread_local! { static TICK: Cell<u32> = const { Cell::new(0) }; }

/// Runs `git` with a fixed identity and strictly increasing commit dates, and
/// returns its combined output.
fn git(dir: &Path, args: &[&str]) -> String {
    let n = TICK.with(|t| {
        t.set(t.get() + 1);
        t.get()
    });
    let date = format!(
        "2020-01-01T{:02}:{:02}:{:02}Z",
        n / 3600,
        n / 60 % 60,
        n % 60
    );
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Alice")
        .env("GIT_AUTHOR_EMAIL", "a@x")
        .env("GIT_COMMITTER_NAME", "Alice")
        .env("GIT_COMMITTER_EMAIL", "a@x")
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .trim()
    .to_string()
}

/// A fresh repo whose app-side identity (git config) is Bob, so commits made
/// through the app are distinguishable from the "Alice" ones made here.
struct TestRepo {
    dir: PathBuf,
}

impl TestRepo {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("gitx-ops-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["config", "user.name", "Bob"]);
        git(&dir, &["config", "user.email", "b@x"]);
        TestRepo { dir }
    }

    fn write(&self, path: &str, content: &str) {
        let full = self.dir.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }

    fn git(&self, args: &[&str]) -> String {
        git(&self.dir, args)
    }

    fn commit_file(&self, path: &str, content: &str, message: &str) {
        self.write(path, content);
        self.git(&["add", "--", path]);
        self.git(&["commit", "-qm", message]);
    }

    fn repo(&self) -> Repository {
        Repository::open(&self.dir).unwrap()
    }

    /// (parent count, trimmed message, author name) of HEAD.
    fn head(&self) -> (usize, String, String) {
        let repo = self.repo();
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        let info = (
            commit.parent_count(),
            commit.message().unwrap().trim().to_string(),
            commit.author().name().unwrap().to_string(),
        );
        info
    }

    fn status(&self) -> Vec<(String, String, bool)> {
        let mut v: Vec<_> = get_status(&self.repo())
            .unwrap()
            .into_iter()
            .map(|f| (f.path, f.status, f.staged))
            .collect();
        v.sort();
        v
    }
}

impl Drop for TestRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn entry(path: &str, status: &str, staged: bool) -> (String, String, bool) {
    (path.to_string(), status.to_string(), staged)
}

fn lines(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

/// Splits a unified diff into (header, hunk header, hunk lines) for its first
/// hunk, the way the diff view does (on '\n' only, keeping any '\r').
fn first_hunk(diff: &str) -> (String, String, String) {
    let all: Vec<&str> = diff.split('\n').collect();
    let at = all.iter().position(|l| l.starts_with("@@")).unwrap();
    let end = all[at + 1..]
        .iter()
        .position(|l| l.starts_with("@@"))
        .map(|i| at + 1 + i)
        .unwrap_or(all.len());
    (
        diff.to_string(),
        all[at].to_string(),
        all[at + 1..end].join("\n"),
    )
}

// ---- status ---------------------------------------------------------------

#[test]
fn each_half_of_a_partly_staged_file_is_labelled_on_its_own() {
    let t = TestRepo::new("labels");
    t.commit_file("a.txt", "1\n", "c1");
    // staged modification, then deleted in the working tree  (git: "MD")
    t.write("a.txt", "2\n");
    t.git(&["add", "a.txt"]);
    std::fs::remove_file(t.dir.join("a.txt")).unwrap();
    // new file staged, then modified  (git: "AM")
    t.write("n.txt", "1\n");
    t.git(&["add", "n.txt"]);
    t.write("n.txt", "2\n");

    assert_eq!(
        t.status(),
        vec![
            entry("a.txt", "deleted", false),
            entry("a.txt", "modified", true),
            entry("n.txt", "added", true),
            entry("n.txt", "modified", false),
        ]
    );
}

#[test]
fn conflicted_files_are_listed() {
    let t = TestRepo::new("conflict");
    t.commit_file("a.txt", "base\n", "base");
    t.git(&["checkout", "-qb", "feature"]);
    t.commit_file("a.txt", "feature\n", "feature change");
    t.git(&["checkout", "-q", "main"]);
    t.commit_file("a.txt", "main\n", "main change");
    t.git(&["merge", "feature"]);

    assert_eq!(t.status(), vec![entry("a.txt", "conflicted", false)]);
}

#[test]
fn a_file_replaced_by_a_symlink_is_listed() {
    let t = TestRepo::new("typechange");
    t.commit_file("b.txt", "x\n", "c1");
    std::fs::remove_file(t.dir.join("b.txt")).unwrap();
    std::os::unix::fs::symlink("elsewhere", t.dir.join("b.txt")).unwrap();

    assert_eq!(t.status(), vec![entry("b.txt", "typechange", false)]);
}

// ---- staging --------------------------------------------------------------

#[test]
fn deleted_files_can_be_staged_and_unstaged() {
    let t = TestRepo::new("deleted");
    t.commit_file("a.txt", "x\n", "c1");
    std::fs::remove_file(t.dir.join("a.txt")).unwrap();
    assert_eq!(t.status(), vec![entry("a.txt", "deleted", false)]);

    stage_file(&t.repo(), "a.txt").unwrap();
    assert_eq!(t.status(), vec![entry("a.txt", "deleted", true)]);

    unstage_file(&t.repo(), "a.txt").unwrap();
    assert_eq!(t.status(), vec![entry("a.txt", "deleted", false)]);
}

#[test]
fn a_repo_with_no_commits_can_stage_diff_unstage_and_commit() {
    let t = TestRepo::new("unborn");
    t.write("a.txt", "x\n");
    assert!(get_branches(&t.repo()).unwrap().is_empty());

    stage_file(&t.repo(), "a.txt").unwrap();
    let diff = get_diff(&t.repo(), "a.txt", true).unwrap();
    assert!(diff.contains("+x"), "{diff}");

    unstage_file(&t.repo(), "a.txt").unwrap();
    assert_eq!(t.status(), vec![entry("a.txt", "added", false)]);

    stage_file(&t.repo(), "a.txt").unwrap();
    create_commit(&t.repo(), "root", false).unwrap();
    assert_eq!(t.head().0, 0);
    assert_eq!(get_branches(&t.repo()).unwrap().len(), 1);
}

#[test]
fn file_names_are_not_treated_as_patterns() {
    let t = TestRepo::new("glob");
    t.commit_file("a[1].txt", "1\n", "c1");
    t.commit_file("a1.txt", "1\n", "c2");
    t.write("a[1].txt", "2\n");
    t.write("a1.txt", "2\n");
    t.git(&["add", "--", "a[1].txt", "a1.txt"]);

    unstage_file(&t.repo(), "a[1].txt").unwrap();
    // Only the file asked for moves; `a[1].txt` is not a pattern matching `a1.txt`.
    assert_eq!(
        t.status(),
        vec![
            entry("a1.txt", "modified", true),
            entry("a[1].txt", "modified", false),
        ]
    );
}

// ---- hunks ----------------------------------------------------------------

#[test]
fn a_stale_hunk_is_rejected_instead_of_staging_old_content() {
    let t = TestRepo::new("stale");
    t.commit_file("a.txt", &lines(30), "c1");
    t.write("a.txt", &lines(30).replace("line 20\n", "line 20 EDIT-1\n"));
    let (full, header, body) = first_hunk(&get_diff(&t.repo(), "a.txt", false).unwrap());

    // The file changes again before the stale diff is acted on.
    t.write("a.txt", &lines(30).replace("line 20\n", "line 20 EDIT-2\n"));

    let err = stage_hunk(&t.repo(), "a.txt", &full, &header, &body).unwrap_err();
    assert!(err.message().contains("changed since"), "{err}");
    assert_eq!(t.git(&["diff", "--cached"]), "", "nothing may be staged");

    let err = discard_hunk(&t.repo(), "a.txt", &full, &header, &body).unwrap_err();
    assert!(err.message().contains("changed since"), "{err}");
    assert!(std::fs::read_to_string(t.dir.join("a.txt"))
        .unwrap()
        .contains("EDIT-2"));
}

#[test]
fn a_current_hunk_stages_unstages_and_discards() {
    let t = TestRepo::new("hunks");
    t.commit_file("a.txt", &lines(30), "c1");
    t.write("a.txt", &lines(30).replace("line 20\n", "line 20 EDIT\n"));

    let (full, header, body) = first_hunk(&get_diff(&t.repo(), "a.txt", false).unwrap());
    stage_hunk(&t.repo(), "a.txt", &full, &header, &body).unwrap();
    assert!(t.git(&["diff", "--cached"]).contains("+line 20 EDIT"));

    let (full, header, body) = first_hunk(&get_diff(&t.repo(), "a.txt", true).unwrap());
    unstage_hunk(&t.repo(), "a.txt", &full, &header, &body).unwrap();
    assert_eq!(t.git(&["diff", "--cached"]), "");

    let (full, header, body) = first_hunk(&get_diff(&t.repo(), "a.txt", false).unwrap());
    discard_hunk(&t.repo(), "a.txt", &full, &header, &body).unwrap();
    assert_eq!(
        std::fs::read_to_string(t.dir.join("a.txt")).unwrap(),
        lines(30)
    );
}

#[test]
fn hunks_of_untracked_files_can_be_staged() {
    let t = TestRepo::new("untracked");
    t.commit_file("keep.txt", "k\n", "c1");
    t.write("new.txt", "one\ntwo\r\nthree");

    let diff = get_diff(&t.repo(), "new.txt", false).unwrap();
    assert!(diff.contains("new file mode 100644"), "{diff}");
    let (full, header, body) = first_hunk(&diff);
    stage_hunk(&t.repo(), "new.txt", &full, &header, &body).unwrap();

    assert_eq!(t.status(), vec![entry("new.txt", "added", true)]);
    // Bytes survive exactly: CRLF kept, missing final newline kept.
    let staged = t.git(&["show", ":new.txt"]);
    assert_eq!(staged.replace('\r', "\\r"), "one\ntwo\\r\nthree");
}

#[test]
fn non_utf8_diff_lines_are_not_blanked() {
    let t = TestRepo::new("latin1");
    std::fs::write(t.dir.join("a.txt"), b"caf\xe9\n").unwrap();
    t.git(&["add", "."]);
    t.git(&["commit", "-qm", "c1"]);
    std::fs::write(t.dir.join("a.txt"), b"caf\xe9 au lait\n").unwrap();

    let diff = get_diff(&t.repo(), "a.txt", false).unwrap();
    assert!(diff.contains("-caf\u{fffd}\n"), "{diff:?}");
    assert!(diff.contains("+caf\u{fffd} au lait"), "{diff:?}");
}

// ---- ignore ---------------------------------------------------------------

#[test]
fn ignoring_anchors_escapes_dedupes_and_refuses_tracked_files() {
    let t = TestRepo::new("ignore");
    t.commit_file("tracked.txt", "t\n", "c1");
    t.write("a[1].txt", "x\n");
    t.write("sub/#odd.txt", "x\n");

    ignore_file(&t.repo(), "a[1].txt").unwrap();
    ignore_file(&t.repo(), "a[1].txt").unwrap();
    ignore_file(&t.repo(), "sub/#odd.txt").unwrap();
    let gitignore = std::fs::read_to_string(t.dir.join(".gitignore")).unwrap();
    assert_eq!(gitignore, "/a\\[1].txt\n/sub/#odd.txt\n");
    // git agrees both files are now ignored.
    assert!(t.git(&["status", "--short"]).contains(".gitignore"));
    assert!(!t.git(&["status", "--short"]).contains("odd"));
    assert!(!t.git(&["status", "--short"]).contains("a[1]"));

    let err = ignore_file(&t.repo(), "tracked.txt").unwrap_err();
    assert!(err.message().contains("already tracked"), "{err}");
}

// ---- commits --------------------------------------------------------------

#[test]
fn committing_during_a_merge_creates_a_real_merge_commit() {
    let t = TestRepo::new("merge");
    t.commit_file("a.txt", "base\n", "base");
    t.git(&["checkout", "-qb", "feature"]);
    t.commit_file("a.txt", "feature\n", "feature change");
    t.git(&["checkout", "-q", "main"]);
    t.commit_file("a.txt", "main\n", "main change");
    t.git(&["merge", "feature"]);
    t.write("a.txt", "resolved\n");
    t.git(&["add", "a.txt"]);

    create_commit(&t.repo(), "Merge feature", false).unwrap();

    assert_eq!(t.head().0, 2);
    assert!(!t.dir.join(".git/MERGE_HEAD").exists());
    assert_eq!(t.repo().state(), git2::RepositoryState::Clean);
}

#[test]
fn amending_keeps_the_author_and_can_reword_without_staged_changes() {
    let t = TestRepo::new("amend");
    t.commit_file("a.txt", "1\n", "by alice");
    assert_eq!(get_head_message(&t.repo()).unwrap().trim(), "by alice");

    create_commit(&t.repo(), "reworded", true).unwrap();

    let (parents, message, author) = t.head();
    assert_eq!(message, "reworded");
    assert_eq!(author, "Alice");
    assert_eq!(parents, 0, "amend replaces, not adds, a commit");
}

#[test]
fn an_empty_commit_is_refused() {
    let t = TestRepo::new("empty");
    t.commit_file("a.txt", "1\n", "c1");
    let err = create_commit(&t.repo(), "nothing here", false).unwrap_err();
    assert!(err.message().contains("nothing to commit"), "{err}");
}

#[cfg(unix)]
#[test]
fn commit_hooks_run_and_can_reject_the_commit() {
    use std::os::unix::fs::PermissionsExt;
    let t = TestRepo::new("hooks");
    t.commit_file("a.txt", "1\n", "c1");
    t.write("a.txt", "2\n");
    t.git(&["add", "a.txt"]);

    let hook = t.dir.join(".git/hooks/pre-commit");
    std::fs::write(&hook, "#!/bin/sh\necho 'lint failed' >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();

    let err = create_commit(&t.repo(), "should be rejected", false).unwrap_err();
    assert!(err.message().contains("lint failed"), "{err}");
    assert_eq!(t.head().1, "c1", "no commit was made");
}

// ---- history --------------------------------------------------------------

#[test]
fn history_includes_detached_head_and_commits_only_a_tag_reaches() {
    let t = TestRepo::new("history");
    t.commit_file("a.txt", "1\n", "c1");
    t.git(&["checkout", "-q", "--detach"]);
    t.git(&["commit", "-q", "--allow-empty", "-m", "detached work"]);
    let detached = t.git(&["rev-parse", "HEAD"]);
    t.git(&["tag", "only-tag"]);
    t.git(&["checkout", "-q", "main"]); // now nothing but the tag reaches it

    let all: Vec<String> = get_commits(&t.repo(), 100, false, None)
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert!(all.contains(&detached), "tag-only commit missing from All");

    t.git(&["checkout", "-q", &detached]);
    let local: Vec<String> = get_commits(&t.repo(), 100, true, None)
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert!(
        local.contains(&detached),
        "detached HEAD missing from Local"
    );
}

#[test]
fn renames_show_as_renames() {
    let t = TestRepo::new("rename");
    t.commit_file("a.txt", &lines(40), "c1");
    t.git(&["mv", "a.txt", "b.txt"]);
    t.git(&["commit", "-qm", "rename"]);
    let head = t.git(&["rev-parse", "HEAD"]);

    let files = get_commit_diff(&t.repo(), &head).unwrap();
    assert_eq!(files.len(), 1, "one rename, not a delete plus an add");
    assert_eq!(files[0].status, "renamed");
    assert_eq!(files[0].path, "b.txt");
    assert_eq!(files[0].old_path.as_deref(), Some("a.txt"));
}

#[test]
fn file_history_skips_merges_that_leave_the_file_unchanged() {
    let t = TestRepo::new("filehistory");
    t.commit_file("a.txt", "1\n", "c1 add a");
    t.git(&["checkout", "-qb", "feature"]);
    t.commit_file("other.txt", "o\n", "feature adds other");
    t.git(&["checkout", "-q", "main"]);
    t.commit_file("a.txt", "2\n", "c2 edit a");
    t.git(&["merge", "--no-ff", "-qm", "merge feature", "feature"]);
    let head = t.git(&["rev-parse", "HEAD"]);

    let history: Vec<String> = get_file_history(&t.repo(), &head, "a.txt")
        .unwrap()
        .into_iter()
        .map(|e| e.message.trim().to_string())
        .collect();
    assert_eq!(history, vec!["c2 edit a", "c1 add a"]);
    // Same answer as git itself.
    assert_eq!(
        t.git(&["log", "--format=%s", "--", "a.txt"]),
        history.join("\n")
    );
}

#[test]
fn the_tree_lists_submodules() {
    let t = TestRepo::new("submodule");
    t.commit_file("a.txt", "1\n", "c1");
    let sub = TestRepo::new("submodule-lib");
    sub.commit_file("s.txt", "s\n", "sub c1");
    t.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "-q",
        sub.dir.to_str().unwrap(),
        "modsub",
    ]);
    t.git(&["commit", "-qm", "add submodule"]);
    let head = t.git(&["rev-parse", "HEAD"]);

    let tree = get_commit_tree(&t.repo(), &head).unwrap();
    let node = tree.iter().find(|n| n.name == "modsub").expect("modsub");
    assert!(node.is_submodule && !node.is_dir);
    assert!(
        !tree
            .iter()
            .find(|n| n.name == "a.txt")
            .unwrap()
            .is_submodule
    );
}

// ---- branches -------------------------------------------------------------

#[test]
fn a_branch_created_from_a_remote_branch_tracks_it() {
    let origin = TestRepo::new("track-origin");
    origin.commit_file("a.txt", "1\n", "c1");
    let clone_dir =
        std::env::temp_dir().join(format!("gitx-ops-track-clone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&clone_dir);
    git(
        &std::env::temp_dir(),
        &[
            "clone",
            "-q",
            origin.dir.to_str().unwrap(),
            clone_dir.to_str().unwrap(),
        ],
    );
    let repo = Repository::open(&clone_dir).unwrap();
    let remote_name = repo
        .branches(Some(git2::BranchType::Remote))
        .unwrap()
        .filter_map(|b| b.ok())
        .filter_map(|(b, _)| b.name().ok().flatten().map(str::to_string))
        .find(|n| n.starts_with("origin/") && !n.ends_with("/HEAD"))
        .unwrap();

    create_branch(&repo, "mine", &remote_name).unwrap();
    let upstream = repo
        .find_branch("mine", git2::BranchType::Local)
        .unwrap()
        .upstream()
        .unwrap();
    assert_eq!(upstream.name().unwrap(), Some(remote_name.as_str()));

    // A branch started from a local branch gets no upstream.
    create_branch(&repo, "plain", "mine").unwrap();
    assert!(repo
        .find_branch("plain", git2::BranchType::Local)
        .unwrap()
        .upstream()
        .is_err());
    let _ = std::fs::remove_dir_all(&clone_dir);
}

#[test]
fn checking_out_a_branch_works_when_a_folder_has_the_same_name() {
    let origin = TestRepo::new("dd-origin");
    origin.commit_file("a.txt", "1\n", "c1");
    origin.git(&["checkout", "-qb", "feature"]);
    origin.commit_file("feature/f.txt", "f\n", "feature work");
    origin.git(&["checkout", "-q", "main"]);

    let clone_dir = std::env::temp_dir().join(format!("gitx-ops-dd-clone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&clone_dir);
    git(
        &std::env::temp_dir(),
        &[
            "clone",
            "-q",
            origin.dir.to_str().unwrap(),
            clone_dir.to_str().unwrap(),
        ],
    );
    // A folder called `feature` exists, and `feature` is only a remote branch.
    std::fs::create_dir_all(clone_dir.join("feature")).unwrap();
    std::fs::write(clone_dir.join("feature/untracked.txt"), "keep me\n").unwrap();

    let repo = Repository::open(&clone_dir).unwrap();
    checkout_branch(&repo, "feature").unwrap();

    let head = repo.head().unwrap();
    assert_eq!(head.shorthand().unwrap(), "feature");
    assert!(clone_dir.join("feature/untracked.txt").exists());
    let _ = std::fs::remove_dir_all(&clone_dir);
}

#[test]
fn a_conflicted_file_shows_its_conflict_markers() {
    let t = TestRepo::new("conflictdiff");
    t.commit_file("a.txt", "base\n", "base");
    t.git(&["checkout", "-qb", "feature"]);
    t.commit_file("a.txt", "feature\n", "feature change");
    t.git(&["checkout", "-q", "main"]);
    t.commit_file("a.txt", "main\n", "main change");
    t.git(&["merge", "feature"]);

    let diff = get_diff(&t.repo(), "a.txt", false).unwrap();
    assert!(diff.contains("+<<<<<<< HEAD"), "{diff}");
    assert!(diff.contains("+>>>>>>> feature"), "{diff}");
}
