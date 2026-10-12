// browser-surface part 4 (profiles): the cookie import's contract and its wizard's steps (MIT reference, see LICENSE-T3,
// T3 Code 1e2ecbd975: packages/contracts/src/browserImport.ts; apps/web/src/components/settings/
// browserImportWizard.logic.ts, BrowserImportWizard.tsx's labels, IntegrationsSettings.tsx `importFailureReason`).
//
// Only cookies are imported: they carry the logged-in sessions. Availability is per source and the reasons are modelled
// explicitly, because some are a permission the user can grant and one is a limitation nothing works around.

export const BROWSER_IMPORT_SOURCE_IDS = ['chrome', 'edge', 'brave', 'vivaldi', 'opera', 'arc', 'helium', 'firefox', 'safari'] as const;
export type BrowserImportSourceId = (typeof BROWSER_IMPORT_SOURCE_IDS)[number];
export const BROWSER_IMPORT_UNAVAILABLE_REASONS = ['notInstalled', 'needsKeychainApproval', 'keychainItemMissing', 'needsFullDiskAccess', 'browserRunning', 'unsupportedPlatform'] as const;
export type BrowserImportUnavailableReason = (typeof BROWSER_IMPORT_UNAVAILABLE_REASONS)[number];
export const BROWSER_IMPORT_FAILURE_REASONS = [...BROWSER_IMPORT_UNAVAILABLE_REASONS,
  'keychainUnavailable', 'unknownSource', 'unknownSourceProfile', 'sessionUnavailable', 'profileNotSaved', 'profileLimitReached', 'readFailed'] as const;
export type BrowserImportFailureReason = (typeof BROWSER_IMPORT_FAILURE_REASONS)[number];

/** A profile inside the source browser (Chromium's "Default" directory); the count is absent when unreadable. */
export type BrowserImportSourceProfile = { directory: string; name: string; cookieCount?: number };
export type BrowserImportSource = { id: BrowserImportSourceId; name: string; profiles: BrowserImportSourceProfile[]; unavailable?: BrowserImportUnavailableReason };
export type BrowserImportInput = { sourceId: BrowserImportSourceId; sourceProfileDirectory: string; targetProfileId: string };
export type BrowserImportResult = { imported: number; skipped: number; skippedDomains: string[] };

const BROWSER_IMPORT_UNAVAILABLE_COPY: Readonly<Record<BrowserImportUnavailableReason, string>> = {
  notInstalled: 'Not installed on this machine.',
  needsKeychainApproval: 'Needs Keychain access to read its cookies.',
  keychainItemMissing: 'No encryption key in your Keychain — sign in to that browser once, then retry.',
  needsFullDiskAccess: 'Give T3 Code Full Disk Access in System Settings → Privacy & Security, then retry.',
  browserRunning: 'Quit the browser first so its cookie database can be read.',
  unsupportedPlatform: "Importing from this browser isn't possible on this platform.",
};
/** What to tell the user when an attempted import fails. */
export const BROWSER_IMPORT_FAILURE_COPY: Readonly<Record<BrowserImportFailureReason, string>> = {
  ...BROWSER_IMPORT_UNAVAILABLE_COPY,
  keychainUnavailable: 'The system keyring could not be accessed. Make sure your desktop keyring is running and unlocked, then retry.',
  unknownSource: 'That browser is no longer available to import from.',
  unknownSourceProfile: 'That browser profile no longer exists.',
  sessionUnavailable: 'The target profile could not be opened.',
  profileNotSaved: "The cookies were imported, but the new profile couldn't be saved. Try again.",
  profileLimitReached: "You've reached the profile limit. Delete a profile or import into an existing one.",
  readFailed: "The browser's cookie database could not be read.",
};

/** IPC flattens the failure to its message, so the reason token travels inside it; anything else reads as readFailed. */
export const importFailureReason = (cause: unknown): BrowserImportFailureReason => {
  const message = String((cause as { message?: unknown } | undefined)?.message ?? '');
  return BROWSER_IMPORT_FAILURE_REASONS.find(reason => message.includes(`failed: ${reason}.`)) ?? 'readFailed';
};

// ── browserImportWizard.logic.ts ──────────────────────────────────────────────────────────────────
export interface WizardTargetProfile { readonly id: string; readonly name: string }
export type WizardTarget = { readonly kind: 'new'; readonly profileId: string } | { readonly kind: 'existing'; readonly profileId: string; readonly name: string };
export type WizardTargetSelection = { readonly kind: 'new' } | { readonly kind: 'existing'; readonly profileId: string };

export function initialTargetSelection(canCreateProfile: boolean, targetProfiles: readonly WizardTargetProfile[]): WizardTargetSelection {
  if (canCreateProfile) return { kind: 'new' };
  const first = targetProfiles[0];
  return first ? { kind: 'existing', profileId: first.id } : { kind: 'new' };
}
export function resolveWizardTarget(selection: WizardTargetSelection, newProfileId: string, targetProfiles: readonly WizardTargetProfile[]): WizardTarget | undefined {
  if (selection.kind === 'new') return { kind: 'new', profileId: newProfileId };
  const profile = targetProfiles.find(candidate => candidate.id === selection.profileId);
  return profile === undefined ? undefined : { kind: 'existing', profileId: selection.profileId, name: profile.name };
}
export type ImportOutcome =
  | { readonly kind: 'imported'; readonly imported: number; readonly skipped: number; readonly skippedDomains: readonly string[]; readonly targetName: string }
  | { readonly kind: 'blocked'; readonly reason: BrowserImportFailureReason };
