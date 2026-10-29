// The previewAutomation host's executor (browser-automation-run.ts) over a fake `@exact/browser-automation`: what it
// asks the page and the frame for each operation, and what it answers. The clone's own rows (the reference runs these
// over CDP and has no host tests for them), with four of previewAutomationRequestConsumer.test.ts's serialization rows:
// a viewport that never renders, a target that is not editable, an operation failure whose cause stays out of the
// answer, and an error a plan carried.
import { describe, expect, it, spyOn } from 'bun:test';
import { adoptFramePrompt, answerFramePrompt, automationAudio, automationControllerOf, keyActions, runAutomation, toggleFrameMute, useAutomationCalls, type AutomationCalls, type RunDeps } from '../../src/browser/browser-automation-run';
import { adoptFrameEvent, frameByElement, frameCommandView, frameIdOf, syncFrames } from '../../src/browser/browser-frames';
import { browserHost } from '../../src/browser/browser-surface';
import { previewRuntimeTabId, type PreviewSessionSnapshot } from '../../src/browser/browser-state';
import { adoptCaptureNative, heldFrame, threadTurnRunning } from '../../src/browser/browser-capture';
import { AGENT_FOCUS_IDLE_MS, agentHoldWaiting, settleAgentHolds } from '../../src/browser/browser-automation-run';
import { noteNow } from '../../src/composer/composer-controls';
import { obj, type Obj } from '../../src/client/domain';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';

const ref = { environmentId: 'env-1', threadId: 'thread-1' };
const RUNTIME = previewRuntimeTabId(ref, 'epoch-1', 'tab-1'), FRAME = frameIdOf(RUNTIME);
const loaded: PreviewSessionSnapshot = { threadId: 'thread-1', tabId: 'tab-1', navStatus: { _tag: 'Success', url: 'http://127.0.0.1:16751/', title: 'Fixture' }, canGoBack: false, canGoForward: false, updatedAt: '2026-10-25T00:00:00.000Z' };
const record = (event: string, fields: Record<string, string> = {}) =>
  new URLSearchParams({ event, id: '1', type: 'push', loading: 'false', back: 'false', forward: 'false', controller: 'person', error: '', recovering: 'false', url: 'http://127.0.0.1:16751/', title: 'Fixture', ...fields }).toString();

type Call = { name: string; options: Obj };
/** A page: each call is logged and answered by `answer` (a script's value as the executor's wrapper returns it). */
function fakePage(answer: (name: string, options: Obj) => unknown = () => null) {
  const log: Call[] = [];
  const call = (name: string) => async (options: Obj) => {
    log.push({ name, options });
    const value = answer(name, options);
    if (value instanceof Error) throw value;
    return value ?? (name === 'evaluate' || name === 'callFunction' ? { result: null } : {});
  };
  const calls = { evaluate: call('evaluate'), callFunction: call('callFunction'), addPreloadScript: call('addPreloadScript'), performActions: call('performActions'),
    handleUserPrompt: call('handleUserPrompt'), setDownloadBehavior: call('setDownloadBehavior'), captureScreenshot: call('captureScreenshot') } as unknown as AutomationCalls;
  return { calls, log };
}
/** A script's answer through the executor's wrapper (`{ text }`, its value as JSON). */
const scripted = (value: unknown) => ({ result: { text: JSON.stringify(value) } });
const failure = (code: string) => Object.assign(new Error(code), { code });

