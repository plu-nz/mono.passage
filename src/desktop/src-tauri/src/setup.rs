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

pub fn create_vault(path: &str, key_id: &str) -> Result<VaultInfo, String> {
    require_selection_allowed()?;
    if !key_id_is_valid(key_id)
        || !gpg_keys()
            .iter()
            .any(|key| key.id.eq_ignore_ascii_case(key_id))
    {
        return Err("Selecione a impressão digital de uma chave GPG privada disponível".into());
    }
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
    let mut keys = Vec::new();
    let mut current: Option<usize> = None;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let fields: Vec<_> = line.split(':').collect();
        match fields.first().copied() {
            Some("sec") => {
                keys.push(GpgKey {
                    id: String::new(),
                    label: String::new(),
                });
                current = Some(keys.len() - 1);
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
    fn key_ids_reject_options_and_short_ids() {
        assert!(key_id_is_valid(&"A".repeat(40)));
        assert!(!key_id_is_valid("--output=/tmp/file"));
        assert!(!key_id_is_valid("12345678"));
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
