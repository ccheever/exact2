// T3 Code 9c161462c9, packages/client-runtime/src/preview/serverBrowserStream.ts
// (MIT; see LICENSE-T3). The server owns the page. Exact's binary WebSocket
// resource and image leaf carry its JPEGs; this state owns no native browser.
import { obj, str } from '../client/domain';
import type { SocketEvent, SocketHandle } from '../transport/transport-session';

export type StreamController = 'agent' | 'you' | 'another-viewer' | 'unclaimed';
export type StreamDialog = { type: string; message: string; defaultValue: string };
export type StreamControl = { canOperate: boolean; controller: StreamController; generation: number; dialog: StreamDialog | null };
export type StreamInput = { type: string; [field: string]: unknown };
export type StreamNotice = { type: 'clipboard'; text: string } | { type: 'popup'; tabId: string }
  | { type: 'download'; id: string; fileName: string; sizeBytes: number }
  | { type: 'fileChooser'; id: string; multiple: boolean; accept: string } | { type: 'fileChooserClosed'; id: string };
export type StreamFrame = {
  decoding: boolean; decodeSequence: number; attention: boolean; runtimeId: string; image: string; seq: number; connected: boolean; width: number; height: number;
  label: string; canOperate: boolean; controlled: boolean; anotherViewer: boolean;
  dialogType: string; dialogMessage: string; dialogDefault: string;
  pointerX: number; pointerY: number; pointerSequence: number; pointerClick: boolean;
  clipboard: string; clipboardSequence: number; chooserId: string; chooserMultiple: boolean; chooserAccept: string; transferError: string;
  refused: boolean; gone: boolean; setupNeed: string; setupCommand: string;
};
export const emptyStreamFrame = (): StreamFrame => ({ decoding: false, decodeSequence: 0, attention: false, runtimeId: '', image: '', seq: 0, connected: false, width: 0, height: 0,
  label: 'Connecting...', canOperate: false, controlled: false, anotherViewer: false,
  dialogType: '', dialogMessage: '', dialogDefault: '', pointerX: 0, pointerY: 0, pointerSequence: -1, pointerClick: false,
  clipboard: '', clipboardSequence: 0, chooserId: '', chooserMultiple: false, chooserAccept: '', transferError: '',
  refused: false, gone: false, setupNeed: '', setupCommand: '' });

export const streamControlLabel = (control: StreamControl | null): string => !control ? 'Connecting...'
  : !control.canOperate ? 'Read-only' : control.controller === 'you' ? 'You have control'
    : control.controller === 'agent' ? 'Agent has control' : control.controller === 'another-viewer' ? 'Another viewer has control' : 'Watching';

const MAX_FRAME_BYTES = 8 * 1024 * 1024;
const controllers = new Set(['agent', 'you', 'another-viewer', 'unclaimed']);
const finite = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);
/** One connection incarnation. Old socket callbacks cannot alter its successor. */
export class ServerBrowserStream {
  frame = emptyStreamFrame();
  control: StreamControl | null = null;
  notices: StreamNotice[] = [];
  retryAt = 0;
  retryDelay = 0;
  failures = 0;
  unauthorized = 0;
  private socket: SocketHandle | null = null;
  private url = '';
  private opened = false;
  private closed = false;
  private chooser = '';
  private painting = false;
  private waiting: ArrayBuffer | null = null;
  private jpeg: ArrayBuffer | null = null;
  private needsWork = false;

