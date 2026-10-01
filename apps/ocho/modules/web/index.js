// Ocho's page module (LLP 1024 D7, LLP 1067 D5): the web has no `fleet` CLI
// and no PTY, so this answers the source's three native ops with a fixture
// fleet in the feed's own event shape (`fleet snapshot --watch` NDJSON), a
// desktop.json kept in memory, and jobs that succeed with fixture output. It
// renders `<ghostty-terminal>` as a placeholder that says where the real
// terminal is.
export const abi = 1;
export const roster = { 'ghostty-terminal': { snapshot: false } };

let announce = null;
let ticking = null;
// The feed's indicator ticks start when the page connects (the build script
// imports this file for its roster; a timer would keep that process alive).
export function connect({ changed }) {
  announce = changed;
  if (!ticking) {
    ticking = setInterval(tick, 3000);
    ticking.unref?.();
  }
}

const now = Date.now();
const iso = (ms) => new Date(now - ms).toISOString();
const epoch = (ms) => Math.floor((now - ms) / 1000);

const session = (id, over) => ({
  id, native_id: '', provider: 'claude', account: '', cwd: '~', title: id, title_source: 'agent', model: '', state: 'idle', pid: 4242,
  status_text: '', status_style: null, status_observed_at: epoch(2000), tmux_pane: '', started: iso(3600e3), tracked: true, managed: true,
  historical: false, pinned: false, hidden: false, archived: false, last_message: '', last_turn_interrupted: false, status_source: 'hook',
  hook_warning: '', attached: false, pending_work: [], pull_requests: [], imessage_attached: false, claude_remote: false, ...over,
});

const machines = [
  {
    id: 'm-local', name: 'Eliots-MacBook-Pro', local: true, addresses: [], notes: '', tags: [], permissions: { codex: 'yolo', claude: 'auto' }, error: '', helper_status: '', sprite_source: '',
    last: {
      hostname: 'Eliots-MacBook-Pro.local', home: '/Users/eliot', platform: 'darwin 26.6', cpu: 'Apple M4 Max', cores: 14, cpu_percent: 23, memory_total: 36 * 2 ** 30, memory_used: 21 * 2 ** 30,
      versions: { codex: '0.159.0', claude: '2.1.4', opencode: '1.2.0' }, live_inventory: true, at: iso(4000), warnings: [],
      sessions: [
        session('s1', { title: 'Ocho client in exact2 · Eliots-MacBook-Pro', account: 'eliot@getfirewood.dev', cwd: '~/Developer/exact2', model: 'claude-fable-5-1', state: 'working', status_text: 'Working… (12s · ↓ 3.2k tokens)', last_message: 'Building the sidebar and the launcher; the terminal module compiles.', pinned: true, started: iso(30e3) }),
        session('s4', { title: 'plan.stanf.org', provider: 'codex', account: 'eliot@expo.io', cwd: '~/Developer/plan.stanf.org', model: 'gpt-6-astra', state: 'idle', last_message: 'Waiting for you.', started: iso(3 * 3600e3), pull_requests: [{ url: 'https://github.com/eiiot/plan/pull/41', repo: 'eiiot/plan', number: 41 }] }),
        session('s5', { title: 'imessage bridge', provider: 'codex', account: 'eliot.supceo@gmail.com', cwd: '~/Developer/fleet', model: 'gpt-6-sol', state: 'paused', pause_reason: 'memory', last_message: 'Paused before the migration step.', started: iso(26 * 3600e3), imessage_attached: true }),
        session('s7', { title: 'shell', provider: 'shell', cwd: '~', state: 'idle', managed: false, tracked: false, started: iso(9 * 86400e3) }),
      ],
    },
  },
  {
    id: 'm-redwood', name: 'redwood', local: false, addresses: ['redwood.tail'], notes: '', tags: ['gpu'], permissions: {}, error: '', helper_status: '', sprite_source: '',
    last: {
      hostname: 'redwood', home: '/home/eliot', platform: 'linux 6.12', cpu: 'AMD EPYC 9354', cores: 32, cpu_percent: 8, memory_total: 128 * 2 ** 30, memory_used: 48 * 2 ** 30,
      versions: { codex: '0.158.0', claude: '2.1.4', opencode: '' }, live_inventory: true, at: iso(9000), warnings: [],
      sessions: [
        session('s2', { title: 'rename to ocho · redwood', account: 'eliot@eliot.sh', cwd: '~/.local/share/fleet/workspaces/fleet-aa31c17c', model: 'claude-opus-5-5', state: 'awaiting approval', last_message: 'Approval needed: git push origin main', started: iso(4 * 60e3) }),
        session('s3', { title: 'devicehub cli', provider: 'codex', account: 'eliot@expo.io', cwd: '~/Developer/devicehub', model: 'gpt-6-sol', state: 'idle', last_message: 'Done. The CLI lists devices and streams logs; **tests pass**.', started: iso(22 * 60e3) }),
        session('s6', { title: 'Fleet mobile app with Expo', account: 'eliot@getfirewood.dev', cwd: '~/Developer/fleet-mobile', model: 'claude-fable-5-1', state: 'closed', pid: 0, archived: true, last_message: "Fixed in Fleet's transcript reader, which is where they came from.", started: iso(3 * 86400e3) }),
      ],
    },
  },
  { id: 'm-studio', name: 'studio', local: false, addresses: ['studio.local'], notes: '', tags: [], permissions: {}, error: 'ssh: connect to host studio.local port 22: Connection timed out', helper_status: '', sprite_source: '', last: null },
];

