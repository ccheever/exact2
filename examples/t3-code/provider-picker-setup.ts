// ModelPickerContent's provider setup, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/components/chat/ModelPickerContent.tsx:103-114 (shouldOfferModelPickerSetup),
// :245-264 (the active instance that needs setup opens selected), :335-357 (instances that
// need setup stay selectable in the rail), :576-590 and :1055-1076 (the footer: the status
// message and "Set up <name>" or "Open provider setup"). Changes: the picker's options are
// the instance's models (getAppModelOptionsForInstance adds only an unavailable saved slug,
// which never counts as available).
import { arr, obj, str, type Obj } from './domain';
import { getProviderStatusMessage, hasProviderSetup } from './provider-status-message';

/** isProviderInstancePickerReady: enabled, available and ready. */
export const pickerReady = (provider: Obj) => provider.enabled === true && provider.availability !== 'unavailable' && provider.status === 'ready';

/** shouldOfferModelPickerSetup: an enabled instance with in-app setup that is not usable yet. */
export function shouldOfferModelPickerSetup(provider: Obj, options: ReadonlyArray<{ isUnavailable?: boolean }>): boolean {
  return provider.enabled === true && provider.status !== 'disabled' && hasProviderSetup(provider)
    && (!pickerReady(provider) || provider.installed !== true || obj(provider.auth).status === 'unauthenticated' || !options.some(option => !option.isUnavailable));
}

/** The instance's picker options: its models, all available. */
export const pickerOptions = (provider: Obj) => arr(provider.models).map(() => ({ isUnavailable: false }));

/**
 * The footer's setup entries: the instances that need setup, the selected one only (or, on
 * an empty favorites view, all of them); none while searching. Each has its status message
 * and the button's label.
 */
export function pickerSetupEntries(entries: Obj[], selected: string, searching: boolean, listed: number) {
  if (searching) return [];
  const needing = entries.filter(entry => shouldOfferModelPickerSetup(entry, pickerOptions(entry)));
  const shown = needing.filter(entry => selected !== 'favorites' ? str(entry.instanceId) === selected : listed === 0);
  return shown.map(entry => ({ id: str(entry.instanceId), message: getProviderStatusMessage(entry),
    label: shown.length > 1 ? `Set up ${str(entry.displayName).trim() || str(entry.driver)}` : 'Open provider setup' }));
}
