use fs2::FileExt;
use passage_desktop_lib::vault::Vault;
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::{self, Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

const MAX_MESSAGE: usize = 64 * 1024;
const PAIR_TTL: u64 = 300;

#[derive(Default, Serialize, Deserialize)]
struct State {
    pending: Vec<Pending>,
    authorized: Vec<Authorized>,
    #[serde(default)]
    locked: bool,
}
#[derive(Serialize, Deserialize)]
struct Pending {
    origin: String,
    installation_id: String,
    code: String,
    expires_at: u64,
    approved: bool,
    #[serde(default)]
    vault_path: Option<PathBuf>,
}
#[derive(Serialize, Deserialize)]
struct Authorized {
    origin: String,
    installation_id: String,
    token_hash: String,
    #[serde(default)]
    vault_path: Option<PathBuf>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random_hex(bytes: usize) -> String {
    let mut data = vec![0u8; bytes];
    OsRng.fill_bytes(&mut data);
    data.iter().map(|b| format!("{b:02x}")).collect()
}
fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn config_dir() -> Result<PathBuf, String> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .ok_or("HOME indisponível")?;
    let dir = base.join("passage");
    if fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Diretório de configuração não pode ser link simbólico".into());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    Ok(dir)
}
fn with_state<T>(f: impl FnOnce(&mut State) -> Result<T, String>) -> Result<T, String> {
    let dir = config_dir()?;
    let lock_path = dir.join("native-host.lock");
    if fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Arquivo de lock inválido".into());
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(lock_path)
        .map_err(|e| e.to_string())?;
    lock.lock_exclusive().map_err(|e| e.to_string())?;
    let path = dir.join("native-host.json");
    if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Estado local inválido".into());
    }
    let mut state: State = if path.exists() {
        serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Estado local inválido: {e}"))?
    } else {
        State::default()
    };
    let before = serde_json::to_vec(&state).map_err(|e| e.to_string())?;
    state.pending.retain(|p| p.expires_at > now());
    let result = f(&mut state);
    let data = serde_json::to_vec(&state).map_err(|e| e.to_string())?;
    if data == before {
        return result;
    }
    let tmp = dir.join(format!("native-host-{}.tmp", random_hex(8)));
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|e| e.to_string())?;
    output.write_all(&data).map_err(|e| e.to_string())?;
    output.sync_all().map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    result
}
fn extension_origin(value: &str) -> Result<String, String> {
    let url = Url::parse(value).map_err(|_| "Origem da extensão inválida")?;
    if url.scheme() != "chrome-extension"
        || url.host_str().is_none()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Origem da extensão inválida".into());
    }
    Ok(value.to_owned())
}
fn site_origin(value: &str) -> Result<String, String> {
    let url = Url::parse(value).map_err(|_| "Origem do site inválida")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.username() != ""
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Origem do site inválida".into());
    }
    Ok(url.origin().ascii_serialization())
}
fn matches_site(urls: &[String], origin: &str) -> bool {
    urls.iter()
        .filter_map(|s| Url::parse(s).ok())
        .any(|u| u.origin().ascii_serialization() == origin && u.scheme() == "https")
}
fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or_else(|| format!("Campo {key} inválido"))
}
fn authorized(state: &State, extension: &str, id: &str, token: &str, vault_path: &Path) -> bool {
    let digest = hash(token);
    state.authorized.iter().any(|a| {
        a.origin == extension
            && a.installation_id == id
            && a.token_hash == digest
            && a.vault_path.as_deref() == Some(vault_path)
    })
}
fn can_access(state: &State, extension: &str, id: &str, token: &str, vault_path: &Path) -> bool {
    !state.locked && authorized(state, extension, id, token, vault_path)
}

fn approve_pending(state: &mut State, code: &str) -> Result<String, String> {
    let pending = state
        .pending
        .iter_mut()
        .find(|p| p.code == code && p.expires_at > now())
        .ok_or("Código inválido ou expirado")?;
    pending.approved = true;
    Ok(format!(
        "{} ({}) — {}",
        pending.origin,
        pending.installation_id,
        pending
            .vault_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "cofre antigo".into())
    ))
}

