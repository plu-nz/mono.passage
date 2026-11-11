type Reply = Record<string, unknown>;
const status = document.getElementById('status')!;
const actions = document.getElementById('actions')!;
const entries = document.getElementById('entries')!;

async function request(message: Record<string, unknown>): Promise<Reply> {
  const response = await browser.runtime.sendMessage(message) as Reply;
  if (response?.type === 'error') throw new Error(String(response.message));
  return response;
}

function button(label: string, action: () => Promise<void>): HTMLButtonElement {
  const element = document.createElement('button');
  element.textContent = label;
  element.addEventListener('click', () => { void action().catch(showError); });
  return element;
}

function showError(error: unknown) {
  status.textContent = error instanceof Error ? error.message : String(error);
}

async function loadEntries() {
  const response = await request({ type: 'list' });
  if (response.type !== 'entries' || !Array.isArray(response.entries)) throw new Error('Resposta inválida do conector.');
  status.textContent = response.entries.length ? 'Escolha uma entrada para preencher.' : 'Nenhuma entrada corresponde a este site.';
  entries.replaceChildren();
  for (const entry of response.entries) {
    if (!entry || typeof entry.id !== 'string' || typeof entry.title !== 'string') continue;
    entries.append(button(`${entry.title}${typeof entry.username === 'string' ? ` · ${entry.username}` : ''}`, async () => {
      const result = await request({ type: 'fill', id: entry.id });
      status.textContent = result.type === 'filled' ? 'Credenciais preenchidas.' : 'Campo de senha não encontrado nesta página.';
    }));
  }
}

async function checkPairing() {
  const response = await request({ type: 'pairStatus' });
  if (response.type === 'paired') {
    actions.replaceChildren();
    try { await loadEntries(); }
    catch (error) { showError(error); offerPairing(); }
  } else status.textContent = 'Aguardando autorização local.';
}

function offerPairing() {
  actions.replaceChildren(button('Solicitar novo pareamento', async () => {
    const pair = await request({ type: 'pair' });
    if (pair.type !== 'pairPending' || typeof pair.code !== 'string') throw new Error('Pareamento indisponível.');
    status.textContent = `Autorize no terminal: passage-native-host approve ${pair.code}`;
    actions.replaceChildren(button('Verificar autorização', checkPairing));
  }));
}

async function initialize() {
  const response = await request({ type: 'status' });
  if (response.type !== 'status') throw new Error('Conector local indisponível.');
  if (response.paired) {
    try { await loadEntries(); }
    catch (error) { showError(error); offerPairing(); }
    return;
  }
  status.textContent = 'Esta extensão ainda não foi pareada.';
  offerPairing();
}

void initialize().catch(showError);
