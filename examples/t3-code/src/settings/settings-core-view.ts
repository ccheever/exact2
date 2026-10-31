// The settingsCore resource: scope sentence, General/Appearance
// sections, palette and restore state for the Contract shell in settings-core.contract.
import { obj, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import type { T3Client } from '../client/client';
import { decodeClientPrefs, dividedSection, generalSections, memberFiles, resolveScope, restoreLabels, scopeRepresentative, scopeTarget, serverContext, type ClientPrefs, type CoreSection } from './settings-core';
export { scopeRepresentative }; // providers-scope.ts, source-control-view.ts
import { appearanceSections, fontStack, modeTiles, palette, SEPARATE_FONT_TARGETS } from '../appearance/settings-appearance';
import { breadcrumbLabel, CORE_ROUTES, foldStopOf, scopeAvailable, searchTargetScope } from './settings-search';
import { scopeNoticeGroups } from './settings-scope-notice';
import type { CustomTheme } from '../appearance/settings-themes';
import { scopeMachine, singleEnvironmentRoute } from '../providers/settings-b-scope';
import { backgroundDialog } from './settings-a-background';
import { archiveConfirmation } from './settings-a-archive';
import { editorView, syncDraft } from '../appearance/settings-appearance-editor';
import { appStageLabel, paintOf, stageBackdropVariant } from '../appearance/settings-appearance-look';
import { letGo } from '../client/let-go';
import { importView, resetImport } from '../appearance/settings-appearance-import';
import { rememberDelivery, updateConfirmation } from './settings-a-about';
import { killConfirmation } from './settings-a-telemetry';
import { hostChecks, hostEditorView } from '../device/settings-a-hosts';
import { easEditorView } from '../device/settings-eas-hosts';
import { libraryCards, removalPicks } from '../appearance/settings-a-collections';
import { installedFonts } from '../appearance/appearance-fonts';
import { fontDiffPreview } from '../appearance/settings-font-previews';
import { readCliCommand } from './cli-command';
import { groupOfMember } from './projects-fleet';
import { quitMode } from './palette-native';

/** The scope sentence keys each label on its text, so a changed label remounts (a reused one-line text kept its old string). */
const keyedLabel = (label: string) => ({ id: `label:${label}`, label, secondary: '', mark: '', ink: '', surface: '', member: '', selected: true, offline: false });

// Pages whose every row is saved on this client; they have no scope sentence (SETTINGS_DEVICE_ONLY_PATHS).
const DEVICE_ONLY = new Set(['general', 'appearance', 'chat', 'diffs', 'snap-shot', 'connections', 'access', 'about']);
const EDITOR_KINDS = new Set(['create', 'edit', 'duplicate']);

/**
 * The settings shell's resolved scope (SettingsScopeContext useResolvedSettingsScope). Other routes' own scope
 * buttons pick one physical checkout, read as the checkout axis. Providers is single-environment (settings-b):
 * with no machine chosen it takes selectSingleEnvironmentScope's, so its sentence and its page name one environment.
 */
export function settingsScopeOf(client: T3Client, route: string, machine: string, projectKeyInput: string, checkoutInput: string, legacyProjectId: string) {
  const legacyGroup = !projectKeyInput && legacyProjectId ? groupOfMember(client, legacyProjectId) : undefined;
  // A bare project id (Project settings from the palette, the sidebar, the thread menu or the details card) is its
  // project on every environment: /projects/$projectKey redirects with the key and no machine (routes/projects.$projectKey.tsx).
  const projectKey = legacyGroup ? legacyGroup.key : projectKeyInput, checkout = legacyGroup ? '' : checkoutInput;
  const chosen = resolveScope(client, machine, projectKey, checkout);
  const machineAxis = scopeMachine(client, route, machine, chosen.kind === 'unavailable' ? [] : chosen.selected);
  // `machine` is the axis the scope resolved: on a single-environment route, the environment chosen when none was.
  return { machine: machineAxis, projectKey, checkout, scope: machineAxis === machine ? chosen : resolveScope(client, machineAxis, projectKey, checkout) };
}


/**
 * The editor's draft opened or closed (a window state, no command): the window (devicePresentation) and the root's
 * `scheme` take its mode when the data source reads again, so ask it now (`t3.notify`, T3LatinChords.swift's wake). A wake
 * let go is made again by the next answer.
 */
const worn = new WeakMap<T3Client, string>();
async function wearMode(client: T3Client, native: Native | null | undefined, mode: string): Promise<void> {
  const was = worn.get(client);
  if (was === mode) return;
  if (was !== undefined && native?.available) {
    try { await native.later({ op: 'wake', topic: 't3.notify' }); } catch (error) { if (letGo(error)) throw error; }
  }
  worn.set(client, mode);
}

export async function settingsCore(client: T3Client, native: Native | null | undefined, machine: string, projectKeyInput: string, checkoutInput: string, legacyProjectId: string, route: string, target: string, active: boolean, dialogKind = '', dialogSubject = '', deliveryStream = 'embedded', deliveryStaged = false, scheme = 'light') {
  rememberDelivery(client, deliveryStream, deliveryStaged); // settings-a-about.ts
  const { projectKey, checkout, scope } = settingsScopeOf(client, route, machine, projectKeyInput, checkoutInput, legacyProjectId);
  const prefs: ClientPrefs = (client.local as unknown as { clientSettings?: ClientPrefs }).clientSettings || decodeClientPrefs({});
  const project = scope.kind === 'project' || scope.kind === 'checkout';
  // useMemberProjectFiles: a project scope reads its members' t3.json on every route, so Restore defaults lists what the scope's
  // target resolves to (useScopedSettings), file tier included.
  const files = active && project ? await memberFiles(client, native, scope.members, scope) : new Map<string, Obj | null>();
  const context = serverContext(client, scope, files);
  const device = client.local.deviceSettings;
  const custom = (client.local as unknown as { customThemes?: CustomTheme[] }).customThemes || [];
  // The theme editor's session: the window's create/edit/duplicate dialog names it whether
  // or not Settings is open, and its draft paints the whole app (settings-appearance-editor.ts).
  // It opens on the app's resolved scheme (app.contract `scheme`; SettingsPanels.tsx and CommandPalette.tsx pass
  // useTheme's resolvedTheme), so mode System on a dark Mac opens the Dark appearance.
  const draft = syncDraft(client, EDITOR_KINDS.has(dialogKind) || active ? dialogKind : '', dialogSubject, prefs, scheme === 'dark' ? 'dark' : 'light');
  const paint = paintOf(client);
  await wearMode(client, native, paint.mode);
  if (dialogKind !== 'import') resetImport(client);
  let sections: CoreSection[] = [];
  // SettingsScopeBoundary renders a device-only page whatever the scope: its rows are this client's.
  const deviceOnly = DEVICE_ONLY.has(route);
  // Worktrees' rows are the storage panel's to draw, between its own (settings-data.ts worktreeRows).
  if (active && CORE_ROUTES.has(route) && route !== 'appearance' && route !== 'worktrees' && (scope.kind !== 'unavailable' || deviceOnly)) sections = generalSections(client, context, route);
  if (active && route === 'appearance') sections = appearanceSections(prefs, stageBackdropVariant(appStageLabel()) !== null, await installedFonts(client, native), target);
  const cli = active && route === 'about' ? await readCliCommand(client, native) : null;
  if (cli) sections = sections.length ? sections.map(section => section.id === 'about' ? { ...section, rows: [...section.rows, cli] } : section)
    : [{ id: 'about', title: 'About', collapsible: false, toggle: false, rows: [cli] }];
  sections = sections.map(dividedSection);
  // ProjectSettingsPanel renders ProjectDefaultsSettings' modelRow: the Project page's Model row is General's, in the project scope.
  const projectModel = active && route === 'projects' && project ? generalSections(client, context).flatMap(section => section.rows).filter(row => row.id === 'default-model').map(row => ({ ...row, divider: false })) : [];
  // useSettingsRestore over useScopedSettings: the scope's representative target, not the focused environment.
  const scoped = scopeTarget(client, context);
  const labels = restoreLabels(client.local as never, scoped ? scoped.settings : null);
  // SettingsScopeBoundary: a search target outside the selected scope explains its owning scope.
  const wanted = target ? searchTargetScope(target) : null;
  const notice = wanted && scope.kind !== 'unavailable' && !scopeAvailable(wanted.scope, scope.kind) ? `${wanted.title} is not available for the selected target. Choose its owning scope to continue.` : '';
  // SettingsScopeBoundary's SettingsScopeNotice: the owning scope's targets ("all" for every other requirement).
  const noticeGroups = notice ? scopeNoticeGroups(wanted!.scope === 'environment' || wanted!.scope === 'project' ? wanted!.scope : 'all', scope.groups, scope.environments) : [];
  const unavailableMessage = scope.kind === 'unavailable' ? scope.message
    // Providers shows the chosen environment's own connection (providers-scope.ts); the other routes read the focused one.
    : scope.kind === 'environment' && !(singleEnvironmentRoute(route) ? scope.connected : client.ready) ? `Reconnect ${scope.environmentLabel} to change its settings.` : '';
  // A jump to a row that exists only with Separate fonts on asks for the setting (shell-members.contract writes it) and
  // waits for it: the answer names the jump only once the setting reads on, as revealSettingsSearchTarget writes it first.
  const separateFontsWanted = active && SEPARATE_FONT_TARGETS.includes(target) && !prefs.typographyAdvanced;
  return {
    ready: active, route, breadcrumb: breadcrumbLabel(route),
    // The search jump this answer is for, and the reveal stop of the fold its row sits under (settings-members.contract).
    jumpTarget: separateFontsWanted ? '' : target, jumpStop: foldStopOf(target), separateFontsWanted, showScope: !deviceOnly, kind: scope.kind, message: deviceOnly ? '' : unavailableMessage, notice, noticeGroups,
    environmentLabel: scope.environmentLabel, projectLabel: scope.projectLabel, projectMark: scope.projectMark, projectInk: scope.projectInk, projectSurface: scope.projectSurface,
    connective: scope.connective, // Every known environment, each with its own machine icon; a single-environment route drops "All environments".
    environmentChoices: singleEnvironmentRoute(route) ? scope.environmentChoices.filter(choice => choice.id) : scope.environmentChoices, environmentIcon: scope.environmentIcon, projectChoices: scope.projectChoices,
    environmentKeyed: [keyedLabel(scope.environmentLabel)], projectKeyed: [keyedLabel(scope.projectLabel)],
    representative: String(scope.members[0]?.id ?? ''), scopeKey: `${machine}|${projectKey}|${checkout}`,
    // ProjectsSettings: an environment or checkout the project is not on offers "Show all environments" (selectEnvironmentAxis
    // to All, which keeps the project and drops the checkout) instead of a dead end.
    showAllEnvironments: route === 'projects' && scope.kind === 'unavailable' && (machine !== '' || checkout !== ''), allEnvironmentsProject: projectKey,
    allEnvironmentsMember: String(scope.groups.find(group => group.projectKey === projectKey)?.memberProjects[0]?.id ?? ''),
    // A setup link inside Settings opens Providers on this environment (ProjectDefaultsSettings.tsx:60-62, 168-173; SettingsPanels.tsx:3260-3266); '' offers none.
    scopeEnvironment: scope.kind === 'unavailable' ? '' : scopeRepresentative(scope)?.environmentId ?? '',
    sections, cliSections: cli ? [{ id: 'about', title: 'About', collapsible: false, toggle: false, rows: [cli], advanced: [] }] : [], projectModel, restoreCount: labels.length, restoreText: labels.length ? `This will reset: ${labels.join(', ')}.` : '',
    appearanceMode: device.appearanceMode, tiles: modeTiles(device.appearanceMode, prefs, custom), themes: libraryCards(prefs, custom, removalPicks(client, dialogKind === 'remove' ? dialogSubject : '')), typographyAdvanced: prefs.typographyAdvanced, themeLight: prefs.themeLight,
    palette: palette(paint.prefs, paint.custom, paint.mode), editor: editorView(draft, custom), themeImport: importView(client), interfaceFont: fontStack(prefs.fontFamilySans, false) ?? 'system-ui', codeFont: fontStack(prefs.fontFamilyCode, true) ?? 'ui-monospace', interfaceSize: prefs.fontSizeInterface, codeSize: prefs.fontSizeCode, codePreview: fontDiffPreview(prefs.diffColorScheme),
    wordWrap: prefs.wordWrap, panelDuration: prefs.panelAnimationDurationMs, quitMode: quitMode(client.local),
    archiveConfirm: archiveConfirmation(client),
    updateConfirm: updateConfirmation(client),
    killConfirm: killConfirmation(client),
    hostEditor: hostEditorView(client), hostChecks: hostChecks(client), easEditor: easEditorView(client),
    // Any settings-a dialog the root must mount (SettingsADialogs).
    saOpen: updateConfirmation(client) || killConfirmation(client).pid !== '' || hostEditorView(client).open || easEditorView(client).open,
    background: backgroundDialog(obj(client.config.settings), client.ready && !!client.environmentId && scope.kind !== 'unavailable' && scope.selected.length === 1),
  };
}