fn revoke_authorized(state: &mut State, origin: &str, id: &str) -> bool {
    let before = state.authorized.len();
    state
        .authorized
        .retain(|a| !(a.origin == origin && a.installation_id == id));
    before != state.authorized.len()
}
fn handle(request: &Value, extension: &str) -> Result<Value, String> {
    let kind = field(request, "type")?;
    let request_id = field(request, "requestId")?;
    let reply = match kind {
        "ping" => json!({"type":"pong","version":1}),
        "status" => {
            let id = field(request, "installationId")?;
            let vault = Vault::open_env()?;
            let (paired, locked) = with_state(|s| {
                Ok((
                    s.authorized.iter().any(|a| {
                        a.origin == extension
                            && a.installation_id == id
                            && a.vault_path.as_deref() == Some(vault.root())
                    }),
                    s.locked,
                ))
            })?;
            json!({"type":"status","version":1,"paired":paired,"locked":locked})
        }
        "pair" => {
            let id = field(request, "installationId")?;
            let vault = Vault::open_env()?;
            let vault_path = vault.root().to_path_buf();
            let (code, expires_at) = with_state(|s| {
                s.pending
                    .retain(|p| !(p.origin == extension && p.installation_id == id));
                let code = random_hex(6);
                let expires_at = now() + PAIR_TTL;
                s.pending.push(Pending {
                    origin: extension.into(),
                    installation_id: id.into(),
                    code: code.clone(),
                    expires_at,
                    approved: false,
                    vault_path: Some(vault_path),
                });
                Ok((code, expires_at))
            })?;
            json!({"type":"pairPending","code":code,"expiresAt":expires_at})
        }
        "pairStatus" => {
            let id = field(request, "installationId")?;
            let vault = Vault::open_env()?;
            let result = with_state(|s| {
                if let Some(pos) = s.pending.iter().position(|p| {
                    p.origin == extension
                        && p.installation_id == id
                        && p.approved
                        && p.vault_path.as_deref() == Some(vault.root())
                }) {
                    s.pending.remove(pos);
                    let token = random_hex(32);
                    s.authorized
                        .retain(|a| !(a.origin == extension && a.installation_id == id));
                    s.authorized.push(Authorized {
                        origin: extension.into(),
                        installation_id: id.into(),
                        token_hash: hash(&token),
                        vault_path: Some(vault.root().to_path_buf()),
                    });
                    Ok(Some(token))
                } else {
                    Ok(None)
                }
            })?;
            match result {
                Some(token) => json!({"type":"paired","token":token}),
                None => json!({"type":"pairPending"}),
            }
        }
        "listForOrigin" | "readForOrigin" => {
            let id = field(request, "installationId")?;
            let token = field(request, "token")?;
            let origin = site_origin(field(request, "origin")?)?;
            let vault = Vault::open_env()?;
            if !with_state(|s| Ok(can_access(s, extension, id, token, vault.root())))? {
                return Err("Extensão não autorizada para este cofre".into());
            }
            if kind == "listForOrigin" {
                let entries: Vec<_> = vault
                    .list()?
                    .into_iter()
                    .filter_map(|e| {
                        let secret = vault.read(&e.id).ok()?;
                        matches_site(&secret.urls, &origin)
                            .then(|| json!({"id":e.id,"title":e.title,"username":secret.username}))
                    })
                    .collect();
                if !with_state(|s| Ok(can_access(s, extension, id, token, vault.root())))? {
                    return Err("Acesso ao cofre bloqueado ou revogado".into());
                }
                json!({"type":"entries","entries":entries})
            } else {
                let entry_id = field(request, "id")?;
                let entry = vault
                    .list()?
                    .into_iter()
                    .find(|e| e.id == entry_id)
                    .ok_or("Entrada indisponível para este site")?;
                let secret = vault.read(entry_id)?;
                if !matches_site(&secret.urls, &origin) {
                    return Err("Entrada indisponível para este site".into());
                }
                if !with_state(|s| Ok(can_access(s, extension, id, token, vault.root())))? {
                    return Err("Acesso ao cofre bloqueado ou revogado".into());
                }
                json!({"type":"secret","id":entry_id,"username":secret.username.or(entry.username),"password":secret.password})
            }
        }
        _ => return Err("Pedido desconhecido".into()),
    };
    let mut reply = reply;
    reply["requestId"] = Value::String(request_id.to_owned());
    Ok(reply)
}
fn read_message(input: &mut impl Read) -> io::Result<Option<Value>> {
    let mut size = [0u8; 4];
    match input.read_exact(&mut size) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let size = u32::from_le_bytes(size) as usize;
    if size == 0 || size > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Mensagem muito grande",
        ));
    }
    let mut data = vec![0; size];
    input.read_exact(&mut data)?;
    serde_json::from_slice(&data)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
