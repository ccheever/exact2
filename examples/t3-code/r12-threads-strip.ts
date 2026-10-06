// Lane r12-threads (driven and tested in lane r13-threads): a non-Git draft with somewhere to run shows
// the composer's context strip with only the machine selector. T3 Code, MIT, see LICENSE-T3:
// BranchToolbar.logic.ts shouldShowEnvironmentIndicator and shouldShowComposerContextStrip (ported as
// they are, with BranchToolbar.logic.test.ts), ChatView.tsx (showComposerEnvironmentIndicator takes
// canPickEnvironment from logicalProjectEnvironments.length > 1) and BranchToolbar.tsx, which renders
// BranchToolbarEnvironmentSelector alone when there are no Git controls: a Select ("Run on") for a
// draft, the static machine row for a started thread (envLocked). A No project draft is the common
// case since c47f4263f9 offers every machine's "No project" folder.
// exact2-required changes: isDraftHeroState is "a draft" (!client.threadId; this client's draft
// never has a timeline); hostsRestingComposerControls is false, because a resting card here draws
// its own controls row (composer-controls.contract ComposerFooter `resting`) and the strip is not
// mounted while the card rests (app-main.contract), so the strip never hosts those controls.
import type { T3Client } from './client';
import { environmentOptions, NO_RUN_ON } from './r4-git-env';
import { runOnMenuWidth } from './r5-composer-menus';
import { autoIndicator, withAutoOption } from './auto-balance'; // auto-balance
import { fleet, type EnvironmentFleet } from './settings-b-fleet';

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
  /** A collapsed composer's controls currently fit in their measured strip host. */
  hostsRestingComposerControls: boolean;
}): boolean {
  return (
    input.hasActiveProject &&
    (input.isDraftHeroState || input.persistInActiveThreads) &&
    (input.isGitRepo || input.showEnvironmentIndicator || input.hostsRestingComposerControls)
  );
}

type Strip = { show: boolean; gitless: boolean; envLocked: boolean } & typeof NO_RUN_ON;

/**
 * The strip of a project that is not a Git repository: shown only when the environment indicator
 * shows (a choice of machines, or a remote one); a draft gets the "Run on" Select with every
 * machine, a started thread the static machine row.
 */
export function gitlessStrip<T extends Strip>(client: T3Client, hidden: T, persist: boolean, source: EnvironmentFleet = fleet): T {
  const options = environmentOptions(client, source), active = options.find(option => option.selected) ?? null;
  const draft = !client.threadId;
  const showEnvironmentIndicator = shouldShowEnvironmentIndicator({ activeEnvironment: active && { isPrimary: active.primary }, canPickEnvironment: options.length > 1 });
  const show = shouldShowComposerContextStrip({ isDraftHeroState: draft, persistInActiveThreads: persist, hasActiveProject: !!client.projectId,
    isGitRepo: false, showEnvironmentIndicator, hostsRestingComposerControls: false });
  if (!show || !active) return hidden;
  // The Select's popup holds only the "Run on" group, so it sizes to the machine labels alone.
  const envMenuWidth = runOnMenuWidth(client.presentation, options.map(option => option.label), []);
  const shown = autoIndicator(client, active); // auto-balance: "Auto balance" leads the group while it is offered
  return { ...hidden, ...NO_RUN_ON, envShow: true, envMachine: shown.machine, envMachineLabel: shown.label,
    envOptions: draft ? withAutoOption(client, options) : [], envMenuWidth, envLocked: !draft, show: true, gitless: true };
}
