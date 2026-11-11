import React from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import type { VaultState } from '@passage/core';
import './style.css';

type Entry = { id: string; title: string };
type Secret = { password: string; details: string };
type GitStatus = { branch: string; upstream: string | null; dirty: boolean; ahead: number; behind: number };
type Pairing = { origin: string; installationId: string };
type GpgKey = { id: string; label: string };
type VaultInfo = { path: string; source: string; initialized: boolean; gpgKeys: GpgKey[]; recipientIds: string[]; passInstalled: boolean; gpgInstalled: boolean };
type CreatedVault = { info: VaultInfo; fingerprint: string };

function App() {
  const [state, setState] = React.useState<VaultState>({ kind: 'unavailable', reason: 'Verificando o ambiente local…' });
  const [entries, setEntries] = React.useState<Entry[]>([]);
  const [query, setQuery] = React.useState('');
  const [selected, setSelected] = React.useState<string | null>(null);
  const [secret, setSecret] = React.useState<Secret | null>(null);
  const [error, setError] = React.useState('');
  const [visible, setVisible] = React.useState(false);
  const [git, setGit] = React.useState<GitStatus | null>(null);
  const [gitError, setGitError] = React.useState('');
  const [pairings, setPairings] = React.useState<Pairing[]>([]);
  const [pairingError, setPairingError] = React.useState('');
  const [pairingCode, setPairingCode] = React.useState('');
  const [vaultInfo, setVaultInfo] = React.useState<VaultInfo | null>(null);
  const [vaultPath, setVaultPath] = React.useState('');
  const [keyId, setKeyId] = React.useState('');
  const [vaultError, setVaultError] = React.useState('');
  const [vaultBusy, setVaultBusy] = React.useState(false);
  const [newKeyName, setNewKeyName] = React.useState('');
  const [newKeyEmail, setNewKeyEmail] = React.useState('');
  const [createdFingerprint, setCreatedFingerprint] = React.useState('');
  const requestNumber = React.useRef(0);

  const refresh = React.useCallback(async () => {
    requestNumber.current += 1;
    setSelected(null);
    setSecret(null);
    setVisible(false);
    try {
      const next = await invoke<VaultState>('vault_state');
      setState(next);
      setEntries(next.kind === 'available' ? await invoke<Entry[]>('list_entries') : []);
      setError('');
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  React.useEffect(() => { void refresh(); }, [refresh]);
  const refreshVaultInfo = React.useCallback(async () => {
    try {
      const info = await invoke<VaultInfo>('vault_info');
      setVaultInfo(info);
      setVaultPath(info.path);
      setKeyId(current => current || info.gpgKeys[0]?.id || '');
      setVaultError('');
    } catch (cause) { setVaultError(String(cause)); }
  }, []);
  React.useEffect(() => { void refreshVaultInfo(); }, [refreshVaultInfo]);
  const refreshGit = React.useCallback(async () => {
    try { setGit(await invoke<GitStatus>('git_status')); setGitError(''); }
    catch (cause) { setGit(null); setGitError(String(cause)); }
  }, []);
  const refreshPairings = React.useCallback(async () => {
    try { setPairings(await invoke<Pairing[]>('list_pairings')); setPairingError(''); }
    catch (cause) { setPairingError(String(cause)); }
  }, []);
  React.useEffect(() => { void refreshGit(); void refreshPairings(); }, [refreshGit, refreshPairings]);
  React.useEffect(() => {
    if (!secret) return;
    const timer = window.setTimeout(() => { setSecret(null); setVisible(false); }, 30_000);
    return () => window.clearTimeout(timer);
  }, [secret]);

  async function openEntry(id: string) {
    const request = ++requestNumber.current;
    setSelected(id);
    setSecret(null);
    setVisible(false);
    setError('');
    try {
      const result = await invoke<Secret>('read_entry', { id });
      if (request === requestNumber.current) setSecret(result);
    } catch (cause) {
      if (request === requestNumber.current) setError(String(cause));
    }
  }

  const filtered = entries.filter(entry => entry.id.toLocaleLowerCase().includes(query.toLocaleLowerCase()));

  async function sync(operation: 'git_pull' | 'git_push') {
    setGitError('');
    try { setGit(await invoke<GitStatus>(operation)); await refresh(); }
    catch (cause) { setGitError(String(cause)); await refreshGit(); }
  }

  async function approvePairing() {
    setPairingError('');
    try { await invoke('approve_pairing', { code: pairingCode.trim() }); setPairingCode(''); await refreshPairings(); }
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
      const info = await invoke<VaultInfo>(operation, operation === 'create_vault' ? { path: vaultPath.trim(), keyId } : { path: vaultPath.trim() });
      setVaultInfo(info);
      setVaultPath(info.path);
      await refresh();
      await refreshGit();
      await refreshPairings();
    } catch (cause) { setVaultError(String(cause)); }
    finally { setVaultBusy(false); }
  }

  async function createKeyAndVault() {
    setVaultBusy(true);
    setVaultError('');
    setCreatedFingerprint('');
    try {
      const result = await invoke<CreatedVault>('create_key_and_vault', { path: vaultPath.trim(), name: newKeyName.trim(), email: newKeyEmail.trim() });
      setVaultInfo(result.info);
      setVaultPath(result.info.path);
      setKeyId(result.fingerprint);
      setCreatedFingerprint(result.fingerprint);
      await refresh();
      await refreshGit();
      await refreshPairings();
    } catch (cause) { setVaultError(String(cause)); }
    finally { setVaultBusy(false); }
  }

  return <main className="shell">
    <header><div className="mark" aria-hidden="true">P</div><div><h1>Passage</h1><p>Seu cofre GNU pass no desktop.</p></div></header>
    <section className="panel vault-panel" aria-label="Configuração do cofre">
      <h2>Cofre</h2>
      <p className="hint">Diretório ativo: <code>{vaultInfo?.path || 'verificando…'}</code>{vaultInfo?.source === 'environment' ? ' · definido por PASSWORD_STORE_DIR' : ''}</p>
      <label htmlFor="vault-path">Diretório do cofre</label>
      <div className="panel-actions"><input id="vault-path" value={vaultPath} onChange={event => setVaultPath(event.target.value)} placeholder="/home/usuario/.password-store" autoComplete="off" disabled={vaultBusy || vaultInfo?.source === 'environment'} /><button disabled={vaultBusy || !vaultPath.trim() || vaultInfo?.source === 'environment'} onClick={() => void configureVault('select_vault')}>Abrir existente</button></div>
      <div className="panel-actions"><select aria-label="Chave GPG para o novo cofre" value={keyId} onChange={event => setKeyId(event.target.value)} disabled={vaultBusy || vaultInfo?.source === 'environment'}><option value="">Selecione uma chave GPG</option>{vaultInfo?.gpgKeys.map(key => <option key={key.id} value={key.id}>{key.label} · {key.id.slice(-16)}</option>)}</select><button disabled={vaultBusy || !vaultPath.trim() || !keyId || vaultInfo?.source === 'environment'} onClick={() => void configureVault('create_vault')}>Criar cofre</button></div>
      <details className="key-creation"><summary>Criar chave GPG e cofre</summary>
        <p>O GPG abrirá uma janela do sistema para definir a frase secreta. O Passage não recebe essa frase. Se desistir, cancele nessa janela.</p>
        <div className="panel-actions"><input aria-label="Nome da nova chave" value={newKeyName} onChange={event => setNewKeyName(event.target.value)} placeholder="Seu nome" autoComplete="name" disabled={vaultBusy} /><input aria-label="Email da nova chave" type="email" value={newKeyEmail} onChange={event => setNewKeyEmail(event.target.value)} placeholder="seu@email.com" autoComplete="email" disabled={vaultBusy} /></div>
        <button disabled={vaultBusy || !vaultPath.trim() || !newKeyName.trim() || !newKeyEmail.trim() || !vaultInfo?.passInstalled || !vaultInfo?.gpgInstalled || vaultInfo?.source === 'environment'} onClick={() => void createKeyAndVault()}>{vaultBusy ? 'Aguarde…' : 'Criar chave e cofre'}</button>
      </details>
      {createdFingerprint && <div className="recovery-notice" role="status"><strong>Chave e cofre criados.</strong><p>Impressão digital: <code>{createdFingerprint}</code></p><p>Faça uma cópia segura da chave privada e guarde a frase secreta antes de depender deste cofre. O repositório Git armazena os arquivos cifrados, mas não recupera a chave privada perdida.</p></div>}
      {vaultInfo && <p className="hint">{!vaultInfo.passInstalled ? 'GNU pass não encontrado. ' : ''}{!vaultInfo.gpgInstalled ? 'GPG não encontrado. ' : ''}{vaultInfo.initialized ? `Inicializado para: ${vaultInfo.recipientIds.join(', ') || 'chave não identificada'}.` : 'Cofre ainda não inicializado.'} {vaultInfo.gpgKeys.length === 0 && vaultInfo.gpgInstalled ? 'Nenhuma chave GPG privada encontrada. Crie ou importe uma chave antes de criar o cofre.' : ''}</p>}
      <button onClick={() => void refreshVaultInfo()} disabled={vaultBusy}>Verificar chaves e cofre</button>
      {vaultError && <p className="panel-error" role="alert">{vaultError}</p>}
    </section>
    {state.kind === 'unavailable' ? <section className="notice">{state.reason}</section> : <>
      <div className="toolbar"><input aria-label="Buscar entradas" placeholder="Buscar no cofre" value={query} onChange={event => setQuery(event.target.value)} /><button onClick={() => void refresh()}>Atualizar</button></div>
      <div className="workspace"><nav aria-label="Entradas do cofre">{filtered.length ? filtered.map(entry => <button className={selected === entry.id ? 'entry selected' : 'entry'} key={entry.id} onClick={() => void openEntry(entry.id)}>{entry.id}</button>) : <p>Nenhuma entrada encontrada.</p>}</nav>
      <section className="detail" aria-label="Detalhes da entrada">{selected ? <><h2>{selected}</h2>{secret ? <><label>Senha</label><div className="password"><code>{visible ? secret.password : '••••••••••••'}</code><button onClick={() => setVisible(!visible)}>{visible ? 'Ocultar' : 'Mostrar'}</button></div>{secret.details && <><label>Outros campos</label><pre>{secret.details}</pre></>}<p className="hint">O conteúdo é removido da tela após 30 segundos. O gpg-agent pode continuar com a chave em cache.</p></> : <p>Abra a entrada para ver seu conteúdo.</p>}</> : <p>Selecione uma entrada.</p>}</section></div>
    </>}
    {error && <p className="error" role="alert">{error}</p>}
    <section className="panel" aria-label="Sincronização Git">
      <h2>Sincronização Git</h2>
      {git ? <p>Branch {git.branch}{git.upstream ? ` · remoto ${git.upstream}` : ' · sem remoto'} · {git.dirty ? 'alterações locais' : 'sem alterações locais'} · ↑{git.ahead} ↓{git.behind}</p> : <p>Este cofre ainda não está pronto para sincronização Git.</p>}
      <div className="panel-actions"><button onClick={() => void refreshGit()}>Verificar</button><button disabled={!git} onClick={() => void sync('git_pull')}>Baixar</button><button disabled={!git} onClick={() => void sync('git_push')}>Enviar</button></div>
      {gitError && <p className="panel-error" role="alert">{gitError}</p>}
      <p className="hint">Configure o remoto pelo Git ou por <code>pass git</code>. Conflitos e alterações locais impedem a sincronização automática.</p>
    </section>
    <section className="panel" aria-label="Extensões autorizadas">
      <h2>Extensões autorizadas</h2>
      <p>Digite o código mostrado na extensão para autorizar esta instalação.</p>
      <div className="panel-actions"><input aria-label="Código de pareamento" value={pairingCode} maxLength={12} onChange={event => setPairingCode(event.target.value)} placeholder="Código de 12 caracteres" /><button disabled={!/^[a-f0-9]{12}$/i.test(pairingCode.trim())} onClick={() => void approvePairing()}>Autorizar</button></div>
      {pairings.map(pairing => <div className="pairing" key={`${pairing.origin}:${pairing.installationId}`}><span>{pairing.origin}<small>{pairing.installationId}</small></span><button onClick={() => void revokePairing(pairing)}>Revogar</button></div>)}
      <button onClick={() => void refreshPairings()}>Atualizar lista</button>
      {pairingError && <p className="panel-error" role="alert">{pairingError}</p>}
    </section>
  </main>;
}

createRoot(document.getElementById('root')!).render(<App />);
