// "Allow edits from maintainers" in a GitHub pull request's More menu (T3 Code 6d450a4284, MIT, see LICENSE-T3:
// components/pullRequest/PullRequestActionsMenu.tsx and packages/client-runtime/src/pullRequestEditing.ts
// canEditPullRequestMaintainerEdits, maintainerEditsChoiceInForce). Shown on an open pull request whose host can
// change it and whose viewer may; the box reads `maintainerCanModify`. A press saves the other value through
// pullRequests.update (T3Client.setMaintainerEdits) and the box shows that choice until the first host reading
// delivered after the save; the panel reads the detail again once it is saved. A refusal puts the box back and says
// "Could not save maintainer access".
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { can } from '../connections/command-permissions';
import { pushToast } from '../shell/toast';
import { readableFailure } from '../details/pr-row-logic';

export type MaintainerEditsChoice = { key: string; observedAt: string | undefined; value: boolean };

export function canEditPullRequestMaintainerEdits(detail: Obj): boolean {
  return detail.state === 'open' && obj(obj(detail.capabilities).edit).maintainerEdits === true && obj(detail.viewerPermissions).maintainerEdits === true
    && typeof detail.maintainerCanModify === 'boolean';
}

/**
 * A saved choice ends at the first host reading delivered after the save. Readers refresh once it succeeds, which
 * drops reads still in flight, so that reading started after the write.
 */
export function maintainerEditsChoiceInForce(choice: MaintainerEditsChoice | null, key: string, observedAt: string | undefined, saving: boolean): MaintainerEditsChoice | null {
  if (choice === null || choice.key !== key) return null;
  if (saving) return choice.observedAt === observedAt ? choice : { ...choice, observedAt };
  return observedAt !== undefined && choice.observedAt === observedAt ? choice : null;
}

type State = { choice: MaintainerEditsChoice | null; saving: boolean };
const states = new WeakMap<object, State>();
const stateOf = (client: object): State => { let state = states.get(client); if (!state) { state = { choice: null, saving: false }; states.set(client, state); } return state; };
const observedAtOf = (detail: Obj): string | undefined => (typeof detail.observedAt === 'string' ? detail.observedAt : undefined);
/** canWriteSourceControl and environmentConnected. */
const canSave = (client: T3Client) => can(client, 'source-control:write') && client.connection === 'connected';

export type MaintainerEditsView = { shown: boolean; checked: boolean; disabled: boolean; saving: boolean };
export const NO_MAINTAINER_EDITS: MaintainerEditsView = { shown: false, checked: false, disabled: true, saving: false };

export function maintainerEditsView(client: T3Client, key: string, detail: Obj): MaintainerEditsView {
  if (!canEditPullRequestMaintainerEdits(detail)) return NO_MAINTAINER_EDITS;
  const state = stateOf(client);
  state.choice = maintainerEditsChoiceInForce(state.choice, key, observedAtOf(detail), state.saving);
  return { shown: true, checked: (state.choice?.value ?? detail.maintainerCanModify) === true, disabled: !canSave(client) || state.saving, saving: state.saving };
}

/** The box's press: `value` is the new choice. `refresh` reads the detail again (refreshDetail). */
export async function saveMaintainerEdits(client: T3Client, native: Native, input: { key: string; reference: Obj; detail: Obj | null; refresh: () => void }, value: boolean): Promise<string> {
  const state = stateOf(client), detail = input.detail;
  if (!detail || !canEditPullRequestMaintainerEdits(detail) || !canSave(client) || state.saving) return '';
  const previous = state.choice;
  state.choice = { key: input.key, observedAt: observedAtOf(detail), value };
  state.saving = true;
  try {
    await native.later({ op: 'wake', topic: 't3.pr' }); // the panel shows "Saving…" while the host works
    await client.setMaintainerEdits(native, { projectId: input.reference.projectId, ...(str(input.reference.host) ? { host: input.reference.host } : {}),
      repository: input.reference.repository, number: input.reference.number }, value);
  } catch (error) {
    state.saving = false;
    state.choice = previous;
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Could not save maintainer access', description: readableFailure(error, 'Try again.') });
    return '';
  }
  state.saving = false;
  input.refresh();
  return '';
}
