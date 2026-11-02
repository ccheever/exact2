// Chords the window's buttons must not declare in some contexts.
//
// AppSidebarLayout.tsx (MIT, see LICENSE-T3): the rich-text composer claims
// Mod+B to bold its selected text (isRichTextBoldShortcut: Meta or Control
// with B, no Alt or Shift, and a selection that is not collapsed), so while it
// holds the focus with text selected the sidebar toggle stays out of the way;
// with nothing selected, in the plain-text composer and everywhere else the
// chord toggles the sidebar.
import { obj, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import { panelState } from '../panel/panel-surfaces';

/** The composer's Rich text setting (Settings › General), on unless turned off. */
export function richTextComposer(client: T3Client): boolean {
  return obj(client.local.clientSettings as Obj | undefined).composerRichTextEnabled !== false;
}

/** isRichTextBoldShortcut over an aria-keyshortcuts chord ("Meta+b", "Control+B"). */
export function isBoldChord(chord: string): boolean {
  const parts = chord.split('+');
  const key = parts.pop() ?? '';
  return key.toLowerCase() === 'b' && parts.length === 1 && (parts[0] === 'Meta' || parts[0] === 'Control');
}

/** The sidebar toggle's chords without the bold chord while the rich-text composer is focused with text selected. */
export function claimBoldChord<T extends { keySidebar: string }>(keys: T, composerFocus: boolean, richText: boolean, selected: boolean): T {
  if (!composerFocus || !richText || !selected || !keys.keySidebar) return keys;
  return { ...keys, keySidebar: keys.keySidebar.split(' ').filter(chord => chord && !isBoldChord(chord)).join(' ') };
}

/** diff.toggle (ChatView onToggleDiff → useRightPanelStore.toggle(ref, "diff")): an open
 * panel showing the diff closes; otherwise the diff surface opens. */
export function diffShown(client: T3Client): boolean {
  const state = panelState(client), active = state.surfaces.find(surface => surface.id === state.active);
  return client.diffOpen || (state.visible && active?.kind === 'diff');
}
