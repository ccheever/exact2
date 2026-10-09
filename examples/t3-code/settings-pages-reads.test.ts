// Settings pages read the connection's subscribed server config (settings-pages-subscribed-config, SC-1). Every
// `data.revision` change (any t3.status/t3.events wake: a thread list change, a provider status) asks each open
// settings page again, as app.contract keys them. Source control, Storage and Keybindings then sent server.getConfig
// and server.getSettings (Keybindings getConfig) again; Archive re-read its archived shell snapshot; Diagnostics
// re-read the resource timeline at each minute tick; the scheduled-task editor re-read every project's refs. The
// reference reads the server config and settings from the environment's subscribed config (useScopedSettings,
// serverConfig: server.getConfig when the connection starts, then subscribeServerConfig) and its page queries once per
// mount and on Refresh or its own actions (client-runtime state/runtime.ts createEnvironmentQueryAtomFamily: Atom.swr
// revalidates on mount only; no refreshIntervalMs on these). These tests go through the app's own answer(), as the
// Contract asks it, with the page asked again whenever one of its arguments changes.
import { afterEach, describe, expect, test } from 'bun:test';
import { answer } from './app';
import { Backend, storage, thread } from './client-fixture';
import { arr, obj, str, type Obj } from './domain';
import { SCHEDULED_TASKS_KEY } from './live-streams';
import { resetPrimary } from './local-primary-fixture';
import { resetHighlightSlicing } from './r12-render-highlight';
import { TELEMETRY_KEY } from './settings-a-telemetry';

const NOW = Date.parse('2026-10-10T08:00:00.000Z');
const task = (id: string, title: string): Obj => ({ id, title, prompt: 'Check the build', enabled: true, projectId: 'p1', threadId: null,
  schedule: { type: 'fixed_time', timeOfDay: '09:00' }, workspaceStrategy: { type: 'worktree', baseRef: 'main', startFromOrigin: true },
  modelSelection: { instanceId: 'codex-personal', model: 'model-a' }, runtimeMode: 'full-access', interactionMode: 'default', lastRunStatus: 'never', runCount: 0 });

/** The lane server behind the fake transport, with the reads these pages make and the streams they open. */
class PageServer extends Backend {
  woke = false;
  tasks: Obj[] = [task('task-1', 'Nightly check')];
  override emit(key: string, value: Obj, subscriptionId = this.subscriptions[key]) { super.emit(key, value, subscriptionId); this.woke = true; }
  override async later(input: unknown): Promise<unknown> {
    const request = obj(input), method = str(request.method);
    if (request.op === 'subscribe' && (request.key === SCHEDULED_TASKS_KEY || request.key === TELEMETRY_KEY)) {
      this.calls.push(request);
      const id = `sub-${++this.serial}`;
      this.subscriptions[str(request.key)] = id;
      if (request.key === SCHEDULED_TASKS_KEY) this.emit(SCHEDULED_TASKS_KEY, { tasks: this.tasks }); // the full list first, as the server's stream
      return this.good({ id });
    }
    if (request.op === 'request' && method === 'vcs.listRefs') { this.calls.push(request); return this.good({ refs: [{ name: 'main', current: true, isDefault: true }] }); }
    if (request.op === 'request' && method === 'server.discoverSourceControl') { this.calls.push(request); return this.good({ versionControlSystems: [], sourceControlProviders: [] }); }
    if (request.op === 'request' && method.startsWith('server.get') && !['server.getConfig', 'server.getSettings'].includes(method)) { this.calls.push(request); return this.good({ readAt: '2026-10-10T07:59:00.000Z' }); }
    return super.later(input);
  }
  /** The server requests sent since `from`, by method. */
  requests(from: number) { return this.calls.slice(from).filter(call => call.op === 'request').map(call => str(call.method)); }
  /** A thread list change on the shell stream (the live drive's `t3 project rename`). */
  rename(title: string) { this.emit('shell', { kind: 'project.updated', sequence: ++this.sequence, project: { ...obj(arr(this.shell.projects)[0]), title } }); }
}

