// The chat canvas for the open thread (T3 Code 1e2ecbd975, MIT, see LICENSE-T3:
// apps/web/src/components/chat/ChatCanvas.tsx, ThreadDetailsCard.tsx's reported card,
// preview/ThreadPreviewMiniPlayer.tsx's MiniPlayerShell gestures, ChatView.logic.ts
// shouldRenderPreviewMiniPlayer). The `chatCanvas` source: Contract measures the canvas (the chat
// column's `chat` frame below its 52 pt header), the composer overlay (`overlay`) and the inline
// card's content (`details-content`) with `t3-frame` hooks; this runs resolveChatCanvasLayout and
// answers the chat lane, the card's fold and the floating player's frame, which Contract draws. It also answers
// ThreadDetailsCard's presentation (inline where a placement is left, else the header's popover) and its density,
// from the content heights it keeps per thread and preferred width.
// A pointer gesture on the player is a resource argument (`serial|sourceKey|direction|dx|dy`, the
// pan's total since it began): the first answer of a serial takes the frame on screen as the
// gesture's start, every answer moves or resizes the store from that start, so asking again with
// the same argument changes nothing. As in the reference the gesture uses no obstacles; the layout
// pass applies the readable chat lane and the card, and never writes its clamp back to the store.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { bridgeReply, type Native } from '../client/protocol';
import { chatLaneMetrics, resolveChatCanvasLayout, type ChatCanvasPreview } from './chat-canvas-layout';
import { clampInterfaceFontSize } from '../appearance/appearance-fonts';
import { measureThreadDetailsCard, resolveThreadDetailsCardDensity, resolveThreadDetailsCardLayout, THREAD_DETAILS_CARD_GAP, type ThreadDetailsCardHeights } from '../details/thread-details-card-layout';
import {
  clampPreviewMiniPlayerPosition, PREVIEW_MINI_PLAYER_CORNER_RADIUS, resizePreviewMiniPlayer, resolveDeviceMiniPlayerCornerRadius, resolveDeviceMiniPlayerSourceSize,
  resolvePreviewMiniPlayerPillInset, RESIZE_DIRECTIONS, type BrowserViewportResizeDirection, type DevicePlatform, type PreviewMiniPlayerFrame, type PreviewMiniPlayerSize,
} from '../device/previewMiniPlayerLayout';
import { previewMiniPlayerSourceKey, type PreviewMiniPlayerState } from '../device/previewMiniPlayerStore';
import { deviceKey, deviceThreadId, miniStoreOf } from '../device/device-session';
import { shownBrowserTab, shownDevice } from '../panel/panel-surfaces';
import { floatingBrowserSize } from '../browser/browser-capture'; // a floating Browser tab
import { inlineOpen } from './shell-prefs';
import { detailsKey } from '../details/shell-details';

/** The chat header above the canvas (ChatCanvas starts below it). */
export const CHAT_HEADER_HEIGHT = 52;

export type ChatCanvasMini = { show: boolean; key: string; x: number; top: number; width: number; height: number; radius: number; pillInset: number };
export type ChatCanvasView = {
  /** False until the first answer: the column centres chat by itself meanwhile. */
  ready: boolean; containerWidth: number; containerHeight: number;
  /** The chat lane (chat.left, chat.width, chat.insetEnd; insetStart is always 0). */
  left: number; width: number; insetEnd: number;
  overlapsChat: boolean; overlapsDetailsCard: boolean;
  /** The inline card's place (resolveThreadDetailsCardLayout): hidden when no readable placement is left. */
  cardShown: boolean; cardHeight: number;
  /** ThreadDetailsCard's mode: the header's popover where no inline placement is left (or chat is narrow); `popoverEpoch`
   * counts the mode's switches, so a popover opened before a switch stays closed after it (the inline mode closes it). */
  popover: boolean; popoverEpoch: number;
  /** resolveThreadDetailsCardDensity for the card's current presentation: full, compact or essential. */
  density: string;
  mini: ChatCanvasMini;
};
/** `wide`: the window's rule for the docked card (chat at least 1012 pt: the lane padding, 640, the 32 pt clearance, the 280 pt card and its 12 pt gap); `popoverOpen`: the header's popover is open. */
export type ChatCanvasArgs = { width: number; viewportHeight: number; detailsInline: boolean; chatMax: number; overlaid: boolean; gesture: string; wide: boolean; popoverOpen: boolean };

