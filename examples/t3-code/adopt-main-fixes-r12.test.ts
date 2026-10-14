// adopt-main-fixes-r12: what round 12 adapts to main and the workarounds its fixes end. Each reads the clone's sources;
// the behaviour is proven by the record's agent drives (tasks/20261013-adopt-main-fixes-r12.md).
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';

const dir = new URL('./', import.meta.url);
const contracts = async () => Promise.all(readdirSync(dir).filter(name => name.endsWith('.contract'))
  .map(async name => ({ name, lines: (await Bun.file(new URL(name, dir)).text()).split('\n') })));
const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
const code = (line: string) => !line.trimStart().startsWith('//');

describe('AD-1: the Mac\'s own presentations main gives what the clone writes stay the reference\'s', () => {
  test('every vertical list opts out of the Mac\'s table (LLP 1115.002 D5): the reference draws its rows as divs', async () => {
    const lists = (await contracts()).flatMap(({ name, lines }) => lines.filter(line => code(line) && /^\s*list\s/.test(line)).map(line => ({ name, line })));
    expect(lists.map(({ name }) => name).sort()).toEqual(['app-main.contract', 'connections-routes.contract', 'diff.contract', 'legacy-sidebar.contract', 'pages-pr-code.contract']);
    for (const { line } of lists) expect(line).toMatch(/^\s*list appearance="none" /);
  });

  test('every popover keeps the painted layer (LLP 1115.002 D7.3): `popover="auto"` and an authored fallback row', async () => {
    const popovers = (await contracts()).flatMap(({ lines }) => lines.filter(line => code(line) && /\bpopover="/.test(line)));
    expect(popovers.length).toBe(62);
    for (const line of popovers) {
      expect(line).toContain('popover="auto"');
      expect(line).toMatch(/\sposition-try-fallbacks=/);
    }
  });
});

// css-workaround-retirements KW-6 (X17): each popover's `position-area` and `position-try-fallbacks` follow its
// reference caller (apps/web/src/components at 1e2ecbd975). Base UI flips a popup's side near an edge and flips its
// alignment by default (the ticket and the app's records; Base UI's source is not installed here). Its sideOffset is a
// margin on the invoker side, which a flip exchanges. `none` stays where the reference does not place a Base UI popup.
const FLIP = '"flip-block, flip-inline, flip-block flip-inline"';
const q = (text: string) => `"${text}"`;
// [owner, id as written, position-area as written ('' = no row), position-try-fallbacks as written, reference caller]
type Site = [owner: string, id: string, area: string, fallbacks: string, reference: string];
const reviewed: Record<string, Site[]> = {
  'auto-balance.contract': [['NoticeMachines', 'id=`notice-machines-${notice.id}`', q('top span-right'), FLIP, 'chat/useAutoBalanceUpdateBanner.tsx:106 top/start']],
  'browser-stage.contract': [['BrowserPresetSelect', 'id="browser-preset-menu"', q('bottom span-right'), FLIP, 'browser/BrowserDeviceToolbar.tsx:197 Select start']],
  'browser-surface.contract': [
    ['BrowserMore', 'id="browser-more-menu"', q('bottom span-left'), FLIP, 'preview/PreviewMoreMenu.tsx:111 end, offset 6'],
    ['BrowserAppearanceItem', 'id="browser-appearance-menu"', q('right span-bottom'), q('flip-inline'), 'preview/PreviewMoreMenu.tsx:128 MenuSubPopup'],
    ['BrowserAddItem', 'id="r4-browser-profiles"', q('right span-bottom'), q('flip-inline'), 'RightPanelTabs.tsx:1275 MenuSubPopup'],
    ['BrowserLauncherProfiles', 'id="launcher-browser-profiles"', q('bottom span-left'), FLIP, 'RightPanelTabs.tsx:539 end, offset 6'],
  ],
  'diff.contract': [['DiffBasePicker', 'id="diff-base-menu"', q('bottom span-right'), FLIP, 'DiffPanel.tsx:753 Combobox start']],
  'legacy-sidebar.contract': [['LegacyOptions', 'id="legacy-sidebar-options"', q('bottom span-left'), FLIP, 'LegacySidebar.tsx:2761 end']],
  'markdown.contract': [['TerminalContextChip', 'id=popupId', q('top'), q('flip-block'), 'contextChipParts.tsx:88 top/center']],
  'pages-hero.contract': [['HeroPicker', 'id="hero-project-menu"', q('bottom'), q('flip-block'), 'chat/DraftHeroHeadline.tsx:252 center']],
  'pages-pr-actions.contract': [
    ['PrdActionsMenu', 'id="pr-more-menu"', q('bottom span-left'), FLIP, 'pullRequest/PullRequestDetailPanel.tsx:1983 end'],
    ['PaFreshnessMark', 'id="pr-freshness"', q('bottom span-right'), FLIP, 'pullRequest/PullRequestDetailPanel.tsx:384 start'],
  ],
  'pages-pr-code.contract': [['PrdCodeToolbar', 'id=menuId', q('bottom span-right'), FLIP, 'pullRequest/PullRequestCodeTab.tsx:1077 start']],
  'pages-pr-compose.contract': [['PrdComposer', 'id=popId', q('top span-left'), FLIP, 'pullRequest/PullRequestComposer.tsx:102 top/end, offset 8']],
  'pages-pr-handoffs.contract': [['PrdCheckoutMenu', 'id="pr-checkout-menu"', q('bottom span-left'), FLIP, 'pullRequest/PullRequestDetailPanel.tsx:1530 end']],
  'pages-pr-meta.contract': [
    ['PrdReactionBar', 'id=pickerId', q('top span-right'), FLIP, 'pullRequest/PullRequestReactions.tsx:137 top/start'],
    ['PrdCandidatePicker', 'id=menuId', q('bottom span-right'), FLIP, 'pullRequest/PullRequestCandidatePicker.tsx:124 start'],
  ],
  'pages-pr-quick.contract': [
    ['PrRowChecks', 'id=popId', q('bottom span-right'), FLIP, 'pullRequest/PullRequestChecksPopover.tsx:197 start'],
    ['PrRowStack', 'id=popId', q('bottom span-right'), FLIP, 'pullRequest/PullRequestStackPopover.tsx:86 start'],
  ],
  'pages-pr-stack.contract': [['PrdStackControls', 'id="pr-stack-menu"', q('bottom span-right'), FLIP, 'pullRequest/PullRequestStackMenu.tsx:148 start']],
  'pages-prs.contract': [
    ['PrSortMenu', 'id="pr-sort-menu"', q('bottom span-right'), FLIP, 'routes/_chat.pull-requests.tsx:2281 start'],
    ['PrHostMenu', 'id="pr-host-menu"', q('bottom span-right'), FLIP, 'routes/_chat.pull-requests.tsx:2281 start'],
    ['PrFiltersMenu', 'id="pr-filters-menu"', q('bottom span-left'), FLIP, 'pullRequest/PullRequestListFilters.tsx:511 end'],
  ],
  'pages-usage-prices.contract': [['UsagePricesApply', 'id="usage-prices-apply"', q('bottom span-right'), FLIP, 'usage/UsagePriceOverrides.tsx:310 start']],
  'pages-usage.contract': [
    ['UsageBreadcrumb', 'id="usage-environment-menu"', q('bottom span-right'), FLIP, 'usage/UsagePage.tsx:1142 start'],
    ['UsageSelect', 'id=menuId', q('bottom span-left'), FLIP, 'usage/UsagePage.tsx:414 Select end'],
  ],
  // UpkeepSelect: each caller names its area (below); a default SelectPopup is item-aligned, which has no area.
  'providers-upkeep.contract': [
    ['UpkeepSelect', 'id=`${selectId}-menu`', 'area', '(area == "none" ? "none" : "flip-block, flip-inline, flip-block flip-inline")', 'settings/CustomModelEditor.tsx, AcpSessionManagementSection.tsx'],
    ['ProviderVersionAdvisory', 'id=advisory.popId', q('bottom span-left'), FLIP, 'settings/ProviderInstanceCard.tsx:799 end'],
  ],
  'r4-git.contract': [
    ['R4BranchPicker', 'id="details-branch-menu"', q('bottom span-right'), FLIP, 'BranchToolbarBranchSelector.tsx:684 panel start'],
    ['R4GitMenu', 'id="details-git-options"', q('bottom span-left'), FLIP, 'GitActionsControl.tsx:1916 end'],
    ['R4EnvRow', 'id="details-environment-menu"', q('bottom span-right'), FLIP, 'BranchToolbarEnvironmentSelector.tsx:135 Select start'],
  ],
  // The tab menu is the system's context menu in the reference (api.contextMenu.show), at the pointer: no area.
  'r4-surfaces.contract': [
    ['R4HeaderBar', 'id="r4-tab-menu"', '', q('none'), 'RightPanelTabs.tsx handleTabContextMenu (native menu)'],
    ['R4AddSurface', 'id="r4-add-surface"', q('bottom span-right'), FLIP, 'RightPanelTabs.tsx:1231 start, offset 6'],
  ],
  'r6-pr.contract': [['R6ChecksPopover', 'id=row.r6.popId', q('bottom span-right'), FLIP, 'pullRequest/PullRequestChecksPopover.tsx:197 start']],
  'settings-a-fonts.contract': [['FontFamilyPicker', 'id=`font-picker-${rowId}`', q('bottom span-left'), FLIP, 'settings/FontFamilyPicker.tsx:219 end']],
  'settings-a-hosts.contract': [['HostRowView', 'id=`device-host-tools-${host.id}`', q('bottom span-left'), FLIP, 'device/DeviceToolVersions.tsx:48 end']],
  'settings-appearance-import.contract': [['AddThemeDialog', 'id="add-theme-sort-menu"', q('bottom span-left'), FLIP, 'settings/ThemeSearchSection.tsx:324 Select end']],
  'settings-b-accent.contract': [['ProviderAccentPicker', 'id=`provider-accent-${editorId}`', q('bottom span-right'), FLIP, 'settings/ProviderAccentColorPicker.tsx:94 start, offset 6']],
  'settings-b-actions.contract': [['ProjectActionEditor', 'id="action-icon-picker"', q('bottom span-right'), FLIP, 'projectScriptEditor.tsx:329 start']],
  // CnMenu: each caller names its area (below).
  'settings-b-kit.contract': [['CnMenu', 'id=menuId', 'area', '(area == "none" ? "none" : (sub ? "flip-inline" : "flip-block, flip-inline, flip-block flip-inline"))', 'ui/menu.tsx MenuPopup, MenuSubPopup']],
  'settings-keybindings.contract': [['WhenEditor', 'id="keybinding-when-editor"', q('bottom span-right'), FLIP, 'settings/KeybindingsSettings.tsx:903 start, offset 6']],
  'settings-kit.contract': [['SkPopup', 'id=menuId', '(endAlign > 0 ? "bottom span-left" : "bottom span-right")', FLIP, 'ui/select.tsx, ui/menu.tsx (each caller\'s align)']],
  'settings-mobile-beta.contract': [
    ['MobileBetaControl', 'id="mobile-beta-ios-popup"', q('bottom span-left'), FLIP, 'NightlyMobileBeta.tsx:118 end'],
    ['MobileBetaControl', 'id="mobile-beta-android-popup"', q('bottom span-left'), FLIP, 'NightlyMobileBeta.tsx:133 end'],
  ],
  'settings-rows.contract': [
    ['ScopeMenu', 'id=menuId', q('bottom span-right'), FLIP, 'settings/SettingsScopeSentence.tsx:105 start'],
    ['CoreRowView', 'id=`inherit-${section}-${row.id}`', q('bottom span-right'), FLIP, 'settings/SettingInheritance.tsx:226 start'],
    ['CoreMenu', 'id=menuId', q('bottom span-left'), FLIP, 'settings/SettingsPanels.tsx Select end'],
    ['TraitsMenu', 'id=menuId', q('bottom span-right'), FLIP, 'chat/TraitsPicker.tsx:700 start'],
  ],
  'settings-scheduled.contract': [['ScheduledEditor', 'id="task-branch-menu"', q('bottom span-right'), FLIP, 'WorktreeBaseBranchPicker.tsx:100 start']],
  'settings-source-control.contract': [
    ['Inheritance', 'id=`inherit-${row.key}`', q('bottom span-right'), FLIP, 'settings/SettingInheritance.tsx:226 start'],
    ['DeviceVersions', 'id=`device-versions-${tool.kind}`', q('bottom span-left'), FLIP, 'device/DeviceToolVersions.tsx:48 end'],
  ],
  'shell-details.contract': [['WorkspaceMenu', 'id="details-workspace-menu"', q('bottom span-right'), FLIP, 'BranchToolbarEnvModeSelector.tsx:230 Select start']],
  // The thread and draft menus are the system's context menu in the reference, at the pointer: no area. Their painted
  // submenus (agent, web) sit beside the row and flip to its start side, as the Appearance submenu does.
  'sidebar-row.contract': [
    ['ThreadRow', 'id=`snooze-${t.id}`', q('bottom span-left'), FLIP, 'Sidebar.tsx:580 end'],
    ['ThreadMenu', 'id="thread-menu"', '', q('none'), 'handleThreadContextMenu (native menu)'],
    ['ThreadMenu', 'id=`thread-menu-sub-${p.id}`', q('right span-bottom'), q('flip-inline'), 'native submenu'],
    ['DraftMenu', 'id="draft-menu"', '', q('none'), 'handleDraftContextMenu (native menu)'],
    ['DraftMenu', 'id=`draft-menu-sub-${p.id}`', q('right span-bottom'), q('flip-inline'), 'native submenu'],
  ],
  'snapshot.contract': [
    ['SnapshotPanel', 'id="snapshot-sound-menu"', q('bottom span-left'), FLIP, 'settings/SnapShotSettings.tsx:509 end'],
    ['SnapshotTile', 'id=`snapshot-contents-${image.id}`', q('top'), q('flip-block'), 'chat/SnapShotAttachmentDetails.tsx:166 top/center'],
  ],
  'snooze-calendar.contract': [['SnoozeDateField', 'id="snooze-calendar"', q('bottom span-right'), FLIP, 'CustomSnoozeDialog.tsx:133 start']],
  'theme-color-picker.contract': [['ThemeColorSwatch', 'id=`theme-color-${row.id}`', q('bottom span-left'), FLIP, 'settings/ThemeColorPicker.tsx:285 end, offset 10']],
  'timeline-plan.contract': [['PlanCard', 'id=`plan-menu-${message.id}`', q('bottom span-left'), FLIP, 'chat/ProposedPlanCard.tsx:163 end']],
};

describe('KW-6: collision placement follows each reference caller', () => {
  test('the reviewed table names every popover, in source order, with its area and fallback order', async () => {
    const found = (await contracts()).map(({ name, lines }) => ({ name, popovers: lines.filter(line => code(line) && /\bpopover="/.test(line)) }))
      .filter(({ popovers }) => popovers.length > 0);
    expect(found.map(({ name }) => name).sort()).toEqual(Object.keys(reviewed).sort());
    for (const { name, popovers } of found) {
      const sites = reviewed[name]!;
      expect(popovers.length).toBe(sites.length);
      sites.forEach(([owner, id, area, fallbacks], index) => {
        const line = popovers[index]!;
        expect(`${owner}: ${line}`).toContain(` ${id} `);
        if (area === '') expect(line).not.toMatch(/\sposition-area=/);
        else expect(line).toContain(` position-area=${area} `);
        expect(line).toContain(` position-try-fallbacks=${fallbacks} `);
      });
    }
  });

  test('a shared popup takes its area from each caller: CnMenu, UpkeepSelect and SkPopup', async () => {
    const callers: [file: string, call: string, area: string][] = [
      ['connections.contract', 'CnMenu(menuId="this-machine-menu-popup"', 'bottom span-left'], // ConnectionsSettings.tsx:3459 end
      ['connections.contract', 'CnMenu(menuId=`environment-menu-', 'bottom span-left'], // ConnectionsSettings.tsx:1705 end
      ['connections.contract', 'CnMenu(menuId=`environment-icon-', 'right span-bottom'], // EnvironmentIconPicker.tsx:102 MenuSubPopup
      ['providers-setup.contract', 'CnMenu(menuId=`${idPrefix}provider-auth-method-', 'none'], // ProviderAuthenticationSection.tsx:203 item-aligned
      ['settings-b-kit.contract', 'CnMenu(menuId=`${selectId}-menu`', 'bottom span-left'], // LoadBalancing/GitHubRouting Select end
      ['settings-projects.contract', 'CnMenu(menuId="import-scripts-menu"', 'bottom span-left'], // ProjectActionsSettings.tsx:159 end
      ['providers-upkeep.contract', 'UpkeepSelect(selectId=`acp-project-', 'none'], // AcpSessionManagementSection.tsx:328 item-aligned
      ['providers-upkeep.contract', 'UpkeepSelect(selectId=`acp-provider-project-', 'none'], // :459 item-aligned
      ['providers-upkeep.contract', 'UpkeepSelect(selectId=`acp-provider-protocol-', 'none'], // :533 item-aligned
      ['providers-upkeep.contract', 'UpkeepSelect(selectId=`custom-model-copy-', 'bottom span-left'], // CustomModelEditor.tsx:331 end
      ['providers-upkeep.contract', 'UpkeepSelect(selectId=`custom-model-option-id-', 'bottom span-right'], // :215 start
      ['providers-upkeep.contract', 'UpkeepSelect(selectId=`custom-model-option-type-', 'bottom span-left'], // :257 end
    ];
    for (const [file, call, area] of callers) {
      const lines = (await source(file)).split('\n').filter(line => line.includes(call));
      expect(`${file} ${call}: ${lines.length}`).toBe(`${file} ${call}: 1`);
      expect(lines[0]).toContain(`area="${area}"`);
    }
    // A MenuSubPopup keeps its -5 px align offset; every other area keeps the 4 pt side offset.
    expect(await source('settings-b-kit.contract')).toContain('margin-top=(sub ? "-0.3125rem" : "0.25rem")');
    // SkPopup: endAlign > 0 is the end-aligned menu (the callers still pass the invoker's width).
    const sk = (await contracts()).flatMap(({ lines }) => lines.filter(line => /^\s*SkPopup\(/.test(line)));
    expect(sk.length).toBe(10);
    for (const line of sk) expect(line).toMatch(/ endAlign=(0|[1-9][0-9]*|width), /);
  });

  test('the geometry that faked an end alignment, a side or a flip is gone', async () => {
    const popovers = (await contracts()).flatMap(({ lines }) => lines.filter(line => code(line) && /\bpopover="/.test(line)));
    for (const line of popovers) {
      // A transparent tail is not a margin: it would stay on the far side after a flip.
      expect(line).not.toMatch(/ padding-(top|bottom)="0\.(25|5)rem"/);
      expect(line).not.toMatch(/ (top|left|right)=(0|"100%") /);
      // The only remaining horizontal margin carries the chevron's 2 pt inset (r4-browser-profiles).
      if (line.includes('margin-left=')) expect(line).toContain('id="r4-browser-profiles" ');
    }
    const retired: [file: string, text: string][] = [
      ['settings-kit.contract', 'endAlign - width'],
      ['settings-b-kit.contract', 'offsetX'],
      ['settings-b-kit.contract', 'endAlign'],
      ['connections.contract', 'offsetY=-37'],
      ['pages-pr-handoffs.contract', '(narrow ? -362 : -299)'],
      ['providers-upkeep.contract', '-18.75rem'],
      ['browser-surface.contract', 'margin-left="-16rem"'],
      ['browser-surface.contract', 'margin-left="-12.5rem"'],
      ['browser-surface.contract', 'margin-left="11.75rem"'],
      ['browser-stage.contract', 'margin-top="1.75rem"'],
      ['timeline-plan.contract', 'margin-left="-11rem"'],
      ['settings-source-control.contract', 'tool.offset'],
      ['theme-color-picker.contract', 'state above'],
      ['theme-color-picker.contract', '(above ?'],
      ['pages-prs.contract', 'width=tail'],
      ['pages-prs.contract', 'filtersRight'],
      ['legacy-sidebar.contract', 'width=tail'],
      ['settings-mobile-beta.contract', 'Tail'],
      ['pages-pr-compose.contract', 'align-items="flex-end" testId=`${scope}-composer-layer`'],
    ];
    for (const [file, text] of retired) expect(`${file}: ${(await source(file)).includes(text)}`).toBe(`${file}: false`);
    expect(await source('settings-a-integrations.ts')).not.toContain('offset:');
  });

  test('the hover layer keeps its best-fit flip (hoverFlip): CSS\'s ordered fallbacks do not cover hover-opened layers yet', async () => {
    expect(await source('hover-layer.contract')).toContain('hoverFlip');
  });
});

describe('RT-3: workarounds main\'s fixes end', () => {
  test('X70 (main 9074a745b): the update pill is aria-disabled, as SidebarUpdatePill.tsx, so Tab reaches it', async () => {
    const pill = (await source('sidebar-icons.contract')).split('\n').find(line => line.includes('aria-label="Check for updates"'))!;
    expect(pill).toContain('button aria-disabled=true aria-label="Check for updates" cursor="not-allowed"');
    expect(/(?<![-\w])disabled=/.test(pill)).toBe(false); // HTML disabled takes no focus
  });

  test('#287 (main ff2251b0c): the "+N" toggle\'s underline is dotted, as ConnectionsSettings.tsx\'s border-b border-dotted', async () => {
    const toggle = (await source('connections-network.contract')).split('\n').find(line => line.includes('testId="network-access-endpoints-toggle"'))!;
    expect(toggle).toContain('border-bottom-width=1 border-bottom-style="dotted"');
  });

  // Review round 1: the reference's other `border-dashed` boxes the clone draws (each was `solid`). The reference's
  // unresolved chip and pending review card keep the clone's stroked rects (listed in EXACT2-GAPS.md's #287 row).
  const dashed: [file: string, anchor: string, reference: string][] = [
    ['settings-appearance-import.contract', 'testId="add-theme-empty"', 'ThemeSearchSection.tsx:359'],
    ['settings-appearance-import.contract', 'testId="add-theme-file"', 'ThemeImportDialog.tsx:520'],
    ['providers-wizard.contract', 'text "No compatible agents found"', 'AcpRegistrySearchStep.tsx:198'],
    ['settings-a-telemetry.contract', 'text "Desktop host signals not connected"', 'ResourceTelemetryDiagnostics.tsx:1151'],
    ['settings-keybindings.contract', 'if view.lines.length == 0 and view.error == ""', 'KeybindingsSettings.tsx:687'],
  ];
  for (const [file, anchor, reference] of dashed) {
    test(`#287: the box at ${reference} is dashed, as the reference's border-dashed (${file})`, async () => {
      const lines = (await source(file)).split('\n');
      const at = lines.findIndex(line => line.includes(anchor));
      expect(at).toBeGreaterThanOrEqual(0);
      // The anchor is the box itself (a testId) or the line right before (its `when`) or after (its first text) it.
      const box = anchor.startsWith('testId=') ? lines[at] : anchor.startsWith('if ') ? lines[at + 1] : lines[at - 1];
      expect(box).toContain('border-style="dashed"');
      expect(box).not.toContain('border-style="solid"');
    });
  }
});

describe('Main\'s new warning lower-width-padding (LLP 1116.009 D5)', () => {
  test('the pull request list\'s error notes (width 100%, 0.75rem padding each side) are border-box, as Tailwind\'s', async () => {
    const lines = (await source('pages-prs.contract')).split('\n');
    const at = lines.findIndex(line => line.includes('for note in page.errors key=note.key'));
    expect(at).toBeGreaterThan(0);
    const box = lines[at - 1];
    expect(box).toContain('width="100%"');
    expect(box).toContain('padding-left="0.75rem" padding-right="0.75rem"');
    expect(box).toContain('box-sizing="border-box"');
  });
});

describe('css-workaround-retirements KW-1: balanced tips and empty states, authored placeholder colours', () => {
  const indent = (line: string) => line.length - line.trimStart().length;
  // The colour of the `::placeholder` part directly under the line at `at` (a style or a field), or ''.
  const partColour = (lines: string[], at: number) => {
    const base = indent(lines[at]!);
    for (let i = at + 1; i < lines.length; i++) {
      const line = lines[i]!;
      if (line.trim() === '' || !code(line)) continue;
      if (indent(line) <= base) return '';
      if (line.trim() === '::placeholder') return (lines[i + 1] ?? '').trim().replace(/^color=/, '');
    }
    return '';
  };
  const styleParts = async () => new Map((await contracts()).flatMap(({ lines }) => lines
    .map((line, at) => [line.match(/^style (\w+)/)?.[1] ?? '', partColour(lines, at)] as const)
    .filter(([name, colour]) => name !== '' && colour !== '')));
  // Every line that names a placeholder: a text field's own, or a prop handed to a component that draws one.
  const placeholderLines = async () => (await contracts()).flatMap(({ name, lines }) => lines
    .map((line, at) => ({ name, lines, at, line }))
    .filter(({ line }) => code(line) && /\bplaceholder=/.test(line)));
  const field = (line: string) => /^\s*(input|textarea)\s/.test(line);
  // SidebarInput's two fields keep an empty native placeholder: "Search" is painted beside them in the sidebar's muted role.
  const painted = [['sidebar.contract', 'aria-label="Search threads"'], ['settings-core.contract', 'aria-label="Search settings"']];
  const MUTED = '"light-dark(#71717b, #818181)"';

  test('every text field with a placeholder gets a colour from its own ::placeholder part or its class\'s', async () => {
    const parts = await styleParts();
    const fields = (await placeholderLines()).filter(({ line }) => field(line));
    expect(fields.length).toBeGreaterThanOrEqual(79);
    const unpainted = fields.filter(({ name, line }) => !painted.some(([file, label]) => name === file && line.includes(label!)));
    expect(fields.length - unpainted.length).toBe(painted.length);
    for (const { name, lines, at, line } of unpainted) {
      const own = partColour(lines, at);
      const cls = line.match(/\bclass=(\w+)/)?.[1] ?? '';
      expect({ field: `${name}:${at + 1}`, colour: own || parts.get(cls) || '' }).not.toEqual({ field: `${name}:${at + 1}`, colour: '' });
    }
  });

  test('every other placeholder line forwards it to a component that owns a field or paints the words', async () => {
    const owners = ['ApprovalComposer', 'RedactedText', 'CodexSignedIn', 'CodexPathRow', 'PrdWords', 'UsagePriceInput', 'R7DevField',
      'CoreField', 'ScopedTextField', 'ScopedTextArea', 'UsageRedacted', 'PoolEmail'];
    const calls = (await placeholderLines()).filter(({ line }) => !field(line));
    expect(calls.length).toBeGreaterThanOrEqual(29);
    for (const { line } of calls) expect(owners).toContain(line.trim().match(/^(\w+)\(/)?.[1] ?? line.trim());
  });

  test('the reference\'s roles: generic muted, the Git publish path at 60%, theme JSON and citation muted, the palette\'s muted', async () => {
    const parts = await styleParts();
    const own = async (file: string, anchor: string) => {
      const lines = (await source(file)).split('\n');
      const at = lines.findIndex(line => code(line) && line.includes(anchor));
      expect(at).toBeGreaterThanOrEqual(0);
      return partColour(lines, at);
    };
    // AeField also styles a row and a button, where a part is refused (lower-attr-tag): its two inputs hold their own.
    for (const style of ['FieldPlaceholder', 'PvField', 'SbField', 'TkField']) expect(parts.get(style)).toBe(MUTED);
    expect(parts.has('AeField')).toBe(false);
    expect(await own('settings-b-actions.contract', 'testId="action-name"')).toBe(MUTED);
    expect(await own('settings-b-actions.contract', 'testId="action-preview-url"')).toBe(MUTED);
    expect(await own('r4-git-publish.contract', 'placeholder=git.publishPlaceholder')).toBe('"light-dark(#71717b99, #81818199)"');
    expect(await own('settings-appearance-import.contract', 'testId="add-theme-json"')).toBe('pal.muted');
    expect(await own('citation-comment.contract', 'testId="citation-comment-text"')).toBe(MUTED);
    // A field whose words follow the Settings palette takes the palette's muted foreground, so custom themes and contrast reach it.
    const themed = (await placeholderLines()).filter(({ line }) => field(line) && line.includes('color=pal.text'));
    expect(themed.length).toBeGreaterThanOrEqual(7);
    for (const { lines, at } of themed) expect(partColour(lines, at)).toBe('pal.muted');
  });

  test('SidebarInput\'s painted "Search" labels keep the sidebar\'s muted role', async () => {
    const sidebar = await source('sidebar.contract');
    expect(sidebar).toMatch(/text "Search" [^\n]*color="light-dark\(#71717b, #a3a3a3\)"/);
    const core = await source('settings-core.contract');
    expect(core).toMatch(/text "Search" [^\n]*color=pal\.sidebarMuted[^\n]*testId="settings-search-placeholder"/);
  });

  test('an empty Select value is drawn in the placeholder colour (ui/select.tsx data-placeholder:text-placeholder)', async () => {
    const kit = await source('settings-kit.contract');
    expect(kit).toContain('text valueLabel font-size="0.875rem" line-height="1.25rem" class=SkInk color=(empty ? "light-dark(#71717b, #818181)" : "light-dark(#27272a, #f5f5f5)")');
    expect(await source('settings-keybindings.contract')).toContain('valueLabel=(draftCommand == "" ? "Command" : data.draftCommandLabel), empty=(draftCommand == "")');
    expect(await source('settings-scheduled.contract')).toContain('projectLabel(data.projects, chosenProject)), empty=(chosenProject == "")');
  });

  // [file, the line that starts the search, the owner's own line after it, the reference's text-balance owner]
  const balanced: [string, string, RegExp, string][] = [
    ['shell-tip.contract', 'component TipCard', /^\s+column /, 'ui/tooltip.tsx:112 TooltipPopup'],
    ['hover-layer.contract', 'component HoverText', /^\s+column /, 'ui/tooltip.tsx:112 TooltipPopup'],
    ['hover-layer.contract', 'component HoverCode', /^\s+column /, 'ui/tooltip.tsx:112 TooltipPopup variant code'],
    ['hover-layer.contract', 'component HoverScopes', /^\s+column /, 'ConnectionsSettings.tsx:296 tooltipStyle'],
    ['media-actions.contract', 'component MediaTip', /testId="media-tooltip"/, 'ui/tooltip.tsx:112 TooltipPopup variant code'],
    ['settings-b-kit.contract', 'component CnTooltip', /^\s+column /, 'ui/tooltip.tsx:112 TooltipPopup'],
    ['pages-pr-links.contract', 'if preview.phase == "error"', /^\s+column /, 'PullRequestLinkPreview.tsx TooltipPopup'],
    ['r4-git-publish.contract', 'pointer-events="none" z-index=82', /^\s+column /, 'GitActionsControl.tsx TooltipPopup'],
    ['settings-diagnostics.contract', 'component StatInfo', /^\s+text tip /, 'DiagnosticsSettings.tsx TooltipPopup'],
    ['composer-controls.contract', 'if which == "context"', /testId="context-window-details"/, 'ContextWindowMeter.tsx:91 tooltipStyle'],
    ['pages-usage.contract', 'component UnpricedPopup', /id="usage-seg-pop-unpriced"/, 'UsagePage.tsx:527 tooltipStyle'],
    ['pages-hero.contract', 'component PagesLanding', /^\s+column flex=1 min-height=0 width="100%" padding="3rem"/, 'ui/empty.tsx:22 Empty'],
    ['r5-shell-prs.contract', 'component PrEmptyState', /testId="pull-requests-empty"/, 'ui/empty.tsx:22 Empty'],
    ['browser-surface.contract', 'component BrowserNothingYet', /testId="browser-empty"/, 'ui/empty.tsx:22 Empty'],
    ['connections.contract', 'if page.environments.length == 0', /testId="environments-empty"/, 'ui/empty.tsx:22 Empty'],
    ['settings-source-control.contract', 'SectionTitle(title="Server environment")', /^\s+column align-items="center"/, 'ui/empty.tsx:22 Empty'],
  ];
  for (const [file, start, owner, reference] of balanced) {
    test(`${reference}: text-balance is ${file}'s inherited text-wrap="balance" (${start})`, async () => {
      const lines = (await source(file)).split('\n');
      const from = lines.findIndex(line => line.includes(start));
      expect(from).toBeGreaterThanOrEqual(0);
      const at = lines.findIndex((line, i) => i >= from && code(line) && owner.test(line));
      expect(at).toBeGreaterThanOrEqual(from);
      expect(lines[at]).toContain('text-wrap="balance"');
    });
  }

  test('balance stays on those owners: never on nowrap or preformatted text, the stacked toasts, the draft headline or a hover card', async () => {
    const uses = (await contracts()).flatMap(({ name, lines }) => lines.filter(line => code(line) && line.includes('text-wrap="balance"')).map(line => ({ name, line })));
    expect(uses.length).toBe(balanced.length);
    for (const { line } of uses) expect(line).not.toMatch(/white-space="(pre|nowrap)"/);
    // ui/toast.tsx balances only the anchored toast (toast.tsx:731); DraftHeroHeadline and the freshness card write none.
    expect(await source('shell-toast.contract')).not.toContain('text-wrap=');
    const hero = await source('pages-hero.contract');
    expect(hero.slice(hero.indexOf('component EmptyThread'), hero.indexOf('component ScratchLink'))).not.toContain('text-wrap=');
    const actions = await source('pages-pr-actions.contract');
    expect(actions.slice(actions.indexOf('component PaFreshnessCard'))).not.toContain('text-wrap="balance"');
  });
});
