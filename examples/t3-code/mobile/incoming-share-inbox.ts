// Pinned365aa87982 IncomingShareProvider and incoming-share-presentation.ts.
// @ref llp/1109.005-composer-and-transcript.decision.md#incoming-share-inbox
import type { T3Client } from './shared/client';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { obj } from './shared/domain';
import { letGo, letGoAware } from './shared/let-go';
import { incomingShareDestination, incomingShareEntry, incomingShareID, incomingShareUUID,
  type IncomingShareEntry, type IncomingShareDestination } from './incoming-share-model';
export interface IncomingShareReservation { shareId: string; adoptionId: string; destination: IncomingShareDestination; attachmentIds: string[]; phase: 'reserved' | 'staged' | 'cancelling' }
interface Inbox { ready: boolean; available: boolean; error: string; revision: number; serial: number; entries: IncomingShareEntry[]; reservations: IncomingShareReservation[] }
const inboxes = new WeakMap<T3Client, Inbox>();
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
function inbox(client: T3Client): Inbox {
  let value = inboxes.get(client);
  if (!value) { value = { ready: false, available: false, error: '', revision: 0, serial: 0, entries: [], reservations: [] }; inboxes.set(client, value); }
  return value;
}
export function mobileIncomingShare(client: T3Client, id: string) { const entry = inbox(client).entries.find(entry => entry.id === id); return entry ? copy(entry) : null; }
export function mobileIncomingShareReservation(client: T3Client, id: string) { const item = inbox(client).reservations.find(item => item.shareId === id); return item ? copy(item) : null; }
export function mobileIncomingShareInboxState(client: T3Client) { const { ready, available, error, revision } = inbox(client); return { ready, available, error, revision }; }
/** Validate the complete native snapshot before replacing any readable saved item. */
export function incomingShareInboxDecode(value: unknown) {
  const raw = obj(value);
  if (typeof raw.available !== 'boolean' || !Array.isArray(raw.entries) || !Array.isArray(raw.reservations)) throw new ClientError('The saved share inbox is invalid.', 'protocol');
  const entries = raw.entries.map(incomingShareEntry).sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  if (new Set(entries.map(entry => entry.id)).size !== entries.length) throw new ClientError('The share inbox contains repeated identities.', 'protocol');
  const reservations: IncomingShareReservation[] = raw.reservations.map(value => {
    const item = obj(value), entry = entries.find(entry => entry.id === item.shareId);
    if (!entry || !incomingShareID(item.shareId) || !incomingShareUUID(item.adoptionId)
      || (item.phase !== 'reserved' && item.phase !== 'staged' && item.phase !== 'cancelling') || !Array.isArray(item.attachmentIds)
      || !item.attachmentIds.every((id): id is string => incomingShareUUID(id) && entry.attachments.some(file => file.id === id))
      || new Set(item.attachmentIds).size !== item.attachmentIds.length) throw new ClientError('The saved share reservation is invalid.', 'protocol');
    return { shareId: item.shareId, adoptionId: item.adoptionId, destination: incomingShareDestination(item.destination),
      attachmentIds: [...item.attachmentIds], phase: item.phase };
  });
  if (new Set(reservations.map(item => item.shareId)).size !== reservations.length || new Set(reservations.map(item => item.adoptionId)).size !== reservations.length)
    throw new ClientError('The share inbox contains overlapping reservations.', 'protocol');
  return { available: raw.available, entries, reservations };
}
/** Each resource answer owns its watch and native handle. A late read cannot
 * replace a newer read. Unavailability of the OS producer never hides saved data. */
export async function mobileIncomingShareRead(client: T3Client, nativeInput?: Native | null) {
  const state = inbox(client), serial = ++state.serial;
  if (!nativeInput?.available) { state.ready = true; state.available = false; state.revision++; return mobileIncomingShareInboxState(client); }
  const native = letGoAware(nativeInput);
  try {
    native.watch('t3.incoming-shares');
    const reply = await bridgeReply(native, { op: 'mobileIncomingShares', action: 'read' });
    if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
    const { available, entries, reservations } = incomingShareInboxDecode(reply.value);
    if (serial === state.serial) { state.entries = entries; state.reservations = reservations; state.available = available; state.error = ''; state.ready = true; state.revision++; }
  } catch (error) {
    if (letGo(error)) throw error;
    if (serial === state.serial) { state.ready = true; state.error = error instanceof Error ? error.message : 'Could not read shared content.'; state.revision++; }
  }
  return mobileIncomingShareInboxState(client);
}
interface Presentation { presentedShareId: string | null; dismissedShareId: string | null }
const empty = (): Presentation => ({ presentedShareId: null, dismissedShareId: null });
/** Matches the pinned transition by durable share ID, including consumption
 * while a sheet remains mounted. The root commits state and navigation together. */
export function incomingSharePresentation(state: Presentation, isShareSheetPresented: boolean, pendingShareId: string | null) {
  if (isShareSheetPresented) return { state: state.presentedShareId !== null && pendingShareId !== state.presentedShareId ? empty() : state, shareIdToPresent: null };
  if (state.presentedShareId !== null && pendingShareId === state.presentedShareId)
    return { state: { presentedShareId: null, dismissedShareId: state.presentedShareId }, shareIdToPresent: null };
  if (pendingShareId === null) return { state: empty(), shareIdToPresent: null };
  if (state.dismissedShareId === pendingShareId) return { state: { ...state, presentedShareId: null }, shareIdToPresent: null };
  return { state: { presentedShareId: pendingShareId, dismissedShareId: null }, shareIdToPresent: pendingShareId };
}
export function mobileIncomingSharePresentation(client: T3Client, serialized: string, sheetPresented: boolean, routeId: string) {
  let previous = empty();
  try { const parsed = obj(JSON.parse(serialized)); previous = { presentedShareId: incomingShareID(parsed.presentedShareId) ? parsed.presentedShareId : null,
    dismissedShareId: incomingShareID(parsed.dismissedShareId) ? parsed.dismissedShareId : null }; } catch { /* Initial root state. */ }
  const state = inbox(client), transition = state.ready ? incomingSharePresentation(previous, sheetPresented, state.entries[0]?.id ?? null) : { state: previous, shareIdToPresent: null };
  return { inboxRevision: state.revision, previousState: serialized, state: JSON.stringify(transition.state), location: transition.shareIdToPresent ? `/new?incomingShareId=${encodeURIComponent(transition.shareIdToPresent)}` : '', requestRoute: routeId };
}
export function incomingShareSubtitle(entry: IncomingShareEntry | null) {
  if (!entry) return '';
  const files = entry.attachments;
  return files.length === 0 ? 'Choose a project for what you shared' : files.length === 1
    ? `Choose a project for the ${files[0]!.kind === 'image' ? 'image' : 'file'} you shared`
    : `Choose a project for the ${files.length} ${files.every(file => file.kind === 'image') ? 'images' : 'files'} you shared`;
}
