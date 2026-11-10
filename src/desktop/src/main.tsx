import React from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import type { VaultState } from '@passage/core';
import './style.css';

type Entry = { id: string; title: string };
type Secret = { password: string; details: string };

function App() {
  const [state, setState] = React.useState<VaultState>({ kind: 'unavailable', reason: 'Verificando o ambiente local…' });
  const [entries, setEntries] = React.useState<Entry[]>([]);
  const [query, setQuery] = React.useState('');
  const [selected, setSelected] = React.useState<string | null>(null);
  const [secret, setSecret] = React.useState<Secret | null>(null);
  const [error, setError] = React.useState('');
  const [visible, setVisible] = React.useState(false);
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

  return <main className="shell">
    <header><div className="mark" aria-hidden="true">P</div><div><h1>Passage</h1><p>Seu cofre GNU pass no desktop.</p></div></header>
    {state.kind === 'unavailable' ? <section className="notice">{state.reason}</section> : <>
      <div className="toolbar"><input aria-label="Buscar entradas" placeholder="Buscar no cofre" value={query} onChange={event => setQuery(event.target.value)} /><button onClick={() => void refresh()}>Atualizar</button></div>
      <div className="workspace"><nav aria-label="Entradas do cofre">{filtered.length ? filtered.map(entry => <button className={selected === entry.id ? 'entry selected' : 'entry'} key={entry.id} onClick={() => void openEntry(entry.id)}>{entry.id}</button>) : <p>Nenhuma entrada encontrada.</p>}</nav>
      <section className="detail" aria-label="Detalhes da entrada">{selected ? <><h2>{selected}</h2>{secret ? <><label>Senha</label><div className="password"><code>{visible ? secret.password : '••••••••••••'}</code><button onClick={() => setVisible(!visible)}>{visible ? 'Ocultar' : 'Mostrar'}</button></div>{secret.details && <><label>Outros campos</label><pre>{secret.details}</pre></>}<p className="hint">O conteúdo é removido da tela após 30 segundos. O gpg-agent pode continuar com a chave em cache.</p></> : <p>Abra a entrada para ver seu conteúdo.</p>}</> : <p>Selecione uma entrada.</p>}</section></div>
    </>}
    {error && <p className="error" role="alert">{error}</p>}
  </main>;
}

createRoot(document.getElementById('root')!).render(<App />);
