// A non-Git draft with somewhere to run shows
// the composer's context strip with only the machine selector. T3 Code, MIT, see LICENSE-T3:
// BranchToolbar.logic.ts shouldShowEnvironmentIndicator and shouldShowComposerContextStrip (ported as
// they are, with BranchToolbar.logic.test.ts), ChatView.tsx (showComposerEnvironmentIndicator takes
// canPickEnvironment from logicalProjectEnvironments.length > 1) and BranchToolbar.tsx, which renders
// BranchToolbarEnvironmentSelector alone when there are no Git controls: a Select ("Run on") for a
// draft, the static machine row for a started thread (envLocked). A No project draft is the common
// case since c47f4263f9 offers every machine's "No project" folder.
// exact2-required changes: isDraftHeroState is "a draft" (!client.threadId; this client's draft
// never has a timeline). Since v0.1.5 a draft always shows the strip, which names the Permissions its
// first send uses; the machine selector shows only with the environment indicator.
// useLabelsOverflow (BranchToolbar.tsx at 74541bab17, with resolveContextStripLabelsCompact) reads the
// laid-out strip from its layout frame (layout-frames.ts) and the module's anchors instead of the DOM.
import type { T3Client } from '../client/client';
import { obj } from '../client/domain';
import { probeKey } from './composer-menus';
import { addTargetOptions, environmentOptions, NO_RUN_ON } from '../details/git-env';
import { runOnMenuWidth } from './composer-menus';
import { autoIndicator, withAutoOption } from './auto-balance';
import { fleet, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { layoutFrames } from '../shell/layout-frames';

// A remote (non-primary) environment is always surfaced, even when it is the
// only environment available: with a single connected machine there is nothing
// to pick, but the user still needs to see where the project runs.
export function shouldShowEnvironmentIndicator(input: {
  activeEnvironment: { isPrimary: boolean } | null;
  canPickEnvironment: boolean;
}): boolean {
  if (input.canPickEnvironment) return true;
  return input.activeEnvironment !== null && !input.activeEnvironment.isPrimary;
}

export function shouldShowComposerContextStrip(input: {
  isDraftHeroState: boolean;
  persistInActiveThreads: boolean;
  hasActiveProject: boolean;
  isGitRepo: boolean;
  showEnvironmentIndicator: boolean;
}): boolean {
  // A new thread always shows the strip, since it names the permissions the first send uses.
  return (
    input.hasActiveProject &&
    (input.isDraftHeroState || (input.persistInActiveThreads && (input.isGitRepo || input.showEnvironmentIndicator)))
  );
}

type Strip = { show: boolean; gitless: boolean; envLocked: boolean } & typeof NO_RUN_ON;

/**
 * The strip of a project that is not a Git repository: a draft's always shows, a started thread's only when the
 * environment indicator shows (a choice of machines, or a remote one); with the indicator a draft gets the "Run on"
 * Select with every machine, a started thread the static machine row.
 */
export function gitlessStrip<T extends Strip>(client: T3Client, hidden: T, persist: boolean, source: EnvironmentFleet = fleet): T {
  const options = environmentOptions(client, source), active = options.find(option => option.selected) ?? null;
  const draft = !client.threadId;
  // A project that can be added to another machine still gets the picker (BranchToolbar hasMultipleEnvironments).
  const add = active && draft ? addTargetOptions(client, source) : [];
  const showEnvironmentIndicator = shouldShowEnvironmentIndicator({ activeEnvironment: active && { isPrimary: active.primary }, canPickEnvironment: options.length > 1 || add.length > 0 });
  const show = shouldShowComposerContextStrip({ isDraftHeroState: draft, persistInActiveThreads: persist, hasActiveProject: !!client.projectId,
    isGitRepo: false, showEnvironmentIndicator });
  if (!show) return hidden;
  if (!showEnvironmentIndicator || !active) return { ...hidden, show: true, gitless: true, envShow: false };
  // The Select's popup holds only the "Run on" group, so it sizes to the machine labels alone.
  const envMenuWidth = runOnMenuWidth(client.presentation, options.map(option => option.label), [], 0, add.length > 0);
  const shown = autoIndicator(client, active); // "Auto balance" leads the group while it is offered
  return { ...hidden, ...NO_RUN_ON, envShow: true, envMachine: shown.machine, envMachineLabel: shown.label,
    envOptions: draft ? withAutoOption(client, options) : [], envAdd: add, envMenuWidth, envLocked: !draft, show: true, gitless: true };
}

const CONTEXT_STRIP_COMPACT_EXPAND_HYSTERESIS_PX = 16;
export function resolveContextStripLabelsCompact(input: { compact: boolean; neededWidth: number; availableWidth: number }): boolean {
  return input.compact
    ? input.neededWidth > input.availableWidth - CONTEXT_STRIP_COMPACT_EXPAND_HYSTERESIS_PX
    : input.neededWidth > input.availableWidth;
}

/** A label in the strip and the anchor names its control and text report under (ComposerStrip: `sc:` and `sl:`). */
export type StripLabel = { anchor: string; text: string };
type StripLayout = { labels: StripLabel[]; controls: string[]; compact: boolean };
const stripLayouts = new WeakMap<T3Client, StripLayout>();
type StripFields = { show: boolean; gitless: boolean; envLocked: boolean; envShow: boolean; envLabel: string; envMachineLabel: string; branchLabel: string; prShow: boolean };
/** The controls and labels the strip view draws, for the measurement below (composer-controls-branch.ts calls it). */
export function noteStripLayout(client: T3Client, view: StripFields): void {
  const workspace = view.envLocked ? { anchor: 'locked', text: view.gitless ? view.envMachineLabel : view.envLabel }
    : { anchor: 'workspace', text: view.gitless || view.envShow ? view.envMachineLabel : view.envLabel };
  const labels = !view.show ? [] : view.gitless ? [workspace] : [workspace, { anchor: 'branch', text: view.branchLabel }];
  const controls = !view.show ? [] : view.gitless ? [workspace.anchor] : [workspace.anchor, ...(view.prShow ? ['pr'] : []), 'branch'];
  stripLayouts.set(client, { labels, controls, compact: stripLayouts.get(client)?.compact ?? false });
}
/** The strip's label texts, laid out by the composer's text probes for their natural widths. */
export function stripLabelTexts(client: T3Client): string[] {
  return (stripLayouts.get(client)?.labels ?? []).map(label => label.text).filter(Boolean);
}
/**
 * useLabelsOverflow: the labels give way to their icons once the controls, each label at its natural width (up to its
 * 240pt cap), need more than the strip's content box. The strip's 7pt inline padding and 1pt border on each side are
 * not free space. A compact strip expands again only with 16pt to spare.
 */
export function stripLabelsCompact(client: T3Client, rootFontSize: number): boolean {
  const layout = stripLayouts.get(client);
  if (!layout || !layout.controls.length) return false;
  const anchors = obj(client.presentation.anchors), strip = layoutFrames(client).strip;
  const stripWidth = Array.isArray(strip) ? Number(strip[2]) : NaN;
  if (!(stripWidth > 0)) return layout.compact;
  const width = (name: string) => { const value = anchors[name]; return Array.isArray(value) ? Number(value[1]) || 0 : 0; };
  const rem = rootFontSize / 16, widths = layout.controls.map(name => width(`sc:${name}`)).filter(value => value > 1);
  let needed = widths.reduce((sum, value) => sum + value, 0) + 4 * rem * Math.max(0, widths.length - 1);
  for (const label of layout.labels) needed += Math.max(0, Math.min(width(probeKey(label.text, 12, 400)) * rem, 240 * rem) - width(`sl:${label.anchor}`));
  layout.compact = resolveContextStripLabelsCompact({ compact: layout.compact, neededWidth: needed, availableWidth: stripWidth - 16 * rem });
  return layout.compact;
}
