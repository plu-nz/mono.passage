import { createSignal, createEffect, createMemo, onCleanup, onMount } from 'solid-js';
import { render } from 'solid-js/web';
import { invoke } from './service';
import type { VaultState } from '@passage/core';
import './style.css';

type Entry = { id: string; title: string };
type Secret = { password: string; details: string; other_details: string; username: string | null; urls: string[]; ciphertext_hash: string };
type GitStatus = { branch: string; upstream: string | null; dirty: boolean; ahead: number; behind: number };
type GithubStatus = { cli_installed: boolean; login: string | null; vault_git_configured: boolean };
type Pairing = { origin: string; installationId: string };
type GpgKey = { id: string; label: string };
type VaultInfo = { path: string; source: string; initialized: boolean; gpgKeys: GpgKey[]; recipientIds: string[]; passInstalled: boolean; gpgInstalled: boolean };
type CreatedVault = { info: VaultInfo; fingerprint: string };
type TotpCode = { code: string; period: number; remaining: number; validUntil: number };

function displayDetails(details: string): string {
  return details.split('\n').map(line => /^\s*(?:otpauth\s*:\s*)?otpauth:\/\//i.test(line) ? 'TOTP: configurado' : line).join('\n');
}

function App() {
  const [state, setState] = createSignal<VaultState>({ kind: 'unavailable', reason: 'Verificando o ambiente local…' });
  const [entries, setEntries] = createSignal<Entry[]>([]);
  const [query, setQuery] = createSignal('');
  const [activeTab, setActiveTab] = createSignal<'vault' | 'sync' | 'extensions' | 'settings'>('vault');
  const [selected, setSelected] = createSignal<string | null>(null);
  const [secret, setSecret] = createSignal<Secret | null>(null);
  const [error, setError] = createSignal('');
  const [visible, setVisible] = createSignal(false);
  const [editing, setEditing] = createSignal(false);
  const [creating, setCreating] = createSignal(false);
  const [entryId, setEntryId] = createSignal('');
  const [entryPassword, setEntryPassword] = createSignal('');
  const [entryUsername, setEntryUsername] = createSignal('');
  const [entryUrl, setEntryUrl] = createSignal('');
  const [entryDetails, setEntryDetails] = createSignal('');
  const [entryBusy, setEntryBusy] = createSignal(false);
  const [git, setGit] = createSignal<GitStatus | null>(null);
  const [gitError, setGitError] = createSignal('');
  const [gitRemote, setGitRemote] = createSignal('');
  const [github, setGithub] = createSignal<GithubStatus | null>(null);
  const [githubError, setGithubError] = createSignal('');
  const [pairings, setPairings] = createSignal<Pairing[]>([]);
  const [pairingError, setPairingError] = createSignal('');
  const [pairingCode, setPairingCode] = createSignal('');
  const [browserLocked, setBrowserLocked] = createSignal<boolean | null>(null);
  const [browserLockError, setBrowserLockError] = createSignal('');
  const [totp, setTotp] = createSignal<TotpCode | null>(null);
  const [vaultInfo, setVaultInfo] = createSignal<VaultInfo | null>(null);
  const [vaultPath, setVaultPath] = createSignal('');
  const [cloneRemote, setCloneRemote] = createSignal('');
  const [keyId, setKeyId] = createSignal('');
  const [vaultError, setVaultError] = createSignal('');
  const [vaultBusy, setVaultBusy] = createSignal(false);
  const [newKeyName, setNewKeyName] = createSignal('');
  const [newKeyEmail, setNewKeyEmail] = createSignal('');
  const [createdFingerprint, setCreatedFingerprint] = createSignal('');
  const requestNumber = { current: 0 };

  const refresh = async () => {
    requestNumber.current += 1;
    setSelected(null);
    setSecret(null);
    setTotp(null);
    setVisible(false);
    setEditing(false);
    setCreating(false);
    try {
      const next = await invoke<VaultState>('vault_state');
      setState(next);
      setEntries(next.kind === 'available' ? await invoke<Entry[]>('list_entries') : []);
      setError('');
    } catch (cause) {
      setError(String(cause));
    }
  };

  onMount(() => { void refresh(); });
  const refreshVaultInfo = async () => {
    try {
      const info = await invoke<VaultInfo>('vault_info');
      setVaultInfo(info);
      setVaultPath(info.path);
      setKeyId(current => current || info.gpgKeys[0]?.id || '');
      setVaultError('');
    } catch (cause) { setVaultError(String(cause)); }
  };
  onMount(() => { void refreshVaultInfo(); });
  const refreshGit = async () => {
    try { setGit(await invoke<GitStatus>('git_status')); setGitError(''); }
    catch (cause) { setGit(null); setGitError(String(cause)); }
  };
  const refreshGithub = async () => {
    try { setGithub(await invoke<GithubStatus>('github_status')); setGithubError(''); }
    catch (cause) { setGithubError(String(cause)); }
  };
  const refreshPairings = async () => {
    try { setPairings(await invoke<Pairing[]>('list_pairings')); setPairingError(''); }
    catch (cause) { setPairingError(String(cause)); }
  };
  onMount(() => { void refreshGit(); void refreshGithub(); void refreshPairings(); });
  const refreshBrowserLock = async () => {
    try { setBrowserLocked(await invoke<boolean>('browser_access_state')); setBrowserLockError(''); }
    catch (cause) { setBrowserLocked(null); setBrowserLockError(String(cause)); }
  };
  onMount(() => { void refreshBrowserLock(); });
  createEffect(() => {
    if (!secret()) return;
    const timer = window.setTimeout(() => { setSecret(null); setTotp(null); setVisible(false); setEditing(false); setEntryPassword(''); setEntryDetails(''); }, 30_000);
    onCleanup(() => window.clearTimeout(timer));
  });
  createEffect(() => {
    if (!totp()) return;
    const timer = window.setTimeout(() => setTotp(null), Math.max(0, totp().validUntil * 1000 - Date.now()));
    onCleanup(() => window.clearTimeout(timer));
  });
  createEffect(() => {
    if (!editing() || !entryPassword() || entryBusy()) return;
    const timer = window.setTimeout(() => { setEditing(false); setCreating(false); setEntryPassword(''); setEntryDetails(''); }, 30_000);
    onCleanup(() => window.clearTimeout(timer));
  });

  async function openEntry(id: string) {
    const request = ++requestNumber.current;
    setSelected(id);
    setSecret(null);
    setTotp(null);
    setVisible(false);
    setEditing(false);
    setCreating(false);
    setError('');
    try {
      const result = await invoke<Secret>('read_entry', { id });
      if (request === requestNumber.current) setSecret(result);
    } catch (cause) {
      if (request === requestNumber.current) setError(String(cause));
    }
  }

  function startCreate() {
    requestNumber.current += 1;
    setSelected(null);
    setSecret(null);
    setVisible(false);
    setCreating(true);
    setEditing(true);
    setEntryId('');
    setEntryPassword('');
    setEntryUsername('');
    setEntryUrl('');
    setEntryDetails('');
    setError('');
  }

  function startEdit() {
    if (!selected() || !secret()) return;
    setEntryId(selected());
    setEntryPassword(secret().password);
    setEntryUsername(secret().username || '');
    setEntryUrl(secret().urls[0] || '');
    setEntryDetails(secret().other_details);
    setCreating(false);
    setEditing(true);
    setVisible(false);
  }

  async function saveEntry() {
    if (!entryId().trim() || !entryPassword() || (!creating() && !secret())) return;
    setEntryBusy(true);
    setError('');
    try {
      const id = entryId().trim();
      await invoke(creating() ? 'create_login_entry' : 'update_login_entry', {
        id, password: entryPassword, username: entryUsername, url: entryUrl,
        details: entryDetails, ...(creating() ? {} : { expectedHash: secret()!.ciphertext_hash })
      });
      setEntryPassword('');
      setEntryDetails('');
      await refresh();
      await openEntry(id);
      await refreshGit();
    } catch (cause) { setError(String(cause)); }
    finally { setEntryBusy(false); }
  }

  async function deleteEntry() {
    if (!selected() || !secret() || entryBusy() || !window.confirm(`Excluir ${selected()} do cofre? Esta ação também poderá ser registrada no Git.`)) return;
    setEntryBusy(true);
    setError('');
    try {
      await invoke('remove_entry', { id: selected, expectedHash: secret().ciphertext_hash });
      await refresh();
      await refreshGit();
    } catch (cause) { setError(String(cause)); }
    finally { setEntryBusy(false); }
  }

  const filtered = createMemo(() => entries().filter(entry => entry.id.toLocaleLowerCase().includes(query().toLocaleLowerCase())));

  async function sync(operation: 'git_pull' | 'git_push') {
    setGitError('');
    try { setGit(await invoke<GitStatus>(operation)); await refresh(); }
    catch (cause) { setGitError(String(cause)); await refreshGit(); }
  }

  async function setupGit(operation: 'git_initialize' | 'git_set_remote' | 'git_publish_new_vault' | 'git_commit_changes' | 'git_merge_preserving_both') {
    setGitError('');
    try {
      setGit(await invoke<GitStatus>(operation, operation === 'git_set_remote' ? { url: gitRemote().trim() } : {}));
      if (operation === 'git_set_remote') setGitRemote('');
      await refreshGithub();
    } catch (cause) { setGitError(String(cause)); await refreshGit(); }
  }

  async function connectGithub() {
    setGithubError('');
    try { await invoke('github_login'); }
    catch (cause) { setGithubError(String(cause)); }
  }

  async function configureGithubVault() {
    setGithubError('');
    try { setGithub(await invoke<GithubStatus>('github_configure_vault')); }
    catch (cause) { setGithubError(String(cause)); }
  }

  async function changeBrowserLock(lock: boolean) {
    setBrowserLockError('');
    try {
      await invoke(lock ? 'lock_browser_access' : 'unlock_browser_access');
      setBrowserLocked(lock);
      if (lock) { requestNumber.current += 1; setSecret(null); setTotp(null); setVisible(false); setEditing(false); setEntryPassword(''); setEntryDetails(''); }
    } catch (cause) { setBrowserLockError(String(cause)); await refreshBrowserLock(); }
  }

  async function showTotp() {
    if (!selected()) return;
    const request = requestNumber.current;
    try {
      const code = await invoke<TotpCode>('generate_totp', { id: selected() });
      if (request === requestNumber.current) { setTotp(code); setError(''); }
    } catch (cause) {
      if (request === requestNumber.current) { setTotp(null); setError(String(cause)); }
    }
  }

  async function approvePairing() {
    setPairingError('');
    try { await invoke('approve_pairing', { code: pairingCode().trim() }); setPairingCode(''); await refreshPairings(); }
    catch (cause) { setPairingError(String(cause)); }
  }

  async function revokePairing(pairing: Pairing) {
    setPairingError('');
    try { await invoke('revoke_pairing', { origin: pairing.origin, installationId: pairing.installationId }); await refreshPairings(); }
    catch (cause) { setPairingError(String(cause)); }
  }

  async function configureVault(operation: 'select_vault' | 'create_vault') {
    setVaultBusy(true);
    setVaultError('');
    try {
      setCreatedFingerprint('');
      const info = await invoke<VaultInfo>(operation, operation === 'create_vault' ? { path: vaultPath().trim(), keyId: keyId() } : { path: vaultPath().trim() });
      setVaultInfo(info);
      setVaultPath(info.path);
      await refresh();
      await refreshGit();
      await refreshGithub();
      await refreshPairings();
    } catch (cause) { setVaultError(String(cause)); }
    finally { setVaultBusy(false); }
  }

  async function createKeyAndVault() {
    setVaultBusy(true);
    setVaultError('');
    setCreatedFingerprint('');
    try {
      const result = await invoke<CreatedVault>('create_key_and_vault', { path: vaultPath().trim(), name: newKeyName().trim(), email: newKeyEmail().trim() });
      setVaultInfo(result.info);
      setVaultPath(result.info.path);
      setKeyId(result.fingerprint);
      setCreatedFingerprint(result.fingerprint);
      await refresh();
      await refreshGit();
      await refreshGithub();
      await refreshPairings();
    } catch (cause) { setVaultError(String(cause)); }
    finally { setVaultBusy(false); }
  }

  async function cloneVault() {
    setVaultBusy(true);
    setVaultError('');
    try {
      const info = await invoke<VaultInfo>('clone_vault', { path: vaultPath().trim(), url: cloneRemote().trim() });
      setVaultInfo(info);
      setVaultPath(info.path);
      setCloneRemote('');
      await refresh();
      await refreshGit();
      await refreshGithub();
      await refreshPairings();
    } catch (cause) { setVaultError(String(cause)); }
    finally { setVaultBusy(false); }
  }

  return <main class="shell">
    <header class="app-header"><div class="brand"><div class="mark" aria-hidden="true">P</div><div><h1>Passage</h1><p>Seu cofre GNU pass</p></div></div><div class="header-status"><span class={state().kind === 'available' ? 'status-dot ready' : 'status-dot'} />{state().kind === 'available' ? 'Cofre disponível' : 'Cofre indisponível'}</div></header>
    <div class="app-body"><div class="context-bar"><span>Cofre <code>{vaultInfo()?.path || 'verificando…'}</code></span><span>{git()?.upstream ? `Git · ↑${git().ahead} ↓${git().behind}` : 'Armazenamento local'}</span></div>
    <nav class="app-tabs" aria-label="Seções"><button class={activeTab() === 'vault' ? 'active' : ''} onClick={() => setActiveTab('vault')}>Cofre</button><button class={activeTab() === 'sync' ? 'active' : ''} onClick={() => setActiveTab('sync')}>Git Sync</button><button class={activeTab() === 'extensions' ? 'active' : ''} onClick={() => setActiveTab('extensions')}>Extensões</button><button class={activeTab() === 'settings' ? 'active' : ''} onClick={() => setActiveTab('settings')}>Configurações</button></nav>
    {activeTab() === 'settings' && <section class="panel vault-panel" aria-label="Configuração do cofre">
      <h2>Cofre</h2>
      <p class="hint">Diretório ativo: <code>{vaultInfo()?.path || 'verificando…'}</code>{vaultInfo()?.source === 'environment' ? ' · definido por PASSWORD_STORE_DIR' : ''}</p>
      <label for="vault-path">Diretório do cofre</label>
      <div class="panel-actions"><input id="vault-path" value={vaultPath()} onInput={event => setVaultPath(event.currentTarget.value)} placeholder="/home/usuario/.password-store" autocomplete="off" disabled={vaultBusy() || vaultInfo()?.source === 'environment'} /><button disabled={vaultBusy() || !vaultPath().trim() || vaultInfo()?.source === 'environment'} onClick={() => void configureVault('select_vault')}>Abrir existente</button></div>
      <div class="panel-actions"><select aria-label="Chave GPG para o novo cofre" value={keyId()} onChange={event => setKeyId(event.currentTarget.value)} disabled={vaultBusy() || vaultInfo()?.source === 'environment'}><option value="">Selecione uma chave GPG</option>{vaultInfo()?.gpgKeys.map(key => <option value={key.id}>{key.label} · {key.id.slice(-16)}</option>)}</select><button disabled={vaultBusy() || !vaultPath().trim() || !keyId() || vaultInfo()?.source === 'environment'} onClick={() => void configureVault('create_vault')}>Criar cofre</button></div>
      <div class="panel-actions"><input aria-label="URL do cofre Git existente" value={cloneRemote()} onInput={event => setCloneRemote(event.currentTarget.value)} placeholder="URL do repositório Git privado" autocomplete="off" disabled={vaultBusy() || vaultInfo()?.source === 'environment'} /><button disabled={vaultBusy() || !vaultPath().trim() || !cloneRemote().trim() || vaultInfo()?.source === 'environment'} onClick={() => void cloneVault()}>Clonar cofre existente</button></div>
      <p class="hint">Para clonar, informe acima um diretório novo e a URL de um remoto com branch main. A chave GPG privada precisa estar disponível neste computador para abrir as entradas.</p>
      <details class="key-creation"><summary>Criar chave GPG e cofre</summary>
        <p>O GPG abrirá uma janela do sistema para definir a frase secreta. O Passage não recebe essa frase. Se desistir, cancele nessa janela.</p>
        <div class="panel-actions"><input aria-label="Nome da nova chave" value={newKeyName()} onInput={event => setNewKeyName(event.currentTarget.value)} placeholder="Seu nome" autocomplete="name" disabled={vaultBusy()} /><input aria-label="Email da nova chave" type="email" value={newKeyEmail()} onInput={event => setNewKeyEmail(event.currentTarget.value)} placeholder="seu@email.com" autocomplete="email" disabled={vaultBusy()} /></div>
        <button disabled={vaultBusy() || !vaultPath().trim() || !newKeyName().trim() || !newKeyEmail().trim() || !vaultInfo()?.passInstalled || !vaultInfo()?.gpgInstalled || vaultInfo()?.source === 'environment'} onClick={() => void createKeyAndVault()}>{vaultBusy() ? 'Aguarde…' : 'Criar chave e cofre'}</button>
      </details>
      {createdFingerprint() && <div class="recovery-notice" role="status"><strong>Chave e cofre criados.</strong><p>Impressão digital: <code>{createdFingerprint()}</code></p><p>Faça uma cópia segura da chave privada e guarde a frase secreta antes de depender deste cofre. O repositório Git armazena os arquivos cifrados, mas não recupera a chave privada perdida.</p></div>}
      {vaultInfo() && <p class="hint">{!vaultInfo().passInstalled ? 'GNU pass não encontrado. ' : ''}{!vaultInfo().gpgInstalled ? 'GPG não encontrado. ' : ''}{vaultInfo().initialized ? `Inicializado para: ${vaultInfo().recipientIds.join(', ') || 'chave não identificada'}.` : 'Cofre ainda não inicializado.'} {vaultInfo().gpgKeys.length === 0 && vaultInfo().gpgInstalled ? 'Nenhuma chave GPG privada encontrada. Crie ou importe uma chave antes de criar o cofre.' : ''}</p>}
      <button onClick={() => void refreshVaultInfo()} disabled={vaultBusy()}>Verificar chaves e cofre</button>
      {vaultError() && <p class="panel-error()" role="alert">{vaultError()}</p>}
    </section>}
    {activeTab() === 'vault' && (state().kind === 'unavailable' ? <section class="notice">{(state() as {kind: "unavailable"; reason: string}).reason}</section> : <>
      <div class="toolbar"><input aria-label="Buscar entradas" placeholder="Buscar no cofre…" value={query()} onInput={event => setQuery(event.currentTarget.value)} /><button onClick={startCreate}>Nova entrada</button><button onClick={() => void refresh()}>Atualizar</button></div>
      <div class="workspace"><nav aria-label="Entradas do cofre">{filtered().length ? filtered().map(entry => <button class={selected() === entry.id ? 'entry selected()' : 'entry'} onClick={() => void openEntry(entry.id)}><span class="entry-icon" aria-hidden="true">{entry.title.slice(0, 2).toUpperCase()}</span><span class="entry-text"><strong>{entry.title}</strong><small>{entry.id}</small></span><span class="entry-chevron" aria-hidden="true">›</span></button>) : <p>Nenhuma entrada encontrada.</p>}</nav>
      <section class="detail" aria-label="Detalhes da entrada">{editing() ? <>
        <h2>{creating() ? 'Nova entrada' : `Editar ${selected()}`}</h2>
        <div class="entry-form">
          <label for="entry-id">Nome ou caminho da entrada</label><input id="entry-id" value={entryId()} onInput={event => setEntryId(event.currentTarget.value)} disabled={!creating() || entryBusy()} placeholder="sites/exemplo.com" autocomplete="off" />
          <label for="entry-password">Senha</label><input id="entry-password" type="password" value={entryPassword()} onInput={event => setEntryPassword(event.currentTarget.value)} disabled={entryBusy()} autocomplete="new-password" />
          <label for="entry-username">Usuário</label><input id="entry-username" value={entryUsername()} onInput={event => setEntryUsername(event.currentTarget.value)} disabled={entryBusy()} autocomplete="off" />
          <label for="entry-url">URL</label><input id="entry-url" type="url" value={entryUrl()} onInput={event => setEntryUrl(event.currentTarget.value)} disabled={entryBusy()} placeholder="https://exemplo.com" autocomplete="url" />
          <label for="entry-details">Outras linhas do pass</label><textarea id="entry-details" value={entryDetails()} onInput={event => setEntryDetails(event.currentTarget.value)} disabled={entryBusy()} rows={6} spellcheck={false} />
          <p class="hint">A primeira linha é a senha. Os campos continuam legíveis pelo GNU pass.</p>
          <div class="form-actions"><button disabled={entryBusy() || !entryId().trim() || !entryPassword()} onClick={() => void saveEntry()}>{entryBusy() ? 'Salvando…' : 'Salvar'}</button><button disabled={entryBusy()} onClick={() => { setEditing(false); setCreating(false); setEntryPassword(''); setEntryDetails(''); }}>Cancelar</button></div>
        </div>
      </> : selected() ? <><div class="detail-heading"><div><small>ENTRADA DO COFRE</small><h2>{selected()}</h2></div><button onClick={startEdit} disabled={!secret()}>Editar</button></div>{secret() ? <>{secret().username && <div class="field-card"><label>Usuário</label><div class="field-value">{secret().username}</div></div>}{secret().urls.length > 0 && <div class="field-card"><label>URL</label><div class="field-value">{secret().urls[0]}</div></div>}<div class="field-card"><label>Senha</label><div class="password"><code>{visible() ? secret().password : '••••••••••••'}</code><button onClick={() => setVisible(!visible())}>{visible() ? 'Ocultar' : 'Mostrar'}</button></div></div>{secret().details && <div class="field-card"><label>Outros campos</label><pre>{displayDetails(secret().details)}</pre></div>}<button disabled={entryBusy()} onClick={() => void deleteEntry()}>Excluir</button><button onClick={() => void showTotp()}>Gerar código TOTP</button>{totp() && <p role="status">Código TOTP: <code>{totp().code}</code> · válido por até {Math.max(0, Math.ceil(totp().validUntil - Date.now() / 1000))} s</p>}<p class="hint">O conteúdo é removido da tela após 30 segundos. O gpg-agent pode continuar com a chave em cache.</p></> : <p>Abra a entrada para ver seu conteúdo.</p>}</> : <p>Selecione uma entrada.</p>}</section></div>
    </>)}
    {error() && <p class="error()" role="alert">{error()}</p>}
    {activeTab() === 'sync' && <section class="panel" aria-label="Sincronização Git">
      <h2>Sincronização Git</h2>
      <h3>GitHub</h3>
      <p>{!github() ? 'Verificando a conta…' : !github().cli_installed ? 'Instale o GitHub CLI (gh) para conectar sua conta.' : github().login ? `Conectado como @${github().login}.` : 'Nenhuma conta do GitHub conectada pelo gh.'} {github()?.vault_git_configured ? 'Este cofre usa a conta do gh para remotos HTTPS do GitHub.' : ''}</p>
      <div class="panel-actions"><button disabled={!github()?.cli_installed} onClick={() => void connectGithub()}>{github()?.login ? 'Trocar conta' : 'Entrar com GitHub'}</button><button onClick={() => void refreshGithub()}>Verificar conta</button><button disabled={!github()?.login || !git()} onClick={() => void configureGithubVault()}>Usar neste cofre</button></div>
      {githubError() && <p class="panel-error()" role="alert">{githubError()}</p>}
      <p class="hint">O login abre o GitHub CLI em um terminal. Depois de concluir no navegador, clique em Verificar conta. A conta é guardada pelo gh; o Passage não armazena o token. Usar neste cofre configura somente o repositório Git local para remotos HTTPS do GitHub.</p>
      {git() ? <p>Branch {git().branch}{git().upstream ? ` · upstream ${git().upstream}` : ' · sem upstream'} · {git().dirty ? 'alterações locais' : 'sem alterações locais'} · ↑{git().ahead} ↓{git().behind}</p> : <p>Este cofre ainda não está pronto para sincronização Git.</p>}
      {!git() && state().kind === 'available' && <button onClick={() => void setupGit('git_initialize')}>Iniciar Git neste cofre</button>}
      {git() && !git().upstream && <><div class="panel-actions"><input aria-label="URL do remoto Git origin" value={gitRemote()} onInput={event => setGitRemote(event.currentTarget.value)} placeholder="URL HTTPS, SSH ou caminho do remoto" autocomplete="off" /><button disabled={!gitRemote().trim()} onClick={() => void setupGit('git_set_remote')}>Adicionar remoto origin</button></div><button onClick={() => void setupGit('git_publish_new_vault')}>Publicar cofre novo</button><p class="hint">Esta ação cria o primeiro commit apenas com arquivos .gpg e .gpg-id e configura o upstream, se o remoto estiver vazio. Para um remoto com histórico, conecte pelo Git. Alterações futuras continuam disponíveis por <code>pass git()</code>.</p></>}
      <div class="panel-actions"><button onClick={() => void refreshGit()}>Verificar</button><button disabled={!git()?.dirty || !git().upstream} onClick={() => void setupGit('git_commit_changes')}>Registrar alterações</button><button disabled={!git()?.upstream} onClick={() => void sync('git_pull')}>Baixar</button><button disabled={!git()?.upstream} onClick={() => void sync('git_push')}>Enviar</button></div>
      {git() && git().ahead > 0 && git().behind > 0 && <><button onClick={() => void setupGit('git_merge_preserving_both')}>Preservar ambas as versões</button><p class="hint">Mescla os commits e, se a mesma entrada .gpg mudou nos dois dispositivos, mantém a versão local e salva a remota como outra entrada cifrada. Conflitos na chave .gpg-id exigem resolução manual. O envio ao remoto continua separado.</p></>}
      {gitError() && <p class="panel-error()" role="alert">{gitError()}</p>}
      <p class="hint">Configure o remoto pelo Git ou por <code>pass git()</code>. Conflitos e alterações locais impedem a sincronização automática.</p>
    </section>}
    {activeTab() === 'extensions' && <section class="panel" aria-label="Extensões autorizadas">
      <h2>Extensões autorizadas</h2>
      <p>{browserLocked() === null ? 'Estado de acesso indisponível.' : browserLocked() ? 'Acesso das extensões bloqueado.' : 'Acesso das extensões liberado.'} O bloqueio não fecha o cache do GPG nem impede o uso de pass no terminal.</p>
      <div class="panel-actions"><button disabled={browserLocked() !== false} onClick={() => void changeBrowserLock(true)}>Bloquear extensões</button><button disabled={browserLocked() !== true} onClick={() => void changeBrowserLock(false)}>Liberar extensões</button><button onClick={() => void refreshBrowserLock()}>Verificar bloqueio</button></div>
      {browserLockError() && <p class="panel-error()" role="alert">{browserLockError()}</p>}
      <p>Digite o código mostrado na extensão para autorizar esta instalação.</p>
      <div class="panel-actions"><input aria-label="Código de pareamento" value={pairingCode()} maxLength={12} onInput={event => setPairingCode(event.currentTarget.value)} placeholder="Código de 12 caracteres" /><button disabled={!/^[a-f0-9]{12}$/i.test(pairingCode().trim())} onClick={() => void approvePairing()}>Autorizar</button></div>
      {pairings().map(pairing => <div class="pairing"><span>{pairing.origin}<small>{pairing.installationId}</small></span><button onClick={() => void revokePairing(pairing)}>Revogar</button></div>)}
      <button onClick={() => void refreshPairings()}>Atualizar lista</button>
      {pairingError() && <p class="panel-error()" role="alert">{pairingError()}</p>}
    </section>}
    </div>
  </main>;
}

render(() => <App />, document.getElementById('root')!);
