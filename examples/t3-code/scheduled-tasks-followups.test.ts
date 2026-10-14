// scheduled-tasks-followups: the three items live-automations-and-clones left, against T3 Code 1e2ecbd975 (MIT, see
// LICENSE-T3). ST-1: ScheduledTaskEditorDialog's "Runs on" switches a new task's environment (the scope's connected
// environments; project, model, base branch, Start from origin and checkout path start over, the rest is kept). ST-2:
// ProjectCloneToastCoordinator's "Open project" on every environment's success toast (handleNewThread with that
// environment's project ref). ST-3: ThreadAutomationsPanel's three TooltipPopups.
import { afterEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import type { Files, Native } from './protocol';
import { initialShell, obj, type Obj } from './domain';
import { liveEvent, liveFleetEvent, liveFleetPass, watchLive, PROJECT_CLONES_KEY, SCHEDULED_TASKS_KEY } from './live-streams';
import { editTarget, scheduledPage } from './scheduled-view';
import { cloneToasts } from './project-clones-live';
import { cloneToast } from './project-clones';
import { toasts } from './toast';
import { fleet, focusFleetProject, type FleetEntry } from './settings-b-fleet';
import { threadOps } from './client-ops-threads';

const NOW = Date.parse('2026-10-11T12:00:00.000Z');
const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
/** The lines of `component name` in `file`, up to the next top-level declaration. */
async function component(file: string, name: string): Promise<string> {
  const lines = (await source(file)).split('\n');
  const start = lines.findIndex(line => line === `component ${name}`);
  if (start < 0) throw new Error(`${file}: no component ${name}`);
  const end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
  return lines.slice(start, end < 0 ? undefined : end).join('\n');
}

type Call = { method: string; payload: Obj; fleet: string };
const provider = (instanceId: string, slug: string) => ({ instanceId, driver: instanceId, enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' }, models: [{ slug, name: slug.toUpperCase(), isDefault: true }] });
let serial = 0;
/** The focused environment "Laptop" (env, project p1 at /repo) and the native bridge every environment answers through. */
function harness() {
  const calls: Call[] = [];
  const subscriptions: Record<string, string> = {};
  const client = {
    environmentId: 'env', threadId: '', projectId: 'p1', ready: true, writable: true, revision: 0, generation: 3, busy: false,
    config: { environment: { label: 'Laptop', capabilities: { projectCloneTracking: true } }, providers: [provider('codex', 'gpt')], settings: {} },
    local: { drafts: {} as Record<string, string>, selections: {} as Record<string, { projectId: string; threadId: string }> },
    shell: { ...initialShell(), projects: [{ id: 'p1', title: 'fixture', workspaceRoot: '/repo' }], threads: [] },
    projectGroups() { return [{ key: 'g1', name: 'fixture', members: this.shell.projects }]; },
    restAccess: () => ({
      request: async (method: string, payload: Obj) => { calls.push({ method, payload, fleet: '' }); return method === 'vcs.listRefs' ? { refs: [{ name: 'main', isRemote: false }] } : {}; },
      read: async (method: string, payload: Obj) => { calls.push({ method, payload, fleet: '' }); return method === 'vcs.listRefs' ? { refs: [{ name: 'main', isRemote: false }] } : {}; },
      call: async (request: Obj) => { const id = `3-${++serial}`; subscriptions[String(request.method)] = id; return { id }; },
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `id-${index}`),
    }),
  };
  const native = {
    available: true, watch: () => {},
    later: async (input: unknown) => {
      const request = obj(input), key = String(request.fleet ?? '');
      calls.push({ method: String(request.method ?? request.op), payload: request, fleet: key });
      if (request.op === 'request' && request.method === 'vcs.listRefs') return { ok: true, generation: 5, value: { refs: [{ name: 'develop', isRemote: false }, { name: 'origin/develop', isRemote: true }] } };
      if (request.op === 'connect') return { ok: true, generation: 4, value: { state: 'connecting', origin: request.origin, environmentId: 'env2', message: '' } };
      return { ok: true, generation: 5, value: {} };
    },
  } as unknown as Native;
  return { client: client as unknown as T3Client & typeof client, native, calls, subscriptions };
}

/** A synchronized background environment "Studio" (env2, project q1 at /studio) in the window's fleet. */
async function studio(): Promise<FleetEntry> {
  const entry: FleetEntry = {
    key: 'http://127.0.0.1:16423\nenv2', origin: 'http://127.0.0.1:16423', environmentId: 'env2', phase: 'connected', message: '', traceId: '',
    generation: 5, synchronized: 5, lastEvent: 0, subscriptions: {}, scopes: [], error: '', requested: false,
    config: { environment: { label: 'Studio', capabilities: { projectCloneTracking: true } }, providers: [provider('claudeAgent', 'sonnet')], settings: {} },
    shell: { ...initialShell(), projects: [{ id: 'q1', title: 'studio-app', workspaceRoot: '/studio' }] },
  };
  fleet.entries.set(entry.key, entry);
  let id = 0;
  await liveFleetPass(async () => ({ id: `5-${++id}` }), entry);
  liveFleetEvent(entry, { key: SCHEDULED_TASKS_KEY, generation: 5, subscriptionId: '5-1', value: { tasks: [] } });
  return entry;
}
afterEach(() => { for (const key of [...fleet.entries.keys()]) if (key.endsWith('\nenv2')) fleet.entries.delete(key); });

describe('ST-1: Runs on switches a new task to another environment', () => {
  test('`new|<environment>` is a new task on that environment', () => {
    expect(editTarget('new|env2', 'env')).toEqual({ environmentId: 'env2', taskId: '', link: false });
    expect(editTarget('new|', 'env')).toEqual({ environmentId: 'env', taskId: '', link: false });
    expect(editTarget('env2|t9', 'env')).toEqual({ environmentId: 'env2', taskId: 't9', link: false });
  });
  test("the editor reads the chosen environment's projects, model and refs, keeps its key and is not a missing task", async () => {
    const { client, native, calls, subscriptions } = harness();
    await studio();
    await watchLive(client, native);
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: subscriptionsOf(subscriptions), value: { tasks: [] } });
    const here = await scheduledPage(client, native, 'env', '', 'task', '', true, NOW);
    expect(here.editors.map(editor => [editor.key, editor.environmentId, editor.projectId, editor.modelKey, editor.missing])).toEqual([['new', 'env', 'p1', 'codex:gpt', false]]);
    expect(here.environments).toEqual([{ value: 'env', label: 'Laptop', selected: true }, { value: 'env2', label: 'Studio', selected: false }]);
    const there = await scheduledPage(client, native, 'env', '', 'task', 'new|env2', true, NOW);
    expect([there.environment, there.available]).toEqual(['Studio', true]);
    expect(there.editors.map(editor => [editor.key, editor.environmentId, editor.projectId, editor.modelKey, editor.baseRef, editor.startFromOrigin, editor.path, editor.missing]))
      .toEqual([['new', 'env2', 'q1', 'claudeAgent:sonnet', 'main', true, '', false]]);
    expect(there.projects).toEqual([{ value: 'q1', label: 'studio-app', selected: false }]);
    expect(there.environments.map(option => [option.value, option.selected])).toEqual([['env', false], ['env2', true]]);
    expect(there.marks.map(mark => mark.value)).toContain('claudeAgent:sonnet');
    // The base branch picker lists Studio's refs, read over Studio's own transport, and looks the reset base "main" up by
    // name there (selectedRefQuery), since Studio's first page does not list it.
    expect(there.branches.map(group => [group.projectId, group.refs.map(ref => [ref.value, ref.badge])])).toEqual([['q1', [['develop', ''], ['origin/develop', 'remote']]]]);
    const reads = calls.filter(call => call.method === 'vcs.listRefs').map(call => [call.fleet, call.fleet ? obj(call.payload.payload) : call.payload]);
    expect(reads).toEqual([['', { cwd: '/repo', limit: 100 }], ['http://127.0.0.1:16423\nenv2', { cwd: '/studio', limit: 100 }], ['http://127.0.0.1:16423\nenv2', { cwd: '/studio', query: 'main', limit: 10 }]]);
  });
  test("Runs on lists the settings scope's connected environments only", async () => {
    const { client, native } = harness();
    await studio();
    const page = await scheduledPage(client, native, 'env', '', 'task', '', true, NOW, 'env');
    expect(page.environments.map(option => option.value)).toEqual(['env']);
  });
  test('the dialog resets project, base branch, origin and path on a change and reopens on the environment, keeping the rest', async () => {
    const editor = await component('settings-scheduled.contract', 'ScheduledEditor');
    expect(editor).toContain('    // The root\'s restOpen for the editor: `new|<environmentId>` is a new task on that environment (scheduled-view.ts editTarget).\n    runsOn: action');
    expect(editor).toContain('  derive chosenProject = projectId != "" ? projectId : draft.projectId');
    expect(editor).toContain([
      '    if field == "environment" and value != draft.environmentId',
      '      projectId = ""', '      baseRef = "main"', '      startFromOrigin = true', '      path = ""', '      refQuery = ""',
      '      runsOn(`new|${value}`, "")'].join('\n'));
    // Every reader of the project goes through the chosen one: the picker, its lookups, the Project select and Save.
    expect(editor).not.toMatch(/taskPicker\(draft\.key, projectId,|group\.projectId == projectId|projectId=\$\{encodeURIComponent\(projectId\)\}/);
    expect(editor).toContain('valueLabel=(chosenProject == "" ? "Select a project" : projectLabel(data.projects, chosenProject)), value=chosenProject');
    expect(editor).toContain('&projectId=${encodeURIComponent(chosenProject)}&');
    // Runs on is a new task's (disabled when editing), and its choices are the page's environments.
    expect(editor).toContain('disabled=(draft.id != "" or busy) id="task-environment"');
    expect(editor).toContain('choose=choose("environment", option.value))');
    expect(await source('settings-rest-dialogs.contract')).toContain('ScheduledEditor(data=scheduled, draft=draft, busy=busy, error=error, close=close, rest=rest, lookupBase=taskBase, runsOn=open("task"))');
    const dialogs = await component('app-settings.contract', 'ProjectDialogs');
    expect(dialogs).toContain('SettingsRestDialogs(editor=restEditor, subject=restSubject, label=restLabel, scheduled=scheduled, projects=projectsSettings, busy=commandPending, error=modalError, rest=rest, raw=restRaw, close=restClose, open=restOpen, taskBase=taskBase)');
    // A new task closing after a Runs on change gives the focus back to New task.
    expect(dialogs).toContain('target=(restEditor == "task" ? (restSubject == "" or startsWith(restSubject, "new|") ? "new-scheduled-task" : `task-actions-${restSubject}`)');
    expect(await source('app-window.contract')).toContain('restClose=restClose, restOpen=restOpen, sbPicker=sbPicker,');
  });
  test('the labels a Runs on change rewrites are plain texts the host repaints (X64 #327 and X82 #494, fixed on main; round 11)', async () => {
    // Round 10 keyed them by their words (a new node per label): a rastered label under a button kept its old words on
    // macOS. Main #327 (a shrunk raster) and #494 (the pump repaints an unselectable text) retire that (adopt-main-fixes-r11).
    const editor = await component('settings-scheduled.contract', 'ScheduledEditor');
    expect(editor).not.toContain('key=shown');
    expect(editor).toContain('                  text data.environment font-size="0.875rem"');
    expect(editor).toContain('                  text taskBaseLabel(baseRef, startFromOrigin, baseLocal) font-size="0.875rem"');
    const select = await component('settings-kit.contract', 'SettingsSelect');
    expect(select).not.toContain('key=shown');
    expect(select).toContain('        text valueLabel font-size="0.875rem"');
    // The triggers keep title="" (no AppKit help tag from aria-label while their label is the button's text).
    expect(editor).toMatch(/aria-label="Runs on"[^\n]* title=""/);
    expect(editor).toMatch(/aria-label="Base branch" role="combobox"[^\n]* title=""/);
    expect(select).toMatch(/aria-label=label disabled=disabled[^\n]* title=""/);
  });
});

describe("ST-2: Open project on another environment's clone toast", () => {
  const done = { projectId: 'q7', destinationPath: '/work/widgets', repository: null, phase: 'done', stage: 'checkout', percent: 100, detail: null, error: null };
  test('every success toast offers Open project, naming its environment', async () => {
    expect(cloneToast(done, 'env2').action).toEqual({ label: 'Open project', op: 'new-thread', id: 'fleet:env2:q7', value: '' });
    const { client, native } = harness();
    const entry = await studio();
    liveFleetEvent(entry, { key: PROJECT_CLONES_KEY, generation: 5, subscriptionId: '5-2', value: [done] });
    await cloneToasts(client, native);
    expect(toasts(client).find(toast => toast.title === 'Cloned widgets')).toMatchObject({ kind: 'success', timeoutMs: 8000, action: { label: 'Open project', op: 'new-thread', id: 'fleet:env2:q7' } });
  });
  test("Open project on a background environment moves the window there, on that project's draft", async () => {
    const { client, native, calls } = harness();
    const entry = await studio();
    const adopted: Obj[] = [];
    const self = Object.assign(client, { adoptStatus: (value: Obj, generation: number) => { adopted.push({ ...value, generation }); } });
    const out = {} as OpOut;
    expect(await threadOps.call(self, 'new-thread', 'fleet:env2:q7', '', 0, native, {} as Files, out)).toBe(true);
    expect(client.local.selections.env2).toEqual({ projectId: 'q7', threadId: '' });
    expect(calls.filter(call => call.payload.op === 'fleetStop' || call.payload.op === 'connect').map(call => [call.payload.op, call.fleet || String(call.payload.origin)]))
      .toEqual([['fleetStop', entry.key], ['connect', 'http://127.0.0.1:16423']]);
    expect(adopted).toEqual([{ state: 'connecting', origin: 'http://127.0.0.1:16423', environmentId: 'env2', message: '', generation: 4 }]);
    expect(fleet.entries.has(entry.key)).toBe(false);
    await expect(focusFleetProject(client, native, 'gone', 'q7')).rejects.toThrow('no longer connected');
  });
  test("Open project on the focused environment opens that project's draft here", async () => {
    const { client, native, calls } = harness();
    client.shell.projects.push({ id: 'p2', title: 'widgets', workspaceRoot: '/work/widgets' });
    client.threadId = 't1';
    const self = Object.assign(client, { connection: 'disconnected', chooseDefaults: () => {}, subscriptions: {} as Record<string, string> });
    expect(await threadOps.call(self, 'new-thread', 'fleet:env:p2', '', 0, native, {} as Files, {} as OpOut)).toBe(true);
    expect([client.projectId, client.threadId]).toEqual(['p2', '']);
    expect(calls.some(call => call.payload.op === 'connect')).toBe(false);
  });
});

describe('ST-3: the Automations icon buttons show the reference tooltips', () => {
  test('each button hands its frame to the window layer as a 600 ms top tooltip on hover and keyboard focus', async () => {
    const button = await component('thread-automations.contract', 'AutomationButton');
    expect(button).toContain('  inject\n    rem: number\n    hoverTipAt: action');
    expect(button).toContain('hoverTipAt(hoverTipAtFrame(testKey, "tooltip", tip, [], "top", align, frame(testKey), (floor(length(tip) / 48) + 1) * rem + 0.625 * rem, `${testKey}-tooltip`), "trigger", on)');
    expect(button).toContain('action hover(over: bool)\n    hovered = over\n    show(over)');
    expect(button).toContain('button id=testKey press=pressed hover=hover focus=show(true) blur=show(false) disabled=disabled aria-label=label testId=testKey');
    // No AppKit help tag beside the drawn tooltip.
    expect(button).toContain('title=""');
    const layer = await source('hover-layer.contract');
    expect(layer).toContain('(kind == "tooltip" or kind == "code" ? 600 : 0)');
    expect(layer).toContain('derive card = tip.kind != "tip" and tip.kind != "policy" and tip.kind != "tooltip"');
  });
  test('a press closes the tooltip before it acts, as TooltipTrigger closeOnClick (Base UI 1.5.0 default true)', async () => {
    const button = await component('thread-automations.contract', 'AutomationButton');
    // show(false) first: the press may open Settings, which the hover layer draws above.
    expect(button).toContain('action pressed\n    show(false)\n    press()');
    expect(button).not.toContain('press=press ');
    // The window's close: the trigger's leave ends a shown tooltip at once (a TooltipPopup has no close delay) and stops
    // one still waiting for its 600 ms (hoverUntil at the current tick, hoverShown false).
    const window = await source('app-window.contract');
    expect(window).toContain('else if tip.key == hoverTip.key and hoverOn == part\n      let until = hoverShown ? hoverWait + hoverDelay(tip.kind) : hoverWait');
    expect(window).toContain('derive hoverShown = hoverTip.key != "" and (hoverOn != "" ? hoverWait == hoverOpen : hoverWait < hoverUntil)');
    expect(await source('hover-layer.contract')).toContain('fn hoverDelay(kind: string): number = kind == "scopes" ? 100 : (kind == "freshness" or kind == "pr-preview" ? 120 : (kind == "usage" ? 50 : 0))');
  });
  test('the words are the reference TooltipPopups: Manage scheduled tasks, Edit automation, Run now', async () => {
    const section = await component('thread-automations.contract', 'ThreadAutomationsSection');
    const tips = [...section.matchAll(/AutomationButton\(icon="([\w-]+)", label=[^,]+, tip="([^"]+)", align="(\w+)"/g)].map(match => [match[1], match[2], match[3]]);
    // Manage sits at the panel's right edge: its tooltip ends at the button's right edge, where Base UI's shift keeps it
    // inside the window (the layer has no shift, X17 #112); the row buttons' tooltips fit centred.
    expect(tips).toEqual([['settings-2', 'Manage scheduled tasks', 'end'], ['pencil', 'Edit automation', 'center'], ['play', 'Run now', 'center']]);
    // The layer's end placement: a 22.5rem row (a "code" tip's 32.5rem) ending at the trigger's right edge, its bubble at the row's end.
    const layer = await component('hover-layer.contract', 'HoverLayer');
    expect(layer).toContain('derive span = tip.kind == "code" ? 32.5 : 22.5');
    expect(layer).toContain('derive left = tip.align == "start" ? tip.x : (tip.align == "end" ? tip.x + tip.width - span * rem : tip.x + tip.width / 2 - span / 2 * rem)');
    expect(layer).toContain('derive justify = tip.align == "start" ? "flex-start" : (tip.align == "end" ? "flex-end" : "center")');
  });
});

function subscriptionsOf(subscriptions: Record<string, string>): string { return subscriptions['scheduledTasks.subscribe']!; }
