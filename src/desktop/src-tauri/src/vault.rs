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
    pub other_details: String,
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
        let (content, hash) = self.read_content(id)?;
        let mut secret = parse_secret(&content);
        secret.ciphertext_hash = hash;
        Ok(secret)
    }

    fn read_content(&self, id: &str) -> Result<(String, String), String> {
        let file = self.existing_file(id)?;
        let before = ciphertext_hash(&file)?;
        let output = self.command(&["show", id])?;
        if ciphertext_hash(&file)? != before {
            return Err("A entrada mudou durante a leitura. Tente novamente.".into());
        }
        let content = String::from_utf8(output).map_err(|_| "A entrada não contém texto UTF-8")?;
        Ok((content, before))
    }

    pub fn create(&self, id: &str, content: &str) -> Result<(), String> {
        let target = self.safe_target(id)?;
        if target.exists() {
            return Err("A entrada já existe".into());
        }
        self.insert(id, content)
    }

    pub fn create_login(
        &self,
        id: &str,
        password: &str,
        username: &str,
        url: &str,
        details: &str,
    ) -> Result<Secret, String> {
        let content = compose_login(password, username, url, details)?;
        self.create(id, &content)?;
        self.read(id)
            .map_err(|reason| format!("Entrada criada, mas não foi possível reabri-la: {reason}"))
    }

    pub fn update_login(
        &self,
        id: &str,
        password: &str,
        username: &str,
        url: &str,
        details: &str,
        expected_hash: &str,
    ) -> Result<Secret, String> {
        let (original, current_hash) = self.read_content(id)?;
        if current_hash != expected_hash {
            return Err("A entrada foi alterada. Atualize antes de salvar.".into());
        }
        let old = parse_secret(&original);
        let content = if details == old.other_details {
            replace_known_fields(&original, password, username, url)?
        } else {
            compose_login(password, username, url, details)?
        };
        self.update(id, &content, expected_hash)?;
        self.read(id).map_err(|reason| {
            format!("Entrada atualizada, mas não foi possível reabri-la: {reason}")
        })
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
    let mut other_details = String::new();
    let mut first_login_seen = false;
    let mut first_url_seen = false;
    for segment in details.split_inclusive('\n') {
        let line = segment.trim_end_matches('\n');
        let mut is_form_field = false;
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "login" | "username" if !first_login_seen => {
                    if !value.is_empty() {
                        username = Some(value.to_owned());
                    }
                    first_login_seen = true;
                    is_form_field = true;
                }
                "url" => {
                    if !value.is_empty() {
                        urls.push(value.to_owned());
                    }
                    if !first_url_seen {
                        is_form_field = true;
                        first_url_seen = true;
                    }
                }
                _ => {}
            }
        }
        if !is_form_field {
            other_details.push_str(segment);
        }
    }
    Secret {
        password: password.to_owned(),
        details: details.trim_end().to_owned(),
        other_details,
        username,
        urls,
        ciphertext_hash: String::new(),
    }
}

fn compose_login(
    password: &str,
    username: &str,
    url: &str,
    details: &str,
) -> Result<String, String> {
    if password.is_empty() || password.len() > 16_384 || password.contains(['\n', '\r', '\0']) {
        return Err("Senha da entrada inválida".into());
    }
    if username.len() > 4_096 || username.contains(['\n', '\r', '\0']) {
        return Err("Login da entrada inválido".into());
    }
    if url.len() > 4_096 || url.contains(['\n', '\r', '\0']) {
        return Err("URL da entrada inválida".into());
    }
    if !url.trim().is_empty() {
        let parsed = url::Url::parse(url.trim()).map_err(|_| "URL da entrada inválida")?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err("Use uma URL HTTP ou HTTPS válida".into());
        }
    }
    if details.len() > 1_048_576 || details.contains('\0') {
        return Err("Outros campos da entrada são inválidos".into());
    }
    let mut content = format!("{password}\n");
    if !username.trim().is_empty() {
        content.push_str(&format!("login: {}\n", username.trim()));
    }
    if !url.trim().is_empty() {
        content.push_str(&format!("url: {}\n", url.trim()));
    }
    if !details.is_empty() {
        content.push_str(details);
        if !details.ends_with('\n') {
            content.push('\n');
        }
    }
    Ok(content)
}

