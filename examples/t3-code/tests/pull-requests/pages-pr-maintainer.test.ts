// "Allow edits from maintainers" (PullRequestActionsMenu.tsx and client-runtime pullRequestEditing.ts):
// when the box shows, the saved choice holding until the first host reading after the save, the write it sends, and
// a refusal putting the box back with the reference's toast.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import { ClientError, type Native } from '../../src/client/protocol';
import { canEditPullRequestMaintainerEdits, maintainerEditsChoiceInForce, maintainerEditsView, saveMaintainerEdits } from '../../src/pull-requests/pages-pr-maintainer';
import { toasts } from '../../src/shell/toast';

const detail = (over: Obj = {}): Obj => ({ state: 'open', maintainerCanModify: false, observedAt: 1_000,
  capabilities: { edit: { maintainerEdits: true } }, viewerPermissions: { maintainerEdits: true }, ...over });

describe('canEditPullRequestMaintainerEdits', () => {
  test('an open pull request the host can change, the viewer may, and that reports the value', () => {
    expect(canEditPullRequestMaintainerEdits(detail())).toBe(true);
    expect([detail({ state: 'merged' }), detail({ capabilities: { edit: {} } }), detail({ viewerPermissions: { maintainerEdits: false } }), detail({ maintainerCanModify: undefined })]
      .map(canEditPullRequestMaintainerEdits)).toEqual([false, false, false, false]);
  });
});

describe('maintainerEditsChoiceInForce: the saved choice holds until the first host reading after the save', () => {
  const choice = { key: 'k', observedAt: 1, value: true };
  test('while saving it follows each reading; after, it ends with a newer reading or another pull request', () => {
    expect(maintainerEditsChoiceInForce(choice, 'k', 2, true)).toEqual({ ...choice, observedAt: 2 });
    expect(maintainerEditsChoiceInForce(choice, 'k', 1, false)).toBe(choice);
    expect(maintainerEditsChoiceInForce(choice, 'k', 2, false)).toBeNull();
    expect(maintainerEditsChoiceInForce(choice, 'k', undefined, false)).toBeNull();
    expect(maintainerEditsChoiceInForce(choice, 'other', 1, true)).toBeNull();
  });
});

describe('the box in the More menu', () => {
  const fixture = (fail = false) => {
    const sent: Obj[] = [];
    const client = { ready: true, connection: 'connected', scopes: ['source-control:write'],
      setMaintainerEdits: async (_native: Native, reference: Obj, allowed: boolean) => { sent.push({ ...reference, maintainerCanModify: allowed }); if (fail) throw new ClientError('Resource not accessible by integration', 'PullRequestOperationError'); } } as unknown as T3Client;
    const native = { later: async () => ({}) } as unknown as Native;
    return { client, native, sent };
  };
  const reference = { projectId: 'p1', host: 'github.com', repository: 'acme/app', number: 7 };
  test('a press sends maintainerCanModify alone and refreshes; the box keeps the choice until a newer reading', async () => {
    const f = fixture();
    let refreshed = 0;
    expect(maintainerEditsView(f.client, 'k', detail())).toEqual({ shown: true, checked: false, disabled: false, saving: false });
    await saveMaintainerEdits(f.client, f.native, { key: 'k', reference, detail: detail(), refresh: () => refreshed++ }, true);
    expect([f.sent, refreshed]).toEqual([[{ ...reference, maintainerCanModify: true }], 1]);
    expect(maintainerEditsView(f.client, 'k', detail()).checked).toBe(true); // the same reading: the choice stands
    expect(maintainerEditsView(f.client, 'k', detail({ observedAt: 1_005, maintainerCanModify: true })).checked).toBe(true);
    expect(maintainerEditsView(f.client, 'k', detail({ observedAt: 1_009, maintainerCanModify: false })).checked).toBe(false); // the host's word now
  });
  test('a refusal puts the box back and says "Could not save maintainer access"', async () => {
    const f = fixture(true);
    await saveMaintainerEdits(f.client, f.native, { key: 'k', reference, detail: detail(), refresh: () => {} }, true);
    expect(maintainerEditsView(f.client, 'k', detail()).checked).toBe(false);
    expect(toasts(f.client).map(toast => [toast.title, toast.description])).toEqual([['Could not save maintainer access', 'Resource not accessible by integration']]);
  });
  test('a save in flight disables only its own pull request\'s box', async () => {
    const f = fixture();
    let release = () => {};
    Object.assign(f.client, { setMaintainerEdits: () => new Promise<void>(resolve => { release = resolve; }) });
    const saved = saveMaintainerEdits(f.client, f.native, { key: 'a', reference, detail: detail(), refresh: () => {} }, true);
    await new Promise(resolve => setTimeout(resolve, 0)); // the wake, then the write is in flight
    expect([maintainerEditsView(f.client, 'a', detail()), maintainerEditsView(f.client, 'b', detail())]).toEqual([
      { shown: true, checked: true, disabled: true, saving: true }, { shown: true, checked: false, disabled: false, saving: false }]);
    release();
    await saved;
    expect(maintainerEditsView(f.client, 'a', detail())).toEqual({ shown: true, checked: true, disabled: false, saving: false });
  });
  test('a wake that fails still sends the write', async () => {
    const f = fixture();
    const native = { later: async () => { throw new ClientError('no module', 'NativeError'); } } as unknown as Native;
    await saveMaintainerEdits(f.client, native, { key: 'k', reference, detail: detail(), refresh: () => {} }, true);
    expect([f.sent.length, toasts(f.client)]).toEqual([1, []]);
  });
  test('without source-control:write the box shows disabled', () => {
    const f = fixture();
    Object.assign(f.client, { scopes: [] });
    expect(maintainerEditsView(f.client, 'k', detail())).toMatchObject({ shown: true, disabled: true });
  });
});
