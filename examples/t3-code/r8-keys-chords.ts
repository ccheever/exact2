// Lane r8-keys: chords the window's buttons must not declare in some contexts.
//
// AppSidebarLayout.tsx (MIT, see LICENSE-T3): the rich-text composer claims
// Mod+B for bold (isRichTextBoldShortcut: Meta or Control with B, no Alt or
// Shift), so while it holds the focus the sidebar toggle stays out of the way;
// the plain-text composer and every other place keep the toggle.
import { obj, type Obj } from './domain';
import type { T3Client } from './client';
import { panelState } from './r4-surfaces-panel';

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

/** The sidebar toggle's chords without the bold chord while the rich-text composer is focused. */
export function claimBoldChord<T extends { keySidebar: string }>(keys: T, composerFocus: boolean, richText: boolean): T {
  if (!composerFocus || !richText || !keys.keySidebar) return keys;
  return { ...keys, keySidebar: keys.keySidebar.split(' ').filter(chord => chord && !isBoldChord(chord)).join(' ') };
}

/** diff.toggle (ChatView onToggleDiff → useRightPanelStore.toggle(ref, "diff")): an open
 * panel showing the diff closes; otherwise the diff surface opens. */
export function diffShown(client: T3Client): boolean {
  const state = panelState(client), active = state.surfaces.find(surface => surface.id === state.active);
  return client.diffOpen || (state.visible && active?.kind === 'diff');
}
