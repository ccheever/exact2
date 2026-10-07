// @ref llp/1106.002-design-system-parity.spec.md#typography-and-font-assets
// Upstream 365aa87982 appearance section components and appearancePreferences.ts.
import { themeCards } from './shared/settings-appearance';
import { MOBILE_THEME_ARTWORK } from './settings-theme-data';
import { mobileTextRoles, normalizeMobilePreferences, resolveMobileAppearance, type MobilePreferences, type MobileThemeId } from './settings-preferences';
import t0 from './themes/light.json';
import t1 from './themes/dark.json';
import t2 from './themes/t3-chat-light.json';
import t3 from './themes/t3-chat-dark.json';
import t4 from './themes/grove-light.json';
import t5 from './themes/grove-dark.json';
import t6 from './themes/ocean-light.json';
import t7 from './themes/ocean-dark.json';
import t8 from './themes/ember-light.json';
import t9 from './themes/ember-dark.json';
import t10 from './themes/iris-light.json';
import t11 from './themes/iris-dark.json';
const palettes: Record<string, Record<string, string>> = { 'light': t0, 'dark': t1, 't3-chat-light': t2, 't3-chat-dark': t3, 'grove-light': t4, 'grove-dark': t5, 'ocean-light': t6, 'ocean-dark': t7, 'ember-light': t8, 'ember-dark': t9, 'iris-light': t10, 'iris-dark': t11 };
export function settingsTokens(scheme: string, themeId: string) {
  const mode = scheme === 'dark' ? 'dark' : 'light';
  return palettes[`${themeId === 't3-code' ? '' : themeId + '-'}${mode}`] ?? palettes[mode]!;
}
export function settingsColors(scheme: string, themeId: string) {
  const tokens = settingsTokens(scheme, themeId);
  const token = (key: string) => tokens[`--color-${key}`]!;
  return { sheet: token('sheet'), grouped: token('grouped-card'), card: token('card'), foreground: token('foreground'),
    muted: token('foreground-muted'), border: token('border'), borderSubtle: token('border-subtle'), separator: token('separator'),
    subtle: token('subtle'), primary: token('primary'), primaryForeground: token('primary-foreground'),
    icon: token('icon'), iconMuted: token('icon-muted'), iconSubtle: token('icon-subtle'), chevron: token('chevron'),
    switchOn: token('switch-active-track'), switchOff: token('switch-inactive-track'), secondaryBorder: token('secondary-border'), link: token('md-link') };
}
function wire(themeId: string, scheme: string) {
  const tokens = settingsTokens(scheme, themeId), token = (key: string) => tokens[`--color-${key}`]!;
  return { screen: token('screen'), card: token('card'), primary: token('primary'), muted: token('foreground-muted'),
    subtle: token('subtle-strong'), bubble: token('user-bubble'), border: token('border'), drawer: token('drawer') };
}
export function settingsAppearanceView(input: unknown, systemScheme: string, ready: boolean, safeBottom = 0) {
  const preferences = normalizeMobilePreferences(input), appearance = resolveMobileAppearance(preferences, systemScheme);
  const colors = settingsColors(appearance.scheme, appearance.themeId), roles = mobileTextRoles(preferences.baseFontSize);
  const terminal = MOBILE_THEME_ARTWORK[appearance.themeId][appearance.scheme];
  // Reuse the desktop clone's built-in theme labels/order. Mobile artwork is the pinned
  // native preview data above, because the clone's orb base uses a different color mix.
  const cards = themeCards({ themeLight: preferences.lightThemeId, themeDark: preferences.darkThemeId }).map(card => ({
    id: card.id, label: card.label, selected: card.selected,
    orbs: (['light', 'dark'] as const).map(mode => {
      const spec = mode === 'light' ? { ax: .72, ay: .22, px: .18, py: .82, middle: .72 } : { ax: .28, ay: .78, px: .82, py: .18, middle: .62 };
      const artwork = MOBILE_THEME_ARTWORK[card.id as MobileThemeId][mode];
      return { base: artwork.base, accent: artwork.accent, action: artwork.action, mode,
        key: `${card.id}-${mode}`, selected: preferences[mode === 'light' ? 'lightThemeId' : 'darkThemeId'] === card.id,
        symbol: mode === 'light' ? 'sun.max' : 'moon', ring: mode === 'dark' ? 'rgba(255, 255, 255, 0.14)' : 'rgba(0, 0, 0, 0.1)',
        ax: spec.ax * 64, ay: spec.ay * 64, px: spec.px * 64, py: spec.py * 64, middle: spec.middle,
        ar: Math.hypot(Math.max(spec.ax, 1 - spec.ax), Math.max(spec.ay, 1 - spec.ay)) * 64,
        pr: Math.hypot(Math.max(spec.px, 1 - spec.px), Math.max(spec.py, 1 - spec.py)) * 64 };
    }),
  }));
  const modes = (['system', 'light', 'dark'] as const).map(id => ({ id, label: id[0]!.toUpperCase() + id.slice(1),
    selected: preferences.themeMode === id, split: id === 'system',
    left: wire(id === 'dark' ? preferences.darkThemeId : preferences.lightThemeId, id === 'dark' ? 'dark' : 'light'),
    right: wire(id === 'light' ? preferences.lightThemeId : preferences.darkThemeId, id === 'light' ? 'light' : 'dark') }));
  return { ready, bottom: Math.max(safeBottom, 18) + 18, colors, roles, cards, modes, ...appearance,
    terminalLabel: `${appearance.terminalFontSize.toFixed(1)} pt`, codeLabel: `${appearance.codeFontSize} pt`, baseLabel: `${appearance.baseFontSize} pt`,
    bodyLine: Math.max(18, Math.round(23 * preferences.baseFontSize / 16)), sampleSmallSize: Math.max(10, Math.round(14 * preferences.baseFontSize / 16)),
    sampleSmallLine: Math.round(Math.max(10, Math.round(14 * preferences.baseFontSize / 16)) * 1.4),
    terminalLine: Math.round(appearance.terminalFontSize * 1.6), terminalForeground: terminal.terminalForeground,
    terminalBackground: terminal.terminalBackground, terminalCursor: terminal.terminalCursor,
    codeLine: Math.max(14, Math.round(22 * appearance.codeFontSize / 12)), codeNumber: Math.max(8, Math.round(11 * appearance.codeFontSize / 12)),
    codeLines: [
      { id: 'signature', number: '1', keyword: 'function', before: '', text: ' formatUser(user) {' },
      { id: 'body', number: '2', keyword: 'return', before: '  ', text: ' `${user.name} <${user.email}>` // demonstrates how long lines behave' },
      { id: 'close', number: '3', keyword: '', before: '', text: '}' },
    ],
  };
}
export function settingsChoices(route: string, preferences: MobilePreferences, ready: boolean) {
  if (route === 'SettingsKeyboard') return { key: 'composerEnterBehavior', title: 'Return key',
    footer: 'Applies to the composer when a hardware keyboard is connected.', ready,
    options: [ { value: 'send', label: 'Send message', description: 'Return sends the message. Shift-Return inserts a new line.' },
      { value: 'newline', label: 'Insert new line', description: 'Return inserts a new line. Command-Return sends the message.' } ]
      .map((option, index) => ({ ...option, selected: preferences.composerEnterBehavior === option.value, separated: index > 0 })) };
  if (route === 'SettingsFollowUp') return { key: 'followUpBehavior', title: 'While the agent is running', ready,
    footer: 'Long-press the send button to use the other option for a single message. With a hardware keyboard, hold Command while sending.',
    options: [ { value: 'queue', label: 'Queue', description: 'Your message waits and runs after the current turn finishes.' },
      { value: 'steer', label: 'Steer', description: 'Your message reaches the agent right away, changing what it is working on.' } ]
      .map((option, index) => ({ ...option, selected: preferences.followUpBehavior === option.value, separated: index > 0 })) };
  if (route === 'SettingsOrganization' || route === 'SettingsProjectGrouping') return { key: 'projectGroupingMode', title: 'Project grouping', footer: '', ready,
    options: [ { value: 'repository', label: 'Group by repository', description: 'Matching repositories appear as one project.' },
      { value: 'repository_path', label: 'Group by repository path', description: 'Keep monorepo paths separate.' },
      { value: 'separate', label: 'Keep separate', description: 'Show every workspace as its own project.' } ]
      .map((option, index) => ({ ...option, selected: preferences.projectGroupingMode === option.value, separated: index > 0 })) };
  throw new Error(`No local preference screen for ${route}.`);
}
