// The settingsCore resource (lane settings-core): scope sentence, General/Appearance
// sections, palette and restore state for the Contract shell in settings-core.contract.
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import type { T3Client } from './client';
import { decodeClientPrefs, generalSections, memberFiles, resolveScope, restoreLabels, serverContext, type ClientPrefs, type CoreSection } from './settings-core';
import { appearanceSections, fontStack, modeTiles, palette } from './settings-appearance';
import { breadcrumbLabel, scopeAvailable, searchTargetScope } from './settings-search';
import type { CustomTheme } from './settings-themes';
import { scopeMachine, singleEnvironmentRoute } from './settings-b-scope';
import { isPrimaryEnvironment } from './local-primary';
import { backgroundDialog } from './settings-a-background';
import { archiveConfirmation } from './settings-a-archive';
import { editorView, syncDraft } from './settings-appearance-editor';
import { paintOf } from './settings-appearance-look';
import { letGo } from './let-go';
import { importView, resetImport } from './settings-appearance-import';
import { rememberDelivery, updateConfirmation } from './settings-a-about';
import { killConfirmation } from './settings-a-telemetry';
import { hostChecks, hostEditorView } from './settings-a-hosts';
import { libraryCards, removalPicks } from './settings-a-collections';
import { fontDiffPreview } from './settings-font-previews';

/** The scope sentence keys each label on its text, so a changed label remounts (a reused one-line text kept its old string). */
const keyedLabel = (label: string) => ({ id: `label:${label}`, label, mark: '', ink: '', surface: '', member: '', selected: true, offline: false });

// Pages whose every row is saved on this client; they have no scope sentence (SETTINGS_DEVICE_ONLY_PATHS).
const DEVICE_ONLY = new Set(['appearance', 'snap-shot', 'connections']);
const EDITOR_KINDS = new Set(['create', 'edit', 'duplicate']);

/**
 * The settings shell's resolved scope (SettingsScopeContext useResolvedSettingsScope). Other routes' own scope
 * buttons pick one physical checkout, read as the checkout axis. Providers is single-environment (settings-b):
 * with no machine chosen it takes selectSingleEnvironmentScope's, so its sentence and its page name one environment.
 */
export function settingsScopeOf(client: T3Client, route: string, machine: string, projectKeyInput: string, checkoutInput: string, legacyProjectId: string) {
  const legacyGroup = !projectKeyInput && legacyProjectId ? client.projectGroups().find(group => group.members.some(member => member.id === legacyProjectId)) : undefined;
  // A bare project id (Project settings from the palette, the sidebar, the thread menu or the details card) is its
  // project on every environment: /projects/$projectKey redirects with the key and no machine (routes/projects.$projectKey.tsx).
  const projectKey = legacyGroup ? legacyGroup.key : projectKeyInput, checkout = legacyGroup ? '' : checkoutInput;
  const chosen = resolveScope(client, machine, projectKey, checkout);
  const machineAxis = scopeMachine(client, route, machine, chosen.kind === 'unavailable' ? [] : chosen.selected);
  return { projectKey, checkout, scope: machineAxis === machine ? chosen : resolveScope(client, machineAxis, projectKey, checkout) };
}

/** SettingsScopeContext's `environment`: the scope's connected environment, the primary first (its setup links and Providers name it). */
export function scopeRepresentative(scope: ReturnType<typeof resolveScope>) {
  const connected = scope.selected.filter(environment => environment.connection.phase === 'connected' && environment.serverConfig !== null);
  return connected.find(candidate => isPrimaryEnvironment(candidate.environmentId)) ?? connected[0];
}

/**
 * The editor's draft opened or closed (a window state, no command): the window (devicePresentation) and the root's
 * `scheme` take its mode when the data source reads again, so ask it now (`t3.notify`, R10Connect.swift's wake). A wake
 * let go is made again by the next answer.
 */
const worn = new WeakMap<T3Client, string>();
async function wearMode(client: T3Client, native: Native | null | undefined, mode: string): Promise<void> {
  const was = worn.get(client);
  if (was === mode) return;
  if (was !== undefined && native?.available) {
    try { await native.later({ op: 'r10Wake', topic: 't3.notify' }); } catch (error) { if (letGo(error)) throw error; }
  }
  worn.set(client, mode);
}

