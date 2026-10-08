// Settings → General's model pickers (New threads › Model and Text generation model): the
// composer's ProviderModelPicker catalog (model-catalog.ts) over the settings scope, at T3 Code
// 1e2ecbd975 (MIT, see LICENSE-T3). ProjectDefaultsSettings.tsx and SettingsPanels.tsx give the
// shared picker lockedProvider null, the representative environment's instance entries (only
// those that support text generation, for that row), the setting's own selection and
// getModelDisabledReason (useScopedModelAvailability.ts). Browsing, searching and favoriting
// never write the setting; a chosen row does, through the settings-core command whose id is
// the picker's target (`<row>:|<machine>|<projectKey>|<checkout>`).
import type { T3Client } from './client';
import { pickerCatalog } from './model-catalog';
import { scopedModelReason, settingsModelTarget } from './settings-core';

type Badge = Parameters<typeof pickerCatalog>[3];

/** The picker's catalog for one General model row; an unknown or unavailable target lists nothing. */
export function settingsPickerCatalog(client: T3Client, target: string, requested: string, query: string, badge: Badge) {
  let resolved: ReturnType<typeof settingsModelTarget> = null;
  try { resolved = settingsModelTarget(client, target); } catch { resolved = null; }
  const view = { config: { providers: resolved?.providers ?? [] }, local: client.local, providerId: resolved?.instanceId ?? '', modelId: resolved?.model ?? '' };
  return pickerCatalog(view, requested, query, badge, { reason: (instanceId, model) => resolved ? scopedModelReason(resolved.context, instanceId, model) : '' });
}
