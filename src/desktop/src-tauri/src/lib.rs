use serde::Serialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum VaultState {
    Unavailable { reason: String },
    Available,
}

#[derive(Serialize)]
struct VaultEntry {
    id: String,
    title: String,
}

#[derive(Serialize)]
struct Secret {
    password: String,
    details: String,
}

fn store_dir() -> Result<PathBuf, String> {
    if let Some(dir) = env::var_os("PASSWORD_STORE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = env::var_os("HOME").ok_or("HOME não está definido")?;
    Ok(PathBuf::from(home).join(".password-store"))
}

fn require_store() -> Result<PathBuf, String> {
    let dir = store_dir()?;
    if !dir.is_dir() {
        return Err(format!("Cofre não encontrado em {}", dir.display()));
    }
    Ok(dir)
}

fn collect_entries(root: &Path, dir: &Path, entries: &mut Vec<VaultEntry>) -> Result<(), String> {
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
            let id = relative.with_extension("").to_string_lossy().into_owned();
            let title = relative
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            entries.push(VaultEntry { id, title });
        }
    }
    Ok(())
}

#[tauri::command]
fn vault_state() -> VaultState {
    if !Command::new("pass")
        .arg("--version")
        .output()
        .is_ok_and(|result| result.status.success())
    {
        return VaultState::Unavailable {
            reason: "GNU pass não está instalado ou não está no PATH.".into(),
        };
    }
    match require_store() {
        Ok(_) => VaultState::Available,
        Err(reason) => VaultState::Unavailable { reason },
    }
}

#[tauri::command]
fn list_entries() -> Result<Vec<VaultEntry>, String> {
    let root = require_store()?;
    let mut entries = Vec::new();
    collect_entries(&root, &root, &mut entries)?;
    entries.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));
    Ok(entries)
}

#[tauri::command]
fn read_entry(id: String) -> Result<Secret, String> {
    let root = require_store()?;
    let relative = Path::new(&id);
    if id.is_empty()
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("Identificador de entrada inválido".into());
    }
    let file = root.join(relative).with_extension("gpg");
    let canonical_root = root.canonicalize().map_err(|error| error.to_string())?;
    let canonical_file = file.canonicalize().map_err(|error| error.to_string())?;
    if !canonical_file.starts_with(&canonical_root) || !canonical_file.is_file() {
        return Err("Entrada fora do cofre".into());
    }
    let result = Command::new("pass")
        .arg("show")
        .arg(&id)
        .output()
        .map_err(|error| format!("Falha ao iniciar pass: {error}"))?;
    if !result.status.success() {
        return Err(
            "Não foi possível abrir a entrada. Verifique a chave GPG e o gpg-agent.".into(),
        );
    }
    let content =
        String::from_utf8(result.stdout).map_err(|_| "A entrada não contém texto UTF-8")?;
    let (password, details) = content.split_once('\n').unwrap_or((&content, ""));
    Ok(Secret {
        password: password.to_owned(),
        details: details.trim_end().to_owned(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            vault_state,
            list_entries,
            read_entry
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Passage");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_gpg_files_and_ignores_symlinks_and_git() {
        let root = env::temp_dir().join(format!("passage-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sites")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join("sites/example.com.gpg"), b"encrypted").unwrap();
        fs::write(root.join(".git/hidden.gpg"), b"encrypted").unwrap();
        fs::write(root.join("notes.txt"), b"plain").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("sites/example.com.gpg"), root.join("alias.gpg"))
            .unwrap();
        let mut entries = Vec::new();
        collect_entries(&root, &root, &mut entries).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "sites/example.com");
        fs::remove_dir_all(root).unwrap();
    }
}
