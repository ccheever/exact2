// Appearance → the theme library's cards (lane settings-a). Reference ThemeSettings.tsx
// ThemeLibrary / CustomThemeCollectionCard / ThemeLibraryCard variantNavigation and the
// removal AlertDialog, ThemePreviewCircles getThemeCardDefinition. An Open VSX extension
// with several themes is one collection card: per appearance, the selected variant's
// orb with a fan of the others; one-appearance themes show only their own orb.
import type { ClientPrefs } from './settings-core';
import { themeCards, themeOrb, type ThemeCard, type ThemeOrb } from './settings-appearance';
import { ClientError } from './protocol';
import type { CustomTheme } from './settings-themes';

export type VariantOption = { themeId: string; label: string; active: boolean; aria: string; orb: ThemeOrb; x: number; y: number; delay: number };
export type VariantMode = { mode: string; themeId: string; label: string; more: number; active: boolean; aria: string; offset: number; orb: ThemeOrb; options: VariantOption[] };
export type RemovalOption = { themeId: string; label: string; orbs: ThemeOrb[]; checked: boolean };
export type LibraryCard = ThemeCard & { collection: boolean; collectionId: string; primary: string; primaryLabel: string; variants: VariantMode[]; removal: RemovalOption[]; picked: number; pickedIds: string };
type Collection = { id: string; label: string };
const collectionOf = (theme: CustomTheme) => (theme as CustomTheme & { collection?: Collection }).collection;
const modesOf = (theme: CustomTheme): ('light' | 'dark')[] => (['light', 'dark'] as const).filter(mode => theme[mode] !== null || theme.appearance === mode);

/** collectionVariantLabels: drop the words every label in the collection shares. */
export function variantLabels(labels: string[]): string[] {
  if (labels.length === 0) return [];
  const words = labels.map(label => label.trim().split(/\s+/)), first = words[0]!;
  const shared = first.findIndex((word, index) => words.some(other => other[index]?.toLowerCase() !== word.toLowerCase()));
  const prefix = shared === -1 ? first.length - 1 : shared;
  return labels.map((label, index) => words[index]!.slice(Math.max(0, prefix)).join(' ').trim() || label);
}

/** The Appearance page's cards: the six library themes, each custom theme, each collection once. */
export function libraryCards(prefs: Pick<ClientPrefs, 'themeLight' | 'themeDark'>, custom: CustomTheme[] = [], picks: RemovalPicks = { subject: '', ids: [] }): LibraryCard[] {
  const plain = new Map(themeCards(prefs, custom).map(card => [card.id, card]));
  const blank = { collection: false, collectionId: '', variants: [] as VariantMode[], removal: [] as RemovalOption[], picked: 0, pickedIds: '' };
  const out: LibraryCard[] = [];
  const seen = new Set<string>();
  for (const id of ['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris']) { const card = plain.get(id); if (card) out.push({ ...card, ...blank, primary: id, primaryLabel: card.label }); }
  for (const theme of custom) {
    const collection = collectionOf(theme);
    const members = collection ? custom.filter(entry => collectionOf(entry)?.id === collection.id) : [theme];
    if (collection && seen.has(collection.id)) continue;
    if (collection) seen.add(collection.id);
    const card = plain.get(theme.id)!;
    if (members.length < 2) {
      // A one-appearance theme only takes its own side of the mix.
      out.push({ ...card, ...blank, primary: theme.id, primaryLabel: card.label, orbs: card.orbs.filter(orb => modesOf(theme).includes(orb.mode as 'light' | 'dark')) });
      continue;
    }
    const labels = variantLabels(members.map(member => member.label));
    const active = (id: string, mode: 'light' | 'dark') => (mode === 'light' ? prefs.themeLight : prefs.themeDark) === id;
    const variants: VariantMode[] = (['light', 'dark'] as const).flatMap(mode => {
      const options = members.flatMap((member, index) => modesOf(member).includes(mode) ? [{ member, label: labels[index]! }] : []);
      if (!options.length) return [];
      const selected = options.find(option => active(option.member.id, mode)) ?? options[0]!;
      const offset = mode === 'light' ? -52 : 52, many = options.length > 1;
      const entry: VariantMode = { mode, themeId: selected.member.id, label: selected.label, more: options.length - 1, active: active(selected.member.id, mode), offset,
        aria: many ? `Choose ${mode} variant, ${options.length} options, currently ${selected.label}` : `Use ${mode} variant, currently ${selected.label}`,
        orb: themeOrb(selected.member.id, mode, active(selected.member.id, mode), custom),
        options: many ? options.map((option, index) => {
          const progress = index / (options.length - 1) - 0.5;
          return { themeId: option.member.id, label: option.label, active: active(option.member.id, mode), orb: themeOrb(option.member.id, mode, active(option.member.id, mode), custom),
            aria: `Use ${option.label} for ${mode} mode${active(option.member.id, mode) ? ', currently active' : ''}`, x: offset + progress * 68, y: Math.abs(progress) * 10, delay: index * 35 };
        }) : [] };
      return [entry];
    });
    const primary = variants.find(variant => variant.active)?.themeId ?? members[0]!.id;
    const id = `collection:${collection!.id}`, picked = picks.subject === id ? picks.ids.filter(pick => members.some(member => member.id === pick)) : [];
    out.push({ ...plain.get(primary)!, id, label: collection!.label, custom: true, selected: false, collection: true, collectionId: collection!.id, primary,
      primaryLabel: members.find(member => member.id === primary)!.label, variants, picked: picked.length, pickedIds: picked.join(','),
      removal: members.map(member => ({ themeId: member.id, label: member.label, checked: picked.includes(member.id), orbs: modesOf(member).map(mode => themeOrb(member.id, mode, false, custom)) })) });
  }
  return out;
}

