// The settingsCore resource (lane settings-core): scope sentence, General/Appearance
// sections, palette and restore state for the Contract shell in settings-core.contract.
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import type { T3Client } from './client';
import { decodeClientPrefs, generalSections, memberFiles, resolveScope, restoreLabels, serverContext, type ClientPrefs, type CoreSection } from './settings-core';
import { appearanceSections, fontStack, modeTiles, palette } from './settings-appearance';
import { breadcrumbLabel, scopeAvailable, searchTargetScope } from './settings-search';
import type { CustomTheme } from './settings-themes';
import { scopeMachine, environmentScopeChoices, environmentScopeIcon } from './settings-b-scope';
import { backgroundDialog } from './settings-a-background';
import { archiveConfirmation } from './settings-a-archive';
import { editorView, previewTheme, syncDraft } from './settings-appearance-editor';
import { importView, resetImport } from './settings-appearance-import';
import { rememberDelivery, updateConfirmation } from './settings-a-about';
import { killConfirmation } from './settings-a-telemetry';
import { hostChecks, hostEditorView } from './settings-a-hosts';
import { libraryCards, removalPicks } from './settings-a-collections';

/** The scope sentence keys each label on its text, so a changed label remounts (a reused one-line text kept its old string). */
const keyedLabel = (label: string) => ({ id: `label:${label}`, label, mark: '', ink: '', surface: '', member: '', selected: true, offline: false });

// Pages whose every row is saved on this client; they have no scope sentence (SETTINGS_DEVICE_ONLY_PATHS).
const DEVICE_ONLY = new Set(['appearance', 'snap-shot', 'connections']);

export async function settingsCore(client: T3Client, native: Native | null | undefined, machine: string, projectKeyInput: string, checkoutInput: string, legacyProjectId: string, route: string, target: string, active: boolean, dialogKind = '', dialogSubject = '', deliveryStream = 'embedded', deliveryStaged = false) {
  rememberDelivery(client, deliveryStream, deliveryStaged); // settings-a-about.ts
  // Other routes' own scope buttons pick one physical checkout; read that as the checkout axis.
  const legacyGroup = !projectKeyInput && legacyProjectId ? client.projectGroups().find(group => group.members.some(member => member.id === legacyProjectId)) : undefined;
  const projectKey = legacyGroup ? legacyGroup.key : projectKeyInput, checkout = legacyGroup ? legacyProjectId : checkoutInput;
  const prefs: ClientPrefs = (client.local as unknown as { clientSettings?: ClientPrefs }).clientSettings || decodeClientPrefs({});
  const machineAxis = scopeMachine(client, route, machine); // settings-b: Providers is single-environment
  const scope = resolveScope(client, machineAxis, projectKey, checkout);
  const project = scope.kind === 'project' || scope.kind === 'checkout';
  const files = active && project && route === 'general' ? await memberFiles(client, native, scope.members) : new Map<string, Obj | null>();
  const context = serverContext(client, scope, files);
  const device = client.local.deviceSettings;
  const custom = (client.local as unknown as { customThemes?: CustomTheme[] }).customThemes || [];
  // The theme editor's draft (settings-appearance-editor.ts) paints the settings while it is open.
  const draft = syncDraft(client, active ? dialogKind : '', dialogSubject, prefs.themeLight, prefs.themeDark, device.appearanceMode === 'dark' ? 'dark' : 'light');
  const preview = previewTheme(client);
  if (dialogKind !== 'import') resetImport(client);
  const paintCustom = preview ? [...custom, preview] : custom;
  const paintPrefs = preview ? { ...prefs, [preview.appearance === 'light' ? 'themeLight' : 'themeDark']: preview.id } : prefs;
  let sections: CoreSection[] = [];
  if (active && route === 'general' && scope.kind !== 'unavailable') sections = generalSections(client, context);
  if (active && route === 'appearance') sections = appearanceSections(prefs, true);
  sections = sections.map(section => ({ ...section, rows: section.rows.map((row, index) => ({ ...row, divider: index > 0 })) }));
  const labels = restoreLabels(client.local as never, obj(client.config.settings), client.ready);
  // SettingsScopeBoundary: a search target outside the selected scope explains its owning scope.
  const wanted = target ? searchTargetScope(target) : null;
  const notice = wanted && scope.kind !== 'unavailable' && !scopeAvailable(wanted.scope, scope.kind) ? `${wanted.title} is not available for the selected target. Choose its owning scope to continue.` : '';
  const unavailableMessage = scope.kind === 'unavailable' ? scope.message
    : scope.kind === 'environment' && !client.ready ? `Reconnect ${scope.environmentLabel} to change its settings.` : '';
  return {
    ready: active, route, breadcrumb: breadcrumbLabel(route), showScope: !DEVICE_ONLY.has(route), kind: scope.kind, message: unavailableMessage, notice,
    environmentLabel: scope.environmentLabel, projectLabel: scope.projectLabel, projectMark: scope.projectMark, projectInk: scope.projectInk, projectSurface: scope.projectSurface,
    connective: scope.connective, environmentChoices: environmentScopeChoices(client, route, scope.environmentChoices), environmentIcon: environmentScopeIcon(client, machineAxis), projectChoices: scope.projectChoices,
    environmentKeyed: [keyedLabel(scope.environmentLabel)], projectKeyed: [keyedLabel(scope.projectLabel)],
    representative: String(scope.members[0]?.id ?? ''), scopeKey: `${machine}|${projectKey}|${checkout}`,
    sections, restoreCount: labels.length, restoreText: labels.length ? `This will reset: ${labels.join(', ')}.` : '',
    appearanceMode: device.appearanceMode, tiles: modeTiles(device.appearanceMode, prefs, custom), themes: libraryCards(prefs, custom, removalPicks(client, dialogKind === 'remove' ? dialogSubject : '')), typographyAdvanced: prefs.typographyAdvanced, themeLight: prefs.themeLight,
    palette: palette(paintPrefs, paintCustom, device.appearanceMode), editor: editorView(draft), themeImport: importView(client), interfaceFont: fontStack(prefs.fontFamilySans, false) ?? 'system-ui', codeFont: fontStack(prefs.fontFamilyCode, true) ?? 'ui-monospace', interfaceSize: prefs.fontSizeInterface, codeSize: prefs.fontSizeCode,
    wordWrap: prefs.wordWrap, panelDuration: prefs.panelAnimationDurationMs,
    archiveConfirm: archiveConfirmation(client),
    updateConfirm: updateConfirmation(client),
    killConfirm: killConfirmation(client),
    hostEditor: hostEditorView(client), hostChecks: hostChecks(client),
    // Any settings-a dialog the root must mount (SettingsADialogs).
    saOpen: updateConfirmation(client) || killConfirmation(client).pid !== '' || hostEditorView(client).open,
    background: backgroundDialog(obj(client.config.settings), client.ready && !!client.environmentId && scope.kind !== 'unavailable'),
  };
}
