pub mod git;
mod pairing;
pub mod setup;
pub mod vault;

use git::{GitStatus, GitVault};
use pairing::{approve_pairing, list_pairings, revoke_pairing};
use serde::Serialize;
use setup::VaultInfo;
use std::process::Command;
use vault::{Entry, Secret, Vault};

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum VaultState {
    Unavailable { reason: String },
    Available,
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
    match Vault::open_env() {
        Ok(vault) if setup::is_initialized(vault.root()) => VaultState::Available,
        Ok(_) => VaultState::Unavailable {
            reason: "O diretório ainda não foi inicializado com GNU pass (.gpg-id ausente).".into(),
        },
        Err(reason) => VaultState::Unavailable { reason },
    }
}

#[tauri::command]
fn vault_info() -> Result<VaultInfo, String> {
    setup::vault_info()
}

#[tauri::command]
fn gpg_keys() -> Vec<setup::GpgKey> {
    setup::gpg_keys()
}

#[tauri::command]
fn select_vault(path: String) -> Result<VaultInfo, String> {
    setup::select_vault(&path)
}

#[tauri::command]
fn create_vault(path: String, key_id: String) -> Result<VaultInfo, String> {
    setup::create_vault(&path, &key_id)
}

#[tauri::command]
fn create_key_and_vault(
    path: String,
    name: String,
    email: String,
) -> Result<setup::CreatedKeyVault, String> {
    setup::create_key_and_vault(&path, &name, &email)
}

#[tauri::command]
fn list_entries() -> Result<Vec<Entry>, String> {
    Vault::open_env()?.list()
}

#[tauri::command]
fn read_entry(id: String) -> Result<Secret, String> {
    Vault::open_env()?.read(&id)
}

#[tauri::command]
fn create_login_entry(
    id: String,
    password: String,
    username: String,
    url: String,
    details: String,
) -> Result<Secret, String> {
    Vault::open_env()?.create_login(&id, &password, &username, &url, &details)
}

#[tauri::command]
fn update_login_entry(
    id: String,
    password: String,
    username: String,
    url: String,
    details: String,
    expected_hash: String,
) -> Result<Secret, String> {
    Vault::open_env()?.update_login(&id, &password, &username, &url, &details, &expected_hash)
}

#[tauri::command]
fn create_entry(id: String, content: String) -> Result<(), String> {
    Vault::open_env()?.create(&id, &content)
}

#[tauri::command]
fn update_entry(id: String, content: String, expected_hash: String) -> Result<(), String> {
    Vault::open_env()?.update(&id, &content, &expected_hash)
}

#[tauri::command]
fn remove_entry(id: String, expected_hash: String) -> Result<(), String> {
    Vault::open_env()?.remove(&id, &expected_hash)
}

#[tauri::command]
fn git_status() -> Result<GitStatus, String> {
    GitVault::open(Vault::open_env()?.root())?.status()
}

#[tauri::command]
fn git_pull() -> Result<GitStatus, String> {
    let vault = Vault::open_env()?;
    let git = GitVault::open(vault.root())?;
    git.pull()?;
    git.status()
}

#[tauri::command]
fn git_push() -> Result<GitStatus, String> {
    let vault = Vault::open_env()?;
    let git = GitVault::open(vault.root())?;
    git.push()?;
    git.status()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            vault_state,
            vault_info,
            gpg_keys,
            select_vault,
            create_vault,
            create_key_and_vault,
            list_entries,
            read_entry,
            create_login_entry,
            update_login_entry,
            create_entry,
            update_entry,
            remove_entry,
            git_status,
            git_pull,
            git_push,
            list_pairings,
            approve_pairing,
            revoke_pairing
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Passage");
}