/** The removal dialog's checked variants, for the card the dialog was opened on. */
export type RemovalPicks = { subject: string; ids: string[] };
const PICKS = new WeakMap<object, RemovalPicks>();
export function removalPicks(owner: object, subject: string): RemovalPicks {
  const picks = PICKS.get(owner);
  if (picks?.subject === subject) return picks;
  const fresh = { subject, ids: [] };
  PICKS.set(owner, fresh);
  return fresh;
}
/** restlocal:theme-pick: check or uncheck one variant (the dialog's checkbox). */
export function toggleRemovalPick(owner: object, subject: string, themeId: string): void {
  const picks = removalPicks(owner, subject);
  picks.ids = picks.ids.includes(themeId) ? picks.ids.filter(id => id !== themeId) : [...picks.ids, themeId];
}

type Prefs = { theme: string; themeLight: string; themeDark: string };
type Local = { customThemes?: CustomTheme[]; clientSettings?: Prefs };
/** theme-collection: "Use the first variants for light and dark". */
export function useCollectionDefaults(local: Local, collectionId: string): void {
  const members = (local.customThemes ?? []).filter(theme => collectionOf(theme)?.id === collectionId);
  if (!members.length) throw new ClientError('That theme collection is no longer installed.');
  const light = members.find(theme => modesOf(theme).includes('light')), dark = members.find(theme => modesOf(theme).includes('dark'));
  if (local.clientSettings) local.clientSettings = { ...local.clientSettings, ...(light ? { themeLight: light.id } : {}), ...(dark ? { themeDark: dark.id } : {}) };
}
/** theme-remove-many: the selected variants; a removed half falls back to T3 Code. */
export function removeThemes(local: Local, ids: string[]): void {
  const themes = local.customThemes ?? [];
  const removing = new Set(ids.filter(id => themes.some(theme => theme.id === id)));
  if (!removing.size) throw new ClientError('Choose at least one theme to remove.');
  local.customThemes = themes.filter(theme => !removing.has(theme.id));
  const fallback = (id: string) => removing.has(id) ? 't3-code' : id;
  if (local.clientSettings) local.clientSettings = { ...local.clientSettings, theme: fallback(local.clientSettings.theme), themeLight: fallback(local.clientSettings.themeLight), themeDark: fallback(local.clientSettings.themeDark) };
}
