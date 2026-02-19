import { useState, useEffect } from "react";
import Launcher from "./Launcher";
import RepoView from "./RepoView";
import "./App.css";

function App() {
  const [repoPath, setRepoPath] = useState<string | null>(null);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const path = params.get("path");
    if (path) {
      setRepoPath(path);
    }
  }, []);

  if (repoPath) {
    return <RepoView repoPath={repoPath} />;
  }

  return <Launcher />;
}

export default App;
