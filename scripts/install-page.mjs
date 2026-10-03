// @ref LLP 1030.003#d6a--the-standard-install-page — generated public UI, separate from the app boot graph.
import { mkdirSync, writeFileSync, readFileSync, realpathSync } from 'node:fs';
import { networkInterfaces } from 'node:os';
import { resolve, extname, sep } from 'node:path';

export const INSTALL_ROOT = '/.exact/install/';
export const INSTALL_PLATFORMS = ['web', 'ios', 'macos'];
export const INSTALL_FILES = [INSTALL_ROOT + 'index.html', ...INSTALL_PLATFORMS.map(p => `${INSTALL_ROOT}${p}/index.html`)];
// The pages' content as data, for a client that draws its own (Exact2 Go, a native app's sheet).
export const INSTALL_DATA = '/.exact/install.json';
/** Every public install route: the pages and their data. */
export const INSTALL_PUBLIC = [...INSTALL_FILES, INSTALL_DATA];
export const LOCAL_IOS_INSTALL_ENDPOINT = '/__dev/install/ios';
const kinds = { web: ['browser'], ios: ['go', 'direct', 'testflight', 'app-store'], macos: ['go', 'download', 'terminal'] };
const labels = { browser: 'Open in browser', go: 'Open in Exact2 Go', direct: 'Install on iPhone or iPad', testflight: 'Install with TestFlight', 'app-store': 'View in the App Store', download: 'Download for Mac', terminal: 'Install from Terminal' };
const details = { go: 'Run this app inside Exact2 Go. Exact2 Go must be installed first.', terminal: 'Review the command, then paste it into Terminal.', direct: 'Device enrollment may be required. Follow the installation steps.', browser: '' };
const escape = value => String(value).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));