const NO_MINI: ChatCanvasMini = { show: false, key: '', x: 0, top: 0, width: 0, height: 0, radius: 12, pillInset: 8 };
export const emptyChatCanvas = (): ChatCanvasView => ({ ready: false, containerWidth: 0, containerHeight: 0, left: 0, width: 0, insetEnd: 0,
  overlapsChat: false, overlapsDetailsCard: false, cardShown: true, cardHeight: 0, popover: false, popoverEpoch: 0, density: 'full', mini: NO_MINI });

type Gesture = { serial: string; sourceKey: string; direction: BrowserViewportResizeDirection | null; dx: number; dy: number };
type Shown = { threadKey: string; sourceKey: string; frame: PreviewMiniPlayerFrame; container: PreviewMiniPlayerSize; source: PreviewMiniPlayerSize };
type Presentation = { frames: Obj; streams: Obj };
type CanvasState = { live: Presentation | null; shown: Shown | null; gesture: { serial: string; threadKey: string; start: PreviewMiniPlayerFrame; container: PreviewMiniPlayerSize; source: PreviewMiniPlayerSize } | null;
  /** ThreadDetailsCard's measurements by `${threadKey}:${preferred width or "popup"}`, kept so more room restores detail. */
  heights: Map<string, ThreadDetailsCardHeights>; popover: boolean | null; popoverEpoch: number };
const states = new WeakMap<T3Client, CanvasState>();
function stateOf(client: T3Client): CanvasState {
  let state = states.get(client);
  if (!state) { state = { live: null, shown: null, gesture: null, heights: new Map(), popover: null, popoverEpoch: 0 }; states.set(client, state); }
  return state;
}

/** ThreadDetailsCard's setMeasurements: a measured density replaces its own height and keeps the other. */
function keepHeights(state: CanvasState, key: string, measured: Partial<ThreadDetailsCardHeights>): ThreadDetailsCardHeights {
  const heights = { full: 0, compact: 0, ...state.heights.get(key), ...measured };
  if (state.heights.size > 64 && !state.heights.has(key)) state.heights.delete(state.heights.keys().next().value!);
  state.heights.set(key, heights);
  return heights;
}

/** `serial|sourceKey|direction|dx|dy`; the direction is `move` for a drag from the handle or the pill. */
export function parseGesture(value: string): Gesture | null {
  const parts = value.split('|');
  if (parts.length !== 5 || !parts[0]) return null;
  const direction = parts[2] === 'move' ? null : RESIZE_DIRECTIONS.find(entry => entry === parts[2]);
  const dx = Number(parts[3]), dy = Number(parts[4]);
  if (direction === undefined || !Number.isFinite(dx) || !Number.isFinite(dy)) return null;
  return { serial: parts[0], sourceKey: parts[1]!, direction, dx, dy };
}

const rect = (value: unknown): number[] | null => (Array.isArray(value) && value.length >= 4 && value.every(entry => Number.isFinite(Number(entry))) ? value.map(Number) : null);

/** The stream's report as DeviceScreenSize: the displayed size and orientation (T3DeviceStream.swift reportValue). */
function screenOf(streams: Obj, hostId: string, deviceId: string): { width: number; height: number; orientation?: string } | null {
  const report = obj(streams[deviceKey({ hostId, deviceId })]);
  const width = Number(report.width), height = Number(report.height);
  return width > 0 && height > 0 ? { width, height, orientation: str(report.orientation) || undefined } : null;
}

