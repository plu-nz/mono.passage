// Run with: node src/extension/tests/browser-smoke.mjs BROWSER HOST INSTALLER
// Uses a disposable profile and vault; never opens the user's own browser profile.
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync, cpSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import https from 'node:https';

const [browserBin, hostBin, installer] = process.argv.slice(2);
if (!browserBin || !hostBin || !installer) throw new Error('Provide browser, host, and installer paths');
const extensionDir = resolve('src/extension/.output/chrome-mv3');
if (!existsSync(join(extensionDir, 'manifest.json'))) throw new Error('Run pnpm build:extension first');
const root = mkdtempSync(join(tmpdir(), 'passage-browser-test-'));
// The copy gives each run a fresh extension ID while preserving shipped permissions.
const testExtensionDir = join(root, 'extension');
cpSync(extensionDir, testExtensionDir, { recursive: true });
const config = join(root, 'config');
const home = join(root, 'home');
const store = join(root, 'store');
const bin = join(root, 'bin');
for (const path of [config, home, store, bin]) mkdirSync(path, { recursive: true });

const cert = join(root, 'cert.pem');
const key = join(root, 'key.pem');
const openssl = spawnSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', key, '-out', cert, '-days', '1', '-subj', '/CN=localhost'], { stdio: 'ignore' });
if (openssl.status !== 0) throw new Error('OpenSSL certificate generation failed');
const server = https.createServer({ cert: readFileSync(cert), key: readFileSync(key) }, (_req, res) => {
  res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
  res.end('<!doctype html><title>Passage test</title><input id="user" autocomplete="username"><input id="password" type="password">');
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const site = `https://localhost:${server.address().port}`;
writeFileSync(join(store, '.gpg-id'), 'test-key\n');
writeFileSync(join(store, 'test.gpg'), 'ciphertext');
writeFileSync(join(bin, 'pass'), `#!/bin/sh\nif [ "$1" = show ]; then printf 'test-password\\nlogin: tester\\nurl: ${site}/login\\n'; exit 0; fi\nexit 1\n`, { mode: 0o700 });
const env = { ...process.env, HOME: home, XDG_CONFIG_HOME: config, PASSWORD_STORE_DIR: store, PATH: bin + ':' + process.env.PATH };
const child = spawn(browserBin, ['--headless=new', '--no-sandbox', '--disable-gpu', '--no-first-run', '--ignore-certificate-errors', '--remote-allow-origins=*', '--remote-debugging-port=0', '--user-data-dir=' + join(root, 'profile'), '--disable-extensions-except=' + testExtensionDir, '--load-extension=' + testExtensionDir, site], { env, stdio: 'ignore' });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

async function cdp(ws, method, params = {}) {
  const id = Math.floor(Math.random() * 1e9);
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`CDP timeout: ${method}`)), 10000);
    const listener = event => {
      const message = JSON.parse(event.data);
      if (message.id !== id) return;
      ws.removeEventListener('message', listener);
      clearTimeout(timer);
      if (message.error) reject(new Error(JSON.stringify(message.error)));
      else resolve(message.result);
    };
    ws.addEventListener('message', listener);
    ws.send(JSON.stringify({ id, method, params }));
  });
}

async function connect(url) {
  const ws = new WebSocket(url);
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
  return ws;
}