const accounts = [
  { email: 'eliot@getfirewood.dev', name: 'eliot@getfirewood.dev', provider: 'claude', notes: '', shared: true, status: 'connected' },
  { email: 'eliot@eliot.sh', name: 'eliot@eliot.sh', provider: 'claude', notes: '', shared: true, status: 'connected' },
  { email: 'eliot@expo.io', name: 'eliot@expo.io', provider: 'codex', notes: '', shared: true, status: 'connected' },
  { email: 'eliot.supceo@gmail.com', name: 'eliot.supceo@gmail.com', provider: 'codex', notes: 'Plus plan', shared: false, status: 'connected' },
];

const presets = {
  'exact2 fable': { provider: 'claude', account: 'eliot@getfirewood.dev', cwd: '~/Developer/exact2', model: 'claude-fable-5-1', effort: 'high', prompt: '', permissions: 'auto', machine_id: 'm-local', shortcut: 1 },
  'redwood codex': { provider: 'codex', account: 'eliot@expo.io', cwd: '~/Developer/fleet', model: '', effort: '', prompt: '', permissions: '', machine_id: 'm-redwood', shortcut: 0 },
};

let desktop = JSON.stringify({
  launch_shortcuts: { accounts: ['claude:eliot@getfirewood.dev', 'codex:eliot@expo.io', 'claude:eliot@eliot.sh'], machines: ['m-local', 'm-redwood'] },
  model_catalogs: [], theme: '', ui_font_size: 13, terminal_font_size: 13, notify_turn_complete: true, notify_input_required: true, disable_mosh: false,
  remote_codex_app_server: false, remote_claude_native: false,
  windows: [{ tabs: [
    { key: 'folder:a1', title: 'ocho', reconnect: [], session: null, machine: null, depth: 0, collapsed: false, folder: true },
    { key: 'm-local:s1:false', title: 'Ocho client in exact2 · Eliots-MacBook-Pro', reconnect: ['attach', 'm-local', 's1'], session: ['m-local', 's1', false], machine: 'm-local', depth: 1, collapsed: false, folder: false },
    { key: 'm-redwood:s2:false', title: 'rename to ocho · redwood', reconnect: ['attach', 'm-redwood', 's2'], session: ['m-redwood', 's2', false], machine: 'm-redwood', depth: 1, collapsed: false, folder: false },
    { key: 'shell:b7', title: 'Shell · redwood', reconnect: ['connect', 'm-redwood'], session: null, machine: 'm-redwood', depth: 0, collapsed: false, folder: false },
  ], active: 0, rail_width: 236 }],
  launch: { machine: 'm-local', provider: 'claude', account: 'eliot@getfirewood.dev', cwd: '~/Developer/exact2', model: 'claude-fable-5-1', effort: 'high', permissions: '' },
  recent_projects: [{ machine: 'm-local', cwd: '~/Developer/exact2' }, { machine: 'm-redwood', cwd: '~/Developer/fleet' }],
  recent_models: [{ provider: 'claude', id: 'claude-fable-5-1', name: 'Fable 5.1' }], model_efforts: {},
}, null, 2);
let desktopMtime = now / 1000;