function https(value) {
  try { const u = new URL(value); return value.startsWith('https://') && u.protocol === 'https:' && !!u.hostname && !u.username && !u.password && !/[\s\\]/.test(value); } catch { return false; }
}
function validURL(value, kind) {
  if (typeof value !== 'string') return false;
  if (https(value)) return true;
  if (kind === 'browser') return /^\/(?!\/)/.test(value) && !/[\s\\]/.test(value) && !value.startsWith(INSTALL_ROOT) && value.split(/[?#]/)[0] !== '/.exact/install';
  if (kind !== 'direct') return false;
  try {
    const u = new URL(value);
    return u.protocol === 'itms-services:' && !u.hostname && !u.pathname && u.searchParams.get('action') === 'download-manifest'
      && [...u.searchParams.keys()].sort().join(',') === 'action,url' && https(u.searchParams.get('url'));
  } catch { return false; }
}

/** Semantic checks supplement app.schema.json; URLs never become executable page markup. */
export function installProblems(manifest) {
  const problems = [];
  for (const [platform, config] of Object.entries(manifest.install ?? {})) {
    if (!kinds[platform] || !config || typeof config !== 'object') { problems.push(`install.${platform}: unsupported platform`); continue; }
    if (config.urls !== undefined && platform !== 'web') problems.push(`install.${platform}.urls: web only`);
    for (const entry of config.urls ?? []) if (!entry.label?.trim() || !https(entry.url)) problems.push('install.web.urls: expected a label and HTTPS URL');
    const seen = new Set();
    for (const method of config.methods ?? []) {
      const at = `install.${platform}.${method.kind}`;
      if (!kinds[platform].includes(method.kind)) problems.push(`${at}: unsupported method for this platform`);
      if (seen.has(method.kind)) problems.push(`${at}: duplicate method`);
      seen.add(method.kind);
      if (!validURL(method.url, method.kind)) problems.push(`${at}: expected HTTPS URL${method.kind === 'direct' ? ' or itms-services link to an HTTPS manifest' : ''}`);
      if (method.kind === 'terminal' && (!method.command?.trim() || /[\x00-\x08\x0b-\x1f\x7f]/.test(method.command))) problems.push(`${at}: provide a copyable command without control characters`);
      if (method.setupUrl !== undefined && (method.kind !== 'go' || !https(method.setupUrl))) problems.push(`${at}: setupUrl requires a Go method and HTTPS URL`);
      if (method.kind !== 'terminal' && method.command !== undefined) problems.push(`${at}: command is only supported for terminal`);
    }
    if (config.recommended && !seen.has(config.recommended)) problems.push(`install.${platform}.recommended: must name a configured method`);
  }
  return problems;
}

/** Normalize known installation routes only; never claim app-owned /install. */
export function installRoute(route) {
  if (route === '/.exact/install' || route === INSTALL_ROOT) return INSTALL_ROOT + 'index.html';
  for (const p of INSTALL_PLATFORMS) if (route === INSTALL_ROOT + p || route === INSTALL_ROOT + p + '/') return `${INSTALL_ROOT}${p}/index.html`;
  return route;
}

function methodsFor(manifest, platform) {
  const config = manifest.install?.[platform];
  const methods = config?.methods ?? (platform === 'web' ? [{ kind: 'browser', url: manifest.start_url ?? '/' }] : []);
  return methods.map((m, i) => ({ ...m, order: i, recommended: config?.recommended === m.kind }))
    .sort((a, b) => Number(b.recommended) - Number(a.recommended) || a.order - b.order);
}

/** What the install pages say (`/.exact/install.json`): the app, its build,
 * each platform's methods in presentation order with their labels and
 * descriptions resolved, and what it can reach. The HTML below is one
 * rendering of it; a native client draws its own from the same data. A
 * root-relative URL (a browser destination, a brand asset) resolves against
 * the origin it was read from. A terminal method's `command` is shown for a
 * person to copy, never run. `reach` is null where the build did not compute
 * it (a JS development build has no bake receipt), never an empty claim;
 * `build.source` and `build.dirty` are null where git did not answer. */
export function installData(manifest, build = {}) {
  const errors = installProblems(manifest);
  if (methodsFor(manifest, 'web').some(m => !validURL(m.url, 'browser'))) errors.push('install.web: invalid browser URL');
  if (errors.length) throw new Error(errors.join('\n'));
  const brand = manifest.brand ?? {}, rooted = path => path && '/' + path;
  const wordmark = brand.wordmark && { ...brand.wordmark, ...(brand.wordmark.image ? { image: rooted(brand.wordmark.image) } : {}), ...(brand.wordmark.font ? { font: rooted(brand.wordmark.font) } : {}) };
  const platforms = Object.fromEntries(INSTALL_PLATFORMS.map(platform => [platform, {
    methods: methodsFor(manifest, platform).map(({ order, ...m }) => {
      const description = m.description ?? details[m.kind] ?? 'Install this app on your device.';
      return { ...m, label: m.label ?? labels[m.kind], ...(description ? { description } : {}) };
    }),
    ...(platform === 'web' && manifest.install?.web?.urls ? { urls: manifest.install.web.urls } : {}),
  }]));
  return {
    exactInstall: 1,
    app: { name: manifest.app?.name ?? manifest.name, ...(manifest.app?.version ? { version: manifest.app.version } : {}) },
    ...(brand.logo || wordmark ? { brand: { ...(brand.logo ? { logo: rooted(brand.logo) } : {}), ...(wordmark ? { wordmark } : {}) } } : {}),
    build: { id: build.id ?? null, source: build.source ?? null, dirty: build.dirty ?? null, builtAt: build.builtAt ?? null, mode: build.mode ?? null },
    platforms,
    reach: build.reach ?? null,
  };
}

export function installPage(manifest, presentation = {}) {
  const data = installData(manifest, presentation.build);
  const name = escape(data.app.name);
  const titles = { web: 'Web', ios: 'iOS', macos: 'macOS' };
  const brand = manifest.brand ?? {}, wordmark = brand.wordmark ?? {};
  const heading = presentation.wordmark ? `<img class="wordmark-image" src="${escape(presentation.wordmark)}" alt="${name}">` : `<span class="wordmark">${escape(wordmark.text ?? manifest.app?.name ?? manifest.name)}</span>`;
  const meta = presentation.build ?? {};
  const buildLabel = meta.id ? `Build ${meta.id.slice(0, 10)}` : 'Build not recorded';
  const version = manifest.app?.version ? `Version ${manifest.app.version} · ` : '';
  const exactMark = readFileSync(new URL('../assets/brand/exact-mark.svg', import.meta.url), 'utf8');
  const panels = INSTALL_PLATFORMS.map(platform => {
    const { methods, urls = [] } = data.platforms[platform];
    const local = platform === 'ios' ? '<!-- exact-local-ios -->' : '';
    return `<section class="platform${methods.length ? '' : ' unavailable'}" id="${platform}" aria-labelledby="heading-${platform}"><h2 id="heading-${platform}">${titles[platform]}</h2>${local}${methods.length ? methods.map(m => {
      const label = escape(m.label), detail = m.description;
      return `<article>${m.recommended ? '<span class="badge">Recommended</span>' : ''}<h3>${label}</h3>${detail ? `<p>${escape(detail)}</p>` : ''}${m.kind === 'browser' ? `<div class="browser-urls"><div class="browser-destination"><span class="url-label" data-current-label>This address</span><a class="browser-url" data-browser-url href="${escape(m.url)}">${escape(m.url)}</a></div>${urls.map(entry => `<div class="browser-destination"><span class="url-label">${escape(entry.label)}</span><a class="browser-url" href="${escape(entry.url)}">${escape(entry.url)}</a></div>`).join('')}</div>` : ''}${m.version ? `<p class="version">${escape(m.version)}</p>` : ''}${m.kind === 'terminal' ? `<pre><code id="command-${platform}">${escape(m.command)}</code></pre><button type="button" data-copy="command-${platform}">Copy command</button><a class="secondary" href="${escape(m.url)}" rel="noopener noreferrer">Installation instructions</a>` : `<a class="action" href="${escape(m.url)}" rel="noopener noreferrer">${label}<span aria-hidden="true"> ↗</span></a>`}${m.kind === 'go' && m.setupUrl ? `<a class="secondary" href="${escape(m.setupUrl)}" rel="noopener noreferrer">Get Exact2 Go</a>` : ''}</article>`;
    }).join('') : '<p class="unavailable-note">Not available yet</p>'}</section>`;
  }).join('');
  // What this app can reach (LLP 1069.008 D7): the bake's rows, base-locale purposes.
  const rows = data.reach ?? [];
  const reach = rows.length ? `<section class="platform reach" id="reach" aria-labelledby="heading-reach"><h2 id="heading-reach">What this app can reach</h2><ul>${rows.map(r => `<li><code>${escape(r.grant)}</code>${r.purpose ? `<p>${escape(r.purpose)}</p>` : ''}<p class="enforced">Enforced by ${escape(r.enforced)}</p></li>`).join('')}</ul></section>` : '';
  return `<!doctype html>
<html lang="${escape(manifest.lang ?? 'en')}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="referrer" content="no-referrer"><title>Get ${name}</title>
<style>
${presentation.font ? `@font-face{font-family:AppWordmark;src:url("${presentation.font}");font-weight:${Number(wordmark.weight ?? 600)};font-display:swap}` : ''}
:root{color-scheme:light;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;color:#1c1c1c;background:#fafaf9}*{box-sizing:border-box}body{margin:0}main{max-width:700px;margin:auto;padding:64px 26px 36px}.brand{display:flex;align-items:center;gap:18px;margin:0 0 24px}.app-logo{width:60px;height:60px;object-fit:contain}h1{margin:0;line-height:1.2;font-size:44px;font-weight:600;letter-spacing:-1.5px}.wordmark{font-family:${presentation.font ? 'AppWordmark' : 'inherit'};font-weight:${Number(wordmark.weight ?? 600)};letter-spacing:${Number(wordmark.letterSpacing ?? -1.5)}px}.wordmark-image{width:auto;max-width:100%;height:56px;object-fit:contain}.eyebrow{font-size:12px;color:#797979;margin-bottom:16px}header{padding-bottom:32px}.release{font:12px/1.8 ui-monospace,SFMono-Regular,monospace;color:#666;overflow-wrap:anywhere}.context{display:flex;gap:8px;flex-wrap:wrap;align-items:center;color:#6b6b6b;font-size:12px;margin-bottom:10px}.context span{padding:5px 9px;border:1px solid #dededb;border-radius:5px}.platform{border-top:1px solid #dededb;padding:26px 0 30px}h2{font-size:18px;letter-spacing:-.02em;margin:0 0 20px}article+article{margin-top:28px}h3{font-size:15px;font-weight:500;margin:0 0 8px}article p{font-size:14px;line-height:1.6;color:#666;margin:8px 0 16px}.browser-url{display:block;font:13px/1.6 ui-monospace,SFMono-Regular,monospace;color:#656565;text-decoration:none;overflow-wrap:anywhere;margin-bottom:18px}.browser-urls{margin:0 0 18px}.browser-destination{margin:12px 0}.browser-destination .browser-url{margin:3px 0}.url-label{font-size:11px;color:#858585}.badge{display:inline-block;background:#efefec;font-size:11px;border-radius:4px;padding:4px 7px;margin:0 0 10px}.action,button{display:inline-block;border:0;background:#222;color:white;border-radius:8px;padding:12px 16px;font:500 13px/1.4 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;text-decoration:none;cursor:pointer}.secondary{display:inline-block;font-size:13px;margin:10px;color:#555}.version{font:12px ui-monospace,monospace}pre{background:#eee;padding:16px;border-radius:8px;white-space:pre-wrap;overflow-wrap:anywhere;font-size:13px;line-height:1.6}.unavailable h2{color:#969696;margin-bottom:10px}.unavailable-note{font-size:14px;color:#8b8b8b;margin:0}.local-device{display:block;width:100%;max-width:420px;margin:14px 0;padding:11px 12px;border:1px solid #cfcfca;border-radius:8px;background:#fff;font:14px system-ui}.local-actions{display:flex;align-items:center;gap:9px;flex-wrap:wrap}.local-actions .quiet{background:transparent;color:#555;border:1px solid #cfcfca}.local-state{min-height:1.5em;margin-bottom:0}.local-log{margin-top:14px}.local-log pre{max-height:220px;overflow:auto;font-size:11px}button:disabled,select:disabled{opacity:.5;cursor:not-allowed}footer{border-top:1px solid #dededb;padding-top:25px;display:flex;align-items:center;gap:10px;color:#858585;font-size:12px}footer svg{width:30px;height:20px}footer .exact-wordmark{font-size:22px;font-weight:600;letter-spacing:-1px;margin-left:-3px}a:focus-visible,button:focus-visible,select:focus-visible{outline:3px solid #777;outline-offset:4px}.reach ul{list-style:none;margin:0;padding:0}.reach li+li{margin-top:16px}.reach code{font:13px ui-monospace,SFMono-Regular,monospace;overflow-wrap:anywhere}.reach p{font-size:14px;line-height:1.6;color:#666;margin:4px 0 0}.reach .enforced{font-size:12px;color:#858585}#status:empty{display:none}#status{color:#555;font-size:13px}@media(max-width:480px){main{padding:36px 24px 28px}h1{font-size:40px}.platform{padding:22px 0 26px}.brand{gap:12px}.app-logo{width:48px;height:48px}}
</style></head><body><main><header><div class="eyebrow">Install</div><div class="brand">${presentation.logo ? `<img class="app-logo" src="${escape(presentation.logo)}" alt="">` : ''}<h1>${heading}</h1></div><div class="context"><span>${escape(meta.mode ?? 'Build type not recorded')}</span><span data-serving><!-- exact-serving -->Static hosting<!-- /exact-serving --></span></div><div class="release">${escape(version + buildLabel)}${meta.source ? `<br>Source ${escape(meta.source)}${meta.dirty ? ' · local changes' : ''}` : ''}${meta.builtAt ? `<br>Built <time datetime="${escape(meta.builtAt)}">${escape(meta.builtAt)}</time>` : ''}</div></header>${panels}${reach}<p id="status" role="status" aria-live="polite"></p><footer><span>Built with</span>${exactMark}<span class="exact-wordmark">Exact2</span></footer></main>
<!-- exact-browser-origins -->
<script>
(() => {
  const origins = JSON.parse(document.getElementById('exact-browser-origins')?.textContent || '[]');
  const addressLabel = host => host === 'localhost' || host === '127.0.0.1' || host === '[::1]' ? 'Localhost · server machine only' : /^100\\.(6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\\./.test(host) || host.endsWith('.ts.net') ? 'Tailscale / VPN' : /^(192\\.168\\.|10\\.|172\\.(1[6-9]|2[0-9]|3[01])\\.)/.test(host) ? 'LAN · same network' : 'This address';
  document.querySelectorAll('[data-browser-url]').forEach(link => {
    link.textContent = link.href;
    link.parentElement.querySelector('[data-current-label]').textContent = addressLabel(new URL(link.href).hostname);
    const list = link.closest('.browser-urls');
    const seen = new Set(Array.from(list.querySelectorAll('a'), a => a.href));
    if (!link.getAttribute('href').startsWith('/')) return;
    for (const entry of origins) {
      const url = new URL(link.getAttribute('href'), entry.origin).href;
      if (seen.has(url)) continue;
      seen.add(url);
      const row = document.createElement('div'), label = document.createElement('span'), anchor = document.createElement('a');
      row.className = 'browser-destination'; label.className = 'url-label'; anchor.className = 'browser-url';
      label.textContent = entry.label; anchor.href = url; anchor.textContent = url;
      row.append(label, anchor); list.append(row);
    }
  });
  document.querySelectorAll('time[datetime]').forEach(time => { time.textContent = new Date(time.dateTime).toLocaleString(undefined, {year:'numeric',month:'short',day:'numeric',hour:'numeric',minute:'2-digit',timeZoneName:'short'}); });
  document.querySelectorAll('[data-copy]').forEach(button => button.addEventListener('click',async () => {
    const node = document.getElementById(button.dataset.copy);
    try { await navigator.clipboard.writeText(node.textContent); document.getElementById('status').textContent = 'Copied. Paste into Terminal when you’re ready.'; }
    catch { const range = document.createRange(); range.selectNodeContents(node); const selection = window.getSelection(); selection.removeAllRanges(); selection.addRange(range); document.getElementById('status').textContent = 'Select and copy the command above.'; }
  }));
})();
</script></body></html>\n`;
}

/** Add the Mac dev server's explicit build/install action to a baked page.
 * Static and published pages never call this, so they cannot grow a signing
 * service accidentally. The per-process token makes cross-site POSTs unable
 * to trigger a device build. @ref LLP 1030.003#d6b--local-ios-installation */
export function developmentInstallPage(page, token) {
  if (!/^[0-9a-f]{64}$/.test(token)) throw new Error('local iOS installer token must be 32 random bytes');
  const marker = '<!-- exact-local-ios -->';
  const endpoint = JSON.stringify(LOCAL_IOS_INSTALL_ENDPOINT);
  const secret = JSON.stringify(token);
  const article = `<article class="local-install" data-local-ios><span class="badge">From this Mac</span><h3>Build, install, and open</h3><p>Choose an iOS Simulator on this Mac or a paired physical device. Exact2 builds the app for that target, installs it, and opens this same app URL. Physical devices must be unlocked with Developer Mode on.</p><label for="local-ios-target">Simulator or device</label><select class="local-device" id="local-ios-target" disabled><option>Looking for local targets…</option></select><div class="local-actions"><button type="button" id="local-ios-start" disabled>Build and install</button><button type="button" class="quiet" id="local-ios-refresh">Refresh targets</button></div><p class="local-state" id="local-ios-state" role="status" aria-live="polite">Looking for an iOS Simulator or paired device…</p><details class="local-log" id="local-ios-log" hidden><summary>Build log</summary><pre></pre></details><noscript><p>JavaScript is required to ask this Mac to build and install the app.</p></noscript></article>
<script>
(() => {
  const endpoint = ${endpoint}, token = ${secret};
  const select = document.querySelector('#local-ios-target');
  const start = document.querySelector('#local-ios-start');
  const refresh = document.querySelector('#local-ios-refresh');
  const state = document.querySelector('#local-ios-state');
  const log = document.querySelector('#local-ios-log');
  let polling = null;
  async function request(method = 'GET', body, refresh = false) {
    const response = await fetch(endpoint + (refresh ? '?refresh=1' : ''), { method, headers: {'x-exact-install-token': token, ...(body ? {'content-type':'application/json'} : {})}, body: body ? JSON.stringify(body) : undefined });
    const value = await response.json().catch(() => ({message:'The development server returned an unreadable response.'}));
    if (!response.ok) throw new Error(value.message || 'The local install request failed.');
    return value;
  }
  function paint(value) {
    const selected = select.value;
    select.replaceChildren(...(value.targets || []).map(target => {
      const option = document.createElement('option'); option.value = target.id;
      option.textContent = [target.name, target.model, target.os && 'iOS ' + target.os, target.state].filter(Boolean).join(' · ');
      return option;
    }));
    if (selected && [...select.options].some(option => option.value === selected)) select.value = selected;
    const busy = value.state === 'building';
    select.disabled = busy || !value.available || !select.options.length;
    start.disabled = select.disabled;
    refresh.disabled = busy;
    start.textContent = busy ? 'Building…' : value.state === 'installed' ? 'Build again' : 'Build and install';
    state.textContent = value.message;
    log.hidden = !value.log;
    log.querySelector('pre').textContent = value.log || '';
    clearTimeout(polling);
    if (busy) polling = setTimeout(load, 1500);
  }
  async function load(force = false) { try { paint(await request('GET', undefined, force)); } catch (error) { state.textContent = error.message; } }
  refresh.addEventListener('click', () => load(true));
  start.addEventListener('click', async () => {
    start.disabled = true; refresh.disabled = true; state.textContent = 'Starting the local Apple build…';
    try { paint(await request('POST', {target:select.value})); } catch (error) { state.textContent = error.message; await load(); }
  });
  load();
})();
</script>`;
  const section = /<section class="platform(?: unavailable)?" id="ios"[\s\S]*?<\/section>/;
  return String(page)
    .replace('<!-- exact-serving -->Static hosting<!-- /exact-serving -->', 'Development server')
    .replace(section, value => value
      .replace('class="platform unavailable"', 'class="platform"')
      .replace(marker, article)
      .replace('<p class="unavailable-note">Not available yet</p>', ''));
}

export function writeInstallPages(dist, manifest, build = {}) {
  // Validated (the default browser URL too) before writing any output.
  installData(manifest, build);
  const asset = (name, font = false) => {
    if (!name) return null;
    if (!/^assets\//.test(name)) throw new Error('brand assets must be inside assets/');
    const root = realpathSync(dist), path = resolve(root, name);
    if (!path.startsWith(resolve(root, 'assets') + sep) || realpathSync(path) !== path) throw new Error('unsafe brand asset path');
    const types = font ? {'.ttf':'font/ttf','.otf':'font/otf','.woff2':'font/woff2','.woff':'font/woff'} : {'.svg':'image/svg+xml','.png':'image/png','.webp':'image/webp','.jpg':'image/jpeg','.jpeg':'image/jpeg'};
    const type = types[extname(path)]; if (!type) throw new Error('unsupported brand asset type');
    return `data:${type};base64,${readFileSync(path).toString('base64')}`;
  };
  const presentation = {build, logo:asset(manifest.brand?.logo), wordmark:asset(manifest.brand?.wordmark?.image), font:asset(manifest.brand?.wordmark?.font, true)};
  for (const platform of [null, ...INSTALL_PLATFORMS]) {
    const path = resolve(dist, '.exact/install', platform ?? '');
    mkdirSync(path, { recursive: true });
    writeFileSync(resolve(path, 'index.html'), installPage(manifest, presentation));
  }
  writeFileSync(resolve(dist, '.' + INSTALL_DATA), JSON.stringify(installData(manifest, build), null, 2) + '\n');
}

// Addresses come from the listener and local interfaces, never proxy/request
// headers or a subprocess. A loopback-only listener must not advertise LAN URLs.
export function installBrowserOrigins({host, port, interfaces = networkInterfaces()} = {}) {
  if (!Number.isInteger(port) || port < 1 || port > 65535) return [];
  const origins = [], seen = new Set();
  const add = (address, label) => {
    if (seen.has(address)) return;
    seen.add(address); origins.push({label, origin:`http://${address}:${port}`});
  };
  if (host === '127.0.0.1' || host === '0.0.0.0') add('127.0.0.1','Localhost · server machine only');
  if (host === '127.0.0.1') return origins;
  for (const entries of Object.values(interfaces)) for (const entry of entries ?? []) {
    if (entry.internal || entry.family !== 'IPv4' || host !== '0.0.0.0' && entry.address !== host) continue;
    const octets = entry.address.split('.').map(Number);
    const tail = octets[0] === 100 && octets[1] >= 64 && octets[1] <= 127;
    const lan = octets[0] === 10 || octets[0] === 192 && octets[1] === 168 || octets[0] === 172 && octets[1] >= 16 && octets[1] <= 31;
    add(entry.address, tail ? 'Tailscale / VPN' : lan ? 'LAN · same network' : 'Network interface');
  }
  const rank = label => label.startsWith('Localhost') ? 0 : label.startsWith('LAN') ? 1 : label.startsWith('Tailscale') ? 2 : 3;
  return origins.sort((a,b) => rank(a.label) - rank(b.label) || a.origin.localeCompare(b.origin));
}
/** The development server's request gate. It answers only to the names it
 * printed at startup (`origins`, from installBrowserOrigins) and `localhost`:
 * a DNS-rebinding page reaches a loopback socket under its own name. Each
 * `allowHosts` name (`--allow-host`, a tunnel's public name) is admitted too:
 * `name` at any port, `name:port` at that port. A request is local — it may
 * see the installer's token — only from a loopback peer naming a loopback
 * host, never through an allowed name, and never with a proxy's forwarding
 * headers: a tunnel that rewrites Host to 127.0.0.1 is still the internet. A
 * browser POST whose Origin is not the host it names is refused (cross-site). */
export function developmentGate(origins, port, allowHosts = []) {
  const loopback = [`http://127.0.0.1:${port}`, `http://localhost:${port}`].map(origin => new URL(origin));
  const names = new Set([...origins.map(o => new URL(o.origin).host), ...loopback.map(u => u.host)]);
  const allowed = allowHosts.map(allowHostName);
  const peers = new Set(['127.0.0.1', '::1', '::ffff:127.0.0.1']);
  return {
    loopbackOrigins: loopback.map(u => u.origin),
    allowHosts: allowed,
    check(req) {
      const headers = req.headers ?? {};
      const host = String(headers.host ?? '').toLowerCase();
      const name = host.replace(/:\d+$/, '');
      const forwarded = ['forwarded', 'x-forwarded-for', 'x-forwarded-host'].some(h => headers[h] !== undefined);
      let crossSite = false;
      if (req.method === 'POST' && headers.origin !== undefined) {
        try { crossSite = new URL(headers.origin).host.toLowerCase() !== host; } catch { crossSite = true; }
      }
      const admitted = !crossSite && (names.has(host) || allowed.some(a => a === host || a === name));
      return { allowed: admitted, local: admitted && !forwarded && loopback.some(u => u.host === host) && peers.has(req.socket?.remoteAddress) };
    },
  };
}
/** One `--allow-host` value, lowercased: a DNS name or IPv4 address, with an
 * optional `:port`. Anything else (a URL, a wildcard, an empty name) throws. */
export function allowHostName(value) {
  const name = String(value ?? '').toLowerCase();
  if (!/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*(?::\d{1,5})?$/.test(name)) throw new Error(`--allow-host takes a host name (example.trycloudflare.com, or name:port), not ${JSON.stringify(value ?? '')}`);
  return name;
}
/** Every `--allow-host <name>` in argv, in order (the flag repeats). */
export function allowHostArgs(argv) {
  const names = [];
  for (let i = 0; i < argv.length; i++) if (argv[i] === '--allow-host') names.push(allowHostName(argv[++i]));
  return names;
}

/** The app URL a local iOS build opens: one of this server's printed
 * addresses, never a request header. A Simulator shares this Mac's loopback;
 * a device needs a network address, so a loopback-only server refuses it. */
export function localInstallURL(kind, origins, port) {
  if (kind === 'simulator') return `http://127.0.0.1:${port}/`;
  const network = origins.find(o => new URL(o.origin).hostname !== '127.0.0.1');
  if (!network) throw Object.assign(new Error('A physical device cannot reach a loopback-only server. Restart it with --lan and open this page again.'), { status: 400 });
  return network.origin + '/';
}

export function installNetworkPage(html, listener) {
  const data = JSON.stringify(installBrowserOrigins(listener)).replaceAll('<', '\\u003c');
  return html.replace('<!-- exact-browser-origins -->', `<script type="application/json" id="exact-browser-origins">${data}</script>`);
}