  begin(url: string): void {
    if (this.url === url) return;
    this.socket?.close(); this.socket = null; this.url = url; this.needsWork = false; this.opened = false; this.closed = false;
    this.notices = []; this.chooser = '';
    this.painting = false; this.waiting = null; this.jpeg = null;
    this.control = null; this.retryAt = 0; this.frame = { ...this.frame, connected: false, decoding: false, refused: false, gone: false, setupNeed: '', setupCommand: '' };
    this.project();
  }
  stop(): void {
    this.url = ''; this.needsWork = false; this.failures = this.unauthorized = this.retryAt = 0; this.socket?.close(); this.socket = null; this.control = null;
    this.notices = []; this.chooser = '';
    this.painting = false; this.waiting = null; this.jpeg = null;
    this.frame = { ...emptyStreamFrame(), seq: this.frame.seq + 1 };
  }
  event(url: string, event: SocketEvent, socket: SocketHandle | undefined, now: number): StreamFrame {
    if (!this.url || url !== this.url || this.closed) return this.frame;
    if (socket) this.socket = socket;
    if (event.type === 'open') { this.opened = true; this.frame.connected = true; }
    else if (event.type === 'message') {
      if (event.data instanceof ArrayBuffer) {
        // The protocol ACK counts receipt, independently of image decode.
        this.socket?.send('{"type":"ack"}'); this.failures = this.unauthorized = 0;
        if (event.data.byteLength > 0 && event.data.byteLength <= MAX_FRAME_BYTES) {
          if (this.painting) this.waiting = event.data;
          else this.paint(event.data);
        }
      } else if (typeof event.data === 'string') this.message(event.data);
    } else if (event.type === 'close' || event.type === 'error') {
      this.closed = true;
      this.socket = null; this.control = null; this.frame.connected = false;
      if (event.code === 4404) this.frame.gone = true;
      else if (event.code === 4503) {
        let setup: Record<string, unknown> = {};
        try { setup = obj(JSON.parse(event.reason ?? '')); } catch { /* the server's default setup command */ }
        this.frame.setupNeed = setup.need === 'libraries' ? 'libraries' : 'sandbox';
        this.frame.setupCommand = str(setup.command) || 'sudo npx t3 browser setup';
      } else if (event.code === 1008 || event.code === 4401 || (!this.opened && (event.code === 1006 || event.type === 'error'))) {
        this.unauthorized++; this.frame.refused = this.unauthorized >= 3;
        this.retryDelay = this.unauthorized === 1 ? 0 : 1000 * 2 ** (this.unauthorized - 1);
        this.retryAt = now + this.retryDelay;
      } else { this.retryDelay = Math.min(500 * 2 ** this.failures++, 10_000); this.retryAt = now + this.retryDelay; }
    }
    this.project(); return this.frame;
  }
  send(input: StreamInput): boolean {
    if (!this.socket || this.socket.readyState !== 1 || !this.control?.canOperate) return false;
    if (input.type !== 'takeControl' && this.control.controller !== 'you') return false;
    this.socket.send(JSON.stringify(input)); return true;
  }
  /** A native image's load/error ends one decode. Keep its source stable while
   * decoding; replacing it every packet would cancel slow decodes forever. */
  painted(image: string): StreamFrame {
    if (this.frame.image !== image || this.frame.decoding) return this.frame;
    this.painting = false;
    const next = this.waiting; this.waiting = null;
    if (next) this.paint(next);
    this.project(); return this.frame;
  }
  pendingJPEG(): { bytes: ArrayBuffer; sequence: number } | null { return this.jpeg ? { bytes: this.jpeg, sequence: this.frame.decodeSequence } : null; }
  decoded(sequence: number, image: string): StreamFrame {
    if (sequence !== this.frame.decodeSequence || !this.jpeg) return this.frame;
    this.jpeg = null; this.frame.image = image; this.frame.decoding = false; this.project(); return this.frame;
  }
  decodeFailed(sequence: number, message: string): StreamFrame {
    if (sequence !== this.frame.decodeSequence || !this.jpeg) return this.frame;
    this.jpeg = null; this.frame.decoding = false; this.frame.transferError = message;
    return this.painted(this.frame.image);
  }
  private paint(jpeg: ArrayBuffer): void {
    this.painting = true; this.jpeg = jpeg; this.frame.decoding = true; this.frame.decodeSequence++;
  }

  retry(): void { this.unauthorized = this.failures = 0; this.retryAt = 0; this.frame.refused = this.frame.gone = false; this.frame.setupNeed = this.frame.setupCommand = ''; }
  private message(text: string): void {
    let value: Record<string, unknown>;
    try { value = obj(JSON.parse(text)); } catch { return; }
    const { type } = value;
    if (type === 'viewport' && finite(value.width) && finite(value.height) && value.width > 0 && value.height > 0) {
      this.frame.width = value.width; this.frame.height = value.height; this.failures = 0;
    } else if (type === 'control' && typeof value.canOperate === 'boolean' && controllers.has(str(value.controller)) && finite(value.generation)) {
      const dialog = value.dialog === null ? null : obj(value.dialog);
      if (dialog !== null && !['type', 'message', 'defaultValue'].every(key => typeof dialog[key] === 'string')) return;
      this.control = { canOperate: value.canOperate, controller: value.controller as StreamController, generation: value.generation,
        dialog: dialog as StreamDialog | null };
    } else if (type === 'pointer' && (value.phase === 'move' || value.phase === 'click') && finite(value.x) && finite(value.y) && finite(value.sequence)) {
      this.frame.pointerX = value.x; this.frame.pointerY = value.y; this.frame.pointerSequence = value.sequence; this.frame.pointerClick = value.phase === 'click';
    } else if (type === 'clipboard' && typeof value.text === 'string') { this.frame.clipboard = value.text; this.frame.clipboardSequence++; }
    else if (type === 'popup' && typeof value.tabId === 'string') this.notice({ type, tabId: value.tabId });
    else if (type === 'download' && typeof value.id === 'string' && typeof value.fileName === 'string' && finite(value.sizeBytes)) this.notice({ type, id: value.id, fileName: value.fileName, sizeBytes: value.sizeBytes });
    else if (type === 'fileChooser' && typeof value.id === 'string' && typeof value.multiple === 'boolean' && typeof value.accept === 'string') {
      this.chooser = value.id; this.frame.chooserId = value.id; this.frame.chooserMultiple = value.multiple; this.frame.chooserAccept = value.accept;
    } else if (type === 'fileChooserClosed' && typeof value.id === 'string' && this.chooser === value.id) { this.chooser = ''; this.frame.chooserId = ''; }
  }
  drainNotices(): StreamNotice[] { const notices = this.notices.splice(0); if (notices.length) this.project(); return notices; }
  requestReconnect(): void { this.needsWork = true; this.project(); }
  transferError(message: string): void { this.frame.transferError = message; this.project(); }
  private notice(value: StreamNotice): void { this.notices.push(value); if (this.notices.length > 32) this.notices.shift(); }
  private project(): void {
    this.frame = { ...this.frame, attention: this.needsWork || this.notices.length > 0 || (this.retryAt > 0 && !this.frame.refused && !this.frame.gone && !this.frame.setupNeed), seq: this.frame.seq + 1, label: streamControlLabel(this.control), canOperate: this.control?.canOperate ?? false,
      controlled: this.control?.canOperate === true && this.control.controller === 'you', anotherViewer: this.control?.controller === 'another-viewer',
      dialogType: this.control?.dialog?.type ?? '', dialogMessage: this.control?.dialog?.message ?? '', dialogDefault: this.control?.dialog?.defaultValue ?? '' };
  }
}
