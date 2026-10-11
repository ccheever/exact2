// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under their own names: apps/web/src/components/settings/
// browserImportWizard.logic.test.ts (all of it) and IntegrationsSettings.logic.test.ts's importFailureReason (2). Clone
// rows: the wizard's labels (BrowserImportWizard.tsx cookieCountLabel and DoneStep's copy).
import { describe, expect, it } from 'bun:test';
import {
  canCloseWizard, cookieCountLabel, doneCopy, formatSkippedDomains, fullDiskAccessRecheckStep, importFailureReason, initialTargetSelection, initialWizardStep,
  isRetryableReason, outcomeToStep, refreshedSourceProfileDirectory, refreshedSourceStep, resolveWizardTarget, type BrowserImportSource,
} from './browser-import';

describe('wizard target selection', () => {
  it('treats an existing profile whose id is new as an existing target', () => {
    const profiles = [{ id: 'new', name: 'Existing new' }];
    const selection = initialTargetSelection(false, profiles);
    expect(selection).toEqual({ kind: 'existing', profileId: 'new' });
    expect(resolveWizardTarget(selection, 'generated', profiles)).toEqual({ kind: 'existing', profileId: 'new', name: 'Existing new' });
  });
  it('rejects an existing target that is no longer listed', () => {
    expect(resolveWizardTarget({ kind: 'existing', profileId: 'removed' }, 'generated', [{ id: 'default', name: 'Default' }])).toBeUndefined();
  });
});

const source = (over: Partial<BrowserImportSource> = {}): BrowserImportSource => ({ id: 'helium', name: 'Helium', profiles: [{ directory: 'Default', name: 'You' }], ...over });

describe('initialWizardStep', () => {
  it('opens on the quit screen when the browser is running', () => {
    expect(initialWizardStep(source({ unavailable: 'browserRunning' }))).toEqual({ step: 'quit' });
  });
  it('opens on configure when the source is ready', () => {
    expect(initialWizardStep(source())).toEqual({ step: 'configure' });
  });
  it('asks for Full Disk Access before choosing an import target', () => {
    expect(initialWizardStep(source({ profiles: [], unavailable: 'needsFullDiskAccess' }))).toEqual({ step: 'fullDiskAccess', resume: 'configure' });
  });
  it('blocks on a reason nothing local can fix', () => {
    expect(initialWizardStep(source({ unavailable: 'unsupportedPlatform' }))).toEqual({ step: 'blocked', reason: 'unsupportedPlatform' });
  });
  it('blocks a ready source that reports no importable profiles', () => {
    expect(initialWizardStep(source({ profiles: [] }))).toEqual({ step: 'blocked', reason: 'unknownSourceProfile' });
  });
  it('handles source availability before checking its profiles', () => {
    expect(initialWizardStep(source({ profiles: [], unavailable: 'needsKeychainApproval' }))).toEqual({ step: 'blocked', reason: 'needsKeychainApproval' });
  });
});

describe('canCloseWizard', () => {
  it('keeps the wizard open while an import writes its target profile', () => {
    expect(canCloseWizard({ step: 'importing' })).toBe(false);
    expect(canCloseWizard({ step: 'configure' })).toBe(true);
    expect(canCloseWizard({ step: 'blocked', reason: 'readFailed' })).toBe(true);
  });
});

describe('outcomeToStep', () => {
  it('lands on done after a successful import', () => {
    expect(outcomeToStep({ kind: 'imported', imported: 12, skipped: 3, skippedDomains: ['example.com'], targetName: 'Work' }))
      .toEqual({ step: 'done', imported: 12, skipped: 3, skippedDomains: ['example.com'], targetName: 'Work' });
  });
  it('routes a reopened browser back to the quit screen', () => {
    expect(outcomeToStep({ kind: 'blocked', reason: 'browserRunning' })).toEqual({ step: 'quit' });
  });
  it('routes a Full Disk Access refusal to its own screen', () => {
    expect(outcomeToStep({ kind: 'blocked', reason: 'needsFullDiskAccess' })).toEqual({ step: 'fullDiskAccess', resume: 'import', checked: true });
  });
  it('surfaces every other failure on the blocked screen', () => {
    expect(outcomeToStep({ kind: 'blocked', reason: 'readFailed' })).toEqual({ step: 'blocked', reason: 'readFailed' });
  });
});

describe('refreshedSourceStep', () => {
  it('moves to configure once a quit browser frees its cookies', () => {
    expect(refreshedSourceStep(source())).toEqual({ step: 'configure' });
  });
  it('stays on quit while the browser is still running', () => {
    expect(refreshedSourceStep(source({ unavailable: 'browserRunning' }))).toEqual({ step: 'quit' });
  });
  it('blocks when the source vanished from the list', () => {
    expect(refreshedSourceStep(undefined)).toEqual({ step: 'blocked', reason: 'unknownSource' });
  });
  it('blocks when the refreshed source has no profiles', () => {
    expect(refreshedSourceStep(source({ profiles: [] }))).toEqual({ step: 'blocked', reason: 'unknownSourceProfile' });
  });
});

