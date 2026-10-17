// Where the browser import wizard and the profile removal confirm put the focus, as the
// reference's Base UI dialogs do (T3 Code 1e2ecbd975 BrowserImportWizard.tsx, IntegrationsSettings.tsx; recorded over CDP
// on the reference with fixture browsers). As the wizard opens, the popup's first Tab stop takes the focus
// (Configure: the first "From" tile; the quit step: Cancel; Full Disk Access: Allow; a blocked source: Close). After a step
// change, Base UI moves the focus to the popup itself only when the element that held it was removed (restoreFocus
// "popup"): a step's buttons go with their step, Allow with a grant, and the close X stays between two steps that can close.
// From the popup Tab goes to the first stop and Shift+Tab to the last (the X). The confirm focuses Cancel, and Cancel gives
// it back to the row's options button (after a removal, Add profile).
//
// The wizard is driven here as the page drives it, through `browserProfilesLocal` and read back through
// `browserProfilesView`, over a fake module whose browsers can be running or Full Disk Access denied: each row checks the
// view's focus facts (openFocus, the Tab stops, when it opened and changed, since when each element is mounted) after
// opening, I've quit it, Import, Try again and a grant. `keptFrom` is app.contract's rule over those facts (the root's
// `importWizardStep`), whose source the last rows check, with the ids and focus wiring of browser-profiles.contract.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { browserProfilesLocal, browserProfilesView, emptyBrowserProfilesView, goTo, setGranted, wizardElements, wizardMounts, wizardOpenFocus, wizardTabStops,
  type BrowserImportWizardView } from '../../src/browser/browser-profiles-settings';
import type { WizardStep } from '../../src/browser/browser-import';
import type { T3Client } from '../../src/client/client';
import type { Files, Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';

// ── The fake module: Chrome (Local State names "You"), running while `world.running` (its SingletonLock names another
// host), and Safari, whose cookie jar refuses to open while `world.denied` (TCC's EPERM). ──────────────────────────────
type Op = { op: string } & Obj;
const localState = btoa(JSON.stringify({ profile: { info_cache: { Default: { name: 'You' } } } }));
const world = { running: false, denied: true };
function ioAnswer(request: Op): Obj {
  const path = String(request.path ?? '');
  switch (request.call) {
    case 'stat': return { kind: 'File' };
    case 'readDirectory': return { entries: [] };
    case 'readFile': return { id: 'staged', size: atob(localState).length };
    case 'readChunk': return { data: localState };
    case 'release': return {};
    case 'query': return { rows: [] };
    case 'readLink': return world.running && path.endsWith('SingletonLock') ? { target: 'elsewhere-4242' } : { code: 'ENOENT', message: 'missing' };
    case 'hostname': return { hostname: 'here' };
    case 'localAddresses': return { addresses: [] };
    case 'lockHeld': return { held: false };
    case 'open': return world.denied && path.endsWith('.binarycookies') ? { code: 'EPERM', message: 'Operation not permitted' } : {};
    default: return { code: 'ENOENT', message: 'missing' };
  }
}
function fakeClient(): T3Client {
  return {
    environmentId: 'local', threadId: 'thread-1', projectId: 'p1', generation: 1, connection: 'connected', ready: true, revision: 0, presentation: {} as Obj, config: { keybindings: [] } as Obj,
    shell: { projects: [], threads: [], sequence: 0 }, local: { clientSettings: {}, deviceSettings: {} }, preferencesLoaded: true, savePreferences: async () => undefined,
    async raw(_native: Native, request: Op) {
      const value = request.op === 'browserImportContext' ? { allowed: true, home: '/fixture-home', platform: 'darwin' } : request.op === 'browserImportIO' ? ioAnswer(request) : {};
      return { ok: true, generation: 0, value };
    },
  } as unknown as T3Client;
}
const module = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: { saved: [] } }) } as unknown as Native;
const storage = {} as Files;

let client: T3Client;
const act = (op: string, value = '') => browserProfilesLocal(client, module, storage, op, value);
const wizard = async () => (await browserProfilesView(client, module)).wizard;
/** app.contract importWizardStep: which elements still hold a focus they held when the root last saw the wizard (its
 *  `focusAt` then): the popup, and every element mounted since. */
const keptFrom = (view: BrowserImportWizardView, seen: number) => ['browser-import-popup', ...view.mounted.filter(element => element.since <= seen).map(element => element.id)];
async function open(browser: string): Promise<BrowserImportWizardView> {
  await act('browser-profiles-sources');
  await act('browser-import-open', `source=${browser}`);
  return wizard();
}

beforeEach(() => { client = fakeClient(); world.running = false; world.denied = true; primaryAt('http://127.0.0.1:16101', 'env-local'); });
afterEach(() => resetPrimary());

