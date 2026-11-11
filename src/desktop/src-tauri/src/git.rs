use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug, Serialize)]
pub struct GitStatus {
    pub branch: String,
    pub upstream: Option<String>,
    pub dirty: bool,
    pub ahead: usize,
    pub behind: usize,
}

pub struct GitVault {
    root: PathBuf,
}

impl GitVault {
    pub fn configure_github_helper(&self) -> Result<(), String> {
        // An empty helper resets inherited helpers for this host only, so a
        // different global credential manager cannot answer first.
        self.output(&[
            "config",
            "--local",
            "--replace-all",
            "credential.https://github.com.helper",
            "",
        ])?;
        self.output(&[
            "config",
            "--local",
            "--add",
            "credential.https://github.com.helper",
            "!gh auth git-credential",
        ])?;
        Ok(())
    }

    pub fn open(root: &Path) -> Result<Self, String> {
        let canonical = root
            .canonicalize()
            .map_err(|_| "Cofre não encontrado".to_string())?;
        let git = Self {
            root: canonical.clone(),
        };
        let toplevel = git.output(&["rev-parse", "--show-toplevel"])?;
        let toplevel = Path::new(toplevel.trim())
            .canonicalize()
            .map_err(|_| "Repositório Git inválido".to_string())?;
        if toplevel != canonical {
            return Err("O cofre precisa ser a raiz de um repositório Git próprio".into());
        }
        Ok(git)
    }

