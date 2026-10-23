import type { T3Client } from '../client/client';
import { composerChipOpen, composerChipPreview } from './composer-chip-bridge';
import { composerFieldObservation } from './composer-field-bridge';
import { chipPopoverView } from './composer-chip-popover';

/** A mounted preview publishes independent, monotonically numbered facts. */
export function composerChipObserved(client: T3Client, value: unknown) {
  const snapshot = composerFieldObservation(value);
  if (snapshot?.owner === `${client.snapshotOwner}:prompt-preview`) composerChipPreview(client, snapshot);
  return chipPopoverView(client);
}

export function composerChipPressed(client: T3Client, value: unknown) {
  composerChipOpen(client, value);
  return chipPopoverView(client);
}