type Clock = { revision: number; now: number; refresh: number; telemetry: number };
type Page = { source: string; args: (clock: Clock, open: boolean) => unknown[]; opens: string[] };
const PAGES: Record<string, Page> = {
  'Source control': { source: 'sourceControlPage', args: (c, open) => ['env1', '', open, c.refresh, c.revision], opens: ['server.discoverSourceControl'] },
  Storage: { source: 'storageSettings', args: (c, open) => ['env1', '', open, c.refresh, c.revision], opens: [] },
  Archive: { source: 'archivedSettings', args: (c, open) => ['env1', '', open, c.now, c.refresh, c.revision], opens: ['orchestration.getArchivedShellSnapshot'] },
  Diagnostics: { source: 'diagnosticsSettings', args: (c, open) => ['env1', '15m', open, c.now, c.refresh, c.revision, c.telemetry],
    opens: ['server.getProcessDiagnostics', 'server.getProcessResourceHistory', 'server.getTraceDiagnostics', 'server.getResourceTelemetryHistory'] },
  'Scheduled tasks': { source: 'scheduledSettings', args: (c, open) => ['env1', '', '', '', open, c.now, c.refresh, c.revision, '', '', ''], opens: [] },
  Keybindings: { source: 'keybindingSettings', args: (c, open) => ['env1', '', open, '', '', '', '', c.refresh, c.revision], opens: [] },
};

// The app's client adopts only a newer generation (client.ts adoptStatus); the app tests share it in file order, and
// this file runs after settings-integrations-reads.test.ts (60, 61).
let generation = 79;
async function launched(setup: (server: PageServer) => void = () => {}) {
  const server = new PageServer(), files = storage().files;
  server.generation = ++generation; server.serial = generation * 1000;
  server.config = { ...server.config, environment: { ...obj(server.config.environment), capabilities: { ...obj(obj(server.config.environment).capabilities), storageCleanup: true, projectSettingsOverrides: true } },
    keybindings: [{ command: 'sidebar.toggle', shortcut: { key: 'b', modKey: true } }], keybindingsConfigPath: '/lane/keybindings.json',
    observability: { logsDirectoryPath: '/lane/logs' }, settings: { ...obj(server.config.settings), storageCleanup: { logsAfterDays: 7 } } };
  server.shell = { ...server.shell, archivedThreads: [{ ...thread('a1'), title: 'Old idea', archivedAt: '2026-10-09T08:00:00.000Z', createdAt: '2026-10-08T08:00:00.000Z' }] };
  setup(server);
  const clock: Clock = { revision: 0, now: NOW, refresh: 0, telemetry: 0 };
  const data = async () => { const value = obj(await answer('snapshot', [clock.now], null, files, server)); clock.revision = Number(value.revision); clock.telemetry = Number(value.telemetry) || 0; };
  for (let turn = 0; turn < 8; turn++) { server.woke = false; await data(); if (!server.woke) break; }
  /** Asks the page as the Contract does when one of its arguments changed; returns its view and the requests it sent. */
  const ask = async (page: Page, open = true) => {
    const from = server.calls.length, view = obj(await answer(page.source, page.args(clock, open), null, files, server));
    return { view, sent: server.requests(from) };
  };
  /** Drains a wake into `data`; the page is asked again when data.revision (or data.telemetry) moved. */
  const wake = async (page: Page) => {
    const before = `${clock.revision}|${clock.telemetry}`;
    for (let turn = 0; turn < 8; turn++) { server.woke = false; await data(); if (!server.woke) break; }
    expect(`${clock.revision}|${clock.telemetry}`).not.toBe(before);
    return ask(page);
  };
  return { server, files, clock, ask, wake };
}

afterEach(async () => {
  resetPrimary(); resetHighlightSlicing();
  for (const page of Object.values(PAGES)) await answer(page.source, page.args({ revision: 0, now: 0, refresh: 0, telemetry: 0 }, false), null, storage().files, null); // the pages close
});

describe('each settings page reads the subscribed config and asks the server only when the reference does', () => {
  for (const [name, page] of Object.entries(PAGES)) {
    test(`${name}: opening reads only the page's own data, and a wake, a minute tick and a reopen read nothing new`, async () => {
      const { server, clock, ask, wake } = await launched();
      const opened = await ask(page);
      expect(opened.view.error ?? '').toBe('');
      expect(opened.sent.sort()).toEqual([...page.opens].sort());
      // A thread list change wakes `data`; its drain bumps data.revision, and the page is asked again.
      server.rename('Example (renamed)');
      expect((await wake(page)).sent).toEqual([]);
      server.rename('Example');
      expect((await wake(page)).sent).toEqual([]);
      // The clock's minute tick (wallTime.epochAtZero + elapsed) asks Archive, Diagnostics and Scheduled tasks again.
      clock.now += 60_000;
      expect((await ask(page)).sent).toEqual([]);
      // Leaving the page and coming back is a new visit: the page's own queries run again, as on the reference's mount.
      await ask(page, false);
      expect((await ask(page)).sent.sort()).toEqual(page.opens.filter(method => method !== 'server.discoverSourceControl').sort());
    });
  }
});

