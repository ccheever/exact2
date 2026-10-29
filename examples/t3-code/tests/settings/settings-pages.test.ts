// Settings pages walked as their contracts draw them. Each route's page is its panel from app-settings.contract, inlined
// component by component (a use's literal and evaluable props bind its view; `children` is the use's body), with the
// `if` arms its state decides and every arm it does not. A dialog or popover is not the page's: the walk leaves it.
//
// - The route modal's guard (dialog-trap.contract RouteFocusGuard) wraps Shift+Tab from the first stop to the last
//   mounted `[data-focus-stop~="page"]`, as Base UI's trap wraps to the popup's last tabbable. So every page control
//   that is a Tab stop carries an id and `page`, and the guard lands on the page's real last control.
// - Every search result lands on an element its own page draws in that page's state (upstream's `targetId ?? id`).
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { appFiles, appPath, appText } from '../support/app-files';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import type { T3Client } from '../../src/client/client';
import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
import { decodeClientPrefs, diagnosticsRow, generalSections, resolveScope, serverContext } from '../../src/settings/settings-core';
import { appearanceSections } from '../../src/appearance/settings-appearance';
import { settingsCatalog } from '../../src/settings/settings-search';
import { branchNamingRows, integrationRows, sourceControlRows } from '../../src/settings/source-control-view';
import { buildRows } from '../../src/settings/keybinding-view';
import { cliCommandRow } from '../../src/settings/cli-command';

type Line = { indent: number; text: string; at: string };
type Node = { line: Line; body: Node[] };
type Component = { file: string; view: Line[] };

const contracts = (() => {
  const files = new Map<string, { components: Map<string, Component>; uses: Map<string, string> }>();
  for (const file of appFiles().filter(name => name.endsWith('.contract') && !name.endsWith('.test.contract'))) {
    const components = new Map<string, Component>(), uses = new Map<string, string>();
    let component: Component | null = null, inView = false, viewIndent = 0;
    readFileSync(appPath(file), 'utf8').split('\n').forEach((raw, index) => {
      const indent = raw.length - raw.trimStart().length, text = raw.trim();
      if (text === '' || text.startsWith('//')) return;
      if (indent === 0) {
        component = null; inView = false;
        const use = /^use (.+) from "(?:[^"]*\/)?([^/"]+)"/.exec(text);
        if (use) for (const name of use[1]!.split(',')) uses.set(name.trim(), use[2]!);
        const declared = /^component (\w+)/.exec(text);
        if (declared) components.set(declared[1]!, component = { file, view: [] });
        return;
      }
      if (!component) return;
      if (inView && indent <= viewIndent) inView = false;
      if (inView) (component as Component).view.push({ indent, text: text.replace(/\s+\/\/ .*$/, ''), at: `${file}:${index + 1}` });
      else if (text === 'view') { inView = true; viewIndent = indent; }
    });
    files.set(file, { components, uses });
  }
  const resolve = (file: string, name: string): Component | null => {
    const scope = files.get(file), from = scope?.uses.get(name);
    return scope?.components.get(name) ?? (from && from !== file ? resolve(from, name) : null);
  };
  return { resolve };
})();

function tree(lines: Line[]): Node[] {
  const root: Node[] = [], stack: { indent: number; body: Node[] }[] = [{ indent: -1, body: root }];
  for (const line of lines) {
    while (stack.at(-1)!.indent >= line.indent) stack.pop();
    const node = { line, body: [] };
    stack.at(-1)!.body.push(node);
    stack.push({ indent: line.indent, body: node.body });
  }
  return root;
}

