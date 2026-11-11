use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub username: Option<String>,
    pub urls: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Secret {
    pub password: String,
    pub details: String,
    pub username: Option<String>,
    pub urls: Vec<String>,
    pub ciphertext_hash: String,
}

#[derive(Clone, Debug)]
pub struct Vault {
    root: PathBuf,
}

impl Vault {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn open_env() -> Result<Self, String> {
        let (root, _) = crate::setup::active_vault_path()?;
        Self::open(root)
    }

    pub fn open(root: PathBuf) -> Result<Self, String> {
        if !root.is_dir() {
            return Err(format!("Cofre não encontrado em {}", root.display()));
        }
        Ok(Self {
            root: root.canonicalize().map_err(|error| error.to_string())?,
        })
    }

    pub fn list(&self) -> Result<Vec<Entry>, String> {
        let mut entries = Vec::new();
        collect_entries(&self.root, &self.root, &mut entries)?;
        entries.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));
        Ok(entries)
    }

    pub fn read(&self, id: &str) -> Result<Secret, String> {
        let file = self.existing_file(id)?;
        let before = ciphertext_hash(&file)?;
        let output = self.command(&["show", id])?;
        if ciphertext_hash(&file)? != before {
            return Err("A entrada mudou durante a leitura. Tente novamente.".into());
        }
        let content = String::from_utf8(output).map_err(|_| "A entrada não contém texto UTF-8")?;
        let mut secret = parse_secret(&content);
        secret.ciphertext_hash = before;
        Ok(secret)
    }

    pub fn create(&self, id: &str, content: &str) -> Result<(), String> {
        let target = self.safe_target(id)?;
        if target.exists() {
            return Err("A entrada já existe".into());
        }
        self.insert(id, content)
    }

    pub fn update(&self, id: &str, content: &str, expected_hash: &str) -> Result<(), String> {
        self.check_hash(id, expected_hash)?;
        self.insert(id, content)
    }

    pub fn remove(&self, id: &str, expected_hash: &str) -> Result<(), String> {
        self.check_hash(id, expected_hash)?;
        self.command(&["rm", "-f", id]).map(|_| ())
    }

    fn check_hash(&self, id: &str, expected_hash: &str) -> Result<(), String> {
        let file = self.existing_file(id)?;
        if expected_hash.len() != 64 || !expected_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("Versão da entrada inválida".into());
        }
        if ciphertext_hash(&file)? != expected_hash {
            return Err("A entrada foi alterada. Atualize antes de salvar.".into());
        }
        Ok(())
    }

    fn command(&self, args: &[&str]) -> Result<Vec<u8>, String> {
        let result = Command::new("pass")
            .args(args)
            .env("PASSWORD_STORE_DIR", &self.root)
            .output()
            .map_err(|error| format!("Falha ao iniciar pass: {error}"))?;
        if !result.status.success() {
            return Err("Operação do pass falhou. Verifique o cofre e a chave GPG.".into());
        }
        Ok(result.stdout)
    }

    fn insert(&self, id: &str, content: &str) -> Result<(), String> {
        if content.is_empty() || content.contains('\0') {
            return Err("Conteúdo de entrada inválido".into());
        }
        let mut child = Command::new("pass")
            .args(["insert", "-m", "-f", id])
            .env("PASSWORD_STORE_DIR", &self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("Falha ao iniciar pass: {error}"))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or("Não foi possível enviar a entrada ao pass")?;
        let result = stdin.write_all(content.as_bytes());
        drop(stdin);
        result.map_err(|_| "Não foi possível enviar a entrada ao pass".to_string())?;
        if !child.wait().map_err(|error| error.to_string())?.success() {
            return Err("Não foi possível salvar a entrada. Verifique a chave GPG.".into());
        }
        Ok(())
    }

    fn safe_target(&self, id: &str) -> Result<PathBuf, String> {
        validate_id(id)?;
        let filename = format!("{id}.gpg");
        let relative = Path::new(&filename);
        let mut current = self.root.clone();
        for component in relative.components() {
            let Component::Normal(part) = component else {
                return Err("Identificador inválido".into());
            };
            current.push(part);
            if let Ok(meta) = fs::symlink_metadata(&current) {
                if meta.file_type().is_symlink() {
                    return Err("Links simbólicos não são permitidos no caminho da entrada".into());
                }
            }
        }
        Ok(current)
    }

    fn existing_file(&self, id: &str) -> Result<PathBuf, String> {
        let target = self.safe_target(id)?;
        if !target.is_file() {
            return Err("Entrada não encontrada".into());
        }
        let canonical = target.canonicalize().map_err(|error| error.to_string())?;
        if !canonical.starts_with(&self.root) {
            return Err("Entrada fora do cofre".into());
        }
        Ok(canonical)
    }
}

