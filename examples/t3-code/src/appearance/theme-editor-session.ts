// MIT T3 Code 1e2ecbd975 (LICENSE-T3): apps/web/src/components/settings/themeEditorStore.ts
// and the session half of ThemeEditorHost.tsx. Changes: the store is a plain object (no
// zustand); the app keeps one per client, and the window's root state (`settingsThemeDialog`
// create/edit/duplicate with its subject) names the session the Contract host shows, so
// the editor lives above the routes (D16). Themes are named by id and read fresh from the
// library on every answer, so an edit or an import under an open session shows at once.
import type { CustomTheme } from './settings-themes';

export type ThemeAppearance = 'light' | 'dark';
export type ThemeEditorSessionInput = {
  /** Set when editing an installed theme; null creates a new one. */
  editingThemeId: string | null;
  /** Theme a new theme starts from: the active one, or a duplicate target. */
  seedThemeId: string | null;
  /** Prefilled name, used by duplicate. */
  seedName: string | null;
  initialAppearance: ThemeAppearance;
};
/** `id` distinguishes two sessions that name the same themes. */
export type ThemeEditorSession = ThemeEditorSessionInput & { id: number };
export type ThemeEditorStore = { session: ThemeEditorSession | null; openThemeEditor(session: ThemeEditorSessionInput): void; closeThemeEditor(): void };

let nextSessionId = 0;
export function createThemeEditorStore(): ThemeEditorStore {
  const store: ThemeEditorStore = {
    session: null,
    openThemeEditor: session => { store.session = { ...session, id: ++nextSessionId }; },
    closeThemeEditor: () => { store.session = null; },
  };
  return store;
}

const stores = new WeakMap<object, ThemeEditorStore>();
/** The client's one editor store (useThemeEditorStore). */
export function themeEditorStore(owner: object): ThemeEditorStore {
  let store = stores.get(owner);
  if (!store) { store = createThemeEditorStore(); stores.set(owner, store); }
  return store;
}

export const BUILT_IN_THEMES: Record<string, string> = { 't3-code': 'T3 Code', 't3-chat': 'T3 Chat', grove: 'Grove', ocean: 'Ocean', ember: 'Ember', iris: 'Iris' };
/** getThemeDefinition: a built-in or an installed custom theme, by id; null when neither. */
export function themeDefinition(id: string | null | undefined, custom: readonly CustomTheme[]): { id: string; label: string; custom: CustomTheme | null } | null {
  if (!id) return null;
  const own = custom.find(theme => theme.id === id);
  if (own) return { id: own.id, label: own.label, custom: own };
  return BUILT_IN_THEMES[id] ? { id, label: BUILT_IN_THEMES[id]!, custom: null } : null;
}

/** Toggle theme editor: close an open session, or open a new theme seeded from the theme active for the appearance. */
export function toggleThemeEditorForTheme(store: ThemeEditorStore, input: { theme: string; themeHalves: Partial<Record<ThemeAppearance, string>> | null; initialAppearance: ThemeAppearance },
  custom: readonly CustomTheme[] = []): void {
  if (store.session) { store.closeThemeEditor(); return; }
  const baseThemeId = themeDefinition(input.theme, custom)?.id ?? null;
  const activeThemeId = input.themeHalves?.[input.initialAppearance] ?? baseThemeId;
  const seedThemeId = activeThemeId ? themeDefinition(activeThemeId, custom)?.id ?? null : null;
  store.openThemeEditor({ editingThemeId: null, seedThemeId, seedName: null, initialAppearance: input.initialAppearance });
}

/** The window's editor dialog (kind, `<subject>#<request>`) as a session input; null for any other dialog. */
export function sessionInputFor(kind: string, request: string, prefs: { theme: string; themeLight: string; themeDark: string }, appearance: ThemeAppearance,
  custom: readonly CustomTheme[]): ThemeEditorSessionInput | null {
  // The window names a request as `<theme id>#<request number>`; theme ids never contain '#'.
  const subject = request.split('#')[0] ?? '';
  // Every kind opens on the app's resolved appearance (ThemeSettings.tsx passes initialAppearance for an edit too);
  // the draft falls back to the theme's own appearance when it has no colours for that one (sourceAppearance).
  if (kind === 'edit') return { editingThemeId: subject, seedThemeId: null, seedName: null, initialAppearance: appearance };
  if (kind === 'duplicate') {
    const seed = themeDefinition(subject, custom);
    return { editingThemeId: null, seedThemeId: seed?.id ?? null, seedName: seed ? `${seed.label} copy` : null, initialAppearance: appearance };
  }
  if (kind !== 'create') return null;
  const store = createThemeEditorStore();
  toggleThemeEditorForTheme(store, { theme: prefs.theme, themeHalves: { light: prefs.themeLight, dark: prefs.themeDark }, initialAppearance: appearance }, custom);
  const { id: _id, ...input } = store.session!;
  return input;
}

/**
 * ThemeEditorPanel's first appearance (ThemeEditorPanel.tsx:379-383): the session's, when the source theme has colours
 * for it (getThemeColorsForMode), else the source's own. A built-in has both; a custom theme may have one.
 */
export function sourceAppearance(source: { custom: CustomTheme | null } | null, initial: ThemeAppearance): ThemeAppearance {
  const own = source?.custom;
  return own && own.appearance !== initial && !own[initial] ? own.appearance : initial;
}

/** ThemeEditorHost's useThemeDefinition pair: the session's themes as the library has them now (null once removed). */
export function sessionThemes(session: ThemeEditorSessionInput | null, custom: readonly CustomTheme[]) {
  return { editingTheme: session ? themeDefinition(session.editingThemeId, custom) : null, seedTheme: session ? themeDefinition(session.seedThemeId, custom) : null };
}