/** A view line's tag, its positional value and its attributes, values as written (help-tags.test.ts's reading). */
function attributes(line: string): { tag: string; attrs: Map<string, string> } {
  const tag = /^[A-Za-z][\w-]*/.exec(line)?.[0] ?? '';
  const attrs = new Map<string, string>();
  let p = tag.length;
  const readValue = () => {
    const start = p;
    let depth = 0;
    while (p < line.length) {
      const ch = line[p]!;
      if (ch === '"') { p++; while (p < line.length && line[p] !== '"') p += line[p] === '\\' ? 2 : 1; p++; continue; }
      if (ch === '`') { p++; let inner = 0; while (p < line.length && !(line[p] === '`' && inner === 0)) { if (line.startsWith('${', p)) { inner++; p++; } else if (line[p] === '}' && inner > 0) inner--; p++; } p++; continue; }
      if ('([{'.includes(ch)) depth++;
      else if (')]}'.includes(ch)) { if (depth === 0) break; depth--; }
      else if (depth === 0 && (/\s/.test(ch) || ch === ',')) break;
      p++;
    }
    return line.slice(start, p);
  };
  // A component use's arguments sit in parentheses, comma-separated; an element's attributes follow its tag.
  if (line[p] === '(') p++;
  for (;;) {
    while (p < line.length && /[\s,)]/.test(line[p]!)) p++;
    if (p >= line.length || line.startsWith('//', p)) break;
    const name = /^-?[A-Za-z_][\w-]*(:[\w-]+)?=(?!=)/.exec(line.slice(p));
    if (name) { p += name[0].length; attrs.set(name[0].slice(0, -1), readValue()); } else readValue();
  }
  return { tag, attrs };
}

const UNKNOWN = Symbol('unknown');
const evaluate = (expr: string, env: Env): unknown => { try { return value(expr, env); } catch { return UNKNOWN; } };

type Control = { at: string; text: string; id: unknown; mark: unknown };
type Walk = { controls: Control[]; ids: Set<string> };

/** The page `component` of `file` draws with `props`: its stops in tree order and the ids it gives its elements. */
function walkPage(file: string, component: string, props: Env): Walk {
  const out: Walk = { controls: [], ids: new Set() };
  const root = contracts.resolve(file, component);
  if (!root) throw new Error(`${file}: no component ${component}`);
  type Slot = { nodes: Node[]; file: string; env: Env; slot: Slot | null } | null;
  const visit = (nodes: Node[], at: string, env: Env, slot: Slot) => {
    let chain: 'none' | 'taken' | 'open' = 'none';
    for (const node of nodes) {
      const text = node.line.text;
      const branch = /^(else if|if) (.+)$/.exec(text), otherwise = text === 'else';
      if (branch || otherwise) {
        if (branch?.[1] === 'if') chain = 'none';
        if (chain === 'taken') continue;
        const holds = otherwise ? true : evaluate(branch![2]!, env);
        if (holds === false || holds === 0 || holds === '') { if (chain === 'none') chain = 'none'; continue; }
        if (holds !== UNKNOWN) chain = 'taken';
        else chain = 'open';
        visit(node.body, at, env, slot);
        continue;
      }
      chain = 'none';
      const loop = /^for (\w+) in (.+?)(?: key=.*)?$/.exec(text);
      if (loop) {
        const list = evaluate(loop[2]!, env);
        const items = Array.isArray(list) ? list.slice(0, 3) : list === UNKNOWN ? [UNKNOWN] : [];
        for (const item of items) visit(node.body, at, item === UNKNOWN ? env : { ...env, [loop[1]!]: item }, slot);
        continue;
      }
      if (text === 'children') { if (slot) visit(slot.nodes, slot.file, slot.env, slot.slot); continue; }
      const { tag, attrs } = attributes(text);
      if (attrs.has('popover')) continue; // a popover's content is not the page's
      if (/^[A-Z]/.test(tag)) {
        if (/(Dialog|Overlay)$/.test(tag)) continue;
        const used = contracts.resolve(at, tag);
        if (!used) continue; // an icon or another leaf
        const inner: Env = {};
        for (const [name, expr] of attrs) { const v = evaluate(expr, env); if (v !== UNKNOWN) inner[name] = v; }
        visit(tree(used.view), used.file, inner, { nodes: node.body, file: at, env, slot });
        continue;
      }
      const id = attrs.has('id') ? evaluate(attrs.get('id')!, env) : undefined;
      if (typeof id === 'string' && id !== '') out.ids.add(id);
      const tabindex = attrs.get('tabindex');
      const stop = ['button', 'link', 'input', 'textarea'].includes(tag) ? tabindex !== '-1' : tabindex !== undefined && tabindex !== '-1';
      const disabled = attrs.has('disabled') && evaluate(attrs.get('disabled')!, env) === true;
      if (stop && attrs.get('aria-hidden') !== 'true' && !disabled) {
        const raw = attrs.get('data-focus-stop');
        const known = raw === undefined ? undefined : evaluate(raw, env);
        // A mark this state cannot decide reads with the props it knows written in (a bound `stop` word).
        const mark = known !== UNKNOWN ? known : raw!.replace(/"[^"]*"|\b[A-Za-z_]\w*\b/g, token => token.startsWith('"') || typeof env[token] !== 'string' ? token : JSON.stringify(env[token]));
        if (mark !== '') out.controls.push({ at: node.line.at, text, id: id ?? attrs.get('id'), mark });
      }
      visit(node.body, at, env, slot);
    }
  };
  visit(tree(root.view), root.file, props, null);
  return out;
}