describe('a change still reaches each page, without a read', () => {
  test('Source control and Storage show a settingsUpdated value from subscribeServerConfig', async () => {
    const { server, ask, wake } = await launched();
    expect((await ask(PAGES['Source control']!)).view.repositories).toMatchObject([{ checked: false }, {}]);
    expect(obj(arr((await ask(PAGES.Storage!)).view.artifacts).find(row => row.key === 'logsAfterDays'))).toMatchObject({ enabled: true, days: '7' });
    server.config.settings = { ...obj(server.config.settings), defaultAutoPull: true, storageCleanup: { logsAfterDays: 14 } };
    server.emit('config', { type: 'settingsUpdated', payload: { settings: server.config.settings } });
    const source = await wake(PAGES['Source control']!), stored = await ask(PAGES.Storage!);
    expect({ sent: [...source.sent, ...stored.sent], pull: obj(arr(source.view.repositories)[0]).checked, logs: obj(arr(stored.view.artifacts).find(row => row.key === 'logsAfterDays')).days })
      .toEqual({ sent: [], pull: true, logs: '14' });
  });

  test('Keybindings show a keybindingsUpdated list', async () => {
    const { server, ask, wake } = await launched();
    expect((await ask(PAGES.Keybindings!)).view).toMatchObject({ count: '1 binding', path: '/lane/keybindings.json' });
    server.config.keybindings = [...arr(server.config.keybindings), { command: 'chat.new', shortcut: { key: 'n', modKey: true } }];
    server.emit('config', { type: 'keybindingsUpdated', payload: { keybindings: server.config.keybindings, issues: [] } });
    const view = await wake(PAGES.Keybindings!);
    expect({ sent: view.sent, count: view.view.count }).toEqual({ sent: [], count: '2 bindings' });
  });

  test('Scheduled tasks show the live list and the editor a new default model; the editor reads its refs once', async () => {
    const { server, ask, wake } = await launched();
    const editor: Page = { ...PAGES['Scheduled tasks']!, args: (c, open) => ['env1', '', 'task', '', open, c.now, c.refresh, c.revision, '', '', ''] };
    expect((await ask(PAGES['Scheduled tasks']!)).sent).toEqual([]); // scheduledTasks.subscribe; its first list arrives as an event
    expect((await wake(PAGES['Scheduled tasks']!)).view.tasks).toMatchObject([{ title: 'Nightly check' }]);
    const opened = await ask(editor);
    expect({ sent: opened.sent, model: obj(arr(opened.view.editors)[0]).modelKey }).toEqual({ sent: ['vcs.listRefs'], model: 'codex-personal:model-a' });
    server.tasks = [...server.tasks, task('task-2', 'Weekly report')];
    server.emit(SCHEDULED_TASKS_KEY, { tasks: server.tasks });
    server.config.settings = { ...obj(server.config.settings), defaultModelSelection: { instanceId: 'codex-personal', model: 'model-b' } };
    server.emit('config', { type: 'settingsUpdated', payload: { settings: server.config.settings } });
    const view = await wake(editor);
    expect({ sent: view.sent, tasks: arr(view.view.tasks).map(entry => entry.title), model: obj(arr(view.view.editors)[0]).modelKey })
      .toEqual({ sent: [], tasks: ['Nightly check', 'Weekly report'], model: 'codex-personal:model-b' });
    // Closing the editor and opening it again reads the refs again, as the picker's mount does.
    await ask(PAGES['Scheduled tasks']!);
    expect((await ask(editor)).sent).toEqual(['vcs.listRefs']);
  });

  test("Archive and Diagnostics read again on Refresh and after the page's own action, once each", async () => {
    const { clock, ask, files, server } = await launched();
    expect((await ask(PAGES.Archive!)).view.groups).toMatchObject([{ threads: [{ title: 'Old idea' }] }]);
    await ask(PAGES.Diagnostics!);
    // Unarchive (a rest command: app.contract bumps settingsRefresh when it succeeds) reads the archive once more.
    expect(obj(await answer('command', ['unarchive-thread', 'env1:p1:a1', '', 0], null, files, server)).message).toBe('');
    clock.refresh++;
    const archive = await ask(PAGES.Archive!);
    expect({ sent: archive.sent, groups: archive.view.groups }).toEqual({ sent: ['orchestration.getArchivedShellSnapshot'], groups: [] });
    const diagnostics = await ask(PAGES.Diagnostics!);
    expect(diagnostics.sent.sort()).toEqual([...PAGES.Diagnostics!.opens].sort());
    expect((await ask(PAGES.Diagnostics!)).sent).toEqual([]);
  });
});