// The feed: the stream's first `state`, then one `machine` per machine, then
// indicators every few seconds so the working session's text moves.
let pending = [
  { type: 'state', state: { machines: machines.map((m) => ({ ...m, last: null })), accounts, presets }, latest_provider_versions: { codex: '0.160.0', claude: '2.1.4', opencode: '1.2.0' }, account_error: '' },
  ...machines.map((m) => ({ type: 'machine', machine: m })),
];
let ticks = 0;
function tick() {
  ticks += 1;
  const s1 = machines[0].last.sessions[0];
  s1.status_observed_at = Math.floor(Date.now() / 1000);
  s1.status_text = `Working… (${12 + ticks * 3}s · ↓ ${(3.2 + ticks * 0.4).toFixed(1)}k tokens)`;
  pending.push({ type: 'session-indicators', indicators: { machine_id: 'm-local', sessions: [s1], unchanged: [] } });
  announce?.('feed');
}

const catalogs = {
  codex: [['gpt-6-astra', 'GPT-6-Astra', 'Frontier intelligence for the most demanding work.'], ['gpt-6-sol', 'GPT-6-Sol', 'Workhorse model for coding and everyday work.'], ['gpt-6-luna', 'GPT-6-Luna', 'Fast and affordable model for easier tasks.']],
  claude: [['claude-fable-5-1', 'Fable 5.1', 'Most capable, for the hardest problems.'], ['claude-opus-5-5', 'Opus 5.5', 'Deep reasoning for complex work.'], ['claude-sonnet-5', 'Sonnet 5', 'Fast and balanced.']],
  opencode: [['opencode/default', 'Default', 'Whatever OpenCode is set up with.']],
};

const secrets = {
  'm-local:s1': [
    { id: 'r-stripe', name: 'STRIPE_API_KEY', reason: 'Check the webhook signing secret against the staging endpoint', status: 'pending' },
    { id: 'r-gh', name: 'GITHUB_TOKEN', reason: 'Read the private exact2 repository', status: 'provided', expires_at: Math.floor(Date.now() / 1000) + 1800 },
  ],
  'm-redwood:s2': [
    { id: 'r-npm', name: 'NPM_TOKEN', reason: 'Publish the renamed ocho package', status: 'pending' },
  ],
};

