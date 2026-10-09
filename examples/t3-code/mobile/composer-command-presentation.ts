// Pinned365aa87982 ComposerCommandPopover/ThreadComposer presentation, T3 Code MIT.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { resolveProviderSkillSourceKind, type ComposerCommandItem, type ComposerTriggerKind } from './composer-command-model';
import { mobileComposerFileIcon } from './composer-editor-document';

export interface ComposerCommandRow {
  id: string; label: string; description: string; icon: string; image: string;
  iconSize: number; slashSkill: boolean; skillName: string; last: boolean;
}
export interface ComposerCommandPopover {
  admission: string; visible: boolean; title: string; empty: string; rows: ComposerCommandRow[];
}
const titles: Partial<Record<ComposerTriggerKind,string>> = {
  'slash-command':'COMMANDS', skill:'SKILLS', path:'FILES', 'pull-request':'PULL REQUESTS',
};
const empty: Partial<Record<ComposerTriggerKind,string>> = {
  'slash-command':'No matching commands.', skill:'No skills found.', path:'No matching files or folders.',
  'pull-request':'No matching pull requests.',
};
/** Visibility belongs to the source Thread parent; a path spinner without rows stays hidden. */
export function mobileComposerCommandPresentation(input: {
  admission: string; trigger: ComposerTriggerKind | null; items: readonly ComposerCommandItem[];
  loading: boolean; error: string | null; voiceBusy: boolean;
}): ComposerCommandPopover {
  const {trigger,items}=input;
  return {
    admission:input.admission,
    visible:!input.voiceBusy && trigger!==null && (items.length>0 || trigger==='pull-request'),
    title:trigger ? titles[trigger] ?? '' : '',
    empty:input.error ?? (input.loading ? trigger==='path' ? 'Searching files…' : 'Loading…' : trigger ? empty[trigger] ?? 'No results.' : 'No results.'),
    rows:items.map((item,index)=>({id:item.id,label:item.label,description:item.description,
      icon:item.type==='path' ? item.kind==='directory' ? 'folder' : '' : item.type==='thread' ? 'thread'
        : item.type==='pull-request' ? 'pull-request' : item.type==='skill'
          ? ({repo:'folder',project:'folder',app:'app',personal:'personal',system:'system',other:'other'} as const)[resolveProviderSkillSourceKind(item.skill)] : 'terminal',
      image:item.type==='path' && item.kind==='file' ? `assets/file-icons/pierre_${mobileComposerFileIcon(item.path)}.png` : '',
      iconSize:item.type==='path' ? 16 : 14,slashSkill:trigger==='slash-command' && item.type==='skill',
      skillName:item.type==='skill' ? item.skill.name : '',last:index===items.length-1})),
  };
}

// RN0.88 StyleSheet.hairlineWidth, used by pinned ComposerCommandPopover.
export function mobileComposerHairline(displayScale: number): number {
  const scale = Number.isFinite(displayScale) && displayScale > 0 ? displayScale : 1;
  return Math.round(0.4 * scale) / scale || 1 / scale;
}
