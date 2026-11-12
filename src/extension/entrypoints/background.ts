import { NATIVE_HOST_NAME } from '@passage/core';

type NativeReply = Record<string, unknown>;
type PopupRequest = { type: 'status' | 'pair' | 'pairStatus' | 'list' } | { type: 'fill'; id: string; tabId: number; url: string };

const identifier = () => crypto.randomUUID();

async function installationId(): Promise<string> {
  const stored = await browser.storage.local.get('installationId');
  if (typeof stored.installationId === 'string') return stored.installationId;
  const id = identifier();
  await browser.storage.local.set({ installationId: id });
  return id;
}

async function credentials(): Promise<{ installationId: string; token: string }> {
  const id = await installationId();
  const stored = await browser.storage.local.get('token');
  if (typeof stored.token !== 'string' || !stored.token) throw new Error('Pareie esta extensão primeiro.');
  return { installationId: id, token: stored.token };
}

async function native(request: Record<string, unknown>): Promise<NativeReply> {
  const requestId = identifier();
  const response = await browser.runtime.sendNativeMessage(NATIVE_HOST_NAME, { ...request, requestId }) as NativeReply;
  if (!response || response.requestId !== requestId) throw new Error('Resposta inválida do conector local.');
  if (response.type === 'error') throw new Error(typeof response.message === 'string' ? response.message : 'Falha no conector local.');
  return response;
}

async function activeTab() {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
  if (!tab?.id || !tab.url) throw new Error('Aba ativa indisponível.');
  const url = new URL(tab.url);
  if (url.protocol !== 'https:') throw new Error('O preenchimento requer uma página HTTPS.');
  return { id: tab.id, origin: url.origin, url: tab.url };
}

async function handle(message: PopupRequest): Promise<NativeReply> {
  if (message.type === 'status') {
    const response = await native({ type: 'status', installationId: await installationId() });
    const token = (await browser.storage.local.get('token')).token;
    return { ...response, paired: response.paired === true && typeof token === 'string' && token.length > 0 };
  }
  if (message.type === 'pair') return native({ type: 'pair', installationId: await installationId() });
  if (message.type === 'pairStatus') {
    const response = await native({ type: 'pairStatus', installationId: await installationId() });
    if (response.type === 'paired' && typeof response.token === 'string') await browser.storage.local.set({ token: response.token });
    return response;
  }
  if (message.type === 'list') {
    const tab = await activeTab();
    const response = await native({ type: 'listForOrigin', ...await credentials(), origin: tab.origin });
    return { ...response, tabId: tab.id, url: tab.url };
  }
  if (message.type === 'fill') {
    if (typeof message.id !== 'string' || !message.id || !Number.isInteger(message.tabId) || typeof message.url !== 'string') throw new Error('Entrada inválida.');
    const tab = await activeTab();
    if (tab.id !== message.tabId || tab.url !== message.url) throw new Error('A aba mudou desde a escolha da entrada.');
    const response = await native({ type: 'readForOrigin', ...await credentials(), origin: tab.origin, id: message.id });
    if (response.type !== 'secret' || typeof response.password !== 'string') throw new Error('Resposta de segredo inválida.');
    const current = await browser.tabs.get(tab.id);
    const active = await browser.tabs.query({ active: true, currentWindow: true });
    if (current.url !== tab.url || active[0]?.id !== tab.id) throw new Error('A aba mudou antes do preenchimento.');
    const result = await browser.scripting.executeScript({
      target: { tabId: tab.id },
      func: (url: string, username: string, password: string) => {
        if (location.href !== url || window.top !== window) return false;
        const fields = [...document.querySelectorAll('input')].filter((item): item is HTMLInputElement => item instanceof HTMLInputElement && !item.disabled && !item.readOnly);
        const passwordField = fields.find(item => item.type === 'password');
        if (!passwordField) return false;
        const usernameField = fields.find(item => item !== passwordField && (item.autocomplete === 'username' || item.type === 'email' || item.name.toLowerCase().includes('user')));
        const setValue = (field: HTMLInputElement, value: string) => {
          const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
          setter?.call(field, value);
          field.dispatchEvent(new Event('input', { bubbles: true }));
          field.dispatchEvent(new Event('change', { bubbles: true }));
        };
        if (usernameField && username) setValue(usernameField, username);
        setValue(passwordField, password);
        return true;
      },
      args: [tab.url, typeof response.username === 'string' ? response.username : '', response.password],
    });
    return { type: result[0]?.result ? 'filled' : 'noFields' };
  }
  throw new Error('Pedido desconhecido.');
}

export default defineBackground(() => {
  browser.runtime.onMessage.addListener((message: PopupRequest, sender) => {
    if (sender.id !== browser.runtime.id || !sender.url?.startsWith(browser.runtime.getURL(''))) return;
    return handle(message).catch(error => ({ type: 'error', message: String(error instanceof Error ? error.message : error) }));
  });
});
