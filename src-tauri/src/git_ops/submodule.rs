use git2::Repository;
use serde::Serialize;

#[derive(Serialize)]
pub struct GitSubmodule {
    pub name: String,
    pub path: String,
    pub url: String,
}

pub fn get_submodules(repo: &Repository) -> Result<Vec<GitSubmodule>, git2::Error> {
    let mut submodules = Vec::new();

    for submodule in repo.submodules()? {
        if let (Some(name), Some(url)) = (submodule.name(), submodule.url()) {
            let path = submodule.path();
            submodules.push(GitSubmodule {
                name: name.to_string(),
                path: path.to_str().unwrap_or("").to_string(),
                url: url.to_string(),
            });
        }
    }

    Ok(submodules)
}