async function evaluate(ws, expression) {
  const result = await cdp(ws, 'Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
  return result.result.value;
}

try {
  let port;
  for (let i = 0; i < 100; i++) {
    if (existsSync(join(root, 'profile', 'DevToolsActivePort'))) {
      port = Number(readFileSync(join(root, 'profile', 'DevToolsActivePort'), 'utf8').split('\n')[0]);
      break;
    }
    if (child.exitCode !== null) throw new Error(`Browser exited: ${child.exitCode}`);
    await delay(100);
  }
  if (!port) throw new Error('Browser CDP endpoint unavailable');
  const targets = () => fetch(`http://127.0.0.1:${port}/json/list`).then(response => response.json());
  const browserInfo = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
  const browserWs = await connect(browserInfo.webSocketDebuggerUrl);
  let loadedExtensionId;
  for (let i = 0; i < 100; i++) {
    const installed = await cdp(browserWs, 'Extensions.getExtensions');
    loadedExtensionId = installed.extensions.find(item => item.name === 'Passage')?.id;
    if (loadedExtensionId) break;
    await delay(100);
  }
  if (!loadedExtensionId) throw new Error('The browser did not load Passage; use Brave or Chrome for Testing');
  const siteTarget = await cdp(browserWs, 'Target.createTarget', { url: site });
  await cdp(browserWs, 'Target.activateTarget', { targetId: siteTarget.targetId });
  let worker, page;
  for (let i = 0; i < 100; i++) {
    const current = await targets();
    worker = current.find(target => target.type === 'service_worker' && target.url === `chrome-extension://${loadedExtensionId}/background.js`);
    if (worker) break;
    await delay(100);
  }
  if (!worker) throw new Error(`Extension worker unavailable: ${JSON.stringify({ loadedExtensionId, targets: (await targets()).map(t => ({ type: t.type, url: t.url })) })}`);
  for (let i = 0; i < 100; i++) {
    page = (await targets()).find(target => target.id === siteTarget.targetId && target.url.startsWith(site));
    if (page) break;
    await delay(100);
  }
  if (!page) throw new Error('Test page unavailable');
  const extensionId = new URL(worker.url).hostname;
  const installed = spawnSync(installer, [hostBin, extensionId, extensionId], { env, encoding: 'utf8' });
  if (installed.status !== 0) throw new Error(installed.stderr);
  const chromeManifest = join(config, 'google-chrome', 'NativeMessagingHosts', 'com.plunz.passage.json');
  for (const dir of [join(home, '.config', 'google-chrome', 'NativeMessagingHosts'), join(home, '.config', 'google-chrome-for-testing', 'NativeMessagingHosts'), join(config, 'chromium', 'NativeMessagingHosts'), join(config, 'google-chrome-for-testing', 'NativeMessagingHosts'), join(root, 'profile', 'NativeMessagingHosts')]) {
    mkdirSync(dir, { recursive: true });
    cpSync(chromeManifest, join(dir, 'com.plunz.passage.json'));
  }
  const workerWs = await connect(worker.webSocketDebuggerUrl);
  const native = payload => evaluate(workerWs, `new Promise((resolve,reject)=>chrome.runtime.sendNativeMessage('com.plunz.passage',${JSON.stringify(payload)},r=>chrome.runtime.lastError?reject(chrome.runtime.lastError.message):resolve(r)))`);
  const pending = await native({ type: 'pair', requestId: 'pair', installationId: 'smoke' });
  if (pending.type !== 'pairPending') throw new Error('Pair request failed');
  const approved = spawnSync(hostBin, ['approve', pending.code], { env, encoding: 'utf8' });
  if (approved.status !== 0) throw new Error(approved.stderr);
  const paired = await native({ type: 'pairStatus', requestId: 'paired', installationId: 'smoke' });
  if (paired.type !== 'paired') throw new Error('Pairing failed');
  await evaluate(workerWs, `chrome.storage.local.set({installationId:'smoke',token:${JSON.stringify(paired.token)}})`);
  const infos = await cdp(browserWs, 'Target.getTargets', { filter: [{}] });
  const tabTarget = infos.targetInfos.find(info => info.type === 'tab' && info.url.startsWith(site));
  if (!tabTarget) throw new Error('Tab target unavailable for extension action');
  await cdp(browserWs, 'Extensions.triggerAction', { id: extensionId, targetId: tabTarget.targetId });
  let popup;
  for (let i = 0; i < 100; i++) {
    popup = (await targets()).find(target => target.url === `chrome-extension://${extensionId}/popup.html`);
    if (popup?.webSocketDebuggerUrl) break;
    await delay(100);
  }
  if (!popup) throw new Error(`Popup unavailable after action: ${JSON.stringify((await targets()).map(t => ({ type: t.type, url: t.url })))}`);
  const popupWs = await connect(popup.webSocketDebuggerUrl);
  for (let i = 0; i < 100; i++) {
    if (await evaluate(popupWs, `document.querySelector('#entries button')?.textContent || ''`)) break;
    await delay(100);
  }
  const entry = await evaluate(popupWs, `document.querySelector('#entries button')?.textContent || ''`);
  if (!entry.includes('tester')) {
    const status = await evaluate(popupWs, `document.querySelector('#status')?.textContent || ''`);
    const tabs = await evaluate(workerWs, `chrome.tabs.query({active:true,currentWindow:true}).then(t=>t.map(x=>({id:x.id,url:x.url})))`);
    throw new Error(`Popup did not list matching login: ${JSON.stringify({ status, tabs })}`);
  }
  await evaluate(popupWs, `document.querySelector('#entries button').click(); true`);
  for (let i = 0; i < 100; i++) {
    if ((await evaluate(popupWs, `document.querySelector('#status')?.textContent || ''`)).includes('preenchidas')) break;
    await delay(100);
  }
  const pageWs = await connect(page.webSocketDebuggerUrl);
  const values = await evaluate(pageWs, `({user:document.querySelector('#user').value,password:document.querySelector('#password').value})`);
  if (values.user !== 'tester' || values.password !== 'test-password') {
    const status = await evaluate(popupWs, `document.querySelector('#status')?.textContent || ''`);
    const tabs = await evaluate(workerWs, `chrome.tabs.query({active:true,currentWindow:true}).then(t=>t.map(x=>({id:x.id,url:x.url})))`);
    throw new Error(`Form was not filled: ${JSON.stringify({ values, status, tabs })}`);
  }
  await cdp(pageWs, 'Page.navigate', { url: `${site}/other` });
  for (let i = 0; i < 100; i++) {
    if (await evaluate(pageWs, `location.pathname === '/other'`)) break;
    await delay(100);
  }
  await evaluate(popupWs, `document.querySelector('#entries button').click(); true`);
  for (let i = 0; i < 100; i++) {
    if ((await evaluate(popupWs, `document.querySelector('#status')?.textContent || ''`)).includes('mudou')) break;
    await delay(100);
  }
  const afterNavigation = await evaluate(pageWs, `({user:document.querySelector('#user').value,password:document.querySelector('#password').value})`);
  const rejected = await evaluate(popupWs, `document.querySelector('#status')?.textContent || ''`);
  if (afterNavigation.user || afterNavigation.password || !rejected.includes('mudou')) throw new Error('Navigation was not rejected');
  console.log(JSON.stringify({ browser: browserBin, extensionId, paired: true, popupListed: true, filled: true, navigationRejected: true }));
  popupWs.close(); browserWs.close(); workerWs.close(); pageWs.close();
} finally {
  child.kill('SIGTERM');
  await new Promise(resolve => server.close(resolve));
  await delay(500);
  rmSync(root, { recursive: true, force: true });
}