function setup() {
  const client = { environmentId: 'env-1', threadId: 'thread-1', projectId: 'p1', generation: 3, connection: 'connected', ready: true, draftKey: 'env-1:thread-1',
    presentation: {}, revision: 0, local: {}, shell: { threads: [], projects: [] }, scopes: ['preview:operate'] } as unknown as T3Client & { revision: number };
  browserHost(client).store.reconcileServerSessions(ref, { sessions: [loaded], serverEpoch: 'epoch-1', revision: 1 });
  syncFrames(client, [{ runtimeId: RUNTIME, url: 'http://127.0.0.1:16751/', environment: 'env-1', profile: 'default', zoom: 1, colorScheme: 'system' }]);
  adoptFrameEvent(client, FRAME, record('navigatesuccess'), 0);
  const native = { available: true, watch() {}, later: async (request: Obj) => { natives.push(request); return { ok: true, generation: 3, value: {} }; } } as unknown as Native;
  const natives: Obj[] = [];
  adoptCaptureNative(client, native);
  const responded: Obj[] = [], rpcs: Array<{ method: string; payload: Obj }> = [];
  const deps: RunDeps = { native, rpc: async (method, payload) => { if (method === 'previewAutomation.respond') responded.push(payload); else rpcs.push({ method, payload }); return {}; }, adopt: () => {} };
  return { client, deps, responded, rpcs, natives };
}
const plan = (operation: string, input: Obj = {}, extra: Obj = {}): Obj => ({
  requestId: `r-${operation}`, connectionId: 'c-1', clientId: 'preview-1', operation, input, environmentId: 'env-1', threadId: 'thread-1', timeoutMs: 15_000,
  generation: 3, budgetMs: 13_500, tabId: 'tab-1', runtimeId: RUNTIME, viewportSetting: { _tag: 'fill' }, ...extra,
});
/** `onSleep`: the page moves on at each of the run's sleeps (command-waits.ts `sleep`, a timer of the run's answer). */
async function run(operation: string, input: Obj, page: ReturnType<typeof fakePage>, extra: Obj = {}, onSleep?: (client: T3Client) => void) {
  const fixture = setup();
  useAutomationCalls(page.calls);
  const timer = globalThis.setTimeout;
  const sleeps = onSleep ? spyOn(globalThis, 'setTimeout').mockImplementation(((fn: () => void, ms?: number) => { onSleep(fixture.client); return timer(fn, ms); }) as unknown as typeof setTimeout) : null;
  try { await runAutomation(fixture.client, plan(operation, input, extra), fixture.deps); }
  finally { useAutomationCalls(null); sleeps?.mockRestore(); }
  return { ...fixture, answer: fixture.responded[0] ?? {} };
}

