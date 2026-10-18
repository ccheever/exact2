import type { T3Client } from '../client/client';
import { composerFieldOwner } from './auto-balance-owner';
import { composerFieldCurrent } from './composer-field-bridge';
import type { ComposerFieldSnapshot } from './composer-field-commands';
import type { ComposerChipPress } from './composer-chip-state';
import { composerChipStore as entry, type ComposerChipPreview } from './composer-chip-store';
import type { ComposerCitationEffect } from './composer-citation-receipts';
import { composerSourceSession } from './composer-rich-state';
import { composerChipFaces } from './composer-chip-presentation';

type Frame = readonly [number, number, number, number];
type Entry = ReturnType<typeof entry>;
const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === 'object' && !Array.isArray(value);
const integer = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value);
function unavailable(client: T3Client): ComposerFieldSnapshot {
  return { generation: client.generation, snapshotOwner: client.snapshotOwner, owner: '', value: '', start: 0, end: 0, live: false, composing: false };
}
function preview(client: T3Client, state: Entry): ComposerFieldSnapshot {
  const current = state.preview;
  return current?.generation === client.generation && current.snapshotOwner === client.snapshotOwner ? current : unavailable(client);
}
/** The preview has its own source session and never reads the active draft. */
export function composerChipPreview(client: T3Client, value: ComposerChipPreview): void {
  if (value.generation !== client.generation || value.snapshotOwner !== client.snapshotOwner || !integer(value.inputSerial) || value.inputSerial < 0) return;
  const state = entry(client);
  const previous = state.preview;
  if (previous && previous.generation === value.generation && previous.snapshotOwner === value.snapshotOwner && previous.owner === value.owner
    && (value.inputSerial < previous.inputSerial || value.inputSerial === previous.inputSerial && value.value !== previous.value)) return;
  state.preview = { ...value };
  state.presses.observe(state.preview, 'preview');
}
function geometry(value: unknown): Frame | null {
  if (!record(value) || value.unavailable !== false) return null;
  const { x, y, width, height } = value;
  return typeof x === 'number' && Number.isFinite(x) && typeof y === 'number' && Number.isFinite(y)
    && typeof width === 'number' && Number.isFinite(width) && width > 0 && typeof height === 'number' && Number.isFinite(height) && height > 0
    ? [x, y, width, height] : null;
}

/** A queued callback carries its measured source identity. Current accepted
 *  field facts and authoritative session chips decide admission again. */
export function composerChipOpen(client: T3Client, value: unknown): ComposerChipPress | null {
  if (!record(value) || !integer(value.generation) || value.generation !== client.generation || value.snapshotOwner !== client.snapshotOwner
    || typeof value.owner !== 'string' || typeof value.value !== 'string' || !integer(value.start) || !integer(value.end)
    || typeof value.source !== 'string' || typeof value.comment !== 'boolean' || !['composer', 'preview'].includes(String(value.surface))) return null;
  const state = entry(client), surface = value.surface === 'preview' ? 'preview' : 'composer';
  const current = surface === 'preview' ? preview(client, state) : composerFieldCurrent(client, composerFieldOwner(client));
  const session = composerSourceSession(client, surface), frame = geometry(value.frame);
  if (!current || !frame || current.owner !== value.owner || current.value !== value.value || !session.matches(current.owner, current.value) || session.mode === 'literal') return null;
  const index = session.chips.findIndex(chip => chip.start === value.start && chip.end === value.end && chip.source === value.source);
  const faces = composerChipFaces(client, current.value, session.chips, surface === 'preview'), face = faces[index];
  if (!face?.interactive || value.comment && !face.comment) return null;
  const chips = session.chips.map((chip, at) => ({ ...chip, label: faces[at]!.label, ...(chip.kind === 'context' ? { contextKind: faces[at]!.kind } : {}) }));
  return state.presses.open(current, surface, chips, { start: value.start, end: value.end, source: value.source }, frame, { comment: value.comment });
}

export function composerChipCurrent(client: T3Client): ComposerChipPress | null {
  const state = entry(client);
  state.presses.observe(composerFieldCurrent(client, composerFieldOwner(client)) ?? unavailable(client), 'composer');
  return state.presses.read(preview(client, state), 'preview');
}
export function composerChipClose(client: T3Client, seq: number): boolean {
  composerChipCurrent(client);
  return entry(client).presses.close(seq);
}
export function composerChipTake(client: T3Client, seq: number): ComposerChipPress | null {
  const current = composerFieldCurrent(client, composerFieldOwner(client));
  composerChipCurrent(client);
  return current ? entry(client).presses.take(current, seq) : null;
}

export function composerCitationPrepare(client: T3Client, value: string, expected: string, effect: ComposerCitationEffect) {
  const current = composerFieldCurrent(client, composerFieldOwner(client));
  if (!current || current.value !== value || current.composing || composerSourceSession(client).mode === 'literal') return null;
  const state = entry(client);
  if (effect.kind === 'finish') {
    const press = composerChipCurrent(client);
    if (!press?.open || press.surface !== 'composer' || press.kind !== 'citation' || press.seq !== effect.seq) return null;
  }
  return state.citations.prepare(current, expected, effect, state.presses.latest, state.presses.interactionRevision);
}
