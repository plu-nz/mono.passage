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
  if (response.type !== 'entries' || !Array.isArray(response.entries) || !Number.isInteger(response.tabId) || typeof response.url !== 'string') throw new Error('Invalid host response.');
  status.textContent = response.entries.length ? 'Choose an entry to fill.' : 'No entry matches this site.';
  entries.replaceChildren();
  for (const entry of response.entries) {
    if (!entry || typeof entry.id !== 'string' || typeof entry.title !== 'string') continue;
    entries.append(button(`${entry.title}${typeof entry.username === 'string' ? ` · ${entry.username}` : ''}`, async () => {
      const result = await request({ type: 'fill', id: entry.id, tabId: response.tabId, url: response.url });
      status.textContent = result.type === 'filled' ? 'Credentials filled.'
        : result.type === 'usernameOnly' ? 'Username filled. Continue to the password step and click Passage again.'
        : result.type === 'ambiguous' ? 'Ambiguous login fields found. No credentials were filled.'
        : 'No suitable login field was found on this page.';
    }));
  }
}

async function checkPairing() {
  const response = await request({ type: 'pairStatus' });
  if (response.type === 'paired') {
    actions.replaceChildren();
    try { await loadEntries(); }
    catch (error) { showError(error); offerPairing(); }
  } else status.textContent = 'Waiting for local approval.';
}

function offerPairing() {
  actions.replaceChildren(button('Request new pairing', async () => {
    const pair = await request({ type: 'pair' });
    if (pair.type !== 'pairPending' || typeof pair.code !== 'string') throw new Error('Pairing unavailable.');
    status.textContent = `Approve in a terminal: passage-native-host approve ${pair.code}`;
    actions.replaceChildren(button('Check approval', checkPairing));
  }));
}

async function initialize() {
  const response = await request({ type: 'status' });
  if (response.type !== 'status') throw new Error('Local host unavailable.');
  if (response.locked === true) {
    status.textContent = 'Extension access is locked in Passage.';
    actions.replaceChildren(button('Check again', initialize));
    entries.replaceChildren();
    return;
  }
  if (response.paired) {
    try { await loadEntries(); }
    catch (error) { showError(error); offerPairing(); }
    return;
  }
  status.textContent = 'This extension is not paired yet.';
  offerPairing();
}

void initialize().catch(showError);