describe('the executor', () => {
  it('status reads the tab’s frame: its page, not shown in a panel', async () => {
    const { answer } = await run('status', {}, fakePage());
    expect(answer).toEqual({ clientId: 'preview-1', connectionId: 'c-1', requestId: 'r-status', ok: true,
      result: { tabId: 'tab-1', viewportSetting: { _tag: 'fill' }, available: true, visible: false, url: 'http://127.0.0.1:16751/', title: 'Fixture', loading: false } });
  });

  it('navigate asks the frame to load and answers once its page has loaded', async () => {
    let step = 0;
    const { answer, client } = await run('navigate', {}, fakePage(), { navigate: { url: 'http://127.0.0.1:16751/b' } }, current => {
      // The root runs the command: the frame reports its navigation, then the load.
      if (step === 0 && frameCommandView(current).kind === 'navigate' && frameCommandView(current).url === 'http://127.0.0.1:16751/b') {
        adoptFrameEvent(current, FRAME, record('navigate', { id: '2', loading: 'true', destination: 'http://127.0.0.1:16751/b' }), 0);
        step = 1;
      } else if (step === 1) { adoptFrameEvent(current, FRAME, record('navigatesuccess', { id: '2', url: 'http://127.0.0.1:16751/b', title: 'B' }), 0); step = 2; }
    });
    expect(answer).toMatchObject({ ok: true, result: { url: 'http://127.0.0.1:16751/b', title: 'B', loading: false } });
    expect(client.revision).toBeGreaterThan(0);
  });

  it('click: the locator’s point in the sandbox, the agent’s cursor, then pointer input at it', async () => {
    const page = fakePage((name, options) => {
      if (name === 'callFunction' && String(options.functionDeclaration).includes('__t3PlaywrightInjected)')) return { result: true };
      if (name === 'callFunction' && String(options.functionDeclaration).includes('scrollIntoView')) return scripted({ x: 40, y: 20 });
      if (name === 'evaluate' && String(options.expression).includes('innerWidth')) return { result: { width: 400, height: 300 } };
      return null;
    });
    const { answer, natives } = await run('click', { locator: 'role=button' }, page);
    expect(answer).toMatchObject({ ok: true, result: {} });
    expect(page.log.find(call => call.name === 'setDownloadBehavior')?.options).toEqual({ id: FRAME, destinationFolder: 'app:/tmp/browser-downloads' });
    expect(page.log.find(call => call.name === 'performActions')?.options).toEqual({ id: FRAME, actions: [{ type: 'pointer', id: 't3-agent-mouse', parameters: { pointerType: 'mouse' },
      actions: [{ type: 'pointerMove', x: 40, y: 20, origin: 'viewport' }, { type: 'pointerDown', button: 0 }, { type: 'pointerUp', button: 0 }] }] });
    expect(natives.filter(request => request.op === 'browserPage').map(request => [request.frame, request.phase, request.x, request.y])).toEqual([[FRAME, 'move', 40, 20], [FRAME, 'click', 40, 20]]);
    expect(page.log.filter(call => call.options.sandbox === 't3-preview-automation').length).toBeGreaterThan(0);
  });

  it('a point outside the viewport fails the operation, its cause kept out of the answer', async () => {
    const page = fakePage((name, options) => name === 'evaluate' && String(options.expression).includes('innerWidth') ? { result: { width: 100, height: 100 } } : null);
    const { answer } = await run('click', { x: 500, y: 5 }, page);
    expect(answer.ok).toBe(false);
    expect(answer.error).toEqual({ _tag: 'PreviewAutomationExecutionError', message: 'Preview automation click request r-click failed on environment env-1 thread thread-1 (tab tab-1).',
      detail: { requestId: 'r-click', environmentId: 'env-1', threadId: 'thread-1', operation: 'click', tabId: 'tab-1' } });
    expect(page.log.some(call => call.name === 'performActions')).toBe(false);
  });

  it('type into a target that is not editable answers PreviewAutomationTargetNotEditableError with the selector’s kind', async () => {
    const page = fakePage((name, options) => name === 'callFunction' && String(options.functionDeclaration).includes('notEditable') ? scripted({ notEditable: true }) : name === 'callFunction' ? { result: true } : null);
    const { answer } = await run('type', { selector: '#name', text: 'hi' }, page);
    expect(answer.error).toEqual({ _tag: 'PreviewAutomationTargetNotEditableError', message: 'Preview automation type request r-type requires an editable target in tab tab-1.',
      detail: { requestId: 'r-type', environmentId: 'env-1', threadId: 'thread-1', operation: 'type', tabId: 'tab-1', selectorKind: 'selector', selectorLength: 5 } });
  });

  it('evaluate runs in the page’s world; a script that is no expression runs as one; undefined answers no result', async () => {
    const page = fakePage((name, options) => {
      if (name !== 'callFunction') return null;
      if (String(options.functionDeclaration).startsWith('async')) return failure('SyntaxError');
      return { result: '{"a":1}' };
    });
    const { answer } = await run('evaluate', { expression: 'const a = 1; ({ a })' }, page);
    expect(answer).toMatchObject({ ok: true, result: { a: 1 } });
    expect(page.log.filter(call => call.name === 'callFunction').every(call => call.options.sandbox === undefined)).toBe(true);
    const nothing = await run('evaluate', { expression: 'undefined' }, fakePage(name => name === 'callFunction' ? { result: '__t3_preview_undefined__' } : null));
    expect(nothing.answer).toEqual({ clientId: 'preview-1', connectionId: 'c-1', requestId: 'r-evaluate', ok: true });
  });

  it('press sends the key natively, a paste included; an editing shortcut edits in the page', async () => {
    expect(keyActions({ key: 'Enter' })).toEqual([{ type: 'key', id: 't3-agent-keyboard', actions: [{ type: 'keyDown', value: '' }, { type: 'keyUp', value: '' }] }]);
    expect(keyActions({ key: 'v', modifiers: ['Meta'] })).toEqual([{ type: 'key', id: 't3-agent-keyboard', actions: [
      { type: 'keyDown', value: '' }, { type: 'keyDown', value: 'v' }, { type: 'keyUp', value: 'v' }, { type: 'keyUp', value: '' }] }]);
    expect(keyActions({ key: 'a', modifiers: ['Shift'] })?.[0]).toMatchObject({ actions: [{ value: '' }, { value: 'A' }, { value: 'A' }, { value: '' }] });
    expect(keyActions({ key: 'CapsLock' })).toBeNull();
    const page = fakePage();
    await run('press', { key: 'a', modifiers: ['Meta'] }, page, { key: { chord: 'Meta+a', text: '', commands: ['selectAll'], editing: '(() => 1)()', dom: '' } });
    expect(page.log.map(call => call.name)).toContain('evaluate');
    expect(page.log.some(call => call.name === 'performActions')).toBe(false);
  });

  it('a frame that never mounts answers the overlay timeout; no tab answers the unavailable target', async () => {
    const page = fakePage(name => name === 'evaluate' ? failure('NotFoundError') : null);
    const { answer } = await run('scroll', {}, page, { budgetMs: 200 });
    expect(answer.error).toMatchObject({ _tag: 'PreviewAutomationTimeoutError', detail: { timeoutMs: 200 } });
    const missing = await run('scroll', {}, fakePage(), { tabId: null, runtimeId: null });
    expect(obj(missing.answer.error)._tag).toBe('PreviewAutomationTabNotFoundError');
  });

  it('a viewport that never renders rolls back to the tab’s size and answers the viewport timeout', async () => {
    const page = fakePage((name, options) => name === 'evaluate' && String(options.expression).includes('innerWidth') ? { result: { width: 10, height: 10 } } : null);
    const shown = setup(), rpcs: string[] = [];
    // The tab shows in the panel, so its frame is laid out and measured.
    const { surfaceStore } = await import('../../src/panel/panel-surfaces');
    surfaceStore(shown.client).panels.set('env-1:thread-1', { surfaces: [{ id: 'browser:tab-1', kind: 'browser', path: '', line: 0, reveal: 0, browser: { tabId: 'tab-1', threadKey: 'env-1:thread-1' } }], active: 'browser:tab-1', visible: true, userRevision: 0 } as never);
    shown.deps.rpc = async (method, payload) => { if (method === 'previewAutomation.respond') shown.responded.push(payload); else rpcs.push(`${method} ${JSON.stringify(payload.viewport)}`); return {}; };
    useAutomationCalls(page.calls);
    try { await runAutomation(shown.client, plan('resize', { timeoutMs: 300 }, { viewport: { _tag: 'freeform', width: 800, height: 600 } }), shown.deps); }
    finally { useAutomationCalls(null); }
    expect(rpcs).toEqual(['preview.resize {"_tag":"freeform","width":800,"height":600}', 'preview.resize {"_tag":"fill"}']);
    expect(shown.responded[0]?.error).toEqual({ _tag: 'PreviewAutomationTimeoutError', message: 'Preview viewport for request r-resize on environment env-1 thread thread-1 tab tab-1 was not rendered within 300ms.',
      detail: { requestId: 'r-resize', environmentId: 'env-1', threadId: 'thread-1', tabId: 'tab-1', timeoutMs: 300 } });
  });

  it('an error the plan carried is answered as it is', async () => {
    const error = { _tag: 'PreviewAutomationTabNotFoundError', message: 'planned', detail: {} };
    const { answer } = await run('snapshot', {}, fakePage(), { failure: error });
    expect(answer).toEqual({ clientId: 'preview-1', connectionId: 'c-1', requestId: 'r-snapshot', ok: false, error });
  });

  it('snapshot reads a large page in slices and its screenshot through captureScreenshot', async () => {
    const text = 'x'.repeat(20_000);
    const page = fakePage((name, options) => {
      if (name === 'callFunction' && String(options.functionDeclaration).includes('__t3PlaywrightInjected)')) return { result: true };
      if (name === 'callFunction') return { result: { slices: 3 } };
      if (name === 'evaluate' && String(options.expression).startsWith('globalThis.__t3Out.slice')) {
        const [from, to] = String(options.expression).match(/\d+/g)!.map(Number);
        return { result: JSON.stringify({ url: 'http://127.0.0.1:16751/', title: 'Fixture', loading: false, visibleText: text, interactiveElements: [] }).slice(from, to) };
      }
      if (name === 'evaluate') return { result: [{ kind: 'console', entry: { level: 'log', text: 'hello' } }] };
      if (name === 'captureScreenshot') return failure('UnknownError');
      return null;
    });
    const { answer } = await run('snapshot', {}, page);
    // No app files in a test: the screenshot fails the operation after the page was read in its three slices.
    expect(page.log.filter(call => call.name === 'evaluate' && String(call.options.expression).startsWith('globalThis.__t3Out')).length).toBe(3);
    expect(answer.ok).toBe(false);
  });

  it('answers a dialog at once while an agent request runs on the tab, and lists it for the agent', async () => {
    const page = fakePage();
    let asked = false;
    const { client } = await run('navigate', { timeoutMs: 200 }, page, { navigate: { url: 'http://127.0.0.1:16751/b' } }, current => {
      if (asked) return;
      asked = true;
      adoptFramePrompt(current, FRAME, 'type=alert&message=Hi&default=');
      adoptFramePrompt(current, FRAME, 'type=confirm&message=Sure%3F&default=');
    });
    expect(page.log.filter(call => call.name === 'handleUserPrompt').map(call => call.options)).toEqual([{ id: FRAME, accept: true }, { id: FRAME, accept: false }]);
    expect(frameByElement(client, FRAME)?.prompt).toBeNull();
  });

  it('holds a dialog for the person when no agent request runs, and answers it with their choice', async () => {
    const { client } = setup(), page = fakePage();
    useAutomationCalls(page.calls);
    try {
      adoptFramePrompt(client, FRAME, 'type=prompt&message=Name%3F&default=Ada');
      expect(page.log).toEqual([]);
      expect(frameByElement(client, FRAME)?.prompt).toEqual({ type: 'prompt', title: '127.0.0.1:16751 says', message: 'Name?', defaultValue: 'Ada' });
      await answerFramePrompt(client, FRAME, 'accept=true&text=Grace');
      expect(page.log.map(call => call.options)).toEqual([{ id: FRAME, accept: true, userText: 'Grace' }]);
      expect(frameByElement(client, FRAME)?.prompt).toBeNull();
      adoptFramePrompt(client, FRAME, 'type=beforeunload&message=&default=');
      expect(frameByElement(client, FRAME)?.prompt).toEqual({ type: 'beforeunload', title: 'Leave site?', message: 'Changes that you made may not be saved.', defaultValue: '' });
      await answerFramePrompt(client, FRAME, 'accept=false&text=');
      expect(page.log.at(-1)?.options).toEqual({ id: FRAME, accept: false });
    } finally { useAutomationCalls(null); }
  });

  it('Mute sets the page’s media and the tab’s audio state; the controller follows the frame', async () => {
    const { client, deps } = setup(), page = fakePage();
    useAutomationCalls(page.calls);
    try { await toggleFrameMute(client, deps.native, RUNTIME, true); } finally { useAutomationCalls(null); }
    expect(page.log[0]?.options).toEqual({ id: FRAME, expression: 'globalThis.__t3PreviewMedia ? globalThis.__t3PreviewMedia.setMuted(true) : null', sandbox: 't3-preview-automation' });
    expect(automationAudio(client, RUNTIME)).toEqual({ audioMuted: true, audible: false });
    expect(automationControllerOf(client, RUNTIME)).toBe('none');
    adoptFrameEvent(client, FRAME, record('currententrychange', { controller: 'agent' }), 0);
    expect(automationControllerOf(client, RUNTIME)).toBe('agent');
  });
});