function job(argv, stdin) {
  const [cmd, ...rest] = argv;
  const ok = (stdout = '') => ({ status: 0, stdout, stderr: '' });
  switch (cmd) {
    case 'save-desktop': desktop = stdin; desktopMtime = Date.now() / 1000; return ok();
    case 'window-title': document.title = rest.join(' '); return ok();
    case 'open-url': window.open(rest.join(''), '_blank'); return ok();
    case 'clipboard-write': navigator.clipboard?.writeText(stdin).catch(() => {}); return ok();
    case 'clipboard-read': return ok('');
    case 'notify': case 'notify-remove': case 'close-terminal': case 'write-terminal': case 'refresh-feed': case 'clear-terminal': case 'reconnect-terminal': case 'save-title-prompt': return ok();
    case 'theme-files': return ok('[]');
    case 'whats-new-history': return ok("e6adfa8a6cc7a9cd99b26347cc7da8467f0b5631\t2026-10-01\tRun local Grok directly and allow verified mixed remote versions (#266)\nc03ff04e0320b8015d550df7e4d0373219799e13\t2026-10-01\tRemove Profiles manager tab (#267)\nb8139a71efddd2f703d8f7ffcda89c37657df148\t2026-10-01\tAdd Google OAuth accounts to Ocho (#260)\n134c1723c26f10212d9e8bc7b4510e2f3e95ab24\t2026-10-01\tRun Fleet and Ocho Codex sessions on EAS with DO and R2 persistence (#258)\n7847256dcfe2ee3f50e30a177f72e5a04131fb04\t2026-10-01\tFix transcript math layout, sharpness, and scrolling (#264)\nde58802d6faa7ab08ed14015a49119405f11c708\t2026-10-01\tBridge legacy fleet services to owner identity (#265)");
    case 'models': {
      const provider = rest[rest.indexOf('--provider') + 1] ?? 'claude';
      return ok(JSON.stringify((catalogs[provider] ?? []).map(([id, name, description], i) => ({ id, name, description, default: i === 0 }))));
    }
    case 'accounts': if (rest[0] === 'usage') return ok(JSON.stringify({ five_hour: { used_percent: 24, resets_at: iso(-3600e3) }, weekly: { used_percent: 61, resets_at: iso(-3 * 86400e3) } })); return ok('');
    case 'secret': {
      // fleet secret list|provide|dismiss|revoke|link M S …: s1 asks for two
      // secrets, s2 for one (its alert shows while s1's tab is active).
      const [verb, machine, sessionId, id] = rest;
      const list = (secrets[`${machine}:${sessionId}`] ??= []);
      if (verb === 'list') return ok(JSON.stringify(list));
      const item = list.find((r) => r.id === id);
      if (verb === 'provide' && item) { item.status = 'provided'; item.expires_at = Math.floor(Date.now() / 1000) + Number((rest[5] ?? '3600s').replace('s', '')); }
      if (verb === 'dismiss' && item) item.status = 'dismissed';
      if (verb === 'revoke' && item) item.status = 'revoked';
      if (verb === 'link') return ok(JSON.stringify(list.find((r) => r.name === id) ?? { id: `r-${id}`, name: id, reason: '', status: 'pending' }));
      return ok('');
    }
    case 'search': {
      if (rest[0] === 'index') return ok(JSON.stringify({ sessions: 5, indexed: 1, chunks: 4, skipped: 0, errors: {}, elapsed: '0.2s' }));
      const hits = [
        { machine: 'm-redwood', machine_name: 'redwood', session: 's3', title: 'devicehub cli', cwd: '~/Developer/devicehub', provider: 'codex', account: 'eliot@expo.io', score: 0.82, matches: 3, turn: 4, role: 'assistant', snippet: 'The CLI lists devices and streams logs; tests pass.', updated: iso(22 * 60e3), historical: false, tmux_pane: '%3' },
        { machine: 'm-local', machine_name: 'Eliots-MacBook-Pro', session: 's4', title: 'plan.stanf.org', cwd: '~/Developer/plan.stanf.org', provider: 'codex', account: 'eliot@expo.io', score: 0.61, matches: 1, turn: 2, role: 'user', snippet: 'Can you add the spring quarter to the planner?', updated: iso(3 * 3600e3), historical: true, tmux_pane: '' },
      ];
      return ok(JSON.stringify(hits));
    }
    case 'directories': return ok(JSON.stringify({ matches: ['~/Developer/exact2', '~/Developer/fleet', '~/Developer/devicehub'], more: false }));
    case 'desktop-update': return ok(JSON.stringify({ state: 'up_to_date' }));
    default: return ok('');
  }
}

export async function later(request) {
  const { op } = request;
  switch (op) {
    case 'feed': { const events = pending; pending = []; return { events, now: Date.now() / 1000 }; }
    case 'desktop': return { text: desktop, mtime: desktopMtime, home: '~/.local/share/fleet', now: Date.now() / 1000 };
    case 'io': {
      const jobs = request.jobs ?? [];
      await new Promise((r) => setTimeout(r, 40));
      return { replies: jobs.map((j) => ({ id: j.id, kind: j.kind, ...job(j.argv ?? [], j.stdin ?? '') })), now: Date.now() / 1000 };
    }
    default: throw new Error(`Ocho answers no ${op}`);
  }
}

const STYLE = `
:host { display: block; position: relative; background: #1b1e24; color: #d7dae0; font: 13px/18px ui-monospace, SFMono-Regular, Menlo, monospace; }
.pane { position: absolute; inset: 0; padding: 8px; overflow: hidden; }
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
  let argv = [];
  try { argv = JSON.parse(props.argv || '[]'); } catch { argv = []; }
  h.pane.innerHTML = `<div class="muted">ocho · ${h.tab}</div><div>$ fleet ${argv.join(' ')}</div><div class="muted">The terminal runs in the macOS app (libghostty). This is the web preview.</div><div><span class="cursor"></span></div>`;
}

export function destroy(h) { h.root.innerHTML = ''; }
