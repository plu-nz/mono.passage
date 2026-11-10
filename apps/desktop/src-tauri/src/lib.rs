use serde::Serialize;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum VaultState {
    Unavailable { reason: String },
}

#[tauri::command]
fn vault_state() -> VaultState {
    VaultState::Unavailable {
        reason: "A integração com GNU pass será implementada na próxima etapa.".into(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![vault_state])
        .run(tauri::generate_context!())
        .expect("failed to run Passage");
}
