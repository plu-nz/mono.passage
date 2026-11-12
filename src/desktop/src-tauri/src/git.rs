use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
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
    pub fn initialize(root: &Path) -> Result<GitStatus, String> {
        let root = root.canonicalize().map_err(|_| "Cofre não encontrado")?;
        if !root.join(".gpg-id").is_file() {
            return Err("Inicialize o cofre com GNU pass antes do Git".into());
        }
        if root.join(".git").exists() {
            return Err("Este cofre já contém um repositório Git".into());
        }
        if Self::open(&root).is_ok() {
            return Err("Este cofre já pertence a um repositório Git".into());
        }
        let result = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&root)
            .output()
            .map_err(|_| "Git não está instalado")?;
        if !result.status.success() {
            return Err("Não foi possível iniciar o repositório Git".into());
        }
        Self::open(&root)?.status()
    }

    pub fn set_remote(&self, url: &str) -> Result<GitStatus, String> {
        let url = url.trim();
        if url.is_empty() || url.starts_with('-') || url.chars().any(char::is_control) {
            return Err("URL do remoto Git inválida".into());
        }
        if self.try_output(&["remote", "get-url", "origin"]).is_some() {
            return Err(
                "O remoto origin já existe. Altere-o pelo Git para evitar substituição acidental."
                    .into(),
            );
        }
        self.output(&["remote", "add", "origin", url])?;
        self.status()
    }

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

    pub fn publish_new_vault(&self) -> Result<GitStatus, String> {
        let status = self.status()?;
        if status.upstream.is_some() {
            return Err("Este cofre já tem upstream. Use Enviar para sincronizar.".into());
        }
        if self.try_output(&["remote", "get-url", "origin"]).is_none() {
            return Err("Adicione um remoto origin antes de publicar o cofre.".into());
        }
        if !self.output(&["ls-remote", "origin"])?.trim().is_empty() {
            return Err(
                "O remoto já contém histórico. Conecte esse cofre manualmente pelo Git.".into(),
            );
        }
        if self
            .try_output(&["rev-parse", "--verify", "HEAD"])
            .is_none()
        {
            if !self
                .output(&["diff", "--cached", "--name-only"])?
                .is_empty()
            {
                return Err("Há arquivos preparados no Git. Revise-os antes de publicar.".into());
            }
            let mut files = vec![".gpg-id".to_string()];
            self.collect_vault_files(&self.root, &mut files)?;
            for file in &files {
                if file.ends_with(".gpg") {
                    validate_ciphertext(
                        &fs::read(self.root.join(file))
                            .map_err(|_| "Não foi possível ler uma entrada")?,
                    )?;
                }
            }
            let literals: Vec<_> = files
                .iter()
                .map(|file| format!(":(literal){file}"))
                .collect();
            let mut args = vec!["add", "--"];
            args.extend(literals.iter().map(String::as_str));
            self.output(&args)?;
            if let Err(reason) = self.verify_staged_ciphertexts(&files) {
                let mut unstage = vec!["rm", "--cached", "-q", "--"];
                unstage.extend(literals.iter().map(String::as_str));
                let _ = self.git(&unstage);
                return Err(reason);
            }
            self.output(&["commit", "-m", "Initialize encrypted vault"])
                .map_err(|_| {
                    "Falha ao criar commit. Configure user.name e user.email no Git.".to_string()
                })?;
        } else if status.dirty {
            return Err("Há alterações locais. Registre ou descarte antes de publicar.".into());
        }
        let branch = self.status()?.branch;
        self.output(&[
            "push",
            "--set-upstream",
            "origin",
            &format!("HEAD:refs/heads/{branch}"),
        ])?;
        self.status()
    }

    pub fn commit_changes(&self) -> Result<GitStatus, String> {
        if self
            .try_output(&["rev-parse", "--verify", "HEAD"])
            .is_none()
        {
            return Err("Publique o cofre novo antes de registrar alterações.".into());
        }
        if !self
            .output(&["diff", "--cached", "--name-only"])?
            .is_empty()
        {
            return Err("Há alterações já preparadas no Git. Revise-as pelo terminal.".into());
        }
        let changed = self.output(&["ls-files", "-m", "-d", "-o", "--exclude-standard", "-z"])?;
        let mut paths = Vec::new();
        for path in changed.split('\0').filter(|path| !path.is_empty()) {
            let relative = Path::new(path);
            if relative.is_absolute()
                || relative
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                return Err("Caminho Git inválido".into());
            }
            if !path.ends_with(".gpg") && relative.file_name().is_none_or(|name| name != ".gpg-id")
            {
                return Err(
                    "Há arquivos fora do formato do cofre. Revise-os pelo terminal.".into(),
                );
            }
            let full = self.root.join(relative);
            self.reject_symlink_components(relative)?;
            if let Ok(metadata) = fs::symlink_metadata(&full) {
                if !metadata.file_type().is_file() {
                    return Err(
                        "O cofre contém links ou arquivos especiais. Revise-os pelo terminal."
                            .into(),
                    );
                }
                if path.ends_with(".gpg") {
                    validate_ciphertext(
                        &fs::read(&full).map_err(|_| "Não foi possível ler uma entrada")?,
                    )?;
                }
            }
            paths.push(format!(":(literal){path}"));
        }
        if paths.is_empty() {
            return Err("Não há alterações no cofre para registrar.".into());
        }
        let mut args = vec!["add", "-A", "--"];
        args.extend(paths.iter().map(String::as_str));
        self.output(&args)?;
        let staged = self.verify_staged_ciphertexts(&paths);
        if let Err(reason) = staged {
            let mut unstage = vec!["reset", "-q", "--"];
            unstage.extend(paths.iter().map(String::as_str));
            let _ = self.git(&unstage);
            return Err(reason);
        }
        self.output(&["commit", "-m", "Update encrypted vault"])
            .map_err(|_| {
                "Falha ao criar commit. Configure user.name e user.email no Git.".to_string()
            })?;
        self.status()
    }

    pub fn merge_preserving_both(&self) -> Result<GitStatus, String> {
        let before = self.require_clean_upstream()?;
        let remote = self.remote(&before.branch)?;
        self.output(&["fetch", "--no-tags", &remote])?;
        let status = self.status()?;
        if status.ahead == 0 || status.behind == 0 {
            return Err("Não há históricos divergentes para resolver.".into());
        }
        let local_head = self.output(&["rev-parse", "HEAD"])?;
        let remote_head = self.output(&["rev-parse", "@{upstream}"])?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "Relógio inválido")?
            .as_nanos();
        let prefix = format!("refs/passage/backup/{stamp}");
        self.output(&["update-ref", &format!("{prefix}-local"), local_head.trim()])?;
        self.output(&[
            "update-ref",
            &format!("{prefix}-remote"),
            remote_head.trim(),
        ])?;
        let merge = self.git(&["merge", "--no-commit", "--no-ff", "@{upstream}"])?;
        let mut created = Vec::new();
        let result = (|| -> Result<(), String> {
            let unmerged = self.output(&["ls-files", "-u", "-z"])?;
            let mut conflicts = std::collections::BTreeSet::new();
            for record in unmerged.split('\0').filter(|record| !record.is_empty()) {
                let (metadata, path) = record.split_once('\t').ok_or("Índice Git inválido")?;
                if !metadata.starts_with("100644 ") && !metadata.starts_with("100755 ") {
                    return Err("Conflito com link ou arquivo especial. Resolva pelo Git; os commits foram preservados.".into());
                }
                conflicts.insert(path.to_string());
            }
            if !merge.status.success() && conflicts.is_empty() {
                return Err(
                    "O Git não conseguiu iniciar a mesclagem. Verifique o repositório.".into(),
                );
            }
            for path in conflicts {
                let relative = Path::new(&path);
                if !path.ends_with(".gpg")
                    || relative.is_absolute()
                    || relative
                        .components()
                        .any(|part| !matches!(part, std::path::Component::Normal(_)))
                {
                    return Err("Conflito fora de uma entrada .gpg. Resolva pelo Git; os dois commits foram preservados.".into());
                }
                self.reject_symlink_components(relative)?;
                let local = self.git(&["show", &format!(":2:{path}")])?;
                let remote = self.git(&["show", &format!(":3:{path}")])?;
                if !local.status.success() || !remote.status.success() {
                    return Err("Conflito com remoção ou renomeação. Resolva pelo Git; os dois commits foram preservados.".into());
                }
                validate_ciphertext(&local.stdout)?;
                validate_ciphertext(&remote.stdout)?;
                let suffix = &remote_head.trim()[..8];
                let backup = relative.with_file_name(format!(
                    "{}.remote-{suffix}.gpg",
                    relative
                        .file_stem()
                        .ok_or("Nome inválido")?
                        .to_string_lossy()
                ));
                let backup_full = self.root.join(&backup);
                let mut options = fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let mut file = options.open(&backup_full).map_err(|_| "Já existe uma cópia com esse nome. Resolva pelo Git; os commits foram preservados.")?;
                created.push(backup_full);
                file.write_all(&remote.stdout)
                    .map_err(|_| "Falha ao salvar a cópia cifrada do remoto")?;
                let literal = format!(":(literal){path}");
                self.output(&["checkout", "--ours", "--", &literal])?;
                self.output(&[
                    "add",
                    "--",
                    &literal,
                    &format!(":(literal){}", backup.to_string_lossy()),
                ])?;
            }
            if !self
                .output(&["diff", "--name-only", "--diff-filter=U"])?
                .is_empty()
            {
                return Err("Restaram conflitos Git. Os commits foram preservados.".into());
            }
            self.output(&[
                "commit",
                "-m",
                "Merge encrypted vault preserving both versions",
            ])?;
            Ok(())
        })();
        if let Err(reason) = result {
            let _ = self.git(&["merge", "--abort"]);
            for path in created {
                let _ = fs::remove_file(path);
            }
            return Err(reason);
        }
        self.status()
    }

    fn collect_vault_files(&self, directory: &Path, files: &mut Vec<String>) -> Result<(), String> {
        for entry in fs::read_dir(directory).map_err(|_| "Não foi possível ler o cofre")? {
            let entry = entry.map_err(|_| "Não foi possível ler o cofre")?;
            let path = entry.path();
            let relative = path
                .strip_prefix(&self.root)
                .map_err(|_| "Caminho inválido")?;
            if relative == Path::new(".git") {
                continue;
            }
            let kind = entry
                .file_type()
                .map_err(|_| "Não foi possível ler o cofre")?;
            if kind.is_dir() {
                self.collect_vault_files(&path, files)?;
            } else if kind.is_file() && relative != Path::new(".gpg-id") {
                if path.file_name().is_some_and(|name| name == ".gpg-id")
                    || path.extension().is_some_and(|ext| ext == "gpg")
                {
                    files.push(relative.to_string_lossy().into_owned());
                } else {
                    return Err(
                        "O cofre contém arquivos além de .gpg e .gpg-id. Revise antes de publicar."
                            .into(),
                    );
                }
            } else if !kind.is_file() {
                return Err(
                    "O cofre contém links ou arquivos especiais. Revise antes de publicar.".into(),
                );
            }
        }
        Ok(())
    }

    fn reject_symlink_components(&self, relative: &Path) -> Result<(), String> {
        let mut current = self.root.clone();
        for component in relative.components() {
            let std::path::Component::Normal(part) = component else {
                return Err("Caminho Git inválido".into());
            };
            current.push(part);
            if fs::symlink_metadata(&current).is_ok_and(|meta| meta.file_type().is_symlink()) {
                return Err("Links simbólicos não são permitidos no cofre Git".into());
            }
        }
        Ok(())
    }

    fn verify_staged_ciphertexts(&self, files: &[String]) -> Result<(), String> {
        for file in files {
            let path = file.strip_prefix(":(literal)").unwrap_or(file);
            if path.ends_with(".gpg") {
                let output = self.git(&["show", &format!(":0:{path}")])?;
                if !output.status.success() {
                    if !self.root.join(path).exists() {
                        continue;
                    }
                    return Err("Não foi possível validar a entrada preparada no Git".into());
                }
                validate_ciphertext(&output.stdout)?;
            }
        }
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

fn validate_ciphertext(bytes: &[u8]) -> Result<(), String> {
    let mut child = Command::new("gpg")
        .args(["--batch", "--no-tty", "--list-packets"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "GPG não está instalado")?;
    child
        .stdin
        .take()
        .ok_or("Falha ao validar entrada")?
        .write_all(bytes)
        .map_err(|_| "Falha ao validar entrada")?;
    let output = child
        .wait_with_output()
        .map_err(|_| "Falha ao validar entrada")?;
    let packets = String::from_utf8_lossy(&output.stdout);
    if (packets.contains(":pubkey enc packet:") || packets.contains(":symkey enc packet:"))
        && (packets.contains(":encrypted data packet:")
            || packets.contains(":aead encrypted packet:"))
    {
        Ok(())
    } else {
        Err("Uma entrada .gpg não contém dados OpenPGP cifrados. Revise o cofre antes de registrar.".into())
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
        fn encrypted(&self, path: &Path, content: &[u8]) {
            let mut child = Command::new("gpg")
                .args([
                    "--batch",
                    "--yes",
                    "--pinentry-mode",
                    "loopback",
                    "--passphrase",
                    "passage-test",
                    "--symmetric",
                    "--output",
                ])
                .arg(path)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(content).unwrap();
            assert!(child.wait().unwrap().success());
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

    #[test]
    fn initializes_only_a_pass_vault_and_preserves_existing_remote() {
        let f = Fixture::new();
        let local = f.root.join("vault");
        fs::create_dir(&local).unwrap();
        assert!(GitVault::initialize(&local).is_err());
        fs::write(local.join(".gpg-id"), b"test-key\n").unwrap();
        let git = GitVault::initialize(&local).unwrap();
        assert_eq!(git.branch, "main");
        assert!(git.dirty);
        let git = GitVault::open(&local).unwrap();
        git.set_remote("https://github.com/example/vault.git")
            .unwrap();
        assert!(git.set_remote("https://example.invalid/other.git").is_err());
        assert_eq!(
            git.output(&["remote", "get-url", "origin"]).unwrap().trim(),
            "https://github.com/example/vault.git"
        );
        assert!(GitVault::initialize(&local).is_err());
    }

    #[test]
    fn publishes_only_encrypted_vault_to_empty_remote() {
        let f = Fixture::new();
        let bare = f.root.join("remote.git");
        f.git(&f.root, &["init", "--bare", bare.to_str().unwrap()]);
        let local = f.root.join("vault");
        fs::create_dir(&local).unwrap();
        fs::write(local.join(".gpg-id"), b"test-key\n").unwrap();
        fs::create_dir(local.join("nested")).unwrap();
        f.encrypted(&local.join("nested/entry.gpg"), b"ciphertext");
        GitVault::initialize(&local).unwrap();
        f.git(&local, &["config", "user.name", "Passage Test"]);
        f.git(&local, &["config", "user.email", "test@example.invalid"]);
        let git = GitVault::open(&local).unwrap();
        git.set_remote(bare.to_str().unwrap()).unwrap();
        let status = git.publish_new_vault().unwrap();
        assert_eq!(status.upstream.as_deref(), Some("origin/main"));
        assert!(!status.dirty);
        assert_eq!(
            git.output(&["ls-files"])
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            [".gpg-id", "nested/entry.gpg"]
        );
        assert!(git.publish_new_vault().is_err());
    }

    #[test]
    fn refuses_plaintext_and_existing_remote_history() {
        let f = Fixture::new();
        let bare = f.root.join("remote.git");
        f.git(&f.root, &["init", "--bare", bare.to_str().unwrap()]);
        let local = f.root.join("vault");
        fs::create_dir(&local).unwrap();
        fs::write(local.join(".gpg-id"), b"test-key\n").unwrap();
        GitVault::initialize(&local).unwrap();
        f.git(&local, &["config", "user.name", "Passage Test"]);
        f.git(&local, &["config", "user.email", "test@example.invalid"]);
        let git = GitVault::open(&local).unwrap();
        git.set_remote(bare.to_str().unwrap()).unwrap();
        fs::write(local.join("secret.txt"), b"plaintext").unwrap();
        assert!(git.publish_new_vault().unwrap_err().contains("além de"));
        assert!(git.try_output(&["rev-parse", "--verify", "HEAD"]).is_none());
        fs::remove_file(local.join("secret.txt")).unwrap();
        f.git(&local, &["add", ".gpg-id"]);
        f.git(&local, &["commit", "-m", "initial"]);
        f.git(&local, &["push", "origin", "main"]);
        assert!(git.publish_new_vault().unwrap_err().contains("histórico"));
        assert!(git.status().unwrap().upstream.is_none());
    }

    #[test]
    fn commits_only_vault_files_and_refuses_plaintext() {
        let f = Fixture::new();
        let local = f.root.join("vault");
        fs::create_dir(&local).unwrap();
        f.git(&local, &["init", "-b", "main"]);
        f.git(&local, &["config", "user.name", "Passage Test"]);
        f.git(&local, &["config", "user.email", "test@example.invalid"]);
        fs::write(local.join(".gpg-id"), b"test-key\n").unwrap();
        f.git(&local, &["add", ".gpg-id"]);
        f.git(&local, &["commit", "-m", "initial"]);
        let git = GitVault::open(&local).unwrap();
        fs::write(local.join("secret.txt"), b"plaintext").unwrap();
        f.encrypted(&local.join("new.gpg"), b"ciphertext");
        assert!(git
            .commit_changes()
            .unwrap_err()
            .contains("fora do formato"));
        assert!(git
            .output(&["diff", "--cached", "--name-only"])
            .unwrap()
            .is_empty());
        fs::remove_file(local.join("secret.txt")).unwrap();
        fs::write(local.join("fake.gpg"), b"plain text renamed as gpg").unwrap();
        assert!(git.commit_changes().unwrap_err().contains("OpenPGP"));
        fs::remove_file(local.join("fake.gpg")).unwrap();
        git.commit_changes().unwrap();
        assert_eq!(
            git.output(&["ls-files"])
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            [".gpg-id", "new.gpg"]
        );
        fs::remove_file(local.join("new.gpg")).unwrap();
        git.commit_changes().unwrap();
        assert_eq!(git.output(&["ls-files"]).unwrap().trim(), ".gpg-id");
    }

    #[test]
    fn refuses_divergence_without_changing_local_or_remote() {
        let f = Fixture::new();
        let bare = f.root.join("remote.git");
        f.git(&f.root, &["init", "--bare", bare.to_str().unwrap()]);
        let local = f.root.join("vault");
        fs::create_dir(&local).unwrap();
        f.git(&local, &["init", "-b", "main"]);
        f.git(&local, &["config", "user.name", "Passage Test"]);
        f.git(&local, &["config", "user.email", "test@example.invalid"]);
        fs::write(local.join(".gpg-id"), b"test-key\n").unwrap();
        f.git(&local, &["add", ".gpg-id"]);
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
        fs::write(second.join("remote.gpg"), b"remote").unwrap();
        f.git(&second, &["add", "remote.gpg"]);
        f.git(&second, &["commit", "-m", "remote"]);
        f.git(&second, &["push"]);
        fs::write(local.join("local.gpg"), b"local").unwrap();
        f.git(&local, &["add", "local.gpg"]);
        f.git(&local, &["commit", "-m", "local"]);
        let git = GitVault::open(&local).unwrap();
        let head = git.output(&["rev-parse", "HEAD"]).unwrap();
        assert!(git.pull().unwrap_err().contains("divergiram"));
        assert!(git.push().unwrap_err().contains("remoto"));
        assert_eq!(git.output(&["rev-parse", "HEAD"]).unwrap(), head);
        assert!(!local.join("remote.gpg").exists());
    }

    #[test]
    fn merge_preserves_both_ciphertexts_on_same_entry_conflict() {
        let f = Fixture::new();
        let bare = f.root.join("remote.git");
        f.git(&f.root, &["init", "--bare", bare.to_str().unwrap()]);
        let local = f.root.join("vault");
        fs::create_dir(&local).unwrap();
        f.git(&local, &["init", "-b", "main"]);
        f.git(&local, &["config", "user.name", "Passage Test"]);
        f.git(&local, &["config", "user.email", "test@example.invalid"]);
        fs::write(local.join(".gpg-id"), b"test-key\n").unwrap();
        f.encrypted(&local.join("entry.gpg"), b"base\n");
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
        f.encrypted(&local.join("entry.gpg"), b"local\n");
        let local_ciphertext = fs::read(local.join("entry.gpg")).unwrap();
        f.git(&local, &["add", "entry.gpg"]);
        f.git(&local, &["commit", "-m", "local"]);
        f.encrypted(&second.join("entry.gpg"), b"remote\n");
        let remote_ciphertext = fs::read(second.join("entry.gpg")).unwrap();
        f.git(&second, &["add", "entry.gpg"]);
        f.git(&second, &["commit", "-m", "remote"]);
        f.git(&second, &["push"]);
        let git = GitVault::open(&local).unwrap();
        let status = git.merge_preserving_both().unwrap();
        assert_eq!(fs::read(local.join("entry.gpg")).unwrap(), local_ciphertext);
        let copies: Vec<_> = fs::read_dir(&local)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("entry.remote-")
            })
            .collect();
        assert_eq!(copies.len(), 1);
        assert_eq!(fs::read(&copies[0]).unwrap(), remote_ciphertext);
        assert_eq!(status.behind, 0);
        assert!(!status.dirty);
        git.push().unwrap();
    }

    #[test]
    fn merge_refuses_recipient_conflict_without_losing_commits() {
        let f = Fixture::new();
        let bare = f.root.join("remote.git");
        f.git(&f.root, &["init", "--bare", bare.to_str().unwrap()]);
        let local = f.root.join("vault");
        fs::create_dir(&local).unwrap();
        f.git(&local, &["init", "-b", "main"]);
        f.git(&local, &["config", "user.name", "Passage Test"]);
        f.git(&local, &["config", "user.email", "test@example.invalid"]);
        fs::write(local.join(".gpg-id"), b"base\n").unwrap();
        f.git(&local, &["add", ".gpg-id"]);
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
        fs::write(local.join(".gpg-id"), b"local\n").unwrap();
        f.git(&local, &["add", ".gpg-id"]);
        f.git(&local, &["commit", "-m", "local"]);
        fs::write(second.join(".gpg-id"), b"remote\n").unwrap();
        f.git(&second, &["add", ".gpg-id"]);
        f.git(&second, &["commit", "-m", "remote"]);
        f.git(&second, &["push"]);
        let git = GitVault::open(&local).unwrap();
        let before = git.output(&["rev-parse", "HEAD"]).unwrap();
        assert!(git
            .merge_preserving_both()
            .unwrap_err()
            .contains("fora de uma entrada"));
        assert_eq!(git.output(&["rev-parse", "HEAD"]).unwrap(), before);
        assert_eq!(fs::read(local.join(".gpg-id")).unwrap(), b"local\n");
        assert!(!git.status().unwrap().dirty);
    }
}