pub fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.contains('\0') || id.contains('\\') || id.ends_with(".gpg") {
        return Err("Identificador de entrada inválido".into());
    }
    for component in Path::new(id).components() {
        match component {
            Component::Normal(name) if !name.to_string_lossy().starts_with('-') => {}
            _ => return Err("Identificador de entrada inválido".into()),
        }
    }
    if id
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("Identificador de entrada inválido".into());
    }
    Ok(())
}

fn collect_entries(root: &Path, dir: &Path, entries: &mut Vec<Entry>) -> Result<(), String> {
    for item in fs::read_dir(dir).map_err(|error| error.to_string())? {
        let item = item.map_err(|error| error.to_string())?;
        let kind = item.file_type().map_err(|error| error.to_string())?;
        let path = item.path();
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if item.file_name() != ".git" {
                collect_entries(root, &path, entries)?;
            }
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "gpg") {
            let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
            let full_name = relative.to_string_lossy();
            let id = full_name
                .strip_suffix(".gpg")
                .unwrap_or_default()
                .to_owned();
            if validate_id(&id).is_err() {
                continue;
            }
            entries.push(Entry {
                title: relative
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                id,
                username: None,
                urls: Vec::new(),
            });
        }
    }
    Ok(())
}

pub fn parse_secret(content: &str) -> Secret {
    let (password, details) = content.split_once('\n').unwrap_or((content, ""));
    let mut username = None;
    let mut urls = Vec::new();
    for line in details.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "login" | "username" if username.is_none() && !value.is_empty() => {
                    username = Some(value.to_owned())
                }
                "url" if !value.is_empty() => urls.push(value.to_owned()),
                _ => {}
            }
        }
    }
    Secret {
        password: password.to_owned(),
        details: details.trim_end().to_owned(),
        username,
        urls,
        ciphertext_hash: String::new(),
    }
}

fn ciphertext_hash(file: &Path) -> Result<String, String> {
    let ciphertext = fs::read(file).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(ciphertext)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn validates_ids_and_rejects_traversal_or_options() {
        assert!(validate_id("sites/example.com").is_ok());
        for id in [
            "", "../x", "x/../y", "x//y", "/tmp/x", "-x", "x/-y", "x\\y", "x.gpg",
        ] {
            assert!(validate_id(id).is_err(), "accepted {id}");
        }
    }

    #[test]
    fn parses_known_fields_without_changing_other_lines() {
        let item =
            parse_secret("secret\nlogin: henrique\nurl: https://example.com\nnotes: keep this\n");
        assert_eq!(item.password, "secret");
        assert_eq!(item.username.as_deref(), Some("henrique"));
        assert_eq!(item.urls, ["https://example.com"]);
        assert!(item.details.contains("notes: keep this"));
    }

    #[test]
    fn listing_ignores_git_and_symlinks() {
        let root = env::temp_dir().join(format!("passage-vault-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sites")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join("sites/example.com.gpg"), b"ciphertext").unwrap();
        fs::write(root.join(".git/hidden.gpg"), b"ciphertext").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("sites/example.com.gpg"), root.join("alias.gpg"))
            .unwrap();
        let vault = Vault::open(root.clone()).unwrap();
        let entries = vault.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "sites/example.com");
        assert!(vault.existing_file("sites/example.com").is_ok());
        assert!(vault.existing_file("alias").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