describe('fullDiskAccessRecheckStep', () => {
  it('marks a still-denied access check for visible feedback', () => {
    expect(fullDiskAccessRecheckStep(source({ unavailable: 'needsFullDiskAccess' }))).toEqual({ step: 'fullDiskAccess', resume: 'configure', checked: true });
  });
  it('moves on once access reveals the source profiles', () => {
    expect(fullDiskAccessRecheckStep(source())).toEqual({ step: 'configure' });
  });
});

describe('refreshedSourceProfileDirectory', () => {
  const refreshed = source({ profiles: [{ directory: 'Default', name: 'Personal' }, { directory: 'Profile 2', name: 'Work' }] });
  it('preserves a selected non-first profile after the browser quits', () => {
    expect(refreshedSourceProfileDirectory('Profile 2', refreshed)).toBe('Profile 2');
  });
  it('falls back only when the old profile vanished and clears an empty result', () => {
    expect(refreshedSourceProfileDirectory('Removed', refreshed)).toBe('Default');
    expect(refreshedSourceProfileDirectory('Removed', source({ profiles: [] }))).toBe('');
  });
});

describe('isRetryableReason', () => {
  it('offers a retry for failures a second attempt can clear', () => {
    expect(isRetryableReason('needsKeychainApproval')).toBe(true);
    expect(isRetryableReason('keychainUnavailable')).toBe(true);
    expect(isRetryableReason('readFailed')).toBe(true);
  });
  it('does not offer a retry for a permanent failure', () => {
    expect(isRetryableReason('unsupportedPlatform')).toBe(false);
    expect(isRetryableReason('unknownSourceProfile')).toBe(false);
    expect(isRetryableReason('profileLimitReached')).toBe(false);
  });
  it('offers a retry once the user has signed in to create the missing key', () => {
    expect(isRetryableReason('keychainItemMissing')).toBe(true);
  });
  it('offers a retry when the cookies landed but the new profile was not saved', () => {
    expect(isRetryableReason('profileNotSaved')).toBe(true);
  });
});

describe('formatSkippedDomains', () => {
  it('joins a short list naturally', () => {
    expect(formatSkippedDomains([])).toBe('');
    expect(formatSkippedDomains(['a.com'])).toBe('a.com');
    expect(formatSkippedDomains(['a.com', 'b.com'])).toBe('a.com and b.com');
    expect(formatSkippedDomains(['a.com', 'b.com', 'c.com'])).toBe('a.com, b.com and c.com');
  });
  it('summarizes a long list', () => {
    expect(formatSkippedDomains(['a.com', 'b.com', 'c.com', 'd.com', 'e.com'])).toBe('a.com, b.com, c.com and 2 more');
  });
});

// Mirrors `BrowserImportFailedError.message`, which IPC flattens to a string before the renderer sees it.
const failure = (reason: string) => ({ message: `Importing cookies from safari failed: ${reason}.` });
describe('importFailureReason', () => {
  it('recovers the reason token from the flattened message', () => {
    expect(importFailureReason(failure('needsFullDiskAccess'))).toBe('needsFullDiskAccess');
    expect(importFailureReason(failure('browserRunning'))).toBe('browserRunning');
    expect(importFailureReason(failure('readFailed'))).toBe('readFailed');
    expect(importFailureReason(failure('keychainUnavailable'))).toBe('keychainUnavailable');
    expect(importFailureReason(failure('profileNotSaved'))).toBe('profileNotSaved');
    expect(importFailureReason(failure('profileLimitReached'))).toBe('profileLimitReached');
  });
  it('falls back to readFailed for anything it cannot classify', () => {
    expect(importFailureReason(new Error('something else entirely'))).toBe('readFailed');
    expect(importFailureReason(undefined)).toBe('readFailed');
  });
});

describe('the wizard labels (clone: BrowserImportWizard.tsx)', () => {
  it('counts cookies as the source tile and the done screen word them', () => {
    expect(cookieCountLabel(undefined)).toBe('');
    expect(cookieCountLabel(0)).toBe('no cookies');
    expect(cookieCountLabel(1)).toBe('1 cookie');
    expect(cookieCountLabel(5065)).toBe('5,065 cookies');
    expect(doneCopy({ imported: 12, skipped: 3, targetName: 'Work' }, 'This device')).toEqual({ title: 'Imported 12 cookies', description: 'Added to Work for This device. 3 cookies skipped.' });
    expect(doneCopy({ imported: 0, skipped: 1, targetName: 'Work' }, 'This device')).toEqual({ title: 'Skipped 1 cookie', description: 'No cookies were imported for This device.' });
    expect(doneCopy({ imported: 0, skipped: 0, targetName: 'Work' }, 'This device')).toEqual({ title: 'No cookies found', description: 'There were no cookies to import for This device.' });
  });
});