const pageMark = (control: Control) => typeof control.mark === 'string' && (control.mark.split(/\s+/).includes('page') || control.mark.includes('"enabled page"'));

// Each route's panel and the props app-settings.contract gives it, in a state that draws its controls. `any` is a row or
// value whose every field is unknown, so the walk takes each arm a field decides.
const any: unknown = new Proxy({}, { get: (_, key) => { if (key === Symbol.toPrimitive || key === 'toJSON') return undefined; throw new Error(`unknown ${String(key)}`); } });
/** `fields` with every other field unknown, as `any`. */
const some = (fields: Record<string, unknown>): unknown => new Proxy(fields, { get: (target, key) => {
  if (typeof key === 'string' && key in target) return target[key];
  if (key === Symbol.toPrimitive || key === 'toJSON') return undefined;
  throw new Error(`unknown ${String(key)}`);
} });
const host = { busy: false, error: '', writable: true };
const pages: Record<string, [file: string, component: string, props: Env]> = {
  chat: ['settings-core-body.contract', 'SettingsCoreBody', { route: 'chat', busy: false, core: { notice: '', message: '', showScope: false, sections: [any] } }],
  'source-control': ['settings-source-control.contract', 'SourceControlPanel', { ...host, data: { available: true, error: '', scanned: true, vcs: [{ kind: 'git', details: false }],
    providers: ['github', 'gitcafe', 'bitbucket'].map(kind => ({ kind, details: true })), repositories: [{}], writing: [{}], writingAdvanced: [{}], hostCredentials: [] } }],
  browser: ['settings-source-control.contract', 'IntegrationsPanel', { ...host, view: 'browser', data: { available: true, error: '', browser: [{}] } }],
  devices: ['settings-source-control.contract', 'IntegrationsPanel', { ...host, view: 'devices', data: { available: true, error: '', simulatorSupport: [any],
    deviceHosts: { available: true, canAdd: true, groups: [any] }, easHosts: { supported: true, disabled: false, editing: false, hosts: [{ id: 'eas-1', label: 'EAS', limits: '' }] } } }],
  worktrees: ['settings-storage.contract', 'StoragePanel', { ...host, view: 'worktrees', data: { available: true, directorySupported: true, error: '', notice: '', creation: [{}], creationAdvanced: [{}],
    branches: [{}], worktrees: [{}], worktreesAdvanced: [{}], keepSupported: true, project: false, databaseNotices: [], artifacts: [], cleanupAvailable: false, cleanupReports: [], diagnostics: [] } }],
  maintenance: ['settings-storage.contract', 'StoragePanel', { ...host, view: 'maintenance', data: { available: true, error: '', notice: '', databaseNotices: [{}], artifacts: [{}],
    cleanupAvailable: true, cleanupReports: [{}], diagnostics: [{}], creation: [], creationAdvanced: [], branches: [] } }],
  providers: ['providers.contract', 'ProvidersPanel', { busy: false, page: { available: true, writable: true, macos: true, rows: [{}], editors: [{}], hubs: [{}] } }],
  projects: ['settings-projects.contract', 'ProjectsPanel', { ...host, data: { ready: true, selected: true } }],
  keybindings: ['settings-keybindings.contract', 'KeybindingsPanel', { busy: false, error: '', adding: false, data: { available: true, error: '', writable: true, groups: [{ rows: [{}] }] } }],
  archived: ['settings-archive.contract', 'ArchivedPanel', { busy: false, error: '', data: { available: true, groups: [{ id: 'g1', first: true, threads: [{}] }, { id: 'g2', first: false, threads: [{}] }] } }],
  'scheduled-tasks': ['settings-scheduled.contract', 'ScheduledPanel', { busy: false, error: '', data: { available: true, writable: true, items: [{}] } }],
  'open-source-licenses': ['settings-licenses.contract', 'NoticesPanel', { query: '', data: { entries: [{ id: 'react', sourceUrl: 'https://github.com/facebook/react' }] } }],
  diagnostics: ['settings-diagnostics.contract', 'DiagnosticsPanel', { busy: false, data: { available: true } }],
  connections: ['connections.contract', 'ConnectionsPanel', { view: 'connections', busy: false, error: '', still: true, menu: '', width: 640,
    page: some({ updateCount: 1, environments: [some({ environmentId: 'e1', key: 'k1' })], machines: ['m1', 'm2'].map(environmentId => some({ environmentId, key: environmentId })) }) }],
  'connections (one machine)': ['connections.contract', 'ConnectionsPanel', { view: 'connections', busy: false, error: '', still: true, menu: '', width: 640,
    page: some({ updateCount: 0, environments: [], machines: [] }) }],
  access: ['connections.contract', 'ConnectionsPanel', { view: 'access', busy: false, error: '', still: true, menu: '', width: 640, page: any }],
  // Appearance's library over its sections as settings-appearance.ts builds them (Separate fonts on draws every font row).
  appearance: ['settings-core-body.contract', 'SettingsCoreBody', { route: 'appearance', busy: false, menu: '', core: { notice: '', message: '', showScope: false,
    tiles: [any], themes: [some({ id: 'custom-1', collection: false, custom: true, orbs: [any] }), some({ id: 'collection-1', collection: true, custom: true })],
    sections: appearanceSections(decodeClientPrefs({ typographyAdvanced: true }), true, []) } }],
  // SnapShots with capture on and a new shortcut waiting: every row, and the shortcut's Save and Cancel.
  'snap-shot': ['snapshot.contract', 'SnapshotPanel', { busy: false, setupOpen: false, data: some({ error: '', setupLabel: 'Open System Settings', enabled: true, captureAvailable: true,
    recording: false, changed: true, canSave: true }) }],
};

