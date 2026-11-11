use serde::Serialize;
use serde_json::Value;
use std::{path::Path, process::Command};

#[derive(Debug, Serialize)]
pub struct GithubStatus {
    pub cli_installed: bool,
    pub login: Option<String>,
    pub vault_git_configured: bool,
}

pub fn status(vault_root: Option<&Path>) -> GithubStatus {
    let cli_installed = Command::new("gh")
        .arg("--version")
        .output()
        .is_ok_and(|result| result.status.success());
    let login = if cli_installed {
        Command::new("gh")
            .args(["auth", "status", "--active", "--json", "hosts"])
            .output()
            .ok()
            .filter(|result| result.status.success())
            .and_then(|result| active_login(&result.stdout))
    } else {
        None
    };
    let vault_git_configured = vault_root.is_some_and(helper_is_configured);
    GithubStatus {
        cli_installed,
        login,
        vault_git_configured,
    }
}

fn active_login(json: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(json).ok()?;
    value
        .get("hosts")?
        .get("github.com")?
        .as_array()?
        .iter()
        .find(|account| account.get("active").and_then(Value::as_bool) == Some(true))?
        .get("login")?
        .as_str()
        .filter(|login| !login.is_empty())
        .map(str::to_owned)
}

pub fn start_login() -> Result<(), String> {
    if !Command::new("gh")
        .arg("--version")
        .output()
        .is_ok_and(|result| result.status.success())
    {
        return Err("Instale o GitHub CLI (gh) para conectar sua conta.".into());
    }
    Command::new("xdg-terminal-exec")
        .args([
            "--title=Passage - GitHub",
            "--",
            "gh",
            "auth",
            "login",
            "--hostname",
            "github.com",
            "--web",
            "--git-protocol",
            "https",
        ])
        .spawn()
        .map_err(|_| {
            "Não foi possível abrir o terminal. Execute gh auth login --web no terminal."
                .to_string()
        })?;
    Ok(())
}

pub fn configure_vault(root: &Path) -> Result<GithubStatus, String> {
    let current = status(Some(root));
    if current.login.is_none() {
        return Err("Conecte sua conta do GitHub antes de ativá-la neste cofre.".into());
    }
    let git = crate::git::GitVault::open(root)?;
    git.configure_github_helper()?;
    Ok(status(Some(root)))
}

fn helper_is_configured(root: &Path) -> bool {
    if crate::git::GitVault::open(root).is_err() {
        return false;
    }
    let output = Command::new("git")
        .args([
            "config",
            "--local",
            "--get-all",
            "credential.https://github.com.helper",
        ])
        .current_dir(root)
        .output();
    output.is_ok_and(|result| {
        result.status.success()
            && String::from_utf8_lossy(&result.stdout)
                .lines()
                .any(|line| line == "!gh auth git-credential")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_active_github_account() {
        let json = br#"{"hosts":{"github.com":[{"login":"other","active":false},{"login":"henrique","active":true}],"enterprise.example":[{"login":"work","active":true}]}}"#;
        assert_eq!(active_login(json).as_deref(), Some("henrique"));
        assert_eq!(active_login(br#"{"hosts":{"github.com":[]}}"#), None);
    }
}