// DesktopBrowserHost.ts at 7c3b3b3605ef: a hidden tab throttles unless the agent drives it in its thread's turn.
describe('a hidden tab the agent drives', () => {
  // DesktopBrowserHost.ts at 7c3b3b3605ef: the turn's end (agentTurnsSettled: the root turn and its subagents) and
  // FOCUS_IDLE after it, or after the last request, let the tab throttle.
  it('keeps drawing through its thread\'s turn, waiting included, and lets go FOCUS_IDLE after the turn ends', async () => {
    const { client } = await run('status', {}, fakePage());
    const thread: Obj = { id: 'thread-1', latestRunId: 'run-1', status: 'running' };
    (client.shell as { threads: Obj[] }).threads = [thread];
    noteNow(client, 60_000); // long past the request's own idle wait
    expect(heldFrame(client, RUNTIME)).not.toBeNull();
    // Blocked on another thread (t3_thread_wait), then parked on its subagents: the turn still runs, each for longer
    // than FOCUS_IDLE, which a turn's end seen at its start would have run out.
    thread.blockedOnThreadWait = true;
    noteNow(client, 70_000);
    expect(heldFrame(client, RUNTIME)).not.toBeNull();
    noteNow(client, 70_000 + AGENT_FOCUS_IDLE_MS + 1_000);
    expect(heldFrame(client, RUNTIME)).not.toBeNull();
    delete thread.blockedOnThreadWait;
    thread.status = 'completed'; thread.pendingBackgroundTasks = [{ kind: 'subagent' }];
    noteNow(client, 80_000);
    expect(heldFrame(client, RUNTIME)).not.toBeNull();
    noteNow(client, 80_000 + AGENT_FOCUS_IDLE_MS + 1_000);
    expect(heldFrame(client, RUNTIME)).not.toBeNull();
    // The turn ends; a dev server it left running is no turn. The tab stays held FOCUS_IDLE longer.
    thread.pendingBackgroundTasks = [{ kind: 'command' }];
    noteNow(client, 90_000);
    expect(heldFrame(client, RUNTIME)).not.toBeNull();
    noteNow(client, 90_000 + AGENT_FOCUS_IDLE_MS - 1);
    expect(heldFrame(client, RUNTIME)).not.toBeNull();
    noteNow(client, 90_000 + AGENT_FOCUS_IDLE_MS);
    expect(heldFrame(client, RUNTIME)).toBeNull();
    // The next turn has not driven the tab yet.
    thread.status = 'running'; thread.latestRunId = 'run-2'; thread.pendingBackgroundTasks = [];
    expect(heldFrame(client, RUNTIME)).toBeNull();
  });
  // backgroundWorkHoldsCompletion parks a run on a monitor too, but agentTurnsSettled waits only for child turns.
  it('lets go FOCUS_IDLE after the turn when the run is parked only on a monitor or a background task', async () => {
    for (const kind of ['monitor', 'background_task']) {
      const { client } = await run('status', {}, fakePage());
      (client.shell as { threads: Obj[] }).threads = [{ id: 'thread-1', latestRunId: 'run-1', status: 'completed', pendingBackgroundTasks: [{ kind }] }];
      noteNow(client, 60_000);
      expect(heldFrame(client, RUNTIME)).not.toBeNull();
      noteNow(client, 60_000 + AGENT_FOCUS_IDLE_MS);
      expect(heldFrame(client, RUNTIME)).toBeNull();
    }
  });
  it('holds FOCUS_IDLE after the last request of a turn that already ended', async () => {
    const { client } = await run('status', {}, fakePage());
    (client.shell as { threads: Obj[] }).threads = [{ id: 'thread-1', latestRunId: 'run-1', status: 'completed' }];
    expect(heldFrame(client, RUNTIME)).not.toBeNull();
    noteNow(client, AGENT_FOCUS_IDLE_MS);
    expect(heldFrame(client, RUNTIME)).toBeNull();
    expect(agentHoldWaiting(client, AGENT_FOCUS_IDLE_MS)).toBe(false);
  });
  it('runs the clock only for a request or an idle wait, and forgets a tab whose thread or frame goes', async () => {
    const { client } = await run('status', {}, fakePage());
    const shell = client.shell as { threads: Obj[] };
    shell.threads = [{ id: 'thread-1', latestRunId: 'run-1', status: 'running' }];
    const turn = (environmentId: string, threadId: string) => threadTurnRunning(client, environmentId, threadId);
    // A shown tab in a running turn needs no clock: the turn's end comes as a shell change.
    settleAgentHolds(client, 60_000, () => true, turn);
    expect(agentHoldWaiting(client, 60_000)).toBe(false);
    // Its thread goes: the hold ends after FOCUS_IDLE, and then the clock may stop.
    shell.threads = [];
    settleAgentHolds(client, 61_000, () => true, turn);
    expect(agentHoldWaiting(client, 61_000)).toBe(true);
    expect(agentHoldWaiting(client, 61_000 + AGENT_FOCUS_IDLE_MS)).toBe(false);
    // Its frame goes (the tab closed, or its environment went): the tab is forgotten.
    expect(automationAudio(client, RUNTIME)).not.toBeNull();
    settleAgentHolds(client, 70_000, () => false, turn);
    expect(automationAudio(client, RUNTIME)).toBeNull();
  });
  it('is not held for a tab the agent never drove', () => {
    const { client } = setup();
    (client.shell as { threads: Obj[] }).threads = [{ id: 'thread-1', latestRunId: 'run-1', status: 'running' }];
    expect(heldFrame(client, RUNTIME)).toBeNull();
  });
});