    pub fn status(&self) -> Result<GitStatus, String> {
        let branch = self.output(&["symbolic-ref", "--quiet", "--short", "HEAD"])?;
        let branch = branch.trim().to_owned();
        if branch.is_empty() {
            return Err("Git está sem branch ativa".into());
        }
        let dirty = !self
            .output(&["status", "--porcelain=v1", "--untracked-files=all"])?
            .is_empty();
        let upstream = self
            .try_output(&[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ])
            .map(|value| value.trim().to_owned());
        let (ahead, behind) = if upstream.is_some() {
            let counts =
                self.output(&["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])?;
            let mut parts = counts.split_whitespace();
            let ahead = parts
                .next()
                .and_then(|value| value.parse().ok())
                .ok_or("Status Git inválido")?;
            let behind = parts
                .next()
                .and_then(|value| value.parse().ok())
                .ok_or("Status Git inválido")?;
            (ahead, behind)
        } else {
            (0, 0)
        };
        Ok(GitStatus {
            branch,
            upstream,
            dirty,
            ahead,
            behind,
        })
    }

    pub fn pull(&self) -> Result<(), String> {
        let before = self.require_clean_upstream()?;
        let remote = self.remote(&before.branch)?;
        self.output(&["fetch", "--no-tags", &remote])?;
        let after = self.status()?;
        if after.behind == 0 {
            return Ok(());
        }
        if after.ahead != 0 {
            return Err(
                "Históricos local e remoto divergiram. Resolva o conflito manualmente no Git."
                    .into(),
            );
        }
        self.output(&["merge", "--ff-only", "@{upstream}"])?;
        Ok(())
    }

    pub fn push(&self) -> Result<(), String> {
        let before = self.require_clean_upstream()?;
        let remote = self.remote(&before.branch)?;
        self.output(&["fetch", "--no-tags", &remote])?;
        let after = self.status()?;
        if after.behind != 0 {
            return Err("Há commits novos no remoto. Atualize antes de enviar; nenhuma versão foi sobrescrita.".into());
        }
        if after.ahead == 0 {
            return Ok(());
        }
        let merge_ref =
            self.output(&["config", "--get", &format!("branch.{}.merge", after.branch)])?;
        let merge_ref = merge_ref.trim();
        if !merge_ref.starts_with("refs/heads/") {
            return Err("Branch remota inválida".into());
        }
        self.output(&["push", &remote, &format!("HEAD:{merge_ref}")])?;
        Ok(())
    }

    fn require_clean_upstream(&self) -> Result<GitStatus, String> {
        let status = self.status()?;
        if status.dirty {
            return Err(
                "O cofre tem alterações Git não salvas. Resolva ou registre antes de sincronizar."
                    .into(),
            );
        }
        if status.upstream.is_none() {
            return Err(
                "Configure um branch remoto de acompanhamento antes de sincronizar.".into(),
            );
        }
        Ok(status)
    }

    fn remote(&self, branch: &str) -> Result<String, String> {
        let remote = self.output(&["config", "--get", &format!("branch.{branch}.remote")])?;
        let remote = remote.trim();
        if remote.is_empty() || remote == "." {
            return Err("Remoto Git inválido".into());
        }
        Ok(remote.to_owned())
    }

    fn try_output(&self, args: &[&str]) -> Option<String> {
        let output = self.git(args).ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn output(&self, args: &[&str]) -> Result<String, String> {
        let output = self.git(args)?;
        if !output.status.success() {
            return Err(format!(
                "Falha em Git {}. Verifique repositório, remoto e autenticação.",
                args[0]
            ));
        }
        String::from_utf8(output.stdout).map_err(|_| "Saída Git inválida".into())
    }

    fn git(&self, args: &[&str]) -> Result<std::process::Output, String> {
        Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "/bin/false")
            .output()
            .map_err(|_| "Git não está instalado ou não pôde ser iniciado".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture {
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let n = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root =
                std::env::temp_dir().join(format!("passage-git-test-{}-{n}", std::process::id()));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }
        fn git(&self, dir: &Path, args: &[&str]) {
            let result = Command::new("git")
                .args(args)
                .current_dir(dir)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "git {:?}: {}",
                args,
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn github_helper_is_scoped_to_vault_repository() {
        let f = Fixture::new();
        let local = f.root.join("vault");
        fs::create_dir(&local).unwrap();
        f.git(&local, &["init"]);
        let git = GitVault::open(&local).unwrap();
        git.configure_github_helper().unwrap();
        let helpers = git
            .output(&[
                "config",
                "--local",
                "--get-all",
                "credential.https://github.com.helper",
            ])
            .unwrap();
        assert_eq!(
            helpers.lines().collect::<Vec<_>>(),
            ["", "!gh auth git-credential"]
        );
        assert!(!f.root.join(".git").exists());
    }

    #[test]
    fn refuses_dirty_worktree_before_fetch_or_push() {
        let f = Fixture::new();
        let bare = f.root.join("remote.git");
        f.git(&f.root, &["init", "--bare", bare.to_str().unwrap()]);
        let local = f.root.join("vault");
        fs::create_dir(&local).unwrap();
        f.git(&local, &["init", "-b", "main"]);
        f.git(&local, &["config", "user.name", "Passage Test"]);
        f.git(&local, &["config", "user.email", "test@example.invalid"]);
        fs::write(local.join("entry.gpg"), b"ciphertext").unwrap();
        f.git(&local, &["add", "."]);
        f.git(&local, &["commit", "-m", "initial"]);
        f.git(&local, &["remote", "add", "origin", bare.to_str().unwrap()]);
        f.git(&local, &["push", "-u", "origin", "main"]);
        let git = GitVault::open(&local).unwrap();
        assert!(!git.status().unwrap().dirty);
        fs::write(local.join("entry.gpg"), b"changed").unwrap();
        assert!(git.status().unwrap().dirty);
        assert!(git.pull().unwrap_err().contains("alterações"));
        assert!(git.push().unwrap_err().contains("alterações"));
    }

    #[test]
    fn refuses_vault_nested_in_another_repository() {
        let f = Fixture::new();
        f.git(&f.root, &["init"]);
        let nested = f.root.join("nested");
        fs::create_dir(&nested).unwrap();
        assert!(GitVault::open(&nested).is_err());
    }

    #[test]
    fn fast_forwards_and_pushes_without_overwriting_divergence() {
        let f = Fixture::new();
        let bare = f.root.join("remote.git");
        f.git(&f.root, &["init", "--bare", bare.to_str().unwrap()]);
        let local = f.root.join("vault");
        fs::create_dir(&local).unwrap();
        f.git(&local, &["init", "-b", "main"]);
        f.git(&local, &["config", "user.name", "Passage Test"]);
        f.git(&local, &["config", "user.email", "test@example.invalid"]);
        fs::write(local.join("entry.gpg"), b"first").unwrap();
        f.git(&local, &["add", "."]);
        f.git(&local, &["commit", "-m", "initial"]);
        f.git(&local, &["remote", "add", "origin", bare.to_str().unwrap()]);
        f.git(&local, &["push", "-u", "origin", "main"]);

        let second = f.root.join("second");
        f.git(
            &f.root,
            &[
                "clone",
                "-b",
                "main",
                bare.to_str().unwrap(),
                second.to_str().unwrap(),
            ],
        );
        f.git(&second, &["config", "user.name", "Passage Test"]);
        f.git(&second, &["config", "user.email", "test@example.invalid"]);
        fs::write(second.join("other.gpg"), b"remote update").unwrap();
        f.git(&second, &["add", "."]);
        f.git(&second, &["commit", "-m", "remote update"]);
        f.git(&second, &["push"]);

        let git = GitVault::open(&local).unwrap();
        git.pull().unwrap();
        assert_eq!(fs::read(local.join("other.gpg")).unwrap(), b"remote update");
        fs::write(local.join("third.gpg"), b"local update").unwrap();
        f.git(&local, &["add", "."]);
        f.git(&local, &["commit", "-m", "local update"]);
        git.push().unwrap();
        assert_eq!(git.status().unwrap().ahead, 0);

        fs::write(local.join("local-only.gpg"), b"local").unwrap();
        f.git(&local, &["add", "."]);
        f.git(&local, &["commit", "-m", "local only"]);
        f.git(&second, &["pull", "--ff-only"]);
        fs::write(second.join("remote-only.gpg"), b"remote").unwrap();
        f.git(&second, &["add", "."]);
        f.git(&second, &["commit", "-m", "remote only"]);
        f.git(&second, &["push"]);
        assert!(git.pull().unwrap_err().contains("divergiram"));
        assert!(git.push().unwrap_err().contains("commits novos"));
        assert!(local.join("local-only.gpg").exists());
        assert!(!local.join("remote-only.gpg").exists());
    }
}