describe('the wizard driven through its ops: the focus facts the root reads', () => {
  test('Configure opens on the first "From" tile; from the popup Tab goes to it and Shift+Tab to the X', async () => {
    const view = await open('chrome');
    expect(view).toMatchObject({ open: true, step: 'configure', openFocus: 'browser-import-from-0', tabFirst: 'browser-import-from-0', tabLast: 'browser-import-x' });
    expect(view.focusAt).toBe(view.openedAt);
    expect(view.mounted.map(element => element.id)).toEqual(['browser-import-from-0', 'browser-import-into-0', 'browser-import-into-1', 'browser-import-cancel', 'browser-import-run', 'browser-import-x']);
    expect(view.mounted.every(element => element.since === view.openedAt)).toBe(true);
    // A tile chosen is no change of screen: nothing moves the focus.
    await act('browser-import-into', 'target=new');
    expect((await wizard()).focusAt).toBe(view.focusAt);
  });

  test('I’ve quit it: Quit → Checking → Quit removes Cancel and I’ve quit it, and keeps the X (scenario (a))', async () => {
    world.running = true;
    const opened = await open('chrome');
    expect(opened).toMatchObject({ step: 'quit', openFocus: 'browser-import-cancel', tabFirst: 'browser-import-cancel', tabLast: 'browser-import-x' });
    await act('browser-import-quit');
    const again = await wizard();
    expect(again.step).toBe('quit');
    expect(again.focusAt).toBe(opened.focusAt + 2); // Checking, then Quit: both changes, though the page read neither
    expect(again.openedAt).toBe(opened.openedAt);
    // From what the root last saw (the opening), the X held the focus throughout; Cancel and I’ve quit it are new.
    expect(keptFrom(again, opened.focusAt)).toEqual(['browser-import-popup', 'browser-import-x']);
    // The X focused during Checking (the root saw Checking): still mounted when Quit comes back.
    expect(again.mounted.find(element => element.id === 'browser-import-x')?.since).toBe(opened.openedAt);
    world.running = false;
    await act('browser-import-quit');
    const configure = await wizard();
    expect(configure).toMatchObject({ step: 'configure', tabFirst: 'browser-import-from-0', tabLast: 'browser-import-x' });
    expect(keptFrom(configure, again.focusAt)).toEqual(['browser-import-popup', 'browser-import-x']);
  });

  test('Import and Try again: Importing has no X, so after it nothing is kept but the popup', async () => {
    const opened = await open('chrome');
    await act('browser-import-run');
    const after = await wizard();
    // The fake Keychain answers nothing: "Couldn’t import", which Try again may clear.
    expect(after).toMatchObject({ step: 'blocked', retry: true, tabFirst: 'browser-import-close', tabLast: 'browser-import-x' });
    expect(after.focusAt).toBe(opened.focusAt + 2); // Importing, then its outcome
    expect(keptFrom(after, opened.focusAt)).toEqual(['browser-import-popup']);
    expect(after.mounted.find(element => element.id === 'browser-import-x')?.since).toBe(after.focusAt);
    await act('browser-import-retry');
    const retried = await wizard();
    expect(retried.step).toBe('blocked');
    expect(retried.focusAt).toBe(after.focusAt + 2); // Importing and back, unread: Close and the X it held are new
    expect(keptFrom(retried, after.focusAt)).toEqual(['browser-import-popup']);
  });

  test('Full Disk Access: Allow as it opens; a grant replaces Allow and keeps Cancel and the X (scenario (b))', async () => {
    const opened = await open('safari');
    expect(opened).toMatchObject({ step: 'fullDiskAccess', fdaGranted: false, openFocus: 'browser-import-fda-allow', tabFirst: 'browser-import-fda-allow', tabLast: 'browser-import-x' });
    expect(opened.mounted.map(element => element.id)).toEqual(['browser-import-fda-allow', 'browser-import-cancel', 'browser-import-fda-continue', 'browser-import-x']);
    world.denied = false; // the 1.5 s poll reads the grant
    const granted = await wizard();
    expect(granted).toMatchObject({ step: 'fullDiskAccess', fdaGranted: true, tabFirst: 'browser-import-cancel', tabLast: 'browser-import-x' });
    expect(granted.focusAt).toBe(opened.focusAt + 1);
    expect(keptFrom(granted, opened.focusAt)).toEqual(['browser-import-popup', 'browser-import-cancel', 'browser-import-fda-continue', 'browser-import-x']);
    // Taken back: Allow mounts again (new), the rest stay.
    world.denied = true;
    const revoked = await wizard();
    expect(keptFrom(revoked, granted.focusAt)).toEqual(['browser-import-popup', 'browser-import-cancel', 'browser-import-fda-continue', 'browser-import-x']);
  });

  test('closing clears the facts, and the next opening is one again', async () => {
    const first = await open('chrome');
    await act('browser-import-close');
    expect(await wizard()).toMatchObject({ open: false, openedAt: -1, focusAt: -1, mounted: [] });
    const second = await open('chrome');
    expect(second.openedAt).toBeGreaterThan(first.focusAt);
  });
});

