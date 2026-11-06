// The editing engine's names for the field it serves (composer-members.contract fieldTarget), read over a test's
// window composer: the engine serves the window's field unless a side chat's has the focus.
import type { Env } from './contract-action-fixture';

export function windowField(env: Env): Env {
  return {
    fieldSide: false, fieldTarget: '', fieldId: 'composer', fieldRich: env.richComposer, fieldOwner: env.richComposerOwner,
    fieldText: env.composerText, fieldData: env.data, fieldEditor: env.editor,
    ...(typeof env.editorOp === 'function' ? { fieldEditorOp: env.editorOp } : {}),
    // The window's send: a harness that stands in `send` for it counts the engine's sends there.
    ...(typeof env.send === 'function' ? { fieldSubmit: (gesture: string) => (env.send as (held: string) => unknown)(gesture) } : {}),
  };
}