/** The floating player this thread renders (shouldRenderPreviewMiniPlayer): none while the panel shows the same device. */
function visiblePlayer(client: T3Client): PreviewMiniPlayerState | null {
  const thread = deviceThreadId(client); // activeThreadRef: a draft's own id too
  if (!thread) return null;
  const player = miniStoreOf(client).get(thread);
  if (!player) return null;
  if (player.source.kind === 'browser') return shownBrowserTab(client) === player.source.tabId ? null : player;
  const shown = shownDevice(client);
  return shown && shown.hostId === player.source.hostId && shown.deviceId === player.source.deviceId ? null : player;
}

/** MiniPlayerShell handlePointerMove: the drag clamps to the container, the resize keeps the aspect from the start frame. */
function applyGesture(client: T3Client, state: CanvasState, gesture: Gesture | null, threadKey: string, player: PreviewMiniPlayerState): void {
  if (!gesture) { state.gesture = null; return; }
  if (state.gesture?.serial !== gesture.serial || state.gesture.threadKey !== threadKey) {
    const shown = state.shown;
    // A gesture starts on the frame on screen; one that began on another source or thread is dropped (the stale-source guard).
    state.gesture = shown && shown.threadKey === threadKey && shown.sourceKey === gesture.sourceKey
      ? { serial: gesture.serial, threadKey, start: shown.frame, container: shown.container, source: shown.source } : null;
  }
  const active = state.gesture;
  if (!active) return;
  const store = miniStoreOf(client), sourceKey = previewMiniPlayerSourceKey(player.source);
  if (sourceKey !== gesture.sourceKey) return;
  if (gesture.direction === null) {
    store.move(threadKey, gesture.sourceKey, clampPreviewMiniPlayerPosition({ x: active.start.x + gesture.dx, y: active.start.y + gesture.dy }, active.container, active.start));
    return;
  }
  const next = resizePreviewMiniPlayer({ start: active.start, direction: gesture.direction, delta: { x: gesture.dx, y: gesture.dy }, source: active.source, container: active.container });
  store.resize(threadKey, gesture.sourceKey, next.width, { x: next.x, y: next.y });
}

/** Fresh frames and stream reports: the module's status, watched while a layout reads them. */
const presented = (presentation: Obj): Presentation => ({ frames: obj(presentation.frames), streams: obj(presentation.deviceStreams) });
async function livePresentation(client: T3Client, native: Native | null | undefined, state: CanvasState, watch: boolean): Promise<Presentation> {
  if (!native?.available || !watch) return presented(obj(client.presentation));
  native.watch('t3.status');
  try {
    const reply = await bridgeReply(native, { op: 'status' });
    if (reply.ok) { state.live = presented(obj(obj(reply.value).presentation)); return state.live; }
  } catch { /* the snapshot's last presentation stands in */ }
  return presented(obj(client.presentation));
}

