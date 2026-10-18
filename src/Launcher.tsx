import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./Launcher.css";

interface CliStatus {
  supported: boolean;
  install_path: string | null;
  installed_at: string | null;
  outdated: boolean;
}

interface RecentRepo {
  path: string;
  name: string;
  lastOpened: string;
}

function Launcher() {
  const [recentRepos, setRecentRepos] = useState<RecentRepo[]>([]);
  const [cloneUrl, setCloneUrl] = useState("");
  const [clonePath, setClonePath] = useState("");
  const [isCloning, setIsCloning] = useState(false);
  const [cliStatus, setCliStatus] = useState<CliStatus | null>(null);
  const [cliMessage, setCliMessage] = useState<string | null>(null);
  const [isInstallingCli, setIsInstallingCli] = useState(false);

  useEffect(() => {
    loadRecentRepos();
    invoke<CliStatus>("cli_install_status")
      .then(setCliStatus)
      .catch((error) => console.error("Failed to get CLI status:", error));
  }, []);

  const installCli = async () => {
    setIsInstallingCli(true);
    try {
      setCliMessage(await invoke<string>("install_cli"));
      setCliStatus(await invoke<CliStatus>("cli_install_status"));
    } catch (error) {
      setCliMessage(`${error}`);
    } finally {
      setIsInstallingCli(false);
    }
  };

  const loadRecentRepos = () => {
    const stored = localStorage.getItem("gitx-tauri-recent-repos");
    if (stored) {
      setRecentRepos(JSON.parse(stored));
    }
  };

  const addToRecentRepos = (path: string) => {
    const name = path.split("/").pop() || path;
    const newRepo: RecentRepo = {
      path,
      name,
      lastOpened: new Date().toISOString(),
    };
    console.log("addToRecentRepos");
    console.log(name);

    const updated = [
      newRepo,
      ...recentRepos.filter((r) => r.path !== path),
    ].slice(0, 10);
    setRecentRepos(updated);
    localStorage.setItem("gitx-tauri-recent-repos", JSON.stringify(updated));
  };

  const openRepoWindow = async (repoPath: string) => {
    try {
      await invoke("open_repo_window", { repoPath });
    } catch (error) {
      console.error("Failed to open repository:", error);
    }
  };

  const selectRecentRepo = async (repo: RecentRepo) => {
    console.log("Selecting recent repo:", repo);
    addToRecentRepos(repo.path);
    await openRepoWindow(repo.path);
  };

  const deleteRecentRepo = (path: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const updated = recentRepos.filter((r) => r.path !== path);
    setRecentRepos(updated);
    localStorage.setItem("gitx-tauri-recent-repos", JSON.stringify(updated));
  };

  const openLocalRepo = async () => {
    console.log("Opening local repo...");
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });

      console.log("Selected:", selected);

      if (selected) {
        const path = Array.isArray(selected) ? selected[0] : selected;
        const isGitRepo = await invoke<boolean>("is_git_repository", { path });

        if (!isGitRepo) {
          alert(
            "Selected folder is not a Git repository. Please select a folder containing a .git directory."
          );
          return;
        }

        addToRecentRepos(path);
        await openRepoWindow(path);
      }
    } catch (error) {
      console.error("Failed to open repository:", error);
    }
  };

  const cloneRepository = async () => {
    if (!cloneUrl || !clonePath) return;

    setIsCloning(true);
    try {
      // Call Rust function to clone the repository
      const repoName = cloneUrl.split("/").pop()?.replace(".git", "") || "repo";
      const fullPath = `${clonePath}/${repoName}`;

      await invoke("clone_repository", { url: cloneUrl, path: fullPath });

      addToRecentRepos(fullPath);
      await openRepoWindow(fullPath);

      // Reset form
      setCloneUrl("");
      setClonePath("");
    } catch (error) {
      console.error("Failed to clone repository:", error);
      alert(`Failed to clone repository: ${error}`);
    } finally {
      setIsCloning(false);
    }
  };

  const selectClonePath = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });

      if (selected) {
        const path = Array.isArray(selected) ? selected[0] : selected;
        setClonePath(path);
      }
    } catch (error) {
      console.error("Failed to select clone path:", error);
    }
  };

  return (
    <div className="launcher">
      <div className="launcher-header">
        <h1>GitX-Tauri</h1>
        <p>Select or open a Git repository</p>
      </div>

      <div className="launcher-content">
        <div className="launcher-section">
          <h2>Recent Repositories</h2>
          {recentRepos.length > 0 ? (
            <div className="recent-repos">
              {recentRepos.map((repo) => (
                <div
                  key={repo.path}
                  className="recent-repo-item"
                  onClick={() => selectRecentRepo(repo)}
                  title={repo.path}
                >
                  <div className="repo-info">
                    <div className="repo-name">{repo.name}</div>
                    <div className="repo-path">{repo.path}</div>
                    <div className="repo-date">
                      {new Date(repo.lastOpened).toLocaleDateString()}
                    </div>
                  </div>
                  <button
                    className="delete-repo-btn"
                    onClick={(e) => deleteRecentRepo(repo.path, e)}
                    title="Remove from recent"
                  >
                    ✕
                  </button>
                </div>
              ))}
            </div>
          ) : (
            <div className="no-recent">No recent repositories</div>
          )}
        </div>

        <div className="launcher-actions">
          <button className="action-button primary" onClick={openLocalRepo}>
            <span className="button-icon">📁</span>
            Open Local Repository
          </button>

          <div className="clone-section">
            <h3>Clone Repository</h3>
            <div className="clone-form">
              <input
                type="text"
                placeholder="Repository URL (https://github.com/user/repo.git)"
                value={cloneUrl}
                onChange={(e) => setCloneUrl(e.target.value)}
                className="clone-input"
                spellCheck={false}
                autoCorrect="off"
                autoCapitalize="off"
                autoComplete="off"
              />
              <div className="clone-path-row">
                <input
                  type="text"
                  placeholder="Clone to..."
                  value={clonePath}
                  onChange={(e) => setClonePath(e.target.value)}
                  className="clone-input"
                  readOnly
                />
                <button onClick={selectClonePath} className="path-button">
                  Browse
                </button>
              </div>
              <button
                onClick={cloneRepository}
                disabled={!cloneUrl || !clonePath || isCloning}
                className="action-button"
              >
                {isCloning ? "Cloning..." : "Clone Repository"}
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* One-time setup, so keep it out of the way and hide it once done. */}
      {cliStatus?.supported &&
        (!cliStatus.installed_at || cliStatus.outdated || cliMessage) && (
          <div className="launcher-footer">
            {cliMessage && <div>{cliMessage}</div>}
            {(!cliStatus.installed_at || cliStatus.outdated) && (
              <button
                onClick={installCli}
                disabled={isInstallingCli}
                className="link-button"
                title={`Adds gitx-tauri to ${cliStatus.install_path}`}
              >
                {isInstallingCli
                  ? "Installing..."
                  : cliStatus.outdated
                    ? "Update command-line tool to this build"
                    : "Install command-line tool"}
              </button>
            )}
          </div>
        )}
    </div>
  );
}

export default Launcher;
