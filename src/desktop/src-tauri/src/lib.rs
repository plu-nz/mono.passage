pub mod git;
mod pairing;
pub mod vault;

use git::{GitStatus, GitVault};
use pairing::{approve_pairing, list_pairings, revoke_pairing};
use serde::Serialize;
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
        Ok(_) => VaultState::Available,
        Err(reason) => VaultState::Unavailable { reason },
    }
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
            list_entries,
            read_entry,
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
