// Source365aa87982 use-composer-command-menu.ts selection, including its optional Limits callback.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {mobileComposerCommandReplacement,replaceTextRange} from './composer-command-model';
/** The source supplies its local callback only when Limits is offered and no attachments exist.
 * This decision prepares an edit; only an accepted native CAS may open the report. */
export function mobileComposerMenuSelection(input:Parameters<typeof mobileComposerCommandReplacement>[0]&{openUsageLimits:boolean}) {
  if(input.openUsageLimits&&input.item.type==='provider-slash-command'&&input.item.command.name==='usage-limits')
    return {...replaceTextRange(input.draftMessage,input.trigger.rangeStart,input.trigger.rangeEnd,''),interactionMode:null,localCommand:'usage-limits' as const};
  const value=mobileComposerCommandReplacement(input);
  return value?{...value,localCommand:null}:null;
}