describe('every Settings page stop carries `page`', () => {
  test.each(Object.keys(pages))('%s', route => {
    const [file, component, props] = pages[route]!;
    const { controls } = walkPage(file, component, props);
    if (process.env.DEBUG_PAGES === route) for (const control of controls) console.log(control.at, JSON.stringify(control.mark), JSON.stringify(control.id));
    expect(controls.length).toBeGreaterThan(0);
    // A stop the guard can return is an element with its own id and the `page` word (querySelectorAll's answer).
    expect(controls.filter(control => !pageMark(control) || control.id === undefined || control.id === '').map(control => `${control.at} ${control.text.slice(0, 90)}`)).toEqual([]);
  });
});

describe("Shift+Tab from the first stop wraps to the page's last control (RouteFocusGuard)", () => {
  const guard = contractComponent('dialog-trap.contract', 'RouteFocusGuard');
  const shiftTab = { shiftKey: true, metaKey: false, ctrlKey: false, altKey: false };
  test.each([
    ['open-source-licenses', 'View project source for'],
    ['devices', 'EasGhostButton'],
    // Environments: Add environment with this machine alone; with more, Pull request sharing's last machine (its fold open).
    ['connections (one machine)', 'manage-connection'],
    ['connections', 'CnSelect'],
    ['access', 'NetXs'],
    // Appearance: Typography's last row (its Advanced fold open); SnapShots: Capture animations; Browser: Show mouse
    // presses in recordings.
    ['appearance', 'ScopedSwitch'],
    ['snap-shot', 'SnapSwitch'],
    ['browser', 'SwitchControl'],
  ] as const)("%s lands on its last control", (route, last) => {
    const [file, component, props] = pages[route]!;
    // The page's stops in tree order, named by their view line, as querySelectorAll names them by id.
    const stops = walkPage(file, component, props).controls.filter(pageMark).map(control => control.at);
    const focused: string[] = [], scrolled: string[] = [];
    guard.call('hold', {}, { guardId: 'route-modal-guard-start', targets: ['settings-search', 'settings-close'], popup: 'settings-modal', preventDefault() {}, stopPropagation() {},
      focus: (id: string) => focused.push(id), scrollIntoView: (id: string) => scrolled.push(id),
      querySelectorAll: (selector: string) => selector.includes('"page"') ? stops : ['settings-search', 'settings-close'] }, ['Tab', shiftTab]);
    expect(focused).toEqual([stops.at(-1)]);
    expect(scrolled).toEqual(focused); // and scrolls it into view, as the web's focus() does
    const line = appText(focused[0]!.split(':')[0]!).split('\n')[Number(focused[0]!.split(':')[1]) - 1]!;
    const owner = appText(focused[0]!.split(':')[0]!).split('\n').slice(0, Number(focused[0]!.split(':')[1])).reverse().find(text => text.startsWith('component '))!;
    expect(`${owner} ${line}`).toContain(last);
  });
});

