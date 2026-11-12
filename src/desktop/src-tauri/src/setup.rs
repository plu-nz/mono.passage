use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpgKey {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultInfo {
    pub path: String,
    pub source: String,
    pub initialized: bool,
    pub recipient_ids: Vec<String>,
    pub gpg_keys: Vec<GpgKey>,
    pub pass_installed: bool,
    pub gpg_installed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreatedKeyVault {
    pub info: VaultInfo,
    pub fingerprint: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    vault_path: PathBuf,
}

fn settings_path() -> Result<PathBuf, String> {
    let base = if let Some(dir) = env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(dir)
    } else {
        PathBuf::from(env::var_os("HOME").ok_or("HOME não está definido")?).join(".config")
    };
    if !base.is_absolute() {
        return Err("XDG_CONFIG_HOME precisa ser absoluto".into());
    }
    Ok(base.join("passage/settings.json"))
}

pub fn is_initialized(path: &Path) -> bool {
    path.is_dir()
        && fs::symlink_metadata(path.join(".gpg-id"))
            .is_ok_and(|metadata| metadata.file_type().is_file())
}

pub fn active_vault_path() -> Result<(PathBuf, &'static str), String> {
    if let Some(path) = env::var_os("PASSWORD_STORE_DIR") {
        return Ok((PathBuf::from(path), "environment"));
    }
    let settings = settings_path()?;
    if settings.is_file() {
        let data =
            fs::read(settings).map_err(|_| "Não foi possível ler as configurações do Passage")?;
        let selected: Settings = serde_json::from_slice(&data)
            .map_err(|_| "Configuração do cofre inválida. Corrija settings.json")?;
        if !selected.vault_path.is_absolute() {
            return Err("Caminho do cofre configurado deve ser absoluto".into());
        }
        return Ok((selected.vault_path, "selection"));
    }
    Ok((
        PathBuf::from(env::var_os("HOME").ok_or("HOME não está definido")?).join(".password-store"),
        "default",
    ))
}

fn persist_selection(path: &Path) -> Result<(), String> {
    let settings = settings_path()?;
    let parent = settings
        .parent()
        .ok_or("Caminho de configuração inválido")?;
    let created_parent = !parent.exists();
    fs::create_dir_all(parent).map_err(|_| "Não foi possível criar a pasta de configuração")?;
    #[cfg(unix)]
    if created_parent {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Não foi possível proteger a pasta de configuração")?;
    }
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Relógio do sistema inválido")?
        .as_nanos();
    let temporary = parent.join(format!(".settings-{}-{suffix}.tmp", std::process::id()));
    let bytes = serde_json::to_vec(&Settings {
        vault_path: path.to_path_buf(),
    })
    .map_err(|_| "Não foi possível serializar a configuração")?;
    let result = (|| -> Result<(), String> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|_| "Não foi possível salvar a configuração")?;
        file.write_all(&bytes)
            .map_err(|_| "Não foi possível salvar a configuração")?;
        file.sync_all()
            .map_err(|_| "Não foi possível confirmar a configuração")?;
        fs::rename(&temporary, &settings).map_err(|_| "Não foi possível ativar a configuração")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn require_selection_allowed() -> Result<(), String> {
    if env::var_os("PASSWORD_STORE_DIR").is_some() {
        Err("PASSWORD_STORE_DIR está definido e prevalece sobre a seleção no Passage".into())
    } else {
        Ok(())
    }
}

pub fn select_vault(path: &str) -> Result<VaultInfo, String> {
    require_selection_allowed()?;
    let path = Path::new(path);
    if !path.is_absolute() || !path.is_dir() {
        return Err("Escolha um diretório de cofre existente com caminho absoluto".into());
    }
    let canonical = path.canonicalize().map_err(|_| "Cofre não encontrado")?;
    if !is_initialized(&canonical) {
        return Err("Este diretório ainda não é um cofre GNU pass: falta .gpg-id".into());
    }
    persist_selection(&canonical)?;
    vault_info()
}

pub fn clone_vault(path: &str, url: &str) -> Result<VaultInfo, String> {
    require_selection_allowed()?;
    let path = clone_vault_to(Path::new(path), url)?;
    persist_selection(&path)?;
    vault_info()
}

fn clone_vault_to(path: &Path, url: &str) -> Result<PathBuf, String> {
    let url = url.trim();
    if url.is_empty() || url.starts_with('-') || url.chars().any(char::is_control) {
        return Err("URL do remoto Git inválida".into());
    }
    validate_new_vault_path(path.to_str().ok_or("Caminho do cofre inválido")?)?;
    if path.exists() {
        return Err("Escolha um diretório que ainda não existe para clonar o cofre".into());
    }
    let output = Command::new("git")
        .args([
            "clone",
            "--no-tags",
            "--single-branch",
            "--branch",
            "main",
            "--",
            url,
        ])
        .arg(path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "/bin/false")
        .output()
        .map_err(|_| "Git não está instalado")?;
    if !output.status.success() {
        return Err("Não foi possível clonar o cofre. Verifique URL, acesso e branch main. Se o Git criou o diretório, revise-o antes de tentar novamente.".into());
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| "Cofre clonado, mas caminho indisponível")?;
    if !is_initialized(&canonical) {
        return Err("O repositório foi clonado, mas não contém .gpg-id na raiz. Revise o diretório criado; ele não foi selecionado como cofre.".into());
    }
    Ok(canonical)
}

pub fn create_vault(path: &str, key_id: &str) -> Result<VaultInfo, String> {
    require_selection_allowed()?;
    if !key_id_is_valid(key_id)
        || !gpg_keys()
            .iter()
            .any(|key| key.id.eq_ignore_ascii_case(key_id))
    {
        return Err("Selecione a impressão digital de uma chave GPG privada disponível".into());
    }
    let path = validate_new_vault_path(path)?;
    let output = Command::new("pass")
        .args(["init", key_id])
        .env("PASSWORD_STORE_DIR", path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|_| "GNU pass não está instalado")?;
    if !output.status.success() {
        return Err(
            "Não foi possível criar o cofre. Verifique GPG e permissões do diretório.".into(),
        );
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| "Cofre criado, mas caminho indisponível")?;
    if !is_initialized(&canonical) {
        return Err("GNU pass não criou o arquivo .gpg-id".into());
    }
    persist_selection(&canonical)?;
    vault_info()
}

fn validate_new_vault_path(path: &str) -> Result<&Path, String> {
    let path = Path::new(path);
    if !path.is_absolute() || path == Path::new("/") {
        return Err("Escolha um diretório novo com caminho absoluto".into());
    }
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err("Link simbólico não é permitido para criar cofre".into());
    }
    if path.exists() {
        if !path.is_dir()
            || fs::read_dir(path)
                .map_err(|_| "Não foi possível ler o diretório")?
                .next()
                .is_some()
        {
            return Err("O diretório do novo cofre precisa estar vazio".into());
        }
    } else {
        let parent = path.parent().ok_or("Diretório pai inválido")?;
        if !parent.is_dir() {
            return Err("O diretório pai precisa existir".into());
        }
    }
    Ok(path)
}

pub fn create_key_and_vault(
    path: &str,
    name: &str,
    email: &str,
) -> Result<CreatedKeyVault, String> {
    create_key_and_vault_with(path, name, email, generate_key)
}

fn create_key_and_vault_with<F>(
    path: &str,
    name: &str,
    email: &str,
    generate: F,
) -> Result<CreatedKeyVault, String>
where
    F: FnOnce(&str) -> Result<String, String>,
{
    require_selection_allowed()?;
    validate_new_vault_path(path)?;
    if !available("pass", "--version") || !available("gpg", "--version") {
        return Err("GNU pass e GPG precisam estar instalados antes de criar o cofre".into());
    }
    let uid = validated_uid(name, email)?;
    let fingerprint = generate(&uid)?;
    let info = create_vault(path, &fingerprint).map_err(|reason| {
        format!("Chave GPG {fingerprint} criada, mas o cofre não foi concluído: {reason} A chave foi preservada. Corrija o problema e crie o cofre escolhendo esta chave.")
    })?;
    Ok(CreatedKeyVault { info, fingerprint })
}

fn validated_uid(name: &str, email: &str) -> Result<String, String> {
    let name = name.trim();
    let email = email.trim();
    if name.is_empty()
        || name.len() > 100
        || name.chars().any(|c| c.is_control() || "<>()".contains(c))
    {
        return Err("Nome inválido para a chave GPG".into());
    }
    if email.len() > 254
        || email
            .bytes()
            .any(|c| !c.is_ascii() || c.is_ascii_control() || c == b' ')
    {
        return Err("E-mail inválido para a chave GPG".into());
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Err("E-mail inválido para a chave GPG".into());
    };
    if local.is_empty()
        || domain.is_empty()
        || domain.contains('@')
        || !domain.contains('.')
        || domain.starts_with('-')
        || domain.ends_with('-')
        || !email
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._+@-".contains(&c))
    {
        return Err("E-mail inválido para a chave GPG".into());
    }
    Ok(format!("{name} <{email}>"))
}

fn generate_key(uid: &str) -> Result<String, String> {
    let output = Command::new("gpg")
        .args([
            "--batch",
            "--pinentry-mode",
            "ask",
            "--status-fd",
            "1",
            "--quick-generate-key",
            uid,
            "default",
            "default",
            "never",
        ])
        .output()
        .map_err(|_| "Não foi possível iniciar GPG".to_string())?;
    let fingerprint = parse_created_fingerprint(&output.stdout);
    if !output.status.success() {
        return Err(match fingerprint {
            Some(id) => format!("GPG não concluiu a geração. Verifique o pinentry. A chave {id} pode ter sido criada; confira suas chaves privadas antes de tentar novamente."),
            None => "GPG não criou a chave. Verifique o pinentry e tente novamente.".into(),
        });
    }
    fingerprint.ok_or("GPG concluiu sem informar a impressão digital. Confira as chaves privadas antes de tentar novamente.".into())
}

fn parse_created_fingerprint(output: &[u8]) -> Option<String> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| line.strip_prefix("[GNUPG:] KEY_CREATED "))
        .filter_map(|line| line.split_whitespace().last())
        .find(|id| key_id_is_valid(id))
        .map(str::to_owned)
}