describe('the Tab stops and the focus as the wizard opens, by screen', () => {
  const facts = (step: string, more: Partial<Parameters<typeof wizardTabStops>[0]> = {}) =>
    ({ step, canClose: step !== 'importing', importDisabled: false, fdaGranted: false, fdaBusy: false, retry: false, from: [], into: [], ...more });
  test('each screen\'s stops in order, the close X last (DialogPopup renders it after the step)', () => {
    expect(wizardTabStops(facts('configure', { from: [1, 2], into: [1] })))
      .toEqual(['browser-import-from-0', 'browser-import-from-1', 'browser-import-into-0', 'browser-import-cancel', 'browser-import-run', 'browser-import-x']);
    expect(wizardTabStops(facts('configure', { from: [1], into: [1], importDisabled: true }))).toEqual(['browser-import-from-0', 'browser-import-into-0', 'browser-import-cancel', 'browser-import-x']);
    expect(wizardTabStops(facts('quit'))).toEqual(['browser-import-cancel', 'browser-import-quit', 'browser-import-x']);
    expect(wizardTabStops(facts('fullDiskAccess'))).toEqual(['browser-import-fda-allow', 'browser-import-cancel', 'browser-import-x']);
    expect(wizardTabStops(facts('fullDiskAccess', { fdaGranted: true }))).toEqual(['browser-import-cancel', 'browser-import-fda-continue', 'browser-import-x']);
    expect(wizardTabStops(facts('fullDiskAccess', { fdaBusy: true }))).toEqual(['browser-import-cancel', 'browser-import-x']);
    expect(wizardTabStops(facts('blocked', { retry: true }))).toEqual(['browser-import-close', 'browser-import-retry', 'browser-import-x']);
    expect(wizardTabStops(facts('done'))).toEqual(['browser-import-done', 'browser-import-x']);
    expect(wizardTabStops(facts('checking'))).toEqual(['browser-import-x']);
    expect(wizardTabStops(facts('importing'))).toEqual([]);
    // A disabled button is no stop but is still mounted (it keeps a focus it held).
    expect(wizardElements(facts('configure', { importDisabled: true })).map(element => element.id)).toContain('browser-import-run');
  });
  test('as it opens, the first stop (the reference over CDP: Personal, Cancel; from source: Allow, Close)', () => {
    expect(wizardOpenFocus(facts('configure', { from: [1, 2], into: [1] }))).toBe('browser-import-from-0');
    expect(wizardOpenFocus(facts('quit'))).toBe('browser-import-cancel');
    expect(wizardOpenFocus(facts('fullDiskAccess'))).toBe('browser-import-fda-allow');
    // Granted already: Allow is replaced by "Allowed" after the first check, so the popup keeps the focus.
    expect(wizardOpenFocus(facts('fullDiskAccess', { fdaGranted: true }))).toBe('browser-import-popup');
    expect(wizardOpenFocus(facts('blocked', { retry: true }))).toBe('browser-import-close');
  });
  test('a change of screen remounts the step and, after a step that cannot close, the X; a grant only Allow', () => {
    const state: { step: WizardStep; fdaGranted: boolean } = { step: { step: 'configure' }, fdaGranted: false };
    const marks = wizardMounts(state), opened = marks.opened;
    goTo(state, { step: 'configure' }); // a vanished target: Configure again, its screen kept
    expect(marks).toMatchObject({ at: opened, step: opened, x: opened });
    goTo(state, { step: 'importing' });
    expect(marks.step).toBeGreaterThan(opened);
    expect(marks.x).toBe(opened); // gone while importing; mounted again only when a closable step follows
    goTo(state, { step: 'blocked', reason: 'readFailed' });
    expect(marks.x).toBe(marks.at);
    const blocked = marks.at;
    goTo(state, { step: 'checking', check: 'fullDiskAccess' });
    goTo(state, { step: 'fullDiskAccess', resume: 'import' });
    expect(marks.x).toBe(blocked); // closable throughout: the X stays
    const fda = marks.at;
    setGranted(state, true);
    expect(marks).toMatchObject({ at: fda + 1, step: fda, allow: fda });
    setGranted(state, true); // no flip, no change
    expect(marks.at).toBe(fda + 1);
  });
});

describe('the contract wiring (browser-profiles.contract, app.contract)', () => {
  test('a closed wizard has no facts, so its next opening counts as one', () => {
    expect(emptyBrowserProfilesView().wizard).toMatchObject({ open: false, openedAt: -1, focusAt: -1, mounted: [], openFocus: '', tabFirst: '', tabLast: '' });
    expect(emptyBrowserProfilesView()).toMatchObject({ removalOpen: false, removalId: '' });
  });
});
