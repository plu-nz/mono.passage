import React from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import type { VaultState } from '@passage/core';
import './style.css';

function App() {
  const [state, setState] = React.useState<VaultState>({
    kind: 'unavailable',
    reason: 'Verificando o ambiente local…',
  });

  React.useEffect(() => {
    invoke<VaultState>('vault_state')
      .then(setState)
      .catch(() => setState({
        kind: 'unavailable',
        reason: 'Execute o aplicativo pelo Tauri para verificar o ambiente local.',
      }));
  }, []);

  return (
    <main className="shell">
      <div className="mark" aria-hidden="true">P</div>
      <p className="eyebrow">Passage · protótipo inicial</p>
      <h1>Seu cofre, em qualquer caminho.</h1>
      <p className="intro">Uma interface para GNU pass. O terminal continua funcionando enquanto construímos a integração com seus navegadores.</p>
      <section className="card" aria-label="Estado do cofre">
        <span className="dot" />
        <div>
          <strong>{state.kind === 'unavailable' ? 'Cofre ainda não conectado' : state.kind === 'locked' ? 'Cofre bloqueado' : 'Cofre aberto'}</strong>
          {state.kind === 'unavailable' && <p>{state.reason}</p>}
        </div>
      </section>
      <p className="footnote">Próximo passo: ler um cofre de teste e conectar a extensão.</p>
    </main>
  );
}

createRoot(document.getElementById('root')!).render(<App />);