pub fn vault_info() -> Result<VaultInfo, String> {
    let (path, source) = active_vault_path()?;
    let recipients = if is_initialized(&path) {
        fs::read_to_string(path.join(".gpg-id")).unwrap_or_default()
    } else {
        String::new()
    };
    let recipient_ids = recipients
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect();
    Ok(VaultInfo {
        path: path.to_string_lossy().into_owned(),
        source: source.into(),
        initialized: is_initialized(&path),
        recipient_ids,
        gpg_keys: gpg_keys(),
        pass_installed: available("pass", "--version"),
        gpg_installed: available("gpg", "--version"),
    })
}

fn available(program: &str, arg: &str) -> bool {
    Command::new(program)
        .arg(arg)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn key_id_is_valid(id: &str) -> bool {
    (id.len() == 40 || id.len() == 64) && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn gpg_keys() -> Vec<GpgKey> {
    let Ok(output) = Command::new("gpg")
        .args([
            "--batch",
            "--with-colons",
            "--fingerprint",
            "--list-secret-keys",
        ])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    parse_gpg_keys(&output.stdout)
}

fn parse_gpg_keys(output: &[u8]) -> Vec<GpgKey> {
    let mut keys = Vec::new();
    let mut current: Option<usize> = None;
    for line in String::from_utf8_lossy(output).lines() {
        let fields: Vec<_> = line.split(':').collect();
        match fields.first().copied() {
            Some("sec") => {
                let validity = fields.get(1).copied().unwrap_or("");
                let capabilities = fields.get(11).copied().unwrap_or("");
                let usable = !matches!(validity, "r" | "e" | "d" | "i")
                    && capabilities.contains('E')
                    && !capabilities.contains('D');
                current = if usable {
                    keys.push(GpgKey {
                        id: String::new(),
                        label: String::new(),
                    });
                    Some(keys.len() - 1)
                } else {
                    None
                };
            }
            Some("fpr") => {
                if let Some(index) = current {
                    if keys[index].id.is_empty() {
                        keys[index].id = fields.get(9).unwrap_or(&"").to_string();
                    }
                }
            }
            Some("uid") => {
                if let Some(index) = current {
                    if keys[index].label.is_empty() {
                        keys[index].label = fields.get(9).unwrap_or(&"").to_string();
                    }
                }
            }
            _ => {}
        }
    }
    keys.retain(|key| key_id_is_valid(&key.id));
    for key in &mut keys {
        if key.label.is_empty() {
            key.label = key.id.clone();
        }
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;

    #[test]
    fn clones_existing_vault_without_overwriting_directory() {
        let root = env::temp_dir().join(format!(
            "passage-clone-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source");
        fs::create_dir(&source).unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.name", "Passage Test"],
            vec!["config", "user.email", "test@example.invalid"],
        ] {
            assert!(Command::new("git")
                .args(args)
                .current_dir(&source)
                .status()
                .unwrap()
                .success());
        }
        fs::write(source.join(".gpg-id"), b"test-key\n").unwrap();
        fs::write(source.join("entry.gpg"), b"ciphertext").unwrap();
        assert!(Command::new("git")
            .args(["add", ".gpg-id", "entry.gpg"])
            .current_dir(&source)
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&source)
            .output()
            .unwrap()
            .status
            .success());
        let target = root.join("restored");
        let cloned = clone_vault_to(&target, source.to_str().unwrap()).unwrap();
        assert_eq!(cloned, target);
        assert_eq!(fs::read(target.join("entry.gpg")).unwrap(), b"ciphertext");
        assert!(clone_vault_to(&target, source.to_str().unwrap()).is_err());
        assert_eq!(fs::read(target.join("entry.gpg")).unwrap(), b"ciphertext");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn key_ids_reject_options_and_short_ids() {
        assert!(key_id_is_valid(&"A".repeat(40)));
        assert!(!key_id_is_valid("--output=/tmp/file"));
        assert!(!key_id_is_valid("12345678"));
    }

    #[test]
    fn validates_identity_before_key_creation() {
        assert!(validated_uid("Alice", "alice@example.com").is_ok());
        for (name, email) in [
            ("", "a@example.com"),
            ("Alice\nBob", "a@example.com"),
            ("Alice", "bad"),
            ("Alice", "x@example.com --output"),
        ] {
            assert!(validated_uid(name, email).is_err());
        }
        assert_eq!(
            parse_created_fingerprint(
                b"[GNUPG:] KEY_CREATED B 0123456789ABCDEF0123456789ABCDEF01234567\n"
            ),
            Some("0123456789ABCDEF0123456789ABCDEF01234567".into())
        );
    }

    #[test]
    fn gpg_parser_only_returns_usable_encryption_keys() {
        fn record(validity: &str, capabilities: &str, fingerprint: &str) -> String {
            let mut fields = vec![""; 12];
            fields[0] = "sec";
            fields[1] = validity;
            fields[11] = capabilities;
            format!(
                "{}\nfpr:::::::::{}:\nuid:::::::::Test Key:\n",
                fields.join(":"),
                fingerprint
            )
        }
        let usable = "0123456789ABCDEF0123456789ABCDEF01234567";
        let other = "89ABCDEF0123456789ABCDEF0123456789ABCDEF";
        let listing = [
            record("u", "scESC", usable),
            record("u", "scSC", other),
            record("r", "scESC", other),
            record("e", "scESC", other),
            record("d", "scESC", other),
            record("i", "scESC", other),
            record("u", "scESCD", other),
        ]
        .join("");
        let keys = parse_gpg_keys(listing.as_bytes());
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].id, usable);
    }

    #[test]
    fn selecting_requires_initialized_store() {
        let root = env::temp_dir().join(format!("passage-select-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        assert!(select_vault(root.to_str().unwrap()).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn create_and_select_with_disposable_gpg_home() {
        let root = env::temp_dir().join(format!("passage-setup-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("gnupg")).unwrap();
        fs::set_permissions(root.join("gnupg"), {
            use std::os::unix::fs::PermissionsExt;
            fs::Permissions::from_mode(0o700)
        })
        .unwrap();
        let result = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "setup::tests::create_and_select_child",
                "--nocapture",
            ])
            .env("PASSAGE_SETUP_TEST_CHILD", "1")
            .env("GNUPGHOME", root.join("gnupg"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("HOME", &root)
            .env_remove("PASSWORD_STORE_DIR")
            .output()
            .unwrap();
        let _ = fs::remove_dir_all(&root);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
    }

    #[test]
    fn create_and_select_child() {
        if env::var_os("PASSAGE_SETUP_TEST_CHILD").is_none() {
            return;
        }
        let home = PathBuf::from(env::var_os("HOME").unwrap());
        let output = Command::new("gpg")
            .args([
                "--batch",
                "--passphrase",
                "",
                "--quick-generate-key",
                "Passage Test <passage-test@example.invalid>",
                "default",
                "default",
                "1d",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let keys = gpg_keys();
        assert_eq!(keys.len(), 1);
        let sign_only = Command::new("gpg")
            .args([
                "--batch",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "",
                "--quick-generate-key",
                "Sign Only <sign-only@example.invalid>",
                "ed25519",
                "sign",
                "1d",
            ])
            .output()
            .unwrap();
        assert!(
            sign_only.status.success(),
            "{}",
            String::from_utf8_lossy(&sign_only.stderr)
        );
        assert_eq!(
            gpg_keys().len(),
            1,
            "Chave apenas de assinatura não deve ser oferecida"
        );
        let listing = Command::new("gpg")
            .args([
                "--batch",
                "--with-colons",
                "--fingerprint",
                "--list-secret-keys",
                "sign-only@example.invalid",
            ])
            .output()
            .unwrap();
        assert!(listing.status.success());
        let fingerprint = String::from_utf8_lossy(&listing.stdout)
            .lines()
            .find(|line| line.starts_with("fpr:"))
            .and_then(|line| line.split(':').nth(9))
            .unwrap()
            .to_owned();
        let sign_store = home.join("sign-only-store");
        assert!(create_vault(sign_store.to_str().unwrap(), &fingerprint).is_err());
        assert!(!sign_store.exists());
        let store = home.join("new-store");
        let info = create_vault(store.to_str().unwrap(), &keys[0].id).unwrap();
        assert_eq!(info.source, "selection");
        assert!(info.initialized);
        assert_eq!(info.recipient_ids, [keys[0].id.clone()]);
        assert_eq!(Vault::open_env().unwrap().root(), store.as_path());
        assert!(create_vault(store.to_str().unwrap(), &keys[0].id).is_err());
        assert_eq!(
            select_vault(store.to_str().unwrap()).unwrap().path,
            store.to_str().unwrap()
        );
        let second = home.join("generated-store");
        let generated = create_key_and_vault_with(
            second.to_str().unwrap(),
            "New Key",
            "new@example.invalid",
            |uid| {
                let result = Command::new("gpg")
                    .args([
                        "--batch",
                        "--pinentry-mode",
                        "loopback",
                        "--passphrase",
                        "",
                        "--status-fd",
                        "1",
                        "--quick-generate-key",
                        uid,
                        "default",
                        "default",
                        "1d",
                    ])
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                Ok(parse_created_fingerprint(&result.stdout).unwrap())
            },
        )
        .unwrap();
        assert!(generated.info.initialized);
        assert_eq!(generated.info.recipient_ids, [generated.fingerprint]);
        assert!(second.join(".gpg-id").is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(settings_path().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}
