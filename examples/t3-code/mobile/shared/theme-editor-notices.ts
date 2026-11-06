// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/theme-editor-notices.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// MIT T3 Code 1e2ecbd975 (LICENSE-T3): ThemeEditorHost.tsx handleSaved. The panel reports
// which path its save took (a theme removed while its editor was open saves as a create),
// and the host activates the theme where the reference does and says what happened.
export type ThemeSaveContext = { created: boolean; mergedAppearance?: 'light' | 'dark' };
export type ThemeNotice = { kind: 'success' | 'error'; title: string; description: string };
type Prefs = { theme: string; themeLight: string; themeDark: string };

export const STORAGE_FAILED: ThemeNotice = { kind: 'error', title: 'Could not save your theme', description: 'Browser storage is unavailable, so the change was not kept.' };

/**
 * The notice for a saved theme. `activate` is setTheme (false when the preference could not
 * be kept); it runs for a create and a merge, never for an edit, whose preference stays
 * untouched (a setTheme there would clear a light/dark mix).
 */
export function themeSavedNotice(saved: { id: string; label: string }, context: ThemeSaveContext, prefs: Prefs, activate: () => boolean): ThemeNotice {
  if (context.mergedAppearance) {
    if (!activate()) return STORAGE_FAILED;
    return { kind: 'success', title: `${saved.label} updated`, description: `Its ${context.mergedAppearance} palette was added.` };
  }
  if (!context.created) {
    const wasActive = prefs.theme === saved.id || prefs.themeLight === saved.id || prefs.themeDark === saved.id;
    return { kind: 'success', title: `${saved.label} saved`, description: wasActive ? 'Your changes are now active.' : 'Your changes are saved.' };
  }
  if (!activate()) return STORAGE_FAILED;
  return { kind: 'success', title: `${saved.label} created`, description: 'It’s now active.' };
}
