use serde::{Deserialize, Serialize};
use std::{env, path::PathBuf, process::Command};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pairing {
    pub origin: String,
    pub installation_id: String,
}

fn host_binary() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("PASSAGE_NATIVE_HOST_BIN") {
        return Ok(PathBuf::from(path));
    }
    let current = env::current_exe().map_err(|e| e.to_string())?;
    Ok(current.with_file_name("passage-native-host"))
}

fn host_command(args: &[&str]) -> Result<String, String> {
    let result = Command::new(host_binary()?)
        .args(args)
        .output()
        .map_err(|_| "Conector local não instalado ou indisponível".to_owned())?;
    if !result.status.success() {
        return Err("Operação de pareamento recusada pelo conector local".into());
    }
    String::from_utf8(result.stdout).map_err(|_| "Resposta inválida do conector local".into())
}

#[tauri::command]
pub fn list_pairings() -> Result<Vec<Pairing>, String> {
    let output = host_command(&["list-json"])?;
    serde_json::from_str(&output).map_err(|_| "Lista de pareamentos inválida".into())
}

#[tauri::command]
pub fn approve_pairing(code: String) -> Result<(), String> {
    if code.len() != 12 || !code.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Código de pareamento inválido".into());
    }
    host_command(&["approve", &code])?;
    Ok(())
}

#[tauri::command]
pub fn revoke_pairing(origin: String, installation_id: String) -> Result<(), String> {
    if !origin.starts_with("chrome-extension://")
        || !origin.ends_with('/')
        || installation_id.len() > 128
        || installation_id.is_empty()
    {
        return Err("Pareamento inválido".into());
    }
    host_command(&["revoke", &origin, &installation_id])?;
    Ok(())
}

#[tauri::command]
pub fn browser_access_state() -> Result<bool, String> {
    let output = host_command(&["lock-status"])?;
    let value: serde_json::Value = serde_json::from_str(&output)
        .map_err(|_| "Estado inválido do conector local".to_owned())?;
    value
        .get("locked")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| "Estado inválido do conector local".to_owned())
}

#[tauri::command]
pub fn lock_browser_access() -> Result<(), String> {
    host_command(&["lock"])?;
    Ok(())
}

#[tauri::command]
pub fn unlock_browser_access() -> Result<(), String> {
    host_command(&["unlock"])?;
    Ok(())
}
