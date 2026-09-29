// Ocho's page module (LLP 1024 D7, LLP 1067 D5): the web has no `fleet` CLI
// and no PTY, so this answers the source's native calls with a fixture fleet
// (the dev loop's data) and renders `<ghostty-terminal>` as a placeholder
// that says where the real terminal is.
export const abi = 1;
export const roster = { 'ghostty-terminal': { snapshot: false } };

let announce = null;
export function connect({ changed }) { announce = changed; }

const now = Date.now();
const ago = (ms) => new Date(now - ms).toISOString();
const machines = [
  { id: 'm-local', name: 'Eliots-MacBook-Pro.local', local: true, online: true, home: '~', detail: 'darwin 26.6 · 14 cores · 21/36 GB · load 3.2' },
  { id: 'm-redwood', name: 'redwood', local: false, online: true, home: '~', detail: 'linux 6.12 · 32 cores · 48/128 GB · load 1.4' },
  { id: 'm-studio', name: 'studio', local: false, online: false, home: '~', detail: 'ssh: connection timed out' },
];
const accounts = [
  { handle: 'claude:eliot@getfirewood.dev', email: 'eliot@getfirewood.dev', provider: 'claude', status: 'connected' },
  { handle: 'claude:eliot@eliot.sh', email: 'eliot@eliot.sh', provider: 'claude', status: 'connected' },
  { handle: 'codex:eliot@expo.io', email: 'eliot@expo.io', provider: 'codex', status: 'connected' },
  { handle: 'codex:eliot.supceo@gmail.com', email: 'eliot.supceo@gmail.com', provider: 'codex', status: 'finish connecting' },
];
const sessions = [
  { id: 's1', machineId: 'm-local', title: 'Ocho client in exact2', provider: 'claude', account: 'eliot@getfirewood.dev', state: 'running', cwd: '~/Developer/exact2', model: 'claude-fable-5-1', summary: 'Building the sidebar and the launcher; the terminal module compiles.', updated: ago(30e3), pinned: true },
  { id: 's2', machineId: 'm-redwood', title: 'rename to ocho', provider: 'claude', account: 'eliot@eliot.sh', state: 'blocked', cwd: '~/.local/share/fleet/workspaces/fleet-aa31c17c', model: 'claude-opus-5-5', summary: 'Approval needed: git push origin main', updated: ago(4 * 60e3), pinned: false },
  { id: 's3', machineId: 'm-redwood', title: 'devicehub cli', provider: 'codex', account: 'eliot@expo.io', state: 'idle', cwd: '~/Developer/devicehub', model: 'gpt-6-sol', summary: 'Done. The CLI lists devices and streams logs; tests pass.', updated: ago(22 * 60e3), pinned: false },
  { id: 's4', machineId: 'm-local', title: 'plan.stanf.org', provider: 'codex', account: 'eliot@expo.io', state: 'idle', cwd: '~/Developer/plan.stanf.org', model: 'gpt-6-astra', summary: 'Waiting for you.', updated: ago(3 * 3600e3), pinned: false },
  { id: 's5', machineId: 'm-local', title: 'imessage bridge', provider: 'codex', account: 'eliot.supceo@gmail.com', state: 'paused', cwd: '~/Developer/fleet', model: 'gpt-6-sol', summary: 'Paused before the migration step.', updated: ago(26 * 3600e3), pinned: false },
  { id: 's6', machineId: 'm-redwood', title: 'Fleet mobile app with Expo', provider: 'claude', account: 'eliot@getfirewood.dev', state: 'closed', cwd: '~/Developer/fleet-mobile', model: 'claude-fable-5-1', summary: "Fixed in Fleet's transcript reader, which is where they came from.", updated: ago(3 * 86400e3), pinned: false },
  { id: 's7', machineId: 'm-local', title: 'shell', provider: 'shell', account: '', state: 'idle', cwd: '~', model: '', summary: '', updated: ago(9 * 86400e3), pinned: false },
];
const rank = { running: 0, blocked: 1, limited: 1, starting: 2, idle: 3, paused: 4, closed: 7, exited: 7 };
const rel = (iso) => { const d = (now - Date.parse(iso)) / 1000; return d < 45 ? 'now' : d < 3600 ? `${Math.floor(d / 60)}m` : d < 86400 ? `${Math.floor(d / 3600)}h` : d < 7 * 86400 ? `${Math.floor(d / 86400)}d` : new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric' }); };
const code = (i) => String(i + 1).padStart(2, '0');
const providerName = (p) => ({ claude: 'Claude', codex: 'Codex', opencode: 'OpenCode', shell: 'Shell' })[p] ?? p;
const catalogs = {
  codex: [['gpt-6-astra', 'GPT-6-Astra', 'Frontier intelligence for the most demanding work.'], ['gpt-6-sol', 'GPT-6-Sol', 'Workhorse model for coding and everyday work.'], ['gpt-6-luna', 'GPT-6-Luna', 'Fast and affordable model for easier tasks.']],
  claude: [['claude-fable-5-1', 'Fable 5.1', 'Most capable, for the hardest problems.'], ['claude-opus-5-5', 'Opus 5.5', 'Deep reasoning for complex work.'], ['claude-sonnet-5', 'Sonnet 5', 'Fast and balanced.']],
};
let tabs = [];
let revision = 0;

function fleet(query = '') {
  const q = String(query ?? '').toLowerCase().trim();
  const rows = sessions
    .filter((s) => !q || [s.title, s.cwd, s.provider, s.account, s.model, s.summary].join(' ').toLowerCase().includes(q))
    .map((s) => ({ ...s, machine: machines.find((m) => m.id === s.machineId)?.name ?? s.machineId, updated: rel(s.updated), live: !['closed', 'exited'].includes(s.state) }))
    .sort((a, b) => (rank[a.state] ?? 5) - (rank[b.state] ?? 5) || Date.parse(sessions.find((s) => s.id === b.id).updated) - Date.parse(sessions.find((s) => s.id === a.id).updated));
  rows.forEach((r, i) => { r.index = i; });
  return {
    ready: true, message: '',
    machines: machines.map((m, index) => ({ ...m, index, status: m.online ? 'online' : 'unreachable', sessions: sessions.filter((s) => s.machineId === m.id && !['closed', 'exited'].includes(s.state)).length, active: sessions.filter((s) => s.machineId === m.id && s.state === 'running').length })),
    sessions: rows,
    accounts: accounts.map((a) => ({ ...a, connected: a.status === 'connected', usage: a.provider === 'claude' ? '5h 32% · week 61%' : '' })),
    tabs: tabs.map((t, i) => { const s = sessions.find((s) => s.id === t.sessionId); return { ...t, slot: i + 1, state: s?.state ?? t.state, status: s ? (s.summary || s.state) : '', depth: 0, folder: false, collapsed: false }; }),
    accountChoices: accounts.map((a, i) => ({ id: a.handle, code: code(i), group: code(i)[0], label: a.email, detail: providerName(a.provider), provider: a.provider })),
    machineChoices: machines.map((m, i) => ({ id: m.id, code: code(i), group: code(i)[0], label: m.name + (m.local ? ' (this Mac)' : ''), detail: m.home, provider: '' })),
    recentDirs: [['m-local', '~/Developer/exact2'], ['m-local', '~/Developer/fleet'], ['m-redwood', '~'], ['m-redwood', '~/Developer/devicehub']].map(([m, c]) => ({ id: c, code: '', group: '', label: c, detail: m, provider: '' })),
    liveCount: sessions.filter((s) => !['closed', 'exited'].includes(s.state)).length,
    revision,
    terminalBackground: tabs.length ? '#1b1e24' : '',
  };
}

function bump() { revision += 1; announce?.('fleet'); }

export async function later(request) {
  const { op, args = [] } = request;
  const receipt = (ok, tabId = '', version = 0, message = '') => ({ ok, message, tabId, version });
  switch (op) {
    case 'fleet': return fleet(args[0]);
    case 'openTab': {
      const [machineId, sessionId, , version] = args;
      let t = tabs.find((t) => t.sessionId === sessionId && t.machineId === machineId);
      if (!t) {
        const s = sessions.find((s) => s.id === sessionId);
        const m = machines.find((m) => m.id === machineId);
        t = { id: `tab-${Math.random().toString(36).slice(2, 8)}`, title: s?.title ?? 'Session', subtitle: `${m?.name ?? machineId} · ${providerName(s?.provider ?? '')}`, provider: s?.provider ?? '', machineId, sessionId, state: s?.state ?? 'idle' };
        tabs.push(t);
      }
      bump();
      return receipt(true, t.id, Number(version));
    }
    case 'openShell': {
      const [machineId, version] = args;
      const m = machines.find((m) => m.id === machineId);
      const t = { id: `tab-${Math.random().toString(36).slice(2, 8)}`, title: 'Shell', subtitle: `Shell · ${m?.name ?? machineId}`, provider: 'shell', machineId, sessionId: '', state: 'idle' };
      tabs.push(t); bump();
      return receipt(true, t.id, Number(version));
    }
    case 'toggleFolder': return receipt(true);
    case 'closeTab': tabs = tabs.filter((t) => t.id !== args[0]); bump(); return receipt(true);
    case 'launch': {
      const [account, machineId, model, cwd, version] = args;
      const provider = String(account).split(':')[0];
      const m = machines.find((m) => m.id === machineId);
      const t = { id: `tab-${Math.random().toString(36).slice(2, 8)}`, title: `${providerName(provider)} · ${(String(cwd).split('/').pop() || '~') === '~' ? model : String(cwd).split('/').pop()}`, subtitle: `${m?.name ?? machineId} · ${model}`, provider, machineId, sessionId: '', state: 'starting' };
      tabs.push(t); bump();
      return receipt(true, t.id, Number(version));
    }
    case 'models': {
      const [machine, provider, account, cwd] = args;
      await new Promise((r) => setTimeout(r, 250));
      const list = catalogs[provider] ?? [];
      return { key: `${machine}|${provider}|${account}|${cwd}`, ready: true, message: list.length ? '' : `No models for ${provider}.`, models: list.map(([id, name, description], i) => ({ id, code: code(i), group: code(i)[0], label: name, detail: description, provider })) };
    }
    case 'sessionAction': return receipt(true, '', 0, `${args[2]} sent`);
    default: throw new Error(`Ocho answers no ${op}`);
  }
}

const STYLE = `
:host { display: block; position: relative; background: #1b1e24; color: #d7dae0; font: 13px/18px ui-monospace, SFMono-Regular, Menlo, monospace; }
.pane { position: absolute; inset: 0; padding: 14px 16px; overflow: hidden; }
.muted { color: #8b919c; }
.cursor { display: inline-block; width: 8px; height: 16px; background: #d7dae0; vertical-align: -3px; animation: blink 1s steps(2) infinite; }
@keyframes blink { 50% { opacity: 0; } }
`;

export function create(tag, element, json, event) {
  const root = element.shadowRoot ?? element.attachShadow({ mode: 'open' });
  const h = { element, root, tab: '' };
  root.innerHTML = `<style>${STYLE}</style><div class="pane"></div>`;
  h.pane = root.querySelector('.pane');
  setProps(h, json);
  return h;
}

export function setProps(h, json) {
  const props = typeof json === 'string' ? JSON.parse(json || '{}') : (json ?? {});
  h.tab = props.tab ?? '';
  const t = tabs.find((t) => t.id === h.tab);
  h.pane.innerHTML = `<div class="muted">ocho · ${t ? t.subtitle : 'no tab'}</div><div>$ fleet ${t ? (t.sessionId ? `attach ${t.machineId} ${t.sessionId}` : `connect ${t.machineId}`) : ''}</div><div class="muted">The terminal runs in the macOS app (libghostty). This is the web preview.</div><div><span class="cursor"></span></div>`;
}

export function destroy(h) { h.root.innerHTML = ''; }
