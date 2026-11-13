import { createSignal, createEffect, createMemo, onCleanup, onMount, type JSX } from 'solid-js';
import { render } from 'solid-js/web';
import { invoke } from './service';
import { version as appVersion } from '../package.json';
import type { VaultState } from '@passage/core';
import './style.css';
import './theme';

type Entry = { id: string; title: string };
type Secret = { password: string; details: string; other_details: string; username: string | null; urls: string[]; ciphertext_hash: string };
type GitStatus = { branch: string; upstream: string | null; dirty: boolean; ahead: number; behind: number };
type GithubStatus = { cli_installed: boolean; login: string | null; vault_git_configured: boolean };
type Pairing = { origin: string; installationId: string };

type Tab = 'vault' | 'sync' | 'settings';

// Bottom navigation: icon above label, the active tab in the accent color.
const tabs: { id: Tab; label: string; icon: () => JSX.Element }[] = [
  { id: 'vault', label: 'Vault', icon: () => <><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V7a4 4 0 0 1 8 0v4" /></> },
  { id: 'sync', label: 'Git Sync', icon: () => <><path d="M20 11a8 8 0 0 0-14.9-3.9L4 8" /><path d="M4 4v4h4" /><path d="M4 13a8 8 0 0 0 14.9 3.9L20 16" /><path d="M20 20v-4h-4" /></> },
  { id: 'settings', label: 'Settings', icon: () => <><path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12" /><circle cx="16" cy="6" r="2" /><circle cx="10" cy="12" r="2" /><circle cx="18" cy="18" r="2" /></> },
];

// Mirrors setup.RefreshChoices on the Go side.
const refreshChoices: [number, string][] = [[0, 'Off'], [5, 'Every 5 seconds'], [15, 'Every 15 seconds'], [30, 'Every 30 seconds'], [60, 'Every minute'], [300, 'Every 5 minutes']];
type GpgKey = { id: string; label: string };
type VaultInfo = { path: string; source: string; initialized: boolean; gpgKeys: GpgKey[]; recipientIds: string[]; passInstalled: boolean; gpgInstalled: boolean };
type CreatedVault = { info: VaultInfo; fingerprint: string };
type TotpCode = { code: string; period: number; remaining: number; validUntil: number };

function displayDetails(details: string): string {
  return details.split('\n').map(line => /^\s*(?:otpauth\s*:\s*)?otpauth:\/\//i.test(line) ? 'TOTP: configured' : line).join('\n');
}

function App() {
  const [state, setState] = createSignal<VaultState>({ kind: 'unavailable', reason: 'Checking local environment…' });
  const [entries, setEntries] = createSignal<Entry[]>([]);
  const [query, setQuery] = createSignal('');
  const [activeTab, setActiveTab] = createSignal<Tab>('vault');
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
  const [refreshSeconds, setRefreshSeconds] = createSignal(30);
  const [refreshError, setRefreshError] = createSignal('');
  const [navPosition, setNavPosition] = createSignal<'top' | 'bottom'>('top');
  const [navError, setNavError] = createSignal('');
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

  // Reload the entry list in the background without touching the open entry
  // or a form in progress, so changes made by `pass` or Git show up.
  const reloadEntries = async () => {
    try {
      const next = await invoke<VaultState>('vault_state');
      setState(next);
      const list = next.kind === 'available' ? await invoke<Entry[]>('list_entries') : [];
      setEntries(list);
      const current = selected();
      if (current && !editing() && !list.some(entry => entry.id === current)) {
        requestNumber.current += 1;
        setSelected(null);
        setSecret(null);
        setTotp(null);
        setVisible(false);
      }
    } catch {
      // Keep the last list; the next tick or an explicit action reports errors.
    }
  };
  onMount(async () => {
    try { setRefreshSeconds(await invoke<number>('refresh_seconds')); } catch (cause) { setRefreshError(String(cause)); }
    try { setNavPosition(await invoke<'top' | 'bottom'>('nav_position')); } catch (cause) { setNavError(String(cause)); }
  });
  createEffect(() => {
    const seconds = refreshSeconds();
    if (seconds <= 0) return;
    const timer = setInterval(() => { void reloadEntries(); }, seconds * 1000);
    onCleanup(() => clearInterval(timer));
  });
  const changeRefreshSeconds = async (seconds: number) => {
    setRefreshError('');
    try {
      await invoke('set_refresh_seconds', { seconds });
      setRefreshSeconds(seconds);
    } catch (cause) {
      setRefreshError(String(cause));
    }
  };
  const changeNavPosition = async (position: 'top' | 'bottom') => {
    setNavError('');
    try {
      await invoke('set_nav_position', { position });
      setNavPosition(position);
    } catch (cause) {
      setNavError(String(cause));
    }
  };
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
    if (!selected() || !secret() || entryBusy() || !window.confirm(`Delete ${selected()} from the vault? This change may also be recorded in Git.`)) return;
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

  // Section navigation plus the vault search; at the top the order is mirrored
  // so the navigation always sits at the window edge.
  const appBar = () => {
    const toolbar = activeTab() === 'vault' && state().kind !== 'unavailable' && <div class="toolbar"><input aria-label="Search entries" placeholder="Search vault…" value={query()} onInput={event => setQuery(event.currentTarget.value)} /></div>;
    const nav = <nav class="app-nav" aria-label="Sections">{tabs.map(tab => <button class={activeTab() === tab.id ? 'active' : ''} aria-current={activeTab() === tab.id ? 'page' : undefined} onClick={() => setActiveTab(tab.id)}><svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true">{tab.icon()}</svg><span>{tab.label}</span></button>)}<button class="nav-new" aria-label="New entry" disabled={state().kind === 'unavailable'} onClick={() => { setActiveTab('vault'); startCreate(); }}><svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg><span>New</span></button></nav>;
    return <div class={`app-bar ${navPosition()}`}>{navPosition() === 'top' ? <>{nav}{toolbar}</> : <>{toolbar}{nav}</>}</div>;
  };
  return <main class="shell">
    {navPosition() === 'top' && appBar()}
    <div class="app-body">
    {activeTab() === 'settings' && <section class="panel vault-panel" aria-label="Layout">
      <h2>Layout</h2>
      <label class="check"><input type="checkbox" checked={navPosition() === 'bottom'} onChange={event => void changeNavPosition(event.currentTarget.checked ? 'bottom' : 'top')} />Show navigation at the bottom</label>
      {navError() && <p class="panel-error" role="alert">{navError()}</p>}
    </section>}
    {activeTab() === 'settings' && <section class="panel vault-panel" aria-label="Entry list">
      <h2>Entry list</h2>
      <label for="refresh-seconds">Refresh automatically</label>
      <div class="panel-actions"><select id="refresh-seconds" value={String(refreshSeconds())} onChange={event => void changeRefreshSeconds(Number(event.currentTarget.value))}>
        {refreshChoices.map(([seconds, label]) => <option value={String(seconds)}>{label}</option>)}
      </select></div>
      <p class="hint">Reloads the list so entries added or removed with <code>pass</code> or Git appear without reopening Passage.</p>
      {refreshError() && <p class="panel-error" role="alert">{refreshError()}</p>}
    </section>}
    {activeTab() === 'settings' && <section class="panel vault-panel" aria-label="Vault setup">
      <h2>Vault</h2>
      <p class="status-line"><span class={state().kind === 'available' ? 'status-dot ready' : 'status-dot'} />{state().kind === 'available' ? 'Vault available' : 'Vault unavailable'}</p>
      <p class="hint">Active directory: <code>{vaultInfo()?.path || 'checking…'}</code>{vaultInfo()?.source === 'environment' ? ' · set by PASSWORD_STORE_DIR' : ''}</p>
      <label for="vault-path">Vault directory</label>
      <div class="panel-actions"><input id="vault-path" value={vaultPath()} onInput={event => setVaultPath(event.currentTarget.value)} placeholder="/home/user/.password-store" autocomplete="off" disabled={vaultBusy() || vaultInfo()?.source === 'environment'} /><button disabled={vaultBusy() || !vaultPath().trim() || vaultInfo()?.source === 'environment'} onClick={() => void configureVault('select_vault')}>Open existing</button></div>
      <div class="panel-actions"><select aria-label="GPG key for the new vault" value={keyId()} onChange={event => setKeyId(event.currentTarget.value)} disabled={vaultBusy() || vaultInfo()?.source === 'environment'}><option value="">Select a GPG key</option>{vaultInfo()?.gpgKeys.map(key => <option value={key.id}>{key.label} · {key.id.slice(-16)}</option>)}</select><button disabled={vaultBusy() || !vaultPath().trim() || !keyId() || vaultInfo()?.source === 'environment'} onClick={() => void configureVault('create_vault')}>Create vault</button></div>
      <div class="panel-actions"><input aria-label="Existing Git vault URL" value={cloneRemote()} onInput={event => setCloneRemote(event.currentTarget.value)} placeholder="Private Git repository URL" autocomplete="off" disabled={vaultBusy() || vaultInfo()?.source === 'environment'} /><button disabled={vaultBusy() || !vaultPath().trim() || !cloneRemote().trim() || vaultInfo()?.source === 'environment'} onClick={() => void cloneVault()}>Clone existing vault</button></div>
      <p class="hint">To clone, enter a new directory and the URL of a remote with a main branch. The private GPG key must be available on this computer to open entries.</p>
      <details class="key-creation"><summary>Create GPG key and vault</summary>
        <p>GPG opens a system prompt to set the passphrase. Passage never receives it. Cancel in that prompt if you change your mind.</p>
        <div class="panel-actions"><input aria-label="New key name" value={newKeyName()} onInput={event => setNewKeyName(event.currentTarget.value)} placeholder="Your name" autocomplete="name" disabled={vaultBusy()} /><input aria-label="New key email" type="email" value={newKeyEmail()} onInput={event => setNewKeyEmail(event.currentTarget.value)} placeholder="you@example.com" autocomplete="email" disabled={vaultBusy()} /></div>
        <button disabled={vaultBusy() || !vaultPath().trim() || !newKeyName().trim() || !newKeyEmail().trim() || !vaultInfo()?.passInstalled || !vaultInfo()?.gpgInstalled || vaultInfo()?.source === 'environment'} onClick={() => void createKeyAndVault()}>{vaultBusy() ? 'Please wait…' : 'Create key and vault'}</button>
      </details>
      {createdFingerprint() && <div class="recovery-notice" role="status"><strong>Key and vault created.</strong><p>Fingerprint: <code>{createdFingerprint()}</code></p><p>Back up your private key and passphrase before relying on this vault. Git stores encrypted files but cannot recover a lost private key.</p></div>}
      {vaultInfo() && <p class="hint">{!vaultInfo().passInstalled ? 'GNU pass not found. ' : ''}{!vaultInfo().gpgInstalled ? 'GPG not found. ' : ''}{vaultInfo().initialized ? `Initialized for: ${vaultInfo().recipientIds.join(', ') || 'unknown key'}.` : 'Vault not initialized yet.'} {vaultInfo().gpgKeys.length === 0 && vaultInfo().gpgInstalled ? 'No private GPG key found. Create or import one before creating a vault.' : ''}</p>}
      <button onClick={() => void refreshVaultInfo()} disabled={vaultBusy()}>Check keys and vault</button>
      {vaultError() && <p class="panel-error" role="alert">{vaultError()}</p>}
    </section>}
    {activeTab() === 'settings' && <section class="panel vault-panel" aria-label="Browser extensions">
      <h2>Browser extensions</h2>
      <p>{browserLocked() === null ? 'Access status unavailable.' : browserLocked() ? 'Extension access locked.' : 'Extension access unlocked.'} Locking does not clear the GPG cache or prevent using pass in a terminal.</p>
      <div class="panel-actions"><button disabled={browserLocked() !== false} onClick={() => void changeBrowserLock(true)}>Lock extensions</button><button disabled={browserLocked() !== true} onClick={() => void changeBrowserLock(false)}>Unlock extensions</button><button onClick={() => void refreshBrowserLock()}>Check lock status</button></div>
      {browserLockError() && <p class="panel-error" role="alert">{browserLockError()}</p>}
      <p>Enter the code shown in the extension to authorize this installation.</p>
      <div class="panel-actions"><input aria-label="Pairing code" value={pairingCode()} maxLength={12} onInput={event => setPairingCode(event.currentTarget.value)} placeholder="12-character code" /><button disabled={!/^[a-f0-9]{12}$/i.test(pairingCode().trim())} onClick={() => void approvePairing()}>Authorize</button></div>
      {pairings().map(pairing => <div class="pairing"><span>{pairing.origin}<small>{pairing.installationId}</small></span><button onClick={() => void revokePairing(pairing)}>Revoke</button></div>)}
      <button onClick={() => void refreshPairings()}>Refresh list</button>
      {pairingError() && <p class="panel-error" role="alert">{pairingError()}</p>}
    </section>}
    {activeTab() === 'settings' && <section class="panel vault-panel" aria-label="About">
      <h2>About</h2>
      <p class="hint">Passage {appVersion}</p>
    </section>}
    {activeTab() === 'vault' && (state().kind === 'unavailable' ? <section class="notice">{(state() as {kind: "unavailable"; reason: string}).reason}</section> : <>
      <div class="workspace"><nav aria-label="Vault entries">{filtered().length ? filtered().map(entry => <button class={selected() === entry.id ? 'entry selected' : 'entry'} onClick={() => void openEntry(entry.id)}><span class="entry-icon" aria-hidden="true">{entry.title.slice(0, 2).toUpperCase()}</span><span class="entry-text"><strong>{entry.title}</strong><small>{entry.id}</small></span><span class="entry-chevron" aria-hidden="true">›</span></button>) : <p>No entries found.</p>}</nav>
      <section class="detail" aria-label="Entry details">{editing() ? <>
        <h2>{creating() ? 'New entry' : `Edit ${selected()}`}</h2>
        <div class="entry-form">
          <label for="entry-id">Entry name or path</label><input id="entry-id" value={entryId()} onInput={event => setEntryId(event.currentTarget.value)} disabled={!creating() || entryBusy()} placeholder="sites/example.com" autocomplete="off" />
          <label for="entry-password">Password</label><input id="entry-password" type="password" value={entryPassword()} onInput={event => setEntryPassword(event.currentTarget.value)} disabled={entryBusy()} autocomplete="new-password" />
          <label for="entry-username">Username</label><input id="entry-username" value={entryUsername()} onInput={event => setEntryUsername(event.currentTarget.value)} disabled={entryBusy()} autocomplete="off" />
          <label for="entry-url">URL</label><input id="entry-url" type="url" value={entryUrl()} onInput={event => setEntryUrl(event.currentTarget.value)} disabled={entryBusy()} placeholder="https://example.com" autocomplete="url" />
          <label for="entry-details">Other pass lines</label><textarea id="entry-details" value={entryDetails()} onInput={event => setEntryDetails(event.currentTarget.value)} disabled={entryBusy()} rows={6} spellcheck={false} />
          <p class="hint">The first line is the password. GNU pass can still read the other fields.</p>
          <div class="form-actions"><button disabled={entryBusy() || !entryId().trim() || !entryPassword()} onClick={() => void saveEntry()}>{entryBusy() ? 'Saving…' : 'Save'}</button><button disabled={entryBusy()} onClick={() => { setEditing(false); setCreating(false); setEntryPassword(''); setEntryDetails(''); }}>Cancel</button></div>
        </div>
      </> : selected() ? <><div class="detail-heading"><div><small>VAULT ENTRY</small><h2>{selected()}</h2></div><button onClick={startEdit} disabled={!secret()}>Edit</button></div>{secret() ? <>{secret().username && <div class="field-card"><label>Username</label><div class="field-value">{secret().username}</div></div>}{secret().urls.length > 0 && <div class="field-card"><label>URL</label><div class="field-value">{secret().urls[0]}</div></div>}<div class="field-card"><label>Password</label><div class="password"><code>{visible() ? secret().password : '••••••••••••'}</code><button onClick={() => setVisible(!visible())}>{visible() ? 'Hide' : 'Show'}</button></div></div>{secret().details && <div class="field-card"><label>Other fields</label><pre>{displayDetails(secret().details)}</pre></div>}<button disabled={entryBusy()} onClick={() => void deleteEntry()}>Delete</button><button onClick={() => void showTotp()}>Generate TOTP code</button>{totp() && <p role="status">TOTP code: <code>{totp().code}</code> · valid for up to {Math.max(0, Math.ceil(totp().validUntil - Date.now() / 1000))} s</p>}<p class="hint">The secret disappears from the screen after 30 seconds. gpg-agent may keep the key cached.</p></> : <p>Open the entry to view its contents.</p>}</> : <p>Select an entry.</p>}</section></div>
    </>)}
    {error() && <p class="error" role="alert">{error()}</p>}
    {activeTab() === 'sync' && <section class="panel" aria-label="Git sync">
      <h2>Git sync</h2>
      <h3>GitHub</h3>
      <p>{!github() ? 'Checking account…' : !github().cli_installed ? 'Install GitHub CLI (gh) to connect your account.' : github().login ? `Connected as @${github().login}.` : 'No GitHub account connected through gh.'} {github()?.vault_git_configured ? 'This vault uses your gh account for GitHub HTTPS remotes.' : ''}</p>
      <div class="panel-actions"><button disabled={!github()?.cli_installed} onClick={() => void connectGithub()}>{github()?.login ? 'Switch account' : 'Sign in with GitHub'}</button><button onClick={() => void refreshGithub()}>Check account</button><button disabled={!github()?.login || !git()} onClick={() => void configureGithubVault()}>Use for this vault</button></div>
      {githubError() && <p class="panel-error" role="alert">{githubError()}</p>}
      <p class="hint">Sign-in opens GitHub CLI in a terminal. After completing it in your browser, click Check account. gh manages your account; Passage does not store the token. Use for this vault configures only this local Git repository for GitHub HTTPS remotes.</p>
      {git() ? <p>Branch {git().branch}{git().upstream ? ` · upstream ${git().upstream}` : ' · no upstream'} · {git().dirty ? 'local changes' : 'no local changes'} · ↑{git().ahead} ↓{git().behind}</p> : <p>This vault is not ready for Git sync yet.</p>}
      {!git() && state().kind === 'available' && <button onClick={() => void setupGit('git_initialize')}>Initialize Git in this vault</button>}
      {git() && !git().upstream && <><div class="panel-actions"><input aria-label="Git origin remote URL" value={gitRemote()} onInput={event => setGitRemote(event.currentTarget.value)} placeholder="HTTPS URL, SSH URL, or remote path" autocomplete="off" /><button disabled={!gitRemote().trim()} onClick={() => void setupGit('git_set_remote')}>Add origin remote</button></div><button onClick={() => void setupGit('git_publish_new_vault')}>Publish new vault</button><p class="hint">This creates the first commit using only .gpg and .gpg-id files and sets the upstream if the remote is empty. Connect a remote with existing history through Git. Future changes remain available through  <code>pass git</code>.</p></>}
      <div class="panel-actions"><button onClick={() => void refreshGit()}>Check</button><button disabled={!git()?.dirty || !git().upstream} onClick={() => void setupGit('git_commit_changes')}>Commit changes</button><button disabled={!git()?.upstream} onClick={() => void sync('git_pull')}>Pull</button><button disabled={!git()?.upstream} onClick={() => void sync('git_push')}>Push</button></div>
      {git() && git().ahead > 0 && git().behind > 0 && <><button onClick={() => void setupGit('git_merge_preserving_both')}>Preserve both versions</button><p class="hint">Merge the commits and, when the same .gpg entry changed on both devices, keep the local version and save the remote version as another encrypted entry. .gpg-id conflicts require manual resolution. Pushing remains a separate action.</p></>}
      {gitError() && <p class="panel-error" role="alert">{gitError()}</p>}
      <p class="hint">Configure the remote through Git or  <code>pass git</code>. Conflicts and local changes prevent automatic sync.</p>
    </section>}
    </div>
    {navPosition() === 'bottom' && appBar()}
  </main>;
}

render(() => <App />, document.getElementById('root')!);