fn write_message(output: &mut impl Write, value: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Resposta muito grande",
        ));
    }
    output.write_all(&(bytes.len() as u32).to_le_bytes())?;
    output.write_all(&bytes)?;
    output.flush()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) == Some("list") {
        for item in with_state(|s| {
            Ok(s.authorized
                .iter()
                .map(|a| format!("{} {}", a.origin, a.installation_id))
                .collect::<Vec<_>>())
        })? {
            println!("{item}");
        }
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("list-json") {
        let vault = Vault::open_env()?;
        let items = with_state(|s| {
            Ok(s.authorized
                .iter()
                .filter(|a| a.vault_path.as_deref() == Some(vault.root()))
                .map(|a| json!({"origin":a.origin,"installationId":a.installation_id}))
                .collect::<Vec<_>>())
        })?;
        println!("{}", serde_json::to_string(&items)?);
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("lock-status") {
        let locked = with_state(|s| Ok(s.locked))?;
        println!("{}", json!({"locked":locked}));
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("lock") {
        with_state(|s| {
            s.locked = true;
            Ok(())
        })?;
        println!("Acesso das extensões bloqueado.");
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("unlock") {
        with_state(|s| {
            s.locked = false;
            Ok(())
        })?;
        println!("Acesso das extensões liberado.");
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("revoke") {
        let origin = extension_origin(
            args.get(2)
                .ok_or("Uso: passage-native-host revoke ORIGEM ID")?,
        )?;
        let id = args
            .get(3)
            .ok_or("Uso: passage-native-host revoke ORIGEM ID")?;
        let removed = with_state(|s| Ok(revoke_authorized(s, &origin, id)))?;
        if !removed {
            return Err("Instalação não encontrada".into());
        }
        println!("Acesso revogado.");
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("approve") {
        let code = args
            .get(2)
            .ok_or("Uso: passage-native-host approve CODIGO")?;
        let approved = with_state(|s| approve_pending(s, code))?;
        println!("Pareamento aprovado: {approved}");
        return Ok(());
    }
    let extension = extension_origin(args.get(1).ok_or("Origem da extensão ausente")?)?;
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    while let Some(request) = read_message(&mut input)? {
        let request_id = request
            .get("requestId")
            .and_then(Value::as_str)
            .unwrap_or("");
        let response = handle(&request, &extension).unwrap_or_else(
            |_| json!({"type":"error","requestId":request_id,"message":"Pedido recusado"}),
        );
        write_message(&mut output, &response)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_uses_selected_vault_and_env_override() {
        let base = env::temp_dir().join(format!("passage-host-settings-{}", random_hex(8)));
        let config = base.join("config");
        let selected = base.join("selected");
        let override_store = base.join("override");
        fs::create_dir_all(config.join("passage")).unwrap();
        fs::create_dir_all(&selected).unwrap();
        fs::create_dir_all(&override_store).unwrap();
        fs::write(
            config.join("passage/settings.json"),
            serde_json::to_vec(&json!({"vaultPath": selected})).unwrap(),
        )
        .unwrap();

        let old_config = env::var_os("XDG_CONFIG_HOME");
        let old_store = env::var_os("PASSWORD_STORE_DIR");
        env::set_var("XDG_CONFIG_HOME", &config);
        env::remove_var("PASSWORD_STORE_DIR");
        assert_eq!(
            Vault::open_env().unwrap().root(),
            selected.canonicalize().unwrap()
        );
        env::set_var("PASSWORD_STORE_DIR", &override_store);
        assert_eq!(
            Vault::open_env().unwrap().root(),
            override_store.canonicalize().unwrap()
        );
        match old_config {
            Some(value) => env::set_var("XDG_CONFIG_HOME", value),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }
        match old_store {
            Some(value) => env::set_var("PASSWORD_STORE_DIR", value),
            None => env::remove_var("PASSWORD_STORE_DIR"),
        }
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn rejects_lookalike_domains() {
        let urls = vec!["https://example.com/login".into()];
        assert!(matches_site(&urls, "https://example.com"));
        assert!(!matches_site(&urls, "https://example.com.evil.test"));
        assert!(!matches_site(&urls, "http://example.com"));
    }
    #[test]
    fn validates_origin() {
        assert!(site_origin("https://example.com").is_ok());
        assert!(site_origin("https://example.com.evil.test").is_ok());
        assert!(site_origin("https://example.com/path").is_err());
        assert!(site_origin("javascript:alert(1)").is_err());
    }
    #[test]
    fn framing_round_trip() {
        let value = json!({"type":"ping","requestId":"1"});
        let mut bytes = Vec::new();
        write_message(&mut bytes, &value).unwrap();
        assert_eq!(read_message(&mut bytes.as_slice()).unwrap(), Some(value));
    }

    #[test]
    fn pairing_requires_approval_and_revocation_blocks_token() {
        let origin = "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/";
        let id = "installation-1";
        let vault_path = Path::new("/example/vault-a");
        let mut state = State::default();
        state.pending.push(Pending {
            origin: origin.into(),
            installation_id: id.into(),
            code: "test-code".into(),
            expires_at: now() + 60,
            approved: false,
            vault_path: Some(vault_path.to_path_buf()),
        });
        assert!(approve_pending(&mut state, "wrong-code").is_err());
        assert!(!state.pending[0].approved);
        assert!(approve_pending(&mut state, "test-code").is_ok());
        assert!(state.pending[0].approved);
        let token = "secret-token";
        state.authorized.push(Authorized {
            origin: origin.into(),
            installation_id: id.into(),
            token_hash: hash(token),
            vault_path: Some(vault_path.to_path_buf()),
        });
        assert!(authorized(&state, origin, id, token, vault_path));
        assert!(!authorized(
            &state,
            origin,
            id,
            token,
            Path::new("/example/vault-b")
        ));
        assert!(!authorized(&state, origin, id, "wrong-token", vault_path));
        assert!(!authorized(
            &state,
            origin,
            "other-installation",
            token,
            vault_path
        ));
        assert!(can_access(&state, origin, id, token, vault_path));
        state.locked = true;
        assert!(!can_access(&state, origin, id, token, vault_path));
        state.locked = false;
        assert!(can_access(&state, origin, id, token, vault_path));
        assert!(revoke_authorized(&mut state, origin, id));
        assert!(!can_access(&state, origin, id, token, vault_path));
    }
}