export async function settingsCore(client: T3Client, native: Native | null | undefined, machine: string, projectKeyInput: string, checkoutInput: string, legacyProjectId: string, route: string, target: string, active: boolean, dialogKind = '', dialogSubject = '', deliveryStream = 'embedded', deliveryStaged = false, scheme = 'light') {
  rememberDelivery(client, deliveryStream, deliveryStaged); // settings-a-about.ts
  const { projectKey, checkout, scope } = settingsScopeOf(client, route, machine, projectKeyInput, checkoutInput, legacyProjectId);
  const prefs: ClientPrefs = (client.local as unknown as { clientSettings?: ClientPrefs }).clientSettings || decodeClientPrefs({});
  const project = scope.kind === 'project' || scope.kind === 'checkout';
  const files = active && project && (route === 'general' || route === 'projects') ? await memberFiles(client, native, scope.members, scope) : new Map<string, Obj | null>();
  const context = serverContext(client, scope, files);
  const device = client.local.deviceSettings;
  const custom = (client.local as unknown as { customThemes?: CustomTheme[] }).customThemes || [];
  // The theme editor's session (D16): the window's create/edit/duplicate dialog names it whether
  // or not Settings is open, and its draft paints the whole app (settings-appearance-editor.ts).
  // It opens on the app's resolved scheme (app.contract `scheme`; SettingsPanels.tsx and CommandPalette.tsx pass
  // useTheme's resolvedTheme), so mode System on a dark Mac opens the Dark appearance.
  const draft = syncDraft(client, EDITOR_KINDS.has(dialogKind) || active ? dialogKind : '', dialogSubject, prefs, scheme === 'dark' ? 'dark' : 'light');
  const paint = paintOf(client);
  await wearMode(client, native, paint.mode);
  if (dialogKind !== 'import') resetImport(client);
  let sections: CoreSection[] = [];
  if (active && route === 'general' && scope.kind !== 'unavailable') sections = generalSections(client, context);
  if (active && route === 'appearance') sections = appearanceSections(prefs, true);
  sections = sections.map(section => ({ ...section, rows: section.rows.map((row, index) => ({ ...row, divider: index > 0 })) }));
  // ProjectSettingsPanel renders ProjectDefaultsSettings' modelRow: the Project page's Model row is General's, in the project scope.
  const projectModel = active && route === 'projects' && project ? generalSections(client, context).flatMap(section => section.rows).filter(row => row.id === 'default-model').map(row => ({ ...row, divider: false })) : [];
  const labels = restoreLabels(client.local as never, obj(client.config.settings), client.ready);
  // SettingsScopeBoundary: a search target outside the selected scope explains its owning scope.
  const wanted = target ? searchTargetScope(target) : null;
  const notice = wanted && scope.kind !== 'unavailable' && !scopeAvailable(wanted.scope, scope.kind) ? `${wanted.title} is not available for the selected target. Choose its owning scope to continue.` : '';
  const unavailableMessage = scope.kind === 'unavailable' ? scope.message
    // Providers shows the chosen environment's own connection (providers-scope.ts); the other routes read the focused one.
    : scope.kind === 'environment' && !(singleEnvironmentRoute(route) ? scope.connected : client.ready) ? `Reconnect ${scope.environmentLabel} to change its settings.` : '';
  return {
    ready: active, route, breadcrumb: breadcrumbLabel(route), showScope: !DEVICE_ONLY.has(route), kind: scope.kind, message: unavailableMessage, notice,
    environmentLabel: scope.environmentLabel, projectLabel: scope.projectLabel, projectMark: scope.projectMark, projectInk: scope.projectInk, projectSurface: scope.projectSurface,
    connective: scope.connective, // Every known environment, each with its own machine icon; a single-environment route drops "All environments".
    environmentChoices: singleEnvironmentRoute(route) ? scope.environmentChoices.filter(choice => choice.id) : scope.environmentChoices, environmentIcon: scope.environmentIcon, projectChoices: scope.projectChoices,
    environmentKeyed: [keyedLabel(scope.environmentLabel)], projectKeyed: [keyedLabel(scope.projectLabel)],
    representative: String(scope.members[0]?.id ?? ''), scopeKey: `${machine}|${projectKey}|${checkout}`,
    // A setup link inside Settings opens Providers on this environment (ProjectDefaultsSettings.tsx:60-62, 168-173; SettingsPanels.tsx:3260-3266); '' offers none.
    scopeEnvironment: scope.kind === 'unavailable' ? '' : scopeRepresentative(scope)?.environmentId ?? '',
    sections, projectModel, restoreCount: labels.length, restoreText: labels.length ? `This will reset: ${labels.join(', ')}.` : '',
    appearanceMode: device.appearanceMode, tiles: modeTiles(device.appearanceMode, prefs, custom), themes: libraryCards(prefs, custom, removalPicks(client, dialogKind === 'remove' ? dialogSubject : '')), typographyAdvanced: prefs.typographyAdvanced, themeLight: prefs.themeLight,
    palette: palette(paint.prefs, paint.custom, paint.mode), editor: editorView(draft, custom), themeImport: importView(client), interfaceFont: fontStack(prefs.fontFamilySans, false) ?? 'system-ui', codeFont: fontStack(prefs.fontFamilyCode, true) ?? 'ui-monospace', interfaceSize: prefs.fontSizeInterface, codeSize: prefs.fontSizeCode, codePreview: fontDiffPreview(prefs.diffColorScheme),
    wordWrap: prefs.wordWrap, panelDuration: prefs.panelAnimationDurationMs,
    archiveConfirm: archiveConfirmation(client),
    updateConfirm: updateConfirmation(client),
    killConfirm: killConfirmation(client),
    hostEditor: hostEditorView(client), hostChecks: hostChecks(client),
    // Any settings-a dialog the root must mount (SettingsADialogs).
    saOpen: updateConfirmation(client) || killConfirmation(client).pid !== '' || hostEditorView(client).open,
    background: backgroundDialog(obj(client.config.settings), client.ready && !!client.environmentId && scope.kind !== 'unavailable' && scope.selected.length === 1),
  };
}