/** The wizard's screens; every one is a place the user can act from. */
export type WizardStep =
  | { readonly step: 'quit' }
  | { readonly step: 'fullDiskAccess'; readonly resume: 'configure' | 'import'; readonly checked?: boolean }
  | { readonly step: 'configure' }
  | { readonly step: 'checking'; readonly check: 'browser' | 'fullDiskAccess' }
  | { readonly step: 'importing' }
  | { readonly step: 'done'; readonly imported: number; readonly skipped: number; readonly skippedDomains: readonly string[]; readonly targetName: string }
  | { readonly step: 'blocked'; readonly reason: BrowserImportFailureReason };

/** The import owns its target store until the write finishes. */
export const canCloseWizard = (step: WizardStep): boolean => step.step !== 'importing';
/** A running browser is the one thing known up front; everything else is found by trying. */
export function initialWizardStep(source: BrowserImportSource): WizardStep {
  if (source.unavailable === 'browserRunning') return { step: 'quit' };
  if (source.unavailable === 'needsFullDiskAccess') return { step: 'fullDiskAccess', resume: 'configure' };
  if (source.unavailable !== undefined) return { step: 'blocked', reason: source.unavailable };
  if (source.profiles.length === 0) return { step: 'blocked', reason: 'unknownSourceProfile' };
  return { step: 'configure' };
}
export function outcomeToStep(outcome: ImportOutcome): WizardStep {
  if (outcome.kind === 'imported') return { step: 'done', imported: outcome.imported, skipped: outcome.skipped, skippedDomains: outcome.skippedDomains, targetName: outcome.targetName };
  if (outcome.reason === 'browserRunning') return { step: 'quit' };
  // The failed import already checked access: say it is still denied rather than show the same screen again.
  if (outcome.reason === 'needsFullDiskAccess') return { step: 'fullDiskAccess', resume: 'import', checked: true };
  return { step: 'blocked', reason: outcome.reason };
}
export function refreshedSourceStep(source: BrowserImportSource | undefined): WizardStep {
  return source === undefined ? { step: 'blocked', reason: 'unknownSource' } : initialWizardStep(source);
}
export function fullDiskAccessRecheckStep(source: BrowserImportSource | undefined): WizardStep {
  const next = refreshedSourceStep(source);
  return next.step === 'fullDiskAccess' ? { ...next, checked: true } : next;
}
export function refreshedSourceProfileDirectory(currentDirectory: string, source: BrowserImportSource): string {
  return source.profiles.some(profile => profile.directory === currentDirectory) ? currentDirectory : source.profiles[0]?.directory ?? '';
}
/** Whether retrying could clear a failure (a keychain approval, a key the user signs in to create, a transient read). */
export function isRetryableReason(reason: BrowserImportFailureReason): boolean {
  switch (reason) {
    case 'needsKeychainApproval': case 'keychainItemMissing': case 'keychainUnavailable': case 'readFailed': case 'sessionUnavailable': case 'profileNotSaved': return true;
    default: return false;
  }
}
/** "example.com and google.com", or "a, b, c and 4 more" past a few. */
export function formatSkippedDomains(domains: readonly string[]): string {
  if (domains.length === 0) return '';
  if (domains.length === 1) return domains[0]!;
  if (domains.length <= 3) return `${domains.slice(0, -1).join(', ')} and ${domains.at(-1)}`;
  return `${domains.slice(0, 3).join(', ')} and ${domains.length - 3} more`;
}

// ── BrowserImportWizard.tsx's labels ──────────────────────────────────────────────────────────────
const grouped = (count: number) => String(count).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
/** "5,065 cookies", or "no cookies", or nothing when the store is unreadable. */
export function cookieCountLabel(count: number | undefined): string {
  if (count === undefined) return '';
  if (count === 0) return 'no cookies';
  return `${grouped(count)} ${count === 1 ? 'cookie' : 'cookies'}`;
}
export const cookieResultCount = (count: number): string => `${grouped(count)} ${count === 1 ? 'cookie' : 'cookies'}`;
/** DoneStep's title and description. */
export function doneCopy(step: { imported: number; skipped: number; targetName: string }, environmentName: string): { title: string; description: string } {
  if (step.imported > 0) return { title: `Imported ${cookieResultCount(step.imported)}`, description: `Added to ${step.targetName} for ${environmentName}.${step.skipped > 0 ? ` ${cookieResultCount(step.skipped)} skipped.` : ''}` };
  if (step.skipped > 0) return { title: `Skipped ${cookieResultCount(step.skipped)}`, description: `No cookies were imported for ${environmentName}.` };
  return { title: 'No cookies found', description: `There were no cookies to import for ${environmentName}.` };
}
