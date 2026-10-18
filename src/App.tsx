import Launcher from "./Launcher";
import RepoView from "./RepoView";
import "./App.css";

// Repo windows are opened at `/?path=<repo>`. Read it synchronously so the
// first render is already the right view (no Launcher flash).
const repoPath = new URLSearchParams(window.location.search).get("path");

function App() {
  if (repoPath) {
    return <RepoView repoPath={repoPath} />;
  }

  return <Launcher />;
}

export default App;