fn replace_known_fields(
    original: &str,
    password: &str,
    username: &str,
    url: &str,
) -> Result<String, String> {
    compose_login(password, username, url, "")?;
    let (_, old_details) = original.split_once('\n').unwrap_or((original, ""));
    let mut result = format!("{password}\n");
    let mut login_seen = false;
    let mut url_seen = false;
    let mut body = String::new();
    for segment in old_details.split_inclusive('\n') {
        let key = segment
            .trim_end_matches('\n')
            .split_once(':')
            .map(|(key, _)| key.trim().to_ascii_lowercase());
        match key.as_deref() {
            Some("login" | "username") if !login_seen => {
                login_seen = true;
                if !username.trim().is_empty() {
                    body.push_str(&format!("login: {}\n", username.trim()));
                }
            }
            Some("url") if !url_seen => {
                url_seen = true;
                if !url.trim().is_empty() {
                    body.push_str(&format!("url: {}\n", url.trim()));
                }
            }
            _ => body.push_str(segment),
        }
    }
    if !login_seen && !username.trim().is_empty() {
        result.push_str(&format!("login: {}\n", username.trim()));
    }
    if !url_seen && !url.trim().is_empty() {
        result.push_str(&format!("url: {}\n", url.trim()));
    }
    result.push_str(&body);
    Ok(result)
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
        assert_eq!(item.other_details, "notes: keep this\n");
    }

    #[test]
    fn editing_keeps_unknown_fields_and_additional_urls() {
        let original = parse_secret("old\nlogin: alice\nurl: https://first.example\notpauth: totp-secret\nurl: https://second.example\ncustom: keep\n");
        let edited =
            compose_login("new", "bob", "https://new.example", &original.other_details).unwrap();
        assert!(edited.contains("login: bob\nurl: https://new.example\n"));
        assert!(
            edited.contains("otpauth: totp-secret\nurl: https://second.example\ncustom: keep\n")
        );
        assert!(!edited.contains("login: alice"));
        assert!(!edited.contains("https://first.example"));
        assert!(compose_login("a\nb", "alice", "https://example.com", "").is_err());
        assert!(compose_login("pw", "alice", "javascript:alert(1)", "").is_err());
        let original = "old\ncustom: before\nlogin: alice\notpauth: secret\nurl: https://first.example\ncustom: after\n";
        let updated = replace_known_fields(original, "new", "bob", "https://new.example").unwrap();
        assert_eq!(updated, "new\ncustom: before\nlogin: bob\notpauth: secret\nurl: https://new.example\ncustom: after\n");
    }

    #[test]
    fn login_crud_with_disposable_pass_store() {
        let root = env::temp_dir().join(format!("passage-login-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("gnupg")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root.join("gnupg"), fs::Permissions::from_mode(0o700)).unwrap();
        }
        let output = Command::new(env::current_exe().unwrap())
            .args(["--exact", "vault::tests::login_crud_child", "--nocapture"])
            .env("PASSAGE_LOGIN_TEST_CHILD", "1")
            .env("GNUPGHOME", root.join("gnupg"))
            .env("HOME", &root)
            .env_remove("PASSWORD_STORE_DIR")
            .output()
            .unwrap();
        let _ = fs::remove_dir_all(&root);
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn login_crud_child() {
        if env::var_os("PASSAGE_LOGIN_TEST_CHILD").is_none() {
            return;
        }
        let home = PathBuf::from(env::var_os("HOME").unwrap());
        let key = Command::new("gpg")
            .args([
                "--batch",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "",
                "--quick-generate-key",
                "Passage Login Test <login-test@example.invalid>",
                "default",
                "default",
                "1d",
            ])
            .output()
            .unwrap();
        assert!(key.status.success());
        let listing = Command::new("gpg")
            .args(["--with-colons", "--fingerprint", "--list-secret-keys"])
            .output()
            .unwrap();
        let fingerprint = String::from_utf8_lossy(&listing.stdout)
            .lines()
            .find(|line| line.starts_with("fpr:"))
            .and_then(|line| line.split(':').nth(9))
            .unwrap()
            .to_owned();
        let root = home.join("store");
        let init = Command::new("pass")
            .args(["init", &fingerprint])
            .env("PASSWORD_STORE_DIR", &root)
            .output()
            .unwrap();
        assert!(init.status.success());
        let vault = Vault::open(root.clone()).unwrap();
        let created = vault
            .create_login(
                "sites/example.com",
                "old-secret",
                "alice",
                "https://example.com",
                "otpauth: test\ncustom: keep\n",
            )
            .unwrap();
        assert_eq!(created.password, "old-secret");
        assert!(root.join("sites/example.com.gpg").is_file());
        let stale_hash = created.ciphertext_hash.clone();
        let changed = Command::new("pass")
            .args(["insert", "-m", "-f", "sites/example.com"])
            .env("PASSWORD_STORE_DIR", &root)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let mut changed = changed;
        changed
            .stdin
            .take()
            .unwrap()
            .write_all(
                b"external\nlogin: alice\nurl: https://example.com\notpauth: test\ncustom: keep\n",
            )
            .unwrap();
        assert!(changed.wait().unwrap().success());
        assert!(vault
            .update_login(
                "sites/example.com",
                "would-overwrite",
                "bob",
                "https://example.com",
                "custom: keep\n",
                &stale_hash
            )
            .is_err());
        let fresh = vault.read("sites/example.com").unwrap();
        assert_eq!(fresh.password, "external");
        let saved = vault
            .update_login(
                "sites/example.com",
                "new-secret",
                "bob",
                "https://new.example",
                &fresh.other_details,
                &fresh.ciphertext_hash,
            )
            .unwrap();
        assert_eq!(saved.password, "new-secret");
        assert_eq!(saved.username.as_deref(), Some("bob"));
        assert!(saved.other_details.contains("otpauth: test"));
        assert!(saved.other_details.contains("custom: keep"));
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
