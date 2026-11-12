// Run with: node src/extension/tests/browser-smoke.mjs BROWSER HOST INSTALLER
// Uses a disposable profile and vault; never opens the user's own browser profile.
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync, cpSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import https from 'node:https';

const [browserBin, hostBin, installer, scenario = 'standard'] = process.argv.slice(2);
if (!browserBin || !hostBin || !installer) throw new Error('Provide browser, host, and installer paths');
const extensionDir = resolve('src/extension/.output/chrome-mv3');
if (!existsSync(join(extensionDir, 'manifest.json'))) throw new Error('Run pnpm build:extension first');
const expectedId = JSON.parse(readFileSync(resolve('src/extension/identity.json'), 'utf8')).id;
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
const forms = {
  standard: '<form><input id="user" autocomplete="username"><input id="password" type="password" autocomplete="current-password"></form>',
  usernameOnly: '<form><input id="user" autocomplete="username"></form>',
  passwordOnly: '<form><input id="password" type="password" autocomplete="current-password"></form>',
  autocompleteOff: '<form><input id="user" name="username" autocomplete="off"><input id="password" type="password" autocomplete="off"></form>',
  ambiguousPassword: '<form><input id="user" autocomplete="username"><input id="password" type="password"><input id="second" type="password"></form>',
  ambiguousUsername: '<form><input id="user" autocomplete="username"><input id="second" autocomplete="username"><input id="password" type="password"></form>',
  hiddenPassword: '<form><input id="user" autocomplete="username"><input id="password" type="password" style="display:none"></form>',
  transparentPassword: '<form><input id="user" autocomplete="username"><input id="password" type="password" style="opacity:0"></form>',
  newPassword: '<form><input id="user" autocomplete="username"><input id="password" type="password" autocomplete="new-password"></form>',
  iframe: '<iframe src="/frame"></iframe>',
  noFields: '<p>No login fields</p>',
  githubLive: '',
  herokuLive: '',
};
if (!(scenario in forms)) throw new Error(`Unknown scenario: ${scenario}`);
const server = https.createServer({ cert: readFileSync(cert), key: readFileSync(key) }, (req, res) => {
  res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
  res.end(`<!doctype html><title>Passage test</title>${req.url === '/frame' ? forms.standard : forms[scenario]}<script>document.addEventListener('input',()=>document.body.dataset.inputs=(Number(document.body.dataset.inputs||0)+1));document.addEventListener('change',()=>document.body.dataset.changes=(Number(document.body.dataset.changes||0)+1));</script>`);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const site = scenario === 'githubLive' ? 'https://github.com/login' : scenario === 'herokuLive' ? 'https://the-internet.herokuapp.com/login' : `https://localhost:${server.address().port}`;
writeFileSync(join(store, '.gpg-id'), 'test-key\n');
writeFileSync(join(store, 'test.gpg'), 'ciphertext');
writeFileSync(join(bin, 'pass'), `#!/bin/sh\nif [ "$1" = show ]; then printf 'test-password\\nlogin: tester\\nurl: ${site}\\n'; exit 0; fi\nexit 1\n`, { mode: 0o700 });
const env = { ...process.env, HOME: home, XDG_CONFIG_HOME: config, PASSWORD_STORE_DIR: store, PATH: bin + ':' + process.env.PATH };
const legacyLoad = browserBin.includes('google-chrome-stable') ? [] : ['--disable-extensions-except=' + testExtensionDir, '--load-extension=' + testExtensionDir];
const child = spawn(browserBin, [...(process.env.PASSAGE_BROWSER_HEADED ? [] : ['--headless=new']), '--no-sandbox', '--disable-gpu', '--no-first-run', '--ignore-certificate-errors', '--enable-unsafe-extension-debugging', '--remote-allow-origins=*', '--remote-debugging-port=0', '--user-data-dir=' + join(root, 'profile'), ...legacyLoad, site], { env, stdio: 'ignore' });
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
  if (!loadedExtensionId) {
    const loaded = await cdp(browserWs, 'Extensions.loadUnpacked', { path: testExtensionDir });
    loadedExtensionId = loaded.id;
  }
  if (!loadedExtensionId) throw new Error('The browser did not load Passage');
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
  if (extensionId !== expectedId) throw new Error(`Extension ID changed: ${extensionId}`);
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
  const expectedStatus = { standard: 'preenchidas', usernameOnly: 'Usuário preenchido', passwordOnly: 'preenchidas', autocompleteOff: 'preenchidas', ambiguousPassword: 'ambíguos', ambiguousUsername: 'ambíguos', hiddenPassword: 'Nenhum campo', transparentPassword: 'Nenhum campo', newPassword: 'Nenhum campo', iframe: 'Nenhum campo', noFields: 'Nenhum campo', githubLive: 'preenchidas', herokuLive: 'preenchidas' }[scenario];
  for (let i = 0; i < 100; i++) {
    if ((await evaluate(popupWs, `document.querySelector('#status')?.textContent || ''`)).includes(expectedStatus)) break;
    await delay(100);
  }
  const pageWs = await connect(page.webSocketDebuggerUrl);
  const userSelector = scenario === 'githubLive' ? '#login_field' : scenario === 'herokuLive' ? '#username' : '#user';
  const passwordSelector = '#password';
  const values = await evaluate(pageWs, `({user:document.querySelector('${userSelector}')?.value || '',password:document.querySelector('${passwordSelector}')?.value || '',inputs:Number(document.body.dataset.inputs||0),changes:Number(document.body.dataset.changes||0)})`);
  const expectedValues = { standard: ['tester', 'test-password'], usernameOnly: ['tester', ''], passwordOnly: ['', 'test-password'], autocompleteOff: ['tester', 'test-password'], githubLive: ['tester', 'test-password'], herokuLive: ['tester', 'test-password'] }[scenario] || ['', ''];
  const eventsExpected = ['standard', 'usernameOnly', 'passwordOnly', 'autocompleteOff'].includes(scenario);
  if (values.user !== expectedValues[0] || values.password !== expectedValues[1] || (eventsExpected && (!values.inputs || !values.changes))) {
    const status = await evaluate(popupWs, `document.querySelector('#status')?.textContent || ''`);
    const tabs = await evaluate(workerWs, `chrome.tabs.query({active:true,currentWindow:true}).then(t=>t.map(x=>({id:x.id,url:x.url})))`);
    throw new Error(`Form was not filled: ${JSON.stringify({ values, status, tabs })}`);
  }
  const otherUrl = ['githubLive', 'herokuLive'].includes(scenario) ? 'https://example.com/' : `${site}/other`;
  await cdp(pageWs, 'Page.navigate', { url: otherUrl });
  for (let i = 0; i < 100; i++) {
    if (await evaluate(pageWs, `location.href.startsWith(${JSON.stringify(otherUrl)})`)) break;
    await delay(100);
  }
  await evaluate(popupWs, `document.querySelector('#entries button').click(); true`);
  for (let i = 0; i < 100; i++) {
    if ((await evaluate(popupWs, `document.querySelector('#status')?.textContent || ''`)).includes('mudou')) break;
    await delay(100);
  }
  const afterNavigation = await evaluate(pageWs, `({user:document.querySelector('${userSelector}')?.value || '',password:document.querySelector('${passwordSelector}')?.value || ''})`);
  const rejected = await evaluate(popupWs, `document.querySelector('#status')?.textContent || ''`);
  if (afterNavigation.user || afterNavigation.password || !(rejected.includes('mudou') || (['githubLive', 'herokuLive'].includes(scenario) && rejected.includes('Aba ativa indisponível')))) throw new Error(`Navigation was not rejected: ${JSON.stringify({ afterNavigation, rejected, currentUrl: await evaluate(pageWs, 'location.href') })}`);
  console.log(JSON.stringify({ browser: browserBin, scenario, extensionId, paired: true, popupListed: true, outcome: expectedStatus, navigationRejected: true }));
  popupWs.close(); browserWs.close(); workerWs.close(); pageWs.close();
} finally {
  child.kill('SIGTERM');
  await new Promise(resolve => server.close(resolve));
  await delay(500);
  rmSync(root, { recursive: true, force: true });
}