describe('every result lands on an element its own page draws, in that page state', () => {
  // The ids a page's contracts give in that state (the walk), and the ones its data names: core sections and rows
  // (CoreSections, CoreRowView), scoped rows' targets (ScopedSettingRow) and keybinding rows' anchors.
  // One connected server that settles threads and continues them after restarts, as the catalog's results assume.
  const client = {
    local: { deviceSettings: { composerCollapseOnScroll: true, planModeEnabled: false, timestampFormat: 'locale', appearanceMode: 'system', sendShortcut: 'enter' },
      clientSettings: decodeClientPrefs({}), groupingMode: 'repository' },
    scopes: AUTH_STANDARD_CLIENT_SCOPES, ready: true, environmentId: 'env1', projectGroups: () => [],
    config: { environment: { environmentId: 'env1', label: 'Studio', serverVersion: '0.0.46',
      capabilities: { projectSettingsOverrides: true, threadAutoSettlement: true, threadRestartContinuation: true } }, providers: [], settings: { projectSettingsOverrides: {} } },
  } as unknown as T3Client;
  const context = serverContext(client, resolveScope(client, '', '', ''), new Map());
  type Sections = { id: string; rows: { id: string }[]; advanced?: { id: string }[] }[];
  const sectionIds = (sections: Sections) => sections.flatMap(section => [section.id, ...section.rows.map(row => row.id), ...(section.advanced ?? []).map(row => row.id)]);
  const core = (route: string) => sectionIds(generalSections(client, context, route));
  const targets = (rows: { target: string }[]) => rows.map(row => row.target).filter(Boolean);
  const walked = (route: string, props: Env = pages[route]![2]) => [...walkPage(pages[route]![0], pages[route]![1], props).ids];
  const sourceControl = sourceControlRows({}, '', 'Local', [], true);
  const keybindings = [{ command: 'sidebar.toggle', key: 'mod+b' }, { command: 'thread.stop', key: 'mod+.' }];
  const anchors = buildRows(keybindings.map(({ command }) => ({ command, shortcut: { key: 'b', modKey: true } })), '').map(row => row.anchor).filter(Boolean);
  const archive = (groups: unknown[]) => walkPage('settings-archive.contract', 'ArchivedPanel', { busy: false, error: '', data: { available: true, loading: false, error: '', groups } }).ids;
  const ids: Record<string, string[]> = {
    general: core('general'), chat: core('chat'), diffs: core('diffs'), threads: core('threads'), labs: core('labs'),
    about: [...core('about'), cliCommandRow({ supported: true, installedPath: null, onPath: false })!.id],
    // Typography's separate font rows show with Separate fonts on, which a jump to one turns on first.
    appearance: [...[false, true].flatMap(typographyAdvanced => sectionIds(appearanceSections(decodeClientPrefs({ typographyAdvanced }), true, []))),
      ...walkPage('settings-core-body.contract', 'SettingsCoreBody', { route: 'appearance', busy: false, core: { notice: '', message: '', showScope: false, sections: [] } }).ids],
    maintenance: [...core('maintenance'), ...walked('maintenance'), diagnosticsRow(false).id],
    worktrees: [...core('worktrees'), ...walked('worktrees'), ...targets(branchNamingRows({}, '', 'Local', true))],
    'source-control': [...walked('source-control'), ...targets([...sourceControl.repositories, ...sourceControl.writing, ...sourceControl.writingAdvanced])],
    browser: [...walked('browser'), ...targets(integrationRows({}, '', 'Local', true).browser)],
    devices: walked('devices'), providers: walked('providers'), projects: walked('projects'), access: walked('access'), 'snap-shot': walked('snap-shot'),
    keybindings: [...walked('keybindings'), ...anchors],
    archived: [...archive([{ id: 'g1', first: true, threads: [any] }, { id: 'g2', first: false, threads: [any] }])],
  };
  // Results whose element their page's owner has yet to draw, by route.
  const waiting: Record<string, string[]> = {};
  const items = settingsCatalog({ connected: true, autoSettle: true, scopeKind: 'all', keybindings, localBackend: true, localEnvironment: true });
  const lands = (item: { route: string; target: string }) => (ids[item.route] ?? []).includes(item.target)
    || item.route === 'connections' && walkPage('connections.contract', 'ConnectionsPanel', { view: 'connections', busy: false, error: '', page: any }).ids.has(item.target);

  test('each result, keybinding commands included', () => {
    expect(items.filter(item => item.route === 'keybindings').map(item => item.target)).toEqual(['keybindings', 'keybinding-sidebar.toggle', 'keybindings']);
    expect(items.filter(item => !lands(item) && !(waiting[item.route] ?? []).includes(item.id)).map(item => `${item.route}#${item.target}`)).toEqual([]);
    // The waiting list holds only results that do not land yet, so it shrinks as their pages draw them.
    expect(items.filter(item => (waiting[item.route] ?? []).includes(item.id) && lands(item)).map(item => item.id)).toEqual([]);
  });

  test('Archived threads lands on the first group while there are groups, on the empty state while there are none', () => {
    expect([...archive([{ id: 'g1', first: true, threads: [any] }, { id: 'g2', first: false, threads: [any] }])].filter(id => id.startsWith('archive'))).toEqual(['archive', 'archive-scroll-target']);
    expect([...archive([])].filter(id => id.startsWith('archive'))).toEqual(['archive', 'archive-scroll-target']);
  });

  test("a view's ids are its own: Browser's are not Devices' or Source control's, Worktrees' are not Maintenance's", () => {
    expect([ids.browser!.includes('browser'), ids.devices!.includes('browser'), ids['source-control']!.includes('browser')]).toEqual([true, false, false]);
    expect([ids.devices!.includes('devices'), ids.browser!.includes('devices')]).toEqual([true, false]);
    expect([ids.worktrees!.includes('storage-worktrees'), ids.maintenance!.includes('storage-worktrees')]).toEqual([true, false]);
    expect([ids.maintenance!.includes('storage-delete-now'), ids.worktrees!.includes('storage-delete-now')]).toEqual([true, false]);
  });
});