/** The `chatCanvas` source. */
export async function chatCanvasView(client: T3Client, native: Native | null | undefined, args: ChatCanvasArgs): Promise<ChatCanvasView> {
  const state = stateOf(client), gesture = parseGesture(args.gesture);
  const player = visiblePlayer(client), threadKey = deviceThreadId(client);
  if (player) applyGesture(client, state, gesture, threadKey, miniStoreOf(client).get(threadKey) ?? player);
  else state.gesture = null;
  const current = player ? miniStoreOf(client).get(threadKey) ?? player : null;
  const width = Math.max(0, args.width);
  const card = args.detailsInline && inlineOpen(client, detailsKey(client)); // ThreadDetailsCard reports itself only while open inline
  // Mid-gesture the frames of its start stand; otherwise a layout with a player or the card follows the module's frames.
  const { frames, streams } = gesture && state.gesture ? state.live ?? presented(obj(client.presentation)) : await livePresentation(client, native, state, !!current || card || args.popoverOpen);
  const chat = rect(frames.chat), overlay = rect(frames.overlay);
  const container = { width, height: Math.max(0, (chat ? chat[3]! : args.viewportHeight) - CHAT_HEADER_HEIGHT) };
  // The lane's rem constants (ps-5, min-w-[40rem]) at the Interface font size, the root size.
  const lane = chatLaneMetrics(args.chatMax, clampInterfaceFontSize((client.local as { clientSettings?: { fontSizeInterface?: unknown } }).clientSettings?.fontSizeInterface));
  // ThreadDetailsCard: the preferred placement (no player), the measurements of the content shown, inline or in the
  // popover, keyed by thread and that placement's width; once the full height is known an open inline card reports
  // itself to the canvas, as tall as its full content (its 1 pt borders included).
  const preferred = args.wide ? resolveThreadDetailsCardLayout({ container, lane, frame: null }) : null;
  const heightsKey = `${detailsKey(client)}:${preferred ? preferred.width : 'popup'}`;
  const heights = keepHeights(state, heightsKey, card || args.popoverOpen ? measureThreadDetailsCard(frames) : {});
  const detailsCard = card && preferred && heights.full > 0 ? { left: preferred.x, right: preferred.x + preferred.width, bottom: preferred.y + Math.min(heights.full, preferred.height) } : null;
  let preview: ChatCanvasPreview | null = null, source: PreviewMiniPlayerSize | null = null;
  if (current) {
    source = current.source.kind === 'browser' ? floatingBrowserSize(client, previewMiniPlayerSourceKey(current.source))
      : resolveDeviceMiniPlayerSourceSize(current.source.platform, screenOf(streams, current.source.hostId, current.source.deviceId));
    preview = { key: previewMiniPlayerSourceKey(current.source), width: current.width, position: current.position, source, lastInteraction: current.lastInteraction };
  }
  const layout = resolveChatCanvasLayout({ container, preview, ...lane, composerHeight: args.overlaid && overlay ? overlay[3]! : 0, detailsCard });
  const placement = args.wide ? resolveThreadDetailsCardLayout({ container, lane, frame: layout.frame, overlapsDetailsCard: layout.overlapsDetailsCard }) : null;
  // ThreadDetailsCard's mode (`placement ? "inline" : "popover"`); a switch closes a popover opened before it.
  const popover = placement === null;
  if (state.popover !== null && state.popover !== popover) state.popoverEpoch++;
  state.popover = popover;
  const density = resolveThreadDetailsCardDensity(placement ? placement.height : Math.max(0, container.height - CHAT_HEADER_HEIGHT), heights);
  let mini = NO_MINI;
  if (current && layout.frame && preview && source) {
    state.shown = { threadKey, sourceKey: preview.key, frame: layout.frame, container, source };
    const radius = current.source.kind === 'browser' ? PREVIEW_MINI_PLAYER_CORNER_RADIUS : resolveDeviceMiniPlayerCornerRadius(current.source.platform as DevicePlatform, layout.frame);
    mini = { show: true, key: current.source.kind === 'browser' ? preview.key : deviceKey({ hostId: current.source.hostId, deviceId: current.source.deviceId }), x: layout.frame.x, top: CHAT_HEADER_HEIGHT + layout.frame.y,
      width: layout.frame.width, height: layout.frame.height, radius, pillInset: resolvePreviewMiniPlayerPillInset(radius) };
  } else state.shown = null;
  return {
    ready: true, containerWidth: container.width, containerHeight: container.height,
    left: layout.chat.left, width: layout.chat.width, insetEnd: layout.chat.insetEnd,
    overlapsChat: layout.overlapsChat, overlapsDetailsCard: layout.overlapsDetailsCard,
    cardShown: !card || placement !== null, cardHeight: placement ? placement.height : Math.max(0, container.height - THREAD_DETAILS_CARD_GAP * 2),
    popover, popoverEpoch: state.popoverEpoch, density,
    mini,
  };
}
